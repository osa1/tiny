use term_input::Key;
use termbox_simple::Termbox;

use config;
use tui::widget::{WidgetRet, Widget};

use std::any::Any;

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

    fn draw(&self, tb : &mut Termbox, pos_x : i32, pos_y : i32) {
        tb.hide_cursor();

        let mut col = 0;
        for char in MSG.chars() {
            tb.change_cell(pos_x + col, pos_y, char, config::get_theme().exit_dialogue.fg, config::get_theme().exit_dialogue.bg);
            col += 1;
        }

        while col < self.width {
            tb.change_cell(pos_x + col, pos_y, ' ', config::get_theme().exit_dialogue.fg, config::get_theme().exit_dialogue.bg);
            col += 1;
        }
    }

    fn keypressed(&mut self, key : Key) -> WidgetRet {
        match key {
            Key::Char('y') | Key::Enter => WidgetRet::Abort,
            _ => WidgetRet::Remove,
        }
    }

    fn event(&mut self, _: Box<Any>) -> WidgetRet {
        WidgetRet::KeyIgnored
    }
}
