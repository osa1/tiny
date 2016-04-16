extern crate libc;
extern crate rustbox;
extern crate termbox_sys;
extern crate tiny;

use std::fs::File;
use std::io::Read;
use std::mem;
use std::time::Duration;

use rustbox::{RustBox, InitOptions, InputMode, Event, Key};
use termbox_sys::{tb_change_cell, tb_width};

use tiny::tui::msg_area::line::Line;
use tiny::tui::style;

const WIDTH : i32 = 10;
const HEIGHT : i32 = 10;

fn loop_() -> Option<String> {
    let mut line = Line::new();
    {
        let mut text = String::new();
        let mut file = File::open("test/lipsum.txt").unwrap();
        file.read_to_string(&mut text);
        line.add_text(&text, style::USER_MSG);
    }

    let rustbox = RustBox::init(InitOptions {
        input_mode: InputMode::Esc,
        buffer_stderr: false,
    }).unwrap();

    // I'm using select() here to test for signals/interrupts. Namely, SIGWINCH
    // needs to be handled somehow for resizing.

    let mut fd_set : libc::fd_set = unsafe { mem::zeroed() };
    unsafe { libc::FD_SET(0, &mut fd_set); }
    let nfds = 1; // stdin + 1

    loop {
        rustbox.clear();
        line.draw(&rustbox, 0, 0, unsafe { tb_width() });
        rustbox.present();

        let mut fd_set_ = fd_set.clone();
        let ret = unsafe {
            libc::select(nfds,
                         &mut fd_set_,           // read fds
                         std::ptr::null_mut(),   // write fds
                         std::ptr::null_mut(),   // error fds
                         std::ptr::null_mut())   // timeval
        };

        if unsafe { ret == -1 || libc::FD_ISSET(0, &mut fd_set_) } {
            match rustbox.peek_event(Duration::new(0, 0), false) {
                Ok(Event::KeyEvent(Key::Esc)) => {
                    break;
                },

                Ok(Event::KeyEvent(Key::Char(ch))) => {
                    line.add_char(ch);
                }

                Ok(Event::ResizeEvent(width, height)) => {

                }

                _ => {}
            }
        }
    }

    None
}

fn main() {
    loop_().map(|err| println!("{}", err));
}
