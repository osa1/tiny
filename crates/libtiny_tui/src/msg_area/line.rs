use crate::config::{Colors, Style};
use crate::line_split::{LineDataCache, LineType};

use libtiny_wire::formatting::{Color, IrcFormatEvent, parse_irc_formatting};
use termbox_simple::{self, Termbox};
use unicode_width::UnicodeWidthChar;

/// A single line added to the widget. May be rendered as multiple lines on the
/// screen.
#[derive(Debug)]
pub(crate) struct Line {
    /// Line segments.
    segments: Vec<StyledString>,

    /// The segment we're currently extending.
    current_seg: StyledString,

    line_data: LineDataCache,
}

#[derive(Debug)]
struct StyledString {
    string: String,
    style: SegStyle,
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub(crate) enum SegStyle {
    /// A specific style. Useful when rendering IRC colors (which should look
    /// the same across color schemes).
    Fixed(Style),

    /// An index to nick colors. Note that the index should be larger than size
    /// of the color list, so make sure to use mod.
    NickColor(usize),

    // Rest of the styles are from the color scheme
    UserMsg,
    ErrMsg,
    Topic,
    Join,
    Part,
    Nick,
    Faded,
    Highlight,
    Timestamp,
}

impl StyledString {
    pub(crate) fn style(&self, colors: &Colors) -> Style {
        use SegStyle::*;
        match self.style {
            Fixed(style) => style,
            NickColor(idx) => Style {
                fg: u16::from(colors.nick[idx % colors.nick.len()]),
                bg: colors.user_msg.bg,
            },
            UserMsg => colors.user_msg,
            ErrMsg => colors.err_msg,
            Topic => colors.topic,
            Join => colors.join,
            Part => colors.part,
            Nick => colors.nick_change,
            Faded => colors.faded,
            Highlight => colors.highlight,
            Timestamp => colors.timestamp,
        }
    }
}

impl Default for StyledString {
    fn default() -> Self {
        StyledString {
            string: String::new(),
            style: SegStyle::UserMsg,
        }
    }
}

impl Line {
    pub(crate) fn new() -> Line {
        Line {
            segments: vec![],
            current_seg: StyledString::default(),
            line_data: LineDataCache::msg_line(0, None),
        }
    }

    pub(crate) fn set_type(&mut self, line_type: LineType) {
        self.line_data.set_line_type(line_type)
    }

    pub(crate) fn line_type(&self) -> LineType {
        self.line_data.line_type()
    }

    fn set_message_style(&mut self, style: SegStyle) {
        // Just update the last segment if it's empty
        if self.current_seg.string.is_empty() {
            self.current_seg.style = style;
        } else if self.current_seg.style != style {
            let seg = std::mem::replace(
                &mut self.current_seg,
                StyledString {
                    string: String::new(),
                    style,
                },
            );
            self.segments.push(seg);
        }
    }

    fn add_attr(&mut self, attr: u16) {
        if let SegStyle::Fixed(style) = self.current_seg.style {
            self.set_message_style(SegStyle::Fixed(Style {
                fg: style.fg | attr,
                bg: style.bg,
            }))
        } else {
            let style = SegStyle::Fixed(Style {
                fg: termbox_simple::TB_DEFAULT | attr,
                bg: termbox_simple::TB_DEFAULT,
            });
            self.set_message_style(style)
        }
    }

    /// Add the text to the line. The text can contain IRC formatting characters. The text should
    /// not contain other control characters like '\n' or '\r'.
    pub(crate) fn add_text(&mut self, str: &str, style: SegStyle) {
        self.add_text_inner(str, style, false);
    }

    pub(crate) fn add_independent_text(&mut self, str: &str, style: SegStyle) {
        self.add_text_inner(str, style, true);
    }

    fn add_text_inner(&mut self, str: &str, style: SegStyle, mut independent: bool) {
        self.set_message_style(style);

        for format_event in parse_irc_formatting(str) {
            match format_event {
                IrcFormatEvent::Bold => self.add_attr(termbox_simple::TB_BOLD),
                IrcFormatEvent::Italic => self.add_attr(termbox_simple::TB_ITALIC),
                IrcFormatEvent::Underline => self.add_attr(termbox_simple::TB_UNDERLINE),
                IrcFormatEvent::Strikethrough => self.add_attr(termbox_simple::TB_STRIKETHROUGH),
                IrcFormatEvent::Monospace => {
                    // TODO
                }
                IrcFormatEvent::Text(text) => {
                    if independent && let Some(ch) = text.chars().next() {
                        if ch.width() == Some(0) {
                            self.current_seg.string.push('◌');
                        }
                        independent = false;
                    }
                    self.current_seg.string.push_str(text);
                }
                IrcFormatEvent::Color { fg, bg } => {
                    let style = SegStyle::Fixed(Style {
                        fg: u16::from(irc_color_to_termbox(fg)),
                        bg: bg
                            .map(|bg| u16::from(irc_color_to_termbox(bg)))
                            .unwrap_or(termbox_simple::TB_DEFAULT),
                    });

                    self.set_message_style(style);
                }
                IrcFormatEvent::ReverseColor => {
                    if let SegStyle::Fixed(Style { fg, bg }) = self.current_seg.style {
                        self.set_message_style(SegStyle::Fixed(Style { fg: bg, bg: fg }));
                    }
                }
                // TODO: ResetColors should only reset the colors and not the formatting.
                // However we don't store the current formatting and we can't apply formatting to
                // a color from the current color scheme (e.g. `SegStyle::UserMsg`). For now reset
                // the colors and formatting.
                IrcFormatEvent::ResetColors | IrcFormatEvent::Reset => {
                    self.set_message_style(SegStyle::UserMsg);
                }
            }
        }

        self.line_data.set_dirty();
    }

    /// Add a single character to the line. The character should not be an IRC formatting character
    /// or other control characters like '\n' and '\r'.
    pub(crate) fn add_char(&mut self, char: char, style: SegStyle) {
        self.set_message_style(style);
        self.current_seg.string.push(char);
        self.line_data.set_dirty();
    }

    /// Calculates the number of lines that this line will be.
    /// The calculation is only done if the line_data is dirty or the window is resized.
    pub(crate) fn rendered_height(&mut self, width: i32) -> i32 {
        let msg_padding = self.line_type().msg_padding();
        if self.line_data.is_dirty() || self.line_data.needs_resize(width, 0, msg_padding) {
            self.line_data = LineDataCache::msg_line(width, msg_padding);
            let mut full_line = self
                .segments
                .iter()
                .flat_map(|s| s.string.chars())
                .chain(self.current_seg.string.chars());
            self.line_data.calculate_height(&mut full_line, 0);
        }
        self.line_data.get_line_count().unwrap() as i32
    }

    pub(crate) fn draw(
        &self,
        tb: &mut Termbox,
        colors: &Colors,
        pos_x: i32,
        pos_y: i32,
        first_line: i32,
        height: i32,
    ) {
        let mut col = pos_x;
        let mut line_num = 0;
        let mut base = None;
        let mut splits = self.line_data.get_splits().iter().copied().peekable();
        let mut chars = self
            .segments
            .iter()
            .chain(std::iter::once(&self.current_seg))
            .flat_map(|seg| {
                let style = seg.style(colors);
                seg.string.chars().map(move |c| (c, style))
            })
            .enumerate()
            .peekable();

        while let Some((char_idx, (c, sty))) = chars.next() {
            if splits.peek().copied() == Some(char_idx as i32) {
                splits.next();
                line_num += 1;
                if line_num >= height {
                    break;
                }
                col = pos_x + self.line_data.new_line_offset();
                base = None;
                // Preserve the existing treatment of whitespace at a wrap, except
                // when it is the base for a following combining scalar.
                if c.is_whitespace()
                    && !chars
                        .peek()
                        .is_some_and(|(_, (next, _))| UnicodeWidthChar::width(*next) == Some(0))
                {
                    continue;
                }
            }
            let width = UnicodeWidthChar::width(c).unwrap_or(0) as i32;
            let y = pos_y + line_num;
            if width == 0 {
                if base.is_none() {
                    if line_num >= first_line {
                        tb.change_cell(col, y, '◌', sty.fg, sty.bg);
                    }
                    base = Some(col);
                    col += 1;
                }
                if line_num >= first_line {
                    tb.append_combining(base.unwrap(), y, c, sty.fg, sty.bg);
                }
            } else {
                let drawn_width = if self.line_data.display_width(line_num) == 1 {
                    1
                } else {
                    width
                };
                let displayed = if drawn_width < width { '�' } else { c };
                if line_num >= first_line {
                    tb.change_cell(col, y, displayed, sty.fg, sty.bg);
                }
                base = Some(col);
                col += drawn_width;
            }
        }
    }
}

////////////////////////////////////////////////////////////////////////////////

// Termbox colors: http://www.calmar.ws/vim/256-xterm-24bit-rgb-color-chart.html
//                 (alternatively just run `cargo run --example colors`)
fn irc_color_to_termbox(irc_color: Color) -> u8 {
    match irc_color {
        Color::White => 255,
        Color::Black => 16,
        Color::Blue => 21,
        Color::Green => 46,
        Color::Red => 196,
        Color::Brown => 88,
        Color::Magenta => 93,
        Color::Orange => 210,
        Color::Yellow => 228,
        Color::LightGreen => 154,
        Color::Cyan => 75,
        Color::LightCyan => 39,
        Color::LightBlue => 38,
        Color::Pink => 129,
        Color::Grey => 243,
        Color::LightGrey => 249,
        Color::Default => termbox_simple::TB_DEFAULT as u8,
        Color::Ansi(ansi_color) => ansi_color,
    }
}

////////////////////////////////////////////////////////////////////////////////

#[test]
fn height_test_1() {
    let mut line = Line::new();
    line.add_text("a b c d e", SegStyle::UserMsg);
    assert_eq!(line.rendered_height(1), 9);
    assert_eq!(line.rendered_height(2), 5);
    assert_eq!(line.rendered_height(3), 3);
    assert_eq!(line.rendered_height(4), 3);
    assert_eq!(line.rendered_height(5), 2);
    assert_eq!(line.rendered_height(6), 2);
    assert_eq!(line.rendered_height(7), 2);
    assert_eq!(line.rendered_height(8), 2);
    assert_eq!(line.rendered_height(9), 1);
}

#[test]
fn height_test_2() {
    let mut line = Line::new();
    line.add_text("ab c d e", SegStyle::UserMsg);
    assert_eq!(line.rendered_height(1), 8);
    assert_eq!(line.rendered_height(2), 4);
    assert_eq!(line.rendered_height(3), 3);
    assert_eq!(line.rendered_height(4), 2);
    assert_eq!(line.rendered_height(5), 2);
    assert_eq!(line.rendered_height(6), 2);
    assert_eq!(line.rendered_height(7), 2);
    assert_eq!(line.rendered_height(8), 1);
}

#[test]
fn height_test_3() {
    let mut line = Line::new();
    line.add_text("ab cd e", SegStyle::UserMsg);
    assert_eq!(line.rendered_height(1), 7);
    assert_eq!(line.rendered_height(2), 4);
    assert_eq!(line.rendered_height(3), 3);
    assert_eq!(line.rendered_height(4), 2);
    assert_eq!(line.rendered_height(5), 2);
    assert_eq!(line.rendered_height(6), 2);
    assert_eq!(line.rendered_height(7), 1);
}

#[test]
fn height_test_4() {
    let mut line = Line::new();
    line.add_text("ab cde", SegStyle::UserMsg);
    assert_eq!(line.rendered_height(1), 6);
    assert_eq!(line.rendered_height(2), 4);
    assert_eq!(line.rendered_height(3), 2);
    assert_eq!(line.rendered_height(4), 2);
    assert_eq!(line.rendered_height(5), 2);
    assert_eq!(line.rendered_height(6), 1);
}

#[test]
fn height_test_5() {
    use std::fs::File;
    use std::io::Read;

    let text: String = {
        let mut text = String::new();
        let mut single_line = String::new();
        let mut file = File::open("test/lipsum.txt").unwrap();
        file.read_to_string(&mut text).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 102); // make sure we did it right
        for (line_idx, line) in lines.iter().enumerate() {
            single_line.push_str(line);
            if line_idx != lines.len() - 1 {
                single_line.push(' ');
            }
        }
        single_line
    };

    let mut line = Line::new();
    line.add_text(&text, SegStyle::UserMsg);
    // lipsum.txt has 1160 words in it. each line should contain at most one
    // word so we should have 1160 lines.
    assert_eq!(line.rendered_height(80), 102);
}

#[test]
fn align_test() {
    let mut line = Line::new();
    line.set_type(LineType::AlignedMsg { msg_padding: 1 });
    /*
    123
     45
     67
     8
    */
    line.add_text("12345678", SegStyle::UserMsg);

    assert_eq!(line.rendered_height(3), 4);
}

#[test]
fn unicode_drawing_wraps_by_columns_and_keeps_styled_marks() {
    let mut line = Line::new();
    line.add_text("Aか", SegStyle::UserMsg);
    line.add_text("\u{3099}日B", SegStyle::Highlight);
    assert_eq!(line.rendered_height(3), 2);
    let mut tb = Termbox::init_test(3, 2);
    line.draw(&mut tb, &Colors::default(), 0, 0, 0, 2);
    tb.present();
    let cells = tb.get_front_buffer().cells;
    assert_eq!(
        (cells[0].ch, cells[1].ch, cells[3].ch, cells[5].ch),
        ('A', 'か', '日', 'B')
    );
    let output = String::from_utf8(tb.take_test_output()).unwrap();
    assert!(output.contains('か'));
    assert!(output.contains('\u{3099}'));
    assert!(output.contains('日'));
}

#[test]
fn unicode_drawing_handles_narrow_and_scrolled_lines() {
    let mut line = Line::new();
    line.add_text("\u{3099}日B", SegStyle::UserMsg);
    assert_eq!(line.rendered_height(1), 3);
    let mut tb = Termbox::init_test(1, 3);
    line.draw(&mut tb, &Colors::default(), 0, 0, 1, 3);
    tb.present();
    let cells = tb.get_front_buffer().cells;
    assert_eq!((cells[0].ch, cells[1].ch, cells[2].ch), (' ', '�', 'B'));

    let mut tb = Termbox::init_test(1, 3);
    line.draw(&mut tb, &Colors::default(), 0, 0, 0, 3);
    tb.present();
    assert_eq!(tb.get_front_buffer().cells[0].ch, '◌');
    assert!(
        String::from_utf8(tb.take_test_output())
            .unwrap()
            .contains('\u{3099}')
    );
}
