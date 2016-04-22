use rustbox::keyboard::Key;
use rustbox::{RustBox};
use time::Tm;

use tui::messaging::MessagingUI;
use tui::MsgTarget;
use tui::style;
use tui::termbox;
use tui::widget::{WidgetRet};

use utils::opt_to_vec;

// TODO: How to reorder tabs?
// TODO: How to report errors?

pub struct Tabbed {
    tabs       : Vec<Tab>,
    active_idx : Option<usize>,
    width      : i32,
    height     : i32,
}

struct Tab {
    widget : MessagingUI,
    src    : MsgSource,
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
    pub fn serv_name<'a>(&'a self) -> &'a str {
        match self {
            &MsgSource::Serv { ref serv_name } => serv_name,
            &MsgSource::Chan { ref serv_name, .. } => serv_name,
            &MsgSource::User { ref serv_name, .. } => serv_name,
        }
    }
}

impl Tab {
    pub fn visible_name<'a>(&'a self) -> &'a str {
        match &self.src {
            &MsgSource::Serv { ref serv_name, .. } => serv_name,
            &MsgSource::Chan { ref chan_name, .. } => chan_name,
            &MsgSource::User { ref nick, .. } => nick,
        }
    }
}

pub enum TabbedRet<'t> {
    KeyHandled,
    KeyIgnored,

    Input {
        msg  : Vec<char>,
        from : &'t MsgSource,
    },
}

impl Tabbed {
    pub fn new(width : i32, height : i32) -> Tabbed {
        Tabbed {
            tabs: Vec::new(),
            active_idx: None,
            width: width,
            height: height,
        }
    }

    /// Returns index of the new tab if a new tab is created.
    pub fn new_server_tab(&mut self, serv_name : &str) -> Option<usize> {
        match self.find_serv_tab_idx(&serv_name) {
            None => {
                self.tabs.push(Tab {
                    widget: MessagingUI::new(self.width, self.height - 1),
                    src: MsgSource::Serv { serv_name: serv_name.to_owned() },
                });
                self.active_idx = Some(self.tabs.len() - 1);
                Some(self.tabs.len() - 1)
            },
            Some(tab_idx) => {
                self.active_idx = Some(tab_idx);
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
                        });
                        self.active_idx = Some(tab_idx + 1);
                        Some(tab_idx + 1)
                    }
                }
            },
            Some(tab_idx) => {
                self.active_idx = Some(tab_idx);
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
                        });
                        self.active_idx = Some(tab_idx + 1);
                        Some(tab_idx + 1)
                    }
                }
            },
            Some(tab_idx) => {
                self.active_idx = Some(tab_idx);
                None
            }
        }
    }

    pub fn draw(&self, rustbox : &RustBox, pos_x : i32, pos_y : i32) {
        match self.active_idx {
            None => {},
            Some(idx) => self.tabs[idx as usize].widget.draw(rustbox, pos_x, pos_y),
        }

        let mut tab_name_col = pos_x;
        for (tab_idx, tab) in self.tabs.iter().enumerate() {
            if self.active_idx == Some(tab_idx) {
                termbox::print(tab_name_col, pos_y + self.height - 1,
                               style::TAB_ACTIVE.fg, style::TAB_ACTIVE.bg,
                               tab.visible_name());
            } else {
                termbox::print(tab_name_col, pos_y + self.height - 1,
                               style::TAB_PASSIVE.fg, style::TAB_PASSIVE.bg,
                               tab.visible_name());
            }
            // len() is OK since sever, chan and nick names are ascii
            tab_name_col += tab.visible_name().len() as i32;
        }
    }

    pub fn keypressed(&mut self, key : Key) -> TabbedRet {
        if key == Key::Tab {
            if self.tabs.len() > 0 {
                self.active_idx = Some((self.active_idx.unwrap() + 1) % self.tabs.len());
            }
            return TabbedRet::KeyHandled;
        }

        match self.active_idx {
            None => TabbedRet::KeyIgnored,
            Some(idx) => {
                match self.tabs[idx as usize].widget.keypressed(key) {
                    WidgetRet::KeyHandled => TabbedRet::KeyHandled,
                    WidgetRet::KeyIgnored => TabbedRet::KeyIgnored,
                    WidgetRet::Input(input) => {
                        TabbedRet::Input {
                            msg: input,
                            from: &self.tabs[idx as usize].src
                        }
                    },
                }
            }
        }
    }

    pub fn resize(&mut self, width : i32, height : i32) {
        self.width = width;
        self.height = height;
        for tab in self.tabs.iter_mut() {
            tab.widget.resize(width, height - 1);
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

        match target {
            &MsgTarget::Server { serv_name } => {
                for (tab_idx, tab) in self.tabs.iter().enumerate() {
                    match &tab.src {
                        &MsgSource::Serv { serv_name: ref serv_name_ } => {
                            if serv_name == serv_name_ {
                                target_idxs.push(tab_idx);
                                break;
                            }
                        },
                        _ => {}
                    }
                }
            },

            &MsgTarget::Chan { serv_name, chan_name } => {
                for (tab_idx, tab) in self.tabs.iter().enumerate() {
                    match &tab.src {
                        &MsgSource::Chan { serv_name: ref serv_name_, chan_name: ref chan_name_ } => {
                            if serv_name == serv_name_ && chan_name == chan_name_ {
                                target_idxs.push(tab_idx);
                                break;
                            }
                        },
                        _ => {}
                    }
                }
            },

            &MsgTarget::User { serv_name, nick } => {
                for (tab_idx, tab) in self.tabs.iter().enumerate() {
                    match &tab.src {
                        &MsgSource::User { serv_name: ref serv_name_, nick: ref nick_ } => {
                            if serv_name == serv_name_ && nick == nick_ {
                                target_idxs.push(tab_idx);
                                break;
                            }
                        },
                        _ => {}
                    }
                }
            },

            &MsgTarget::AllServTabs { serv_name } => {
                for (tab_idx, tab) in self.tabs.iter().enumerate() {
                    if tab.src.serv_name() == serv_name {
                        target_idxs.push(tab_idx);
                    }
                }
            },

            &MsgTarget::AllTabs => {
                for tab_idx in 0 .. self.tabs.len() {
                    target_idxs.push(tab_idx);
                }
            },

            &MsgTarget::CurrentTab => {
                target_idxs.push(self.active_idx.unwrap());
            },

            &MsgTarget::MultipleTabs(ref targets) => {
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
        match target {
            &MsgTarget::Server { serv_name } => {
                opt_to_vec(self.new_server_tab(serv_name))
            },

            &MsgTarget::Chan { serv_name, chan_name } => {
                opt_to_vec(self.new_chan_tab(serv_name, chan_name))
            },

            &MsgTarget::User { serv_name, nick } => {
                opt_to_vec(self.new_user_tab(serv_name, nick))
            },

            &MsgTarget::MultipleTabs(ref targets) => {
                targets.iter().flat_map(|t : &Box<MsgTarget>| self.maybe_create_tab(&*t)).collect()
            }

            _ => vec![]
        }
    }

    #[inline]
    pub fn add_client_err_msg(&mut self, msg : &str, target : &MsgTarget) {
        self.apply_to_target(target, &|tab : &mut Tab| {
            tab.widget.add_client_err_msg(msg);
        });
    }

    #[inline]
    pub fn add_client_msg(&mut self, msg : &str, target : &MsgTarget) {
        self.apply_to_target(target, &|tab : &mut Tab| {
            tab.widget.add_client_msg(msg);
        });
    }

    #[inline]
    pub fn add_privmsg(&mut self, sender : &str, msg : &str, tm : &Tm, target : &MsgTarget) {
        self.apply_to_target(target, &|tab : &mut Tab| {
            tab.widget.add_privmsg(sender, msg, tm);
        });
    }

    #[inline]
    pub fn add_msg(&mut self, msg : &str, tm : &Tm, target : &MsgTarget) {
        self.apply_to_target(target, &|tab : &mut Tab| {
            tab.widget.add_msg(msg, tm);
        });
    }

    #[inline]
    pub fn add_err_msg(&mut self, msg : &str, tm : &Tm, target : &MsgTarget) {
        self.apply_to_target(target, &|tab : &mut Tab| {
            tab.widget.add_err_msg(msg, tm);
        });
    }

    #[inline]
    pub fn set_topic(&mut self, title : &str, target : &MsgTarget) {
        self.apply_to_target(target, &|tab : &mut Tab| {
            tab.widget.set_topic(title.to_owned());
        });
    }

    ////////////////////////////////////////////////////////////////////////////
    // Helpers

    fn find_serv_tab_idx(&self, serv_name_ : &str) -> Option<usize> {
        for (tab_idx, tab) in self.tabs.iter().enumerate() {
            match &tab.src {
                &MsgSource::Serv { ref serv_name } => {
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
            match &tab.src {
                &MsgSource::Chan { ref serv_name, ref chan_name } => {
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
            match &tab.src {
                &MsgSource::User { ref serv_name, ref nick } => {
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
