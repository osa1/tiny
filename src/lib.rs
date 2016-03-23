#![feature(alloc_system)]

extern crate alloc_system;
extern crate libc;
extern crate rustbox;

pub mod msg_area;
pub mod text_field;
pub mod msg;

use std::io::Read;
use std::io::Write;
use std::net::TcpStream;
use std::os::unix::io::AsRawFd;
use std::time::Duration;

use rustbox::{RustBox, InitOptions, InputMode, Event, Key};

use msg_area::MsgArea;
use text_field::{TextField, TextFieldRet};

use msg::Msg;

pub struct Tiny {
    stream : TcpStream,
}

impl Tiny {
    pub fn init() -> Tiny {
        let stream = TcpStream::connect("chat.freenode.org:6665").unwrap();
        // Why can't disable the timeout?
        stream.set_read_timeout(Some(Duration::from_millis(1)));

        Tiny {
            stream: stream,
        }
    }

    pub fn mainloop(&mut self) {
        let rustbox = RustBox::init(InitOptions {
            input_mode: InputMode::Esc,
            buffer_stderr: false,
        }).unwrap();

        let stream_fd = self.stream.as_raw_fd();

        let mut fd_set : libc::fd_set = unsafe { std::mem::zeroed() };
        unsafe {
            // 0 is stdin
            libc::FD_SET(0, &mut fd_set);
            libc::FD_SET(stream_fd, &mut fd_set);
        }

        let nfds = stream_fd + 1;

        // From the RFC:
        // "IRC messages are always lines of characters terminated with a CR-LF
        // (Carriage Return - Line Feed) pair, and these messages shall not
        // exceed 512 characters in length, counting all characters including
        // the trailing CR-LF."
        let mut msg_buf : [u8; 512] = [0; 512];

        let mut msg_area   = MsgArea::new(rustbox.height() as i32 - 1);
        let mut text_field = TextField::new(rustbox.width() as i32);

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

            if unsafe { libc::FD_ISSET(0, &mut fd_set_) } {
                match rustbox.peek_event(Duration::new(0, 0), false) {
                    Ok(Event::KeyEvent(Key::Esc)) => {
                        break;
                    },
                    Ok(Event::KeyEvent(key)) => {
                        // TODO
                    },
                    Ok(_) => {},
                    Err(_) => {}
                }
            } else if unsafe { libc::FD_ISSET(stream_fd, &mut fd_set_) } {
                let bytes_read = self.stream.read(&mut msg_buf).unwrap();
                // msg_area.add_msg(&msg_buf);
            }

            rustbox.clear();
            msg_area.draw(&rustbox, 0, 0);
            text_field.draw(&rustbox, 0, (rustbox.height() - 1) as i32);
            rustbox.present();
        }
    }
}
