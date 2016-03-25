use std::borrow::Borrow;
use std::cmp::min;
use std::io::Write;
use std::io;

use rustbox::{RustBox, Style, Color};

static LINEBREAK : char = '\\';

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

    style : Style,
    fg : Color,
    bg : Color,
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

    #[inline]
    pub fn add_msg_str(&mut self, msg_str : &str) {
        writeln!(&mut io::stderr(), "adding msg: {:?}", msg_str).unwrap();
        self.add_msg_str_(msg_str, Style::empty(), Color::Default, Color::Default);
    }

    #[inline]
    pub fn add_server_msg(&mut self, msg_str : &str) {
        writeln!(&mut io::stderr(), "adding msg: {:?}", msg_str).unwrap();
        self.add_msg_str_(msg_str, Style::empty(), Color::Yellow, Color::Default);
    }

    #[inline]
    pub fn add_err_msg_str(&mut self, msg_str : &str) {
        writeln!(&mut io::stderr(), "adding msg: {:?}", msg_str).unwrap();
        self.add_msg_str_(msg_str, Style::empty(), Color::White, Color::Red);
    }

    #[inline]
    pub fn add_msg(&mut self, msg : &Vec<char>) {
        self.add_msg_(msg, Style::empty(), Color::Default, Color::Default);
    }

    #[inline]
    pub fn add_err_msg(&mut self, msg : &Vec<char>) {
        self.add_msg_(msg, Style::empty(), Color::White, Color::Red);
    }

    pub fn draw(&self, rustbox : &RustBox, pos_x : i32, pos_y : i32) {
        let mut row = self.height - 1;
        let mut line_idx = min(self.scroll + self.height, self.msgs.len() as i32) - 1;
        while line_idx >= 0 && row >= 0 {
            let line = &self.msgs[line_idx as usize];

            if line.continuation {
                rustbox.print_char(pos_x as usize, (pos_y + row) as usize,
                                   line.style, line.fg, line.bg, LINEBREAK);
            }

            let pos_x = if line.continuation { pos_x + 1 } else { pos_x };

            rustbox.print(pos_x as usize, (pos_y + row) as usize,
                          line.style, line.fg, line.bg, &line.msg);

            row -= 1;
            line_idx -= 1;
        }
    }

    ////////////////////////////////////////////////////////////////////////////

    fn add_msg_str_(&mut self, msg : &str, style : Style, fg : Color, bg : Color) {
        // Take the fast path when number of bytes (which gives the max number
        // of characters possibly be in the string) is smaller than the width.
        if msg.len() <= self.width as usize {
            if self.need_to_scroll() {
                self.scroll += 1;
            }

            self.msgs.push(Line {
                msg: msg.to_owned(),
                continuation: false,
                style: style,
                fg: fg,
                bg: bg,
            });
        } else {
            // Need to split the lines, taking the slow path that uses an
            // intermediate vector.
            self.add_msg_(&msg.chars().collect(), style, fg, bg);
        }
    }

    fn add_msg_(&mut self, msg : &Vec<char>, style : Style, fg : Color, bg : Color) {
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
                style: style,
                fg: fg,
                bg: bg,
            });
        }

        // We need to check the scroll before adding the messages, as the check
        // uses current scroll and number of messages.
        if self.need_to_scroll() {
            self.scroll += lines.len() as i32;
        }

        self.msgs.append(&mut lines);
    }

    ////////////////////////////////////////////////////////////////////////////

    pub fn scroll_up(&mut self) {
        if self.scroll > 0 {
            self.scroll -= 1;
        }
    }

    pub fn scroll_down(&mut self) {
        if (self.msgs.len() as i32) > self.scroll + self.height {
            self.scroll += 1;
        }
    }

    pub fn page_up(&mut self) {

    }

    pub fn page_down(&mut self) {

    }

    /// Do we need to scroll when adding a new message?
    #[inline]
    fn need_to_scroll(&self) -> bool {
        self.scroll + self.height == self.msgs.len() as i32
    }
}
