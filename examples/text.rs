extern crate libc;
extern crate mio;
extern crate term_input;
extern crate termbox_simple;
extern crate tiny;

use mio::Events;
use mio::Poll;
use mio::PollOpt;
use mio::Ready;
use mio::Token;
use mio::unix::EventedFd;
use std::fs::File;
use std::io::Read;
// use std::io::Write;
// use std::io;

use term_input::{Input, Event, Key};
use termbox_simple::*;

use tiny::config;
use tiny::tui::msg_area::MsgArea;

fn main() {
    let mut tui = Termbox::init().unwrap();
    tui.set_output_mode(OutputMode::Output256);
    tui.set_clear_attributes(0, 0);

    let mut msg_area = MsgArea::new(tui.width(), tui.height());

    {
        let mut text = String::new();
        let mut file = File::open("test/lipsum.txt").unwrap();
        file.read_to_string(&mut text).unwrap();
        let single_line_text = text.lines().collect::<Vec<&str>>().join("");
        msg_area.set_style(config::get_theme().err_msg);
        msg_area.add_text(&single_line_text);
        // writeln!(io::stderr(), "full text added: {}", single_line_text).unwrap();
        msg_area.flush_line();

        for line in text.lines() {
            msg_area.set_style(config::get_theme().topic);
            msg_area.add_text(">>>");
            msg_area.set_style(config::get_theme().user_msg);
            msg_area.add_text("  ");
            msg_area.add_text(line);
            msg_area.flush_line();
        }
    }

    let poll = Poll::new().unwrap();
    poll.register(
        &EventedFd(&libc::STDIN_FILENO),
        Token(libc::STDIN_FILENO as usize),
        Ready::readable(),
        PollOpt::level()).unwrap();

    tui.clear();
    msg_area.draw(&mut tui, 0, 0);
    tui.present();

    let mut ev_buffer: Vec<Event> = Vec::new();
    let mut input = Input::new();
    let mut events = Events::with_capacity(10);
    'mainloop:
    loop {
        match poll.poll(&mut events, None) {
            Err(_) => {
                tui.resize();
                msg_area.resize(tui.width(), tui.height());

                tui.clear();
                msg_area.draw(&mut tui, 0, 0);
                tui.present();
            }
            Ok(_) => {
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

                        // This does not work anymore: ev_loop handles SIGWINCH signals
                        // Event::Resize => {
                        //     ctx.0.resize();
                        //     ctx.1.resize(ctx.0.width(), ctx.0.height());
                        // }

                        _ => {}
                    }
                }

                tui.clear();
                msg_area.draw(&mut tui, 0, 0);
                tui.present();
            }
        }
    }
}
