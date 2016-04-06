#![feature(alloc_system)]

extern crate alloc_system;
extern crate libc;
extern crate rustbox;

mod comms;
mod utils;
pub mod msg;
pub mod tui;

use std::cmp::max;
use std::mem;

use comms::{Comms, CommsRet};
use msg::{Pfx, Msg};
use tui::{TUI, TUIRet};

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
        self.tui.new_server_tab("local".to_string());

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
                if self.handle_stdin() == LoopRet::Abort { return LoopRet::Abort; }
            }

            // A socket is ready
            // TODO: Can multiple sockets be set in single select() call? I
            // assume yes for now.
            else {
                for comm in self.comms.iter_mut() {
                    let fd = comm.get_raw_fd();
                    if unsafe { libc::FD_ISSET(fd, &mut fd_set_) } {
                        // TODO: Handle disconnects
                        if Tiny::handle_socket(&mut self.tui, comm) == LoopRet::Abort {
                            return LoopRet::Abort;
                        }
                    }
                }
            }
        }
    }

    ////////////////////////////////////////////////////////////////////////////

    fn handle_stdin(&mut self) -> LoopRet {
        match self.tui.keypressed() {
            TUIRet::Abort => LoopRet::Abort,
            TUIRet::Input { serv_name, pfx, msg } => {
                self.tui.show_msg_current_tab(&format!("Input serv_name: {}, pfx: {:?}, msg: {}",
                                                       serv_name, pfx,
                                                       msg.iter().cloned().collect::<String>()));
                // We know msg has at least one character as the TUI won't
                // accept it otherwise.
                if msg[0] == '/' {
                    self.handle_command(serv_name, pfx,
                                        (&msg[ 1 .. ]).into_iter().cloned().collect())
                } else {
                    self.send_msg(&serv_name, pfx.as_ref(), msg);
                    LoopRet::Continue
                }
            },
            _ => LoopRet::Continue
        }
    }

    fn handle_command(&mut self, serv_name : String, pfx : Option<Pfx>, msg : String) -> LoopRet {
        let words : Vec<&str> = msg.split_whitespace().into_iter().collect();
        if words[0] == "connect" {
            self.connect(words[1]);
            LoopRet::Continue
        } else if words[0] == "quit" {
            LoopRet::Abort
        } else {
            self.tui.show_error(
                &format!("Unsupported command: {}", words[0]), &serv_name, pfx.as_ref());
            LoopRet::Continue
        }
    }

    fn connect(&mut self, serv_name : &str) {
        match utils::drop_port(serv_name) {
            None => {
                self.tui.show_error_all_tabs("connect: Need a <host>:<port>");
            },
            Some(host) => {
                self.tui.new_server_tab(host.to_owned());
                self.tui.show_msg_current_tab(&format!("Created tab: {}", host));
                self.tui.show_msg("Connecting...", host, None);

                match Comms::try_connect(serv_name, host, &self.nick, &self.hostname, &self.realname) {
                    Ok(comms) => {
                        self.comms.push(comms);
                    },
                    Err(err) => {
                        self.tui.show_error(&format!("Error: {}", err), host, None);
                    }
                }
            }
        }
    }

    fn send_msg(&mut self, serv_name : &str, pfx : Option<&Pfx>, msg : Vec<char>) {
        if let Some(comm) = self.find_comm(serv_name) {
            let msg_target = match pfx {
                None => serv_name,
                Some(&Pfx::Server(ref serv_name)) => serv_name.as_ref(),
                Some(&Pfx::User { ref nick, .. }) => nick.as_ref(),
            };

            Msg::privmsg(msg_target, msg.into_iter().collect::<String>().as_ref(), comm).unwrap();
            return;
        }

        // OMG Rust seriously sucks. If I move this code to the else{} block I
        // get a compile error because None borrows self. So we need to make
        // sure previous blocks returns something.

        self.tui.show_error_current_tab(
            &format!("send_msg(): Can't find serv {:?} in {:?}",
                     serv_name,
                     self.comms.iter().map(|c| c.serv_name.clone()).collect::<Vec<String>>()));
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

    fn handle_socket(tui : &mut TUI, comm : &mut Comms) -> LoopRet {
        let mut disconnect : Option<libc::c_int> = None;

        for ret in comm.read_incoming_msg() {
            tui.show_msg_current_tab(&format!("{:?}", ret));
            match ret {
                CommsRet::Disconnected { fd } => {
                    disconnect = Some(fd);
                },
                CommsRet::Err { serv_name, err_msg } => {
                    tui.show_error(&err_msg, &serv_name, None);
                },
                CommsRet::IncomingMsg { serv_name, pfx, ty, msg } => {
                    tui.show_msg(&msg, serv_name, Some(&pfx));
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
}
