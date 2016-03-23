use std::borrow::Borrow;

use rustbox::{RustBox, Style, Color};
use rustbox::keyboard::Key;

pub struct TextField {
    buffer : String,
    cursor : i32,
}

pub enum TextFieldRet {
    SendMsg,
    Nothing,
}

impl TextField {
    pub fn new() -> TextField {
        TextField {
            buffer: String::with_capacity(512),
            cursor: 0,
        }
    }

    pub fn get_msg(&self) -> &String {
        &self.buffer
    }

    pub fn draw(&self, rustbox : &RustBox, pos_x : i32, pos_y : i32, width : i32, height : i32) {
        // draw text
        rustbox.print(pos_x as usize, pos_y as usize,
                      Style::all(), Color::White, Color::Default, self.buffer.borrow());
        // draw cursor
        rustbox.print_char(pos_x as usize + self.buffer.len(), pos_y as usize,
                           Style::all(), Color::Green, Color::Default, ' ');
    }

    pub fn keypressed(&mut self, key : Key) -> TextFieldRet {
        TextFieldRet::Nothing
    }
}
