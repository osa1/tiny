#![feature(alloc_system)]
#![feature(test)]

extern crate alloc_system;
extern crate ev_loop;
extern crate libc;
extern crate net2;
extern crate rand;
extern crate time;

extern crate term_input;
extern crate termbox_simple;

mod conn;
mod logger;
mod utils;
mod wire;
pub mod trie;
pub mod tui;

use std::io::Write;
use std::os::unix::io::{RawFd};
use std::path::PathBuf;

use conn::{Conn, ConnEv};
use ev_loop::{EvLoop, EvLoopCtrl, READ_EV};
use logger::Logger;
use term_input::{Input, Event};
use tui::tabbed::MsgSource;
use tui::{TUI, TUIRet, MsgTarget};
use wire::{Cmd, Msg, Pfx, Receiver};

pub struct Tiny {
    /// A connection to a server is maintained by 'Conn'.
    conns: Vec<Conn>,
    tui: TUI,
    input_ev_handler: Input,
    nick: String,
    hostname: String,
    realname: String,
    logger: Logger,
}

impl Tiny {
    pub fn run(nick: String, hostname: String, realname: String) {
        let mut tiny = Tiny {
            conns: Vec::with_capacity(1),
            tui: TUI::new(),
            input_ev_handler: Input::new(),
            nick: nick,
            hostname: hostname,
            realname: realname,
            logger: Logger::new(PathBuf::from("tiny_logs")),
        };

        tiny.tui.new_server_tab("debug");
        tiny.tui.draw();

        let mut ev_loop: EvLoop<Tiny> = EvLoop::new();
        // we maintain this separately as otherwise we're having borrow checker problems
        let mut ev_buffer = vec![];
        ev_loop.add_fd(libc::STDIN_FILENO, READ_EV, Box::new(move |_, ctrl, tiny| {
            tiny.handle_stdin(ctrl, &mut ev_buffer);
            tiny.tui.draw();
        }));

        {
            let mut sig_mask: libc::sigset_t = unsafe { std::mem::zeroed() };
            unsafe {
                libc::sigemptyset(&mut sig_mask as *mut libc::sigset_t);
                libc::sigaddset(&mut sig_mask as *mut libc::sigset_t, libc::SIGWINCH);
            };

            ev_loop.add_signal(&sig_mask, Box::new(|_, tiny| {
                tiny.tui.resize();
                tiny.tui.draw();
            }));

            tiny.tui.draw();
        }

        ev_loop.add_timer(1000, 1000, Box::new(|ctrl, tiny, _| {
            for conn_idx in 0 .. tiny.conns.len() {
                let mut evs = Vec::with_capacity(1);
                {
                    let conn = &mut tiny.conns[conn_idx];
                    conn.tick(&mut evs, &mut tiny.tui);
                }
                tiny.handle_socket_evs(conn_idx, evs, ctrl);
                // debug
                // tiny.tui.add_msg(
                //     "tick!",
                //     &time::now(),
                //     &MsgTarget::Server { serv_name: conn.get_serv_name() });
                // tiny.tui.draw();
            }
        }));

        ev_loop.run(tiny);
    }

    fn handle_stdin(&mut self, ctrl: &mut EvLoopCtrl<Tiny>, ev_buffer: &mut Vec<Event>) {
        self.input_ev_handler.read_input_events(ev_buffer);
        for ev in ev_buffer.drain(..) {
            match self.tui.handle_input_event(ev) {
                TUIRet::Abort => {
                    ctrl.stop();
                }
                TUIRet::Input { msg, from } => {
                    writeln!(self.tui,
                             "Input source: {:#?}, msg: {}",
                             from, msg.iter().cloned().collect::<String>()).unwrap();

                    // We know msg has at least one character as the TUI won't accept it otherwise.
                    if msg[0] == '/' {
                        self.handle_command(ctrl, from, (&msg[ 1 .. ]).into_iter().cloned().collect());
                    } else {
                        self.send_msg(from, msg);
                    }
                }
                TUIRet::KeyHandled => {}
                // TUIRet::KeyIgnored(_) | TUIRet::EventIgnored(_) => {}
                ev => {
                    writeln!(self.tui, "Ignoring event: {:?}", ev).unwrap();
                }
            }
        }
    }

    fn handle_command(&mut self, ctrl: &mut EvLoopCtrl<Tiny>, src: MsgSource, msg: String) {
        let words : Vec<&str> = msg.split_whitespace().into_iter().collect();
        if words[0] == "connect" {
            self.connect(ctrl, words[1]);
        } else if words[0] == "join" {
            self.join(src, words[1]);
        } else if words[0] == "quit" {
            ctrl.stop();
        } else {
            self.tui.add_client_err_msg(
                &format!("Unsupported command: {}", words[0]), &MsgTarget::CurrentTab);
        }
    }

    fn connect(&mut self, ctrl: &mut EvLoopCtrl<Tiny>, serv_addr: &str) {
        match utils::drop_port(serv_addr) {
            None => {
                self.tui.add_client_err_msg("connect: Need a <host>:<port>", &MsgTarget::CurrentTab);
            }
            Some(serv_name) => {
                self.tui.new_server_tab(serv_name);
                writeln!(self.tui, "Created tab: {}", serv_name).unwrap();
                self.tui.add_client_msg("Connecting...",
                                        &MsgTarget::Server { serv_name: serv_name });

                let conn = Conn::new(serv_addr, serv_name, &self.nick, &self.hostname, &self.realname);
                let fd = conn.get_raw_fd();
                self.conns.push(conn);
                ctrl.add_fd(fd, READ_EV, Box::new(move |_, ctrl, tiny| {
                    match tiny.find_fd_conn(fd) {
                        None => {
                            writeln!(tiny.tui, "BUG: Can't find fd in conns: {:?}", fd).unwrap();
                            ctrl.remove_self();
                        }
                        Some(conn_idx) => {
                            tiny.handle_socket(conn_idx, ctrl);
                            tiny.tui.draw();
                        }
                    }
                }));
            }
        }
    }

    fn join(&mut self, src: MsgSource, chan: &str) {
        wire::join(chan, &mut self.tui).unwrap();
        match self.find_conn(src.serv_name()) {
            Some(conn) => {
                wire::join(chan, conn).unwrap();
                return;
            }
            None => {
                // drop the borrowed self and run next statement
                // rustc is too dumb to figure that None can't borrow.
            },
        }

        self.tui.add_client_err_msg(
            &format!("Can't JOIN: Not connected to server {}", src.serv_name()),
            &MsgTarget::CurrentTab);
    }

    fn send_msg(&mut self, from: MsgSource, msg: Vec<char>) {
        let msg_string = msg.iter().cloned().collect::<String>();

        match from {
            MsgSource::Serv { .. } => {
                self.tui.add_client_err_msg("Can't send PRIVMSG to a server.",
                                            &MsgTarget::CurrentTab);
            },

            MsgSource::Chan { serv_name, chan_name } => {
                {
                    let conn = self.find_conn(&serv_name).unwrap();
                    wire::privmsg(&chan_name, &msg_string, conn).unwrap();
                }
                self.tui.add_privmsg(&self.nick, &msg_string,
                                     &time::now(),
                                     &MsgTarget::Chan { serv_name: &serv_name,
                                                        chan_name: &chan_name });
            },

            MsgSource::User { serv_name, nick } => {
                {
                    let conn = self.find_conn(&serv_name).unwrap();
                    wire::privmsg(&nick, &msg_string, conn).unwrap();
                }
                self.tui.add_privmsg(&self.nick, &msg_string,
                                     &time::now(),
                                     &MsgTarget::User { serv_name: &serv_name, nick: &nick });
            }
        }
    }

    fn find_conn(&mut self, serv_name : &str) -> Option<&mut Conn> {
        for conn in self.conns.iter_mut() {
            if conn.get_serv_name() == serv_name {
                return Some(conn);
            }
        }
        None
    }

    fn find_fd_conn(&mut self, fd: RawFd) -> Option<usize> {
        for (i, conn) in self.conns.iter().enumerate() {
            if conn.get_raw_fd() == fd {
                return Some(i);
            }
        }
        None
    }

    ////////////////////////////////////////////////////////////////////////////

    fn handle_socket(&mut self, conn_idx: usize, ctrl: &mut EvLoopCtrl<Tiny>) {
        let mut evs = Vec::with_capacity(2);
        {
            let mut conn = &mut self.conns[conn_idx];
            conn.read_incoming_msg(&mut evs, &mut self.tui)
        }
        self.handle_socket_evs(conn_idx, evs, ctrl);
    }

    fn handle_socket_evs(&mut self, conn_idx: usize, evs: Vec<ConnEv>, ctrl: &mut EvLoopCtrl<Tiny>) {
        for ev in evs.into_iter() {
            match ev {
                ConnEv::Disconnected => {
                    let mut conn = &mut self.conns[conn_idx];
                    ctrl.remove_self();
                    self.tui.add_err_msg(
                        "Disconnected.",
                        &time::now(),
                        &MsgTarget::AllServTabs {
                            serv_name: conn.get_serv_name(),
                        });
                    ctrl.remove_self(); // remove old fd
                    self.tui.add_client_msg("Connecting...",
                                            &MsgTarget::Server { serv_name: conn.get_serv_name() });
                    conn.reconnect();
                    let fd = conn.get_raw_fd();
                    // FIXME: Duplicated code
                    ctrl.add_fd(fd, READ_EV, Box::new(move |_, ctrl, tiny| {
                        let conn_idx = tiny.find_fd_conn(fd).unwrap();
                        tiny.handle_socket(conn_idx, ctrl);
                        tiny.tui.draw();
                    }));
                }
                ConnEv::Err(err_msg) => {
                    self.tui.add_err_msg(
                        &err_msg,
                        &time::now(),
                        &MsgTarget::Server { serv_name: self.conns[conn_idx].get_serv_name() });
                }
                ConnEv::Msg(msg) => {
                    self.handle_msg(conn_idx, msg, time::now());
                }
            }
        }
    }

    fn handle_msg(&mut self, conn_idx: usize, msg: Msg, tm: time::Tm) {
        let conn = &self.conns[conn_idx];
        let pfx = match msg.pfx {
            None => { return; /* TODO: log this */ }
            Some(pfx) => pfx
        };
        match msg.cmd {

            Cmd::PRIVMSG { receivers, contents } => {
                let receiver = match pfx {
                    Pfx::Server(_) => conn.get_serv_name(),
                    Pfx::User { ref nick, .. } => nick,
                };
                match receivers {
                    Receiver::Chan(chan) => {
                        writeln!(self.logger.get_chan_file(&conn.get_serv_name(), &chan),
                                 "PRIVMSG: {}", contents).unwrap();
                        self.tui.add_privmsg(receiver, &contents, &tm, &MsgTarget::Chan {
                            serv_name: conn.get_serv_name(),
                            chan_name: &chan,
                        });
                    }
                    Receiver::User(_) => {
                        let msg_target = pfx_to_target(&pfx, conn.get_serv_name());
                        // TODO: Set the topic if a new tab is created.
                        self.tui.add_privmsg(receiver, &contents, &tm, &msg_target);
                    }
                }
            }

            Cmd::JOIN { chan } => {
                match pfx {
                    Pfx::Server(_) => {
                        writeln!(self.tui, "Weird JOIN message pfx {:?}", pfx).unwrap();
                    }
                    Pfx::User { nick, .. } => {
                        let serv_name = self.conns[conn_idx].get_serv_name();
                        writeln!(self.logger.get_chan_file(serv_name, &chan),
                                 "JOIN: {}", nick).unwrap();
                        if nick == self.nick {
                            self.tui.new_chan_tab(&serv_name, &chan);
                        } else {
                            self.tui.add_nick(
                                &nick,
                                Some(&time::now()),
                                &MsgTarget::Chan { serv_name: &serv_name, chan_name: &chan });
                        }
                    }
                }
            }

            Cmd::PART { chan, .. } => {
                match pfx {
                    Pfx::Server(_) => {
                        writeln!(self.tui, "Weird PART message pfx {:?}", pfx).unwrap();
                    },
                    Pfx::User { nick, .. } => {
                        let serv_name = self.conns[conn_idx].get_serv_name();
                        writeln!(self.logger.get_chan_file(serv_name, &chan),
                                 "PART: {}", nick).unwrap();
                        self.tui.remove_nick(
                            &nick,
                            Some(&time::now()),
                            &MsgTarget::Chan { serv_name: serv_name, chan_name: &chan });
                    }
                }
            }

            Cmd::QUIT { .. } => {
                match pfx {
                    Pfx::Server(_) => {
                        writeln!(self.tui, "Weird QUIT message pfx {:?}", pfx).unwrap();
                    },
                    Pfx::User { ref nick, .. } => {
                        let serv_name = self.conns[conn_idx].get_serv_name();
                        self.tui.remove_nick(
                            nick,
                            Some(&time::now()),
                            &MsgTarget::AllUserTabs { serv_name: serv_name, nick: nick });
                    }
                }
            }

            Cmd::NOTICE { nick, msg } => {
                let conn = &self.conns[conn_idx];
                if nick == "*" || nick == self.nick {
                    self.tui.add_msg(&msg, &time::now(), &pfx_to_target(&pfx, conn.get_serv_name()));
                } else {
                    writeln!(self.tui, "Weird NOTICE target: {}", nick).unwrap();
                }
            }

            Cmd::NICK { nick } => {
                match pfx {
                    Pfx::Server(_) => {
                        writeln!(self.tui, "Weird NICK message pfx {:?}", pfx).unwrap();
                    },
                    Pfx::User { nick: ref old_nick, .. } => {
                        let serv_name = unsafe { self.conns.get_unchecked(conn_idx) }.get_serv_name();
                        self.tui.rename_nick(
                            old_nick, &nick, &time::now(),
                            &MsgTarget::AllUserTabs { serv_name: serv_name, nick: old_nick, });
                    }
                }
            }

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
                        msg, &time::now(), &MsgTarget::Server { serv_name: conn.get_serv_name() });
                }

                else if n == 4 // RPL_MYINFO
                        || n == 5 // RPL_BOUNCE
                        || (n >= 252 && n <= 254)
                                   /* RPL_LUSEROP, RPL_LUSERUNKNOWN, */
                                   /* RPL_LUSERCHANNELS */ {
                    let conn = &self.conns[conn_idx];
                    let msg  = params.into_iter().collect::<Vec<String>>().join(" ");
                    self.tui.add_msg(
                        &msg, &time::now(), &MsgTarget::Server { serv_name: conn.get_serv_name() });
                }

                else if n == 265
                        || n == 266
                        || n == 250 {
                    let conn = &self.conns[conn_idx];
                    let msg  = &params[params.len() - 1];
                    self.tui.add_msg(
                        msg, &time::now(), &MsgTarget::Server { serv_name: conn.get_serv_name() });
                }

                // RPL_TOPIC
                else if n == 332 {
                    // FIXME: RFC 2812 says this will have 2 arguments, but freenode
                    // sends 3 arguments (extra one being our nick).
                    assert!(params.len() == 3 || params.len() == 2);
                    let conn  = &self.conns[conn_idx];
                    let chan  = &params[params.len() - 2];
                    let topic = &params[params.len() - 1];
                    self.tui.set_topic(topic, &MsgTarget::Chan {
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
                        writeln!(self.tui, "adding nick {} to {:?}", nick, chan_target).unwrap();
                        self.tui.add_nick(nick, None, &chan_target);
                    }
                }

                // RPL_ENDOFNAMES: End of NAMES list
                else if n == 366 {}

                else {
                    writeln!(self.tui,
                             "Ignoring numeric reply msg:\nPfx: {:?}, num: {:?}, args: {:?}",
                             pfx, n, params).unwrap();
                }
            }

            _ => {
                writeln!(self.tui, "Ignoring msg:\nPfx: {:?}, msg: {:?}", pfx, msg.cmd).unwrap();
            }
        }
    }
}

fn pfx_to_target<'a>(pfx : &'a Pfx, curr_serv : &'a str) -> MsgTarget<'a> {
    match *pfx {
        Pfx::Server(_) => MsgTarget::Server { serv_name: curr_serv },
        Pfx::User { ref nick, .. } => MsgTarget::User { serv_name: curr_serv, nick: nick },
    }
}
