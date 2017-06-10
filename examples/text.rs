extern crate ev_loop;
extern crate libc;
extern crate term_input;
extern crate termbox_simple;
extern crate tiny;

use std::fs::File;
use std::io::Read;

use std::io;
use std::io::Write;

use term_input::{Input, Event, Key};
use termbox_simple::*;
use ev_loop::{EvLoop, READ_EV};

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
        msg_area.set_style(config::ERR_MSG);
        msg_area.add_text(&single_line_text);
        writeln!(io::stderr(), "full text added: {}", single_line_text).unwrap();
        msg_area.flush_line();

        for line in text.lines() {
            msg_area.set_style(config::TOPIC);
            msg_area.add_text(">>>");
            msg_area.set_style(config::USER_MSG);
            msg_area.add_text("  ");
            msg_area.add_text(line);
            msg_area.flush_line();
        }
    }

    let mut ev_loop: EvLoop<(Termbox, MsgArea)> = EvLoop::new();

    {
        let mut sig_mask: libc::sigset_t = unsafe { std::mem::zeroed() };
        unsafe {
            libc::sigemptyset(&mut sig_mask as *mut libc::sigset_t);
            libc::sigaddset(&mut sig_mask as *mut libc::sigset_t, libc::SIGWINCH);
        };

        ev_loop.add_signal(&sig_mask, Box::new(|_, ctx| {
            ctx.0.resize();
            ctx.1.resize(ctx.0.width(), ctx.0.height());

            ctx.0.clear();
            ctx.1.draw(&mut ctx.0, 0, 0);
            ctx.0.present();
        }));
    }

    {
        let mut ev_buffer: Vec<Event> = Vec::new();
        let mut input = Input::new();
        ev_loop.add_fd(libc::STDIN_FILENO, READ_EV, Box::new(move |_, ctrl, ctx| {
            input.read_input_events(&mut ev_buffer);
            for ev in ev_buffer.drain(0 ..) {
                match ev {
                    Event::Key(Key::Esc) => {
                        ctrl.stop();
                    },

                    Event::Key(Key::Ctrl('p')) => {
                        ctx.1.scroll_up();
                    },

                    Event::Key(Key::Ctrl('n')) => {
                        ctx.1.scroll_down();
                    },

                    Event::Key(Key::PageUp) => {
                        ctx.1.page_up();
                    },

                    Event::Key(Key::PageDown) => {
                        ctx.1.page_down();
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

            ctx.0.clear();
            ctx.1.draw(&mut ctx.0, 0, 0);
            ctx.0.present();
        }));
    }

    tui.clear();
    msg_area.draw(&mut tui, 0, 0);
    tui.present();

    ev_loop.run((tui, msg_area));
}
