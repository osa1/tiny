use std::borrow::Borrow;
use std::cmp::{max, min};

use rustbox::{RustBox, Style, Color};
use rustbox::keyboard::Key;

// TODO: Make this a setting
static SCROLLOFF : i32 = 5;

pub struct TextField {
    buffer : Vec<char>,
    cursor : i32,

    /// Horizontal scroll
    scroll : i32,

    width : i32,
}

pub enum TextFieldRet {
    SendMsg,
    KeyHandled,
    KeyIgnored,
}

impl TextField {
    pub fn new(width : i32) -> TextField {
        TextField {
            buffer: Vec::with_capacity(512),
            cursor: 0,
            scroll: 0,
            width: width,
        }
    }

    pub fn get_msg(&self) -> &Vec<char> {
        &self.buffer
    }

    pub fn clear_buffer(&mut self) {
        self.buffer.clear();
    }

    pub fn draw(&self, rustbox : &RustBox, pos_x : i32, pos_y : i32) {
        // draw text
        let buffer_borrow : &[char] = self.buffer.borrow();

        let slice : &[char] =
            &buffer_borrow[ self.scroll as usize ..
                            min(self.buffer.len(), (self.scroll + self.width) as usize) ];

        let string : String = slice.iter().cloned().collect();

        rustbox.print(pos_x as usize, pos_y as usize,
                      Style::empty(), Color::White, Color::Default, string.borrow());

        // draw cursor
        // TODO: render the char under the cursor
        rustbox.print_char((pos_x + self.cursor - self.scroll) as usize, pos_y as usize,
                           Style::empty(), Color::Blue, Color::Blue, ' ');
    }

    pub fn keypressed(&mut self, key : Key) -> TextFieldRet {
        match key {
            Key::Char(ch) => {
                self.buffer.insert(self.cursor as usize, ch);
                self.inc_cursor();
                TextFieldRet::KeyHandled
            },
            Key::Backspace => {
                if self.cursor > 0 {
                    self.buffer.remove(self.cursor as usize - 1);
                    self.dec_cursor();
                }
                TextFieldRet::KeyHandled
            },
            Key::Ctrl(ch) => {
                if ch == 'a' {
                    self.move_cursor(0);
                } else if ch == 'e' {
                    let cur = self.buffer.len() as i32; // Rust sucks
                    self.move_cursor(cur);
                } else if ch == 'k' {
                    self.buffer.drain(self.cursor as usize ..);
                } else if ch == 'w' {
                    // TODO: First consume whitespace under the cursor
                    let end_range = self.cursor as usize;
                    let mut begin_range = max(0, self.cursor - 1) as usize;
                    while begin_range > 0
                            && !self.buffer[begin_range].is_whitespace() {
                        begin_range -= 1;
                    }
                    self.buffer.drain(begin_range .. end_range);
                    self.move_cursor(begin_range as i32);
                }
                TextFieldRet::KeyHandled
            },
            Key::Left => {
                self.dec_cursor();
                TextFieldRet::KeyHandled
            },
            Key::Right => {
                self.inc_cursor();
                TextFieldRet::KeyHandled
            },
            Key::Enter => TextFieldRet::SendMsg,
            _ => TextFieldRet::KeyIgnored,
        }
    }

    ////////////////////////////////////////////////////////////////////////////
    // Manipulating cursor

    fn inc_cursor(&mut self) {
        let cur = min(self.buffer.len() as i32, self.cursor + 1);
        self.move_cursor(cur);
    }

    fn dec_cursor(&mut self) {
        let cur = max(0, self.cursor - 1);
        self.move_cursor(cur);
    }

    // NOTE: This doesn't do bounds checking! Use dec_cursor() and inc_cursor().
    fn move_cursor(&mut self, cursor : i32) {
        self.cursor = cursor;

        let left_end  = self.scroll;
        let right_end = self.scroll + self.width;

        if cursor - SCROLLOFF < left_end {
            self.scroll = max(0, cursor - SCROLLOFF);
        } else if cursor + SCROLLOFF > right_end {
            self.scroll = min(max(0, cursor + SCROLLOFF - self.width),
                              max(0, self.buffer.len() as i32 - self.width));
        }
    }
}
