extern crate libc;
extern crate term_input;
extern crate termbox_simple;
extern crate tiny;

use std::fs::File;
use std::io::Read;
use std::mem;

use std::io;
use std::io::Write;

use term_input::{Input, Event, Key};
use termbox_simple::*;

use tiny::tui::msg_area::MsgArea;
use tiny::tui::style;

fn loop_() -> Option<String> {
    let mut tui = Termbox::init().unwrap();
    tui.set_output_mode(OutputMode::Output256);
    tui.set_clear_attributes(0, 0);

    let mut msg_area = MsgArea::new(tui.width(), tui.height());

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
    unsafe { libc::FD_SET(libc::STDIN_FILENO, &mut fd_set); }
    let nfds = libc::STDIN_FILENO + 1;

    let mut input = Input::new();
    let mut ev_buffer : Vec<Event> = Vec::new();

    'mainloop:
    loop {
        tui.clear();
        msg_area.draw(&mut tui, 0, 0);
        tui.present();

        let mut fd_set_ = fd_set.clone();
        let ret = unsafe {
            libc::select(nfds,
                         &mut fd_set_,           // read fds
                         std::ptr::null_mut(),   // write fds
                         std::ptr::null_mut(),   // error fds
                         std::ptr::null_mut())   // timeval
        };

        if unsafe { ret == -1 || libc::FD_ISSET(0, &mut fd_set_) } {
            input.read_input_events(&mut ev_buffer);
            for ev in ev_buffer.drain(0 ..) {
                match ev {
                    Event::Key(Key::Esc) => {
                        break 'mainloop;
                    },

                    Event::Key(Key::Ctrl('p')) => {
                        msg_area.scroll_up();
                    },

                    Event::Key(Key::Ctrl('n')) => {
                        msg_area.scroll_down();
                    },

                    Event::Key(Key::PageUp) => {
                        msg_area.page_up();
                    },

                    Event::Key(Key::PageDown) => {
                        msg_area.page_down();
                    },

                    // Ok(Event::KeyEvent(Key::Char(ch))) => {
                    // }

                    Event::Resize => {
                        tui.resize();
                        msg_area.resize(tui.width(), tui.height());
                    }

                    _ => {}
                }
            }
        }
    }

    None
}

fn main() {
    loop_().map(|err| println!("{}", err));
}
