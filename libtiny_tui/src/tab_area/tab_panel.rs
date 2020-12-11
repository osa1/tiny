use termbox_simple::Termbox;

use crate::config::Colors;
use crate::tab::Tab;
use crate::tab_area::tab_line::TabLine;
use crate::tab_area::{arrow_style, calculate_panel_width, is_server_tab};

const UP_ARROW: char = '↑';
const DOWN_ARROW: char = '↓';
const VERTICAL_LINE: char = '│';

/// A panel where tabs are displayed vertically
/// with scrolling up and down on overflow
#[derive(Debug, Copy, Clone)]
pub(crate) struct TabPanel {
    pub(crate) active_idx: usize,
    pub(crate) width: i32,
    height: i32,
    page: usize,
}

impl From<TabLine> for TabPanel {
    fn from(line: TabLine) -> TabPanel {
        TabPanel {
            active_idx: line.active_idx,
            width: calculate_panel_width(line.width),
            height: 1,
            page: 1,
        }
    }
}

impl TabPanel {
    pub(crate) fn draw(
        &self,
        tb: &mut Termbox,
        tabs: &[Tab],
        height: i32,
        colors: &Colors,
        statusline_height: i32,
    ) {
        let mut pos_y = statusline_height;
        let skipped = height as usize * (self.page.saturating_sub(1));
        let mut tabs_iter = tabs.iter().enumerate().skip(skipped).peekable();
        for idx in pos_y..height {
            if let Some((tab_idx, tab)) = tabs_iter.next() {
                // indent the channels under their server
                let x_offset = if is_server_tab(tab) { 0 } else { 1 };
                tab.draw(
                    tb,
                    colors,
                    x_offset,
                    pos_y,
                    self.active_idx == tab_idx,
                    Some(self.width - 1 - x_offset),
                );
            }
            if idx == statusline_height && self.page > 1 {
                // top arrow
                let arrow_style = arrow_style(&tabs[..skipped], colors);
                tb.change_cell(self.width, pos_y, UP_ARROW, arrow_style.fg, arrow_style.bg);
            } else if idx == height - 1 && tabs_iter.peek().is_some() {
                // bottom arrow
                let arrow_style = arrow_style(&tabs[skipped + idx as usize + 1..], colors);
                tb.change_cell(
                    self.width,
                    pos_y,
                    DOWN_ARROW,
                    arrow_style.fg,
                    arrow_style.bg,
                );
            } else {
                tb.change_cell(
                    self.width,
                    pos_y,
                    VERTICAL_LINE,
                    colors.faded.fg,
                    colors.faded.bg,
                );
            }
            pos_y += 1;
        }
    }

    pub(crate) fn resize(
        &mut self,
        tabs: &mut [Tab],
        tui_width: i32,
        height: i32,
        statusline_height: i32,
    ) {
        self.width = calculate_panel_width(tui_width);
        self.height = height - statusline_height;
        self.page = 1;
        let tab_area_offset = calculate_panel_width(tui_width) + 1;
        // resize all the tabs
        for tab in tabs {
            tab.widget
                .resize(tui_width - tab_area_offset, height - statusline_height);
        }
    }

    pub(crate) fn next_tab_(&mut self, tabs_len: usize) {
        if self.active_idx == tabs_len - 1 {
            // go to beginning of tab list
            self.active_idx = 0;
            self.page = 1;
        } else {
            self.active_idx += 1;
            if self.active_idx == self.height as usize * self.page {
                self.page += 1;
            }
        }
    }

    pub(crate) fn prev_tab_(&mut self, tabs_len: usize) {
        if self.active_idx == 0 {
            self.active_idx = tabs_len - 1;
            self.page = tabs_len / self.height as usize;
        } else {
            if self.active_idx == self.height as usize * (self.page.saturating_sub(1)) {
                self.page -= 1;
            }
            self.active_idx -= 1;
        }
    }

    pub(crate) fn next_page(&mut self, tabs_len: usize) {
        let max_page = (tabs_len / self.height as usize) + 1;
        if self.page == max_page {
            self.page = 1;
        } else {
            self.page += 1;
        }
    }

    pub(crate) fn prev_page(&mut self, tabs_len: usize) {
        let max_page = (tabs_len / self.height as usize) + 1;
        if self.page == 1 {
            self.page = max_page;
        } else {
            self.page -= 1;
        }
    }
}
