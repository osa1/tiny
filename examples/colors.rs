extern crate rustbox;
extern crate termbox_sys;

use rustbox::{RustBox, InitOptions, InputMode};
use termbox_sys::*;

fn main() {
    let tui = RustBox::init(InitOptions {
        input_mode: InputMode::Esc,
        buffer_stderr: false,
    }).unwrap();

    unsafe {
        tb_select_output_mode(TB_OUTPUT_256);
        tb_clear();
    }

    let row = 0;
    let row = draw_range(0,   16,  row);
    let row = draw_range(16,  232, row + 1);
    let _   = draw_range(232, 256, row + 1);

    unsafe { tb_present(); }
    let _ = tui.poll_event(false);
}

fn draw_range(begin : u16, end : u16, mut row : i32) -> i32 {
    let mut col = 0;
    for i in begin .. end {
        if col != 0 && col % 24 == 0 {
            col = 0;
            row += 1;
        }

        let string = format!("{:>3}", i);
        unsafe {
            tb_change_cell(col,     row, string.chars().nth(0).unwrap() as u32, i, 0);
            tb_change_cell(col + 2, row, string.chars().nth(2).unwrap() as u32, i, 0);
            tb_change_cell(col + 1, row, string.chars().nth(1).unwrap() as u32, i, 0);
        }
        col += 4;
    }

    row + 1
}
