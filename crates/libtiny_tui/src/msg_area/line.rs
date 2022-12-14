use crate::config::{Colors, Style};
use crate::line_split::{LineDataCache, LineType};

use libtiny_wire::formatting::{parse_irc_formatting, Color, IrcFormatEvent};
use termbox_simple::{self, Termbox};

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

    fn add_text_inner(&mut self, str: &str) {
        for format_event in parse_irc_formatting(str) {
            match format_event {
                IrcFormatEvent::Bold
                | IrcFormatEvent::Italic
                | IrcFormatEvent::Underline
                | IrcFormatEvent::Strikethrough
                | IrcFormatEvent::Monospace => {
                    // TODO
                }
                IrcFormatEvent::Text(text) => {
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
                IrcFormatEvent::Reset => {
                    self.set_message_style(SegStyle::UserMsg);
                }
            }
        }
    }

    pub(crate) fn add_text(&mut self, str: &str, style: SegStyle) {
        self.set_message_style(style);
        self.add_text_inner(str)
    }

    pub(crate) fn add_char(&mut self, char: char, style: SegStyle) {
        self.set_message_style(style);
        self.current_seg.string.push(char);
    }

    pub(crate) fn force_recalculation(&mut self) {
        self.line_data.set_dirty()
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
        let mut char_idx = 0;
        let mut split_indices_iter = self.line_data.get_splits().iter().copied().peekable();

        for seg in self
            .segments
            .iter()
            .chain(std::iter::once(&self.current_seg))
        {
            let sty = seg.style(colors);
            for c in seg.string.chars() {
                // If split_indices_iter yields we already know the indices for the start of each line. If it
                // does not then we just continue outputting on this line.
                if let Some(next_line_start) = split_indices_iter.peek() {
                    if char_idx == *next_line_start as usize {
                        // Move to next line
                        line_num += 1;
                        if line_num >= height {
                            break;
                        }
                        // Reset column
                        col = pos_x + self.line_data.new_line_offset();

                        // Move to the next line start index
                        split_indices_iter.next();
                        // Don't draw whitespaces
                        if c.is_whitespace() {
                            char_idx += 1;
                            continue;
                        }
                    }
                }
                // Write out the character
                if line_num >= first_line {
                    tb.change_cell(col, pos_y + line_num, c, sty.fg, sty.bg);
                }
                col += 1;
                char_idx += 1;
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
    line.add_text_inner("12345678");

    assert_eq!(line.rendered_height(3), 4);
}
