pub mod line;

use std::cmp::max;
use std::mem;
use std::str;

use termbox_simple::Termbox;

pub use self::line::Line;
pub use self::line::SegStyle;
use crate::config::Colors;

pub struct MsgArea {
    lines: Vec<Line>,

    // Rendering related
    width: i32,
    height: i32,

    /// Vertical scroll: An offset from the last visible line.
    /// E.g. when this is 0, `self.lines[self.lines.len() - 1]` is drawn at the
    /// bottom of screen.
    scroll: i32,

    line_buf: Line,

    /// Cached total rendered height of all lines. Invalidate on resize, update
    /// when adding new lines.
    lines_height: Option<i32>,
}

impl MsgArea {
    pub fn new(width: i32, height: i32) -> MsgArea {
        MsgArea {
            lines: Vec::new(),
            width,
            height,
            scroll: 0,
            line_buf: Line::new(),
            lines_height: Some(0),
        }
    }

    pub fn resize(&mut self, width: i32, height: i32) {
        self.width = width;
        self.height = height;
        self.lines_height = None;
    }

    pub fn draw(&self, tb: &mut Termbox, colors: &Colors, pos_x: i32, pos_y: i32) {
        // Where to render current line
        let mut row = pos_y + self.height - 1;

        // How many visible lines to skip
        let mut skip = self.scroll;

        // Draw lines in reverse order
        let mut line_idx = (self.lines.len() as i32) - 1;
        while line_idx >= 0 && row >= pos_y {
            let line = &self.lines[line_idx as usize];
            let line_height = line.rendered_height(self.width);
            debug_assert!(line_height > 0);

            if skip >= line_height {
                // skip the whole line
                line_idx -= 1;
                skip -= line_height;
                continue;
            }

            // Rendered line height
            let height = line_height - skip;

            // Where to start rendering this line?
            let line_row = row - height + 1;

            // How many lines to skip in the `Line` before rendering
            let render_from = max(0, pos_y - line_row);

            line.draw(tb, colors, pos_x, line_row, render_from, height, self.width);
            row = line_row - 1;
            line_idx -= 1;
            skip = 0;

            if line_row < pos_y {
                break;
            }
        }
    }
}

////////////////////////////////////////////////////////////////////////////////
// Scrolling

impl MsgArea {
    fn lines_height(&mut self) -> i32 {
        match self.lines_height {
            Some(height) => height,
            None => {
                let mut total_height = 0;
                for line in &self.lines {
                    total_height += line.rendered_height(self.width);
                }
                self.lines_height = Some(total_height);
                total_height
            }
        }
    }

    pub fn scroll_up(&mut self) {
        if self.scroll < max(0, self.lines_height() - self.height) {
            self.scroll += 1;
        }
    }

    pub fn scroll_down(&mut self) {
        if self.scroll > 0 {
            self.scroll -= 1;
        }
    }

    pub fn scroll_top(&mut self) {
        self.scroll = max(0, self.lines_height() - self.height);
    }

    pub fn scroll_bottom(&mut self) {
        self.scroll = 0;
    }

    pub fn page_up(&mut self) {
        for _ in 0..10 {
            self.scroll_up();
        }
    }

    pub fn page_down(&mut self) {
        self.scroll = max(0, self.scroll - 10);
    }
}

////////////////////////////////////////////////////////////////////////////////
// Adding/removing text

impl MsgArea {
    pub fn set_style(&mut self, style: SegStyle) {
        self.line_buf.set_style(style);
    }

    pub fn add_text(&mut self, str: &str) {
        self.line_buf.add_text(str);
    }

    pub fn add_char(&mut self, char: char) {
        self.line_buf.add_char(char);
    }

    pub fn flush_line(&mut self) -> usize {
        let line_height = self.line_buf.rendered_height(self.width);
        self.lines
            .push(mem::replace(&mut self.line_buf, Line::new()));
        if self.scroll != 0 {
            self.scroll += line_height;
        }
        if let Some(ref mut total_height) = self.lines_height {
            *total_height += line_height;
        }
        self.lines.len() - 1
    }

    #[inline]
    pub fn modify_line<F>(&mut self, idx: usize, f: F)
    where
        F: Fn(&mut Line),
    {
        f(&mut self.lines[idx]);
    }

    pub fn clear(&mut self) {
        self.lines.clear();
        self.scroll = 0;
        self.lines_height = None;
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn newline_scrolling() {
        let mut msg_area = MsgArea::new(100, 1);
        // Adding a new line when scroll is 0 should not change it
        assert_eq!(msg_area.scroll, 0);
        msg_area.add_text("line1");
        msg_area.flush_line();
        assert_eq!(msg_area.scroll, 0);

        msg_area.add_text("line2");
        msg_area.flush_line();
        assert_eq!(msg_area.scroll, 0);

        msg_area.scroll_up();
        assert_eq!(msg_area.scroll, 1);
        msg_area.add_text("line3");
        msg_area.flush_line();
        assert_eq!(msg_area.scroll, 2);
    }
}
