extern crate libc;
extern crate term_input;
extern crate termbox_simple;

use std::mem;

use term_input::{Input, Event, Key};
use termbox_simple::*;

fn main() {
    let mut tui = Termbox::init().unwrap();
    tui.set_output_mode(OutputMode::Output256);
    tui.set_clear_attributes(0, 0);

    let mut input = Input::new();
    let mut ev_buffer : Vec<Event> = Vec::new();

    let mut fg = true;

    // Set up the descriptors for select()
    let mut fd_set : libc::fd_set = unsafe { mem::zeroed() };
    unsafe { libc::FD_SET(libc::STDIN_FILENO, &mut fd_set); }

    'mainloop:
    loop {
        tui.clear();

        let row = 0;
        let row = draw_range(&mut tui, 0,   16,  row,     fg);
        let row = draw_range(&mut tui, 16,  232, row + 1, fg);
        let _   = draw_range(&mut tui, 232, 256, row + 1, fg);

        tui.present();

        let mut fd_set_ = fd_set.clone();
        let ret =
            unsafe {
                libc::select(libc::STDIN_FILENO + 1,
                             &mut fd_set_,           // read fds
                             std::ptr::null_mut(),   // write fds
                             std::ptr::null_mut(),   // error fds
                             std::ptr::null_mut())   // timeval
            };

        if unsafe { ret == -1 || libc::FD_ISSET(libc::STDIN_FILENO, &mut fd_set_) } {
            input.read_input_events(&mut ev_buffer);
            for ev in ev_buffer.iter() {
                match ev {
                    &Event::Key(Key::Tab) => {
                        fg = !fg;
                    },
                    &Event::Key(Key::Esc) => { break 'mainloop; },
                    _ => {},
                }
            }
        }
    }
}

fn draw_range(tui : &mut Termbox, begin : u16, end : u16, mut row : i32, fg : bool) -> i32 {
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
