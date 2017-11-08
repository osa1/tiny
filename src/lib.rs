#![cfg_attr(test, feature(test))]
#![feature(alloc_system)]
#![feature(allocator_api)]
#![feature(ascii_ctype)]
#![feature(const_fn)]
#![feature(global_allocator)]
#![feature(offset_to)]

extern crate alloc_system;

#[global_allocator]
static ALLOC: alloc_system::System = alloc_system::System;

#[cfg(test)]
extern crate quickcheck;

extern crate libc;
extern crate mio;
extern crate native_tls;
extern crate net2;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_yaml;
extern crate time;

extern crate take_mut;
extern crate term_input;
extern crate termbox_simple;

#[macro_use]
mod utils;

mod conn;
mod logger;
mod stream;
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
use mio::unix::UnixReady;
use std::ascii::AsciiExt;
use std::error::Error;
use std::fs::File;
use std::io::Read;
use std::io::Write;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;
use std::time::Instant;

use conn::{Conn, ConnEv};
use logger::Logger;
use term_input::Input;
use tui::tabbed::MsgSource;
use tui::tabbed::TabStyle;
use tui::{MsgTarget, TUIRet, Timestamp, TUI};
use wire::{Cmd, Msg, Pfx};

////////////////////////////////////////////////////////////////////////////////////////////////////

pub fn run() {
    let config_path = config::get_config_path();
    if !config_path.is_file() {
        generate_default_config();
    } else {
        match parse_config(config_path) {
            Err(yaml_err) => {
                println!("Can't parse config file:");
                println!("{}", yaml_err);
                ::std::process::exit(1);
            }
            Ok(config::Config {
                servers,
                defaults,
                colors,
                log_dir,
            }) => {
                let args = ::std::env::args().into_iter().collect::<Vec<String>>();
                let servers = if args.len() >= 2 {
                    // connect only to servers that match at least one of
                    // the given patterns
                    let pats = &args[1..];
                    servers
                        .into_iter()
                        .filter(|s| {
                            for pat in pats {
                                if s.addr.contains(pat) {
                                    return true;
                                }
                            }
                            false
                        })
                        .collect()
                } else {
                    servers
                };
                Tiny::run(servers, defaults, log_dir, colors)
            }
        }
    }
}

fn parse_config(config_path: PathBuf) -> serde_yaml::Result<config::Config> {
    let contents = {
        let mut str = String::new();
        let mut file = File::open(config_path).unwrap();
        file.read_to_string(&mut str).unwrap();
        str
    };

    serde_yaml::from_str(&contents)
}

fn generate_default_config() {
    let config_path = config::get_config_path();
    {
        let mut file = File::create(&config_path).unwrap();
        file.write_all(config::get_default_config_yaml().as_bytes())
            .unwrap();
    }
    println!(
        "\
tiny couldn't find a config file at {0:?}, and created a config file with defaults.
You may want to edit {0:?} before re-running tiny.",
        config_path
    );
}

////////////////////////////////////////////////////////////////////////////////////////////////////

struct Tiny<'poll> {
    conns: Vec<Conn<'poll>>,
    defaults: config::Defaults,
    servers: Vec<config::Server>,
    tui: TUI,
    input_ev_handler: Input,
    logger: Logger,
}

const STDIN_TOKEN: Token = Token(libc::STDIN_FILENO as usize);

impl<'poll> Tiny<'poll> {
    pub fn run(
        servers: Vec<config::Server>,
        defaults: config::Defaults,
        log_dir: String,
        colors: config::Colors,
    ) {
        let poll = Poll::new().unwrap();

        poll.register(
            &EventedFd(&libc::STDIN_FILENO),
            STDIN_TOKEN,
            Ready::readable(),
            PollOpt::level(),
        ).unwrap();

        let mut conns = Vec::with_capacity(servers.len());
        for server in servers.iter().cloned() {
            let conn = Conn::from_server(server, &poll);
            conns.push(conn);
        }

        let mut tiny = Tiny {
            conns: conns,
            defaults: defaults,
            servers: servers,
            tui: TUI::new(colors),
            input_ev_handler: Input::new(),
            logger: Logger::new(PathBuf::from(log_dir)),
        };

        tiny.init_mentions_tab();
        tiny.tui.draw();

        let mut last_tick = Instant::now();
        let mut events = Events::with_capacity(10);
        'mainloop: loop {
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
                                    tiny.logger.get_debug_logs().write_line(format_args!(
                                        "BUG: Can't find Token in conns: {:?}",
                                        event.token()
                                    ));
                                }
                                Some(conn_idx) => {
                                    tiny.handle_socket(&poll, event.readiness(), conn_idx);
                                }
                            }
                        }
                    }

                    if last_tick.elapsed() >= Duration::from_secs(1) {
                        for conn_idx in 0..tiny.conns.len() {
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
            &MsgTarget::Server {
                serv_name: "mentions",
            },
        );
    }

    fn handle_stdin(&mut self, poll: &'poll Poll) -> bool {
        let mut ev_buffer = Vec::with_capacity(10);
        let mut abort = false;
        self.input_ev_handler.read_input_events(&mut ev_buffer);
        for ev in ev_buffer.drain(..) {
            match self.tui.handle_input_event(ev) {
                TUIRet::Abort => {
                    abort = true;
                }
                TUIRet::Input { msg, from } => {
                    self.logger.get_debug_logs().write_line(format_args!(
                        "Input source: {:#?}, msg: {}",
                        from,
                        msg.iter().cloned().collect::<String>()
                    ));

                    // We know msg has at least one character as the TUI won't accept it otherwise.
                    if msg[0] == '/' {
                        let msg_str: String = (&msg[1..]).into_iter().cloned().collect();
                        self.handle_cmd(poll, from, &msg_str);
                    } else {
                        self.send_msg(from, &msg.into_iter().collect::<String>(), false);
                    }
                }
                TUIRet::KeyHandled =>
                    {}
                // TUIRet::KeyIgnored(_) | TUIRet::EventIgnored(_) => {}
                ev => {
                    self.logger
                        .get_debug_logs()
                        .write_line(format_args!("Ignoring event: {:?}", ev));
                }
            }
        }
        abort
    }

    fn handle_cmd(&mut self, poll: &'poll Poll, src: MsgSource, msg: &str) {
        let words: Vec<&str> = msg.split_whitespace().into_iter().collect();

        if words[0] == "connect" && words.len() == 2 {
            self.connect(poll, src, words[1]);
        } else if words[0] == "connect" && words.len() == 1 {
            self.tui.add_client_msg(
                "Reconnecting...",
                &MsgTarget::AllServTabs {
                    serv_name: src.serv_name(),
                },
            );
            match find_conn(&mut self.conns, src.serv_name()) {
                Some(conn) => {
                    conn.reconnect(None);
                }
                None => {
                    self.logger
                        .get_debug_logs()
                        .write_line(format_args!("Can't reconnect to {}", src.serv_name()));
                }
            }
        } else if words[0] == "join" && words.len() == 2 {
            self.join(src, words[1]);
        } else if words[0] == "msg" {
            // need to find index of the third word
            let mut word_indices = utils::split_whitespace_indices(msg);
            word_indices.next(); // "/msg"
            word_indices.next(); // target
            if let Some(msg_begins) = word_indices.next() {
                let msg = &msg[msg_begins..];
                let serv = src.serv_name();
                self.send_msg(
                    MsgSource::User {
                        serv_name: serv.to_owned(),
                        nick: words[1].to_owned(),
                    },
                    msg,
                    false,
                );
            } else {
                self.tui
                    .add_client_err_msg("/msg usage: /msg target message", &MsgTarget::CurrentTab);
            }
        } else if words[0] == "me" {
            let mut word_indices = utils::split_whitespace_indices(msg);
            word_indices.next(); // "/me"
            if let Some(msg_begins) = word_indices.next() {
                let msg = &msg[msg_begins..];
                self.send_msg(src, msg, true);
            } else {
                self.tui
                    .add_client_err_msg("/me usage: /me message", &MsgTarget::CurrentTab);
            }
        } else if words[0] == "away" {
            let mut word_indices = utils::split_whitespace_indices(msg);
            word_indices.next(); // "/away"
            let msg = {
                if let Some(msg_begins) = word_indices.next() {
                    Some(&msg[msg_begins..])
                } else {
                    None
                }
            };
            if let Some(conn) = find_conn(&mut self.conns, src.serv_name()) {
                conn.away(msg);
            }
        } else if words[0] == "close" {
            match src {
                MsgSource::Serv { ref serv_name } if serv_name == "mentions" => {
                    // ignore
                }
                MsgSource::Serv { serv_name } => {
                    self.tui.close_server_tab(&serv_name);
                    let conn_idx = find_conn_idx(&self.conns, &serv_name).unwrap();
                    self.conns.remove(conn_idx);
                }
                MsgSource::Chan {
                    serv_name,
                    chan_name,
                } => {
                    self.tui.close_chan_tab(&serv_name, &chan_name);
                    self.part(&serv_name, &chan_name);
                }
                MsgSource::User { serv_name, nick } => {
                    self.tui.close_user_tab(&serv_name, &nick);
                }
            }
        } else if words[0] == "nick" && words.len() == 2 {
            if let Some(conn) = find_conn(&mut self.conns, src.serv_name()) {
                let new_nick = words[1];
                conn.set_nick(new_nick);
                self.tui
                    .set_nick(conn.get_serv_name(), Rc::new(new_nick.to_owned()));
            }
        } else if words[0] == "reload" {
            match parse_config(config::get_config_path()) {
                Ok(config::Config { colors, .. }) =>
                    self.tui.set_colors(colors),
                Err(err) => {
                    self.tui
                        .add_client_err_msg("Can't parse config file:", &MsgTarget::CurrentTab);
                    for line in err.description().lines() {
                        self.tui.add_client_err_msg(line, &MsgTarget::CurrentTab);
                    }
                }
            }
        } else if words[0] == "names" {
            if let MsgSource::Chan {
                ref serv_name,
                ref chan_name,
            } = src
            {
                let nicks_vec = self.tui
                    .get_nicks(serv_name, chan_name)
                    .map(|nicks| nicks.to_strings(""));
                if let Some(nicks_vec) = nicks_vec {
                    let target = MsgTarget::Chan {
                        serv_name: serv_name,
                        chan_name: chan_name,
                    };
                    self.tui.add_client_msg(
                        &format!("{} users: {}", nicks_vec.len(), nicks_vec.join(", ")),
                        &target,
                    );
                }
            } else {
                self.tui.add_client_err_msg(
                    "/names only supported in chan tabs",
                    &MsgTarget::CurrentTab,
                );
            }
        } else if words[0] == "clear" {
            self.tui.clear(&src.to_target());
        } else {
            self.tui.add_client_err_msg(
                &format!("Unsupported command: \"/{}\"", msg),
                &MsgTarget::CurrentTab,
            );
        }
    }

    fn connect(&mut self, poll: &'poll Poll, src: MsgSource, serv_addr: &str) {
        fn split_port(s: &str) -> Option<(&str, &str)> {
            s.find(':').map(|split| (&s[0..split], &s[split + 1..]))
        }

        let (serv_name, serv_port) = {
            match split_port(serv_addr) {
                None => {
                    self.tui
                        .add_client_err_msg("connect: Need a <host>:<port>", &MsgTarget::CurrentTab);
                    return;
                }
                Some((serv_name, serv_port)) =>
                    match serv_port.parse::<u16>() {
                        Err(err) => {
                            self.tui.add_client_err_msg(
                                &format!("connect: Can't parse port {}: {}", serv_port, err),
                                &MsgTarget::CurrentTab,
                            );
                            return;
                        }
                        Ok(serv_port) =>
                            (serv_name, serv_port),
                    },
            }
        };

        let mut reconnect = false; // borrowchk workaround
                                   // see if we already have a Conn for this server
        if let Some(conn) = find_conn(&mut self.conns, serv_name) {
            conn.reconnect(Some((serv_name, serv_port)));
            reconnect = true;
        }

        if reconnect {
            self.tui.add_client_msg(
                "Connecting...",
                &MsgTarget::AllServTabs {
                    serv_name: serv_name,
                },
            );
            return;
        }

        // otherwise create a new Conn, tab etc.
        self.tui.new_server_tab(serv_name);
        self.tui.add_client_msg(
            "Connecting...",
            &MsgTarget::Server {
                serv_name: serv_name,
            },
        );

        let new_conn = {
            match find_conn(&mut self.conns, src.serv_name()) {
                Some(current_conn) =>
                    Conn::from_conn(current_conn, serv_name, serv_port),
                None => {
                    self.logger
                        .get_debug_logs()
                        .write_line(format_args!("connecting to {} {}", serv_name, serv_port));
                    Conn::from_server(
                        config::Server {
                            addr: serv_name.to_owned(),
                            port: serv_port,
                            hostname: self.defaults.hostname.clone(),
                            realname: self.defaults.realname.clone(),
                            nicks: self.defaults.nicks.clone(),
                            auto_cmds: self.defaults.auto_cmds.clone(),
                        },
                        poll,
                    )
                }
            }
        };
        self.conns.push(new_conn);
    }

    fn join(&mut self, src: MsgSource, chan: &str) {
        if let Some(conn) = find_conn(&mut self.conns, src.serv_name()) {
            conn.join(chan);
            return;
        }

        self.tui.add_client_err_msg(
            &format!("Can't JOIN: Not connected to server {}", src.serv_name()),
            &MsgTarget::CurrentTab,
        );
    }

    fn part(&mut self, serv_name: &str, chan: &str) {
        let conn = find_conn(&mut self.conns, serv_name).unwrap();
        conn.part(chan);
    }

    fn send_msg(&mut self, from: MsgSource, msg: &str, ctcp_action: bool) {
        // `tui_target`: Where to show the message on TUI
        // `msg_target`: Actual PRIVMSG target to send to the server
        // `serv_name`: Server name to find connection in `self.conns`
        let (tui_target, msg_target, serv_name) = {
            match from {
                MsgSource::Serv { ref serv_name } => {
                    if serv_name == "mentions" {
                        self.tui.add_client_err_msg(
                            "Use `/connect <server>` to connect to a server",
                            &MsgTarget::CurrentTab,
                        );
                    } else {
                        self.tui.add_client_err_msg(
                            "Can't send message to a server.",
                            &MsgTarget::CurrentTab,
                        );
                    }
                    return;
                }

                MsgSource::Chan {
                    ref serv_name,
                    ref chan_name,
                } =>
                    (
                        MsgTarget::Chan {
                            serv_name: serv_name,
                            chan_name: chan_name,
                        },
                        chan_name,
                        serv_name,
                    ),

                MsgSource::User {
                    ref serv_name,
                    ref nick,
                } => {
                    let msg_target = if nick.eq_ignore_ascii_case("nickserv") {
                        MsgTarget::Server {
                            serv_name: serv_name,
                        }
                    } else {
                        MsgTarget::User {
                            serv_name: serv_name,
                            nick: nick,
                        }
                    };
                    (msg_target, nick, serv_name)
                }
            }
        };

        let conn = find_conn(&mut self.conns, serv_name).unwrap();
        let ts = Timestamp::now();
        let extra_len = msg_target.len() as i32 + if ctcp_action {
            9 // "\0x1ACTION \0x1".len()
        } else {
            0
        };
        let send_fn = if ctcp_action {
            Conn::ctcp_action
        } else {
            Conn::privmsg
        };
        for msg in conn.split_privmsg(extra_len, msg) {
            send_fn(conn, msg_target, msg);
            self.tui
                .add_privmsg(conn.get_nick(), msg, ts, &tui_target, ctcp_action);
        }
    }

    ////////////////////////////////////////////////////////////////////////////

    fn handle_socket(&mut self, poll: &'poll Poll, readiness: Ready, conn_idx: usize) {
        let mut evs = Vec::with_capacity(2);
        {
            let conn = &mut self.conns[conn_idx];
            if readiness.is_readable() {
                conn.recv(&mut evs, &mut self.logger);
            }
            if readiness.is_writable() {
                conn.send(&mut evs);
            }
            if readiness.contains(UnixReady::hup()) {
                conn.enter_disconnect_state();
                self.tui.add_err_msg(
                    &format!(
                        "Connection error (HUP). \
                         Will try to reconnect in {} seconds.",
                        conn::RECONNECT_TICKS
                    ),
                    Timestamp::now(),
                    &MsgTarget::AllServTabs {
                        serv_name: conn.get_serv_name(),
                    },
                );
            }
        }
        self.handle_conn_evs(poll, conn_idx, evs);
    }

    fn handle_conn_evs(&mut self, poll: &'poll Poll, conn_idx: usize, evs: Vec<ConnEv>) {
        for ev in evs {
            self.handle_conn_ev(poll, conn_idx, ev);
        }
    }

    fn handle_conn_ev(&mut self, poll: &'poll Poll, conn_idx: usize, ev: ConnEv) {
        match ev {
            ConnEv::Connected => {
                self.tui.add_msg(
                    "Connected.",
                    Timestamp::now(),
                    &MsgTarget::AllServTabs {
                        serv_name: self.conns[conn_idx].get_serv_name(),
                    },
                );
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
                    for cmd in &auto_cmds {
                        self.handle_cmd(
                            poll,
                            MsgSource::Serv {
                                serv_name: serv_name.to_owned(),
                            },
                            cmd,
                        );
                    }
                }
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
                    Timestamp::now(),
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
                conn.reconnect(None);
            }
            ConnEv::Err(err) => {
                // TODO: Some of these errors should not cause a disconnect,
                // e.g. EAGAIN, EWOULDBLOCK ...
                let conn = &mut self.conns[conn_idx];
                conn.enter_disconnect_state();
                self.tui.add_err_msg(
                    &format!(
                        "Connection error: {}. \
                         Will try to reconnect in {} seconds.",
                        err.description(),
                        conn::RECONNECT_TICKS
                    ),
                    Timestamp::now(),
                    &MsgTarget::AllServTabs {
                        serv_name: conn.get_serv_name(),
                    },
                );
            }
            ConnEv::Msg(msg) => {
                self.handle_msg(conn_idx, msg, Timestamp::now());
            }
            ConnEv::NickChange(new_nick) => {
                let conn = &self.conns[conn_idx];
                self.tui.set_nick(conn.get_serv_name(), Rc::new(new_nick));
            }
        }
    }

    fn handle_msg(&mut self, conn_idx: usize, msg: Msg, ts: Timestamp) {
        let conn = &self.conns[conn_idx];
        let pfx = match msg.pfx {
            None => {
                return; /* TODO: log this */
            }
            Some(pfx) =>
                pfx,
        };
        match msg.cmd {
            Cmd::PRIVMSG { target, msg } | Cmd::NOTICE { target, msg } => {
                let origin = match pfx {
                    Pfx::Server(_) =>
                        conn.get_serv_name(),
                    Pfx::User { ref nick, .. } =>
                        nick,
                };

                let (msg, is_ctcp_action) = wire::check_ctcp_action_msg(&msg);

                match target {
                    wire::MsgTarget::Chan(chan) => {
                        self.logger
                            .get_chan_logs(conn.get_serv_name(), &chan)
                            .write_line(format_args!("PRIVMSG: {}", msg));
                        let msg_target = MsgTarget::Chan {
                            serv_name: conn.get_serv_name(),
                            chan_name: &chan,
                        };
                        // highlight the message if it mentions us
                        if msg.find(conn.get_nick()).is_some() {
                            self.tui.add_privmsg_highlight(
                                origin,
                                msg,
                                ts,
                                &msg_target,
                                is_ctcp_action,
                            );
                            self.tui.set_tab_style(TabStyle::Highlight, &msg_target);
                            let mentions_target = MsgTarget::Server {
                                serv_name: "mentions",
                            };
                            self.tui.add_msg(
                                &format!(
                                    "{} in {}:{}: {}",
                                    origin,
                                    conn.get_serv_name(),
                                    chan,
                                    msg
                                ),
                                ts,
                                &mentions_target,
                            );
                            self.tui
                                .set_tab_style(TabStyle::Highlight, &mentions_target);
                        } else {
                            self.tui
                                .add_privmsg(origin, msg, ts, &msg_target, is_ctcp_action);
                            self.tui.set_tab_style(TabStyle::NewMsg, &msg_target);
                        }
                    }
                    wire::MsgTarget::User(target) => {
                        let serv_name = conn.get_serv_name();
                        let msg_target = match pfx {
                            Pfx::Server(_) =>
                                MsgTarget::Server {
                                    serv_name: serv_name,
                                },
                            Pfx::User { ref nick, .. } if nick.eq_ignore_ascii_case("nickserv") =>
                                MsgTarget::Server {
                                    serv_name: serv_name,
                                },
                            Pfx::User { ref nick, .. } =>
                                MsgTarget::User {
                                    serv_name: serv_name,
                                    nick: nick,
                                },
                        };
                        self.tui
                            .add_privmsg(origin, msg, ts, &msg_target, is_ctcp_action);
                        if target == conn.get_nick() {
                            self.tui.set_tab_style(TabStyle::Highlight, &msg_target);
                        } else {
                            // not sure if this case can happen
                            self.tui.set_tab_style(TabStyle::NewMsg, &msg_target);
                        }
                    }
                }
            }

            Cmd::JOIN { chan } =>
                match pfx {
                    Pfx::Server(_) => {
                        self.logger
                            .get_debug_logs()
                            .write_line(format_args!("Weird JOIN message pfx {:?}", pfx));
                    }
                    Pfx::User { nick, .. } => {
                        let serv_name = self.conns[conn_idx].get_serv_name();
                        self.logger
                            .get_chan_logs(serv_name, &chan)
                            .write_line(format_args!("JOIN: {}", nick));
                        if nick == conn.get_nick() {
                            self.tui.new_chan_tab(serv_name, &chan);
                        } else {
                            self.tui.add_nick(
                                &nick,
                                Some(Timestamp::now()),
                                &MsgTarget::Chan {
                                    serv_name: serv_name,
                                    chan_name: &chan,
                                },
                            );
                        }
                    }
                },

            Cmd::PART { chan, .. } =>
                match pfx {
                    Pfx::Server(_) => {
                        self.logger
                            .get_debug_logs()
                            .write_line(format_args!("Weird PART message pfx {:?}", pfx));
                    }
                    Pfx::User { nick, .. } =>
                        if nick != conn.get_nick() {
                            let serv_name = self.conns[conn_idx].get_serv_name();
                            self.logger
                                .get_chan_logs(serv_name, &chan)
                                .write_line(format_args!("PART: {}", nick));
                            self.tui.remove_nick(
                                &nick,
                                Some(Timestamp::now()),
                                &MsgTarget::Chan {
                                    serv_name: serv_name,
                                    chan_name: &chan,
                                },
                            );
                        },
                },

            Cmd::QUIT { .. } =>
                match pfx {
                    Pfx::Server(_) => {
                        self.logger
                            .get_debug_logs()
                            .write_line(format_args!("Weird QUIT message pfx {:?}", pfx));
                    }
                    Pfx::User { ref nick, .. } => {
                        let serv_name = self.conns[conn_idx].get_serv_name();
                        self.tui.remove_nick(
                            nick,
                            Some(Timestamp::now()),
                            &MsgTarget::AllUserTabs {
                                serv_name: serv_name,
                                nick: nick,
                            },
                        );
                    }
                },

            Cmd::NICK { nick } =>
                match pfx {
                    Pfx::Server(_) => {
                        self.logger
                            .get_debug_logs()
                            .write_line(format_args!("Weird NICK message pfx {:?}", pfx));
                    }
                    Pfx::User {
                        nick: ref old_nick, ..
                    } => {
                        let serv_name =
                            unsafe { self.conns.get_unchecked(conn_idx) }.get_serv_name();
                        self.tui.rename_nick(
                            old_nick,
                            &nick,
                            Timestamp::now(),
                            &MsgTarget::AllUserTabs {
                                serv_name: serv_name,
                                nick: old_nick,
                            },
                        );
                    }
                },

            Cmd::PING { .. } =>
                {}
            // ignore
            Cmd::Other { ref cmd, .. } if cmd == "PONG" =>
                {}

            // ignore
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
                    let conn = &self.conns[conn_idx];
                    let msg = &params[1];
                    self.tui.add_msg(
                        msg,
                        Timestamp::now(),
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
                    let conn = &self.conns[conn_idx];
                    let msg = params.into_iter().collect::<Vec<String>>().join(" ");
                    self.tui.add_msg(
                        &msg,
                        Timestamp::now(),
                        &MsgTarget::Server {
                            serv_name: conn.get_serv_name(),
                        },
                    );
                } else if n == 265 || n == 266 || n == 250 {
                    let conn = &self.conns[conn_idx];
                    let msg = &params[params.len() - 1];
                    self.tui.add_msg(
                        msg,
                        Timestamp::now(),
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
                    let conn = &self.conns[conn_idx];
                    let chan = &params[params.len() - 2];
                    let topic = &params[params.len() - 1];
                    self.tui.show_topic(
                        topic,
                        Timestamp::now(),
                        &MsgTarget::Chan {
                            serv_name: conn.get_serv_name(),
                            chan_name: chan,
                        },
                    );
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
                            &nick[1..]
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
                        &MsgTarget::Server {
                            serv_name: self.conns[conn_idx].get_serv_name(),
                        },
                    );
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
                            serv_name: self.conns[conn_idx].get_serv_name(),
                        },
                    );
                }
                // ERR_NOSUCHNICK
                else if n == 401 {
                    let nick = &params[1];
                    let msg = &params[2];
                    let serv_name = self.conns[conn_idx].get_serv_name();
                    self.tui.add_client_msg(
                        msg,
                        &MsgTarget::User {
                            serv_name: serv_name,
                            nick: nick,
                        },
                    );
                }
                // Not in any of the RFCs. Also known as ERR_BANONCHAN on the internets.
                // Sent by freenode when nick change failed. See issue #29.
                else if n == 435 && params.len() == 4 {
                    // 0: old nick
                    // 1: new nick
                    let chan = &params[2];
                    let msg = &params[3];
                    let serv_name = self.conns[conn_idx].get_serv_name();
                    self.tui.add_err_msg(
                        &format!("{}: {}", chan, msg),
                        Timestamp::now(),
                        &MsgTarget::Server {
                            serv_name: serv_name,
                        },
                    );
                }
                // add everything else to debug file
                else {
                    self.logger.get_debug_logs().write_line(format_args!(
                        "Ignoring numeric reply msg:\nPfx: {:?}, num: {:?}, args: {:?}",
                        pfx,
                        n,
                        params
                    ));
                }
            }

            _ => {
                self.logger.get_debug_logs().write_line(format_args!(
                    "Ignoring msg:\nPfx: {:?}, msg: {:?}",
                    pfx,
                    msg.cmd
                ));
            }
        }
    }
}

fn find_token_conn_idx(conns: &[Conn], token: Token) -> Option<usize> {
    for (conn_idx, conn) in conns.iter().enumerate() {
        if conn.get_conn_tok() == token {
            return Some(conn_idx);
        }
    }
    None
}

fn find_conn<'a, 'poll>(
    conns: &'a mut [Conn<'poll>],
    serv_name: &str,
) -> Option<&'a mut Conn<'poll>> {
    match find_conn_idx(conns, serv_name) {
        None =>
            None,
        Some(idx) =>
            Some(unsafe { conns.get_unchecked_mut(idx) }),
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
