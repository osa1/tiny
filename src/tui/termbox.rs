/// Some utilities for termbox

use termbox_sys;

#[inline]
pub fn print(mut pos_x : i32, pos_y : i32, fg : u16, bg : u16, str : &str) {
    for char in str.chars() {
        print_char(pos_x, pos_y, fg, bg, char);
        pos_x += 1;
    }
}

#[inline]
pub fn print_char(pos_x : i32, pos_y : i32, fg : u16, bg : u16, char : char) {
    unsafe {
        termbox_sys::tb_change_cell(pos_x, pos_y, char as u32, fg, bg);
    }
}

#[inline]
pub fn print_chars(mut pos_x : i32, pos_y : i32, fg : u16, bg : u16, chars : &[char]) {
    for char in chars.iter() {
        print_char(pos_x, pos_y, fg, bg, *char);
        pos_x += 1;
    }
}
