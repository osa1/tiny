//! Some utilities for termbox

use crate::config::Style;
use termbox_simple::Termbox;
use unicode_width::UnicodeWidthChar;

/// Display columns occupied by one independent piece of text.
pub(crate) fn text_width(text: &str) -> usize {
    let mut has_base = false;
    text.chars()
        .map(|ch| {
            let width = UnicodeWidthChar::width(ch).unwrap_or(1);
            if width == 0 {
                if has_base {
                    0
                } else {
                    has_base = true;
                    1
                }
            } else {
                has_base = true;
                width
            }
        })
        .sum()
}

/// Draw one independent text, preserving the style of each combining character.
pub(crate) fn print_styled_chars<C>(tb: &mut Termbox, mut pos_x: i32, pos_y: i32, chars: C) -> i32
where
    C: Iterator<Item = (char, Style)>,
{
    let mut base = None;
    for (ch, style) in chars {
        let width = UnicodeWidthChar::width(ch).unwrap_or(1);
        if width == 0 {
            let x = match base {
                Some(x) => x,
                None => {
                    let x = pos_x;
                    tb.change_cell(x, pos_y, '◌', style.fg, style.bg);
                    pos_x += 1;
                    base = Some(x);
                    x
                }
            };
            tb.append_combining(x, pos_y, ch, style.fg, style.bg);
        } else {
            tb.change_cell(pos_x, pos_y, ch, style.fg, style.bg);
            base = Some(pos_x);
            pos_x += if width == 2 && pos_x == tb.width() - 1 {
                1
            } else {
                width as i32
            };
        }
    }
    pos_x
}

pub(crate) fn print_chars<C>(
    tb: &mut Termbox,
    pos_x: i32,
    pos_y: i32,
    style: Style,
    chars: C,
) -> i32
where
    C: Iterator<Item = char>,
{
    print_styled_chars(tb, pos_x, pos_y, chars.map(|ch| (ch, style)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_combining_and_right_edge() {
        let style = Style { fg: 1, bg: 0 };
        let mut tb = Termbox::init_test(5, 1);
        assert_eq!(
            print_chars(&mut tb, 0, 0, style, "A日B\u{3099}日".chars()),
            5
        );
        tb.present();
        let cells = tb.get_front_buffer().cells;
        assert_eq!(
            (cells[0].ch, cells[1].ch, cells[3].ch, cells[4].ch),
            ('A', '日', 'B', '�')
        );
        assert!(
            String::from_utf8(tb.take_test_output())
                .unwrap()
                .contains("B\u{3099}")
        );
    }

    #[test]
    fn leading_marks_are_independent_and_right_edge_marks_remain_attached() {
        let style = Style { fg: 1, bg: 0 };
        let mut tb = Termbox::init_test(3, 1);
        print_chars(&mut tb, 0, 0, style, "A".chars());
        assert_eq!(
            print_chars(&mut tb, 1, 0, style, "\u{3099}B\u{3099}".chars()),
            3
        );
        tb.present();
        let output = String::from_utf8(tb.take_test_output()).unwrap();
        assert!(output.contains("◌\u{3099}"));
        assert!(output.contains("B\u{3099}"));
        assert_eq!(text_width("\u{3099}日A\u{3099}"), 4);

        let mut narrow = Termbox::init_test(1, 1);
        print_chars(&mut narrow, 0, 0, style, "日\u{3099}".chars());
        narrow.present();
        assert_eq!(narrow.get_front_buffer().cells[0].ch, '�');
        assert!(
            String::from_utf8(narrow.take_test_output())
                .unwrap()
                .contains("�\u{3099}")
        );
    }
}
