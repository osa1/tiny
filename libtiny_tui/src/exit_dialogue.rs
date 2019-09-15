use term_input::Key;
use termbox_simple::Termbox;

use crate::{config::Colors, widget::WidgetRet};

pub(crate) struct ExitDialogue {
    width: i32,
}

impl ExitDialogue {
    pub(crate) fn new(width: i32) -> ExitDialogue {
        ExitDialogue { width }
    }
}

static MSG: &str = "Really quit?";

impl ExitDialogue {
    pub(crate) fn resize(&mut self, width: i32) {
        self.width = width;
    }

    pub(crate) fn draw(&self, tb: &mut Termbox, colors: &Colors, pos_x: i32, pos_y: i32) {
        tb.hide_cursor();

        let mut col = 0;
        for char in MSG.chars() {
            tb.change_cell(
                pos_x + col,
                pos_y,
                char,
                colors.exit_dialogue.fg,
                colors.exit_dialogue.bg,
            );
            col += 1;
        }

        while col < self.width {
            tb.change_cell(
                pos_x + col,
                pos_y,
                ' ',
                colors.exit_dialogue.fg,
                colors.exit_dialogue.bg,
            );
            col += 1;
        }
    }

    pub(crate) fn keypressed(&self, key: Key) -> WidgetRet {
        match key {
            Key::Char('y') | Key::Char('\r') => WidgetRet::Abort,
            _ => WidgetRet::Remove,
        }
    }
}
