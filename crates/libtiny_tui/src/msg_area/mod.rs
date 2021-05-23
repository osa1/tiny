pub(crate) mod line;

use std::collections::VecDeque;
use std::{cmp::max, mem, str};
use termbox_simple::Termbox;

pub(crate) use self::line::{Line, SegStyle};
use crate::config::Colors;
use crate::line_split::LineType;
use crate::messaging::{Timestamp, MSG_NICK_SUFFIX_LEN};

pub(crate) struct MsgArea {
    lines: VecDeque<Line>,
    scrollback: usize,

    // Rendering related
    width: i32,
    height: i32,

    /// Vertical scroll
    scroll: Scroll,

    line_buf: Line,

    /// Cached total rendered height of all lines. Invalidate on resize, update
    /// when adding new lines.
    lines_height: Option<i32>,

    layout: Layout,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Layout {
    Compact,
    Aligned { max_nick_len: usize },
}

impl Layout {
    fn msg_padding(&self) -> usize {
        match self {
            Layout::Compact => 0,
            Layout::Aligned { max_nick_len } => {
                Timestamp::WIDTH + max_nick_len + MSG_NICK_SUFFIX_LEN
            }
        }
    }
}

#[derive(Debug, Default)]
struct Scroll {
    /// An offset of visible lines from the last visible line of the most recent Line
    /// E.g. when this is 0, `self.lines.last()` is drawn at the
    /// bottom of screen. When it is not 0, it is N lines up in the message area.
    scroll: i32,
    /// Current index into `self.lines` of the Line drawn at the top of the screen.
    /// Used to recalculate `scroll` on resizing of window and maintain scrolling in the correct place
    line_idx: usize,
}

impl MsgArea {
    pub(crate) fn new(width: i32, height: i32, scrollback: usize, layout: Layout) -> MsgArea {
        MsgArea {
            lines: VecDeque::with_capacity(512.min(scrollback)),
            scrollback,
            width,
            height,
            scroll: Scroll::default(),
            line_buf: Line::new(),
            lines_height: Some(0),
            layout,
        }
    }

    pub(crate) fn get_height(&self) -> i32 {
        self.height
    }

    pub(crate) fn resize(&mut self, width: i32, height: i32) {
        self.width = width;
        self.height = height;
        self.lines_height = None;
        self.total_visible_lines();
    }

    pub(crate) fn layout(&self) -> Layout {
        self.layout
    }

    /// Used to force a line to be aligned.
    pub(crate) fn set_current_line_alignment(&mut self) {
        let msg_padding = self.layout.msg_padding();
        self.line_buf.set_type(LineType::AlignedMsg { msg_padding });
    }

    pub(crate) fn draw(&mut self, tb: &mut Termbox, colors: &Colors, pos_x: i32, pos_y: i32) {
        // Where to render current line
        let mut row = pos_y + self.height - 1;

        // How many visible lines to skip
        let mut skip = self.scroll.scroll;

        // Draw lines in reverse order
        let mut line_idx = (self.lines.len() as i32) - 1;
        while line_idx >= 0 && row >= pos_y {
            let line = &mut self.lines[line_idx as usize];
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

            line.draw(tb, colors, pos_x, line_row, render_from, height);
            row = line_row - 1;
            line_idx -= 1;
            skip = 0;

            if line_row < pos_y {
                break;
            }
        }

        // Set index of the current top-most line, +1 since last loop subtracted
        self.scroll.line_idx = (line_idx + 1) as usize;
    }
}

////////////////////////////////////////////////////////////////////////////////
// Scrolling

impl MsgArea {
    /// The total number of visible lines if each Line was rendered at the current screen width
    fn total_visible_lines(&mut self) -> i32 {
        match self.lines_height {
            Some(height) => height,
            None => {
                let mut total_height = 0;
                let recalculate_scroll = self.recalculate_scroll();
                for (idx, line) in &mut self.lines.iter_mut().enumerate() {
                    let height = line.rendered_height(self.width);
                    // If we need to recalculate, keep adding line heights until we get to line_idx
                    if recalculate_scroll && idx >= self.scroll.line_idx {
                        self.scroll.scroll += height;
                    }
                    total_height += height;
                }
                self.lines_height = Some(total_height);
                total_height
            }
        }
    }

    pub(crate) fn scroll_up(&mut self) {
        if self.scroll.scroll < max(0, self.total_visible_lines() - self.height) {
            self.scroll.scroll += 1;
        }
    }

    pub(crate) fn scroll_down(&mut self) {
        if self.scroll.scroll > 0 {
            self.scroll.scroll -= 1;
        }
    }

    pub(crate) fn scroll_top(&mut self) {
        self.scroll.scroll = max(0, self.total_visible_lines() - self.height);
    }

    pub(crate) fn scroll_bottom(&mut self) {
        self.scroll.scroll = 0;
    }

    pub(crate) fn page_up(&mut self) {
        for _ in 0..10 {
            self.scroll_up();
        }
    }

    pub(crate) fn page_down(&mut self) {
        self.scroll.scroll = max(0, self.scroll.scroll - 10);
    }

    /// Determines if we need to recalculate the scroll offset due to resizing of the window
    fn recalculate_scroll(&mut self) -> bool {
        if self.scroll.scroll > 0 {
            // Subtract current height since our self.scroll.line_idx is the top of the window,
            // but self.scroll.scroll is the offset from the bottom
            self.scroll.scroll = -self.height;
            true
        } else {
            false
        }
    }
}

////////////////////////////////////////////////////////////////////////////////
// Adding/removing text
impl MsgArea {
    pub(crate) fn add_text(&mut self, str: &str, style: SegStyle) {
        self.line_buf.add_text(str, style);
    }

    pub(crate) fn flush_line(&mut self) -> usize {
        let line_height = self.line_buf.rendered_height(self.width);
        // Check if we're about to overflow
        let mut removed_line_height = 0;
        if self.lines.len() == self.scrollback {
            // Remove oldest line
            if let Some(mut removed) = self.lines.pop_front() {
                removed_line_height = removed.rendered_height(self.width);
            }
        }
        self.lines
            .push_back(mem::replace(&mut self.line_buf, Line::new()));
        if self.scroll.scroll != 0 {
            self.scroll.scroll += line_height;
        }
        if let Some(ref mut total_height) = self.lines_height {
            *total_height += line_height - removed_line_height;
        }
        self.lines.len() - 1
    }

    pub(crate) fn modify_line<F>(&mut self, idx: usize, f: F)
    where
        F: Fn(&mut Line),
    {
        f(&mut self.lines[idx]);
        // Line was modified so we need to invalidate its height
        self.lines[idx].force_recalculation();
    }

    pub(crate) fn clear(&mut self) {
        self.lines.clear();
        self.scroll.scroll = 0;
        self.lines_height = None;
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn newline_scrolling() {
        let mut msg_area = MsgArea::new(100, 1, usize::MAX, Layout::Compact);
        // Adding a new line when scroll is 0 should not change it
        assert_eq!(msg_area.scroll.scroll, 0);
        msg_area.add_text("line1", SegStyle::UserMsg);
        msg_area.flush_line();
        assert_eq!(msg_area.scroll.scroll, 0);

        msg_area.add_text("line2", SegStyle::UserMsg);
        msg_area.flush_line();
        assert_eq!(msg_area.scroll.scroll, 0);

        msg_area.scroll_up();
        assert_eq!(msg_area.scroll.scroll, 1);
        msg_area.add_text("line3", SegStyle::UserMsg);
        msg_area.flush_line();
        assert_eq!(msg_area.scroll.scroll, 2);
    }

    #[test]
    fn test_max_lines() {
        // Can't show more than 3 lines.
        let mut msg_area = MsgArea::new(100, 1, 3, Layout::Compact);
        msg_area.add_text("first", SegStyle::UserMsg);
        msg_area.flush_line();
        msg_area.add_text("second", SegStyle::UserMsg);
        msg_area.flush_line();
        msg_area.add_text("third", SegStyle::UserMsg);
        msg_area.flush_line();
        assert_eq!(msg_area.lines.len(), 3);
        msg_area.add_text("fourth", SegStyle::UserMsg);
        // Will pop out "first" line
        msg_area.flush_line();
        assert_eq!(msg_area.lines.len(), 3);
        assert_eq!(msg_area.total_visible_lines(), 3);
    }
}
