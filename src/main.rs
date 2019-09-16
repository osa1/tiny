#![cfg_attr(test, feature(test))]
#![feature(drain_filter)]
#![feature(ptr_offset_from)]

// #[macro_use]
// mod utils;

// mod cmd;
mod cmd_line_args;
mod config;
// mod conn;
// mod stream;
// mod wire;

use futures_util::stream::StreamExt;
use std::error::Error;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
// use std::time::Duration;
// use std::time::Instant;
// use time::Tm;

// use cmd::{parse_cmd, ParseCmdResult};
use cmd_line_args::{parse_cmd_line_args, CmdLineArgs};
// use conn::{Conn, ConnErr, ConnEv};
use libtiny_tui::{Colors, MsgSource, MsgTarget, TUIRet, TabStyle, TUI};
use term_input::{Event, Input};
// use wire::{Cmd, Msg, Pfx};

////////////////////////////////////////////////////////////////////////////////////////////////////

fn main() {
    let CmdLineArgs {
        servers: server_args,
        config_path,
    } = parse_cmd_line_args(std::env::args());
    let config_path = config_path.unwrap_or_else(config::get_default_config_path);
    if config_path.is_dir() {
        println!("The config path is a directory.");
        ::std::process::exit(1);
    } else if !config_path.is_file() {
        config::generate_default_config(&config_path);
    } else {
        match config::parse_config(&config_path) {
            Err(yaml_err) => {
                println!("Can't parse config file:");
                println!("{}", yaml_err);
                ::std::process::exit(1);
            }
            Ok(config::Config {
                servers,
                defaults,
                colors,
                // log_dir: _,
            }) => {
                let servers = if !server_args.is_empty() {
                    // connect only to servers that match at least one of
                    // the given patterns
                    servers
                        .into_iter()
                        .filter(|s| {
                            for server in &server_args {
                                if s.addr.contains(server) {
                                    return true;
                                }
                            }
                            false
                        })
                        .collect()
                } else {
                    servers
                };
                run(servers, defaults, colors, config_path)
            }
        }
    }
}

fn run(
    servers: Vec<config::Server>,
    defaults: config::Defaults,
    colors: Colors,
    config_path: PathBuf,
) {
    let mut tui = TUI::new(colors);

    // init "mentions" tab
    tui.new_server_tab("mentions");
    tui.add_client_msg(
        "Any mentions to you will be listed here.",
        &MsgTarget::Server {
            serv_name: "mentions",
        },
    );
    tui.draw();

    // One task for each client to handle IRC events
    // One task for stdin
    let executor = tokio::runtime::Runtime::new().unwrap();

    // A reference to the TUI will be shared with each connection event handler
    let tui = Arc::new(Mutex::new(tui));

    let mut clients: Vec<libtiny::Client> = Vec::with_capacity(servers.len());

    for server in servers.iter().cloned() {
        let server_info = libtiny::ServerInfo {
            addr: server.addr,
            port: server.port,
            pass: server.pass,
            hostname: server.hostname,
            realname: server.realname,
            nicks: server.nicks,
            auto_join: server.join,
            nickserv_ident: server.nickserv_ident,
        };

        let (client, mut rcv_ev) = libtiny::Client::new(server_info);
        let tui_clone = tui.clone();
        let client_clone = client.clone();

        // Spawn a task to handle connection events
        executor.spawn(async move {
            while let Some(ev) = rcv_ev.next().await {
                handle_conn_ev(&mut *tui_clone.lock().unwrap(), &client_clone, ev);
            }
        });

        clients.push(client);
    }

    // Spawn a task for input events
    executor.spawn(async move {
        let mut input = Input::new();
        while let Some(mb_ev) = input.next().await {
            match mb_ev {
                Err(io_err) => {
                    println!("term input error: {:?}", io_err);
                    // TODO: Close connections here
                    return;
                }
                Ok(ev) => {
                    handle_input_ev(&mut *tui.lock().unwrap(), &mut clients, ev);
                }
            }
            tui.lock().unwrap().draw();
        }
    });

    executor.shutdown_on_idle();
}

fn handle_conn_ev(tui: &mut TUI, client: &libtiny::Client, ev: libtiny::Event) {
    use libtiny::Event::*;
    match ev {
        Connecting => {
            tui.add_client_msg(
                "Connecting...",
                &MsgTarget::AllServTabs {
                    serv_name: client.get_serv_name(),
                },
            );
        }
        Connected => {
            tui.add_msg(
                "Connected.",
                time::now(),
                &MsgTarget::AllServTabs {
                    serv_name: client.get_serv_name(),
                },
            );
        }
        Disconnected => {
            tui.add_err_msg(
                &format!(
                    "Disconnected. Will try to reconnect in {} seconds.",
                    libtiny::RECONNECT_SECS
                ),
                time::now(),
                &MsgTarget::AllServTabs {
                    serv_name: client.get_serv_name(),
                },
            );
        }
        IoErr(err) => {
            tui.add_err_msg(
                &format!(
                    "Connection error: {}. Will try to reconnect in {} seconds.",
                    err.description(),
                    libtiny::RECONNECT_SECS
                ),
                time::now(),
                &MsgTarget::AllServTabs {
                    serv_name: client.get_serv_name(),
                },
            );
        }
        CantResolveAddr => {
            tui.add_err_msg(
                "Can't resolve address",
                time::now(),
                &MsgTarget::AllServTabs {
                    serv_name: client.get_serv_name(),
                },
            );
        }
        NickChange(new_nick) => {
            tui.set_nick(client.get_serv_name(), &new_nick);
        }
        Msg(msg) => {
            handle_msg(tui, client, msg);
        }
    }
}

fn handle_msg(tui: &mut TUI, client: &libtiny::Client, msg: libtiny::wire::Msg) {
    use libtiny::wire;
    use libtiny::wire::{Cmd, Pfx};

    let pfx = msg.pfx;
    let ts = time::now();
    match msg.cmd {
        Cmd::PRIVMSG {
            target,
            msg,
            is_notice,
            is_action,
        } => {
            let pfx = match pfx {
                Some(pfx) => pfx,
                None => {
                    // TODO: log this?
                    return;
                }
            };

            // sender to be shown in the UI
            let origin = match pfx {
                Pfx::Server(_) => client.get_serv_name(),
                Pfx::User { ref nick, .. } => nick,
            };

            match target {
                wire::MsgTarget::Chan(chan) => {
                    let tui_msg_target = MsgTarget::Chan {
                        serv_name: &client.get_serv_name(),
                        chan_name: &chan,
                    };
                    // highlight the message if it mentions us
                    if msg.find(&client.get_nick()).is_some() {
                        tui.add_privmsg_highlight(origin, &msg, ts, &tui_msg_target, is_action);
                        tui.set_tab_style(TabStyle::Highlight, &tui_msg_target);
                        let mentions_target = MsgTarget::Server {
                            serv_name: "mentions",
                        };
                        tui.add_msg(
                            &format!("{} in {}:{}: {}", origin, client.get_serv_name(), chan, msg),
                            ts,
                            &mentions_target,
                        );
                        tui.set_tab_style(TabStyle::Highlight, &mentions_target);
                    } else {
                        tui.add_privmsg(origin, &msg, ts, &tui_msg_target, is_action);
                        tui.set_tab_style(TabStyle::NewMsg, &tui_msg_target);
                    }
                }
                wire::MsgTarget::User(target) => {
                    let serv_name = client.get_serv_name();
                    let msg_target = {
                        match pfx {
                            Pfx::Server(_) => MsgTarget::Server { serv_name },
                            Pfx::User { ref nick, .. } => {
                                // show NOTICE messages in server tabs if we don't have a tab
                                // for the sender already (see #21)
                                if is_notice && !tui.does_user_tab_exist(serv_name, nick) {
                                    MsgTarget::Server { serv_name }
                                } else {
                                    MsgTarget::User { serv_name, nick }
                                }
                            }
                        }
                    };
                    tui.add_privmsg(origin, &msg, ts, &msg_target, is_action);
                    if target == client.get_nick() {
                        tui.set_tab_style(TabStyle::Highlight, &msg_target);
                    } else {
                        // not sure if this case can happen
                        tui.set_tab_style(TabStyle::NewMsg, &msg_target);
                    }
                }
            }
        }
        Cmd::JOIN { chan } => {
            let nick = match pfx {
                Some(Pfx::User { nick, .. }) => nick,
                _ => {
                    // TODO: log this?
                    return;
                }
            };

            let serv_name = client.get_serv_name();
            if nick == client.get_nick() {
                tui.new_chan_tab(serv_name, &chan);
            } else {
                let nick = drop_nick_prefix(&nick);
                let ts = Some(time::now());
                tui.add_nick(
                    nick,
                    ts,
                    &MsgTarget::Chan {
                        serv_name,
                        chan_name: &chan,
                    },
                );
                // Also update the private message tab if it exists
                // Nothing will be shown if the user already known to be online by the tab
                if tui.does_user_tab_exist(serv_name, nick) {
                    tui.add_nick(nick, ts, &MsgTarget::User { serv_name, nick });
                }
            }
        }
        Cmd::PART { chan, .. } => unimplemented!(),
        Cmd::QUIT { .. } => unimplemented!(),
        Cmd::NICK { nick } => unimplemented!(),
        Cmd::Reply { num: 433, .. } => unimplemented!(),
        Cmd::PING { .. } | Cmd::PONG { .. } => unimplemented!(),
        Cmd::ERROR { msg } => unimplemented!(),
        Cmd::TOPIC { chan, topic } => unimplemented!(),
        Cmd::CAP {
            client,
            subcommand,
            params,
        } => unimplemented!(),
        Cmd::AUTHENTICATE { .. } => unimplemented!(),
        Cmd::Reply { num: n, params } => unimplemented!(),
        Cmd::Other { cmd: _, params } => unimplemented!(),
    }
}

fn handle_input_ev(tui: &mut TUI, clients: &mut Vec<libtiny::Client>, ev: Event) {
    match tui.handle_input_event(ev) {
        TUIRet::Abort => {
            // TODO: abort here
        }
        TUIRet::KeyHandled => {}
        TUIRet::KeyIgnored(_) => {}
        TUIRet::EventIgnored(ev) => {
            // TODO: log this
        }
        TUIRet::Input { msg, from } => {
            // We know msg has at least one character as the TUI won't accept it otherwise.
            if msg[0] == '/' {
                let cmd_str: String = (&msg[1..]).into_iter().cloned().collect();
                handle_cmd(tui, clients, &cmd_str)
            } else {
                let msg_str: String = msg.into_iter().collect();
                send_msg(tui, clients, &from, msg_str, false)
            }
        }
        TUIRet::Lines { lines, from } => {
            for line in lines.into_iter() {
                send_msg(tui, clients, &from, line, false)
            }
        }
    }
}

fn handle_cmd(tui: &mut TUI, clients: &mut Vec<libtiny::Client>, cmd: &str) {
    unimplemented!()
}

fn send_msg(
    tui: &mut TUI,
    clients: &mut Vec<libtiny::Client>,
    src: &MsgSource,
    msg: String,
    ctcp_action: bool,
) {
    if src.serv_name() == "mentions" {
        tui.add_client_err_msg(
            "Use `/connect <server>` to connect to a server",
            &MsgTarget::CurrentTab,
        );
        return;
    }

    let client = clients
        .iter_mut()
        .find(|client| client.get_serv_name() == src.serv_name())
        .unwrap();

    // TODO: For errors:
    //
    // tui.add_client_err_msg(
    //     &format!("Can't find server: {}", serv_name),
    //     &MsgTarget::CurrentTab,
    // );

    // `tui_target`: Where to show the message on TUI
    // `msg_target`: Actual PRIVMSG target to send to the server
    // `serv_name`: Server name to find connection in `clients`
    let (tui_target, msg_target, serv_name) = {
        match src {
            MsgSource::Serv { ref serv_name } => {
                // we don't split raw messages to 512-bytes long chunks
                client.raw_msg(msg);
                return;
            }

            MsgSource::Chan {
                ref serv_name,
                ref chan_name,
            } => (
                MsgTarget::Chan {
                    serv_name,
                    chan_name,
                },
                chan_name,
                serv_name,
            ),

            MsgSource::User {
                ref serv_name,
                ref nick,
            } => {
                let msg_target = if nick.eq_ignore_ascii_case("nickserv")
                    || nick.eq_ignore_ascii_case("chanserv")
                {
                    MsgTarget::Server { serv_name }
                } else {
                    MsgTarget::User { serv_name, nick }
                };
                (msg_target, nick, serv_name)
            }
        }
    };

    let ts = time::now();
    let extra_len = msg_target.len()
        + if ctcp_action {
            9 // "\0x1ACTION \0x1".len()
        } else {
            0
        };
    for msg in client.split_privmsg(extra_len, &msg) {
        client.privmsg(msg_target, msg, ctcp_action);
        tui.add_privmsg(&client.get_nick(), msg, ts, &tui_target, ctcp_action);
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

/*
impl<'poll> Tiny<'poll> {
    fn handle_cmd(&mut self, poll: &'poll Poll, src: MsgSource, msg: &str) {
        match parse_cmd(msg) {
            ParseCmdResult::Ok { cmd, rest } => {
                (cmd.cmd_fn)(rest, poll, self, src);
            }
            // ParseCmdResult::Ambiguous(vec) => {
            //     self.tui.add_client_err_msg(
            //         &format!("Unsupported command: \"/{}\"", msg),
            //         &MsgTarget::CurrentTab,
            //     );
            //     self.tui.add_client_err_msg(
            //         &format!("Did you mean one of {:?} ?", vec),
            //         &MsgTarget::CurrentTab,
            //     );
            // },
            ParseCmdResult::Unknown => self.tui.add_client_err_msg(
                &format!("Unsupported command: \"/{}\"", msg),
                &MsgTarget::CurrentTab,
            ),
        }
    }

    fn part(&mut self, serv_name: &str, chan: &str) {
        let conn = find_conn(&mut self.conns, serv_name).unwrap();
        conn.part(chan);
    }

    fn send_msg(&mut self, _: &MsgSource, _: &str, _: bool) {
    }

    ////////////////////////////////////////////////////////////////////////////

    fn handle_conn_ev(&mut self, conn_idx: usize, ev: ConnEv) {
        match ev {
            ConnEv::Connected => {
                self.tui.add_msg(
                    "Connected.",
                    time::now(),
                    &MsgTarget::AllServTabs {
                        serv_name: self.conns[conn_idx].get_serv_name(),
                    },
                );
            }
            ConnEv::Disconnected => {
                let conn = &mut self.conns[conn_idx];
                let target = MsgTarget::AllServTabs {
                    serv_name: conn.get_serv_name(),
                };
                self.tui.add_err_msg(
                    &format!(
                        "Disconnected. Will try to reconnect in {} seconds.",
                        conn::RECONNECT_TICKS
                    ),
                    time::now(),
                    &target,
                );
                self.tui.clear_nicks(&target);
            }
            ConnEv::WantReconnect => {
                let conn = &mut self.conns[conn_idx];
                self.tui.add_client_msg(
                    "Connecting...",
                    &MsgTarget::AllServTabs {
                        serv_name: conn.get_serv_name(),
                    },
                );
                match conn.reconnect(None) {
                    Ok(()) => {}
                    Err(err) => {
                        self.tui.add_err_msg(
                            &reconnect_err_msg(&err),
                            time::now(),
                            &MsgTarget::AllServTabs {
                                serv_name: conn.get_serv_name(),
                            },
                        );
                    }
                }
            }
            ConnEv::Err(err) => {
                let conn = &mut self.conns[conn_idx];
                conn.enter_disconnect_state();
                self.tui.add_err_msg(
                    &reconnect_err_msg(&err),
                    time::now(),
                    &MsgTarget::AllServTabs {
                        serv_name: conn.get_serv_name(),
                    },
                );
            }
            ConnEv::Msg(msg) => {
                self.handle_msg(conn_idx, msg, time::now());
            }
            ConnEv::NickChange(new_nick) => {
                let conn = &self.conns[conn_idx];
                self.tui.set_nick(conn.get_serv_name(), &new_nick);
            }
        }
    }

    fn handle_msg(&mut self, conn_idx: usize, msg: Msg, ts: Tm) {
        let conn = &self.conns[conn_idx];
        let pfx = msg.pfx;
        match msg.cmd {

            Cmd::PART { chan, .. } => match pfx {
                Some(Pfx::User { nick, .. }) => {
                    if nick != conn.get_nick() {
                        let serv_name = conn.get_serv_name();
                        self.tui.remove_nick(
                            &nick,
                            Some(time::now()),
                            &MsgTarget::Chan {
                                serv_name,
                                chan_name: &chan,
                            },
                        );
                    }
                }
                _pfx => {
                    // self.logger
                    //     .get_debug_logs()
                    //     .write_line(format_args!("Weird PART message pfx {:?}", pfx));
                }
            },

            Cmd::QUIT { .. } => match pfx {
                Some(Pfx::User { ref nick, .. }) => {
                    let serv_name = conn.get_serv_name();
                    self.tui.remove_nick(
                        nick,
                        Some(time::now()),
                        &MsgTarget::AllUserTabs { serv_name, nick },
                    );
                }
                _pfx => {
                    // self.logger
                    //     .get_debug_logs()
                    //     .write_line(format_args!("Weird QUIT message pfx {:?}", pfx));
                }
            },

            Cmd::NICK { nick } => match pfx {
                Some(Pfx::User {
                    nick: ref old_nick, ..
                }) => {
                    let serv_name = conn.get_serv_name();
                    self.tui.rename_nick(
                        old_nick,
                        &nick,
                        time::now(),
                        &MsgTarget::AllUserTabs {
                            serv_name,
                            nick: old_nick,
                        },
                    );
                }
                _pfx => {
                    // self.logger
                    //     .get_debug_logs()
                    //     .write_line(format_args!("Weird NICK message pfx {:?}", pfx));
                }
            },

            Cmd::Reply { num: 433, .. } => {
                // ERR_NICKNAMEINUSE
                if conn.is_nick_accepted() {
                    // Nick change request from user failed. Just show an error message.
                    self.tui.add_err_msg(
                        "Nickname is already in use",
                        time::now(),
                        &MsgTarget::AllServTabs {
                            serv_name: conn.get_serv_name(),
                        },
                    );
                }
            }

            Cmd::PING { .. } | Cmd::PONG { .. } =>
                // ignore
                {}

            Cmd::ERROR { ref msg } => {
                let serv_name = conn.get_serv_name();
                self.tui
                    .add_err_msg(msg, time::now(), &MsgTarget::AllServTabs { serv_name });
            }

            Cmd::TOPIC {
                ref chan,
                ref topic,
            } => {
                self.tui.show_topic(
                    topic,
                    time::now(),
                    &MsgTarget::Chan {
                        serv_name: conn.get_serv_name(),
                        chan_name: chan,
                    },
                );
            }

            Cmd::CAP {
                client: _,
                ref subcommand,
                ref params,
            } => {
                match subcommand.as_ref() {
                    "NAK" => {
                        if params.iter().any(|cap| cap.as_str() == "sasl") {
                            let msg_target = MsgTarget::Server {
                                serv_name: conn.get_serv_name(),
                            };
                            self.tui.add_err_msg(
                                "Server rejected using SASL authenication capability",
                                time::now(),
                                &msg_target,
                            );
                        }
                    }
                    "LS" => {
                        if !params.iter().any(|cap| cap.as_str() == "sasl") {
                            let msg_target = MsgTarget::Server {
                                serv_name: conn.get_serv_name(),
                            };
                            self.tui.add_err_msg(
                                "Server does not support SASL authenication",
                                time::now(),
                                &msg_target,
                            );
                        }
                    }
                    "ACK" => {}
                    _cmd => {
                        // self.logger
                        //     .get_debug_logs()
                        //     .write_line(format_args!("CAP subcommand {} is not handled", cmd));
                    }
                };
            }

            Cmd::AUTHENTICATE { .. } =>
                // ignore
                {}

            Cmd::Reply { num: n, params } => {
                if n <= 003 /* RPL_WELCOME, RPL_YOURHOST, RPL_CREATED */
                        || n == 251 /* RPL_LUSERCLIENT */
                        || n == 255 /* RPL_LUSERME */
                        || n == 372 /* RPL_MOTD */
                        || n == 375 /* RPL_MOTDSTART */
                        || n == 376
                /* RPL_ENDOFMOTD */
                {
                    debug_assert_eq!(params.len(), 2);
                    let msg = &params[1];
                    self.tui.add_msg(
                        msg,
                        time::now(),
                        &MsgTarget::Server {
                            serv_name: conn.get_serv_name(),
                        },
                    );
                } else if n == 4 // RPL_MYINFO
                        || n == 5 // RPL_BOUNCE
                        || (n >= 252 && n <= 254)
                /* RPL_LUSEROP, RPL_LUSERUNKNOWN, */
                /* RPL_LUSERCHANNELS */
                {
                    let msg = params.into_iter().collect::<Vec<String>>().join(" ");
                    self.tui.add_msg(
                        &msg,
                        time::now(),
                        &MsgTarget::Server {
                            serv_name: conn.get_serv_name(),
                        },
                    );
                } else if n == 265 || n == 266 || n == 250 {
                    let msg = &params[params.len() - 1];
                    self.tui.add_msg(
                        msg,
                        time::now(),
                        &MsgTarget::Server {
                            serv_name: conn.get_serv_name(),
                        },
                    );
                }
                // RPL_TOPIC
                else if n == 332 {
                    // FIXME: RFC 2812 says this will have 2 arguments, but freenode
                    // sends 3 arguments (extra one being our nick).
                    assert!(params.len() == 3 || params.len() == 2);
                    let chan = &params[params.len() - 2];
                    let topic = &params[params.len() - 1];
                    self.tui.show_topic(
                        topic,
                        time::now(),
                        &MsgTarget::Chan {
                            serv_name: conn.get_serv_name(),
                            chan_name: chan,
                        },
                    );
                }
                // RPL_NAMREPLY: List of users in a channel
                else if n == 353 {
                    let chan = &params[2];
                    let chan_target = MsgTarget::Chan {
                        serv_name: conn.get_serv_name(),
                        chan_name: chan,
                    };

                    for nick in params[3].split_whitespace() {
                        self.tui
                            .add_nick(drop_nick_prefix(nick), None, &chan_target);
                    }
                }
                // RPL_ENDOFNAMES: End of NAMES list
                else if n == 366 {
                }
                // RPL_UNAWAY or RPL_NOWAWAY
                else if n == 305 || n == 306 {
                    let msg = &params[1];
                    self.tui.add_client_msg(
                        msg,
                        &MsgTarget::AllServTabs {
                            serv_name: conn.get_serv_name(),
                        },
                    );
                }
                // ERR_NOSUCHNICK
                else if n == 401 {
                    let nick = &params[1];
                    let msg = &params[2];
                    let serv_name = conn.get_serv_name();
                    self.tui
                        .add_client_msg(msg, &MsgTarget::User { serv_name, nick });
                // RPL_AWAY
                } else if n == 301 {
                    let serv_name = conn.get_serv_name();
                    let nick = &params[1];
                    let msg = &params[2];
                    self.tui.add_client_msg(
                        &format!("{} is away: {}", nick, msg),
                        &MsgTarget::User { serv_name, nick },
                    );
                } else {
                    match pfx {
                        Some(Pfx::Server(msg_serv_name)) => {
                            let conn_serv_name = conn.get_serv_name();
                            let msg_target = MsgTarget::Server {
                                serv_name: conn_serv_name,
                            };
                            self.tui.add_privmsg(
                                &msg_serv_name,
                                &params.join(" "),
                                time::now(),
                                &msg_target,
                                false,
                            );
                            self.tui.set_tab_style(TabStyle::NewMsg, &msg_target);
                        }
                        _pfx => {
                            // add everything else to debug file
                            // self.logger.get_debug_logs().write_line(format_args!(
                            //     "Ignoring numeric reply msg:\nPfx: {:?}, num: {:?}, args: {:?}",
                            //     pfx, n, params
                            // ));
                        }
                    }
                }
            }

            Cmd::Other { cmd: _, params } => match pfx {
                Some(Pfx::Server(msg_serv_name)) => {
                    let conn_serv_name = conn.get_serv_name();
                    let msg_target = MsgTarget::Server {
                        serv_name: conn_serv_name,
                    };
                    self.tui.add_privmsg(
                        &msg_serv_name,
                        &params.join(" "),
                        time::now(),
                        &msg_target,
                        false,
                    );
                    self.tui.set_tab_style(TabStyle::NewMsg, &msg_target);
                }
                _pfx => {
                    // self.logger.get_debug_logs().write_line(format_args!(
                    //     "Ignoring msg:\nPfx: {:?}, msg: {} :{}",
                    //     pfx,
                    //     cmd,
                    //     params.join(" "),
                    // ));
                }
            },
        }
    }
}
*/

/// Nicks may have prefixes, indicating it is a operator, founder, or
/// something else.
/// Channel Membership Prefixes:
/// http://modern.ircdocs.horse/#channel-membership-prefixes
///
/// Returns the nick without prefix
fn drop_nick_prefix(nick: &str) -> &str {
    static PREFIXES: [char; 5] = ['~', '&', '@', '%', '+'];

    if PREFIXES.contains(&nick.chars().nth(0).unwrap()) {
        &nick[1..]
    } else {
        nick
    }
}
