extern crate ev_loop;
extern crate term_input;
extern crate termbox_simple;
extern crate libc;

use ev_loop::{EvLoop, READ_EV};
use term_input::{Input, Event, Key};
use termbox_simple::*;

fn main() {
    let mut tui = Termbox::init().unwrap();
    tui.set_output_mode(OutputMode::Output256);
    tui.set_clear_attributes(0, 0);

    let mut input = Input::new();
    let mut ev_buffer: Vec<Event> = Vec::new();

    let mut fg = true;
    draw(&mut tui, fg);

    let mut ev_loop: EvLoop<Termbox> = EvLoop::new();

    ev_loop.add_fd(libc::STDIN_FILENO, READ_EV, Box::new(move |_, ctrl, tui| {
        input.read_input_events(&mut ev_buffer);
        for ev in ev_buffer.iter() {
            match ev {
                &Event::Key(Key::Tab) => {
                    fg = !fg;
                },
                &Event::Key(Key::Esc) => {
                    ctrl.stop();
                },
                _ => {},
            }
        }
        draw(tui, fg);
    }));

    ev_loop.run(tui);
}

fn draw(tui: &mut Termbox, fg: bool) {
    tui.clear();

    let row = 0;
    let row = draw_range(tui, 0,   16,  row,     fg);
    let row = draw_range(tui, 16,  232, row + 1, fg);
    let _   = draw_range(tui, 232, 256, row + 1, fg);

    tui.present();
}

fn draw_range(tui: &mut Termbox, begin: u16, end: u16, mut row: i32, fg: bool) -> i32 {
    let mut col = 0;
    for i in begin .. end {
        if col != 0 && col % 24 == 0 {
            col = 0;
            row += 1;
        }

        let string = format!("{:>3}", i);
        let fg_ = if fg { i } else { 0 };
        let bg_ = if fg { 0 } else { i };
        tui.change_cell(col,     row, string.chars().nth(0).unwrap(), fg_, bg_);
        tui.change_cell(col + 2, row, string.chars().nth(2).unwrap(), fg_, bg_);
        tui.change_cell(col + 1, row, string.chars().nth(1).unwrap(), fg_, bg_);
        col += 4;
    }

    row + 1
}
