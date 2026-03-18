//! Some utilities for termbox

use crate::config::Style;
use termbox_simple::Termbox;
use unicode_width::UnicodeWidthChar;

pub(crate) fn print_chars<C>(
    tb: &mut Termbox,
    mut pos_x: i32,
    pos_y: i32,
    style: Style,
    chars: C,
) -> i32
where
    C: Iterator<Item = char>,
{
    for char in chars {
        let char_width = UnicodeWidthChar::width(char).unwrap_or(1) as i32;
        tb.change_cell(pos_x, pos_y, char, style.fg, style.bg);
        // For wide characters (like CJK), we need to skip the appropriate number of columns
        pos_x += char_width;
    }

    pos_x
}
