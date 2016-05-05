/// Some utilities for termbox

use termbox_sys;

#[inline]
pub fn print(pos_x : i32, pos_y : i32, fg : u16, bg : u16, str : &str) {
    for (char_idx, char) in str.chars().enumerate() {
        print_char(pos_x + (char_idx as i32), pos_y, fg, bg, char);
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
