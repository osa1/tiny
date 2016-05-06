extern crate libc;
extern crate rustbox;
extern crate termbox_sys;
extern crate tiny;

use std::fs::File;
use std::io::Read;
use std::mem;
use std::time::Duration;

use std::io;
use std::io::Write;

use rustbox::{RustBox, InitOptions, InputMode, Event, Key};

use tiny::tui::msg_area::MsgArea;
use tiny::tui::style;

fn loop_() -> Option<String> {
    let rustbox = RustBox::init(InitOptions {
        input_mode: InputMode::Esc,
        buffer_stderr: false,
    }).unwrap();

    let mut msg_area = MsgArea::new(rustbox.width() as i32, rustbox.height() as i32);

    {
        let mut text = String::new();
        let mut file = File::open("test/lipsum.txt").unwrap();
        file.read_to_string(&mut text).unwrap();
        let single_line_text = text.lines().collect::<Vec<&str>>().join("");
        msg_area.set_style(&style::ERR_MSG);
        msg_area.add_text(&single_line_text);
        writeln!(io::stderr(), "full text added: {}", single_line_text).unwrap();
        msg_area.flush_line();

        for line in text.lines() {
            msg_area.set_style(&style::TOPIC);
            msg_area.add_text(">>>");
            msg_area.set_style(&style::SERVER_MSG);
            msg_area.add_text("  ");
            msg_area.add_text(line);
            msg_area.flush_line();
        }
    }

    // I'm using select() here to test for signals/interrupts. Namely, SIGWINCH
    // needs to be handled somehow for resizing.

    let mut fd_set : libc::fd_set = unsafe { mem::zeroed() };
    unsafe { libc::FD_SET(0, &mut fd_set); }
    let nfds = 1; // stdin + 1

    loop {
        rustbox.clear();
        msg_area.draw(&rustbox, 0, 0);
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

                Ok(Event::KeyEvent(Key::Ctrl('p'))) => {
                    msg_area.scroll_up();
                },

                Ok(Event::KeyEvent(Key::Ctrl('n'))) => {
                    msg_area.scroll_down();
                },

                Ok(Event::KeyEvent(Key::PageUp)) => {
                    msg_area.page_up();
                },

                Ok(Event::KeyEvent(Key::PageDown)) => {
                    msg_area.page_down();
                },

                // Ok(Event::KeyEvent(Key::Char(ch))) => {
                // }

                Ok(Event::ResizeEvent(width, height)) => {
                    msg_area.resize(width, height);
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
