pub mod exit_dialogue;
pub mod messaging;
pub mod msg_area;
pub mod tab;
pub mod termbox;
pub mod text_field;
pub mod widget;
pub mod statusline;

use std::str;

pub use self::messaging::Timestamp;
use config::Colors;
use config::Style;
use notifier::Notifier;
use term_input::{Arrow, Event, Key};
use termbox_simple::{OutputMode, Termbox};
use trie::Trie;
use tui::messaging::MessagingUI;
pub use tui::tab::Tab;
pub use tui::tab::TabStyle;
use tui::widget::WidgetRet;
use tui::statusline::{draw_statusline, statusline_visible};

#[derive(Debug)]
pub enum TUIRet {
    Abort,
    KeyHandled,
    KeyIgnored(Key),
    EventIgnored(Event),

    /// INVARIANT: The vec will have at least one char.
    // Can't make MsgSource a ref because of this weird error:
    // https://users.rust-lang.org/t/borrow-checker-bug/5165
    Input {
        msg: Vec<char>,
        from: MsgSource,
    },

    /// A pasted string. Send directly.
    Lines {
        lines: Vec<String>,
        from: MsgSource,
    },
}

/// Target of a message to be shown on TUI.
pub enum MsgTarget<'a> {
    Server {
        serv_name: &'a str,
    },
    Chan {
        serv_name: &'a str,
        chan_name: &'a str,
    },
    User {
        serv_name: &'a str,
        nick: &'a str,
    },

    /// Show the message in all tabs of a server.
    AllServTabs {
        serv_name: &'a str,
    },

    /// Show the message all server tabs that have the user. (i.e. channels,
    /// privmsg tabs)
    AllUserTabs {
        serv_name: &'a str,
        nick: &'a str,
    },

    /// Show the message in currently active tab.
    CurrentTab,
}

/// Source of a message from the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MsgSource {
    /// Message sent in a server tab.
    Serv { serv_name: String },

    /// Message sent in a channel tab.
    Chan {
        serv_name: String,
        chan_name: String,
    },

    /// Message sent in a privmsg tab.
    User { serv_name: String, nick: String },
}

const LEFT_ARROW: char = '<';
const RIGHT_ARROW: char = '>';

pub struct TUI {
    /// Termbox instance
    tb: Termbox,

    /// Color scheme
    colors: Colors,

    tabs: Vec<Tab>,
    active_idx: usize,
    width: i32,
    height: i32,
    h_scroll: i32,

    /// Do we want to show statusline?
    show_statusline: bool,
    /// Is there room for statusline?
    statusline_visible: bool,
}

impl MsgSource {
    pub fn serv_name(&self) -> &str {
        match *self {
            MsgSource::Serv { ref serv_name }
            | MsgSource::Chan { ref serv_name, .. }
            | MsgSource::User { ref serv_name, .. } => serv_name,
        }
    }

    pub fn to_target(&self) -> MsgTarget {
        match *self {
            MsgSource::Serv { ref serv_name } => MsgTarget::Server { serv_name },
            MsgSource::Chan {
                ref serv_name,
                ref chan_name,
            } => MsgTarget::Chan {
                serv_name,
                chan_name,
            },
            MsgSource::User {
                ref serv_name,
                ref nick,
            } => MsgTarget::User { serv_name, nick },
        }
    }

    pub fn visible_name(&self) -> &str {
        match *self {
            MsgSource::Serv { ref serv_name, .. } => serv_name,
            MsgSource::Chan { ref chan_name, .. } => chan_name,
            MsgSource::User { ref nick, .. } => nick,
        }
    }
}

impl TUI {
    pub fn new(colors: Colors) -> TUI {
        let mut tb = Termbox::init().unwrap(); // TODO: check errors
        tb.set_output_mode(OutputMode::Output256);
        tb.set_clear_attributes(colors.clear.fg, colors.clear.bg);

        let width = tb.width() as i32;
        let height = tb.height() as i32;

        TUI {
            tb,
            colors,
            tabs: Vec::new(),
            active_idx: 0,
            width,
            height,
            h_scroll: 0,
            show_statusline: false,
            statusline_visible: statusline_visible(width, height),
        }
    }

    pub fn set_colors(&mut self, colors: Colors) {
        self.tb
            .set_clear_attributes(colors.clear.fg, colors.clear.bg);
        self.colors = colors;
    }

    fn new_tab(&mut self, idx: usize, src: MsgSource, status: bool, notifier: Notifier) {
        use std::collections::HashMap;

        let mut switch_keys: HashMap<char, i8> = HashMap::with_capacity(self.tabs.len());
        for tab in &self.tabs {
            if let Some(key) = tab.switch {
                switch_keys.entry(key).and_modify(|e| *e += 1).or_insert(1);
            }
        }

        let switch = {
            let mut ret = None;
            let mut n = 0;
            for ch in src.visible_name().chars() {
                if !ch.is_alphabetic() {
                    continue;
                }
                match switch_keys.get(&ch) {
                    None => {
                        ret = Some(ch);
                        break;
                    }
                    Some(n_) => {
                        if ret == None || n > *n_ {
                            ret = Some(ch);
                            n = *n_;
                        }
                    }
                }
            }
            ret
        };

        let statusline_height = if self.statusline_visible && self.show_statusline { 1 } else { 0 };
        self.tabs.insert(
            idx,
            Tab {
                widget: MessagingUI::new(self.width, self.height - 1 - statusline_height, status),
                src,
                style: TabStyle::Normal,
                switch,
                notifier,
            },
        );
    }

    /// Returns index of the new tab if a new tab is created.
    pub fn new_server_tab(&mut self, serv_name: &str) -> Option<usize> {
        match self.find_serv_tab_idx(serv_name) {
            None => {
                let tab_idx = self.tabs.len();
                self.new_tab(
                    tab_idx,
                    MsgSource::Serv {
                        serv_name: serv_name.to_owned(),
                    },
                    true,
                    Notifier::Mentions,
                );
                Some(tab_idx)
            }
            Some(_) => None,
        }
    }

    /// Closes a server tab and all associated channel tabs.
    pub fn close_server_tab(&mut self, serv_name: &str) {
        if let Some(tab_idx) = self.find_serv_tab_idx(serv_name) {
            self.tabs
                .retain(|tab: &Tab| tab.src.serv_name() != serv_name);
            if self.active_idx == tab_idx {
                self.select_tab(if tab_idx == 0 { 0 } else { tab_idx - 1 });
            }
        }
    }

    /// Returns index of the new tab if a new tab is created.
    pub fn new_chan_tab(&mut self, serv_name: &str, chan_name: &str) -> Option<usize> {
        match self.find_chan_tab_idx(serv_name, chan_name) {
            None => match self.find_last_serv_tab_idx(serv_name) {
                None => {
                    self.new_server_tab(serv_name);
                    self.new_chan_tab(serv_name, chan_name)
                }
                Some(serv_tab_idx) => {
                    let mut status_val: bool = true;
                    let mut notifier = Notifier::Mentions;
                    for tab in &self.tabs {
                        if let MsgSource::Serv {
                            serv_name: ref serv_name_,
                        } = tab.src
                        {
                            if serv_name == serv_name_ {
                                status_val = tab.widget.get_ignore_state();
                                notifier = tab.notifier;
                                break;
                            }
                        }
                    }
                    let tab_idx = serv_tab_idx + 1;
                    self.new_tab(
                        tab_idx,
                        MsgSource::Chan {
                            serv_name: serv_name.to_owned(),
                            chan_name: chan_name.to_owned(),
                        },
                        status_val,
                        notifier,
                    );
                    if self.active_idx >= tab_idx {
                        self.next_tab();
                    }
                    if let Some(nick) = self.tabs[serv_tab_idx].widget.get_nick().map(str::to_owned)
                    {
                        self.tabs[tab_idx].widget.set_nick(nick);
                    }
                    Some(tab_idx)
                }
            },
            Some(_) => None,
        }
    }

    pub fn close_chan_tab(&mut self, serv_name: &str, chan_name: &str) {
        if let Some(tab_idx) = self.find_chan_tab_idx(serv_name, chan_name) {
            self.tabs.remove(tab_idx);
            if self.active_idx == tab_idx {
                self.select_tab(if tab_idx == 0 { 0 } else { tab_idx - 1 });
            }
        }
    }

    /// Returns index of the new tab if a new tab is created.
    pub fn new_user_tab(&mut self, serv_name: &str, nick: &str) -> Option<usize> {
        match self.find_user_tab_idx(serv_name, nick) {
            None => match self.find_last_serv_tab_idx(serv_name) {
                None => {
                    self.new_server_tab(serv_name);
                    self.new_user_tab(serv_name, nick)
                }
                Some(tab_idx) => {
                    self.new_tab(
                        tab_idx + 1,
                        MsgSource::User {
                            serv_name: serv_name.to_owned(),
                            nick: nick.to_owned(),
                        },
                        true,
                        Notifier::Messages,
                    );
                    if let Some(nick) = self.tabs[tab_idx].widget.get_nick().map(str::to_owned) {
                        self.tabs[tab_idx + 1].widget.set_nick(nick);
                    }
                    self.tabs[tab_idx + 1].widget.join(nick, None);
                    Some(tab_idx + 1)
                }
            },
            Some(_) => None,
        }
    }

    pub fn close_user_tab(&mut self, serv_name: &str, nick: &str) {
        if let Some(tab_idx) = self.find_user_tab_idx(serv_name, nick) {
            self.tabs.remove(tab_idx);
            if self.active_idx == tab_idx {
                self.select_tab(if tab_idx == 0 { 0 } else { tab_idx - 1 });
            }
        }
    }

    pub fn handle_input_event(&mut self, ev: Event) -> TUIRet {
        match ev {
            Event::Resize => {
                // This never happens, probably because the our select() loop,
                // termbox can't really get resize signals.
                self.resize();
                TUIRet::KeyHandled
            }

            Event::Key(key) => self.keypressed(key),

            Event::String(str) => {
                // For some reason on my terminal newlines in text are
                // translated to carriage returns when pasting so we check for
                // both just to make sure
                if str.contains('\n') || str.contains('\r') {
                    return self.edit_input(&str);
                } else {
                    // TODO this may be too slow for pasting long single lines
                    for ch in str.chars() {
                        self.handle_input_event(Event::Key(Key::Char(ch)));
                    }
                }
                TUIRet::KeyHandled
            }

            ev => TUIRet::EventIgnored(ev),
        }
    }

    /// Edit current input + `str` before sending.
    fn edit_input(&mut self, str: &str) -> TUIRet {
        let tab = &mut self.tabs[self.active_idx].widget;
        let tf = tab.flush_input_field();
        match paste_lines(tf, &str) {
            Ok(lines) => {
                // If there's only one line just add it to the input field, do not send it
                if lines.len() == 1 {
                    tab.set_input_field(&lines[0]);
                    TUIRet::KeyHandled
                } else {
                    // Otherwise add the lines to text field history and send it
                    for line in &lines {
                        tab.add_input_field_history(line);
                    }
                    TUIRet::Lines {
                        lines,
                        from: self.tabs[self.active_idx].src.clone(),
                    }
                }
            }
            Err(err) => {
                use std::env::VarError;
                match err {
                    PasteError::Io(err) => {
                        self.add_client_err_msg(
                            &format!("Error while running $EDITOR: {:?}", err),
                            &MsgTarget::CurrentTab,
                        );
                    }
                    PasteError::Var(VarError::NotPresent) => {
                        self.add_client_err_msg(
                            "Can't paste multi-line string: \
                             make sure your $EDITOR is set",
                            &MsgTarget::CurrentTab,
                        );
                    }
                    PasteError::Var(VarError::NotUnicode(_)) => {
                        self.add_client_err_msg(
                            "Can't paste multi-line string: \
                             can't parse $EDITOR (not unicode)",
                            &MsgTarget::CurrentTab,
                        );
                    }
                    PasteError::PastedCmd => {
                        self.add_client_err_msg(
                            "One of the pasted lines looks like a command.",
                            &MsgTarget::CurrentTab,
                        );
                    }
                }
                TUIRet::KeyHandled
            }
        }
    }

    pub fn keypressed(&mut self, key: Key) -> TUIRet {
        match self.tabs[self.active_idx].widget.keypressed(key) {
            WidgetRet::KeyHandled => TUIRet::KeyHandled,
            WidgetRet::KeyIgnored => self.handle_keypress(key),
            WidgetRet::Input(input) => TUIRet::Input {
                msg: input,
                from: self.tabs[self.active_idx].src.clone(),
            },
            WidgetRet::Remove => unimplemented!(),
            WidgetRet::Abort => TUIRet::Abort,
        }
    }

    fn handle_keypress(&mut self, key: Key) -> TUIRet {
        match key {
            Key::Ctrl('n') => {
                self.next_tab();
                TUIRet::KeyHandled
            }

            Key::Ctrl('p') => {
                self.prev_tab();
                TUIRet::KeyHandled
            }

            Key::Ctrl('x') => self.edit_input(""),

            Key::AltChar(c) => match c.to_digit(10) {
                Some(i) => {
                    let new_tab_idx: usize = if i as usize > self.tabs.len() || i == 0 {
                        self.tabs.len() - 1
                    } else {
                        i as usize - 1
                    };
                    if new_tab_idx > self.active_idx {
                        for _ in 0..new_tab_idx - self.active_idx {
                            self.next_tab_();
                        }
                    } else if new_tab_idx < self.active_idx {
                        for _ in 0..self.active_idx - new_tab_idx {
                            self.prev_tab_();
                        }
                    }
                    self.tabs[self.active_idx].set_style(TabStyle::Normal);
                    TUIRet::KeyHandled
                }
                None => {
                    // multiple tabs can have same switch character so scan
                    // forwards instead of starting from the first tab
                    for i in 1..=self.tabs.len() {
                        let idx = (self.active_idx + i) % self.tabs.len();
                        if self.tabs[idx].switch == Some(c) {
                            self.select_tab(idx);
                            break;
                        }
                    }
                    TUIRet::KeyHandled
                }
            },

            Key::AltArrow(Arrow::Left) => {
                self.move_tab_left();
                TUIRet::KeyHandled
            }

            Key::AltArrow(Arrow::Right) => {
                self.move_tab_right();
                TUIRet::KeyHandled
            }

            key => TUIRet::KeyIgnored(key),
        }
    }

    pub fn resize(&mut self) {
        self.tb.resize();
        self.tb.clear();

        self.width = self.tb.width();
        self.height = self.tb.height();

        self.statusline_visible = statusline_visible(self.width, self.height);
        let statusline_height = if self.statusline_visible && self.show_statusline { 1 } else { 0 };
        for tab in &mut self.tabs {
            tab.widget.resize(self.width, self.height - 1 - statusline_height);
        }
        // scroll the tab bar so that currently active tab is still visible
        let (mut tab_left, mut tab_right) = self.rendered_tabs();
        if tab_left == tab_right {
            // nothing to show
            return;
        }
        while self.active_idx < tab_left || self.active_idx >= tab_right {
            if self.active_idx >= tab_right {
                // scroll right
                self.h_scroll += self.tabs[tab_left].width() + 1;
            } else if self.active_idx < tab_left {
                // scroll left
                self.h_scroll -= self.tabs[tab_left - 1].width() + 1;
            }
            let (tab_left_, tab_right_) = self.rendered_tabs();
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
            self.h_scroll -= self.tabs[tab_left - 1].width() + 1;
            // get new bounds
            let (tab_left_, tab_right_) = self.rendered_tabs();
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

    pub fn get_nicks(&self, serv_name: &str, chan_name: &str) -> Option<&Trie> {
        match self.find_chan_tab_idx(serv_name, chan_name) {
            None => None,
            Some(i) => Some(self.tabs[i].widget.get_nicks()),
        }
    }
}

////////////////////////////////////////////////////////////////////////////////
// Rendering

fn arrow_style(tabs: &[Tab], colors: &Colors) -> Style {
    let tab_style = tabs
        .iter()
        .map(|tab| tab.style)
        .max()
        .unwrap_or(TabStyle::Normal);
    match tab_style {
        TabStyle::Normal => colors.tab_normal,
        TabStyle::NewMsg => colors.tab_new_msg,
        TabStyle::Highlight => colors.tab_highlight,
    }
}

impl TUI {
    fn draw_left_arrow(&self) -> bool {
        self.h_scroll > 0
    }

    fn draw_right_arrow(&self) -> bool {
        let w1 = self.h_scroll + self.width;
        let w2 = {
            let mut w = if self.draw_left_arrow() { 2 } else { 0 };
            let last_tab_idx = self.tabs.len() - 1;
            for (tab_idx, tab) in self.tabs.iter().enumerate() {
                w += tab.width();
                if tab_idx != last_tab_idx {
                    w += 1;
                }
            }
            w
        };

        w2 > w1
    }

    // right one is exclusive
    fn rendered_tabs(&self) -> (usize, usize) {
        let mut i = 0;

        {
            let mut skip = self.h_scroll;
            while skip > 0 && i < self.tabs.len() - 1 {
                skip -= self.tabs[i].width() + 1;
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
            if self.draw_right_arrow() {
                width_left -= 2;
            }
            // drop any tabs that overflows from the screen
            for (tab_idx, tab) in (&self.tabs[i..]).iter().enumerate() {
                if tab.width() > width_left {
                    break;
                } else {
                    j += 1;
                    width_left -= tab.width();
                    if tab_idx != self.tabs.len() - i {
                        width_left -= 1;
                    }
                }
            }
        }

        debug_assert!(i < self.tabs.len());
        debug_assert!(j <= self.tabs.len());
        debug_assert!(i <= j);

        (i, j)
    }

    pub fn draw(&mut self) {
        self.tb.clear();

        let statusline_height = if self.statusline_visible && self.show_statusline { 1 } else { 0 };
        self.tabs[self.active_idx]
            .widget
            .draw(&mut self.tb, &self.colors, 0, statusline_height);

        if self.show_statusline && self.statusline_visible {
            draw_statusline(
                &mut self.tb,
                self.width,
                &self.colors,
                &self.tabs[self.active_idx].visible_name(),
                &self.tabs[self.active_idx].notifier,
                self.tabs[self.active_idx].widget.get_ignore_state()
            );
        }

        // decide whether we need to draw left/right arrows in tab bar
        let left_arr = self.draw_left_arrow();
        let right_arr = self.draw_right_arrow();

        let (tab_left, tab_right) = self.rendered_tabs();

        let mut pos_x: i32 = 0;
        if left_arr {
            let style = arrow_style(&self.tabs[0..tab_left], &self.colors);
            self.tb
                .change_cell(pos_x, self.height - 1, LEFT_ARROW, style.fg, style.bg);
            pos_x += 2;
        }

        // Debugging
        // eprintln!("number of tabs to draw: {}", tab_right - tab_left);
        // eprintln!("left_arr: {}, right_arr: {}", left_arr, right_arr);

        // finally draw the tabs
        for (tab_idx, tab) in (&self.tabs[tab_left..tab_right]).iter().enumerate() {
            tab.draw(
                &mut self.tb,
                &self.colors,
                pos_x,
                self.height - 1,
                self.active_idx == tab_idx + tab_left,
            );
            pos_x += tab.width() as i32 + 1; // +1 for margin
        }

        if right_arr {
            let style = arrow_style(&self.tabs[tab_right..], &self.colors);
            self.tb
                .change_cell(pos_x, self.height - 1, RIGHT_ARROW, style.fg, style.bg);
        }

        self.tb.present();
    }

    ////////////////////////////////////////////////////////////////////////////
    // Moving between tabs, horizontal scroll updates

    fn select_tab(&mut self, tab_idx: usize) {
        if tab_idx < self.active_idx {
            while tab_idx < self.active_idx {
                self.prev_tab_();
            }
        } else {
            while tab_idx > self.active_idx {
                self.next_tab_();
            }
        }
        self.tabs[self.active_idx].set_style(TabStyle::Normal);
    }

    fn next_tab(&mut self) {
        self.next_tab_();
        self.tabs[self.active_idx].set_style(TabStyle::Normal);
    }

    fn prev_tab(&mut self) {
        self.prev_tab_();
        self.tabs[self.active_idx].set_style(TabStyle::Normal);
    }

    pub fn switch(&mut self, string: &str) {
        let mut next_idx = self.active_idx;
        for (tab_idx, tab) in self.tabs.iter().enumerate() {
            match tab.src {
                MsgSource::Serv { ref serv_name } => {
                    if serv_name.contains(string) {
                        next_idx = tab_idx;
                        break;
                    }
                }
                MsgSource::Chan { ref chan_name, .. } => {
                    if chan_name.contains(string) {
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
        if next_idx != self.active_idx {
            self.select_tab(next_idx);
        }
    }

    fn next_tab_(&mut self) {
        if self.active_idx == self.tabs.len() - 1 {
            self.active_idx = 0;
            self.h_scroll = 0;
        } else {
            // either the next tab is visible, or we should scroll so that the
            // next tab becomes visible
            let next_active = self.active_idx + 1;
            loop {
                let (tab_left, tab_right) = self.rendered_tabs();
                if (next_active >= tab_left && next_active < tab_right)
                    || (next_active == tab_left && tab_left == tab_right)
                {
                    break;
                }
                self.h_scroll += self.tabs[tab_left].width() + 1;
            }
            self.active_idx = next_active;
        }
    }

    fn prev_tab_(&mut self) {
        if self.active_idx == 0 {
            let next_active = self.tabs.len() - 1;
            while self.active_idx != next_active {
                self.next_tab_();
            }
        } else {
            let next_active = self.active_idx - 1;
            loop {
                let (tab_left, tab_right) = self.rendered_tabs();
                if (next_active >= tab_left && next_active < tab_right)
                    || (next_active == tab_left && tab_left == tab_right)
                {
                    break;
                }
                self.h_scroll -= self.tabs[tab_left - 1].width() + 1;
            }
            if self.h_scroll < 0 {
                self.h_scroll = 0
            };
            self.active_idx = next_active;
        }
    }

    fn move_tab_left(&mut self) {
        if self.active_idx == 0 {
            return;
        }
        if self.is_server_tab(self.active_idx) {
            // move all server tabs
            let (left, right) = self.server_tab_range(self.active_idx);
            if left > 0 {
                let mut insert_idx = left - 1;
                while insert_idx > 0 && !self.is_server_tab(insert_idx) {
                    insert_idx -= 1;
                }
                let to_move: Vec<Tab> = self.tabs.drain(left..right).collect();
                self.tabs
                    .splice(insert_idx..insert_idx, to_move.into_iter());
                self.select_tab(insert_idx);
            }
        } else if !self.is_server_tab(self.active_idx - 1) {
            let tab = self.tabs.remove(self.active_idx);
            self.tabs.insert(self.active_idx - 1, tab);
            let active_idx = self.active_idx - 1;
            self.select_tab(active_idx);
        }
    }

    fn move_tab_right(&mut self) {
        if self.active_idx == self.tabs.len() - 1 {
            return;
        }
        if self.is_server_tab(self.active_idx) {
            // move all server tabs
            let (left, right) = self.server_tab_range(self.active_idx);
            if right < self.tabs.len() {
                let right_next = self.server_tab_range(right).1;
                let insert_idx = right_next - (right - left);
                let to_move: Vec<Tab> = self.tabs.drain(left..right).collect();
                self.tabs
                    .splice(insert_idx..insert_idx, to_move.into_iter());
                self.select_tab(insert_idx);
            }
        } else if !self.is_server_tab(self.active_idx + 1) {
            let tab = self.tabs.remove(self.active_idx);
            self.tabs.insert(self.active_idx + 1, tab);
            let active_idx = self.active_idx + 1;
            self.select_tab(active_idx);
        }
    }

    ////////////////////////////////////////////////////////////////////////////
    // Interfacing with tabs

    fn apply_to_target<F>(&mut self, target: &MsgTarget, f: &F)
    where
        F: Fn(&mut Tab, bool),
    {
        // Creating a vector just to make borrow checker happy. Borrow checker
        // sucks once more. Here it sucks 2x, I can't even create a Vec<&mut Tab>,
        // I need a Vec<usize>.
        //
        // (I could use an array on stack but whatever)
        let mut target_idxs: Vec<usize> = Vec::with_capacity(1);

        match *target {
            MsgTarget::Server { serv_name } => {
                for (tab_idx, tab) in self.tabs.iter().enumerate() {
                    if let MsgSource::Serv {
                        serv_name: ref serv_name_,
                    } = tab.src
                    {
                        if serv_name == serv_name_ {
                            target_idxs.push(tab_idx);
                            break;
                        }
                    }
                }
            }

            MsgTarget::Chan {
                serv_name,
                chan_name,
            } => {
                for (tab_idx, tab) in self.tabs.iter().enumerate() {
                    if let MsgSource::Chan {
                        serv_name: ref serv_name_,
                        chan_name: ref chan_name_,
                    } = tab.src
                    {
                        if serv_name == serv_name_ && chan_name == chan_name_ {
                            target_idxs.push(tab_idx);
                            break;
                        }
                    }
                }
            }

            MsgTarget::User { serv_name, nick } => {
                for (tab_idx, tab) in self.tabs.iter().enumerate() {
                    if let MsgSource::User {
                        serv_name: ref serv_name_,
                        nick: ref nick_,
                    } = tab.src
                    {
                        if serv_name == serv_name_ && nick == nick_ {
                            target_idxs.push(tab_idx);
                            break;
                        }
                    }
                }
            }

            MsgTarget::AllServTabs { serv_name } => {
                for (tab_idx, tab) in self.tabs.iter().enumerate() {
                    if tab.src.serv_name() == serv_name {
                        target_idxs.push(tab_idx);
                    }
                }
            }

            MsgTarget::AllUserTabs { serv_name, nick } => {
                for (tab_idx, tab) in self.tabs.iter().enumerate() {
                    match tab.src {
                        MsgSource::Serv { .. } => {}
                        MsgSource::Chan {
                            serv_name: ref serv_name_,
                            ..
                        } => {
                            if serv_name_ == serv_name && tab.widget.has_nick(nick) {
                                target_idxs.push(tab_idx);
                            }
                        }
                        MsgSource::User {
                            serv_name: ref serv_name_,
                            nick: ref nick_,
                        } => {
                            if serv_name_ == serv_name && nick_ == nick {
                                target_idxs.push(tab_idx);
                            }
                        }
                    }
                }
            }

            MsgTarget::CurrentTab => {
                target_idxs.push(self.active_idx);
            }
        }

        // Create server/chan/user tab when necessary
        if target_idxs.is_empty() {
            if let Some(idx) = self.maybe_create_tab(target) {
                target_idxs.push(idx);
            }
        }

        for tab_idx in target_idxs {
            f(&mut self.tabs[tab_idx], self.active_idx == tab_idx);
        }
    }

    fn maybe_create_tab(&mut self, target: &MsgTarget) -> Option<usize> {
        match *target {
            MsgTarget::Server { serv_name } | MsgTarget::AllServTabs { serv_name } => {
                self.new_server_tab(serv_name)
            }

            MsgTarget::Chan {
                serv_name,
                chan_name,
            } => self.new_chan_tab(serv_name, chan_name),

            MsgTarget::User { serv_name, nick } => self.new_user_tab(serv_name, nick),

            _ => None,
        }
    }

    pub fn set_tab_style(&mut self, style: TabStyle, target: &MsgTarget) {
        self.apply_to_target(target, &|tab: &mut Tab, is_active: bool| {
            if !is_active && tab.style < style {
                tab.set_style(style);
            }
        });
    }

    /// An error message coming from Tiny, probably because of a command error
    /// etc. Those are not timestamped and not logged.
    pub fn add_client_err_msg(&mut self, msg: &str, target: &MsgTarget) {
        self.apply_to_target(target, &|tab: &mut Tab, _| {
            tab.widget.add_client_err_msg(msg);
        });
    }

    /// A notify message coming from tiny, usually shows a response of a command
    /// e.g. "Notifications enabled".
    pub fn add_client_notify_msg(&mut self, msg: &str, target: &MsgTarget) {
        self.apply_to_target(target, &|tab: &mut Tab, _| {
            tab.widget.add_client_notify_msg(msg);
        });
    }

    /// A message from client, usually just to indidate progress, e.g.
    /// "Connecting...". Not timestamed and not logged.
    pub fn add_client_msg(&mut self, msg: &str, target: &MsgTarget) {
        self.apply_to_target(target, &|tab: &mut Tab, _| {
            tab.widget.add_client_msg(msg);
        });
    }

    /// privmsg is a message coming from a server or client. Shown with sender's
    /// nick/name and receive time and logged.
    pub fn add_privmsg(
        &mut self,
        sender: &str,
        msg: &str,
        ts: Timestamp,
        target: &MsgTarget,
        ctcp_action: bool,
    ) {
        self.apply_to_target(target, &|tab: &mut Tab, _| {
            tab.widget.add_privmsg(sender, msg, ts, false, ctcp_action);
            let nick = tab.widget.get_nick();
            if let Some(nick_) = nick {
                tab.notifier
                    .notify_privmsg(sender, msg, target, nick_, false);
            }
        });
    }

    /// Similar to `add_privmsg`, except the whole message is highlighted.
    pub fn add_privmsg_highlight(
        &mut self,
        sender: &str,
        msg: &str,
        ts: Timestamp,
        target: &MsgTarget,
        ctcp_action: bool,
    ) {
        self.apply_to_target(target, &|tab: &mut Tab, _| {
            tab.widget.add_privmsg(sender, msg, ts, true, ctcp_action);
            let nick = tab.widget.get_nick();
            if let Some(nick_) = nick {
                tab.notifier
                    .notify_privmsg(sender, msg, target, nick_, true);
            }
        });
    }

    /// A message without any explicit sender info. Useful for e.g. in server
    /// and debug log tabs. Timestamped and logged.
    pub fn add_msg(&mut self, msg: &str, ts: Timestamp, target: &MsgTarget) {
        self.apply_to_target(target, &|tab: &mut Tab, _| {
            tab.widget.add_msg(msg, ts);
        });
    }

    /// Error messages related with the protocol - e.g. can't join a channel,
    /// nickname is in use etc. Timestamped and logged.
    pub fn add_err_msg(&mut self, msg: &str, ts: Timestamp, target: &MsgTarget) {
        self.apply_to_target(target, &|tab: &mut Tab, _| {
            tab.widget.add_err_msg(msg, ts);
        });
    }

    pub fn show_topic(&mut self, title: &str, ts: Timestamp, target: &MsgTarget) {
        self.apply_to_target(target, &|tab: &mut Tab, _| {
            tab.widget.show_topic(title, ts);
        });
    }

    pub fn clear_nicks(&mut self, target: &MsgTarget) {
        self.apply_to_target(target, &|tab: &mut Tab, _| {
            tab.widget.clear_nicks();
        });
    }

    pub fn add_nick(&mut self, nick: &str, ts: Option<Timestamp>, target: &MsgTarget) {
        self.apply_to_target(target, &|tab: &mut Tab, _| {
            tab.widget.join(nick, ts);
        });
    }

    pub fn remove_nick(&mut self, nick: &str, ts: Option<Timestamp>, target: &MsgTarget) {
        self.apply_to_target(target, &|tab: &mut Tab, _| {
            tab.widget.part(nick, ts);
        });
    }

    pub fn rename_nick(
        &mut self,
        old_nick: &str,
        new_nick: &str,
        ts: Timestamp,
        target: &MsgTarget,
    ) {
        self.apply_to_target(target, &|tab: &mut Tab, _| {
            tab.widget.nick(old_nick, new_nick, ts);
            tab.update_source(&|src: &mut MsgSource| {
                if let MsgSource::User { ref mut nick, .. } = *src {
                    nick.clear();
                    nick.push_str(new_nick);
                }
            });
        });
    }

    pub fn set_nick(&mut self, serv_name: &str, new_nick: &str) {
        let target = MsgTarget::AllServTabs { serv_name };
        self.apply_to_target(&target, &|tab: &mut Tab, _| {
            tab.widget.set_nick(new_nick.to_owned())
        });
    }

    pub fn clear(&mut self, target: &MsgTarget) {
        self.apply_to_target(target, &|tab: &mut Tab, _| tab.widget.clear());
    }

    pub fn toggle_statusline(&mut self) {
        self.show_statusline = !self.show_statusline;
        let statusline_height = if self.statusline_visible && self.show_statusline { 1 } else { 0 };
        for tab in &mut self.tabs {
            tab.widget.resize(self.width, self.height - 1 - statusline_height);
        }
    }

    pub fn toggle_ignore(&mut self, target: &MsgTarget) {
        if let MsgTarget::AllServTabs { serv_name } = *target {
            let mut status_val: bool = false;
            for tab in &self.tabs {
                if let MsgSource::Serv {
                    serv_name: ref serv_name_,
                } = tab.src
                {
                    if serv_name == serv_name_ {
                        status_val = tab.widget.get_ignore_state();
                        break;
                    }
                }
            }
            self.apply_to_target(target, &|tab: &mut Tab, _| {
                tab.widget.set_or_toggle_ignore(Some(!status_val));
            });
        } else {
            self.apply_to_target(target, &|tab: &mut Tab, _| {
                tab.widget.set_or_toggle_ignore(None);
            });
        }
    }

    pub fn does_user_tab_exist(&self, serv_name_: &str, nick_: &str) -> bool {
        for tab in &self.tabs {
            if let MsgSource::User {
                ref serv_name,
                ref nick,
            } = tab.src
            {
                if serv_name_ == serv_name && nick_ == nick {
                    return true;
                }
            }
        }
        false
    }

    pub fn set_notifier(&mut self, notifier: Notifier, target: &MsgTarget) {
        self.apply_to_target(target, &|tab: &mut Tab, _| {
            tab.notifier = notifier;
        });
    }

    pub fn show_notify_mode(&mut self, target: &MsgTarget) {
        self.apply_to_target(target, &|tab: &mut Tab, _| {
            let msg = match tab.notifier {
                Notifier::Off => "Notifications are off",
                Notifier::Mentions => "Notifications enabled for mentions",
                Notifier::Messages => "Notifications enabled for all messages",
            };
            tab.widget.add_client_notify_msg(msg);
        });
    }

    ////////////////////////////////////////////////////////////////////////////
    // Helpers

    fn find_serv_tab_idx(&self, serv_name_: &str) -> Option<usize> {
        for (tab_idx, tab) in self.tabs.iter().enumerate() {
            if let MsgSource::Serv { ref serv_name } = tab.src {
                if serv_name_ == serv_name {
                    return Some(tab_idx);
                }
            }
        }
        None
    }

    fn find_chan_tab_idx(&self, serv_name_: &str, chan_name_: &str) -> Option<usize> {
        for (tab_idx, tab) in self.tabs.iter().enumerate() {
            if let MsgSource::Chan {
                ref serv_name,
                ref chan_name,
            } = tab.src
            {
                if serv_name_ == serv_name && chan_name_ == chan_name {
                    return Some(tab_idx);
                }
            }
        }
        None
    }

    fn find_user_tab_idx(&self, serv_name_: &str, nick_: &str) -> Option<usize> {
        for (tab_idx, tab) in self.tabs.iter().enumerate() {
            if let MsgSource::User {
                ref serv_name,
                ref nick,
            } = tab.src
            {
                if serv_name_ == serv_name && nick_ == nick {
                    return Some(tab_idx);
                }
            }
        }
        None
    }

    /// Index of the last tab with the given server name.
    fn find_last_serv_tab_idx(&self, serv_name: &str) -> Option<usize> {
        for (tab_idx, tab) in self.tabs.iter().enumerate().rev() {
            if tab.src.serv_name() == serv_name {
                return Some(tab_idx);
            }
        }
        None
    }

    fn is_server_tab(&self, idx: usize) -> bool {
        match self.tabs[idx].src {
            MsgSource::Serv { .. } => true,
            MsgSource::Chan { .. } | MsgSource::User { .. } => false,
        }
    }

    /// Given a tab index return range of tabs for the server of this tab.
    fn server_tab_range(&self, idx: usize) -> (usize, usize) {
        debug_assert!(idx < self.tabs.len());
        let mut left = idx;
        while !self.is_server_tab(left) {
            left -= 1;
        }
        let mut right = idx + 1;
        while right < self.tabs.len() && !self.is_server_tab(right) {
            right += 1;
        }
        (left, right)
    }
}

#[derive(Debug)]
pub enum PasteError {
    Io(::std::io::Error),
    Var(::std::env::VarError),
    /// One of the lines looks like a command. This is something we don't support yet.
    PastedCmd,
}

impl From<::std::io::Error> for PasteError {
    fn from(err: ::std::io::Error) -> PasteError {
        PasteError::Io(err)
    }
}

impl From<::std::env::VarError> for PasteError {
    fn from(err: ::std::env::VarError) -> PasteError {
        PasteError::Var(err)
    }
}

/// The user tried to paste the multi-line string passed as the argument. Run $EDITOR to edit a
/// temporary file with the string as the contents. On exit, parse the final contents of the file
/// (ignore comment lines), and send each line in the file as a message. Abort if any of the lines
/// look like a command (e.g. `/msg ...`). I don't know what's the best way to handle commands in
/// this context.
///
/// Ok(str) => final string to send
/// Err(str) => err message to show
///
/// FIXME: Ideally this function should get a `Termbox` argument and return a new `Termbox` because
/// we shutdown the current termbox instance and initialize it again after running $EDITOR.
pub fn paste_lines(tf: String, str: &str) -> Result<Vec<String>, PasteError> {
    use std::io::Read;
    use std::io::Write;
    use std::io::{Seek, SeekFrom};
    use std::process::Command;

    use termbox_simple::*;

    let editor = ::std::env::var("EDITOR")?;
    let mut tmp_file = ::tempfile::NamedTempFile::new()?;

    writeln!(
        tmp_file,
        "\
         # You pasted a multi-line message. When you close the editor final version of\n\
         # this file will be sent (ignoring these lines). Delete contents to abort the\n\
         # paste."
    )?;
    write!(tmp_file, "{}", tf)?;
    write!(tmp_file, "{}", str.replace('\r', "\n"))?;

    unsafe {
        tb_shutdown();
    }
    let ret = Command::new(editor).arg(tmp_file.path()).status();
    unsafe {
        tb_init();
    }
    // No need to set color mode etc. again as they're held in static variables and won't be reset
    // after tb_shutdown()

    let ret = ret?;
    if !ret.success() {
        return Ok(vec![]); // assume aborted
    }

    let mut tmp_file = tmp_file.into_file();
    tmp_file.seek(SeekFrom::Start(0))?;

    let mut file_contents = String::new();
    tmp_file.read_to_string(&mut file_contents)?;

    let mut filtered_lines = vec![];
    for s in file_contents.lines() {
        // Ignore if the char is '#'. To actually send a `#` add space.
        // For empty lines, send " ".
        let first_char = s.chars().next();
        if first_char == Some('#') {
            // skip this line
            continue;
        } else if s.is_empty() {
            filtered_lines.push(" ".to_owned());
        } else {
            filtered_lines.push(s.to_owned());
        }
    }

    Ok(filtered_lines)
}
