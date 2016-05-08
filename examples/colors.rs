extern crate rustbox;
extern crate termbox_sys;

use rustbox::{RustBox, InitOptions, InputMode, Event, Key};
use termbox_sys::*;

fn main() {
    let tui = RustBox::init(InitOptions {
        input_mode: InputMode::Esc,
        buffer_stderr: false,
    }).unwrap();

    unsafe { tb_select_output_mode(TB_OUTPUT_256); }

    let mut fg = true;
    loop {
        unsafe { tb_clear(); }

        let row = 0;
        let row = draw_range(0,   16,  row,     fg);
        let row = draw_range(16,  232, row + 1, fg);
        let _   = draw_range(232, 256, row + 1, fg);

        unsafe { tb_present(); }

        match tui.poll_event(false) {
            Ok(Event::KeyEvent(Key::Tab)) => {
                fg = !fg;
            },
            _ => { break; }
        }
    }
}

fn draw_range(begin : u16, end : u16, mut row : i32, fg : bool) -> i32 {
    let mut col = 0;
    for i in begin .. end {
        if col != 0 && col % 24 == 0 {
            col = 0;
            row += 1;
        }

        let string = format!("{:>3}", i);
        unsafe {
            let fg_ = if fg { i } else { 0 };
            let bg_ = if fg { 0 } else { i };
            tb_change_cell(col,     row, string.chars().nth(0).unwrap() as u32, fg_, bg_);
            tb_change_cell(col + 2, row, string.chars().nth(2).unwrap() as u32, fg_, bg_);
            tb_change_cell(col + 1, row, string.chars().nth(1).unwrap() as u32, fg_, bg_);
        }
        col += 4;
    }

    row + 1
}
