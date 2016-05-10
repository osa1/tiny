#![feature(alloc_system)]
#![feature(test)]

extern crate alloc_system;
extern crate libc;
extern crate rand;
extern crate rustbox;
extern crate termbox_sys;
extern crate test;
extern crate time;

mod comms;
mod utils;
pub mod msg;
pub mod tui;

use std::cmp::max;
use std::io::Write;
use std::io;
use std::mem;

use comms::{Comms, CommsRet};
use msg::{Pfx, Cmd, Msg};
use tui::tabbed::MsgSource;
use tui::{TUI, TUIRet, MsgTarget};

pub struct Tiny {
    /// A connection to a server is maintained by 'Comms'.
    comms    : Vec<Comms>,

    tui      : TUI,

    nick     : String,
    hostname : String,
    realname : String,
}

#[derive(PartialEq, Eq)]
enum LoopRet {
    Abort,
    Continue,
    Disconnected { fd : libc::c_int },
    UpdateFds,
}

impl Tiny {
    pub fn new(nick : String, hostname : String, realname : String) -> Tiny {
        Tiny {
            comms: Vec::with_capacity(1),
            tui: TUI::new(),
            nick: nick,
            hostname: hostname,
            realname: realname,
        }
    }

    pub fn mainloop(&mut self) {
        self.tui.new_server_tab("debug");

        loop {
            // Set up the descriptors for select()
            let mut fd_set : libc::fd_set = unsafe { mem::zeroed() };

            // 0 is stdin
            unsafe { libc::FD_SET(0, &mut fd_set); }

            let mut max_fd : libc::c_int = 0;
            for comm in self.comms.iter() {
                let fd = comm.get_raw_fd();
                max_fd = max(fd, max_fd);
                unsafe { libc::FD_SET(fd, &mut fd_set); }
            }

            let nfds = max_fd + 1;

            // Start the loop
            if self.mainloop_(fd_set, nfds) == LoopRet::Abort {
                break;
            }
        }
    }

    fn mainloop_(&mut self, fd_set : libc::fd_set, nfds : libc::c_int) -> LoopRet {
        loop {
            self.tui.draw();

            let mut fd_set_ = fd_set.clone();
            let ret =
                unsafe {
                    libc::select(nfds,
                                 &mut fd_set_,           // read fds
                                 std::ptr::null_mut(),   // write fds
                                 std::ptr::null_mut(),   // error fds
                                 std::ptr::null_mut())   // timeval
                };

            // A resize signal (SIGWINCH) causes select() to fail, but termbox's
            // signal handler runs and we need to run termbox's poll_event() to
            // be able to catch the resize event. So, when stdin is ready we
            // call the TUI event handler, but we also call it when select() is
            // interrupted for some reason, just to be able to handle resize
            // events.
            //
            // See also https://github.com/nsf/termbox/issues/71.
            if unsafe { ret == -1 || libc::FD_ISSET(0, &mut fd_set_) } {
                match self.handle_stdin() {
                    LoopRet::Abort => { return LoopRet::Abort; },
                    LoopRet::UpdateFds => { return LoopRet::UpdateFds; }
                    _ => {}
                }
            }

            // A socket is ready
            // TODO: Can multiple sockets be set in single select() call? I
            // assume yes for now.
            else {
                // Collecting comms to read in this Vec becuase Rust sucs.
                let mut comm_idxs = Vec::with_capacity(1);
                for (comm_idx, comm) in self.comms.iter_mut().enumerate() {
                    let fd = comm.get_raw_fd();
                    if unsafe { libc::FD_ISSET(fd, &mut fd_set_) } {
                        comm_idxs.push(comm_idx);
                    }
                }

                let mut abort = false;
                let mut reset_fds = false;
                for comm_idx in comm_idxs {
                    match self.handle_socket(comm_idx) {
                        LoopRet::Abort => { abort = true; },
                        LoopRet::Disconnected { .. } => {
                            let comm = self.comms.remove(comm_idx);
                            self.tui.add_err_msg(
                                "Disconnected.", &time::now(),
                                &MsgTarget::AllServTabs {
                                    serv_name: &comm.serv_name,
                                });
                            reset_fds = true;
                        },
                        _ => {}
                    }
                }

                if abort {
                    return LoopRet::Abort;
                } else if reset_fds {
                    return LoopRet::Continue;
                }
            }
        }
    }

    ////////////////////////////////////////////////////////////////////////////

    fn handle_stdin(&mut self) -> LoopRet {
        match self.tui.keypressed_peek() {
            TUIRet::Abort => LoopRet::Abort,
            TUIRet::Input { msg, from } => {
                writeln!(self.tui,
                         "Input source: {:#?}, msg: {}",
                         from, msg.iter().cloned().collect::<String>()).unwrap();

                writeln!(io::stderr(),
                         "Input source: {:#?}, msg: {}",
                         from, msg.iter().cloned().collect::<String>()).unwrap();

                // We know msg has at least one character as the TUI won't
                // accept it otherwise.
                if msg[0] == '/' {
                    self.handle_command(from, (&msg[ 1 .. ]).into_iter().cloned().collect())
                } else {
                    self.send_msg(from, msg);
                    LoopRet::Continue
                }
            },
            _ => LoopRet::Continue
        }
    }

    fn handle_command(&mut self, src : MsgSource, msg : String) -> LoopRet {
        let words : Vec<&str> = msg.split_whitespace().into_iter().collect();
        if words[0] == "connect" {
            self.connect(words[1]);
            LoopRet::UpdateFds
        } else if words[0] == "join" {
            self.join(src, words[1]);
            LoopRet::Continue
        } else if words[0] == "quit" {
            LoopRet::Abort
        } else {
            self.tui.add_client_err_msg(
                &format!("Unsupported command: {}", words[0]), &MsgTarget::CurrentTab);
            LoopRet::Continue
        }
    }

    fn connect(&mut self, serv_addr : &str) {
        match utils::drop_port(serv_addr) {
            None => {
                self.tui.add_client_err_msg("connect: Need a <host>:<port>",
                                            &MsgTarget::CurrentTab);
            },
            Some(serv_name) => {
                self.tui.new_server_tab(serv_name);
                writeln!(self.tui, "Created tab: {}", serv_name).unwrap();
                self.tui.add_client_msg("Connecting...",
                                        &MsgTarget::Server { serv_name: serv_name });

                match Comms::try_connect(serv_addr, serv_name,
                                         &self.nick, &self.hostname, &self.realname) {
                    Ok(comms) => {
                        self.comms.push(comms);
                    },
                    Err(err) => {
                        self.tui.add_client_err_msg(&format!("Error: {}", err),
                                                    &MsgTarget::Server { serv_name: serv_name });
                    }
                }
            }
        }
    }

    fn join(&mut self, src : MsgSource, chan : &str) {
        Msg::join(chan, &mut self.tui).unwrap(); // debug
        match self.find_comm(src.serv_name()) {
            Some(comm) => {
                Msg::join(chan, comm).unwrap();
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

    fn send_msg(&mut self, from : MsgSource, msg : Vec<char>) {
        let msg_string = msg.iter().cloned().collect::<String>();

        match from {
            MsgSource::Serv { .. } => {
                self.tui.add_client_err_msg("Can't send PRIVMSG to a server.",
                                            &MsgTarget::CurrentTab);
            },

            MsgSource::Chan { serv_name, chan_name } => {
                {
                    let comm = self.find_comm(&serv_name).unwrap();
                    Msg::privmsg(&chan_name, &msg_string, comm).unwrap();
                }
                self.tui.add_privmsg(&self.nick, &msg_string,
                                     &time::now(),
                                     &MsgTarget::Chan { serv_name: &serv_name,
                                                        chan_name: &chan_name });
            },

            MsgSource::User { serv_name, nick } => {
                {
                    let comm = self.find_comm(&serv_name).unwrap();
                    Msg::privmsg(&nick, &msg_string, comm).unwrap();
                }
                self.tui.add_privmsg(&self.nick, &msg_string,
                                     &time::now(),
                                     &MsgTarget::User { serv_name: &serv_name, nick: &nick });
            }
        }
    }

    fn find_comm(&mut self, serv_name : &str) -> Option<&mut Comms> {
        for comm in self.comms.iter_mut() {
            if comm.serv_name == serv_name {
                return Some(comm);
            }
        }
        None
    }

    ////////////////////////////////////////////////////////////////////////////

    fn handle_socket(&mut self, comm_idx : usize) -> LoopRet {
        let mut disconnect : Option<libc::c_int> = None;

        let rets = {
            let mut comm = unsafe { self.comms.get_unchecked_mut(comm_idx) };
            comm.read_incoming_msg()
        };

        for ret in rets {
            // tui.show_msg_current_tab(&format!("{:?}", ret));
            writeln!(&mut io::stderr(), "incoming msg: {:?}", ret).unwrap();
            match ret {
                CommsRet::Disconnected { fd } => {
                    disconnect = Some(fd);
                },
                CommsRet::Err { err_msg } => {
                    let serv_name = &unsafe { self.comms.get_unchecked(comm_idx) }.serv_name;
                    self.tui.add_err_msg(&err_msg, &time::now(),
                                         &MsgTarget::Server { serv_name: serv_name });
                },
                CommsRet::IncomingMsg { pfx, cmd, args } => {
                    self.handle_incoming_msg(comm_idx, pfx, cmd, args, time::now());
                },
                CommsRet::SentMsg { .. } => {
                    // TODO: Probably just ignore this?
                }
            }
        }

        if let Some(fd) = disconnect {
            LoopRet::Disconnected { fd: fd }
        } else {
            LoopRet::Continue
        }
    }

    fn handle_incoming_msg(&mut self, comm_idx : usize, pfx : Pfx, cmd : Cmd, args : Vec<String>,
                           tm : time::Tm) {
        match cmd {

            ////////////////////////////////////////////////////////////////////
            // Commands, messages

            Cmd::Str(ref s) if s == "PRIVMSG" => {
                debug_assert!(args.len() == 2);
                let target = unsafe { args.get_unchecked(0) };
                let msg    = unsafe { args.get_unchecked(1) };
                let comm   = unsafe { &self.comms.get_unchecked(comm_idx) };

                let sender = match pfx {
                    Pfx::Server(_) => &comm.serv_name,
                    Pfx::User { ref nick, .. } => nick,
                };

                if target.as_bytes()[0] == b'#' {
                    self.tui.add_privmsg(sender, &msg, &tm, &MsgTarget::Chan {
                        serv_name: &comm.serv_name,
                        chan_name: &target,
                    });
                }

                else if target == &self.nick {
                    let msg_target = pfx_to_target(&pfx, &comm.serv_name);
                    // TODO: Set the topic if a new tab is created.
                    self.tui.add_privmsg(sender, msg, &tm, &msg_target);
                }

                else {
                    writeln!(self.tui, "Weird PRIVMSG target: {}", target).unwrap();
                }
            },

            Cmd::Str(ref s) if s == "JOIN" => {
                debug_assert!(args.len() == 1);
                let chan = unsafe { args.get_unchecked(0) };
                let serv_name = &unsafe { self.comms.get_unchecked(comm_idx) }.serv_name;
                match pfx {
                    Pfx::Server(_) => {
                        writeln!(self.tui, "Weird JOIN message pfx {:?}", pfx).unwrap();
                    },
                    Pfx::User { nick, .. } => {
                        if nick == self.nick {
                            self.tui.new_chan_tab(serv_name, chan);
                        } else {
                            self.tui.add_nick(
                                &nick,
                                Some(&time::now()),
                                &MsgTarget::Chan { serv_name: serv_name, chan_name: chan });
                        }
                    }
                }
            },

            Cmd::Str(ref s) if s == "PART" => {
                let serv_name = &unsafe { self.comms.get_unchecked(comm_idx) }.serv_name;
                match pfx {
                    Pfx::Server(_) => {
                        writeln!(self.tui, "Weird PART message pfx {:?}", pfx).unwrap();
                    },
                    Pfx::User { nick, .. } => {
                        for chan in args {
                            self.tui.remove_nick(
                                &nick,
                                Some(&time::now()),
                                &MsgTarget::Chan { serv_name: serv_name, chan_name: &chan });
                        }
                    }
                }
            },

            Cmd::Str(ref s) if s == "QUIT" => {
                let serv_name = &unsafe { self.comms.get_unchecked(comm_idx) }.serv_name;
                match pfx {
                    Pfx::Server(_) => {
                        writeln!(self.tui, "Weird QUIT message pfx {:?}", pfx).unwrap();
                    },
                    Pfx::User { nick, .. } => {
                        self.tui.remove_nick(
                            &nick,
                            Some(&time::now()),
                            &MsgTarget::AllUserTabs { serv_name: serv_name, nick: &nick });
                    }
                }
            },

            Cmd::Str(ref s) if s == "NOTICE" => {
                debug_assert!(args.len() == 2);
                let target = unsafe { args.get_unchecked(0) };
                let msg    = unsafe { args.get_unchecked(1) };
                let comm   = unsafe { &self.comms.get_unchecked(comm_idx) };

                if target == "*" || target == &self.nick {
                    self.tui.add_msg(&msg, &time::now(), &pfx_to_target(&pfx, &comm.serv_name));
                } else {
                    writeln!(self.tui, "Weird NOTICE target: {}", target).unwrap();
                }
            },

            Cmd::Str(ref s) if s == "NICK" => {
                match pfx {
                    Pfx::Server(_) => {
                        writeln!(self.tui, "Weird NICK message pfx {:?}", pfx).unwrap();
                    },
                    Pfx::User { nick: ref old_nick, .. } => {
                        let serv_name = &unsafe { self.comms.get_unchecked(comm_idx) }.serv_name;
                        let new_nick = &args[0];
                        self.tui.rename_nick(old_nick, new_nick, &time::now(),
                                             &MsgTarget::AllUserTabs {
                                                 serv_name: serv_name,
                                                 nick: old_nick,
                                             });
                    }
                }
            },

            ////////////////////////////////////////////////////////////////////
            // Numeric replies

            Cmd::Num(n) if n <= 003 // RPL_WELCOME, RPL_YOURHOST, RPL_CREATED
                        || n == 251 // RPL_LUSERCLIENT
                        || n == 255 // RPL_LUSERME
                        || n == 372 // RPL_MOTD
                        || n == 375 // RPL_MOTDSTART
                        || n == 376 // RPL_ENDOFMOTD
                        => {
                debug_assert!(args.len() == 2);

                // Logging just to understand more about these replies.
                // writeln!(self.tui,
                //          "Got some message:\nPfx: {:?}, Cmd: {:?}, args: {:?}",
                //          pfx, cmd, args).unwrap();

                let comm = unsafe { &self.comms.get_unchecked(comm_idx) };
                let msg  = unsafe { args.get_unchecked(1) };

                self.tui.add_msg(
                    &msg, &time::now(), &MsgTarget::Server { serv_name: &comm.serv_name });
            },

            Cmd::Num(n) if n == 4 // RPL_MYINFO
                        || n == 5 // RPL_BOUNCE
                        || (n >= 252 && n <= 254)
                                  // RPL_LUSEROP, RPL_LUSERUNKNOWN,
                                  // RPL_LUSERCHANNELS
                        => {
                let comm = unsafe { &self.comms.get_unchecked(comm_idx) };
                let msg  = args.into_iter().collect::<Vec<String>>().join(" ");
                self.tui.add_msg(
                    &msg, &time::now(), &MsgTarget::Server { serv_name: &comm.serv_name });
            }

            Cmd::Num(n) if n == 265
                        || n == 266
                        || n == 250
                        => {
                let comm = unsafe { &self.comms.get_unchecked(comm_idx) };
                let msg  = &args[args.len() - 1];
                self.tui.add_msg(
                    msg, &time::now(), &MsgTarget::Server { serv_name: &comm.serv_name });
            }

            // RPL_TOPIC
            Cmd::Num(332) => {
                // FIXME: RFC 2812 says this will have 2 arguments, but freenode
                // sends 3 arguments (extra one being our nick).
                debug_assert!(args.len() == 3 || args.len() == 2);
                let comm  = unsafe { &self.comms.get_unchecked(comm_idx) };
                let chan  = unsafe { args.get_unchecked(args.len() - 2) };
                let topic = unsafe { args.get_unchecked(args.len() - 1) };
                self.tui.set_topic(topic, &MsgTarget::Chan {
                    serv_name: &comm.serv_name,
                    chan_name: chan,
                });
            }

            // RPL_NAMREPLY: List of users in a channel
            Cmd::Num(353) => {
                let comm  = unsafe { &self.comms.get_unchecked(comm_idx) };
                let chan  = &args[2];
                let chan_target = MsgTarget::Chan {
                    serv_name: &comm.serv_name,
                    chan_name: chan,
                };


                for nick in args[3].split_whitespace() {
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
            Cmd::Num(366) => {}

            ////////////////////////////////////////////////////////////////////

            _ => {
                writeln!(self.tui,
                         "Ignoring incoming msg:\nPfx: {:?}, Cmd: {:?}, args: {:?}",
                         pfx, cmd, args).unwrap();
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
