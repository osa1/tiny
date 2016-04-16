// use std::io;
// use std::io::Write;

use rustbox::{RustBox};
use termbox_sys::tb_change_cell;

use tui::style::{StyleRef};
use tui::style;

/// A single line added to the widget. May be rendered as multiple lines on the
/// screen.
#[derive(Debug)]
pub struct Line {
    /// Note that this String may not be directly renderable - TODO: explain.
    str       : String,

    /// Number of _visible_ (e.g. excludes StyleRef encodings) characters in the
    /// line.
    len_chars : i32,

    /// Char indexes (not counting StyleRefs) of split positions of the string -
    /// when the line doesn't fit into the screen we split it into multiple
    /// lines using these.
    splits    : Vec<i32>,
}

impl Line {
    pub fn new() -> Line {
        Line {
            str: String::new(),
            len_chars: 0,
            splits: Vec::new(),
        }
    }

    pub fn add_text(&mut self, str : &str, style : StyleRef) {
        self.str.reserve(str.len() + 1); // +1 for the StyleRef
        // first 32 ascii chars are never rendered, we use those to encode Style
        unsafe { self.str.as_mut_vec() }.push(style);

        let mut chars_added = 0;
        for (char_idx, char) in str.chars().enumerate() {
            self.str.push(char);
            if char.is_whitespace() {
                self.splits.push(self.len_chars + char_idx as i32);
            }
            chars_added += 1;
        }

        self.len_chars += chars_added;
        // no need to reset the style as the next string will set it again
    }

    pub fn add_char(&mut self, char : char) {
        if char.is_whitespace() {
            self.splits.push(self.len_chars);
        }
        self.str.push(char);
        self.len_chars += 1;
    }

    pub fn len_chars(&self) -> i32 {
        self.len_chars
    }

    /// How many lines does this take when rendered? O(n) where n = number of
    /// split positions in the lines (i.e.  whitespaces).
    pub fn rendered_height(&self, width : i32) -> i32 {
        let mut lines : i32 = 1;
        let mut line_start : i32 = 0;

        for split_idx in 0 .. self.splits.len() {
            let char_idx = *unsafe { self.splits.get_unchecked(split_idx) };
            // writeln!(io::stderr(), "rendered_height: char_idx: {}", char_idx);
            let col = char_idx - line_start;

            // How many more chars can we render in this line?
            let slots_in_line : i32 = width - (col + 1);

            // How many chars do we need to render if until the next split
            // point?
            let chars_until_next_split : i32 =
                // -1 becuase we don't need to render the space or EOL.
                *self.splits.get(split_idx + 1).unwrap_or(&self.len_chars) - 1 - char_idx;

            // writeln!(io::stderr(),
            //          "rendered_height: slots_in_line: {}, chars_until_next_split: {}",
            //          slots_in_line, chars_until_next_split);

            if (chars_until_next_split as i32) > slots_in_line {
                // writeln!(io::stderr(), "splitting at {}", char_idx);
                lines += 1;
                line_start = char_idx + 1;
            }
        }

        lines
    }

    pub fn draw(&self, _ : &RustBox, pos_x : i32, pos_y : i32, width : i32) {
        let mut col = pos_x;
        let mut row = pos_y;

        let mut next_split_idx : usize = 0;

        let mut char_idx : i32 = 0;

        let mut fg  = style::get_style(style::USER_MSG).fg;
        let mut bg  = style::get_style(style::USER_MSG).bg;

        for char in self.str.chars() {
            if (char as i32) < 32 {
                let style = style::get_style(char as u8);
                fg = style.fg;
                bg = style.bg;
                continue;
            }

            else if char.is_whitespace() {
                // We may want to move to the next line
                next_split_idx += 1;
                let next_split = self.splits.get(next_split_idx).unwrap_or(&self.len_chars);

                // How many more chars can we render in this line?
                let slots_in_line = width - (col - pos_x);

                // How many chars do we need to render if until the next
                // split point?
                assert!(*next_split > char_idx);
                let chars_until_next_split : i32 = *next_split - char_idx;

                // writeln!(io::stderr(), "chars_until_next_split: {}, slots_in_line: {}",
                //          chars_until_next_split, slots_in_line);

                if (chars_until_next_split as i32) <= slots_in_line {
                    // keep rendering chars
                    unsafe { tb_change_cell(col, row, char as u32, fg, bg); }
                    col += 1;
                } else {
                    // need to split here. ignore whitespace char.
                    row += 1;
                    col = pos_x;
                }

                char_idx += 1;
            }

            else {
                // Not possible to split. Need to make sure we don't render out
                // of bounds.
                if col - pos_x < width {
                    unsafe { tb_change_cell(col, row, char as u32, fg, bg); }
                    col += 1;
                }

                char_idx += 1;
            }
        }
    }
}

////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {

extern crate test;

use self::test::Bencher;
use std::fs::File;
use std::io::Read;
use super::*;

use tui::style;

#[test]
fn height_test_1() {
    let mut line = Line::new();
    line.add_text("a b c d e", style::USER_MSG);
    assert_eq!(line.rendered_height(1), 5);
    assert_eq!(line.rendered_height(2), 5);
    assert_eq!(line.rendered_height(3), 3);
    assert_eq!(line.rendered_height(4), 3);
    assert_eq!(line.rendered_height(5), 2);
    assert_eq!(line.rendered_height(6), 2);
    assert_eq!(line.rendered_height(7), 2);
    assert_eq!(line.rendered_height(8), 2);
    assert_eq!(line.rendered_height(9), 1);
}

#[test]
fn height_test_2() {
    let mut line = Line::new();
    line.add_text("ab c d e", style::USER_MSG);
    assert_eq!(line.rendered_height(1), 4);
    assert_eq!(line.rendered_height(2), 4);
    assert_eq!(line.rendered_height(3), 3);
    assert_eq!(line.rendered_height(4), 2);
    assert_eq!(line.rendered_height(5), 2);
    assert_eq!(line.rendered_height(6), 2);
    assert_eq!(line.rendered_height(7), 2);
    assert_eq!(line.rendered_height(8), 1);
}

#[test]
fn height_test_3() {
    let mut line = Line::new();
    line.add_text("ab cd e", style::USER_MSG);
    assert_eq!(line.rendered_height(1), 3);
    assert_eq!(line.rendered_height(2), 3);
    assert_eq!(line.rendered_height(3), 3);
    assert_eq!(line.rendered_height(4), 2);
    assert_eq!(line.rendered_height(5), 2);
    assert_eq!(line.rendered_height(6), 2);
    assert_eq!(line.rendered_height(7), 1);
}

#[test]
fn height_test_4() {
    let mut line = Line::new();
    line.add_text("ab cde", style::USER_MSG);
    assert_eq!(line.rendered_height(1), 2);
    assert_eq!(line.rendered_height(2), 2);
    assert_eq!(line.rendered_height(3), 2);
    assert_eq!(line.rendered_height(4), 2);
    assert_eq!(line.rendered_height(5), 2);
    assert_eq!(line.rendered_height(6), 1);
}

#[test]
fn height_test_5() {
    let mut line = Line::new();
    line.add_text("abcde", style::USER_MSG);
    for i in 0 .. 6 {
        assert_eq!(line.rendered_height(i), 1);
    }
}

#[bench]
fn bench_rendered_height(b : &mut Bencher) {

    // 1160 words, 2,237 ns/iter (+/- 150)

    let mut text = String::new();
    {
        let mut file = File::open("test/lipsum.txt").unwrap();
        file.read_to_string(&mut text);
    }

    let mut line = Line::new();
    line.add_text(&text, style::USER_MSG);
    b.iter(|| {
        line.rendered_height(1)
    });
}

} // mod tests
