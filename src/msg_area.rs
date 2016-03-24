use std::borrow::Borrow;
use std::cmp::min;
use std::io::Write;
use std::io;

use rustbox::{RustBox, Style, Color};

static LINEBREAK : char = '↳';

pub struct MsgArea {
    msgs   : Vec<Line>,

    width  : i32,
    height : i32,

    /// Vertical scroll
    scroll : i32,

    // TODO: logging
}

struct Line {
    /// A line. INVARIANT: Not longer than the width of the widget.
    msg : String,

    /// Is this continuation of previous line?
    continuation : bool,
}

impl MsgArea {
    pub fn new(width : i32, height : i32) -> MsgArea {
        MsgArea {
            msgs: Vec::new(),
            width: width,
            height: height,
            scroll: 0,
        }
    }

    pub fn add_msg(&mut self, msg : &Vec<char>) {
        // Decide whether to scroll
        let need_to_scroll = self.scroll + self.height == self.msgs.len() as i32;

        // Add the line(s)
        let mut msg_slice : &[char] = msg.borrow();
        let mut lines : Vec<Line> = Vec::with_capacity(1);
        while msg_slice.len() != 0 {
            let first_line = lines.len() == 0;
            let split = if first_line { self.width } else { self.width - 1 };

            let (line, rest) = msg_slice.split_at(min(msg_slice.len(), split as usize));
            msg_slice = rest;

            lines.push(Line {
                msg: line.iter().cloned().collect(),
                continuation: !first_line,
            });
        }

        if need_to_scroll {
            self.scroll += lines.len() as i32;
        }

        self.msgs.append(&mut lines);
    }

    pub fn add_msg_str(&mut self, msg_str : &str) {
        writeln!(&mut io::stderr(), "adding msg: {:?}", msg_str);
        self.add_msg(&msg_str.chars().collect());
    }

    pub fn draw(&self, rustbox : &RustBox, pos_x : i32, pos_y : i32) {
        let mut row = self.height - 1;
        let mut line_idx = min(self.scroll + self.height, self.msgs.len() as i32 - 1);
        while line_idx >= 0 {
            let line = &self.msgs[line_idx as usize];

            if line.continuation {
                rustbox.print_char(pos_x as usize, (pos_y + row) as usize,
                                   Style::empty(), Color::Blue, Color::Default,
                                   LINEBREAK);
            }

            let pos_x = if line.continuation { pos_x + 1 } else { pos_x };

            rustbox.print(pos_x as usize, (pos_y + row) as usize,
                          Style::empty(), Color::Blue, Color::Default,
                          &line.msg);

            row -= 1;
            line_idx -= 1;
        }
    }
}
