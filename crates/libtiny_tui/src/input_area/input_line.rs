use crate::{config::Colors, line_split::LineDataCache};
use std::{ops::RangeBounds, vec::Drain};
use termbox_simple::Termbox;
use unicode_width::UnicodeWidthChar;

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

    /*
     *    Functions to interface with InputLine::buffer
     */

    /// Interface for Vec::get()
    pub(crate) fn get(&self, idx: usize) -> char {
        self.buffer[idx]
    }

    /// Interface for Vec::len()
    pub(crate) fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Interface for Vec::drain()
    pub(crate) fn drain<R>(&mut self, range: R) -> Drain<'_, char>
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
        if idx == self.buffer.len() - 1 && !self.line_data.is_dirty() {
            self.line_data
                .calculate_height(&mut self.buffer.iter().copied(), idx);
        } else {
            self.line_data.set_dirty();
        }
    }

    /*
     *    End of InputLine::buffer interface
     */

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

// Both scrolling and wrapping use scalar indices but advance in terminal columns.
fn draw_input<I: Iterator<Item = char>>(
    tb: &mut Termbox,
    colors: &Colors,
    line: I,
    line_len: usize,
    pos_x: i32,
    pos_y: i32,
    width: i32,
    cursor: i32,
    scroll: Option<i32>,
    splits: &[i32],
    completion: &Option<CompletionRange>,
) {
    let right = width.min(tb.width());
    if right <= 0 || pos_y < 0 || pos_y >= tb.height() {
        return;
    }
    let start = scroll.unwrap_or(0).max(0) as usize;
    let start = start.min(line_len);
    let cursor = (cursor.max(0) as usize).min(line_len);
    let mut x = pos_x.max(0).min(right - 1);
    let mut y = pos_y;
    let mut base = None;
    let mut cursor_xy = None;
    let mut split_iter = splits.iter().copied().peekable();
    let mut complete = true;

    for (idx, ch) in line.enumerate().skip(start) {
        if scroll.is_none() {
            while split_iter
                .peek()
                .is_some_and(|&split| split as usize == idx)
            {
                split_iter.next();
                x = 0;
                y += 1;
                base = None;
            }
        }
        if y >= tb.height() {
            complete = false;
            break;
        }
        let style = if completion
            .as_ref()
            .is_some_and(|range| idx >= range.start_idx && idx < range.end_idx)
        {
            colors.completion
        } else {
            colors.user_msg
        };
        let char_width = UnicodeWidthChar::width(ch).unwrap_or(0) as i32;
        if char_width == 0 {
            if base.is_none() {
                if x >= right {
                    complete = false;
                    break;
                }
                tb.change_cell(x, y, '◌', style.fg, style.bg);
                base = Some((x, y));
                x += 1;
            }
            let (bx, by) = base.unwrap();
            if idx == cursor {
                cursor_xy = Some((bx, by));
            }
            tb.append_combining(bx, by, ch, style.fg, style.bg);
            continue;
        }
        let available = right - x;
        let row_width = if y == pos_y {
            right - pos_x.max(0)
        } else {
            right
        };
        if available <= 0 || (char_width > available && row_width > 1) {
            complete = false;
            break;
        }
        let displayed = if char_width > available { '�' } else { ch };
        if idx == cursor {
            cursor_xy = Some((x, y));
        }
        tb.change_cell(x, y, displayed, style.fg, style.bg);
        base = Some((x, y));
        x += if displayed == '�' && char_width > available {
            1
        } else {
            char_width
        };
    }

    if cursor == line_len && complete {
        if scroll.is_none() && x >= right {
            x = 0;
            y += 1;
        }
        cursor_xy = Some((x.min(right - 1), y.min(tb.height() - 1)));
    }
    let (cx, cy) = cursor_xy.unwrap_or((x.min(right - 1), y.min(tb.height() - 1)));
    // Styling rather than replacing the cell preserves any combining marks on it.
    if cursor >= line_len && complete && x < right {
        tb.change_cell(cx, cy, ' ', colors.cursor.fg, colors.cursor.bg);
    } else {
        tb.set_cell_attributes(cx, cy, colors.cursor.fg, colors.cursor.bg);
    }
    tb.set_cursor(Some((cx as u16, cy as u16)));
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
    completion_range: &Option<CompletionRange>,
) {
    draw_input(
        tb,
        colors,
        line.iter().copied(),
        line.len(),
        pos_x,
        pos_y,
        width,
        cursor,
        Some(scroll),
        &[],
        completion_range,
    );
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
    draw_input(
        tb,
        colors,
        line.buffer.iter().copied(),
        line.buffer.len(),
        pos_x,
        pos_y,
        width,
        cursor,
        None,
        line.line_data.get_splits(),
        completion_range,
    );
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
    let completion = &completions[current_completion];
    let original = original_buffer.get_buffer();
    let (before, after) = original.split_at(insertion_point);
    let chars = before
        .iter()
        .copied()
        .chain(completion.chars())
        .chain(after.iter().copied());
    let completion_len = completion.chars().count();
    let completion_range = Some(CompletionRange {
        start_idx: word_starts,
        end_idx: insertion_point + completion_len,
    });
    let mut layout = LineDataCache::input_line(width, pos_x.max(0) as usize);
    if !should_scroll {
        layout.calculate_height(chars.clone(), 0);
    }
    draw_input(
        tb,
        colors,
        chars,
        original.len() + completion_len,
        pos_x,
        pos_y,
        width,
        cursor,
        if should_scroll {
            Some(scroll.unwrap_or(0))
        } else {
            None
        },
        layout.get_splits(),
        &completion_range,
    );
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
    if should_scroll {
        draw_line_scroll(
            tb,
            colors,
            &line.buffer,
            pos_x,
            pos_y,
            width,
            cursor,
            scroll.unwrap_or(0),
            &completion_range,
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
    use super::{CompletionRange, draw_line, draw_line_autocomplete};
    use crate::config::{Colors, Style};
    use termbox_simple::Termbox;

    fn colors() -> Colors {
        let mut colors = Colors::default();
        colors.user_msg = Style { fg: 1, bg: 2 };
        colors.completion = Style { fg: 3, bg: 4 };
        colors.cursor = Style { fg: 5, bg: 6 };
        colors
    }

    fn wrapped(text: &str, width: i32, nick: i32, cursor: i32) -> Termbox {
        let mut tb = Termbox::init_test(width as u16, 6);
        let mut line = InputLine::from_buffer(text.chars().collect());
        line.calculate_height(width, nick as usize);
        draw_line(
            &mut tb,
            &colors(),
            &line,
            nick,
            0,
            width,
            cursor,
            false,
            None,
            None,
        );
        tb.present();
        tb
    }

    #[test]
    fn wrapped_unicode_columns_and_cursor() {
        let tb = wrapped("A日中한B", 5, 1, 2);
        let buf = tb.get_front_buffer();
        assert_eq!(buf.cells[1].ch, 'A');
        assert_eq!(buf.cells[2].ch, '日');
        assert_eq!(buf.cells[5].ch, '中');
        assert_eq!(buf.cells[7].ch, '한');
        assert_eq!(buf.cells[9].ch, 'B');
        assert_eq!(buf.cells[5].fg, colors().cursor.fg);

        let tb = wrapped("A日B", 4, 0, 3);
        let buf = tb.get_front_buffer();
        assert_eq!(buf.cells[0].ch, 'A');
        assert_eq!(buf.cells[1].ch, '日');
        assert_eq!(buf.cells[3].ch, 'B');
        assert_eq!(buf.cells[4].fg, colors().cursor.fg);

        let tb = wrapped("A日B", 2, 0, 1);
        let buf = tb.get_front_buffer();
        assert_eq!(buf.cells[0].ch, 'A');
        assert_eq!(buf.cells[2].ch, '日');
        assert_eq!(buf.cells[2].fg, colors().cursor.fg);
        assert_eq!(buf.cells[4].ch, 'B');

        let tb = wrapped("日B", 1, 0, 1);
        let buf = tb.get_front_buffer();
        assert_eq!(buf.cells[0].ch, '�');
        assert_eq!(buf.cells[1].ch, 'B');
        assert_eq!(buf.cells[1].fg, colors().cursor.fg);
    }

    #[test]
    fn combining_marks_survive_cursor_and_style_boundaries() {
        let mut tb = Termbox::init_test(2, 4);
        let mut line = InputLine::from_buffer("Aか\u{3099}B".chars().collect());
        line.calculate_height(2, 0);
        draw_line(
            &mut tb,
            &colors(),
            &line,
            0,
            0,
            2,
            2,
            false,
            None,
            Some(CompletionRange {
                start_idx: 2,
                end_idx: 3,
            }),
        );
        tb.present();
        let output = String::from_utf8_lossy(&tb.take_test_output()).into_owned();
        assert!(output.contains('\u{3099}'));
        let buf = tb.get_front_buffer();
        assert_eq!(buf.cells[2].ch, 'か');
        assert_eq!(buf.cells[2].fg, colors().cursor.fg);
        assert_eq!(buf.cells[4].ch, 'B');

        let mut tb = wrapped("A\u{3099}B", 1, 0, 1);
        assert!(String::from_utf8_lossy(&tb.take_test_output()).contains('\u{3099}'));
        assert_eq!(tb.get_front_buffer().cells[0].ch, 'A');

        let mut tb = wrapped("\u{3099}A", 3, 0, 0);
        assert!(String::from_utf8_lossy(&tb.take_test_output()).contains('\u{3099}'));
        assert_eq!(tb.get_front_buffer().cells[0].ch, '◌');
    }

    #[test]
    fn scrolling_uses_scalar_indices_and_column_widths() {
        let mut tb = Termbox::init_test(5, 2);
        let line = InputLine::from_buffer("A日中한B".chars().collect());
        draw_line(&mut tb, &colors(), &line, 0, 0, 5, 2, true, Some(1), None);
        tb.present();
        let buf = tb.get_front_buffer();
        assert_eq!(buf.cells[0].ch, '日');
        assert_eq!(buf.cells[2].ch, '中');
        assert_eq!(buf.cells[2].fg, colors().cursor.fg);
        assert_eq!(buf.cells[4].ch, ' ');

        let mut tb = Termbox::init_test(1, 2);
        draw_line(&mut tb, &colors(), &line, 0, 0, 1, 1, true, Some(1), None);
        tb.present();
        assert_eq!(tb.get_front_buffer().cells[0].ch, '�');
    }

    #[test]
    fn completion_preview_inserts_scalars_and_styles_both_modes() {
        let original = InputLine::from_buffer("A日B".chars().collect());
        let candidates = vec!["中한".to_owned(), "語".to_owned()];
        for scroll in [false, true] {
            for (candidate, expected) in [(0, '中'), (1, '語')] {
                let mut tb = Termbox::init_test(10, 3);
                draw_line_autocomplete(
                    &original,
                    2,
                    1,
                    &candidates,
                    candidate,
                    &mut tb,
                    &colors(),
                    0,
                    0,
                    10,
                    0,
                    scroll,
                    Some(0),
                );
                tb.present();
                let buf = tb.get_front_buffer();
                assert_eq!(buf.cells[1].ch, '日');
                assert_eq!(buf.cells[3].ch, expected);
                assert_eq!(buf.cells[1].fg, colors().completion.fg);
                assert_eq!(buf.cells[3].fg, colors().completion.fg);
                let b = if candidate == 0 { 7 } else { 5 };
                assert_eq!(buf.cells[b].ch, 'B');
                assert_eq!(buf.cells[b].fg, colors().user_msg.fg);
            }
        }
    }
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

    #[test]
    fn appended_input_matches_recalculation_after_edits() {
        for width in [1, 2, 3, 5, 8] {
            let mut line = InputLine::new();
            line.calculate_height(width, 0);
            for ch in "A日か\u{3099} 本 B中 한".chars() {
                line.insert(line.len(), ch);
                let mut fresh = InputLine::from_buffer(line.get_buffer().to_vec());
                assert_eq!(
                    line.calculate_height(width, 0),
                    fresh.calculate_height(width, 0)
                );
                assert_eq!(line.line_data.get_splits(), fresh.line_data.get_splits());
            }
            line.remove(1);
            line.insert(line.len(), '語');
            let mut fresh = InputLine::from_buffer(line.get_buffer().to_vec());
            assert_eq!(
                line.calculate_height(width, 0),
                fresh.calculate_height(width, 0)
            );
            assert_eq!(line.line_data.get_splits(), fresh.line_data.get_splits());
        }
    }
}
