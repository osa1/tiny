use config::Colors;
use term_input::Key;
use termbox_simple::Termbox;

use std::any::Any;

pub enum WidgetRet {
    /// Key is handled by the widget.
    KeyHandled,

    /// Key is ignored by the widget.
    KeyIgnored,

    /// An input is submitted.
    Input(Vec<char>),

    /// Remove the widget. E.g. close the tab, hide the dialogue etc.
    Remove,

    /// An exit event happened.
    Abort,
}

pub trait Widget {
    fn resize(&mut self, width: i32, height: i32);
    fn draw(&self, tb: &mut Termbox, colors: &Colors, pos_x: i32, pos_y: i32);
    fn keypressed(&mut self, key: Key) -> WidgetRet;
    fn event(&mut self, ev: Box<Any>) -> WidgetRet;
}

// Not sure if this Impl is a good idea -- a stack of widgets is a widget.
impl Widget for Vec<Box<Widget>> {
    fn resize(&mut self, width: i32, height: i32) {
        for widget in self {
            widget.resize(width, height);
        }
    }

    fn draw(&self, tb: &mut Termbox, colors: &Colors, pos_x: i32, pos_y: i32) {
        for widget in self {
            widget.draw(tb, colors, pos_x, pos_y);
        }
    }

    fn keypressed(&mut self, key: Key) -> WidgetRet {
        if !self.is_empty() {
            let i = self.len() - 1;
            self[i].keypressed(key)
        } else {
            WidgetRet::KeyIgnored
        }
    }

    fn event(&mut self, ev: Box<Any>) -> WidgetRet {
        if !self.is_empty() {
            let i = self.len() - 1;
            return self[i].event(ev);
        }
        WidgetRet::KeyIgnored
    }
}
