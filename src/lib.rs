#![feature(alloc_system)]

extern crate alloc_system;
extern crate libc;
extern crate rustbox;

mod cmd;
mod comms;
mod utils;
pub mod msg;
pub mod tui;

use std::borrow::Borrow;
use std::cmp::max;
use std::error::Error;
use std::ffi::CStr;
use std::io::Write;
use std::io;
use std::mem;

use cmd::Cmd;
use comms::{Comms, CommsRet};
use msg::Pfx;
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
    // Disconnected { fd : libc::c_int }
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
            TUIRet::Input { serv_name, pfx, mut msg } => {
                // We know msg has at least one character as the TUI won't
                // accept it otherwise.
                if msg[0] == '/' {
                    self.handle_command(serv_name, pfx,
                                        (&msg[ 1 .. ]).into_iter().cloned().collect())
                } else {
                    self.send_msg(serv_name, pfx, msg)
                }
            },
            _ => LoopRet::Continue
        }
    }

    fn handle_command(&mut self, serv_name : String, pfx : Pfx, msg : String) -> LoopRet {
        let words : Vec<&str> = msg.split_whitespace().into_iter().collect();
        if words[0] == "connect" {
            self.connect(words[1]);
            LoopRet::Continue
        } else if words[0] == "quit" {
            LoopRet::Abort
        } else {
            self.tui.show_cmd_error(format!("Unsupported command: {}", words[0]).borrow());
            LoopRet::Continue
        }
    }

    fn connect(&mut self, serv_name : &str) {
        match serv_name.find(':') {
            None => {
                self.tui.show_conn_error("connect: Need a <host>:<port>");
            },
            Some(split) => {
                let host = &serv_name[ 0 .. split ];
                self.tui.new_server_tab(host.to_owned());
                self.tui.show_server_msg(host, "Connecting...");

                // TODO: This will block the UI

                match Comms::try_connect(serv_name, &self.nick, &self.hostname, &self.realname) {
                    Ok(comms) => {
                        self.comms.push(comms);
                    },
                    Err(err) => {
                        self.tui.show_server_err(host, &format!("Error: {}", err));
                    }
                }
            }
        }
    }

    fn send_msg(&mut self, serv_name : String, pfx : Pfx, msg : Vec<char>) -> LoopRet {
        panic!()
    }

    ////////////////////////////////////////////////////////////////////////////

    fn handle_socket(tui : &mut TUI, comm : &mut Comms) -> LoopRet {
        panic!()
    }
}
