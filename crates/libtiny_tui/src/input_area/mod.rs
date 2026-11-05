use std::cmp::{max, min};
use std::mem;

use termbox_simple::Termbox;
use unicode_width::UnicodeWidthChar;

use crate::config::{Colors, Style};
use crate::key_map::KeyAction;
use crate::line_split::LineDataCache;
use crate::msg_area::MsgArea;
use crate::termbox;
use crate::trie::Trie;
use crate::utils;
use crate::widget::WidgetRet;

pub(crate) mod input_line;
use self::input_line::{InputLine, draw_line, draw_line_autocomplete};

/// Minimal number of terminal columns to keep around the cursor when scrolling.
const SCROLL_OFF: i32 = 5;

/// Minimum width of InputArea for wrapping
const SCROLL_FALLBACK_WIDTH: i32 = 36;

/// Input history size
const HIST_SIZE: usize = 30;

pub(crate) struct InputArea {
    /// The message that's currently being edited (not yet sent)
    buffer: InputLine,

    /// Cursor position
    cursor: i32,

    /// Width of the widget
    width: i32,

    /// Height of the widget, invalidated on input and resize
    height: Option<i32>,

    /// Maximum lines the widget can grow to
    max_lines: i32,

    /// Amount of scroll in the input field
    scroll: Option<i32>,

    /// A history of sent messages/commands. Once added messages are never
    /// modified. A modification attempt should result in a new buffer with a
    /// copy of the vector in history. (old contents of the buffer will be lost)
    history: Vec<InputLine>,

    mode: Mode,

    /// Current nickname. Not available on initialization (e.g. before registration with the
    /// server). Set with `set_nick`.
    nick: Option<Nickname>,
}

enum Mode {
    /// Editing the buffer
    Edit,

    /// Browsing history
    History(i32),

    /// Auto-completing a nick in channel
    Autocomplete {
        original_buffer: InputLine,
        insertion_point: usize,
        word_starts: usize,
        completions: Vec<String>,
        current_completion: usize,
    },
}

pub(crate) struct Nickname {
    value: String,
    color: usize,
}

static NICKNAME_SUFFIX: &str = ": ";

impl Nickname {
    fn new(value: String, color: usize) -> Nickname {
        Nickname { value, color }
    }

    /// Calculates the length of the nickname based on given width, including the NICKNAME_SUFFIX.
    /// Width should be the width of the `InputArea`. When length of the nick is 30% or less of the
    /// `TextField` width we show it (returns width of the nick), otherwise we don't (returns 0).
    fn len(&self, input_area_width: i32) -> usize {
        let len = termbox::text_width(&self.value) + termbox::text_width(NICKNAME_SUFFIX);
        if len as f32 <= input_area_width as f32 * (30f32 / 100f32) {
            len
        } else {
            0
        }
    }

    fn draw(&self, tb: &mut Termbox, colors: &Colors, mut pos_x: i32, pos_y: i32, width: i32) {
        if self.len(width) > 0 {
            let nick_color = colors.nick[self.color % colors.nick.len()];
            let style = Style {
                fg: u16::from(nick_color),
                bg: colors.user_msg.bg,
            };
            pos_x = termbox::print_chars(tb, pos_x, pos_y, style, self.value.chars());
            termbox::print_chars(tb, pos_x, pos_y, colors.faded, NICKNAME_SUFFIX.chars());
        }
    }
}

impl InputArea {
    pub(crate) fn new(width: i32, max_lines: i32) -> InputArea {
        InputArea {
            buffer: InputLine::new(),
            cursor: 0,
            width,
            height: Some(1),
            max_lines,
            scroll: None,
            history: Vec::with_capacity(HIST_SIZE),
            mode: Mode::Edit,
            nick: None,
        }
    }

    pub(crate) fn set_nick(&mut self, value: String, color: usize) {
        self.nick = Some(Nickname::new(value, color))
    }

    pub(crate) fn get_nick(&self) -> Option<String> {
        self.nick.as_ref().map(|nick| nick.value.clone())
    }

    /// Resizes input area
    pub(crate) fn resize(&mut self, width: i32, max_lines: i32) {
        self.width = width;
        self.height = None;
        self.max_lines = max_lines;
        if self.scroll.is_some() {
            self.move_cursor(self.cursor);
        }
    }

    pub(crate) fn draw(
        &mut self,
        tb: &mut Termbox,
        colors: &Colors,
        pos_x: i32,
        parent_y: i32,
        parent_height: i32,
        msg_area: &mut MsgArea,
    ) {
        let input_field_height = self.get_height(self.width);
        // if the height of msg_area needs to change...
        if parent_height - input_field_height != msg_area.get_height() {
            msg_area.resize(self.width, parent_height - input_field_height);
        }
        let pos_y = parent_y + parent_height - input_field_height;
        let mut nick_length = 0;
        if let Some(nick) = &self.nick {
            nick.draw(tb, colors, pos_x, pos_y, self.width);
            nick_length = nick.len(self.width) as i32;
        }
        match self.mode {
            Mode::Edit => {
                draw_line(
                    tb,
                    colors,
                    &self.buffer,
                    pos_x + nick_length,
                    pos_y,
                    self.width,
                    self.cursor,
                    self.should_scroll(),
                    self.scroll,
                    None,
                );
            }
            Mode::History(hist_curs) => {
                draw_line(
                    tb,
                    colors,
                    &self.history[hist_curs as usize],
                    pos_x + nick_length,
                    pos_y,
                    self.width,
                    self.cursor,
                    self.should_scroll(),
                    self.scroll,
                    None,
                );
            }
            Mode::Autocomplete {
                ref original_buffer,
                insertion_point,
                word_starts,
                ref completions,
                current_completion,
            } => draw_line_autocomplete(
                original_buffer,
                insertion_point,
                word_starts,
                completions,
                current_completion,
                tb,
                colors,
                pos_x + nick_length,
                pos_y,
                self.width,
                self.cursor,
                self.should_scroll(),
                self.scroll,
            ),
        }
    }

    pub(crate) fn keypressed(&mut self, key_action: &KeyAction) -> WidgetRet {
        match key_action {
            KeyAction::InputSend => {
                if self.current_buffer_len() > 0 {
                    self.modify();

                    let ret = mem::replace(&mut self.buffer, InputLine::new());
                    if self.history.len() == HIST_SIZE {
                        self.history.remove(0);
                    }
                    self.history.push(ret.clone());

                    self.move_cursor(0);

                    WidgetRet::Input(ret.get_buffer().to_owned())
                } else {
                    WidgetRet::KeyHandled
                }
            }
            KeyAction::InputDeletePrevChar => {
                if self.cursor > 0 {
                    self.modify();
                    self.buffer.remove(self.cursor as usize - 1);
                    self.dec_cursor();
                }
                WidgetRet::KeyHandled
            }
            KeyAction::InputDeleteNextChar => {
                if self.cursor < self.current_buffer_len() {
                    self.modify();
                    self.buffer.remove(self.cursor as usize);
                    self.move_cursor(self.cursor);
                }
                WidgetRet::KeyHandled
            }
            KeyAction::InputMoveCursStart => {
                self.move_cursor(0);
                WidgetRet::KeyHandled
            }
            KeyAction::InputMoveCursEnd => {
                self.move_cursor_to_end();
                WidgetRet::KeyHandled
            }
            KeyAction::InputDeleteToStart => {
                if self.cursor != 0 {
                    self.modify();
                    self.buffer.drain(..self.cursor as usize);
                    self.move_cursor(0);
                }
                WidgetRet::KeyHandled
            }
            KeyAction::InputDeleteToEnd => {
                if self.cursor != self.current_buffer_len() {
                    self.modify();
                    self.buffer.drain(self.cursor as usize..);
                    self.move_cursor(self.cursor);
                }
                WidgetRet::KeyHandled
            }
            KeyAction::InputDeletePrevWord => {
                self.consume_word_before_curs();
                WidgetRet::KeyHandled
            }
            KeyAction::InputMoveCursLeft => {
                self.dec_cursor();
                WidgetRet::KeyHandled
            }
            KeyAction::InputMoveCursRight => {
                self.inc_cursor();
                WidgetRet::KeyHandled
            }
            KeyAction::InputMoveWordLeft => {
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
            KeyAction::InputMoveWordRight => {
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
            KeyAction::InputPrevEntry => {
                self.completion_prev_entry();
                WidgetRet::KeyHandled
            }
            KeyAction::InputNextEntry => {
                self.completion_next_entry();
                WidgetRet::KeyHandled
            }
            KeyAction::Input(ch) => {
                self.modify();
                self.buffer.insert(self.cursor as usize, *ch);
                self.inc_cursor();
                WidgetRet::KeyHandled
            }
            KeyAction::Command(cmd) => WidgetRet::Command(cmd.to_owned()),
            _ => WidgetRet::KeyIgnored,
        }
    }

    /// Get contents of the text field and cursor location and clear it.
    pub(crate) fn flush(&mut self) -> (String, i32) {
        self.modify();
        let cursor = mem::replace(&mut self.cursor, 0);
        let contents = self.buffer.drain(..).collect();
        self.move_cursor(0);
        (contents, cursor)
    }

    /// Add a line to the text field history.
    pub(crate) fn add_history(&mut self, str: &str) {
        self.history
            .push(InputLine::from_buffer(str.chars().collect()));
    }

    pub(crate) fn set(&mut self, str: &str) {
        self.mode = Mode::Edit;
        self.buffer = InputLine::from_buffer(str.chars().collect());
        self.height = None;
        self.move_cursor_to_end();
    }

    pub(crate) fn set_cursor(&mut self, cursor: i32) {
        self.move_cursor(cursor.clamp(0, self.current_buffer_len()));
    }

    fn consume_word_before_curs(&mut self) {
        // No modifications can happen if the scroll is at the beginning
        if self.cursor == 0 {
            return;
        }

        self.modify();

        let char = self.buffer.get((self.cursor - 1) as usize);

        // Try to imitate vim's behaviour here.
        if char.is_whitespace() {
            self.consume_before(char::is_whitespace);
            self.consume_before(char::is_alphanumeric);
        } else {
            let char = self.buffer.get((self.cursor - 1) as usize);
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
        while begin_range >= 0 && f(self.buffer.get(begin_range as usize)) {
            begin_range -= 1;
        }
        self.buffer.drain(((begin_range + 1) as usize)..end_range);
        self.move_cursor(begin_range + 1);
    }

    fn completion_prev_entry(&mut self) {
        // invalidate height calculation
        self.height = None;
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
                    (insertion_point + completions[current_completion].chars().count()) as i32;

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
    }

    fn completion_next_entry(&mut self) {
        // invalidate height calculation
        self.height = None;
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
                    (insertion_point + completions[current_completion].chars().count()) as i32;

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
    }

    // Ignoring auto-completions
    pub(crate) fn shown_line(&mut self) -> &mut InputLine {
        match &mut self.mode {
            Mode::Edit | Mode::Autocomplete { .. } => &mut self.buffer,
            Mode::History(hist_curs) => &mut self.history[*hist_curs as usize],
        }
    }

    fn calculate_height_autocomplete(&self, width: i32, nick_length: usize) -> usize {
        if self.in_autocomplete() {
            let mut layout = LineDataCache::input_line(width, nick_length);
            layout.calculate_height(self.current_chars(), 0);
            layout.get_line_count().unwrap()
        } else {
            1
        }
    }

    /// Gets the height of the widget. If the height is larger than the allowed max lines (set on
    /// initialization) turn on scroll else turn off scroll.
    pub(crate) fn get_height(&mut self, width: i32) -> i32 {
        let height = match self.height {
            Some(height) => height,
            None => self.calculate_height(width),
        };

        // Check for scroll fallback
        if height >= self.max_lines || width <= SCROLL_FALLBACK_WIDTH {
            self.scroll_on();
            1
        } else {
            self.scroll_off();
            height
        }
    }

    fn scroll_on(&mut self) {
        if self.scroll.is_none() {
            self.scroll = Some(0);
        }
        self.move_cursor(self.cursor);
    }

    fn scroll_off(&mut self) {
        self.scroll = None;
    }

    fn calculate_height(&mut self, width: i32) -> i32 {
        let mut nick_length = 0;
        if let Some(nick) = &self.nick {
            nick_length = nick.len(self.width);
        }
        let line_count = if self.in_autocomplete() {
            self.calculate_height_autocomplete(width, nick_length) as i32
        } else {
            self.shown_line().calculate_height(width, nick_length) as i32
        };
        self.height = Some(line_count);
        line_count
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
            } => (original_buffer.len() + completions[current_completion].chars().count()) as i32,
        }
    }

    /// The displayed scalars, including an uncommitted completion, without copying the input.
    fn current_chars(&self) -> impl Iterator<Item = char> + Clone + '_ {
        let (before, completion, after): (&[char], &str, &[char]) = match &self.mode {
            Mode::Edit => (self.buffer.get_buffer(), "", &[]),
            Mode::History(index) => (self.history[*index as usize].get_buffer(), "", &[]),
            Mode::Autocomplete {
                original_buffer,
                insertion_point,
                completions,
                current_completion,
                ..
            } => {
                let (before, after) = original_buffer.get_buffer().split_at(*insertion_point);
                (before, &completions[*current_completion], after)
            }
        };
        before
            .iter()
            .copied()
            .chain(completion.chars())
            .chain(after.iter().copied())
    }

    fn char_at(&self, idx: usize) -> char {
        match self.mode {
            Mode::Edit => self.buffer.get(idx),
            Mode::History(hist_curs) => self.history[hist_curs as usize].get(idx),
            Mode::Autocomplete {
                ref original_buffer,
                insertion_point,
                ref completions,
                current_completion,
                ..
            } => {
                let completion_len = completions[current_completion].chars().count();
                if idx < insertion_point {
                    original_buffer.get(idx)
                } else if idx < insertion_point + completion_len {
                    completions[current_completion]
                        .chars()
                        .nth(idx - insertion_point)
                        .unwrap()
                } else {
                    original_buffer.get(idx - completion_len)
                }
            }
        }
    }

    ////////////////////////////////////////////////////////////////////////////

    fn in_autocomplete(&self) -> bool {
        matches!(self.mode, Mode::Autocomplete { .. })
    }

    fn modify(&mut self) {
        // invalidate height calculation
        self.height = None;
        match self.mode {
            Mode::Edit => {}
            Mode::History(hist_idx) => {
                self.buffer = self.history[hist_idx as usize].clone();
            }
            Mode::Autocomplete {
                ref mut original_buffer,
                mut insertion_point,
                ref mut completions,
                current_completion,
                ..
            } => {
                let mut buffer = mem::replace(original_buffer, InputLine::new());
                let completions: Vec<String> = mem::take(completions);
                let word = &completions[current_completion];

                // FIXME: This is inefficient
                for char in word.chars() {
                    buffer.insert(insertion_point, char);
                    insertion_point += 1;
                }

                self.buffer = buffer
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

    /// Update cursor location, possibly after an update. Update scroll value to fit as much of the
    /// input field as possible to the screen.
    fn move_cursor(&mut self, cursor: i32) {
        let line_len = self.current_buffer_len();

        assert!(cursor >= 0 && cursor <= line_len);
        self.cursor = cursor;

        if let Some(scroll) = self.scroll {
            let nick_length = self.nick.as_ref().map_or(0, |nick| nick.len(self.width));
            let width = (self.width - nick_length as i32).max(1);
            let line_len = line_len as usize;
            let cursor_idx = cursor as usize;
            let scroll = (scroll.max(0) as usize).min(line_len);
            let mut start = 0;
            let mut left = 0;
            let mut column = 0;
            let mut base_col = 0;
            let mut base_width = 1;
            let mut cursor_col = 0;
            let mut cursor_width = 1;
            // Keep the scroll start on a base, and map a cursor on a mark back to
            // that same base. The initial mark gets the dotted-circle column.
            for (idx, ch) in self.current_chars().enumerate() {
                let char_width = ch.width().unwrap_or(0) as i32;
                let effective = if idx == 0 && char_width == 0 {
                    1
                } else {
                    char_width.min(width)
                };
                if idx == 0 || char_width != 0 {
                    base_col = column;
                    base_width = effective.max(1);
                    if idx <= scroll {
                        start = idx;
                        left = column;
                    }
                }
                if idx == cursor_idx {
                    cursor_col = base_col;
                    cursor_width = base_width;
                }
                column += effective;
            }
            let total = column;
            if scroll == line_len {
                start = line_len;
                left = total;
            }
            if cursor_idx == line_len {
                cursor_col = total;
                cursor_width = 1;
            }
            let scrolloff = if width < 2 * SCROLL_OFF + 1 {
                0
            } else {
                SCROLL_OFF
            };
            // Find the nearest valid scalar boundary in the chosen direction.
            let target = if total + 1 < width {
                0
            } else if cursor_col - scrolloff < left {
                (cursor_col - scrolloff).max(0)
            } else if cursor_col + cursor_width - 1 + scrolloff >= (left + width).min(total) {
                (cursor_col + cursor_width + scrolloff - width)
                    .max(0)
                    .min((total + 1 - width).max(0))
            } else {
                left
            };
            if target != left {
                let mut col = 0;
                let mut chosen = None;
                let mut last_boundary = 0;
                for (idx, ch) in self
                    .current_chars()
                    .chain(std::iter::once('\0'))
                    .enumerate()
                {
                    if idx > cursor_idx {
                        break;
                    }
                    let char_width = ch.width().unwrap_or(0) as i32;
                    let boundary = idx == 0 || idx == line_len || char_width != 0;
                    if boundary {
                        last_boundary = idx;
                        if target < left && col <= target {
                            chosen = Some(idx);
                        } else if target > left && idx >= start && col >= target {
                            chosen = Some(idx);
                            break;
                        }
                    }
                    col += if idx == 0 && char_width == 0 {
                        1
                    } else {
                        char_width.min(width)
                    };
                }
                start = chosen.unwrap_or(if target < left { 0 } else { last_boundary });
            }
            self.scroll = Some(start as i32);
        }
    }
}

impl InputArea {
    pub(crate) fn autocomplete(&mut self, dict: &Trie) {
        if self.in_autocomplete() {
            // scroll next if you hit the KeyAction::InputAutoComplete key again
            self.completion_prev_entry();
            return;
        }

        // invalidate height calculation
        self.height = None;

        let cursor_right = self.cursor;
        let mut cursor_left = max(0, cursor_right - 1);

        let completions = {
            let line = &self.shown_line().get_buffer();

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
            let completion_len = completions[0].chars().count();
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
}

////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn nickname_width_counts_columns_and_isolated_marks() {
        let nick = Nickname::new("日a\u{3099}".into(), 0);
        assert_eq!(nick.len(17), 5);
        assert_eq!(nick.len(16), 0);
        let nick = Nickname::new("\u{3099}日".into(), 0);
        assert_eq!(nick.len(17), 5);
    }

    #[test]
    fn nickname_draws_suffix_after_display_columns() {
        let nick = Nickname::new("\u{3099}日".into(), 0);
        let colors = Colors::default();
        let mut tb = Termbox::init_test(20, 1);
        nick.draw(&mut tb, &colors, 0, 0, 20);
        tb.present();
        let cells = tb.get_front_buffer().cells;
        assert_eq!((cells[0].ch, cells[1].ch, cells[3].ch), ('◌', '日', ':'));
        assert!(
            String::from_utf8(tb.take_test_output())
                .unwrap()
                .contains("◌\u{3099}")
        );
    }

    #[test]
    fn text_field_bug() {
        let mut text_field = InputArea::new(10, 50);
        text_field.keypressed(&KeyAction::Input('a'));
        text_field.keypressed(&KeyAction::Input(' '));
        text_field.keypressed(&KeyAction::Input('b'));
        text_field.keypressed(&KeyAction::Input(' '));
        text_field.keypressed(&KeyAction::Input('c'));
        text_field.keypressed(&KeyAction::InputSend);
        text_field.keypressed(&KeyAction::InputPrevEntry);
        // this panics:
        text_field.keypressed(&KeyAction::InputMoveWordLeft);
        // a b ^c
        assert_eq!(text_field.cursor, 4);
        text_field.keypressed(&KeyAction::InputMoveWordRight);
        assert_eq!(text_field.cursor, 5);
    }

    #[test]
    fn test_set_buffer() {
        let mut input_area = InputArea::new(40, 50);
        // a string that will be more than one line - 41 characters
        let multiline_string_no_spaces = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        input_area.set(multiline_string_no_spaces);
        assert_eq!(input_area.get_height(input_area.width), 2);
    }

    fn visible_text(input: &InputArea) -> String {
        (0..input.current_buffer_len() as usize)
            .map(|idx| input.char_at(idx))
            .collect()
    }

    #[test]
    fn unicode_completion_cycles_at_insertion_point_and_sends_history() {
        let mut dict = Trie::new();
        dict.insert("日本語");
        dict.insert("日光");
        let mut input = InputArea::new(8, 50);
        input.set("日!끝");
        input.set_cursor(1);
        input.get_height(8);
        input.autocomplete(&dict);
        assert_eq!(visible_text(&input), "日光!끝");
        assert_eq!(input.cursor, 2);
        assert_eq!(input.current_buffer_len(), 4);
        input.keypressed(&KeyAction::InputPrevEntry);
        assert_eq!(visible_text(&input), "日本語!끝");
        assert_eq!(input.cursor, 3);
        input.keypressed(&KeyAction::InputNextEntry);
        assert_eq!(visible_text(&input), "日光!끝");
        input.keypressed(&KeyAction::Input('中'));
        assert_eq!(visible_text(&input), "日光中!끝");
        input.keypressed(&KeyAction::InputDeletePrevChar);
        input.keypressed(&KeyAction::InputDeleteNextChar);
        assert_eq!(visible_text(&input), "日光끝");
        match input.keypressed(&KeyAction::InputSend) {
            WidgetRet::Input(chars) => assert_eq!(chars.iter().collect::<String>(), "日光끝"),
            _ => panic!("expected sent input"),
        }
        input.keypressed(&KeyAction::InputPrevEntry);
        assert_eq!(visible_text(&input), "日光끝");
        input.keypressed(&KeyAction::InputMoveCursLeft);
        input.keypressed(&KeyAction::Input('語'));
        assert_eq!(visible_text(&input), "日光語끝");
        input.keypressed(&KeyAction::InputPrevEntry);
        assert_eq!(visible_text(&input), "日光끝");
    }

    #[test]
    fn nick_completion_keeps_combining_marks_in_the_prefix() {
        let mut dict = Trie::new();
        dict.insert("か\u{3099}め");
        let mut input = InputArea::new(40, 50);
        input.set("say か\u{3099} !");
        input.set_cursor(6);
        input.autocomplete(&dict);
        assert_eq!(visible_text(&input), "say か\u{3099}め !");
        match input.keypressed(&KeyAction::InputSend) {
            WidgetRet::Input(chars) => {
                assert_eq!(chars.iter().collect::<String>(), "say か\u{3099}め !")
            }
            _ => panic!("expected sent input"),
        }
    }

    #[test]
    fn completion_height_uses_inserted_scalars() {
        let mut dict = Trie::new();
        dict.insert("日本語");
        let mut input = InputArea::new(40, 50);
        input.set("日!abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLM");
        input.set_cursor(1);
        input.autocomplete(&dict);
        let preview = visible_text(&input);
        let mut expected = InputLine::from_buffer(preview.chars().collect());
        assert_eq!(
            input.get_height(40) as usize,
            expected.calculate_height(40, 0)
        );
        assert_eq!(preview, "日本語!abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLM");
    }

    #[test]
    fn scrolling_uses_columns_and_preserves_combining_boundaries() {
        let mut input = InputArea::new(6, 50);
        input.set("A日か\u{3099}本B");
        input.get_height(6);
        for cursor in 0..=input.current_buffer_len() {
            input.set_cursor(cursor);
            let start = input.scroll.unwrap() as usize;
            assert!(start <= cursor as usize);
            let chars: Vec<_> = visible_text(&input).chars().collect();
            assert!(start == 0 || start == chars.len() || chars[start].width().unwrap_or(0) != 0);
            let columns: usize = chars[start..cursor as usize]
                .iter()
                .map(|ch| ch.width().unwrap_or(0))
                .sum();
            assert!(columns < 6);
        }
        input.resize(3, 50);
        input.set_cursor(input.current_buffer_len());
        assert!(input.scroll.unwrap() > 0);
        input.keypressed(&KeyAction::InputDeletePrevChar);
        input.keypressed(&KeyAction::InputMoveCursStart);
        input.keypressed(&KeyAction::InputDeleteNextChar);
        assert_eq!(visible_text(&input), "日か\u{3099}本");
        input.resize(20, 50);
        input.get_height(20);
        assert_eq!(input.scroll, Some(0));
        input.resize(1, 50);
        input.set("日A");
        input.get_height(1);
        assert_eq!(input.scroll, Some(2));
    }
}
