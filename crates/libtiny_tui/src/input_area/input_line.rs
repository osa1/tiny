use crate::{config::Colors, line_split::LineDataCache, termbox, utils};
use std::{cmp::min, ops::RangeBounds, vec::Drain};
use termbox_simple::Termbox;

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
            line_data: LineDataCache::input_line(0, 0),
        }
    }

    /// Creates an InputLine from a buffer.
    pub(crate) fn from_buffer(buffer: Vec<char>) -> InputLine {
        InputLine {
            buffer,
            line_data: LineDataCache::input_line(0, 0),
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
            self.line_data
                .calculate_height(&mut self.buffer.iter().copied(), idx);
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
        if self.line_data.is_dirty() || self.line_data.needs_resize(width, nick_length, None) {
            self.line_data = LineDataCache::input_line(width, nick_length);
            self.line_data
                .calculate_height(&mut self.buffer.iter().copied(), 0);
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
    let mut split_indices_iter = line.line_data.get_splits().iter().copied().peekable();
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
            line,
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
