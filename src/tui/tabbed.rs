use rustbox::keyboard::Key;
use rustbox::{RustBox, Color};
use rustbox;

use tui::messaging::MessagingUI;
use tui::MsgTarget;
use tui::style::Style;
use tui::widget::{Widget, WidgetRet};

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

    pub fn new_server_tab(&mut self, serv_name : String) {
        match self.find_serv_tab_idx(&serv_name) {
            None => {
                self.tabs.push(Tab {
                    widget: MessagingUI::new(self.width, self.height - 1),
                    src: MsgSource::Serv { serv_name: serv_name },
                });
                self.active_idx = Some(self.tabs.len() - 1);
            },
            Some(tab_idx) => {
                self.active_idx = Some(tab_idx);
            }
        }
    }

    pub fn new_chan_tab(&mut self, serv_name : String, chan_name : String) {
        match self.find_last_serv_tab_idx(&serv_name) {
            None => {
                self.new_server_tab(serv_name.clone());
                self.new_chan_tab(serv_name, chan_name);
            },
            Some(tab_idx) => {
                self.tabs.insert(tab_idx + 1, Tab {
                    widget: MessagingUI::new(self.width, self.height - 1),
                    src: MsgSource::Chan { serv_name: serv_name, chan_name: chan_name },
                });
                self.active_idx = Some(self.tabs.len() - 1);
            }
        }
    }

    pub fn new_user_tab(&mut self, serv_name : String, nick : String) {
        match self.find_last_serv_tab_idx(&serv_name) {
            None => {
                self.new_server_tab(serv_name.clone());
                self.new_user_tab(serv_name, nick);
            },
            Some(tab_idx) => {
                self.tabs.insert(tab_idx + 1, Tab {
                    widget: MessagingUI::new(self.width, self.height - 1),
                    src: MsgSource::User { serv_name: serv_name, nick: nick },
                });
            }
        }
    }

    pub fn draw(&self, rustbox : &RustBox, pos_x : i32, pos_y : i32) {
        match self.active_idx {
            None => {},
            Some(idx) => self.tabs[idx as usize].widget.draw(rustbox, pos_x, pos_y),
        }

        let mut tab_name_col = 0;
        for (tab_idx, tab) in self.tabs.iter().enumerate() {
            if self.active_idx == Some(tab_idx) {
                rustbox.print(tab_name_col, (self.height as usize) - 1,
                              rustbox::RB_BOLD, Color::White, Color::Blue, tab.visible_name());
            } else {
                rustbox.print(tab_name_col, (self.height as usize) - 1,
                              rustbox::RB_BOLD, Color::White, Color::Default, tab.visible_name());
            }
            // len() is OK since sever, chan and nick names are ascii
            tab_name_col += tab.visible_name().len();
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
        for tab in self.tabs.iter_mut() {
            // TODO: Widgets should resize themselves lazily
            tab.widget.resize(width, height);
        }
    }

    ////////////////////////////////////////////////////////////////////////////
    // Interfacing with tabs

    #[inline]
    pub fn add_msg(&mut self, msg : &str, target : &MsgTarget, style : Style) {
        match target {
            &MsgTarget::Server { serv_name } =>  {
                for tab in self.tabs.iter_mut() {
                    match &tab.src {
                        &MsgSource::Serv { serv_name: ref serv_name_ } => {
                            if serv_name == serv_name_ {
                                tab.widget.add_msg(msg, style);
                                return;
                            }
                        },
                        _ => {}
                    }
                }
                panic!("Can't add msg {} to {:?}", msg, target);
            },

            &MsgTarget::Chan { serv_name, chan_name } => {
                for tab in self.tabs.iter_mut() {
                    match &tab.src {
                        &MsgSource::Chan { serv_name: ref serv_name_, chan_name: ref chan_name_ } => {
                            if serv_name == serv_name_ && chan_name == chan_name_ {
                                tab.widget.add_msg(msg, style);
                                return;
                            }
                        },
                        _ => {}
                    }
                }
                panic!("Can't add msg {} to {:?}", msg, target);
            },

            &MsgTarget::User { serv_name, nick } => {
                for tab in self.tabs.iter_mut() {
                    match &tab.src {
                        &MsgSource::User { serv_name: ref serv_name_, nick: ref nick_ } => {
                            if serv_name == serv_name_ && nick == nick_ {
                                tab.widget.add_msg(msg, style);
                                return;
                            }
                        },
                        _ => {}
                    }
                }
                panic!("Can't add msg {} to {:?}", msg, target);
            },

            &MsgTarget::AllServTabs { serv_name } => {
                for tab in self.tabs.iter_mut() {
                    if tab.src.serv_name() == serv_name {
                        tab.widget.add_msg(msg, style);
                    }
                }
            },

            &MsgTarget::AllTabs => {
                for tab in self.tabs.iter_mut() {
                    tab.widget.add_msg(msg, style);
                }
            },

            &MsgTarget::CurrentTab => {
                self.tabs[self.active_idx.unwrap()].widget.add_msg(msg, style);
            },

            &MsgTarget::MultipleTabs(ref targets) => {
                for target in targets.iter() {
                    self.add_msg(msg, target, style);
                }
            }
        }
    }

    pub fn add_msg_all_tabs(&mut self, msg : &str, style : Style) {
        for tab in self.tabs.iter_mut() {
            tab.widget.add_msg(msg, style);
        }
    }

    pub fn add_msg_current_tab(&mut self, msg : &str, style : Style) {
        self.tabs[self.active_idx.unwrap()].widget.add_msg(msg, style);
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
