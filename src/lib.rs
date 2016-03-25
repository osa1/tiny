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
use std::error::Error;
use std::ffi::CString;
use std::io::Write;
use std::io;
use std::mem;

use cmd::Cmd;
use comms::{Comms, CommsRet};
use tui::{TUI, TUIRet};

pub struct Tiny {
    /// A connection to a server is maintained by 'Comms'. No 'Comms' mean no
    /// connection.
    comms : Option<Comms>,
}

#[derive(PartialEq, Eq)]
enum LoopRet { Abort, Continue, Disconnected }

impl Tiny {
    pub fn new() -> Tiny {
        Tiny {
            comms: None,
        }
    }

    pub fn mainloop(&mut self) {
        let mut tui = TUI::new();

        loop {
            match self.comms {
                None => {
                    if self.mainloop_no_comms(&mut tui) == LoopRet::Abort { break; }
                },
                Some(_) => {
                    if self.mainloop_comms(&mut tui) == LoopRet::Abort { break; }
                }
            }
        }
    }

    fn mainloop_no_comms(&mut self, tui : &mut TUI) -> LoopRet {
        // I want my tail calls back
        loop {
            match tui.idle_loop() {
                TUIRet::SendMsg(cmd) => {
                    if cmd[0] == '/' {
                        // a command attempt
                        match Cmd::parse(&cmd) {
                            Ok(Cmd::Connect(server)) => {
                                writeln!(io::stderr(), "trying to connect: {}", server).unwrap();
                                match Comms::try_connect(server.borrow()) {
                                    Err(err) => {
                                        tui.show_conn_error(err.description());
                                    },
                                    Ok(comms) => {
                                        self.comms = Some(comms);
                                        return LoopRet::Continue;
                                    }
                                }
                            },
                            // Ok(_) => {
                            //     tui.show_conn_error("Not connected.");
                            // },
                            Err(err_msg) => {
                                tui.show_user_error(err_msg.borrow());
                            }
                        }
                    } else {
                        // Trying to send a message - not going to happen
                        tui.show_user_error("Can't send message - not connected to a server.");
                    }
                },
                TUIRet::Abort => { return LoopRet::Abort; },
                ret => {
                    panic!("TUI.idle_loop() returned unexpected ret value: {:?}", ret);
                }
            }
        }
    }

    fn mainloop_comms(&mut self, tui : &mut TUI) -> LoopRet {
        // Set up the descriptors for select()
        let stream_fd = self.comms.as_ref().unwrap().get_raw_fd();

        let mut fd_set : libc::fd_set = unsafe { mem::zeroed() };
        unsafe {
            // 0 is stdin
            libc::FD_SET(0, &mut fd_set);
            libc::FD_SET(stream_fd, &mut fd_set);
        }

        let nfds = stream_fd + 1;

        // Start the loop
        self.mainloop_stream_(tui, fd_set, stream_fd, nfds)
    }

    fn mainloop_stream_(&mut self, tui : &mut TUI,
                        fd_set    : libc::fd_set,
                        stream_fd : libc::c_int,
                        nfds      : libc::c_int) -> LoopRet {
        loop {
            tui.draw();

            let mut fd_set_ = fd_set.clone();
            let ret =
                unsafe {
                    libc::select(nfds,
                                 &mut fd_set_,           // read fds
                                 std::ptr::null_mut(),   // write fds
                                 std::ptr::null_mut(),   // error fds
                                 std::ptr::null_mut())   // timeval
                };

            if ret == -1 {
                let err_c_msg =
                    unsafe { CString::from_raw(libc::strerror(*libc::__errno_location())) };

                tui.show_conn_error(
                    format!("Internal error: select() failed: {}",
                            err_c_msg.to_str().unwrap()).borrow());
            }

            // stdin is ready
            else if unsafe { libc::FD_ISSET(0, &mut fd_set_) } {
                if self.handle_stdin(tui) == LoopRet::Abort { return LoopRet::Abort; }
            }

            // Socket is ready
            else if unsafe { libc::FD_ISSET(stream_fd, &mut fd_set_) } {
                // TODO: Handle disconnects
                match self.handle_socket(tui) {
                    LoopRet::Abort => { return LoopRet::Abort; },
                    LoopRet::Disconnected => {
                        self.reset_state();
                        return LoopRet::Disconnected;
                    },
                    LoopRet::Continue => {}
                }
                if self.handle_socket(tui) == LoopRet::Abort { return LoopRet::Abort; }
            }
        }
    }

    fn handle_stdin(&mut self, tui : &mut TUI) -> LoopRet {
        match tui.keypressed() {
            TUIRet::Abort => LoopRet::Abort,
            TUIRet::EventIgnored(_) => {
                // TODO: What to do here?
                LoopRet::Continue
            },
            TUIRet::SendMsg(mut msg) => {
                // Add CR-LF and send
                msg.push('\r');
                msg.push('\n');
                {
                    let msg_str : String = msg.iter().cloned().collect();
                    self.comms.as_mut().unwrap().send_raw(msg_str.as_bytes()).unwrap();
                }

                // Use another version without CR-LF to show the message
                {
                    let msg_slice : &[char] = msg.borrow();
                    let msg_slice : &[char] = &msg_slice[ 0 .. msg.len() - 2]; // Drop CRLF
                    let msg_str : String = msg_slice.iter().cloned().collect();
                    let msg_slice : &str = msg_str.borrow();
                    writeln!(io::stderr(), "sending msg: {}", msg_slice).unwrap();
                    tui.show_outgoing_msg(msg_slice);
                }

                LoopRet::Continue
            },
            _ => LoopRet::Continue
        }
    }

    fn handle_socket(&mut self, tui : &mut TUI) -> LoopRet {
        let mut disconnect = false;

        for ret in self.comms.as_mut().unwrap().read_incoming_msg() {
            match ret {
                CommsRet::Disconnected => {
                    disconnect = true;
                },
                CommsRet::ShowErr(err) => {
                    tui.show_conn_error(err.borrow());
                },
                CommsRet::ShowIncomingMsg(msg) => {
                    tui.show_incoming_msg(msg.borrow());
                },
                CommsRet::ShowServerMsg { ty, msg } => {
                    tui.show_server_msg(ty.borrow(), msg.borrow());
                }
            }
        }

        if disconnect { LoopRet::Disconnected } else { LoopRet::Continue }
    }

    fn reset_state(&mut self) {
        self.comms = None;
    }
}
