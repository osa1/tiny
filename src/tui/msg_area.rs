use std::borrow::Borrow;
use std::cmp::{max, min};
use std::io::Write;
use std::io;
use std::mem;

use rustbox::{RustBox, Style, Color, Key};
use tui::widget::{Widget, WidgetRet};

static LINEBREAK : char = '\\';

pub struct MsgArea {
    lines  : Vec<Line>,

    width  : i32,
    height : i32,

    /// Total number of lines added to the widget. Note that this is not the
    /// same thing as lines.len(): This simply returns number of lines added to
    /// the widget, rather than number of lines shown by the widget.
    total_lines : i32,

    /// Vertical scroll
    scroll : i32,

    // TODO: logging
}

#[derive(Debug)]
struct Line {
    /// A line. INVARIANT: Not longer than the width of the widget, and shown as
    /// single line in the TUI.
    msg : String,

    /// Is this continuation of previous line?
    continuation : bool,

    /// Index of the line in the buffer. Continuations have same indexes as the
    /// original message.
    line_idx : i32,

    style : Style,
    fg : Color,
    bg : Color,
}

impl MsgArea {
    pub fn new(width : i32, height : i32) -> MsgArea {
        MsgArea {
            lines: Vec::new(),
            width: width,
            height: height,
            total_lines: 0,
            scroll: 0,
        }
    }

    ////////////////////////////////////////////////////////////////////////////
    // Resizing

    fn resize_(&mut self, width : i32, height : i32) {
        // either scroll to bottom ...
        let scroll_to_bottom       = self.need_to_scroll();
        // ... or make sure the first visible line is still visible
        let first_visible_line_idx =
            if self.lines.is_empty() { 0 } else { self.lines[self.scroll as usize].line_idx };

        // Update the size
        self.width = width;
        self.height = height;

        // Combine/re-split lines
        self.resplit_lines();

        // Update the scroll
        if (self.lines.len() as i32) < self.height {
            self.scroll = 0;
        } else if scroll_to_bottom {
            self.scroll = (self.lines.len() as i32) - self.height;
        } else {
            match self.find_line_idx(first_visible_line_idx) {
                None => {/* TODO: Log this somewhere, this is a bug */},
                Some(idx) => { self.scroll = idx as i32; }
            }
        }
    }

    /// Combine continuations with original lines, add lines again in the
    /// original order. Should be called after updating width to have any
    /// effect.
    fn resplit_lines(&mut self) {
        // We could probably modify in-place using two indexes, but whatever.

        let old_lines : Vec<Line> = {
            let total_lines = self.lines.len();
            mem::replace(&mut self.lines, Vec::with_capacity(total_lines))
        };

        writeln!(&mut io::stderr(), "old lines: {:?}", old_lines).unwrap();

        let mut line_idx = 0;
        while line_idx < old_lines.len() {
            // How many bytes combined string needs?
            let mut total_len = old_lines[line_idx].msg.len();
            {
                let mut cont_idx  = line_idx + 1;
                while cont_idx < old_lines.len() && old_lines[cont_idx].continuation {
                    total_len += old_lines[cont_idx].msg.len();
                    cont_idx  += 1;
                }
            }

            // Combine all lines
            let mut new_line = String::with_capacity(total_len);
            new_line.push_str(old_lines[line_idx].msg.borrow());
            let mut cont_idx = line_idx + 1;
            while cont_idx < old_lines.len() && old_lines[cont_idx].continuation {
                new_line.push_str(old_lines[cont_idx].msg.borrow());
                cont_idx += 1;
            }

            // Finally add the combined line to the fresh buffer
            self.add_msg_(new_line.chars().collect::<Vec<char>>().borrow(),
                          old_lines[line_idx].style,
                          old_lines[line_idx].fg,
                          old_lines[line_idx].bg);


            line_idx = cont_idx;
        }

        writeln!(&mut io::stderr(), "new lines: {:?}", self.lines).unwrap();
    }

    /// Find index of given line in 'self.lines'.
    fn find_line_idx(&self, line_idx : i32) -> Option<usize> {
        for (line_idx, line) in self.lines.iter().enumerate() {
            if line.line_idx == line_idx as i32 {
                return Some(line_idx)
            }
        }
        None
    }

    ////////////////////////////////////////////////////////////////////////////

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
    pub fn add_msg(&mut self, msg : &[char]) {
        self.add_msg_(msg, Style::empty(), Color::Default, Color::Default);
    }

    #[inline]
    pub fn add_err_msg(&mut self, msg : &[char]) {
        self.add_msg_(msg, Style::empty(), Color::White, Color::Red);
    }

    fn draw_(&self, rustbox : &RustBox, pos_x : i32, pos_y : i32) {
        let mut row = self.height - 1;
        let mut line_idx = min(self.scroll + self.height, self.lines.len() as i32) - 1;
        while line_idx >= 0 && row >= 0 {
            let line = &self.lines[line_idx as usize];

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
    // Adding new messages

    fn add_msg_str_(&mut self, msg : &str, style : Style, fg : Color, bg : Color) {
        // Take the fast path when number of bytes (which gives the max number
        // of characters possibly be in the string) is smaller than the width.
        if msg.len() <= self.width as usize {
            if self.need_to_scroll() {
                self.scroll += 1;
            }

            self.lines.push(Line {
                msg: msg.to_owned(),
                continuation: false,
                line_idx: self.total_lines,
                style: style,
                fg: fg,
                bg: bg,
            });

            self.total_lines += 1;
        } else {
            // Need to split the lines, taking the slow path that uses an
            // intermediate vector.
            self.add_msg_(&msg.chars().collect::<Vec<char>>().borrow(), style, fg, bg);
        }
    }

    fn add_msg_(&mut self, mut msg : &[char], style : Style, fg : Color, bg : Color) {
        let mut lines : Vec<Line> = Vec::with_capacity(1);
        while msg.len() != 0 {
            let first_line = lines.len() == 0;
            let split = if first_line { self.width } else { self.width - 1 };

            let (line, rest) = msg.split_at(min(msg.len(), split as usize));
            msg = rest;

            lines.push(Line {
                msg: line.iter().cloned().collect(),
                continuation: !first_line,
                line_idx: self.total_lines,
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

        self.lines.append(&mut lines);

        self.total_lines += 1;
    }

    ////////////////////////////////////////////////////////////////////////////

    pub fn scroll_up(&mut self) {
        if self.scroll > 0 {
            self.scroll -= 1;
        }
    }

    pub fn scroll_down(&mut self) {
        if (self.lines.len() as i32) > self.scroll + self.height {
            self.scroll += 1;
        }
    }

    pub fn page_up(&mut self) {
        self.scroll = max(0, self.scroll - 10);
    }

    pub fn page_down(&mut self) {
        self.scroll = min(self.scroll + 10, (self.lines.len() as i32) - self.height);
    }

    /// Do we need to scroll when adding a new message?
    #[inline]
    fn need_to_scroll(&self) -> bool {
        self.scroll + self.height == self.lines.len() as i32
    }
}

impl Widget for MsgArea {
    fn draw(&self, rustbox : &RustBox, pos_x : i32, pos_y : i32) {
        self.draw_(rustbox, pos_x, pos_y)
    }

    fn keypressed(&mut self, key : Key) -> WidgetRet {
        WidgetRet::KeyIgnored
    }

    fn resize(&mut self, width : i32, height : i32) {
        self.resize_(width, height)
    }
}
