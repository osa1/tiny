use rustbox::keyboard::Key;
use rustbox::{RustBox, Color};
use rustbox;

use msg::Pfx;
use tui::messaging::MessagingUI;
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
    serv_name : String,
    pfx       : Option<Pfx>,
    widget    : MessagingUI,
}

impl Tab {
    pub fn visible_name<'a>(&'a self) -> &'a str {
        match &self.pfx {
            &None => &self.serv_name,
            &Some(Pfx::Server(ref serv_name)) => serv_name,
            &Some(Pfx::User { ref nick, .. }) => nick,
            &Some(Pfx::Chan { ref chan_name }) => chan_name,
        }
    }
}

pub enum TabbedRet<'t> {
    KeyHandled,
    KeyIgnored,

    Input {
        serv_name : &'t str,
        pfx       : Option<&'t Pfx>,
        msg       : Vec<char>
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

    /// Create a tab if it doesn't exist, and make it current.
    pub fn new_tab(&mut self, serv_name : String, pfx : Pfx) {
        for (tab_idx, tab) in self.tabs.iter().enumerate() {
            if tab.serv_name == serv_name && tab.pfx.as_ref() == Some(&pfx) {
                self.active_idx = Some(tab_idx);
                return;
            }
        }

        // Do we have a tab for this server?
        match self.find_serv_last_tab_idx(&serv_name) {
            None => {
                // Well, we need a server tab too.
                self.new_server_tab(serv_name.clone());
                self.new_tab(serv_name, pfx)
            },
            Some(idx) => {
                self.tabs.insert(idx, Tab {
                    serv_name: serv_name,
                    pfx: Some(pfx),
                    widget: MessagingUI::new(self.width, self.height - 1)
                });
                self.active_idx = Some(idx);
            }
        }
    }

    /// Create a tab if it doesn't exist, and make it current.
    pub fn new_server_tab(&mut self, serv_name : String) {
        match self.find_serv_tab_idx(&serv_name) {
            None => {
                self.tabs.push(Tab {
                    serv_name: serv_name,
                    pfx: None,
                    widget: MessagingUI::new(self.width, self.height - 1)
                });
                self.active_idx = Some(self.tabs.len() - 1);
            },
            Some(idx) => {
                self.active_idx = Some(idx);
            }
        }
    }

    pub fn close_tab(&mut self, serv_name : &str, pfx : &Pfx) {
        if let Some(idx) = self.find_msg_tab_idx(serv_name, pfx) {
            self.tabs.remove(idx);
        } else {
            panic!("Tabbed.close_tab(): Trying to close a non-existent tab.")
        }
    }

    /// Closes all tabs with the given serv_name!
    pub fn close_serv_tab(&mut self, serv_name : &str) {
        if let Some(idx) = self.find_serv_tab_idx(serv_name) {
            let ends = self.find_serv_last_tab_idx(serv_name).unwrap();
            self.tabs.drain(idx .. ends);
        } else {
            panic!("Tabbed.close_tab(): Trying to close a non-existent tab.")
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
            // len() is OK since nick and chan names are ascii
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
                        let tab = &self.tabs[idx as usize];
                        TabbedRet::Input {
                            serv_name: &tab.serv_name,
                            pfx: tab.pfx.as_ref(),
                            msg: input,
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
    pub fn add_msg(&mut self, msg : &str, serv_name : &str, pfx : Option<&Pfx>, style : Style) {
        match pfx {
            None => {
                let serv_tab_idx = self.find_serv_tab_idx(serv_name).unwrap();
                self.tabs[serv_tab_idx].widget.add_msg(msg, style);
            },
            Some(ref pfx) => {
                let msg_tab_idx = self.find_msg_tab_idx(serv_name, pfx).unwrap();
                self.tabs[msg_tab_idx].widget.add_msg(msg, style);
            }
        }
    }

    /// Add a message to all tabs of a server.
    pub fn add_msg_all_serv_tabs(&mut self, msg : &str, serv_name : &str, style : Style) {
        for tab in self.tabs.iter_mut() {
            if tab.serv_name.as_str() == serv_name {
                tab.widget.add_msg(msg, style);
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

    #[inline]
    fn find_serv_tab_idx(&self, serv_name : &str) -> Option<usize> {
        self.find_tab_idx(serv_name, None)
    }

    /// Returns a position to insert() new tabs for the given serv_name.
    fn find_serv_last_tab_idx(&self, serv_name : &str) -> Option<usize> {
        for (tab_idx, tab) in self.tabs.iter().enumerate().rev() {
            if tab.serv_name == serv_name {
                return Some(tab_idx + 1);
            }
        }
        None
    }

    #[inline]
    fn find_msg_tab_idx(&self, serv_name : &str, pfx : &Pfx) -> Option<usize> {
        self.find_tab_idx(serv_name, Some(pfx))
    }

    fn find_tab_idx(&self, serv_name : &str, pfx : Option<&Pfx>) -> Option<usize> {
        for (tab_idx, tab) in self.tabs.iter().enumerate() {
            if tab.serv_name == serv_name && tab.pfx.as_ref() == pfx {
                return Some(tab_idx);
            }
        }
        None
    }
}
