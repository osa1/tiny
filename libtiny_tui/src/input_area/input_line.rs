use crate::{config::Colors, termbox, utils};
use std::{cmp::min, ops::RangeBounds, vec::Drain};
use termbox_simple::Termbox;

/// Cache that stores the state of InputLine's height calculation.
/// `line_count` is used as the dirty bit to invalidate the cache.
#[derive(Clone, Debug)]
struct LineDataCache {
    /// Indices to split on when we draw, not always whitespaces
    split_indices: Vec<i32>,
    /// The total number of lines (height) that will be rendered
    line_count: Option<i32>,
    /// The current width of InputArea. Used in determining if we need to invalidate due to resize.
    width: i32,
    /// The current width of the InputLine (may be shorter due to nickname)
    line_width: i32,
    /// Current nickname length. Used in determining if we need to invalidate due to resize.
    nick_length: usize,
    /// The index into InputLine::buffer of the last whitespace that we saw in calculate_height()
    last_whitespace_idx: Option<i32>,
    /// True if the last character was a whitespace character
    prev_char_is_whitespace: bool,
    /// The length of the current line that is being added to.
    /// Used to determine when to wrap to the next line in calculate_height()
    current_line_length: i32,
}

impl LineDataCache {
    fn new() -> LineDataCache {
        LineDataCache {
            split_indices: Vec::new(),
            line_count: None,
            width: 0,
            line_width: 0,
            nick_length: 0,
            last_whitespace_idx: None,
            prev_char_is_whitespace: false,
            current_line_length: 0,
        }
    }

    /// Performs a check to see if the width or nickname length changed
    /// which would require an invalidation of the cache and recalculation of
    /// the InputLine height.
    fn needs_resize(&self, width: i32, nick_length: usize) -> bool {
        self.width != width || self.nick_length != nick_length
    }

    /// Sets `line_count` to `None`, which invalidates the cache.
    fn set_dirty(&mut self) {
        self.line_count = None;
    }

    /// Checks if the cache is invalidated by seeing if
    /// `line_count` is `None`.
    fn is_dirty(&self) -> bool {
        self.line_count.is_none()
    }

    /// Resets the cache to a default state that requires
    /// a height calculation.
    fn reset(&mut self, width: i32, nick_length: usize) {
        self.split_indices.clear();
        self.line_count = None;
        self.width = width;
        self.nick_length = nick_length;
        self.line_width = width - nick_length as i32;
        self.last_whitespace_idx = None;
        self.prev_char_is_whitespace = false;
        self.current_line_length = 0;
    }

    fn get_line_count(&self) -> Option<usize> {
        self.line_count.map(|c| c as usize)
    }

    /// Function that calculates the height of the `InputLine`.
    /// and sets `split_indices` for drawing.
    /// An `offset` allows for resuming the calculation - see InputLine::insert().
    /// `offset` must be less than or equal to the current buffer size.
    ///
    /// Scans through the buffer in one pass to determine how many lines
    /// will be needed to render the text with word wrapping.
    /// If an offset is provided, it will continue the calculation
    /// from the saved state and save the new line count in `line_count`.
    fn calculate_height(&mut self, buffer: &Vec<char>, offset: usize) {
        debug_assert!(offset <= buffer.len());
        let mut temp_count = 1;
        if let Some(line_count) = self.line_count {
            temp_count = line_count;
            // If we made space for the cursor, subtract it.
            if self.current_line_length == self.line_width {
                temp_count -= 1;
            }
        }
        for (c, current_idx) in buffer.iter().skip(offset).zip(offset..) {
            let current_idx = current_idx as i32;
            self.current_line_length += 1;

            if c.is_whitespace() {
                // Splitting
                if self.current_line_length > self.line_width {
                    // we're on a whitespace so just go to next line
                    temp_count += 1;
                    // this character will be the first one on the next line
                    self.current_line_length = 1;
                    // nick is shown on the first line, set width to full width in the consecutive
                    // lines
                    self.line_width = self.width;
                    // store index for drawing
                    self.split_indices.push(current_idx);
                }
                // store whitespace for splitting
                self.last_whitespace_idx = Some(current_idx);
                self.prev_char_is_whitespace = true;
            } else {
                // Splitting
                if self.current_line_length > self.line_width {
                    // if the previous character was a whitespace, then we have a clean split
                    if !self.prev_char_is_whitespace && self.last_whitespace_idx.is_some() {
                        // move back to the last whitespace and get the length of the input that
                        // will be on the next line
                        self.current_line_length = current_idx - self.last_whitespace_idx.unwrap();
                        // store index for drawing
                        self.split_indices
                            .push(self.last_whitespace_idx.unwrap() + 1);
                    } else {
                        // unclean split on non-whitespace
                        self.current_line_length = 1;
                        // store index for drawing
                        self.split_indices.push(current_idx);
                    }
                    // invalidate whitespace since we split here
                    self.last_whitespace_idx = None;
                    // moved to next line
                    temp_count += 1;
                    // set width to full width
                    self.line_width = self.width;
                }
                self.prev_char_is_whitespace = false;
            }
        }

        // Last line length is `line_width`, make room for cursor
        if self.current_line_length == self.line_width {
            temp_count += 1;
        }
        self.line_count = Some(temp_count);
    }
}
#[derive(Clone, Debug)]
pub(crate) struct InputLine {
    /// Input buffer
    buffer: Vec<char>,

    /// A cache that will allow us to quickly add
    /// characters to the buffer and update the number
    /// of lines needed to render with text wrapping.
    line_data: LineDataCache,
}

impl InputLine {
    pub(crate) fn new() -> InputLine {
        InputLine {
            buffer: Vec::with_capacity(512),
            line_data: LineDataCache::new(),
        }
    }

    /// Creates an InputLine from a buffer.
    pub(crate) fn from_buffer(buffer: Vec<char>) -> InputLine {
        InputLine {
            buffer,
            line_data: LineDataCache::new(),
        }
    }

    /// Returns pointer to InputLine's buffer
    pub(crate) fn get_buffer(&self) -> &[char] {
        &self.buffer
    }

    /**
     **    Functions to interface with InputLine::buffer
     **/

    /// Interface for Vec::get()
    pub(crate) fn get(&self, idx: usize) -> char {
        self.buffer[idx]
    }

    /// Interface for Vec::len()
    pub(crate) fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Interface for Vec::drain()
    pub(crate) fn drain<R>(&mut self, range: R) -> Drain<char>
    where
        R: RangeBounds<usize>,
    {
        self.line_data.set_dirty();
        self.buffer.drain(range)
    }

    /// Interface for Vec::remove()
    pub(crate) fn remove(&mut self, idx: usize) -> char {
        self.line_data.set_dirty();
        self.buffer.remove(idx)
    }

    /// Interface for Vec::insert()
    /// When the insertion is at the end of the buffer
    /// we can use the saved state to quickly calculate if
    /// we're moving to the next line, without fully recalculating.
    pub(crate) fn insert(&mut self, idx: usize, element: char) {
        self.buffer.insert(idx, element);
        if idx == self.buffer.len() - 1 {
            self.line_data.calculate_height(&self.buffer, idx);
        } else {
            self.line_data.set_dirty();
        }
    }

    /**
     **    End of InputLine::buffer interface
     **/

    /// Calculate hedight of the widget, taking nickname length into account. Only needed when
    /// buffer is wider than width and scrolling is off.
    pub(crate) fn calculate_height(&mut self, width: i32, nick_length: usize) -> usize {
        if self.line_data.is_dirty() || self.line_data.needs_resize(width, nick_length) {
            self.line_data.reset(width, nick_length);
            self.line_data.calculate_height(&self.buffer, 0);
        }
        self.line_data.get_line_count().unwrap()
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
    let slice: &[char] =
        &line[scroll as usize..min(line.len(), (scroll + (width - pos_x)) as usize)];
    termbox::print_chars(tb, pos_x, pos_y, colors.user_msg, slice.iter().copied());
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

pub(crate) struct CompletionRange {
    start_idx: usize,
    end_idx: usize,
}

fn draw_line_wrapped(
    tb: &mut Termbox,
    colors: &Colors,
    pos_x: i32,
    pos_y: i32,
    width: i32,
    cursor: i32,
    line: &InputLine,
    completion_range: &Option<CompletionRange>,
) {
    let mut col = pos_x;
    let mut line_num = 0;

    let mut cursor_xychar = (0, 0, ' ');
    let mut check_cursor = |char_idx: usize, cursor: i32, pos_x: i32, pos_y: i32, c: char| {
        if char_idx == cursor as usize {
            cursor_xychar = (pos_x, pos_y, c);
        }
    };
    let mut split_indices_iter = line.line_data.split_indices.iter().copied().peekable();
    for (char_idx, c) in line.buffer.iter().enumerate() {
        let mut style = colors.user_msg;
        // for autocompletion highlighting
        if let Some(completion_range) = completion_range {
            if char_idx >= completion_range.start_idx && char_idx < completion_range.end_idx {
                style = colors.completion;
            }
        }
        // If split_indices_iter yields we already know the indices for the start of each line. If it
        // does not then we just continue outputting on this line.
        if let Some(next_line_start) = split_indices_iter.peek() {
            if char_idx == *next_line_start as usize {
                // move to next line
                line_num += 1;
                // reset column
                col = 0;
                // move to the next line start index
                split_indices_iter.next();
            }
        }
        // Write out the character
        tb.change_cell(col, pos_y + line_num, *c, style.fg, style.bg);
        // Check if the cursor is on this character
        check_cursor(char_idx, cursor, col, pos_y + line_num, *c);
        col += 1;
    }

    // Cursor may be (probably) after all text
    if col == width {
        // render cursor on next line
        line_num += 1;
        col = 0;
    }
    check_cursor(line.buffer.len(), cursor, col, pos_y + line_num, ' ');

    tb.change_cell(
        cursor_xychar.0,
        cursor_xychar.1,
        cursor_xychar.2,
        colors.cursor.fg,
        colors.cursor.bg,
    );

    tb.set_cursor(Some(((cursor_xychar.0) as u16, (cursor_xychar.1) as u16)));
}

pub(crate) fn draw_line_autocomplete(
    original_buffer: &InputLine,
    insertion_point: usize,
    word_starts: usize,
    completions: &[String],
    current_completion: usize,
    tb: &mut Termbox,
    colors: &Colors,
    pos_x: i32,
    pos_y: i32,
    width: i32,
    cursor: i32,
    should_scroll: bool,
    scroll: Option<i32>,
) {
    let completion: &str = &completions[current_completion];

    let mut orig_buf_iter = original_buffer.get_buffer().iter().copied();
    let mut completion_iter = completion.chars();

    if should_scroll {
        let cursor_x_off = cursor - scroll.unwrap();
        let cursor_y_off = 0;

        // draw a placeholder for the cursor
        tb.change_cell(
            pos_x + cursor_x_off,
            pos_y + cursor_y_off,
            ' ',
            colors.user_msg.fg,
            colors.user_msg.bg,
        );

        let iter: utils::InsertIterator<char> =
            utils::insert_iter(&mut orig_buf_iter, &mut completion_iter, insertion_point);

        for (char_idx, char) in iter.enumerate() {
            let x_off;
            let y_off;
            let mut can_scroll = true;
            if should_scroll {
                let scroll = scroll.unwrap_or(0);
                x_off = (char_idx as i32) - scroll;
                y_off = 0;
                if char_idx >= ((scroll + width) as usize) {
                    break;
                }
                if char_idx < scroll as usize {
                    can_scroll = false
                }
            } else {
                x_off = char_idx as i32 % width;
                y_off = char_idx as i32 / width;
            }
            if can_scroll {
                if char_idx >= word_starts && char_idx < insertion_point + completion.len() {
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
        // TODO: Think of a better way to handle this inefficiency
        let iter: utils::InsertIterator<char> =
            utils::insert_iter(&mut orig_buf_iter, &mut completion_iter, insertion_point);

        let mut line = InputLine::from_buffer(iter.collect());
        line.calculate_height(width, pos_x as usize);
        let completion_range = CompletionRange {
            start_idx: word_starts,
            end_idx: (insertion_point + completion.len()),
        };
        draw_line_wrapped(
            tb,
            colors,
            pos_x,
            pos_y,
            width,
            cursor,
            &line,
            &Some(completion_range),
        );
    }
}

pub(crate) fn draw_line(
    tb: &mut Termbox,
    colors: &Colors,
    line: &InputLine,
    pos_x: i32,
    pos_y: i32,
    width: i32,
    cursor: i32,
    should_scroll: bool,
    scroll: Option<i32>,
    completion_range: Option<CompletionRange>,
) {
    if should_scroll || (line.len() as i32) < width - pos_x {
        draw_line_scroll(
            tb,
            colors,
            &line.buffer,
            pos_x,
            pos_y,
            width,
            cursor,
            scroll.unwrap_or(0),
        );
    } else {
        draw_line_wrapped(
            tb,
            colors,
            pos_x,
            pos_y,
            width,
            cursor,
            &line,
            &completion_range,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::InputLine;
    #[test]
    fn test_calculate_height() {
        let buffer: Vec<char> = String::from("012345").chars().collect();
        let mut line = InputLine::from_buffer(buffer);

        assert_eq!(line.calculate_height(6, 2), 2);

        // one extra line for the cursor
        assert_eq!(line.calculate_height(6, 0), 2);
        assert_eq!(line.calculate_height(3, 0), 3);

        line.insert(3, ' '); // 123 456
        assert_eq!(line.calculate_height(6, 0), 2);

        // Input containing lines that are longer than the width
        let buffer: Vec<char> = String::from("01 3456").chars().collect();
        let mut line = InputLine::from_buffer(buffer);
        assert_eq!(line.calculate_height(3, 0), 3);
        line.insert(line.len(), ' ');
        // "01 3456 "
        // Each line should be:
        // "01 "
        // "345"
        // "6 "
        assert_eq!(line.calculate_height(3, 0), 3);

        line.insert(line.len(), '8');
        line.insert(line.len(), '9');
        line.insert(line.len(), 'X');
        line.insert(line.len(), '1');
        line.insert(line.len(), '2');
        line.insert(line.len(), ' ');
        line.insert(line.len(), '4');
        // "01 3456 89X12 "
        // Each line should be:
        // "01 "
        // "345"
        // "6 "
        // "89X"
        // "12 "
        // "4"
        assert_eq!(line.calculate_height(3, 0), 6);

        // First line has no whitespaces, but whitespaces follow
        let buffer: Vec<char> = String::from("012345 78 X 12 34").chars().collect();
        let mut line = InputLine::from_buffer(buffer);
        // "0123"
        // "45 "
        // "78 X"
        // " 12 "
        // "34"
        assert_eq!(line.calculate_height(4, 0), 5);
    }
}
