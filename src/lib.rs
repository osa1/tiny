#![feature(alloc_system)]

extern crate alloc_system;
extern crate libc;
extern crate rustbox;

mod cmd;
mod tui;
mod utils;
pub mod msg;
pub mod msg_area;
pub mod text_field;

use std::borrow::Borrow;
use std::error::Error;
use std::ffi::CString;
use std::io::Read;
use std::io::Write;
use std::io;
use std::mem;
use std::net::TcpStream;
use std::os::unix::io::AsRawFd;
use std::str;
use std::time::Duration;

use cmd::Cmd;
use tui::{TUI, TUIRet};
use utils::find_byte;

pub struct Tiny {
    /// Buffer used to read bytes from the socket.
    read_buf : [u8; 512],

    /// _Partial_ messages collected here until they make a complete message.
    msg_buf  : Vec<u8>,

    /// TCP stream to the IRC server.
    stream   : Option<TcpStream>,
}

#[derive(PartialEq, Eq)]
enum LoopRet { Abort, Continue, Disconnected }

impl Tiny {
    pub fn new() -> Tiny {
        Tiny {
            read_buf: [0; 512],
            msg_buf: Vec::new(),
            stream: None,
        }
    }

    pub fn mainloop(&mut self) {
        let mut tui = TUI::new();

        loop {
            match self.stream {
                None => {
                    if self.mainloop_no_stream(&mut tui) == LoopRet::Abort { break; }
                },
                Some(_) => {
                    if self.mainloop_stream(&mut tui) == LoopRet::Abort { break; }
                }
            }
        }
    }

    fn mainloop_no_stream(&mut self, tui : &mut TUI) -> LoopRet {
        // I want my tail calls back
        loop {
            match tui.idle_loop() {
                TUIRet::SendMsg(cmd) => {
                    if cmd[0] == '/' {
                        // a command attempt
                        match Cmd::parse(&cmd) {
                            Ok(Cmd::Connect(server)) => {
                                writeln!(io::stderr(), "trying to connect: {}", server).unwrap();
                                match TcpStream::connect::<&str>(server.borrow()) {
                                    Err(err) => {
                                        tui.show_conn_error(err.description());
                                    },
                                    Ok(stream) => {
                                        self.stream = Some(stream);
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

    fn mainloop_stream(&mut self, tui : &mut TUI) -> LoopRet {

        ////////////////////
        // Set up the socket

        // Why can't disable the timeout?
        self.stream.as_ref().unwrap().set_read_timeout(Some(Duration::from_millis(1))).unwrap();

        //////////////////////////////////////
        // Set up the descriptors for select()

        let stream_fd = self.stream.as_ref().unwrap().as_raw_fd();

        let mut fd_set : libc::fd_set = unsafe { mem::zeroed() };
        unsafe {
            // 0 is stdin
            libc::FD_SET(0, &mut fd_set);
            libc::FD_SET(stream_fd, &mut fd_set);
        }

        let nfds = stream_fd + 1;

        /////////////////
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
            TUIRet::SendMsg(msg) => {
                let msg_str : String = msg.iter().cloned().collect();
                writeln!(io::stderr(), "sending msg: {}", msg_str).unwrap();
                self.stream.as_ref()
                           .unwrap()
                           .write_all(msg_str.as_bytes()).unwrap();
                tui.show_outgoing_msg(msg_str.borrow());
                LoopRet::Continue
            },
            _ => LoopRet::Continue
        }
    }

    fn handle_socket(&mut self, tui : &mut TUI) -> LoopRet {
        // Handle disconnects
        match self.stream.as_ref().unwrap().read(&mut self.read_buf) {
            Err(_) => {
                // TODO: I don't understand why this happens. I'm randomly
                // getting "temporarily unavailable" errors.
                // tui.show_conn_error(format!("Connection lost: {}", err).borrow());
                // return LoopRet::Disconnected;
            },
            Ok(bytes_read) => {
                if bytes_read == 0 {
                    tui.show_conn_error("Connection lost");
                    return LoopRet::Disconnected;
                }
            }
        }

        // Have we read any CRLFs? In that case just process the message and
        // update the buffers. Otherwise just push the partial message to the
        // buffer.
        {
            // (Creating a new scope for read_buf_)
            let mut read_buf_ : &[u8] = &self.read_buf;
            loop {
                match find_byte(read_buf_, b'\r') {
                    None => {
                        // Push the partial message to the message buffer, keep
                        // reading until a complete message is read.
                        match find_byte(read_buf_, 0) {
                            None => {
                                self.msg_buf.extend_from_slice(read_buf_);
                            },
                            Some(slice_end) => {
                                self.msg_buf.extend_from_slice(&read_buf_[ 0 .. slice_end ]);
                            }
                        };
                        break;
                    },
                    Some(cr_idx) => {
                        self.msg_buf.extend_from_slice(&read_buf_[ 0 .. cr_idx ]);
                        match str::from_utf8(self.msg_buf.borrow()) {
                            Err(err) =>
                                tui.show_conn_error(
                                    format!("Can't parse incoming message: {}", err).borrow()),
                            Ok(str) => tui.show_incoming_msg(str),
                        }

                        self.msg_buf.clear();

                        // Next char should be NL, skip that.
                        read_buf_ = &read_buf_[ cr_idx + 2 .. ];
                    }
                }
            }
        }

        self.read_buf = unsafe { mem::zeroed() };

        LoopRet::Continue
    }

    fn reset_state(&mut self) {
        self.stream = None;
        self.msg_buf.clear();
    }
}
