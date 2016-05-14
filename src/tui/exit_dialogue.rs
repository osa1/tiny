use rustbox::{RustBox, Key};

use trie::Trie;
use tui::style;
use tui::termbox;
use tui::widget::{WidgetRet, Widget};

pub struct ExitDialogue {
    width : i32,
}

impl ExitDialogue {
    pub fn new(width : i32) -> ExitDialogue {
        ExitDialogue {
            width: width,
        }
    }
}

static MSG : &'static str = "Really quit?";

impl Widget for ExitDialogue {
    fn resize(&mut self, width : i32, _ : i32) {
        self.width = width;
    }

    fn draw(&self, _ : &RustBox, pos_x : i32, pos_y : i32) {
        termbox::hide_cursor();

        let mut col = 0;
        for char in MSG.chars() {
            termbox::print_char(pos_x + col, pos_y, style::YELLOW.fg, style::YELLOW.bg, char);
            col += 1;
        }

        while col < self.width {
            termbox::print_char(pos_x + col, pos_y, style::YELLOW.fg, style::YELLOW.bg, ' ');
            col += 1;
        }
    }

    fn keypressed(&mut self, key : Key) -> WidgetRet {
        match key {
            Key::Char('y') | Key::Enter => WidgetRet::Abort,
            _ => WidgetRet::Remove,
        }
    }

    fn autocomplete(&mut self, _ : &Trie) {}
}
