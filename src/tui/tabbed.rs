use std::borrow::Borrow;

use rustbox::keyboard::Key;
use rustbox::RustBox;

use tui::messaging::MessagingUI;
use tui::widget::{Widget, WidgetRet};

// TODO: How to reorder tabs?
// TODO: How to report errors?

pub struct Tabbed {
    tabs       : Vec<Tab>,
    active_idx : Option<i32>,
    width      : i32,
    height     : i32,
}

struct Tab {
    name   : String,
    widget : MessagingUI,
}

pub enum TabbedRet<'t> {
    KeyHandled,
    KeyIgnored,
    Input(&'t str, Vec<char>),
}

impl Tabbed {
    pub fn new(width : i32, height : i32,) -> Tabbed {
        Tabbed {
            tabs: Vec::new(),
            active_idx: None,
            width: width,
            height: height,
        }
    }

    pub fn new_tab(&mut self, tab_name : String, widget : MessagingUI) {
        match self.active_idx {
            None => {
                self.tabs.push(Tab {
                    name: tab_name,
                    widget: widget,
                });
                self.active_idx = Some((self.tabs.len() as i32) - 1);
            },
            Some(idx) => {
                self.tabs.insert((idx + 1) as usize, Tab {
                    name: tab_name,
                    widget: widget,
                });
                self.active_idx = Some(idx + 1);
            }
        }
    }

    pub fn close_tab(&mut self, tab_name : &str) -> Option<MessagingUI> {
        let tab_idx : Option<usize> = self.find_tab_idx(tab_name);
        tab_idx.map(|tab_idx| self.tabs.remove(tab_idx).widget)
    }

    pub fn draw(&self, rustbox : &RustBox, pos_x : i32, pos_y : i32) {
        match self.active_idx {
            None => {},
            Some(idx) => self.tabs[idx as usize].widget.draw(rustbox, pos_x, pos_y),
        }
    }

    pub fn keypressed(&mut self, key : Key) -> TabbedRet {
        match self.active_idx {
            None => TabbedRet::KeyIgnored,
            Some(idx) => {
                match self.tabs[idx as usize].widget.keypressed(key) {
                    WidgetRet::KeyHandled => TabbedRet::KeyHandled,
                    WidgetRet::KeyIgnored => TabbedRet::KeyIgnored,
                    WidgetRet::Input(input) =>
                        TabbedRet::Input(self.tabs[idx as usize].name.borrow(), input),
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
    fn find_tab_mut(&mut self, pfx : &str) -> Option<&mut Tab> {
        self.tabs.iter_mut().find(|t| t.name.as_str() == pfx)
    }

    #[inline]
    fn find_tab_idx(&self, pfx : &str) -> Option<usize> {
        self.tabs.iter().enumerate().find(|t| t.1.name.as_str() == pfx).map(|t| t.0)
    }

    pub fn show_incoming_msg(&mut self, pfx : &str, ty : &str, msg : &str) {
        // We need a *mut here instead of &mut because Rust suck. Basically if
        // this value is a None, an Option<&mut Tab> still has a reference to
        // &mut self so we can't call any methods.
        let tab : Option<*mut Tab> = self.find_tab_mut(pfx).map(|t| (t as *mut _));
        match tab {
            None => {
                let width = self.width;
                let height = self.height;
                self.new_tab(pfx.to_owned(), MessagingUI::new(width, height));
                self.show_incoming_msg(pfx, ty, msg);
            },
            Some(p) => {
                unsafe { (*p).widget.show_incoming_msg(ty, msg) };
            }
        }
    }
}
