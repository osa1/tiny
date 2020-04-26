use std::{
    cmp::{max, min},
    mem,
};

use term_input::{Arrow, Key};
use termbox_simple::Termbox;

use crate::{config::Colors, termbox, trie::Trie, utils, widget::WidgetRet};

// TODO: Make these settings
const SCROLL_OFF: i32 = 5;

/// Minimum width of TextField for wrapping
const SCROLL_FALLBACK_WIDTH: i32 = 36;

/// Input history size
const HIST_SIZE: usize = 30;

pub(crate) struct TextField {
    /// The message that's currently being edited (not yet sent)
    buffer: Vec<char>,

    /// Cursor in currently shown line
    cursor: i32,

    /// Width of the widget
    width: i32,

    /// Max lines before turning into scroll mode
    max_lines: i32,

    /// Config value for text field wrapping
    text_field_wrap: bool,

    wrapped_lines: Option<Vec<Vec<char>>>,

    /// Amount of scroll in the input field
    scroll: Option<i32>,

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
    pub(crate) fn new(width: i32, max_lines: i32, text_field_wrap: bool) -> TextField {
        TextField {
            buffer: Vec::with_capacity(512),
            cursor: 0,
            width,
            max_lines,
            text_field_wrap,
            wrapped_lines: None,
            scroll: if text_field_wrap { None } else { Some(0) },
            history: Vec::with_capacity(HIST_SIZE),
            mode: Mode::Edit,
        }
    }

    pub(crate) fn resize(&mut self, width: i32, max_lines: i32) {
        self.width = width;
        self.max_lines = max_lines;
        self.scroll = self.get_scroll_for_resize();
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
                    self.width,
                    self.cursor,
                    self.should_scroll(),
                    self.scroll,
                    &self.wrapped_lines,
                    None,
                );
            }
            Mode::History(hist_curs) => {
                draw_line(
                    tb,
                    colors,
                    &self.history[hist_curs as usize],
                    pos_x,
                    pos_y,
                    self.width,
                    self.cursor,
                    self.should_scroll(),
                    self.scroll,
                    &self.wrapped_lines,
                    None,
                );
            }
            Mode::Autocomplete {
                ref original_buffer,
                insertion_point,
                word_starts,
                ref completions,
                current_completion,
            } => {
                let completion: &str = &completions[current_completion];

                let mut orig_buf_iter = original_buffer.iter().cloned();
                let mut completion_iter = completion.chars();

                if self.should_scroll() {
                    let cursor_x_off = self.cursor - self.scroll.unwrap();
                    let cursor_y_off = 0;

                    // draw a placeholder for the cursor
                    tb.change_cell(
                        pos_x + cursor_x_off,
                        pos_y + cursor_y_off,
                        ' ',
                        colors.user_msg.fg,
                        colors.user_msg.bg,
                    );

                    let iter: utils::InsertIterator<char> = utils::insert_iter(
                        &mut orig_buf_iter,
                        &mut completion_iter,
                        insertion_point,
                    );

                    for (char_idx, char) in iter.enumerate() {
                        let x_off;
                        let y_off;
                        let mut can_scroll = true;
                        if self.should_scroll() {
                            let scroll = self.scroll.unwrap_or(0);
                            x_off = (char_idx as i32) - scroll;
                            y_off = 0;
                            if char_idx >= ((scroll + self.width) as usize) {
                                break;
                            }
                            if char_idx < scroll as usize {
                                can_scroll = false
                            }
                        } else {
                            x_off = char_idx as i32 % self.width;
                            y_off = char_idx as i32 / self.width;
                        }
                        if can_scroll {
                            if char_idx >= word_starts
                                && char_idx < insertion_point + completion.len()
                            {
                                tb.change_cell(
                                    pos_x + x_off,
                                    pos_y + y_off,
                                    char,
                                    colors.completion.fg,
                                    colors.completion.bg,
                                );
                            } else {
                                tb.change_cell(
                                    pos_x + x_off,
                                    pos_y + y_off,
                                    char,
                                    colors.user_msg.fg,
                                    colors.user_msg.bg,
                                );
                            }
                        }
                    }
                    tb.set_cursor(Some((
                        (pos_x + cursor_x_off) as u16,
                        (pos_y + cursor_y_off) as u16,
                    )));
                } else {
                    let iter: utils::InsertIterator<char> = utils::insert_iter(
                        &mut orig_buf_iter,
                        &mut completion_iter,
                        insertion_point,
                    );
                    let lines: Vec<Vec<char>> =
                        wrap_lines(&iter.into_iter().collect(), self.width as usize);
                    let completion_range = CompletionRange {
                        start_idx: word_starts as i32,
                        end_idx: (insertion_point + completion.len()) as i32,
                    };
                    draw_line_wrapped(
                        tb,
                        colors,
                        pos_x,
                        pos_y,
                        self.width,
                        self.cursor,
                        &lines,
                        &Some(completion_range),
                    );
                }
            }
        }
    }

    pub(crate) fn keypressed(&mut self, key: Key) -> WidgetRet {
        match key {
            Key::Char('\r') => {
                if self.current_buffer_len() > 0 {
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
                if self.cursor < self.current_buffer_len() {
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
                    if self.cursor != self.current_buffer_len() {
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
                let len = self.current_buffer_len() as usize;
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

    fn current_buffer_len(&self) -> i32 {
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

    /// Calculate how many lines of text will be in the textfield
    /// based on the width of the widget
    pub(crate) fn calculate_lines(&mut self) -> i32 {
        if !self.scroll.is_some() {
            let len = self.current_buffer_len();
            if len >= self.width {
                let wrapped_lines = wrap_lines(self.shown_line(), self.width as usize);
                let mut line_count = wrapped_lines.len() as i32;
                let last_line_len = wrapped_lines.last().unwrap().len();

                // might need space to move cursor to next line
                match self.mode {
                    Mode::Autocomplete {
                        ref completions,
                        current_completion,
                        ..
                    } => {
                        if last_line_len + completions[current_completion].len()
                            >= self.width as usize
                        {
                            line_count += 1;
                        }
                    }
                    _ => {
                        if last_line_len == self.width as usize {
                            line_count += 1;
                        }
                    }
                }

                self.wrapped_lines = Some(wrapped_lines);
                line_count
            } else {
                1
            }
        } else {
            1
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
        let cur = min(self.current_buffer_len(), self.cursor + 1);
        self.move_cursor(cur);
    }

    fn dec_cursor(&mut self) {
        let cur = max(0, self.cursor - 1);
        self.move_cursor(cur);
    }

    fn move_cursor_to_end(&mut self) {
        let cursor = self.current_buffer_len();
        self.move_cursor(cursor);
    }

    fn move_cursor(&mut self, cursor: i32) {
        assert!(cursor >= 0 && cursor <= self.current_buffer_len());
        self.cursor = cursor;

        if self.scroll.is_some() {
            if self.current_buffer_len() + 1 >= self.width {
                let scrolloff = {
                    if self.width < 2 * SCROLL_OFF + 1 {
                        0
                    } else {
                        SCROLL_OFF
                    }
                };

                let left_end = self.scroll.unwrap();
                let right_end = self.scroll.unwrap() + self.width;

                if cursor - scrolloff < left_end {
                    self.scroll = Some(max(0, cursor - scrolloff));
                } else if cursor + scrolloff >= right_end {
                    let scroll = min(
                        // +1 because cursor should be visible, i.e.
                        // right_end > cursor should hold after this
                        max(0, cursor + 1 + scrolloff - self.width),
                        // +1 because cursor goes one more character
                        // after the buffer, to be able to add chars
                        max(0, self.current_buffer_len() + 1 - self.width),
                    );
                    self.scroll = Some(scroll);
                }
            } else {
                self.scroll = Some(0);
            }
        }
    }
}

fn draw_line_scroll(
    tb: &mut Termbox,
    colors: &Colors,
    line: &[char],
    pos_x: i32,
    pos_y: i32,
    width: i32,
    cursor: i32,
    scroll: i32,
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

struct CompletionRange {
    start_idx: i32,
    end_idx: i32,
}

fn draw_line_wrapped(
    tb: &mut Termbox,
    colors: &Colors,
    pos_x: i32,
    pos_y: i32,
    width: i32,
    cursor: i32,
    lines: &Vec<Vec<char>>,
    completion_range: &Option<CompletionRange>,
) {
    // handle text field wrapping
    let mut y = pos_y;
    let mut cursor_xy: (i32, i32) = (0, 0);
    let mut cursor_char = ' ';

    let mut cursor_counter: i32 = 0;
    // eprintln!("{:?}", lines);
    for l in lines {
        for (idx, c) in l.iter().enumerate() {
            let x_off = pos_x + idx as i32;

            let mut style = colors.user_msg;
            // for autocompletion highlighting
            if let Some(completion_range) = completion_range {
                if cursor_counter >= completion_range.start_idx
                    && cursor_counter < completion_range.end_idx
                {
                    style = colors.completion;
                }
            }

            tb.change_cell(x_off, y, *c, style.fg, style.bg);

            if cursor_counter + 1 == cursor {
                if idx as i32 == width - 1 {
                    cursor_xy = (pos_x, y + 1);
                } else {
                    cursor_xy = (x_off + 1, y);
                }
            } else if cursor_counter == cursor {
                cursor_xy = (x_off, y);
                cursor_char = *c;
            }
            cursor_counter += 1;
        }
        y += 1;
    }

    tb.change_cell(
        cursor_xy.0,
        cursor_xy.1,
        cursor_char,
        colors.cursor.fg,
        colors.cursor.bg,
    );

    tb.set_cursor(Some(((cursor_xy.0) as u16, (cursor_xy.1) as u16)));
}

fn draw_line(
    tb: &mut Termbox,
    colors: &Colors,
    line: &Vec<char>,
    pos_x: i32,
    pos_y: i32,
    width: i32,
    cursor: i32,
    should_scroll: bool,
    scroll: Option<i32>,
    wrapped_lines: &Option<Vec<Vec<char>>>,
    completion_range: Option<CompletionRange>,
) {
    if should_scroll || line.len() < width as usize {
        draw_line_scroll(
            tb,
            colors,
            line,
            pos_x,
            pos_y,
            width,
            cursor,
            scroll.unwrap_or(0),
        );
    } else {
        let lines: Vec<Vec<char>> = match wrapped_lines {
            Some(wrapped_lines) => wrapped_lines.clone(),
            None => wrap_lines(line, width as usize),
        };

        draw_line_wrapped(
            tb,
            colors,
            pos_x,
            pos_y,
            width,
            cursor,
            &lines,
            &completion_range,
        );
    }
}

fn wrap_lines(buffer: &Vec<char>, width: usize) -> Vec<Vec<char>> {
    let mut lines: Vec<Vec<char>> = Vec::new();
    let mut current_line_len = 1;
    let mut last_whitespace_idx: Option<usize> = None;
    let mut current_line_start_idx: usize = 0;

    for (idx, c) in buffer.iter().enumerate() {
        // store whitespace if we need to go back to it and split
        if c.is_whitespace() {
            last_whitespace_idx = Some(idx);
        }
        // eprintln!("current_line_len {} width {}", current_line_len, width);
        let mut line_end_idx = idx;

        // need to break the line
        if current_line_len == width {
            if !c.is_whitespace() {
                // go back to last whitespace and cut it off
                if let Some(last_whitespace_idx) = last_whitespace_idx {
                    line_end_idx = last_whitespace_idx;
                }
            }
            // last character of line is whitespace, so wrap on it
            lines.push(buffer[current_line_start_idx..=line_end_idx].into());
            current_line_start_idx = line_end_idx + 1;
            // set the length of the next line based on how many characters back we went
            current_line_len = max(1, idx - line_end_idx);
        } else {
            current_line_len += 1;
        }

        // if the rest of the characters fit on one line then just push and break
        if buffer.len() - current_line_start_idx <= width {
            lines.push(buffer[current_line_start_idx..].into());
            break;
        }
    }
    lines
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

    fn should_scroll(&self) -> bool {
        self.scroll.is_some()
    }

    fn get_scroll_for_resize(&self) -> Option<i32> {
        if !self.text_field_wrap || self.max_lines == 1 || self.width <= SCROLL_FALLBACK_WIDTH {
            self.scroll.or(Some(0))
        } else {
            None
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
        let mut text_field = TextField::new(10, 10, false);
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
