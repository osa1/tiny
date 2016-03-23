#![feature(alloc_system)]

extern crate alloc_system;
extern crate libc;
extern crate rustbox;

mod msg_widget;
mod text_field;

use std::io::Read;
use std::io::Write;
use std::net::TcpStream;
use std::os::unix::io::AsRawFd;
use std::time::Duration;

use rustbox::{RustBox, InitOptions, InputMode, Event, Key};

use msg_widget::MsgWidget;
use text_field::{TextField, TextFieldRet};

fn main() {
    let rustbox = RustBox::init(InitOptions {
        input_mode: InputMode::Esc,
        buffer_stderr: false,
    }).unwrap();

    let mut stream = TcpStream::connect("chat.freenode.org:6665").unwrap();
    stream.set_read_timeout(Some(Duration::from_millis(1)));
    let stream_fd = stream.as_raw_fd();

    let mut fd_set : libc::fd_set = unsafe { std::mem::zeroed() };
    unsafe {
        // 0 is stdin
        libc::FD_SET(0, &mut fd_set);
        libc::FD_SET(stream.as_raw_fd(), &mut fd_set);
    }

    let nfds = stream_fd + 1;

    // From the RFC:
    // "IRC messages are always lines of characters terminated with a CR-LF
    // (Carriage Return - Line Feed) pair, and these messages shall not exceed
    // 512 characters in length, counting all characters including the trailing
    // CR-LF."
    let mut msg_buf : [u8; 512] = [0; 512];

    // println!("Socket's read timeout: {:?}", stream.read_timeout());

    let mut msg_widget = MsgWidget::new();
    let mut text_field = TextField::new();

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
            panic!("select returned zero!");
        }

        if unsafe { libc::FD_ISSET(0, &mut fd_set_) } {
            match rustbox.peek_event(Duration::new(0, 0), false) {
                Ok(Event::KeyEvent(Key::Esc)) => {
                    break;
                },
                Ok(Event::KeyEvent(key)) => {
                    match text_field.keypressed(key) {
                        TextFieldRet::SendMsg => {
                            stream.write(text_field.get_msg().as_bytes());
                        },
                        TextFieldRet::Nothing => {},
                    }
                },
                Ok(_) => {},
                Err(_) => {}
            }
        } else if unsafe { libc::FD_ISSET(stream_fd, &mut fd_set_) } {
            let bytes_read = stream.read(&mut msg_buf).unwrap();
            msg_widget.add_irc_raw_msg(&msg_buf);
        }

        rustbox.clear();
        msg_widget.draw(&rustbox, 0, 0, rustbox.width() as i32, (rustbox.height() - 1) as i32);
        text_field.draw(&rustbox, 0, (rustbox.height() - 1) as i32, 0, 0);
        rustbox.present();
    }
}
