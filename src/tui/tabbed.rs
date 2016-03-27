use std::borrow::Borrow;

use rustbox::keyboard::Key;
use rustbox::RustBox;

use tui::widget::{Widget, WidgetRet};

// TODO: How to reorder tabs?

pub struct Tabbed {
    tabs       : Vec<Tab>,
    active_idx : Option<i32>,
}

struct Tab {
    name   : String,
    widget : Box<Widget>,
}

pub enum TabbedRet<'t> {
    KeyHandled,
    KeyIgnored,
    Input(&'t str, Vec<char>),
}

impl Tabbed {
    fn new() -> Tabbed {
        Tabbed {
            tabs: Vec::new(),
            active_idx: None,
        }
    }

    pub fn new_tab(&mut self, tab_name : String, widget : Box<Widget>) {
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

    pub fn close_tab(&mut self, tab_name : &str) -> Option<Box<Widget>> {
        let tab_idx : Option<usize> =
            self.tabs.iter().enumerate().find(|t| t.1.name.as_str() == tab_name).map(|t| t.0);

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
            tab.widget.resize(width, height);
        }
    }
}
