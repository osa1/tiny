#![cfg_attr(test, feature(test))]
#![feature(alloc_system)]
#![feature(ascii_ctype)]
#![feature(offset_to)]

extern crate alloc_system;
extern crate libc;
extern crate mio;
extern crate net2;
extern crate time;
extern crate serde;
extern crate serde_yaml;
#[macro_use] extern crate serde_derive;

extern crate term_input;
extern crate termbox_simple;

#[macro_use]
mod utils;

mod conn;
mod logger;
mod wire;
pub mod config;
pub mod trie;
pub mod tui;

use mio::Events;
use mio::Poll;
use mio::PollOpt;
use mio::Ready;
use mio::Token;
use mio::unix::EventedFd;
use std::ascii::AsciiExt;
use std::fs::File;
use std::io::Read;
use std::io::Write;
use std::os::unix::io::RawFd;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

use conn::{Conn, ConnEv};
use logger::Logger;
use term_input::Input;
use tui::tabbed::MsgSource;
use tui::tabbed::TabStyle;
use tui::{TUI, TUIRet, MsgTarget, Timestamp};
use wire::{Cmd, Msg, Pfx};

////////////////////////////////////////////////////////////////////////////////////////////////////

pub fn run() {
    let config_path = config::get_config_path();
    if !config_path.is_file() {
        generate_default_config();
    } else {
        let contents = {
            let mut str = String::new();
            let mut file = File::open(config_path).unwrap();
            file.read_to_string(&mut str).unwrap();
            str
        };

        match serde_yaml::from_str(&contents) {
            Err(yaml_err) => {
                println!("Can't parse config file:");
                println!("{}", yaml_err);
                ::std::process::exit(1);
            },
            Ok(config::Config { servers, defaults, log_dir }) => {
                Tiny::run(servers, defaults, log_dir)
            }
        }
    }
}

fn generate_default_config() {
    let config_path = config::get_config_path();
    {
        let mut file = File::create(&config_path).unwrap();
        file.write_all(config::get_default_config_yaml().as_bytes()).unwrap();
    }
    println!("\
tiny couldn't find a config file at {0:?}, and created a config file with defaults.
You may want to edit {0:?} before re-running tiny.", config_path);

}

////////////////////////////////////////////////////////////////////////////////////////////////////

struct Tiny {
    conns: Vec<Conn>,
    defaults: config::Defaults,
    servers: Vec<config::Server>,
    tui: TUI,
    input_ev_handler: Input,
    logger: Logger,
}

static STDIN_TOKEN: Token = Token(libc::STDIN_FILENO as usize);

fn register_fd(poll: &Poll, fd: RawFd) {
    poll.register(
        &EventedFd(&fd),
        Token(fd as usize),
        Ready::readable(),
        PollOpt::level()).unwrap();
}

fn deregister_fd(poll: &Poll, fd: RawFd) {
    poll.deregister(&EventedFd(&fd)).unwrap();
}

impl Tiny {
    pub fn run(servers: Vec<config::Server>, defaults: config::Defaults, log_dir: String) {
        let poll = Poll::new().unwrap();

        register_fd(&poll, libc::STDIN_FILENO);

        let mut conns = Vec::with_capacity(servers.len());
        for server in servers.iter().cloned() {
            let conn = Conn::from_server(server);
            let fd = conn.get_raw_fd();
            register_fd(&poll, fd);
            conns.push(conn);
        }

        let mut tiny = Tiny {
            conns: conns,
            defaults: defaults,
            servers: servers,
            tui: TUI::new(),
            input_ev_handler: Input::new(),
            logger: Logger::new(PathBuf::from(log_dir)),
        };

        tiny.init_mentions_tab();
        tiny.tui.draw();

        let mut last_tick = Instant::now();
        let mut events = Events::with_capacity(10);
        'mainloop:
        loop {
            // FIXME this will sometimes miss the tick deadline
            match poll.poll(&mut events, Some(Duration::from_secs(1))) {
                Err(_) => {
                    // usually SIGWINCH, which is caught by term_input
                    if tiny.handle_stdin(&poll) {
                        break 'mainloop;
                    }
                }
                Ok(_) => {
                    for event in events.iter() {
                        let token = event.token();
                        if token == STDIN_TOKEN {
                            if tiny.handle_stdin(&poll) {
                                break 'mainloop;
                            }
                        } else {
                            match find_token_conn_idx(&tiny.conns, token) {
                                None => {
                                    tiny.logger.get_debug_logs().write_line(
                                        format_args!("BUG: Can't find Token in conns: {:?}", event.token()));
                                }
                                Some(conn_idx) => {
                                    tiny.handle_socket(&poll, conn_idx);
                                }
                            }
                        }
                    }

                    if last_tick.elapsed() >= Duration::from_secs(1) {
                        for conn_idx in 0 .. tiny.conns.len() {
                            let mut evs = Vec::with_capacity(2);
                            {
                                let conn = &mut tiny.conns[conn_idx];
                                conn.tick(&mut evs, tiny.logger.get_debug_logs());
                            }
                            tiny.handle_conn_evs(&poll, conn_idx, evs);
                        }
                        last_tick = Instant::now();
                    }
                }
            }

            tiny.tui.draw();
        }
    }

    fn init_mentions_tab(&mut self) {
        self.tui.new_server_tab("mentions");
        self.tui.add_client_msg(
            "Any mentions to you will be listed here.",
            &MsgTarget::Server { serv_name: "mentions" });
    }

    fn handle_stdin(&mut self, poll: &Poll) -> bool {
        let mut ev_buffer = Vec::with_capacity(10);
        let mut abort = false;
        self.input_ev_handler.read_input_events(&mut ev_buffer);
        for ev in ev_buffer.drain(..) {
            match self.tui.handle_input_event(ev) {
                TUIRet::Abort => {
                    abort = true;
                }
                TUIRet::Input { msg, from } => {
                    self.logger.get_debug_logs().write_line(
                    format_args!("Input source: {:#?}, msg: {}",
                                 from, msg.iter().cloned().collect::<String>()));

                    // We know msg has at least one character as the TUI won't accept it otherwise.
                    if msg[0] == '/' {
                        let msg_str: String = (&msg[ 1 .. ]).into_iter().cloned().collect();
                        self.handle_command(poll, from, &msg_str);
                    } else {
                        self.send_msg(from, &msg.into_iter().collect::<String>());
                    }
                }
                TUIRet::KeyHandled => {}
                // TUIRet::KeyIgnored(_) | TUIRet::EventIgnored(_) => {}
                ev => {
                    self.logger.get_debug_logs().write_line(
                        format_args!("Ignoring event: {:?}", ev));
                }
            }
        }
        abort
    }

    fn handle_command(&mut self, poll: &Poll, src: MsgSource, msg: &str) {
        let words: Vec<&str> = msg.split_whitespace().into_iter().collect();

        if words[0] == "connect" && words.len() == 2 {
            self.connect(poll, src, words[1]);
        }

        else if words[0] == "connect" && words.len() == 1 {
            if !self.reconnect(poll, src.serv_name()) {
                self.logger.get_debug_logs().write_line(
                    format_args!("Can't reconnect to {}", src.serv_name()));
            }
        }

        else if words[0] == "join" {
            self.join(src, words[1]);
        }

        else if words[0] == "msg" {
            // need to find index of the third word
            let mut word_indices = utils::split_whitespace_indices(&msg);
            word_indices.next(); // "/msg"
            word_indices.next(); // target
            if let Some(msg_begins) = word_indices.next() {
                let msg = &msg[msg_begins ..];
                let serv = src.serv_name();
                self.send_msg(
                    MsgSource::User {
                        serv_name: serv.to_owned(),
                        nick: words[1].to_owned()
                    }, msg);
            } else {
                self.tui.add_client_err_msg(
                    "/msg usage: /msg target message",
                    &MsgTarget::CurrentTab);
            }
        }

        else if words[0] == "close" {
            match src {
                MsgSource::Serv { ref serv_name } if serv_name == "mentions" => {
                    // ignore
                }
                MsgSource::Serv { serv_name } => {
                    self.tui.close_server_tab(&serv_name);
                    self.disconnect(poll, &serv_name);
                }
                MsgSource::Chan { serv_name, chan_name } => {
                    self.tui.close_chan_tab(&serv_name, &chan_name);
                    self.part(&serv_name, &chan_name);
                }
                MsgSource::User { serv_name, nick } => {
                    self.tui.close_user_tab(&serv_name, &nick);
                }
            }
        }

        else {
            self.tui.add_client_err_msg(
                &format!("Unsupported command: {}", words[0]), &MsgTarget::CurrentTab);
        }
    }

    fn connect(&mut self, poll: &Poll, src: MsgSource, serv_addr: &str) {

        fn split_port(s: &str) -> Option<(&str, &str)> {
            s.find(':').map(|split| (&s[ 0 .. split ], &s[ split + 1 .. ]))
        }

        let (serv_name, serv_port) = {
            match split_port(serv_addr) {
                None => {
                    self.tui.add_client_err_msg(
                        "connect: Need a <host>:<port>",
                        &MsgTarget::CurrentTab);
                    return;
                }
                Some((serv_name, serv_port)) => {
                    match serv_port.parse::<u16>() {
                        Err(err) => {
                            self.tui.add_client_err_msg(
                                &format!("connect: Can't parse port {}: {}", serv_port, err),
                                &MsgTarget::CurrentTab);
                            return;
                        }
                        Ok(serv_port) => {
                            (serv_name, serv_port)
                        }
                    }
                }
            }
        };

        let mut reconnect = false; // borrowchk workaround
        // see if we already have a Conn for this server
        if let Some(conn) = find_conn(&mut self.conns, serv_name) {
            deregister_fd(poll, conn.get_raw_fd());
            conn.reconnect(Some((serv_name, serv_port)));
            let fd = conn.get_raw_fd();
            register_fd(poll, fd);
            reconnect = true;
        }

        if reconnect {
            self.tui.add_client_msg(
                "Connecting...",
                &MsgTarget::AllServTabs { serv_name: serv_name });
            return;
        }

        // otherwise create a new Conn, tab etc.
        self.tui.new_server_tab(serv_name);
        self.tui.add_client_msg("Connecting...",
                                &MsgTarget::Server { serv_name: serv_name });

        let new_conn = {
            match find_conn(&mut self.conns, src.serv_name()) {
                Some(current_conn) =>
                    Conn::from_conn(current_conn, serv_name, serv_port),
                None => {
                    self.logger.get_debug_logs().write_line(
                        format_args!("connecting to {} {}", serv_name, serv_port));
                    Conn::from_server(config::Server {
                        addr: serv_name.to_owned(),
                        port: serv_port,
                        hostname: self.defaults.hostname.clone(),
                        realname: self.defaults.realname.clone(),
                        nicks: self.defaults.nicks.clone(),
                        auto_cmds: self.defaults.auto_cmds.clone(),
                    })
                }
            }
        };
        let fd = new_conn.get_raw_fd();
        self.conns.push(new_conn);
        register_fd(poll, fd);
    }

    fn disconnect(&mut self, poll: &Poll, serv_name: &str) {
        let conn_idx = find_conn_idx(&mut self.conns, serv_name).unwrap();
        let conn = self.conns.remove(conn_idx);
        deregister_fd(poll, conn.get_raw_fd());
        // just drop the conn
    }

    fn reconnect(&mut self, poll: &Poll, serv_name: &str) -> bool {
        self.tui.add_client_msg(
            "Reconnecting...",
            &MsgTarget::AllServTabs { serv_name: serv_name });
        match find_conn(&mut self.conns, serv_name) {
            Some(conn) => {
                deregister_fd(poll, conn.get_raw_fd());
                conn.reconnect(None);
                let fd = conn.get_raw_fd();
                register_fd(poll, fd);
                true
            }
            None => false
        }
    }

    fn join(&mut self, src: MsgSource, chan: &str) {
        match find_conn(&mut self.conns, src.serv_name()) {
            Some(conn) => {
                conn.join(chan);
                return;
            }
            None => {
                // drop the borrowed self and run next statement
                // rustc is too dumb to figure that None can't borrow.
            }
        }

        self.tui.add_client_err_msg(
            &format!("Can't JOIN: Not connected to server {}", src.serv_name()),
            &MsgTarget::CurrentTab);
    }

    fn part(&mut self, serv_name: &str, chan_name: &str) {
        let conn = find_conn(&mut self.conns, serv_name).unwrap();
        wire::part(conn, chan_name).unwrap();
    }

    fn send_msg(&mut self, from: MsgSource, msg: &str) {
        match from {
            MsgSource::Serv { ref serv_name } => {
                if serv_name == "mentions" {
                    self.tui.add_client_err_msg("Use `/connect <server>` to connect to a server",
                                                &MsgTarget::CurrentTab);

                } else {
                    self.tui.add_client_err_msg("Can't send PRIVMSG to a server.",
                                                &MsgTarget::CurrentTab);
                }
            },

            MsgSource::Chan { serv_name, chan_name } => {
                let conn = find_conn(&mut self.conns, &serv_name).unwrap();
                conn.privmsg(&chan_name, msg);
                self.tui.add_privmsg(conn.get_nick(), msg,
                                     Timestamp::now(),
                                     &MsgTarget::Chan { serv_name: &serv_name,
                                                        chan_name: &chan_name });
            },

            MsgSource::User { serv_name, nick } => {
                let conn = find_conn(&mut self.conns, &serv_name).unwrap();
                conn.privmsg(&nick, msg);
                let msg_target = if nick.eq_ignore_ascii_case("nickserv") {
                    MsgTarget::Server { serv_name: &serv_name }
                } else {
                    MsgTarget::User { serv_name: &serv_name, nick: &nick }
                };
                self.tui.add_privmsg(conn.get_nick(), msg, Timestamp::now(), &msg_target);
            }
        }
    }

    ////////////////////////////////////////////////////////////////////////////

    fn handle_socket(&mut self, poll: &Poll, conn_idx: usize) {
        let mut evs = Vec::with_capacity(2);
        {
            let mut conn = &mut self.conns[conn_idx];
            conn.read_incoming_msg(&mut evs, &mut self.logger);
        }
        self.handle_conn_evs(poll, conn_idx, evs);
    }

    fn handle_conn_evs(&mut self, poll: &Poll, conn_idx: usize, evs: Vec<ConnEv>) {
        for ev in evs.into_iter() {
            match ev {
                ConnEv::Connected => {
                    let mut serv_auto_cmds = None;
                    {
                        let conn = &self.conns[conn_idx];
                        let conn_name = conn.get_serv_name();
                        for server in &self.servers {
                            if server.addr == conn_name {
                                // redundant clone() here because of aliasing
                                serv_auto_cmds = Some((server.addr.clone(), server.auto_cmds.clone()));
                                break;
                            }
                        }
                    }
                    if let Some((serv_name, auto_cmds)) = serv_auto_cmds {
                        for cmd in auto_cmds.iter() {
                            self.handle_command(poll, MsgSource::Serv { serv_name: serv_name.to_owned() }, cmd);
                        }
                    }
                }
                ConnEv::Disconnected => {
                    let conn = &mut self.conns[conn_idx];
                    self.tui.add_err_msg(
                        &format!("Disconnected. Will try to reconnect in {} seconds.",
                                 conn::RECONNECT_TICKS),
                        Timestamp::now(),
                        &MsgTarget::AllServTabs { serv_name: conn.get_serv_name() });
                    deregister_fd(poll, conn.get_raw_fd());

                }
                ConnEv::WantReconnect => {
                    let mut conn = &mut self.conns[conn_idx];
                    self.tui.add_client_msg(
                        "Connecting...",
                        &MsgTarget::AllServTabs { serv_name: conn.get_serv_name() });
                    conn.reconnect(None);
                    let fd = conn.get_raw_fd();
                    register_fd(poll, fd);
                }
                ConnEv::Err(err) => {
                    // TODO: Some of these errors should not cause a disconnect,
                    // e.g. EAGAIN, EWOULDBLOCK ...
                    let mut conn = &mut self.conns[conn_idx];
                    conn.enter_disconnect_state();
                    self.tui.add_err_msg(
                        &format!("{:?}", err),
                        Timestamp::now(),
                        &MsgTarget::AllServTabs { serv_name: conn.get_serv_name() });
                    deregister_fd(poll, conn.get_raw_fd());
                }
                ConnEv::Msg(msg) => {
                    self.handle_msg(conn_idx, msg, Timestamp::now());
                }
            }
        }
    }

    fn handle_msg(&mut self, conn_idx: usize, msg: Msg, ts: Timestamp) {
        let conn = &self.conns[conn_idx];
        let pfx = match msg.pfx {
            None => { return; /* TODO: log this */ }
            Some(pfx) => pfx
        };
        match msg.cmd {

            Cmd::PRIVMSG { target, msg } | Cmd::NOTICE { target, msg } => {
                let origin = match pfx {
                    Pfx::Server(_) => conn.get_serv_name(),
                    Pfx::User { ref nick, .. } => nick,
                };
                match target {
                    wire::MsgTarget::Chan(chan) => {
                        self.logger.get_chan_logs(&conn.get_serv_name(), &chan).write_line(
                            format_args!("PRIVMSG: {}", msg));
                        let msg_target = MsgTarget::Chan {
                            serv_name: conn.get_serv_name(),
                            chan_name: &chan,
                        };
                        // highlight the message if it mentions us
                        if msg.find(conn.get_nick()).is_some() {
                            self.tui.add_privmsg_higlight(origin, &msg, ts, &msg_target);
                            self.tui.set_tab_style(TabStyle::Highlight, &msg_target);
                            let mentions_target = MsgTarget::Server { serv_name: "mentions" };
                            self.tui.add_msg(
                                &format!("{} in {}:{}: {}",
                                         origin, conn.get_serv_name(), chan, msg),
                                ts,
                                &mentions_target);
                            self.tui.set_tab_style(TabStyle::Highlight, &mentions_target);
                        } else {
                            self.tui.add_privmsg(origin, &msg, ts, &msg_target);
                            self.tui.set_tab_style(TabStyle::NewMsg, &msg_target);
                        }
                    }
                    wire::MsgTarget::User(target) => {
                        let serv_name = conn.get_serv_name();
                        let msg_target = match pfx {
                            Pfx::Server(_) =>
                                MsgTarget::Server { serv_name: serv_name },
                            Pfx::User { ref nick, .. } if nick.eq_ignore_ascii_case("nickserv") =>
                                MsgTarget::Server { serv_name: serv_name },
                            Pfx::User { ref nick, .. } =>
                                MsgTarget::User { serv_name: serv_name, nick: nick },
                        };
                        self.tui.add_privmsg(origin, &msg, ts, &msg_target);
                        if target == conn.get_nick() {
                            self.tui.set_tab_style(TabStyle::Highlight, &msg_target);
                        } else {
                            // not sure if this case can happen
                            self.tui.set_tab_style(TabStyle::NewMsg, &msg_target);
                        }
                    }
                }
            }

            Cmd::JOIN { chan } => {
                match pfx {
                    Pfx::Server(_) => {
                        self.logger.get_debug_logs().write_line(
                            format_args!("Weird JOIN message pfx {:?}", pfx));
                    }
                    Pfx::User { nick, .. } => {
                        let serv_name = self.conns[conn_idx].get_serv_name();
                        self.logger.get_chan_logs(serv_name, &chan).write_line(
                            format_args!("JOIN: {}", nick));
                        if nick == conn.get_nick() {
                            self.tui.new_chan_tab(&serv_name, &chan);
                        } else {
                            self.tui.add_nick(
                                &nick,
                                Some(Timestamp::now()),
                                &MsgTarget::Chan { serv_name: &serv_name, chan_name: &chan });
                        }
                    }
                }
            }

            Cmd::PART { chan, .. } => {
                match pfx {
                    Pfx::Server(_) => {
                        self.logger.get_debug_logs().write_line(
                            format_args!("Weird PART message pfx {:?}", pfx));
                    },
                    Pfx::User { nick, .. } => {
                        if nick != conn.get_nick() {
                            let serv_name = self.conns[conn_idx].get_serv_name();
                            self.logger.get_chan_logs(serv_name, &chan).write_line(
                                format_args!("PART: {}", nick));
                            self.tui.remove_nick(
                                &nick,
                                Some(Timestamp::now()),
                                &MsgTarget::Chan { serv_name: serv_name, chan_name: &chan });
                        }
                    }
                }
            }

            Cmd::QUIT { .. } => {
                match pfx {
                    Pfx::Server(_) => {
                        self.logger.get_debug_logs().write_line(
                            format_args!("Weird QUIT message pfx {:?}", pfx));
                    },
                    Pfx::User { ref nick, .. } => {
                        let serv_name = self.conns[conn_idx].get_serv_name();
                        self.tui.remove_nick(
                            nick,
                            Some(Timestamp::now()),
                            &MsgTarget::AllUserTabs { serv_name: serv_name, nick: nick });
                    }
                }
            }

            Cmd::NICK { nick } => {
                match pfx {
                    Pfx::Server(_) => {
                        self.logger.get_debug_logs().write_line(
                            format_args!("Weird NICK message pfx {:?}", pfx));
                    },
                    Pfx::User { nick: ref old_nick, .. } => {
                        let serv_name = unsafe { self.conns.get_unchecked(conn_idx) }.get_serv_name();
                        self.tui.rename_nick(
                            old_nick, &nick, Timestamp::now(),
                            &MsgTarget::AllUserTabs { serv_name: serv_name, nick: old_nick, });
                    }
                }
            }

            Cmd::PING { .. } => {}, // ignore
            Cmd::Other { ref cmd, .. } if cmd == "PONG" => {}, // ignore

            Cmd::Reply { num: n, params } => {
                if n <= 003 /* RPL_WELCOME, RPL_YOURHOST, RPL_CREATED */
                        || n == 251 /* RPL_LUSERCLIENT */
                        || n == 255 /* RPL_LUSERME */
                        || n == 372 /* RPL_MOTD */
                        || n == 375 /* RPL_MOTDSTART */
                        || n == 376 /* RPL_ENDOFMOTD */ {
                    debug_assert!(params.len() == 2);
                    let conn = &self.conns[conn_idx];
                    let msg  = &params[1];
                    self.tui.add_msg(
                        msg, Timestamp::now(), &MsgTarget::Server { serv_name: conn.get_serv_name() });
                }

                else if n == 4 // RPL_MYINFO
                        || n == 5 // RPL_BOUNCE
                        || (n >= 252 && n <= 254)
                                   /* RPL_LUSEROP, RPL_LUSERUNKNOWN, */
                                   /* RPL_LUSERCHANNELS */ {
                    let conn = &self.conns[conn_idx];
                    let msg  = params.into_iter().collect::<Vec<String>>().join(" ");
                    self.tui.add_msg(
                        &msg, Timestamp::now(), &MsgTarget::Server { serv_name: conn.get_serv_name() });
                }

                else if n == 265
                        || n == 266
                        || n == 250 {
                    let conn = &self.conns[conn_idx];
                    let msg  = &params[params.len() - 1];
                    self.tui.add_msg(
                        msg, Timestamp::now(), &MsgTarget::Server { serv_name: conn.get_serv_name() });
                }

                // RPL_TOPIC
                else if n == 332 {
                    // FIXME: RFC 2812 says this will have 2 arguments, but freenode
                    // sends 3 arguments (extra one being our nick).
                    assert!(params.len() == 3 || params.len() == 2);
                    let conn  = &self.conns[conn_idx];
                    let chan  = &params[params.len() - 2];
                    let topic = &params[params.len() - 1];
                    self.tui.show_topic(topic, Timestamp::now(), &MsgTarget::Chan {
                        serv_name: conn.get_serv_name(),
                        chan_name: chan,
                    });
                }

                // RPL_NAMREPLY: List of users in a channel
                else if n == 353 {
                    let conn = unsafe { &self.conns.get_unchecked(conn_idx) };
                    let chan = &params[2];
                    let chan_target = MsgTarget::Chan {
                        serv_name: conn.get_serv_name(),
                        chan_name: chan,
                    };

                    for nick in params[3].split_whitespace() {
                        // Apparently some nicks have a '@' prefix (indicating ops)
                        // TODO: Not sure where this is documented
                        let nick = if nick.chars().nth(0) == Some('@') {
                            &nick[1 .. ]
                        } else {
                            nick
                        };
                        self.tui.add_nick(nick, None, &chan_target);
                    }
                }

                // ERR_NICKNAMEINUSE
                else if n == 433 {
                    // TODO
                    self.tui.add_err_msg(
                        &format!("Nick already in use: {:?}", params[1]),
                        Timestamp::now(),
                        &MsgTarget::Server { serv_name: self.conns[conn_idx].get_serv_name() });
                }

                // RPL_ENDOFNAMES: End of NAMES list
                else if n == 366 {}

                else {
                    self.logger.get_debug_logs().write_line(
                        format_args!("Ignoring numeric reply msg:\nPfx: {:?}, num: {:?}, args: {:?}",
                                     pfx, n, params));
                }
            }

            _ => {
                self.logger.get_debug_logs().write_line(
                    format_args!("Ignoring msg:\nPfx: {:?}, msg: {:?}", pfx, msg.cmd));
            }
        }
    }
}

fn find_token_conn_idx(conns: &[Conn], token: Token) -> Option<usize> {
    for (conn_idx, conn) in conns.iter().enumerate() {
        if Token(conn.get_raw_fd() as usize) == token {
            return Some(conn_idx);
        }
    }
    None
}

fn find_conn<'a>(conns: &'a mut [Conn], serv_name: &str) -> Option<&'a mut Conn> {
    match find_conn_idx(conns, serv_name) {
        None => None,
        Some(idx) => Some(unsafe { conns.get_unchecked_mut(idx) } ),
    }
}

fn find_conn_idx(conns: &[Conn], serv_name: &str) -> Option<usize> {
    for (conn_idx, conn) in conns.iter().enumerate() {
        if conn.get_serv_name() == serv_name {
            return Some(conn_idx);
        }
    }
    None
}
