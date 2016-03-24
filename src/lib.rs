#![feature(alloc_system)]

extern crate alloc_system;
extern crate libc;
extern crate rustbox;

mod tui;
mod utils;
pub mod msg;
pub mod msg_area;
pub mod text_field;

use std::borrow::Borrow;
use std::fmt::Debug;
use std::io::Read;
use std::io::Write;
use std::io;
use std::mem;
use std::net::TcpStream;
use std::os::unix::io::AsRawFd;
use std::str;
use std::time::Duration;

use rustbox::{RustBox, InitOptions, InputMode, Event, Key};

use msg_area::MsgArea;
use text_field::{TextField, TextFieldRet};

use msg::Msg;
use tui::TUI;
use utils::{find_byte};

fn mainloop_no_stream(tui : &mut TUI) {
    tui.idle_loop();
}

pub fn mainloop() {
    let mut tui = TUI::new();
    mainloop_no_stream(&mut tui);

    // let stream = tui.try_unwrap();

    // // Why can't disable the timeout?
    // self.stream.set_read_timeout(Some(Duration::from_millis(1)));

    // let stream_fd = self.stream.as_raw_fd();

    // let mut fd_set : libc::fd_set = unsafe { std::mem::zeroed() };
    // unsafe {
    //     // 0 is stdin
    //     libc::FD_SET(0, &mut fd_set);
    //     libc::FD_SET(stream_fd, &mut fd_set);
    // }

    // let nfds = stream_fd + 1;

    // // From the RFC:
    // // "IRC messages are always lines of characters terminated with a CR-LF
    // // (Carriage Return - Line Feed) pair, and these messages shall not
    // // exceed 512 characters in length, counting all characters including
    // // the trailing CR-LF."
    // let mut read_buf : [u8; 512] = [0; 512];

    // // We collect partial messages here. Note that this can contain multiple
    // // messages.
    // let mut msg_buf : Vec<u8> = Vec::new();

    // let mut msg_area   = MsgArea::new(tui.width() as i32,
    //                                   tui.height() as i32 - 1);
    // let mut text_field = TextField::new(tui.width() as i32);

    // loop {
    //     let mut fd_set_ = fd_set.clone();
    //     let ret =
    //         unsafe {
    //             libc::select(nfds,
    //                          &mut fd_set_,           // read fds
    //                          std::ptr::null_mut(),   // write fds
    //                          std::ptr::null_mut(),   // error fds
    //                          std::ptr::null_mut())   // timeval
    //         };

    //     if ret == -1 {
    //         panic!("select failed!");
    //     }

    //     // stdin is ready
    //     if unsafe { libc::FD_ISSET(0, &mut fd_set_) } {
    //         match tui.peek_event(Duration::new(0, 0), false) {
    //             Ok(Event::KeyEvent(Key::Esc)) => {
    //                 break;
    //             },
    //             Ok(Event::KeyEvent(key)) => {


    //                 // TODO

    //             },
    //             Ok(_) => {},
    //             Err(_) => {}
    //         }
    //     }

    //     // Socket is ready
    //     else if unsafe { libc::FD_ISSET(stream_fd, &mut fd_set_) } {
    //         // TODO: zero means something
    //         let bytes_read = self.stream.read(&mut read_buf).unwrap();

    //         // Did we read any CRLFs? In that case just process the message
    //         // and update buffers. Otherwise push the partial message to the
    //         // buffer.

    //         // Creating a new scope for read_buf_
    //         {
    //             let mut read_buf_ : &[u8] = &read_buf;
    //             loop {
    //                 match find_byte(read_buf_, b'\r') {
    //                     None => {
    //                         // Push the partial message to the message buffer, keep
    //                         // reading until a complete message is read.
    //                         match find_byte(read_buf_, 0) {
    //                             None => { msg_buf.extend_from_slice(read_buf_); },
    //                             Some(slice_end) => {
    //                                 msg_buf.extend_from_slice(&read_buf_[ 0 .. slice_end ]);
    //                             }
    //                         };
    //                         break;
    //                     },
    //                     Some(cr_idx) => {
    //                         msg_buf.extend_from_slice(&read_buf_[ 0 .. cr_idx ]);
    //                         msg_area.add_msg_str(str::from_utf8(msg_buf.borrow()).unwrap());
    //                         msg_area.add_msg_str(format!("{:?}", Msg::parse(&msg_buf)).borrow());
    //                         msg_buf.clear();
    //                         // Next char should be NL, skip that.
    //                         read_buf_ = &read_buf_[ cr_idx + 2 .. ];
    //                     }
    //                 }
    //             }
    //         }

    //         read_buf = unsafe { mem::zeroed() };
    //     }

    //     tui.clear();
    //     msg_area.draw(&tui, 0, 0);
    //     text_field.draw(&tui, 0, (self.rustbox.height() - 1) as i32);
    //     tui.present();
    // }
}

fn try_unwrap<T, E : Debug>(tui : RustBox, res : Result<T, E>) -> Option<T> {
    panic!()
}
