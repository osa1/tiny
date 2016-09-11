//! Some utilities for termbox

use termbox_simple::Termbox;

#[inline]
pub fn print(tb: &mut Termbox, mut pos_x : i32, pos_y : i32, fg : u16, bg : u16, str : &str) {
    for char in str.chars() {
        tb.change_cell(pos_x, pos_y, char, fg, bg);
        pos_x += 1;
    }
}

#[inline]
pub fn print_chars(tb: &mut Termbox, mut pos_x : i32, pos_y : i32, fg : u16, bg : u16, chars : &[char]) {
    for char in chars.iter() {
        tb.change_cell(pos_x, pos_y, *char, fg, bg);
        pos_x += 1;
    }
}
