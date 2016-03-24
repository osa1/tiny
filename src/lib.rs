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
use std::fmt::Debug;
use std::io::Read;
use std::io::Write;
use std::io;
use std::mem;
use std::net::TcpStream;
use std::os::unix::io::AsRawFd;
use std::str;
use std::time::Duration;

use cmd::Cmd;
use msg::Msg;
use tui::{TUI, TUIRet};
use utils::find_byte;

pub fn mainloop() {
    let mut tui = TUI::new();

    loop {
        match mainloop_no_stream(&mut tui) {
            None => { break; },
            Some(stream) => {
                if mainloop_stream(&mut tui, stream) {
                    break;
                }
            },
        }
    }
}

fn mainloop_no_stream(tui : &mut TUI) -> Option<TcpStream> {
    // I want my tail calls back
    loop {
        match tui.idle_loop() {
            TUIRet::SendMsg(cmd) => {
                if cmd[0] == '/' {
                    // a command attempt
                    match Cmd::parse(&cmd) {
                        Ok(Cmd::Connect(server)) => {
                            match TcpStream::connect::<&str>(server.borrow()) {
                                Err(err) => {
                                    tui.show_conn_error(err.description());
                                },
                                Ok(stream) => {
                                    return Some(stream);
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
            TUIRet::Abort => { return None; },
            ret => {
                panic!("TUI.idle_loop() returned unexpected ret value: {:?}", ret);
            }
        }
    }
}

// true -> abort
fn mainloop_stream(tui : &mut TUI, mut stream : TcpStream) -> bool {
    // Why can't disable the timeout?
    stream.set_read_timeout(Some(Duration::from_millis(1)));

    let stream_fd = stream.as_raw_fd();

    let mut fd_set : libc::fd_set = unsafe { std::mem::zeroed() };
    unsafe {
        // 0 is stdin
        libc::FD_SET(0, &mut fd_set);
        libc::FD_SET(stream_fd, &mut fd_set);
    }

    let nfds = stream_fd + 1;

    // TODO: An IRC message is at most 512-byte long (including CRLF), but can
    // the server send() multiple messages at once?
    let mut read_buf : [u8; 512] = [0; 512];

    // We collect _partial_ messages here.
    let mut msg_buf : Vec<u8> = Vec::new();

    loop {
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
            panic!("select failed!");
        }

        // stdin is ready
        if unsafe { libc::FD_ISSET(0, &mut fd_set_) } {

        }

        // Socket is ready
        else if unsafe { libc::FD_ISSET(stream_fd, &mut fd_set_) } {
            // TODO: zero means something
            let bytes_read = stream.read(&mut read_buf).unwrap();

            // Did we read any CRLFs? In that case just process the message
            // and update buffers. Otherwise push the partial message to the
            // buffer.
            {
                // (Creating a new scope for read_buf_)
                let mut read_buf_ : &[u8] = &read_buf;
                loop {
                    match find_byte(read_buf_, b'\r') {
                        None => {
                            // Push the partial message to the message buffer, keep
                            // reading until a complete message is read.
                            match find_byte(read_buf_, 0) {
                                None => { msg_buf.extend_from_slice(read_buf_); },
                                Some(slice_end) => {
                                    msg_buf.extend_from_slice(&read_buf_[ 0 .. slice_end ]);
                                }
                            };
                            break;
                        },
                        Some(cr_idx) => {
                            // msg_buf.extend_from_slice(&read_buf_[ 0 .. cr_idx ]);
                            // msg_area.add_msg_str(str::from_utf8(msg_buf.borrow()).unwrap());
                            // msg_area.add_msg_str(format!("{:?}", Msg::parse(&msg_buf)).borrow());
                            // msg_buf.clear();
                            // // Next char should be NL, skip that.
                            // read_buf_ = &read_buf_[ cr_idx + 2 .. ];
                        }
                    }
                }
            }

            read_buf = unsafe { mem::zeroed() };
        }
    }
}
