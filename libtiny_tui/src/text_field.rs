use std::{
    cmp::{max, min},
    mem,
};

use term_input::{Arrow, Key};
use termbox_simple::Termbox;

use crate::{config::Colors, termbox, trie::Trie, utils, widget::WidgetRet};

// TODO: Make these settings
const SCROLLOFF: i32 = 5;
const HIST_SIZE: usize = 30;

pub(crate) struct TextField {
    /// The message that's currently being edited (not yet sent)
    buffer: Vec<char>,

    /// Cursor in currently shown line
    cursor: i32,

    /// Horizontal scroll
    scroll: i32,

    /// Width of the widget
    width: i32,

    /// A history of sent messages/commands. Once added messages are never
    /// modified. A modification attempt should result in a new buffer with a
    /// copy of the vector in history. (old contents of the buffer will be lost)
    history: Vec<Vec<char>>,

    mode: Mode,
}

enum Mode {
    /// Editing the buffer
    Edit,

    /// Browsing history
    History(i32),

    /// Auto-completing a nick in channel
    Autocomplete {
        original_buffer: Vec<char>,
        insertion_point: usize,
        word_starts: usize,
        completions: Vec<String>,
        current_completion: usize,
    },
}

impl TextField {
    pub(crate) fn new(width: i32) -> TextField {
        TextField {
            buffer: Vec::with_capacity(512),
            cursor: 0,
            scroll: 0,
            width,
            history: Vec::with_capacity(HIST_SIZE),
            mode: Mode::Edit,
        }
    }

    pub(crate) fn resize(&mut self, width: i32) {
        self.width = width;
        let cursor = self.cursor;
        self.move_cursor(cursor);
    }

    pub(crate) fn draw(&self, tb: &mut Termbox, colors: &Colors, pos_x: i32, pos_y: i32) {
        match self.mode {
            Mode::Edit => {
                draw_line(
                    tb,
                    colors,
                    &self.buffer,
                    pos_x,
                    pos_y,
                    self.scroll,
                    self.width,
                    self.cursor,
                );
            }
            Mode::History(hist_curs) => {
                draw_line(
                    tb,
                    colors,
                    &self.history[hist_curs as usize],
                    pos_x,
                    pos_y,
                    self.scroll,
                    self.width,
                    self.cursor,
                );
            }
            Mode::Autocomplete {
                ref original_buffer,
                insertion_point,
                word_starts,
                ref completions,
                current_completion,
            } => {
                // draw a placeholder for the cursor
                tb.change_cell(
                    pos_x + self.cursor - self.scroll,
                    pos_y,
                    ' ',
                    colors.user_msg.fg,
                    colors.user_msg.bg,
                );

                let completion: &str = &completions[current_completion];

                let mut orig_buf_iter = original_buffer.iter().cloned();
                let mut completion_iter = completion.chars();

                let iter: utils::InsertIterator<char> =
                    utils::insert_iter(&mut orig_buf_iter, &mut completion_iter, insertion_point);

                for (char_idx, char) in iter.enumerate() {
                    if char_idx >= ((self.scroll + self.width) as usize) {
                        break;
                    }

                    if char_idx >= self.scroll as usize {
                        if char_idx >= word_starts && char_idx < insertion_point + completion.len()
                        {
                            tb.change_cell(
                                pos_x + (char_idx as i32) - self.scroll,
                                pos_y,
                                char,
                                colors.completion.fg,
                                colors.completion.bg,
                            );
                        } else {
                            tb.change_cell(
                                pos_x + (char_idx as i32) - self.scroll,
                                pos_y,
                                char,
                                colors.user_msg.fg,
                                colors.user_msg.bg,
                            );
                        }
                    }
                }

                tb.set_cursor(Some((
                    (pos_x + self.cursor - self.scroll) as u16,
                    pos_y as u16,
                )));
            }
        }
    }

    pub(crate) fn keypressed(&mut self, key: Key) -> WidgetRet {
        match key {
            Key::Char('\r') => {
                if self.line_len() > 0 {
                    self.modify();

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

                    WidgetRet::Input(ret)
                } else {
                    WidgetRet::KeyHandled
                }
            }

            Key::Char(ch) => {
                self.modify();
                self.buffer.insert(self.cursor as usize, ch);
                self.inc_cursor();
                WidgetRet::KeyHandled
            }

            Key::Backspace => {
                if self.cursor > 0 {
                    self.modify();
                    self.buffer.remove(self.cursor as usize - 1);
                    self.dec_cursor();
                }
                WidgetRet::KeyHandled
            }

            Key::Del => {
                if self.cursor < self.line_len() {
                    self.modify();
                    self.buffer.remove(self.cursor as usize);
                }
                WidgetRet::KeyHandled
            }

            Key::Ctrl(ch) => {
                if ch == 'a' {
                    self.move_cursor(0);
                    WidgetRet::KeyHandled
                } else if ch == 'e' {
                    self.move_cursor_to_end();
                    WidgetRet::KeyHandled
                } else if ch == 'k' {
                    if self.cursor != self.line_len() {
                        self.modify();
                        self.buffer.drain(self.cursor as usize..);
                    }
                    WidgetRet::KeyHandled
                } else if ch == 'w' {
                    self.consume_word_before_curs();
                    WidgetRet::KeyHandled
                } else {
                    WidgetRet::KeyIgnored
                }
            }

            Key::Arrow(Arrow::Left) => {
                self.dec_cursor();
                WidgetRet::KeyHandled
            }

            Key::Arrow(Arrow::Right) => {
                self.inc_cursor();
                WidgetRet::KeyHandled
            }

            Key::CtrlArrow(Arrow::Left) => {
                if self.cursor > 0 {
                    let mut cur = self.cursor as usize;
                    let mut skipped = false;
                    while cur > 0 && self.char_at(cur - 1).is_whitespace() {
                        cur -= 1;
                        skipped = true;
                    }
                    while cur > 0 && self.char_at(cur - 1).is_alphanumeric() {
                        cur -= 1;
                        skipped = true;
                    }
                    if !skipped {
                        cur -= 1; // skip at least one char
                    }
                    self.move_cursor(cur as i32);
                }
                WidgetRet::KeyHandled
            }

            Key::CtrlArrow(Arrow::Right) => {
                let len = self.line_len() as usize;
                if (self.cursor as usize) < len {
                    let mut cur = self.cursor as usize;
                    let mut skipped = false;
                    while cur < len && self.char_at(cur).is_alphanumeric() {
                        cur += 1;
                        skipped = true;
                    }
                    while cur < len && self.char_at(cur).is_whitespace() {
                        cur += 1;
                        skipped = true;
                    }
                    if !skipped {
                        cur += 1; // skip at least one char
                    }
                    self.move_cursor(cur as i32);
                }
                WidgetRet::KeyHandled
            }

            ////////////////////////////////////////////////////////////////////
            // Scrolling in history or autocompletion list
            Key::Arrow(Arrow::Up) => {
                let mode = mem::replace(&mut self.mode, Mode::Edit);

                match mode {
                    Mode::Edit => {
                        if !self.history.is_empty() {
                            self.mode = Mode::History((self.history.len() as i32) - 1);
                            self.move_cursor_to_end();
                        }
                    }
                    Mode::History(hist_curs) => {
                        self.mode = Mode::History(if hist_curs > 0 {
                            hist_curs - 1
                        } else {
                            hist_curs
                        });
                        self.move_cursor_to_end();
                    }
                    Mode::Autocomplete {
                        original_buffer,
                        insertion_point,
                        word_starts,
                        completions,
                        current_completion,
                        ..
                    } => {
                        let current_completion = if current_completion == completions.len() - 1 {
                            0
                        } else {
                            current_completion + 1
                        };

                        let cursor =
                            (insertion_point + completions[current_completion].len()) as i32;

                        self.mode = Mode::Autocomplete {
                            original_buffer,
                            insertion_point,
                            word_starts,
                            completions,
                            current_completion,
                        };

                        self.move_cursor(cursor);
                    }
                }

                WidgetRet::KeyHandled
            }

            Key::Arrow(Arrow::Down) => {
                let mode = mem::replace(&mut self.mode, Mode::Edit);

                match mode {
                    Mode::Edit => {}
                    Mode::History(hist_curs) => {
                        if hist_curs != (self.history.len() - 1) as i32 {
                            self.mode = Mode::History(hist_curs + 1);
                        } else {
                            self.mode = Mode::Edit;
                        }
                        self.move_cursor_to_end();
                    }
                    Mode::Autocomplete {
                        original_buffer,
                        insertion_point,
                        word_starts,
                        completions,
                        current_completion,
                        ..
                    } => {
                        let current_completion = if current_completion == 0 {
                            completions.len() - 1
                        } else {
                            current_completion - 1
                        };

                        let cursor =
                            (insertion_point + completions[current_completion].len()) as i32;

                        self.mode = Mode::Autocomplete {
                            original_buffer,
                            insertion_point,
                            word_starts,
                            completions,
                            current_completion,
                        };

                        self.move_cursor(cursor);
                    }
                }

                WidgetRet::KeyHandled
            }

            ////////////////////////////////////////////////////////////////////
            _ => WidgetRet::KeyIgnored,
        }
    }

    /// Get contents of the text field and clear it.
    pub(crate) fn flush(&mut self) -> String {
        self.cursor = 0;
        self.buffer.drain(..).collect()
    }

    /// Add a line to the text field history.
    pub(crate) fn add_history(&mut self, str: &str) {
        self.history.push(str.chars().collect());
    }

    pub(crate) fn set(&mut self, str: &str) {
        self.mode = Mode::Edit;
        self.buffer = str.chars().collect();
        self.move_cursor_to_end();
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
            self.consume_before(char::is_whitespace);
            self.consume_before(char::is_alphanumeric);
        } else {
            let char = self.buffer[(self.cursor - 1) as usize];
            if char.is_alphanumeric() {
                self.consume_before(char::is_alphanumeric);
            } else if self.cursor != 0 {
                // consume at least one char
                let cursor = self.cursor;
                self.buffer.remove(cursor as usize - 1);
                self.move_cursor(cursor - 1);
            }
        }
    }

    fn consume_before<F>(&mut self, f: F)
    where
        F: Fn(char) -> bool,
    {
        let end_range = self.cursor as usize;
        let mut begin_range = self.cursor - 1;
        while begin_range >= 0 && f(self.buffer[begin_range as usize]) {
            begin_range -= 1;
        }
        self.buffer.drain(((begin_range + 1) as usize)..end_range);
        self.move_cursor(begin_range + 1);
    }

    // Ignoring auto-completions
    fn shown_line(&self) -> &Vec<char> {
        match self.mode {
            Mode::Edit | Mode::Autocomplete { .. } => &self.buffer,
            Mode::History(hist_curs) => &self.history[hist_curs as usize],
        }
    }

    fn line_len(&self) -> i32 {
        match self.mode {
            Mode::Edit => self.buffer.len() as i32,
            Mode::History(hist_curs) => self.history[hist_curs as usize].len() as i32,
            Mode::Autocomplete {
                ref original_buffer,
                ref completions,
                current_completion,
                ..
            } => (original_buffer.len() + completions[current_completion].len()) as i32,
        }
    }

    fn char_at(&self, idx: usize) -> char {
        match self.mode {
            Mode::Edit => self.buffer[idx],
            Mode::History(hist_curs) => self.history[hist_curs as usize][idx],
            Mode::Autocomplete {
                ref original_buffer,
                insertion_point,
                ref completions,
                current_completion,
                ..
            } => {
                if idx < insertion_point {
                    original_buffer[idx]
                } else if idx >= insertion_point
                    && idx < insertion_point + completions[current_completion].len()
                {
                    completions[current_completion]
                        .chars()
                        .nth(idx - insertion_point)
                        .unwrap()
                } else {
                    original_buffer[idx - completions[current_completion].len()]
                }
            }
        }
    }

    ////////////////////////////////////////////////////////////////////////////

    fn in_autocomplete(&self) -> bool {
        match self.mode {
            Mode::Autocomplete { .. } => true,
            _ => false,
        }
    }

    fn modify(&mut self) {
        match self.mode {
            Mode::Edit => {}
            Mode::History(hist_idx) => {
                self.buffer.clear();
                self.buffer
                    .extend_from_slice(&self.history[hist_idx as usize]);
            }
            Mode::Autocomplete {
                ref mut original_buffer,
                mut insertion_point,
                ref mut completions,
                current_completion,
                ..
            } => {
                let mut buffer: Vec<char> = mem::replace(original_buffer, vec![]);
                let completions: Vec<String> = mem::replace(completions, vec![]);
                let word = &completions[current_completion];

                // FIXME: This is inefficient
                for char in word.chars() {
                    buffer.insert(insertion_point, char);
                    insertion_point += 1;
                }

                self.buffer = buffer;
            }
        }

        self.mode = Mode::Edit;
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

    fn move_cursor_to_end(&mut self) {
        let cursor = self.line_len();
        self.move_cursor(cursor);
    }

    fn move_cursor(&mut self, cursor: i32) {
        assert!(cursor >= 0 && cursor <= self.line_len());
        self.cursor = cursor;

        if self.line_len() + 1 < self.width {
            self.scroll = 0;
        } else {
            let scrolloff = {
                if self.width < 2 * SCROLLOFF + 1 {
                    0
                } else {
                    SCROLLOFF
                }
            };

            let left_end = self.scroll;
            let right_end = self.scroll + self.width;

            if cursor - scrolloff < left_end {
                self.scroll = max(0, cursor - scrolloff);
            } else if cursor + scrolloff >= right_end {
                self.scroll = min(
                    // +1 because cursor should be visible, i.e.
                    // right_end > cursor should hold after this
                    max(0, cursor + 1 + scrolloff - self.width),
                    // +1 because cursor goes one more character
                    // after the buffer, to be able to add chars
                    max(0, self.line_len() + 1 - self.width),
                );
            }
        }
    }
}

fn draw_line(
    tb: &mut Termbox,
    colors: &Colors,
    line: &[char],
    pos_x: i32,
    pos_y: i32,
    scroll: i32,
    width: i32,
    cursor: i32,
) {
    let slice: &[char] = &line[scroll as usize..min(line.len(), (scroll + width) as usize)];
    termbox::print_chars(tb, pos_x, pos_y, colors.user_msg, slice.iter().cloned());

    // On my terminal the cursor is only shown when there's a character
    // under it.
    if cursor as usize >= line.len() {
        tb.change_cell(
            pos_x + cursor - scroll,
            pos_y,
            ' ',
            colors.cursor.fg,
            colors.cursor.bg,
        );
    }
    tb.set_cursor(Some(((pos_x + cursor - scroll) as u16, pos_y as u16)));
}

impl TextField {
    pub(crate) fn autocomplete(&mut self, dict: &Trie) {
        if self.in_autocomplete() {
            // AWFUL CODE YO
            self.keypressed(Key::Arrow(Arrow::Up));
            return;
        }

        let cursor_right = self.cursor;
        let mut cursor_left = max(0, cursor_right - 1);

        let completions = {
            let line = self.shown_line();

            while cursor_left >= 0
                && line
                    .get(cursor_left as usize)
                    .map(|c| utils::is_nick_char(*c))
                    .unwrap_or(false)
            {
                cursor_left -= 1;
            }

            let word = {
                if cursor_left == cursor_right {
                    &[]
                } else {
                    cursor_left += 1;
                    &line[(cursor_left as usize)..(cursor_right as usize)]
                }
            };

            dict.drop_pfx(&mut word.iter().cloned())
        };

        if !completions.is_empty() {
            let completion_len = completions[0].len();
            self.mode = Mode::Autocomplete {
                original_buffer: self.shown_line().to_owned(),
                insertion_point: self.cursor as usize,
                word_starts: cursor_left as usize,
                completions,
                current_completion: 0,
            };
            let cursor = self.cursor;
            self.move_cursor(cursor + completion_len as i32);
        }
    }
}

////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {

    use super::*;
    use term_input::{Arrow, Key};

    #[test]
    fn text_field_bug() {
        let mut text_field = TextField::new(10);
        text_field.keypressed(Key::Char('a'));
        text_field.keypressed(Key::Char(' '));
        text_field.keypressed(Key::Char('b'));
        text_field.keypressed(Key::Char(' '));
        text_field.keypressed(Key::Char('c'));
        text_field.keypressed(Key::Char('\r'));
        text_field.keypressed(Key::Arrow(Arrow::Up));
        // this panics:
        text_field.keypressed(Key::CtrlArrow(Arrow::Left));
        // a b ^c
        assert_eq!(text_field.cursor, 4);
        text_field.keypressed(Key::CtrlArrow(Arrow::Right));
        assert_eq!(text_field.cursor, 5);
    }
}
