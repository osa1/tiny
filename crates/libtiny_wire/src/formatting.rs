//! Implements parsing IRC formatting characters. Reference:
//! <https://modern.ircdocs.horse/formatting.html>

#![allow(clippy::single_match)]

const CHAR_BOLD: char = '\x02';
const CHAR_ITALIC: char = '\x1D';
const CHAR_UNDERLINE: char = '\x1F';
const CHAR_STRIKETHROUGH: char = '\x1E';
const CHAR_MONOSPACE: char = '\x11';
const CHAR_COLOR: char = '\x03';
const CHAR_HEX_COLOR: char = '\x04';
const CHAR_REVERSE_COLOR: char = '\x16';
const CHAR_RESET: char = '\x0F';

static TAB_STR: &str = "        ";

#[derive(Debug, PartialEq, Eq)]
pub enum IrcFormatEvent<'a> {
    Text(&'a str),

    Bold,
    Italic,
    Underline,
    Strikethrough,
    Monospace,

    Color {
        fg: Color,
        bg: Option<Color>,
    },

    /// Reverse current background and foreground
    ReverseColor,

    /// Reset formatting to the default
    Reset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    White,
    Black,
    Blue,
    Green,
    Red,
    Brown,
    Magenta,
    Orange,
    Yellow,
    LightGreen,
    Cyan,
    LightCyan,
    LightBlue,
    Pink,
    Grey,
    LightGrey,
    Default,
    Ansi(u8),
}

impl Color {
    fn from_code(code: u8) -> Self {
        match code {
            0 => Color::White,
            1 => Color::Black,
            2 => Color::Blue,
            3 => Color::Green,
            4 => Color::Red,
            5 => Color::Brown,
            6 => Color::Magenta,
            7 => Color::Orange,
            8 => Color::Yellow,
            9 => Color::LightGreen,
            10 => Color::Cyan,
            11 => Color::LightCyan,
            12 => Color::LightBlue,
            13 => Color::Pink,
            14 => Color::Grey,
            15 => Color::LightGrey,
            16 => Color::Ansi(52),
            17 => Color::Ansi(94),
            18 => Color::Ansi(100),
            19 => Color::Ansi(58),
            20 => Color::Ansi(22),
            21 => Color::Ansi(29),
            22 => Color::Ansi(23),
            23 => Color::Ansi(24),
            24 => Color::Ansi(17),
            25 => Color::Ansi(54),
            26 => Color::Ansi(53),
            27 => Color::Ansi(89),
            28 => Color::Ansi(88),
            29 => Color::Ansi(130),
            30 => Color::Ansi(142),
            31 => Color::Ansi(64),
            32 => Color::Ansi(28),
            33 => Color::Ansi(35),
            34 => Color::Ansi(30),
            35 => Color::Ansi(25),
            36 => Color::Ansi(18),
            37 => Color::Ansi(91),
            38 => Color::Ansi(90),
            39 => Color::Ansi(125),
            40 => Color::Ansi(124),
            41 => Color::Ansi(166),
            42 => Color::Ansi(184),
            43 => Color::Ansi(106),
            44 => Color::Ansi(34),
            45 => Color::Ansi(49),
            46 => Color::Ansi(37),
            47 => Color::Ansi(33),
            48 => Color::Ansi(19),
            49 => Color::Ansi(129),
            50 => Color::Ansi(127),
            51 => Color::Ansi(161),
            52 => Color::Ansi(196),
            53 => Color::Ansi(208),
            54 => Color::Ansi(226),
            55 => Color::Ansi(154),
            56 => Color::Ansi(46),
            57 => Color::Ansi(86),
            58 => Color::Ansi(51),
            59 => Color::Ansi(75),
            60 => Color::Ansi(21),
            61 => Color::Ansi(171),
            62 => Color::Ansi(201),
            63 => Color::Ansi(198),
            64 => Color::Ansi(203),
            65 => Color::Ansi(215),
            66 => Color::Ansi(227),
            67 => Color::Ansi(191),
            68 => Color::Ansi(83),
            69 => Color::Ansi(122),
            70 => Color::Ansi(87),
            71 => Color::Ansi(111),
            72 => Color::Ansi(63),
            73 => Color::Ansi(177),
            74 => Color::Ansi(207),
            75 => Color::Ansi(205),
            76 => Color::Ansi(217),
            77 => Color::Ansi(223),
            78 => Color::Ansi(229),
            79 => Color::Ansi(193),
            80 => Color::Ansi(157),
            81 => Color::Ansi(158),
            82 => Color::Ansi(159),
            83 => Color::Ansi(153),
            84 => Color::Ansi(147),
            85 => Color::Ansi(183),
            86 => Color::Ansi(219),
            87 => Color::Ansi(212),
            88 => Color::Ansi(16),
            89 => Color::Ansi(233),
            90 => Color::Ansi(235),
            91 => Color::Ansi(237),
            92 => Color::Ansi(239),
            93 => Color::Ansi(241),
            94 => Color::Ansi(244),
            95 => Color::Ansi(247),
            96 => Color::Ansi(250),
            97 => Color::Ansi(254),
            98 => Color::Ansi(231),
            _ => Color::Default,
        }
    }
}

struct FormatEventParser<'a> {
    str: &'a str,

    /// Current index in `str`. We maintain indices to be able to extract slices from `str`.
    cursor: usize,
}

impl<'a> FormatEventParser<'a> {
    fn new(str: &'a str) -> Self {
        Self { str, cursor: 0 }
    }

    fn peek(&self) -> Option<char> {
        self.str[self.cursor..].chars().next()
    }

    fn next(&mut self) -> Option<char> {
        let next = self.str[self.cursor..].chars().next();
        if let Some(char) = next {
            self.cursor += char.len_utf8();
        }
        next
    }

    fn bump(&mut self, amt: usize) {
        self.cursor += amt;
    }

    fn parse_text(&mut self) -> &'a str {
        let cursor = self.cursor;
        while let Some(next) = self.next() {
            if is_irc_format_char(next) || next.is_ascii_control() {
                self.cursor -= 1;
                return &self.str[cursor..self.cursor];
            }
        }
        &self.str[cursor..]
    }

    /// Parse a color code. Expects the color code prefix ('\x03') to be consumed. Does not
    /// increment the cursor when result is `None`.
    fn parse_color(&mut self) -> Option<(Color, Option<Color>)> {
        match self.parse_color_code() {
            None => None,
            Some(fg) => {
                if let Some(char) = self.peek() {
                    if char == ',' {
                        let cursor = self.cursor;
                        self.bump(1); // consume ','
                        match self.parse_color_code() {
                            None => {
                                // comma was not part of the color code, revert the cursor
                                self.cursor = cursor;
                                Some((fg, None))
                            }
                            Some(bg) => Some((fg, Some(bg))),
                        }
                    } else {
                        Some((fg, None))
                    }
                } else {
                    Some((fg, None))
                }
            }
        }
    }

    /// Parses at least one, at most two digits. Does not increment the cursor when result is `None`.
    fn parse_color_code(&mut self) -> Option<Color> {
        fn to_dec(ch: char) -> Option<u8> {
            ch.to_digit(10).map(|c| c as u8)
        }

        let c1_char = self.peek()?;
        let c1_digit = match to_dec(c1_char) {
            None => {
                return None;
            }
            Some(c1_digit) => {
                self.bump(1); // consume digit
                c1_digit
            }
        };

        match self.peek() {
            None => Some(Color::from_code(c1_digit)),
            Some(c2) => match to_dec(c2) {
                None => Some(Color::from_code(c1_digit)),
                Some(c2_digit) => {
                    self.bump(1); // consume digit
                    Some(Color::from_code(c1_digit * 10 + c2_digit))
                }
            },
        }
    }

    fn skip_hex_code(&mut self) {
        // rrggbb
        for _ in 0..6 {
            // Use `next` here to avoid incrementing cursor too much
            let _ = self.next();
        }
    }
}

/// Is the character start of an IRC formatting char?
fn is_irc_format_char(c: char) -> bool {
    matches!(
        c,
        CHAR_BOLD
            | CHAR_ITALIC
            | CHAR_UNDERLINE
            | CHAR_STRIKETHROUGH
            | CHAR_MONOSPACE
            | CHAR_COLOR
            | CHAR_HEX_COLOR
            | CHAR_REVERSE_COLOR
            | CHAR_RESET
    )
}

impl<'a> Iterator for FormatEventParser<'a> {
    type Item = IrcFormatEvent<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let next = match self.peek() {
                None => return None,
                Some(next) => next,
            };

            match next {
                CHAR_BOLD => {
                    self.bump(1);
                    return Some(IrcFormatEvent::Bold);
                }

                CHAR_ITALIC => {
                    self.bump(1);
                    return Some(IrcFormatEvent::Italic);
                }

                CHAR_UNDERLINE => {
                    self.bump(1);
                    return Some(IrcFormatEvent::Underline);
                }

                CHAR_STRIKETHROUGH => {
                    self.bump(1);
                    return Some(IrcFormatEvent::Strikethrough);
                }

                CHAR_MONOSPACE => {
                    self.bump(1);
                    return Some(IrcFormatEvent::Monospace);
                }

                CHAR_COLOR => {
                    self.bump(1);
                    match self.parse_color() {
                        Some((fg, bg)) => return Some(IrcFormatEvent::Color { fg, bg }),
                        None => {
                            // Just skip the control char
                        }
                    }
                }

                CHAR_HEX_COLOR => {
                    self.bump(1);
                    self.skip_hex_code();
                }

                CHAR_REVERSE_COLOR => {
                    self.bump(1);
                    return Some(IrcFormatEvent::ReverseColor);
                }

                CHAR_RESET => {
                    self.bump(1);
                    return Some(IrcFormatEvent::Reset);
                }

                '\t' => {
                    self.bump(1);
                    return Some(IrcFormatEvent::Text(TAB_STR));
                }

                '\n' | '\r' => {
                    // RFC 2812 does not allow standalone CR or LF in messages so we're free to
                    // interpret this however we want.
                    self.bump(1);
                    return Some(IrcFormatEvent::Text(" "));
                }

                other if other.is_ascii_control() => {
                    // ASCII controls other than tab, CR, and LF are ignored.
                    self.bump(1);
                    continue;
                }

                _other => return Some(IrcFormatEvent::Text(self.parse_text())),
            }
        }
    }
}

pub fn parse_irc_formatting<'a>(s: &'a str) -> impl Iterator<Item = IrcFormatEvent> + 'a {
    FormatEventParser::new(s)
}

/// Removes all IRC formatting characters and ASCII control characters.
pub fn remove_irc_control_chars(str: &str) -> String {
    let mut s = String::with_capacity(str.len());

    for event in parse_irc_formatting(str) {
        match event {
            IrcFormatEvent::Bold
            | IrcFormatEvent::Italic
            | IrcFormatEvent::Underline
            | IrcFormatEvent::Strikethrough
            | IrcFormatEvent::Monospace
            | IrcFormatEvent::Color { .. }
            | IrcFormatEvent::ReverseColor
            | IrcFormatEvent::Reset => {}
            IrcFormatEvent::Text(text) => s.push_str(text),
        }
    }

    s
}

#[test]
fn test_translate_irc_control_chars() {
    assert_eq!(
        remove_irc_control_chars("  Le Voyageur imprudent  "),
        "  Le Voyageur imprudent  "
    );
    assert_eq!(remove_irc_control_chars("\x0301,02foo"), "foo");
    assert_eq!(remove_irc_control_chars("\x0301,2foo"), "foo");
    assert_eq!(remove_irc_control_chars("\x031,2foo"), "foo");
    assert_eq!(remove_irc_control_chars("\x031,foo"), ",foo");
    assert_eq!(remove_irc_control_chars("\x03,foo"), ",foo");
}

#[test]
fn test_parse_text_1() {
    let s = "just \x02\x1d\x1f\x1e\x11\x04rrggbb\x16\x0f testing";
    let mut parser = parse_irc_formatting(s);
    assert_eq!(parser.next(), Some(IrcFormatEvent::Text("just ")));
    assert_eq!(parser.next(), Some(IrcFormatEvent::Bold));
    assert_eq!(parser.next(), Some(IrcFormatEvent::Italic));
    assert_eq!(parser.next(), Some(IrcFormatEvent::Underline));
    assert_eq!(parser.next(), Some(IrcFormatEvent::Strikethrough));
    assert_eq!(parser.next(), Some(IrcFormatEvent::Monospace));
    assert_eq!(parser.next(), Some(IrcFormatEvent::ReverseColor));
    assert_eq!(parser.next(), Some(IrcFormatEvent::Reset));
    assert_eq!(parser.next(), Some(IrcFormatEvent::Text(" testing")));
    assert_eq!(parser.next(), None);
}

#[test]
fn test_parse_text_2() {
    let s = "a\x03";
    let mut parser = parse_irc_formatting(s);
    assert_eq!(parser.next(), Some(IrcFormatEvent::Text("a")));
    assert_eq!(parser.next(), None);
}

#[test]
fn test_parse_text_3() {
    let s = "a\x03b";
    let mut parser = parse_irc_formatting(s);
    assert_eq!(parser.next(), Some(IrcFormatEvent::Text("a")));
    assert_eq!(parser.next(), Some(IrcFormatEvent::Text("b")));
    assert_eq!(parser.next(), None);
}

#[test]
fn test_parse_text_4() {
    let s = "a\x031,2b";
    let mut parser = parse_irc_formatting(s);
    assert_eq!(parser.next(), Some(IrcFormatEvent::Text("a")));
    assert_eq!(
        parser.next(),
        Some(IrcFormatEvent::Color {
            fg: Color::Black,
            bg: Some(Color::Blue)
        })
    );
    assert_eq!(parser.next(), Some(IrcFormatEvent::Text("b")));
    assert_eq!(parser.next(), None);
}

#[test]
fn test_parse_text_5() {
    let s = "\x0301,02a";
    let mut parser = parse_irc_formatting(s);
    assert_eq!(
        parser.next(),
        Some(IrcFormatEvent::Color {
            fg: Color::Black,
            bg: Some(Color::Blue),
        })
    );
    assert_eq!(parser.next(), Some(IrcFormatEvent::Text("a")));
    assert_eq!(parser.next(), None);

    let s = "\x0301,2a";
    let mut parser = parse_irc_formatting(s);
    assert_eq!(
        parser.next(),
        Some(IrcFormatEvent::Color {
            fg: Color::Black,
            bg: Some(Color::Blue),
        })
    );
    assert_eq!(parser.next(), Some(IrcFormatEvent::Text("a")));
    assert_eq!(parser.next(), None);

    let s = "\x031,2a";
    let mut parser = parse_irc_formatting(s);
    assert_eq!(
        parser.next(),
        Some(IrcFormatEvent::Color {
            fg: Color::Black,
            bg: Some(Color::Blue),
        })
    );
    assert_eq!(parser.next(), Some(IrcFormatEvent::Text("a")));
    assert_eq!(parser.next(), None);

    let s = "\x031,a";
    let mut parser = parse_irc_formatting(s);
    assert_eq!(
        parser.next(),
        Some(IrcFormatEvent::Color {
            fg: Color::Black,
            bg: None,
        })
    );
    assert_eq!(parser.next(), Some(IrcFormatEvent::Text(",a")));
    assert_eq!(parser.next(), None);

    let s = "\x03,a";
    let mut parser = parse_irc_formatting(s);
    assert_eq!(parser.next(), Some(IrcFormatEvent::Text(",a")));
    assert_eq!(parser.next(), None);
}

#[test]
fn test_parse_color() {
    let s = "";
    let mut parser = FormatEventParser::new(s);
    assert_eq!(parser.parse_color(), None);

    let s = "a";
    let mut parser = FormatEventParser::new(s);
    assert_eq!(parser.parse_color(), None);

    let s = "1a";
    let mut parser = FormatEventParser::new(s);
    assert_eq!(parser.parse_color(), Some((Color::Black, None)));

    let s = "1,2a";
    let mut parser = FormatEventParser::new(s);
    assert_eq!(
        parser.parse_color(),
        Some((Color::Black, Some(Color::Blue)))
    );

    let s = "01,2a";
    let mut parser = FormatEventParser::new(s);
    assert_eq!(
        parser.parse_color(),
        Some((Color::Black, Some(Color::Blue)))
    );

    let s = "01,02a";
    let mut parser = FormatEventParser::new(s);
    assert_eq!(
        parser.parse_color(),
        Some((Color::Black, Some(Color::Blue)))
    );
}

#[test]
fn test_newline() {
    assert_eq!(remove_irc_control_chars("\na\nb\nc\n"), " a b c ");
    assert_eq!(remove_irc_control_chars("\ra\rb\rc\r"), " a b c ");
}

#[test]
fn test_tab() {
    assert_eq!(
        remove_irc_control_chars("\ta\tb\tc\t"),
        "        a        b        c        "
    );
}
