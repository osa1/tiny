use termbox_simple::Termbox;

use crate::config::Colors;
use crate::tab::Tab;
use crate::tab_area::arrow_style;
use crate::tab_area::tab_panel::TabPanel;

const LEFT_ARROW: char = '<';
const RIGHT_ARROW: char = '>';

/// A one line display of all tabs
/// with scrolling to the left and right on overflow
#[derive(Debug, Copy, Clone)]
pub(crate) struct TabLine {
    pub(crate) active_idx: usize,
    pub(crate) width: i32,
    h_scroll: i32,
}

impl From<TabPanel> for TabLine {
    fn from(panel: TabPanel) -> TabLine {
        TabLine {
            active_idx: panel.active_idx,
            width: 1,
            h_scroll: 0,
        }
    }
}

impl TabLine {
    pub fn new(width: i32) -> TabLine {
        TabLine {
            width,
            h_scroll: 0,
            active_idx: 0,
        }
    }

    fn draw_left_arrow(&self) -> bool {
        self.h_scroll > 0
    }

    fn draw_right_arrow(&self, tabs: &[Tab]) -> bool {
        let w1 = self.h_scroll + self.width;
        let w2 = {
            let mut w = if self.draw_left_arrow() { 2 } else { 0 };
            let last_tab_idx = tabs.len() - 1;
            for (tab_idx, tab) in tabs.iter().enumerate() {
                w += tab.width();
                if tab_idx != last_tab_idx {
                    w += 1;
                }
            }
            w
        };

        w2 > w1
    }

    pub(crate) fn draw(&self, tb: &mut Termbox, tabs: &[Tab], height: i32, colors: &Colors) {
        // decide whether we need to draw left/right arrows in tab bar
        let left_arr = self.draw_left_arrow();
        let right_arr = self.draw_right_arrow(tabs);

        let (tab_left, tab_right) = self.rendered_tabs(tabs);

        let mut pos_x: i32 = 0;
        if left_arr {
            let style = arrow_style(&tabs[0..tab_left], colors);
            tb.change_cell(pos_x, height - 1, LEFT_ARROW, style.fg, style.bg);
            pos_x += 2;
        }

        // Debugging
        // debug!("number of tabs to draw: {}", tab_right - tab_left);
        // debug!("left_arr: {}, right_arr: {}", left_arr, right_arr);

        // finally draw the tabs
        for (tab_idx, tab) in (&tabs[tab_left..tab_right]).iter().enumerate() {
            tab.draw(
                tb,
                &colors,
                pos_x,
                height - 1,
                self.active_idx == tab_idx + tab_left,
                None,
            );
            pos_x += tab.width() as i32 + 1; // +1 for margin
        }

        if right_arr {
            let style = arrow_style(&tabs[tab_right..], colors);
            tb.change_cell(pos_x, height - 1, RIGHT_ARROW, style.fg, style.bg);
        }
    }

    pub(crate) fn resize(&mut self, tabs: &mut [Tab], width: i32, height: i32) {
        self.width = width;
        for tab in tabs.iter_mut() {
            tab.widget.resize(self.width, height - 1);
        }
        // scroll the tab bar so that currently active tab is still visible
        let (mut tab_left, mut tab_right) = self.rendered_tabs(tabs);
        if tab_left == tab_right {
            // nothing to show
            return;
        }
        while self.active_idx < tab_left || self.active_idx >= tab_right {
            if self.active_idx >= tab_right {
                // scroll right
                self.h_scroll += tabs[tab_left].width() + 1;
            } else if self.active_idx < tab_left {
                // scroll left
                self.h_scroll -= tabs[tab_left - 1].width() + 1;
            }
            let (tab_left_, tab_right_) = self.rendered_tabs(tabs);
            tab_left = tab_left_;
            tab_right = tab_right_;
        }
        // the selected tab is visible. scroll to the left as much as possible
        // to make more tabs visible.
        let mut num_visible = tab_right - tab_left;
        loop {
            if tab_left == 0 {
                break;
            }
            // save current scroll value
            let scroll_orig = self.h_scroll;
            // scoll to the left
            self.h_scroll -= tabs[tab_left - 1].width() + 1;
            // get new bounds
            let (tab_left_, tab_right_) = self.rendered_tabs(tabs);
            // commit if these two conditions hold
            let num_visible_ = tab_right_ - tab_left_;
            let more_tabs_visible = num_visible_ > num_visible;
            let selected_tab_visible = self.active_idx >= tab_left_ && self.active_idx < tab_right_;
            if !(more_tabs_visible && selected_tab_visible) {
                // revert scroll value and abort
                self.h_scroll = scroll_orig;
                break;
            }
            // otherwise commit
            tab_left = tab_left_;
            num_visible = num_visible_;
        }
    }

    // right one is exclusive
    fn rendered_tabs(&self, tabs: &[Tab]) -> (usize, usize) {
        if tabs.is_empty() {
            return (0, 0);
        }

        let mut i = 0;

        {
            let mut skip = self.h_scroll;
            while skip > 0 && i < tabs.len() - 1 {
                skip -= tabs[i].width() + 1;
                i += 1;
            }
        }

        // drop tabs overflow on the right side
        let mut j = i;
        {
            // how much space left on screen
            let mut width_left = self.width;
            if self.draw_left_arrow() {
                width_left -= 2;
            }
            if self.draw_right_arrow(tabs) {
                width_left -= 2;
            }
            // drop any tabs that overflows from the screen
            for (tab_idx, tab) in (&tabs[i..]).iter().enumerate() {
                if tab.width() > width_left {
                    break;
                } else {
                    j += 1;
                    width_left -= tab.width();
                    if tab_idx != tabs.len() - i {
                        width_left -= 1;
                    }
                }
            }
        }

        debug_assert!(i < tabs.len());
        debug_assert!(j <= tabs.len());
        debug_assert!(i <= j);

        (i, j)
    }

    /// After closing a tab scroll left if there is space on the right and we can fit more tabs
    /// from the left into the visible part of the tab bar.
    pub fn fix_scroll_after_close(&mut self, tabs: &[Tab]) {
        let (tab_left, tab_right) = self.rendered_tabs(tabs);

        if tab_left == 0 {
            self.h_scroll = 0;
            return;
        }

        // Size of shown part of the tab bar. DOES NOT include LEFT_ARROW.
        let mut shown_width = 0;
        for (tab_idx, tab) in tabs[tab_left..tab_right].iter().enumerate() {
            shown_width += tab.width();
            if tab_idx != tab_right - 1 {
                shown_width += 1; // space between tabs
            }
        }

        // How much space left in tab bar. Not accounting for LEFT_ARROW here!
        let mut space_left = self.width - shown_width;

        // How much to scroll left
        let mut scroll_left = 0;

        // Start iterating tabs on the left, add the tab size to `scroll_left` as long as scrolling
        // doesn't make the right-most tab go out of bounds
        for left_tab_idx in (0..tab_left).rev() {
            let tab_width = tabs[left_tab_idx].width() + 1; // 1 for space
            let draw_arrow = left_tab_idx != 0;
            let tab_with_arrow_w = tab_width + if draw_arrow { 2 } else { 0 };

            if tab_with_arrow_w <= space_left {
                scroll_left += tab_width;
                space_left -= tab_width;
            } else {
                break;
            }
        }

        self.h_scroll -= scroll_left;
    }

    pub(crate) fn next_tab_(&mut self, tabs: &[Tab]) {
        if self.active_idx == tabs.len() - 1 {
            self.active_idx = 0;
            self.h_scroll = 0;
        } else {
            // either the next tab is visible, or we should scroll so that the
            // next tab becomes visible
            let next_active = self.active_idx + 1;
            loop {
                let (tab_left, tab_right) = self.rendered_tabs(tabs);
                if (next_active >= tab_left && next_active < tab_right)
                    || (next_active == tab_left && tab_left == tab_right)
                {
                    break;
                }
                self.h_scroll += tabs[tab_left].width() + 1;
            }
            self.active_idx = next_active;
        }
    }

    pub(crate) fn prev_tab_(&mut self, tabs: &[Tab]) {
        if self.active_idx == 0 {
            let next_active = tabs.len() - 1;
            while self.active_idx != next_active {
                self.next_tab_(tabs);
            }
        } else {
            let next_active = self.active_idx - 1;
            loop {
                let (tab_left, tab_right) = self.rendered_tabs(tabs);
                if (next_active >= tab_left && next_active < tab_right)
                    || (next_active == tab_left && tab_left == tab_right)
                {
                    break;
                }
                self.h_scroll -= tabs[tab_left - 1].width() + 1;
            }
            if self.h_scroll < 0 {
                self.h_scroll = 0
            };
            self.active_idx = next_active;
        }
    }
}
