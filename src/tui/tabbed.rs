use term_input::Key;
use termbox_simple::Termbox;

use tui::messaging::MessagingUI;
use tui::messaging::Timestamp;
use tui::MsgTarget;
use tui::style;
use tui::style::Style;
use tui::termbox;
use tui::widget::{WidgetRet};

use utils::opt_to_vec;

static LEFT_ARROW: char = '<';
static RIGHT_ARROW: char = '>';

// TODO: How to reorder tabs?
// TODO: How to report errors?

pub struct Tabbed {
    tabs: Vec<Tab>,
    active_idx: usize,
    width: i32,
    height: i32,
    h_scroll: i32,
}

struct Tab {
    widget: MessagingUI,
    src: MsgSource,
    style: TabStyle,
}

#[derive(Copy, Clone)]
pub enum TabStyle {
    Important,
    Highlight,
    Normal,
}

impl TabStyle {
    pub fn get_style(self) -> Style {
        match self {
            TabStyle::Important => style::TAB_IMPORTANT,
            TabStyle::Highlight => style::TAB_HIGHLIGHT,
            TabStyle::Normal => style::TAB_NORMAL,
        }
    }
}

/// TUI source of a message from the user.
#[derive(Debug, Clone)]
pub enum MsgSource {
    /// Message sent to a server tab.
    Serv { serv_name : String },

    /// Message sent to a channel tab.
    Chan { serv_name : String, chan_name : String },

    /// Message sent to a privmsg tab.
    User { serv_name : String, nick : String },
}

impl MsgSource {
    pub fn serv_name(&self) -> &str {
        match *self {
            MsgSource::Serv { ref serv_name } => serv_name,
            MsgSource::Chan { ref serv_name, .. } => serv_name,
            MsgSource::User { ref serv_name, .. } => serv_name,
        }
    }
}

impl Tab {
    pub fn visible_name(&self) -> &str {
        match self.src {
            MsgSource::Serv { ref serv_name, .. } => serv_name,
            MsgSource::Chan { ref chan_name, .. } => chan_name,
            MsgSource::User { ref nick, .. } => nick,
        }
    }

    pub fn set_style(&mut self, style: TabStyle) {
        self.style = style;
    }

    pub fn width(&self) -> i32 {
        // TODO: assuming ASCII string here. We should probably switch to a AsciiStr type.
        self.visible_name().len() as i32
    }

    pub fn draw(&self, tb: &mut Termbox, pos_x: i32, pos_y: i32, active: bool) {
        let style: Style = if active {
            style::TAB_ACTIVE
        } else {
            self.style.get_style()
        };

        termbox::print(tb, pos_x, pos_y, style.fg, style.bg, self.visible_name());
    }
}

pub enum TabbedRet<'t> {
    KeyHandled,
    KeyIgnored,

    Input {
        msg  : Vec<char>,
        from : &'t MsgSource,
    },

    Abort,
}

impl Tabbed {
    pub fn new(width : i32, height : i32) -> Tabbed {
        Tabbed {
            tabs: Vec::new(),
            active_idx: 0,
            width: width,
            height: height,
            h_scroll: 0,
        }
    }

    /// Returns index of the new tab if a new tab is created.
    pub fn new_server_tab(&mut self, serv_name : &str) -> Option<usize> {
        match self.find_serv_tab_idx(&serv_name) {
            None => {
                self.tabs.push(Tab {
                    widget: MessagingUI::new(self.width, self.height - 1),
                    src: MsgSource::Serv { serv_name: serv_name.to_owned() },
                    style: TabStyle::Normal,
                });
                let tab_idx = self.tabs.len() - 1;
                self.select_tab(tab_idx);
                Some(tab_idx)
            },
            Some(tab_idx) => {
                self.select_tab(tab_idx);
                None
            }
        }
    }

    /// Returns index of the new tab if a new tab is created.
    pub fn new_chan_tab(&mut self, serv_name : &str, chan_name : &str) -> Option<usize> {
        match self.find_chan_tab_idx(&serv_name, &chan_name) {
            None => {
                match self.find_last_serv_tab_idx(&serv_name) {
                    None => {
                        self.new_server_tab(serv_name);
                        self.new_chan_tab(serv_name, chan_name)
                    },
                    Some(tab_idx) => {
                        self.tabs.insert(tab_idx + 1, Tab {
                            widget: MessagingUI::new(self.width, self.height - 1),
                            src: MsgSource::Chan { serv_name: serv_name.to_owned(),
                                                   chan_name: chan_name.to_owned() },
                            style: TabStyle::Normal,
                        });
                        self.select_tab(tab_idx + 1);
                        Some(tab_idx + 1)
                    }
                }
            },
            Some(tab_idx) => {
                self.select_tab(tab_idx);
                None
            }
        }
    }

    /// Returns index of the new tab if a new tab is created.
    pub fn new_user_tab(&mut self, serv_name : &str, nick : &str) -> Option<usize> {
        match self.find_user_tab_idx(serv_name, nick) {
            None => {
                match self.find_last_serv_tab_idx(&serv_name) {
                    None => {
                        self.new_server_tab(serv_name);
                        self.new_user_tab(serv_name, nick)
                    },
                    Some(tab_idx) => {
                        self.tabs.insert(tab_idx + 1, Tab {
                            widget: MessagingUI::new(self.width, self.height - 1),
                            src: MsgSource::User { serv_name: serv_name.to_owned(),
                                                   nick: nick.to_owned() },
                            style: TabStyle::Normal,
                        });
                        self.select_tab(tab_idx + 1);
                        Some(tab_idx + 1)
                    }
                }
            },
            Some(tab_idx) => {
                self.select_tab(tab_idx);
                None
            }
        }
    }

    pub fn keypressed(&mut self, key : Key) -> TabbedRet {
        match self.tabs[self.active_idx].widget.keypressed(key) {
            WidgetRet::KeyHandled => TabbedRet::KeyHandled,
            WidgetRet::KeyIgnored => self.handle_keypress(key),
            WidgetRet::Input(input) => {
                TabbedRet::Input {
                    msg: input,
                    from: &self.tabs[self.active_idx].src
                }
            },
            WidgetRet::Remove => unimplemented!(),
            WidgetRet::Abort => TabbedRet::Abort,
        }
    }

    fn handle_keypress(&mut self, key : Key) -> TabbedRet {
        match key {
            Key::Ctrl('n') => {
                self.next_tab();
                TabbedRet::KeyHandled
            },

            Key::Ctrl('p') => {
                self.prev_tab();
                TabbedRet::KeyHandled
            },

            _ => TabbedRet::KeyIgnored,
        }
    }

    pub fn resize(&mut self, width : i32, height : i32) {
        self.width = width;
        self.height = height;
        for tab in self.tabs.iter_mut() {
            tab.widget.resize(width, height - 1);
        }
    }
}

////////////////////////////////////////////////////////////////////////////////
// Rendering

fn arrow_style(tabs: &[Tab]) -> Style {
    let mut arrow_style = style::TAB_NORMAL;

    for tab in tabs  {
        match tab.style {
            TabStyle::Important => {
                arrow_style = style::TAB_IMPORTANT;
                break;
            }
            TabStyle::Highlight => {
                arrow_style = style::TAB_HIGHLIGHT;
            }
            TabStyle::Normal => {}
        }
    }

    arrow_style
}

impl Tabbed {
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
            if self.draw_left_arrow() { width_left -= 2; }
            if self.draw_right_arrow() { width_left -= 2; }
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

    pub fn draw(&self, tb : &mut Termbox, mut pos_x: i32, pos_y: i32) {
        self.tabs[self.active_idx].widget.draw(tb, pos_x, pos_y);

        // decide whether we need to draw left/right arrows in tab bar
        let left_arr = self.draw_left_arrow();
        let right_arr = self.draw_right_arrow();

        let (tab_left, tab_right) = self.rendered_tabs();

        if left_arr {
            let style = arrow_style(&self.tabs[0..tab_left]);
            tb.change_cell(pos_x, pos_y + self.height - 1,
                           LEFT_ARROW,
                           style.fg, style.bg);
            pos_x += 2;
        }

        // Debugging
        // use std::io;
        // use std::io::Write;
        // writeln!(io::stderr(), "number of tabs to draw: {}", tab_right - tab_left).unwrap();
        // writeln!(io::stderr(), "left_arr: {}, right_arr: {}", left_arr, right_arr).unwrap();

        // finally draw the tabs
        for (tab_idx, tab) in (&self.tabs[tab_left .. tab_right]).iter().enumerate() {
            tab.draw(tb, pos_x, pos_y + self.height - 1, self.active_idx == tab_idx + tab_left);
            // len() is OK since server, chan and nick names are ascii
            pos_x += tab.visible_name().len() as i32 + 1; // +1 for margin
        }

        if right_arr {
            let style = arrow_style(&self.tabs[tab_right..]);
            tb.change_cell(pos_x, pos_y + self.height - 1,
                           RIGHT_ARROW,
                           style.fg, style.bg);
        }
    }

    ////////////////////////////////////////////////////////////////////////////
    // Moving between tabs, horizontal scroll updates

    fn select_tab(&mut self, tab_idx: usize) {
        self.active_idx = tab_idx;
    }

    fn next_tab(&mut self) {
        if self.active_idx == self.tabs.len() - 1 {
            self.active_idx = 0;
            self.h_scroll = 0;
        } else {
            // either the next tab is visible, or we should scroll so that the
            // next tab becomes visible
            let next_active = self.active_idx + 1;
            loop {
                let (tab_left, tab_right) = self.rendered_tabs();
                if next_active >= tab_left && next_active < tab_right {
                    break;
                }
                self.h_scroll += self.tabs[tab_left].width() + 1;
            }
            self.active_idx = next_active;
        }
    }

    fn prev_tab(&mut self) {
        if self.active_idx == 0 {
            let next_active = self.tabs.len() - 1;
            while self.active_idx != next_active {
                self.next_tab();
            }
        } else {
            let next_active = self.active_idx - 1;
            loop {
                let (tab_left, tab_right) = self.rendered_tabs();
                if next_active >= tab_left && next_active < tab_right {
                    break;
                }
                self.h_scroll -= self.tabs[tab_right - 1].width() + 1;
            }
            if self.h_scroll < 0 { self.h_scroll = 0 };
            self.active_idx = next_active;
        }
    }

    ////////////////////////////////////////////////////////////////////////////
    // Interfacing with tabs

    fn apply_to_target<F>(&mut self, target : &MsgTarget, f : &F)
            where F : Fn(&mut Tab) -> () {

        // Creating a vector just to make borrow checker happy. Borrow checker
        // sucks once more. Here it sucks 2x, I can't even create a Vec<&mut Tab>,
        // I need a Vec<usize>.
        //
        // (I could use an array on stack but whatever)
        let mut target_idxs : Vec<usize> = Vec::with_capacity(1);

        match *target {
            MsgTarget::Server { serv_name } => {
                for (tab_idx, tab) in self.tabs.iter().enumerate() {
                    match tab.src {
                        MsgSource::Serv { serv_name: ref serv_name_ } => {
                            if serv_name == serv_name_ {
                                target_idxs.push(tab_idx);
                                break;
                            }
                        },
                        _ => {}
                    }
                }
            },

            MsgTarget::Chan { serv_name, chan_name } => {
                for (tab_idx, tab) in self.tabs.iter().enumerate() {
                    match tab.src {
                        MsgSource::Chan { serv_name: ref serv_name_, chan_name: ref chan_name_ } => {
                            if serv_name == serv_name_ && chan_name == chan_name_ {
                                target_idxs.push(tab_idx);
                                break;
                            }
                        },
                        _ => {}
                    }
                }
            },

            MsgTarget::User { serv_name, nick } => {
                for (tab_idx, tab) in self.tabs.iter().enumerate() {
                    match tab.src {
                        MsgSource::User { serv_name: ref serv_name_, nick: ref nick_ } => {
                            if serv_name == serv_name_ && nick == nick_ {
                                target_idxs.push(tab_idx);
                                break;
                            }
                        },
                        _ => {}
                    }
                }
            },

            MsgTarget::AllServTabs { serv_name } => {
                for (tab_idx, tab) in self.tabs.iter().enumerate() {
                    if tab.src.serv_name() == serv_name {
                        target_idxs.push(tab_idx);
                    }
                }
            },

            MsgTarget::AllUserTabs { serv_name, nick } => {
                for (tab_idx, tab) in self.tabs.iter().enumerate() {
                    if tab.src.serv_name() == serv_name && tab.widget.has_nick(nick) {
                        target_idxs.push(tab_idx);
                    }
                }
            },

            MsgTarget::AllTabs => {
                for tab_idx in 0 .. self.tabs.len() {
                    target_idxs.push(tab_idx);
                }
            },

            MsgTarget::CurrentTab => {
                target_idxs.push(self.active_idx);
            },

            MsgTarget::MultipleTabs(ref targets) => {
                for target in targets.iter() {
                    self.apply_to_target(target, f);
                }
            }
        }

        // Create server/chan/user tab when necessary
        if target_idxs.len() == 0 {
            for idx in self.maybe_create_tab(target) {
                target_idxs.push(idx);
            }
        }

        for tab_idx in target_idxs {
            f(unsafe { self.tabs.get_unchecked_mut(tab_idx) });
        }
    }

    fn maybe_create_tab(&mut self, target : &MsgTarget) -> Vec<usize> {
        match *target {
            MsgTarget::Server { serv_name } => {
                opt_to_vec(self.new_server_tab(serv_name))
            },

            MsgTarget::Chan { serv_name, chan_name } => {
                opt_to_vec(self.new_chan_tab(serv_name, chan_name))
            },

            MsgTarget::User { serv_name, nick } => {
                opt_to_vec(self.new_user_tab(serv_name, nick))
            },

            MsgTarget::MultipleTabs(ref targets) => {
                targets.iter().flat_map(|t : &MsgTarget| self.maybe_create_tab(t)).collect()
            }

            _ => vec![]
        }
    }

    pub fn set_tab_style(&mut self, style: TabStyle, target: &MsgTarget) {
        self.apply_to_target(target, &|tab: &mut Tab| tab.set_style(style));
    }

    pub fn add_client_err_msg(&mut self, msg : &str, target : &MsgTarget) {
        self.apply_to_target(target, &|tab : &mut Tab| {
            tab.widget.add_client_err_msg(msg);
        });
    }

    pub fn add_client_msg(&mut self, msg : &str, target : &MsgTarget) {
        self.apply_to_target(target, &|tab : &mut Tab| {
            tab.widget.add_client_msg(msg);
        });
    }

    pub fn add_privmsg(&mut self, sender: &str, msg: &str, ts: Timestamp, target: &MsgTarget) {
        self.apply_to_target(target, &|tab : &mut Tab| {
            tab.widget.add_privmsg(sender, msg, ts);
        });
    }

    pub fn add_msg(&mut self, msg: &str, ts: Timestamp, target: &MsgTarget) {
        self.apply_to_target(target, &|tab : &mut Tab| {
            tab.widget.add_msg(msg, ts);
        });
    }

    pub fn add_err_msg(&mut self, msg: &str, ts: Timestamp, target: &MsgTarget) {
        self.apply_to_target(target, &|tab : &mut Tab| {
            tab.widget.add_err_msg(msg, ts);
        });
    }

    pub fn show_topic(&mut self, title: &str, ts: Timestamp, target: &MsgTarget) {
        self.apply_to_target(target, &|tab : &mut Tab| {
            tab.widget.show_topic(title, ts);
        });
    }

    pub fn add_nick(&mut self, nick: &str, ts: Option<Timestamp>, target: &MsgTarget) {
        self.apply_to_target(target, &|tab : &mut Tab| {
            tab.widget.join(nick, ts);
        });
    }

    pub fn remove_nick(&mut self, nick: &str, ts: Option<Timestamp>, target: &MsgTarget) {
        self.apply_to_target(target, &|tab : &mut Tab| {
            tab.widget.part(nick, ts);
        });
    }

    pub fn rename_nick(&mut self, old_nick: &str, new_nick: &str, ts: Timestamp, target: &MsgTarget) {
        self.apply_to_target(target, &|tab : &mut Tab| {
            tab.widget.nick(old_nick, new_nick, ts);
        });
    }

    ////////////////////////////////////////////////////////////////////////////
    // Helpers

    fn find_serv_tab_idx(&self, serv_name_ : &str) -> Option<usize> {
        for (tab_idx, tab) in self.tabs.iter().enumerate() {
            match tab.src {
                MsgSource::Serv { ref serv_name } => {
                    if serv_name_ == serv_name {
                        return Some(tab_idx);
                    }
                },
                _ => {},
            }
        }
        None
    }

    fn find_chan_tab_idx(&self, serv_name_ : &str, chan_name_ : &str) -> Option<usize> {
        for (tab_idx, tab) in self.tabs.iter().enumerate() {
            match tab.src {
                MsgSource::Chan { ref serv_name, ref chan_name } => {
                    if serv_name_ == serv_name && chan_name_ == chan_name {
                        return Some(tab_idx);
                    }
                },
                _ => {},
            }
        }
        None
    }

    fn find_user_tab_idx(&self, serv_name_ : &str, nick_ : &str) -> Option<usize> {
        for (tab_idx, tab) in self.tabs.iter().enumerate() {
            match tab.src {
                MsgSource::User { ref serv_name, ref nick } => {
                    if serv_name_ == serv_name && nick_ == nick {
                        return Some(tab_idx);
                    }
                },
                _ => {},
            }
        }
        None
    }

    /// Index of the last tab with the given server name.
    fn find_last_serv_tab_idx(&self, serv_name : &str) -> Option<usize> {
        for (tab_idx, tab) in self.tabs.iter().enumerate().rev() {
            if tab.src.serv_name() == serv_name {
                return Some(tab_idx);
            }
        }
        None
    }
}
