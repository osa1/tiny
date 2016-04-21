use std::borrow::Borrow;
use std::cmp::{max, min};
use std::mem;

use rustbox::keyboard::Key;
use rustbox::{RustBox};

use tui::style;
use tui::termbox;
use tui::widget::{WidgetRet};

// TODO: Make this a setting
static SCROLLOFF : i32 = 5;

pub struct TextField {
    buffer : Vec<char>,
    cursor : i32,

    /// Horizontal scroll
    scroll : i32,

    width : i32,
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

    pub fn resize(&mut self, width : i32, _ : i32) {
        self.width = width;
        let cursor = self.cursor;
        self.move_cursor(cursor);
    }

    pub fn get_msg(&mut self) -> &mut Vec<char> {
        &mut self.buffer
    }

    pub fn clear_buffer(&mut self) {
        self.buffer.clear();
        self.move_cursor(0);
    }

    pub fn draw(&self, _ : &RustBox, pos_x : i32, pos_y : i32) {
        // draw text
        let buffer_borrow : &[char] = self.buffer.borrow();

        let slice : &[char] =
            &buffer_borrow[ self.scroll as usize ..
                            min(self.buffer.len(), (self.scroll + self.width) as usize) ];

        let string : String = slice.iter().cloned().collect();

        termbox::print(pos_x, pos_y, style::USER_MSG.fg, style::USER_MSG.bg, &string);

        // draw cursor
        // TODO: render the char under the cursor
        termbox::print_char(pos_x + self.cursor - self.scroll, pos_y,
                            style::CURSOR.fg, style::CURSOR.bg, 'x');
    }

    pub fn keypressed(&mut self, key : Key) -> WidgetRet {
        match key {
            Key::Char(ch) => {
                self.buffer.insert(self.cursor as usize, ch);
                self.inc_cursor();
                WidgetRet::KeyHandled
            },
            Key::Backspace => {
                if self.cursor > 0 {
                    self.buffer.remove(self.cursor as usize - 1);
                    self.dec_cursor();
                }
                WidgetRet::KeyHandled
            },
            Key::Ctrl(ch) => {
                if ch == 'a' {
                    self.move_cursor(0);
                    WidgetRet::KeyHandled
                } else if ch == 'e' {
                    let cur = self.buffer_len();
                    self.move_cursor(cur);
                    WidgetRet::KeyHandled
                } else if ch == 'k' {
                    self.buffer.drain(self.cursor as usize ..);
                    WidgetRet::KeyHandled
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
                    WidgetRet::KeyHandled
                } else {
                    WidgetRet::KeyIgnored
                }
            },
            Key::Left => {
                self.dec_cursor();
                WidgetRet::KeyHandled
            },
            Key::Right => {
                self.inc_cursor();
                WidgetRet::KeyHandled
            },
            Key::Enter => {
                let ret = mem::replace(&mut self.buffer, Vec::new());
                self.move_cursor(0);
                if ret.len() == 0 {
                    WidgetRet::KeyHandled
                } else {
                    WidgetRet::Input(ret)
                }
            },
            _ => WidgetRet::KeyIgnored,
        }
    }

    #[inline]
    fn buffer_len(&self) -> i32 {
        self.buffer.len() as i32
    }

    ////////////////////////////////////////////////////////////////////////////
    // Manipulating cursor

    fn inc_cursor(&mut self) {
        let cur = min(self.buffer_len(), self.cursor + 1);
        self.move_cursor(cur);
    }

    fn dec_cursor(&mut self) {
        let cur = max(0, self.cursor - 1);
        self.move_cursor(cur);
    }

    // NOTE: This doesn't do bounds checking! Use dec_cursor() and inc_cursor().
    // move_cursor(0) should always be safe.
    fn move_cursor(&mut self, cursor : i32) {
        self.cursor = cursor;

        if self.buffer_len() + 1 < self.width {
            self.scroll = 0;
        } else {
            let scrolloff = { if self.width < 2 * SCROLLOFF + 1 { 0 } else { SCROLLOFF } };

            let left_end  = self.scroll;
            let right_end = self.scroll + self.width;

            if cursor - scrolloff < left_end {
                self.scroll = max(0, cursor - scrolloff);
            } else if cursor + scrolloff >= right_end {
                self.scroll = min(// +1 because cursor should be visible, i.e.
                                  // right_end > cursor should hold after this
                                  max(0, cursor + 1 + scrolloff - self.width),
                                  // +1 because cursor goes one more character
                                  // after the buffer, to be able to add chars
                                  max(0, self.buffer_len() + 1 - self.width));
            }
        }
    }
}
