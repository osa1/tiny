use std::borrow::Borrow;
use std::cmp::{max, min};
use std::mem;

use rustbox::keyboard::Key;
use rustbox::{RustBox};

use tui::style;
use tui::termbox;
use tui::widget::{WidgetRet, Widget};

// TODO: Make these settings
const SCROLLOFF : i32 = 5;
const HIST_SIZE : usize = 30;

pub struct TextField {
    /// The message that's currently being edited (not yet sent)
    buffer : Vec<char>,

    /// Cursor in currently shown line
    cursor : i32,

    /// Horizontal scroll
    scroll : i32,

    /// Width of the widget
    width : i32,

    /// A history of sent messages/commands. Once added messages are never
    /// modified. A modification attempt should result in a new buffer with a
    /// copy of the vector in history. (old contents of the buffer will be lost)
    history : Vec<Vec<char>>,

    /// Only available when moving in `history` vector.
    /// INVARIANT: When available, it's a valid index in history.
    hist_curs : Option<i32>,
}

impl TextField {
    pub fn new(width : i32) -> TextField {
        TextField {
            buffer: Vec::with_capacity(512),
            cursor: 0,
            scroll: 0,
            width: width,
            history: Vec::with_capacity(HIST_SIZE),
            hist_curs: None,
        }
    }

    pub fn resize_(&mut self, width : i32, _ : i32) {
        self.width = width;
        let cursor = self.cursor;
        self.move_cursor(cursor);
    }

    pub fn draw_(&self, _ : &RustBox, pos_x : i32, pos_y : i32) {
        // draw text
        let line_borrow : &[char] = {
            if let Some(hist_curs) = self.hist_curs {
                self.history[hist_curs as usize].borrow()
            } else {
                self.buffer.borrow()
            }
        };

        let slice : &[char] =
            &line_borrow[ self.scroll as usize ..
                          min(line_borrow.len(), (self.scroll + self.width) as usize) ];

        termbox::print_chars(pos_x, pos_y, style::USER_MSG.fg, style::USER_MSG.bg, slice);

        // On my terminal the cursor is only shown when there's a character
        // under it.
        if self.cursor as usize >= line_borrow.len() {
            termbox::print_char(pos_x + self.cursor - self.scroll, pos_y,
                                style::USER_MSG.fg, style::USER_MSG.bg,
                                ' ');
        }
        termbox::set_cursor(pos_x + self.cursor - self.scroll, pos_y);
    }

    pub fn keypressed_(&mut self, key : Key) -> WidgetRet {
        match key {
            Key::Char(ch) => {
                self.modify();
                self.buffer.insert(self.cursor as usize, ch);
                self.inc_cursor();
                WidgetRet::KeyHandled
            },

            Key::Backspace => {
                if self.cursor > 0 {
                    self.modify();
                    self.buffer.remove(self.cursor as usize - 1);
                    self.dec_cursor();
                }
                WidgetRet::KeyHandled
            },

            Key::Ctrl(ch) => {
                if ch == 'a' {
                    self.move_cursor(0);
                    WidgetRet::KeyHandled
                }

                else if ch == 'e' {
                    let cur = self.line_len();
                    self.move_cursor(cur);
                    WidgetRet::KeyHandled
                }

                else if ch == 'k' {
                    self.modify();
                    self.buffer.drain(self.cursor as usize ..);
                    WidgetRet::KeyHandled
                }

                else if ch == 'w' {
                    self.consume_word_before_curs();
                    WidgetRet::KeyHandled
                }

                else {
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
                if let Some(hist_curs) = self.hist_curs {
                    self.buffer.clear();
                    self.buffer.extend_from_slice(&self.history[hist_curs as usize]);
                }

                // FIXME: There's a bug here when hist_cursor is not None
                let ret = mem::replace(&mut self.buffer, Vec::new());
                if self.history.len() == HIST_SIZE {
                    let mut reuse = self.history.remove(0);
                    reuse.clear();
                    reuse.extend_from_slice(&ret);
                    self.history.push(reuse);
                } else {
                    self.history.push(ret.clone());
                }

                self.move_cursor(0);
                self.hist_curs = None;

                if ret.len() == 0 {
                    WidgetRet::KeyHandled
                } else {
                    WidgetRet::Input(ret)
                }
            },

            ////////////////////////////////////////////////////////////////////
            // Scrolling in history

            Key::Up => {
                match self.hist_curs {
                    Some(hist_curs) if hist_curs > 0 => {
                        self.hist_curs = Some(hist_curs - 1);
                        let cur = self.line_len();
                        self.move_cursor(cur);
                    },
                    Some(_) => {},
                    None => {
                        if !self.history.is_empty() {
                            self.hist_curs = Some((self.history.len() as i32) - 1);
                            let cur = self.line_len();
                            self.move_cursor(cur);
                        }
                    }
                }
                WidgetRet::KeyHandled
            },

            Key::Down => {
                match self.hist_curs {
                    Some(hist_curs) => {
                        if (hist_curs as usize) < self.history.len() - 1 {
                            self.hist_curs = Some(hist_curs + 1);
                            let cur = self.line_len();
                            self.move_cursor(cur);
                        } else {
                            self.hist_curs = None;
                            let cur = self.line_len();
                            self.move_cursor(cur);
                        }
                    },
                    None => {}
                }
                WidgetRet::KeyHandled
            },

            ////////////////////////////////////////////////////////////////////

            _ => WidgetRet::KeyIgnored,
        }
    }

    fn consume_word_before_curs(&mut self) {
        // No modifications can happen if the scroll is at the beginning
        if self.cursor == 0 {
            return;
        }

        self.modify();

        let char = self.buffer[(self.cursor - 1) as usize];

        // Try to imitate vim's behaviour here.
        if char.is_whitespace() {
            self.consume_before(|c| c.is_whitespace());
        }

        else if char.is_alphanumeric() {
            self.consume_before(|c| c.is_alphanumeric());
        }

        else {
            self.consume_before(|c| !c.is_alphanumeric());
        }
    }

    fn consume_before<F>(&mut self, f : F) where F : Fn(char) -> bool {
        let end_range = self.cursor as usize;
        let mut begin_range = self.cursor - 1;
        while begin_range >= 0 && f(self.buffer[begin_range as usize]) {
            begin_range -= 1;
        }
        self.buffer.drain(((begin_range + 1) as usize) .. end_range);
        self.move_cursor(begin_range + 1);
    }

    #[inline]
    fn line_len(&self) -> i32 {
        if let Some(hist_curs) = self.hist_curs {
            self.history[hist_curs as usize].len() as i32
        } else {
            self.buffer.len() as i32
        }
    }

    ////////////////////////////////////////////////////////////////////////////
    // We never modify history, so if the user attempts at a modification, we
    // just make the history entry current message by updating the current
    // buffer. (old contents are lost)

    fn modify(&mut self) {
        if let Some(hist_idx) = self.hist_curs {
            self.buffer.clear();
            self.buffer.extend_from_slice(self.history[hist_idx as usize].borrow());
            self.hist_curs = None;
        }
    }

    ////////////////////////////////////////////////////////////////////////////
    // Manipulating cursor

    fn inc_cursor(&mut self) {
        let cur = min(self.line_len(), self.cursor + 1);
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

        if self.line_len() + 1 < self.width {
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
                                  max(0, self.line_len() + 1 - self.width));
            }
        }
    }
}

impl Widget for TextField {
    fn resize(&mut self, width : i32, height : i32) {
        self.resize_(width, height);
    }

    fn draw(&self, rustbox : &RustBox, pos_x : i32, pos_y : i32) {
        self.draw_(rustbox, pos_x, pos_y);
    }

    fn keypressed(&mut self, key : Key) -> WidgetRet {
        self.keypressed_(key)
    }
}
