use std::cmp::max;

use libtiny_common::{ChanNameRef, MsgSource, TabStyle};
use termbox_simple::Termbox;

use crate::config::{Colors, Style};
use crate::tab::Tab;

use tab_line::TabLine;
use tab_panel::TabPanel;

mod tab_line;
mod tab_panel;

/// Controls and displays tabs for servers, channels and private messages
#[derive(Debug, Copy, Clone)]
pub(crate) enum TabArea {
    TabLine(TabLine),
    TabPanel(TabPanel),
}

impl TabArea {
    pub(crate) fn new(width: i32) -> TabArea {
        TabArea::TabLine(TabLine::new(width))
    }

    pub(crate) fn draw(&self, tb: &mut Termbox, tabs: &[Tab], height: i32, colors: &Colors) {
        match self {
            TabArea::TabLine(line) => line.draw(tb, tabs, height, colors),
            TabArea::TabPanel(panel) => panel.draw(tb, tabs, height, colors),
        }
    }

    pub(crate) fn resize(&mut self, tabs: &mut [Tab], width: i32, height: i32) {
        match self {
            TabArea::TabLine(line) => line.resize(tabs, width, height),
            TabArea::TabPanel(panel) => {
                panel.resize(tabs, width, height);
                let active_idx = panel.active_idx;
                self.select_tab(active_idx, tabs);
            }
        }
    }

    pub(crate) fn calculate_x_offset(&mut self, tui_width: i32) -> i32 {
        match self {
            TabArea::TabLine(_) => 0,
            TabArea::TabPanel(_) => calculate_panel_width(tui_width) + 1,
        }
    }

    pub(crate) fn active_idx(&self) -> usize {
        match self {
            TabArea::TabLine(line) => line.active_idx,
            TabArea::TabPanel(panel) => panel.active_idx,
        }
    }

    pub(crate) fn next_tab_(&mut self, tabs: &[Tab]) {
        match self {
            TabArea::TabLine(line) => line.next_tab_(tabs),
            TabArea::TabPanel(panel) => panel.next_tab_(tabs.len()),
        }
    }

    pub(crate) fn prev_tab_(&mut self, tabs: &[Tab]) {
        match self {
            TabArea::TabLine(line) => line.prev_tab_(tabs),
            TabArea::TabPanel(panel) => panel.prev_tab_(tabs.len()),
        }
    }

    pub fn select_tab(&mut self, tab_idx: usize, tabs: &[Tab]) {
        if tab_idx < self.active_idx() {
            while tab_idx < self.active_idx() {
                self.prev_tab_(tabs);
            }
        } else {
            while tab_idx > self.active_idx() {
                self.next_tab_(tabs);
            }
        }
    }

    pub(crate) fn move_tab_left(&mut self, tabs: &mut Vec<Tab>) {
        if self.active_idx() == 0 {
            return;
        }
        if self.is_server_tab(self.active_idx(), tabs) {
            // move all server tabs
            let (left, right) = self.server_tab_range(self.active_idx(), tabs);
            if left > 0 {
                let mut insert_idx = left - 1;
                while insert_idx > 0 && !self.is_server_tab(insert_idx, tabs) {
                    insert_idx -= 1;
                }
                let to_move: Vec<Tab> = tabs.drain(left..right).collect();
                tabs.splice(insert_idx..insert_idx, to_move.into_iter());
                self.select_tab(insert_idx, tabs);
            }
        } else if !self.is_server_tab(self.active_idx() - 1, tabs) {
            let mut active_idx = self.active_idx();
            let tab = tabs.remove(active_idx);
            tabs.insert(active_idx - 1, tab);
            active_idx = active_idx - 1;
            self.select_tab(active_idx, tabs);
        }
    }

    pub(crate) fn move_tab_right(&mut self, tabs: &mut Vec<Tab>) {
        if self.active_idx() == tabs.len() - 1 {
            return;
        }
        if self.is_server_tab(self.active_idx(), tabs) {
            // move all server tabs
            let (left, right) = self.server_tab_range(self.active_idx(), tabs);
            if right < tabs.len() {
                let right_next = self.server_tab_range(right, tabs).1;
                let insert_idx = right_next - (right - left);
                let to_move: Vec<Tab> = tabs.drain(left..right).collect();
                tabs.splice(insert_idx..insert_idx, to_move.into_iter());
                self.select_tab(insert_idx, tabs);
            }
        } else if !self.is_server_tab(self.active_idx() + 1, tabs) {
            let active_idx = self.active_idx();
            let tab = tabs.remove(active_idx);
            tabs.insert(active_idx + 1, tab);
            let active_idx = active_idx + 1;
            self.select_tab(active_idx, tabs);
        }
    }

    fn is_server_tab(&self, idx: usize, tabs: &[Tab]) -> bool {
        is_server_tab(&tabs[idx])
    }

    /// Given a tab index return range of tabs for the server of this tab.
    fn server_tab_range(&self, idx: usize, tabs: &[Tab]) -> (usize, usize) {
        debug_assert!(idx < tabs.len());
        let mut left = idx;
        while !self.is_server_tab(left, tabs) {
            left -= 1;
        }
        let mut right = idx + 1;
        while right < tabs.len() && !self.is_server_tab(right, tabs) {
            right += 1;
        }
        (left, right)
    }

    pub(crate) fn switch(&mut self, string: &str, tabs: &[Tab]) {
        let mut next_idx = self.active_idx();
        for (tab_idx, tab) in tabs.iter().enumerate() {
            match tab.src {
                MsgSource::Serv { ref serv } => {
                    if serv.contains(string) {
                        next_idx = tab_idx;
                        break;
                    }
                }
                MsgSource::Chan { ref chan, .. } => {
                    // TODO: Case sensitive matching here is not ideal
                    if chan.display().contains(string) {
                        next_idx = tab_idx;
                        break;
                    }
                }
                MsgSource::User { ref nick, .. } => {
                    if nick.contains(string) {
                        next_idx = tab_idx;
                        break;
                    }
                }
            }
        }
        if next_idx != self.active_idx() {
            self.select_tab(next_idx, tabs);
        }
    }

    pub fn fix_scroll_after_close(&mut self, tabs: &[Tab]) {
        match self {
            TabArea::TabLine(line) => line.fix_scroll_after_close(tabs),
            TabArea::TabPanel(_) => {}
        }
    }

    ////////////////////////////////////////////////////////////////////////////
    // Helpers

    pub(crate) fn find_serv_tab_idx(&self, serv_: &str, tabs: &[Tab]) -> Option<usize> {
        for (tab_idx, tab) in tabs.iter().enumerate() {
            if let MsgSource::Serv { ref serv } = tab.src {
                if serv_ == serv {
                    return Some(tab_idx);
                }
            }
        }
        None
    }

    pub(crate) fn find_chan_tab_idx(
        &self,
        serv_: &str,
        chan_: &ChanNameRef,
        tabs: &[Tab],
    ) -> Option<usize> {
        for (tab_idx, tab) in tabs.iter().enumerate() {
            if let MsgSource::Chan { ref serv, ref chan } = tab.src {
                if serv_ == serv && chan_ == chan {
                    return Some(tab_idx);
                }
            }
        }
        None
    }

    pub(crate) fn find_user_tab_idx(
        &self,
        serv_: &str,
        nick_: &str,
        tabs: &[Tab],
    ) -> Option<usize> {
        for (tab_idx, tab) in tabs.iter().enumerate() {
            if let MsgSource::User { ref serv, ref nick } = tab.src {
                if serv_ == serv && nick_ == nick {
                    return Some(tab_idx);
                }
            }
        }
        None
    }

    /// Index of the last tab with the given server name.
    pub(crate) fn find_last_serv_tab_idx(&self, serv: &str, tabs: &[Tab]) -> Option<usize> {
        for (tab_idx, tab) in tabs.iter().enumerate().rev() {
            if tab.src.serv_name() == serv {
                return Some(tab_idx);
            }
        }
        None
    }
}

fn calculate_panel_width(tui_width: i32) -> i32 {
    max(tui_width / 8, 10)
}

fn is_server_tab(tab: &Tab) -> bool {
    match tab.src {
        MsgSource::Serv { .. } => true,
        MsgSource::Chan { .. } | MsgSource::User { .. } => false,
    }
}

fn arrow_style(tabs: &[Tab], colors: &Colors) -> Style {
    let tab_style = tabs
        .iter()
        .map(|tab| tab.style)
        .max()
        .unwrap_or(TabStyle::Normal);
    match tab_style {
        TabStyle::Normal => colors.tab_normal,
        TabStyle::JoinOrPart => colors.tab_joinpart,
        TabStyle::NewMsg => colors.tab_new_msg,
        TabStyle::Highlight => colors.tab_highlight,
    }
}
