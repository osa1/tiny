pub mod line;

use std::cmp::{max, min};
use std::mem;

use rustbox::{RustBox, Key};
use rustbox;
use time;

use tui::style::Style;
use tui::style;
use tui::widget::{Widget, WidgetRet};
use self::line::Line;

use std::io::Write;
use std::io;

////////////////////////////////////////////////////////////////////////////////

pub struct MsgArea {
    lines       : Vec<Line>,

    // Rendering related
    width       : i32,
    height      : i32,

    /// Vertical scroll: An offset from the last visible line. (not a `Line` in
    /// `self.lines`, a rendered line)
    scroll      : i32,

    // TODO: logging
}

impl MsgArea {
    pub fn new(width : i32, height : i32) -> MsgArea {
        MsgArea {
            lines: Vec::new(),
            width: width,
            height: height,
            scroll: 0,
        }
    }

    pub fn add_line(&mut self, lines : Vec<(String, &'static Style)>) {
        // let mut line = Line::new();
        // for (line_str, line_style) in lines.into_iter() {
        //     line.add_segment(line_str, line_style);
        // }

        // if self.scroll != 0 {
        //     // need to update the scroll to render the same place.
        //     self.scroll += line.vertical_space(self.width);
        // }

        // self.lines.push(line);
    }

    pub fn resize(&mut self, width : i32, height : i32) {
        if self.scroll == 0 {
            self.width = width;
            self.height = height;
            return;
        }

        let line_at_middle = {
            let middle_row = (self.scroll + self.height) / 2;
            self.line_at_row(middle_row).unwrap_or(0)
        };

        self.width = width;
        self.height = height;

        self.scroll = self.row_at_line(line_at_middle)
                        .map(|s| max(0, s - height / 2))
                        .unwrap_or(0);
    }

    pub fn draw(&self, rustbox : &RustBox, pos_x : i32, pos_y : i32) {
        // Draw lines in reverse order
        let mut line_idx = self.line_at_row(self.scroll);

    }

    /// NOTE: `row` starts from the bottom, e.g. last line that we render has
    /// row 0. also, it's absolute (e.g. it doesn't depend on `scroll`)
    fn line_at_row(&self, row : i32) -> Option<usize> {
        panic!()
        // let mut current_row = 0;
        // for (line_idx, line) in self.lines.iter().enumerate().rev() {
        //     current_row += line.vertical_space(self.width);
        //     if current_row > row {
        //         return Some(line_idx);
        //     }
        // }
        // None
    }

    fn row_at_line(&self, line_idx : usize) -> Option<i32> {
        panic!()
        // let mut rows : i32 = 0;
        // for (line_idx_, line) in self.lines.iter().enumerate().rev() {
        //     if line_idx == line_idx_ {
        //         rows += line.vertical_space(self.width);
        //     } else {
        //         return Some(rows);
        //     }
        // }
        // None
    }
}

/*
////////////////////////////////////////////////////////////////////////////////
// Resizing

impl MsgArea {
    fn resize_(&mut self, width : i32, height : i32) {
        // either scroll to bottom ...
        let scroll_to_bottom = self.need_to_scroll();
        // ... or make sure the first visible line is still visible
        let first_visible_line_offset =
            if self.lines.is_empty() { 0 } else { self.lines[self.scroll as usize].start_idx() };

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
            match self.find_line_idx(first_visible_line_offset) {
                None => {
                    panic!("find_line_idx can't find index of {}", first_visible_line_offset);
                },
                Some(idx) => {
                    self.scroll = idx as i32;
                }
            }
        }
    }

    /// Combine continuations with original lines, add lines again in the
    /// original order. Should be called after updating width to have any
    /// effect.
    fn resplit_lines(&mut self) {
        // Breaks invariants.
        self.combine_continuations();
        // Make sure the invariants hold again.
        self.split_lines();
    }

    /// WARNING: Invalidates some of the invariants. To be used during a resize.
    /// Call `split_lines()` after this.
    fn combine_continuations(&mut self) {
        let mut old_lines : Vec<Line> = {
            let total_lines = self.lines.len();
            mem::replace(&mut self.lines, Vec::with_capacity(total_lines))
        };

        let old_lines_n = old_lines.len();

        let mut line_idx = 0;
        while line_idx < old_lines_n {
            let mut line = mem::replace(&mut old_lines[line_idx], Line::new());
            debug_assert!(!line.continuation);

            let mut cont_idx = line_idx + 1;
            while cont_idx < old_lines_n && old_lines[cont_idx].continuation {
                line.extend(&old_lines[cont_idx]);
                cont_idx += 1;
            }

            self.lines.push(line);
            line_idx = cont_idx;
        }
    }

    fn split_lines(&mut self) {
        if cfg!(debug_assertions) {
            for line in self.lines.iter() {
                assert!(!line.continuation);
            }
        }

        let mut combined_lines_n = self.lines.len();
        let mut combined_lines   =
            mem::replace(&mut self.lines, Vec::with_capacity(combined_lines_n));

        for mut line in combined_lines {
            if line.len() as i32 > self.width {
                while let Some(new_line) = line.maybe_split((self.width as usize) - 1, &self.buf) {
                    self.lines.push(line);
                    line = new_line;
                }
                self.lines.push(line);
            }
        }
    }

    /// Find index of given line in 'self.lines'.
    fn find_line_idx(&self, offset : usize) -> Option<usize> {
        for (line_idx_in_vec, line) in self.lines.iter().enumerate() {
            if offset >= line.start_idx() && offset < line.end_idx() {
                return Some(line_idx_in_vec)
            }
        }
        None
    }
}

////////////////////////////////////////////////////////////////////////////////
// Adding new messages

impl MsgArea {
    fn add_segment(&mut self, msg : &str, style : &'static Style, mut line : Line) -> Line {
        let start_idx = self.buf.len();
        self.buf.extend(msg.chars());
        let end_idx = self.buf.len();
        let mut seg = LineSegment {
            start_idx: start_idx,
            end_idx: end_idx,
            style: style,
        };

        writeln!(io::stderr(), "adding segment: {:?}", seg);
        line.segs.push(seg);
        writeln!(io::stderr(), "line.len(): {:?}, self.width: {}", line.len(), self.width);

        while let Some(new_line) = line.maybe_split(self.width as usize,
                                                    &self.buf[ start_idx .. end_idx ]) {
            writeln!(io::stderr(), "adding line: {:?}", line);
            self.lines.push(line);
            line = new_line;
        }

        line
    }

    pub fn add_client_err_msg(&mut self, msg : &str) {

    }

    pub fn add_client_msg(&mut self, msg : &str) {
        let mut line = Line::new();
        line = self.add_segment(msg, &style::USER_MSG, line);
        self.lines.push(line);
        self.total_lines += 1;
    }

    pub fn add_privmsg(&mut self, sender : &str, msg : &str, tm : &time::Tm) {

    }

    pub fn add_msg(&mut self, msg : &str, tm : &time::Tm) {
        let mut line = Line::new();
        line = self.add_segment(&format!("[{}]", tm.strftime("%H:%M").unwrap()),
                                &style::ERR_MSG, line);
        line = self.add_segment(" ", &style::USER_MSG, line);
        line = self.add_segment(msg, &style::USER_MSG, line);
        self.lines.push(line);
        self.total_lines += 1;
    }

    pub fn add_err_msg(&mut self, msg : &str, tm : &time::Tm) {

    }
}

////////////////////////////////////////////////////////////////////////////////
// Scrolling

impl MsgArea {
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

}

////////////////////////////////////////////////////////////////////////////////

impl Widget for MsgArea {
    fn draw(&self, rustbox : &RustBox, pos_x : i32, pos_y : i32) {
        let mut row = self.height - 1;
        let mut line_idx = min(self.scroll + self.height, self.lines.len() as i32) - 1;
        while line_idx >= 0 && row >= 0 {
            let line = &self.lines[line_idx as usize];

            // if line.continuation {
            //     rustbox.print_char(pos_x as usize, (pos_y + row) as usize,
            //                        line.style.style, line.style.fg, line.style.bg, LINEBREAK);
            // }
            //
            // let pos_x = if line.continuation { pos_x + 1 } else { pos_x };

            let mut col = 0;
            for seg in line.segs.iter() {
                for i in seg.start_idx .. seg.end_idx {
                    unsafe {
                        rustbox.change_cell((pos_x + col) as usize, (pos_y + row) as usize,
                                            *self.buf.get_unchecked(i) as u32,
                                            rustbox::Style::from_color(seg.style.fg).bits(),
                                            rustbox::Style::from_color(seg.style.bg).bits());
                    }
                    col += 1;
                }
            }

            row -= 1;
            line_idx -= 1;
        }
    }

    fn keypressed(&mut self, _ : Key) -> WidgetRet {
        WidgetRet::KeyIgnored
    }

    fn resize(&mut self, width : i32, height : i32) {
        self.resize_(width, height)
    }
}

////////////////////////////////////////////////////////////////////////////////
// Tests

#[test]
fn test_buffer() {
    let mut area = MsgArea::new(10, 10);
    area.add_client_msg("first line");
    area.add_client_msg("second line");
    assert_eq!(&area.buf, &"first linesecond line".chars().collect::<Vec<char>>());
}

#[test]
fn test_split_segments_1() {
    let mut line = Line::new();


}

#[test]
fn test_count_segments() {
    let mut area = MsgArea::new(10, 10);
    area.add_client_msg("first line"); // one segment
    area.add_client_msg("second line"); // two segments
    let n_segments : usize = area.lines.iter().map(|l| l.segs.len()).sum();
    assert_eq!(n_segments, 3);
}
*/
