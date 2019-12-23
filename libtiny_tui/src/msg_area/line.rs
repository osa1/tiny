use std::mem;
use termbox_simple::{self, Termbox};

use crate::{config, config::Colors, utils::translate_irc_control_chars};

/// A single line added to the widget. May be rendered as multiple lines on the
/// screen.
#[derive(Debug)]
pub(crate) struct Line {
    /// Line segments.
    segs: Vec<Seg>,

    /// The segment we're currently extending.
    current_seg: Seg,

    /// Number of characters in the line (includes all segments).
    len_chars: i32,

    /// Char indices (NOT byte indices!) of split positions of the line - when
    /// the line doesn't fit into the screen we split it into multiple lines
    /// using these positions as split points.
    ///
    /// It's important that these are really indices ignoring invisible chars,
    /// as we use difference between two indices in this vector as length of
    /// substrings.
    splits: Vec<i32>,
}

#[derive(Debug)]
struct Seg {
    text: String,
    style: SegStyle,
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub(crate) enum SegStyle {
    /// A specific style. Useful when rendering IRC colors (which should look
    /// the same across color schemes).
    Fixed(config::Style),

    /// An index to nick colors. Note that the index should be larger than size
    /// of the color list, so make sure to use mod.
    Index(usize),

    /// A style from the current color scheme.
    SchemeStyle(SchemeStyle),
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub(crate) enum SchemeStyle {
    UserMsg,
    ErrMsg,
    Topic,
    Join,
    Part,
    Nick,
    Faded,
    Highlight,
    Timestamp,
}

impl Seg {
    fn style(&self, colors: &Colors) -> config::Style {
        match self.style {
            SegStyle::Fixed(style) => style,
            SegStyle::Index(idx) => config::Style {
                fg: u16::from(colors.nick[idx % colors.nick.len()]),
                bg: colors.user_msg.bg,
            },
            SegStyle::SchemeStyle(sty) => {
                use self::SchemeStyle::*;
                match sty {
                    UserMsg => colors.user_msg,
                    ErrMsg => colors.err_msg,
                    Topic => colors.topic,
                    Join => colors.join,
                    Part => colors.part,
                    Nick => colors.nick_change,
                    Faded => colors.faded,
                    Highlight => colors.highlight,
                    Timestamp => colors.timestamp,
                }
            }
        }
    }
}

// TODO get rid of this
const TERMBOX_COLOR_PREFIX: char = '\x00';

impl Line {
    pub(crate) fn new() -> Line {
        Line {
            segs: vec![],
            current_seg: Seg {
                text: String::new(),
                style: SegStyle::SchemeStyle(SchemeStyle::UserMsg),
            },
            len_chars: 0,
            splits: Vec::new(),
        }
    }

    pub(crate) fn set_style(&mut self, style: SegStyle) {
        // Just update the last segment if it's empty
        if self.current_seg.text.is_empty() {
            self.current_seg.style = style;
        } else if self.current_seg.style != style {
            self.flush_current_seg(style);
        }
    }

    pub(crate) fn add_timestamp(&mut self, ts_str: &str) {
        debug_assert!(self.segs.is_empty());
        self.set_style(SegStyle::SchemeStyle(SchemeStyle::Timestamp));
        self.add_text(ts_str);
        self.add_char(' ');
    }

    pub(crate) fn add_text(&mut self, str: &str) {
        fn push_color(ret: &mut String, irc_fg: u8, irc_bg: Option<u8>) {
            ret.push(TERMBOX_COLOR_PREFIX);
            ret.push(0 as char); // style
            ret.push(irc_color_to_termbox(irc_fg) as char);
            ret.push(
                irc_bg
                    .map(irc_color_to_termbox)
                    .unwrap_or(termbox_simple::TB_DEFAULT as u8) as char,
            );
        }
        let str = translate_irc_control_chars(str, push_color);
        self.current_seg.text.reserve(str.len());

        let mut iter = str.chars();
        while let Some(char) = iter.next() {
            if char == TERMBOX_COLOR_PREFIX {
                let st = iter.next().unwrap() as u8;
                let fg = iter.next().unwrap() as u8;
                let bg = iter.next().unwrap() as u8;
                let fg = (u16::from(st) << 8) | u16::from(fg);
                let bg = u16::from(bg);
                let style = config::Style { fg, bg };
                self.set_style(SegStyle::Fixed(style));
            } else if char > '\x08' {
                self.current_seg.text.push(char);
                if char.is_whitespace() {
                    self.splits.push(self.len_chars);
                }
                self.len_chars += 1;
            }
        }
    }

    pub(crate) fn add_char(&mut self, char: char) {
        assert_ne!(char, TERMBOX_COLOR_PREFIX);
        if char.is_whitespace() {
            self.splits.push(self.len_chars);
        }
        self.current_seg.text.push(char);
        self.len_chars += 1;
    }

    /// How many lines does this take when rendered? O(n) where n = number of
    /// split positions in the line (i.e. whitespaces).
    pub(crate) fn rendered_height(&self, width: i32) -> i32 {
        let mut lines: i32 = 1;
        let mut line_start: i32 = 0;

        for split_idx in 0..self.splits.len() {
            let char_idx = self.splits[split_idx];
            // debug!("rendered_height: char_idx: {}", char_idx);
            let col = char_idx - line_start;

            // How many more chars can we render in this line?
            let slots_in_line: i32 = width - (col + 1);

            // How many chars do we need to render until the next split point?
            let chars_until_next_split: i32 =
                // -1 because we don't need to render the last space.
                *self.splits.get(split_idx + 1).unwrap_or(&self.len_chars) - 1 - char_idx;

            // debug!("rendered_height: slots_in_line: {}, chars_until_next_split: {}",
            //        slots_in_line, chars_until_next_split);

            if (chars_until_next_split as i32) > slots_in_line {
                // debug!("splitting at {}", char_idx);
                lines += 1;
                line_start = char_idx + 1;
            }
        }

        lines
    }

    pub(crate) fn draw(
        &self,
        tb: &mut Termbox,
        colors: &Colors,
        pos_x: i32,
        pos_y: i32,
        first_line: i32,
        height: i32,
        width: i32,
    ) {
        let mut col = pos_x;
        let mut line = 0;

        let mut next_split_idx: usize = 0;

        let mut char_idx: i32 = 0;

        let last_seg: [&Seg; 1] = [&self.current_seg];
        for seg in self.segs.iter().chain(last_seg.iter().copied()) {
            let sty = seg.style(colors);

            for char in seg.text.chars() {
                if char.is_whitespace() {
                    // We may want to move to the next line
                    next_split_idx += 1;
                    let next_split = self.splits.get(next_split_idx).unwrap_or(&self.len_chars);

                    // How many more chars can we render in this line?
                    let slots_in_line = width - (col - pos_x);

                    // How many chars do we need to render if until the next
                    // split point?
                    assert!(*next_split > char_idx);
                    let chars_until_next_split: i32 = *next_split - char_idx;

                    // debug!("chars_until_next_split: {},
                    //        slots_in_line: {}",
                    //        chars_until_next_split,
                    //        slots_in_line);

                    if (chars_until_next_split as i32) <= slots_in_line {
                        // keep rendering chars
                        if line >= first_line {
                            tb.change_cell(col, pos_y + line, char, sty.fg, sty.bg);
                        }
                        col += 1;
                    } else {
                        // need to split here. ignore whitespace char.
                        line += 1;
                        if line >= height {
                            break;
                        }
                        col = pos_x;
                    }
                } else {
                    debug_assert!(!char.is_ascii_control());

                    // Not possible to split. Need to make sure we don't render out
                    // of bounds.
                    if col - pos_x < width {
                        if line >= first_line {
                            tb.change_cell(col, pos_y + line, char, sty.fg, sty.bg);
                        }
                        col += 1;
                    }
                }

                char_idx += 1;
            }
        }
    }

    fn flush_current_seg(&mut self, style: SegStyle) {
        let seg = mem::replace(
            &mut self.current_seg,
            Seg {
                text: String::new(),
                style,
            },
        );
        self.segs.push(seg);
    }
}

////////////////////////////////////////////////////////////////////////////////

// IRC colors: http://en.wikichip.org/wiki/irc/colors
// Termbox colors: http://www.calmar.ws/vim/256-xterm-24bit-rgb-color-chart.html
//                 (alternatively just run `cargo run --example colors`)
fn irc_color_to_termbox(irc_color: u8) -> u8 {
    match irc_color {
        0 => 15,  // white
        1 => 0,   // black
        2 => 17,  // navy
        3 => 2,   // green
        4 => 9,   // red
        5 => 88,  // maroon
        6 => 5,   // purple
        7 => 130, // olive
        8 => 11,  // yellow
        9 => 10,  // light green
        10 => 6,  // teal
        11 => 14, // cyan
        12 => 12, // awful blue
        13 => 13, // magenta
        14 => 8,  // gray
        15 => 7,  // light gray
        _ => termbox_simple::TB_DEFAULT as u8,
    }
}

////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {

    extern crate test;

    use super::*;
    use std::{fs::File, io::Read};
    use test::Bencher;

    #[test]
    fn height_test_1() {
        let mut line = Line::new();
        line.add_text("a b c d e");
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
        line.add_text("ab c d e");
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
        line.add_text("ab cd e");
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
        line.add_text("ab cde");
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
        line.add_text("abcde");
        for i in 0..6 {
            assert_eq!(line.rendered_height(i), 1);
        }
    }

    #[test]
    fn height_test_6() {
        let text: String = {
            let mut text = String::new();
            let mut single_line = String::new();
            let mut file = File::open("test/lipsum.txt").unwrap();
            file.read_to_string(&mut text).unwrap();
            let lines: Vec<&str> = text.lines().collect();
            assert_eq!(lines.len(), 102); // make sure we did it right
            for (line_idx, line) in lines.iter().enumerate() {
                single_line.push_str(line);
                if line_idx != lines.len() - 1 {
                    single_line.push(' ');
                }
            }
            single_line
        };

        let mut line = Line::new();
        line.add_text(&text);
        // lipsum.txt has 1160 words in it. each line should contain at most one
        // word so we should have 1160 lines.
        assert_eq!(line.rendered_height(1), 1160);
    }

    #[bench]
    fn bench_rendered_height(b: &mut Bencher) {
        // 1160 words, 2,237 ns/iter (+/- 150)

        let mut text = String::new();
        {
            let mut file = File::open("test/lipsum.txt").unwrap();
            file.read_to_string(&mut text).unwrap();
        }

        let mut line = Line::new();
        line.add_text(&text);
        b.iter(|| line.rendered_height(1));
    }
} // mod tests
