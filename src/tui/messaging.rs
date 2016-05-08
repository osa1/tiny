use rustbox::{RustBox, Key};
use time::Tm;

use std::collections::HashMap;
use std::collections::HashSet;

use rand;
use rand::Rng;

use tui::exit_dialogue::ExitDialogue;
use tui::msg_area::MsgArea;
use tui::style;
use tui::text_field::TextField;
use tui::widget::{WidgetRet, Widget};
use utils::in_slice;

/// A messaging screen is just a text field to type messages and msg area to
/// show incoming/sent messages.
pub struct MessagingUI {
    /// Channel topic, user info etc.
    topic : Option<String>,

    /// Incoming and sent messages appear
    msg_area : MsgArea,

    /// Stacked user input fields. Topmost one handles keypresses.
    input_field : Vec<Box<Widget>>,

    width  : i32,
    height : i32,

    // NOTE: Color is encoded in Termbox's 216 colors. (in 256-color mode)
    nick_colors      : HashMap<String, u8>,
    available_colors : HashSet<u8>,

    // All nicks in the channel. Need to keep this up-to-date to be able to
    // properly highlight mentions.
    nicks : HashSet<String>,

    last_activity_line : Option<Box<ActivityLine>>,
}

/// An activity line is just a line that we update on joins / leaves /
/// disconnects. We group activities that happen in the same minute to avoid
/// redundantly showing lines.
struct ActivityLine {
    tm_hour  : i32,
    tm_min   : i32,
    line_idx : usize,
}

impl MessagingUI {
    pub fn new(width : i32, height : i32) -> MessagingUI {
        MessagingUI {
            topic: None,
            msg_area: MsgArea::new(width, height - 1),
            input_field: vec![Box::new(TextField::new(width))],
            width: width,
            height: height,
            nick_colors: HashMap::new(),
            available_colors: (16 .. 232).into_iter().collect(),
            nicks: HashSet::new(),
            last_activity_line: None,
        }
    }

    pub fn set_topic(&mut self, topic : String) {
        self.topic = Some(topic);
        // FIXME: Disabling this - need to decide when/how to draw channel topics
        // self.msg_area.resize(self.width, self.height - 2);
    }

    pub fn draw(&self, rustbox : &RustBox, pos_x : i32, pos_y : i32) {
        // TODO: Most channels have long topics that don't fit into single line.
        // if let Some(ref topic) = self.topic {
        //     // rustbox.print(pos_x as usize, pos_y as usize,
        //     //               style::TOPIC.style,
        //     //               style::TOPIC.fg,
        //     //               style::TOPIC.bg,
        //     //               topic);
        //     self.msg_area.draw(rustbox, pos_x, pos_y + 1);
        // } else {
        //     self.msg_area.draw(rustbox, pos_x, pos_y);
        // }

        self.msg_area.draw(rustbox, pos_x, pos_y);
        self.input_field.draw(rustbox, pos_x, pos_y + self.height - 1);
    }

    pub fn keypressed(&mut self, key : Key) -> WidgetRet {
        match key {
            Key::Ctrl('q') => {
                self.toggle_exit_dialogue();
                WidgetRet::KeyHandled
            },

            Key::PageUp => {
                self.msg_area.page_up();
                WidgetRet::KeyHandled
            },

            Key::PageDown => {
                self.msg_area.page_down();
                WidgetRet::KeyHandled
            },

            key => {
                match self.input_field.keypressed(key) {
                    WidgetRet::KeyIgnored => {
                        // self.show_server_msg("KEY IGNORED", format!("{:?}", key).as_ref());
                        WidgetRet::KeyIgnored
                    },
                    WidgetRet::Remove => {
                        self.input_field.pop();
                        WidgetRet::KeyHandled

                    },
                    ret => ret,
                }
            },
        }
    }

    pub fn resize(&mut self, width : i32, height : i32) {
        self.width = width;
        self.height = height;
        self.msg_area.resize(width, height - 1);
        self.input_field.resize(width, 1);
    }

    fn toggle_exit_dialogue(&mut self) {
        assert!(self.input_field.len() > 0);
        // FIXME: This is a bit too fragile I think. Since we only stack an exit
        // dialogue on top of the input field at the moment, checking the len()
        // is fine. If we decide to stack more stuff it'll break.
        if self.input_field.len() == 1 {
            self.input_field.push(Box::new(ExitDialogue::new(self.width)));
        } else {
            self.input_field.pop();
        }
    }
}

////////////////////////////////////////////////////////////////////////////////
// Adding new messages

impl MessagingUI {
    pub fn add_client_err_msg(&mut self, msg : &str) {
        self.reset_activity_line();

        self.msg_area.set_style(&style::ERR_MSG);
        self.msg_area.add_text(msg);
        self.msg_area.flush_line();
    }

    pub fn add_client_msg(&mut self, msg : &str) {
        self.reset_activity_line();

        self.msg_area.set_style(&style::USER_MSG);
        self.msg_area.add_text(msg);
        self.msg_area.flush_line();
        self.reset_activity_line();
    }

    pub fn add_privmsg(&mut self, sender : &str, msg : &str, tm : &Tm) {
        self.reset_activity_line();

        let translated = translate_irc_colors(msg);
        let msg = {
            match translated {
                Some(ref str) => &str,
                None => msg,
            }
        };

        self.msg_area.set_style(&style::USER_MSG);
        self.msg_area.add_text(&format!("[{}] <", tm.strftime("%H:%M").unwrap()));

        {
            let nick_color = self.get_nick_color(sender);
            let style = style::Style { fg: nick_color as u16, bg: style::USER_MSG.bg };
            self.msg_area.set_style(&style);
            self.msg_area.add_text(sender);
        }

        self.msg_area.set_style(&style::USER_MSG);
        self.msg_area.add_text("> ");

        // Highlight nicks
        {
            let nick_idxs =
                WordIdxs::new(msg)
                    .filter(|&(word_left, word_right)|
                            self.nicks.contains(unsafe {
                                msg.slice_unchecked(word_left, word_right)
                            }))
                    // need to allocate a vector here to make borrow checker
                    // happy (self.nicks is borrowed)
                    .collect::<Vec<(usize, usize)>>();

            let mut last_idx = 0;
            for (nick_start, nick_end) in nick_idxs.into_iter() {
                self.msg_area.set_style(&style::USER_MSG);
                self.msg_area.add_text(&msg[ last_idx .. nick_start ]);

                {
                    let nick = unsafe { msg.slice_unchecked(nick_start, nick_end) };
                    let nick_color = self.get_nick_color(nick);
                    let style = style::Style { fg: nick_color as u16, bg: style::USER_MSG.bg };
                    self.msg_area.set_style(&style);
                    self.msg_area.add_text(nick);
                }

                last_idx = nick_end;
            }

            if last_idx != msg.len() {
                self.msg_area.set_style(&style::USER_MSG);
                self.msg_area.add_text(&msg[ last_idx .. ]);
            }
        }

        self.msg_area.flush_line();
    }

    pub fn add_msg(&mut self, msg : &str, tm : &Tm) {
        self.reset_activity_line();

        self.msg_area.set_style(&style::USER_MSG);
        self.msg_area.add_text(&format!("[{}] {}", tm.strftime("%H:%M").unwrap(), msg));
        self.msg_area.flush_line();
    }

    pub fn add_err_msg(&mut self, msg : &str, tm : &Tm) {
        self.reset_activity_line();

        self.msg_area.set_style(&style::USER_MSG);
        self.msg_area.add_text(&format!("[{}] ", tm.strftime("%H:%M").unwrap()));
        self.msg_area.set_style(&style::ERR_MSG);
        self.msg_area.add_text(msg);
        self.msg_area.flush_line();
    }

    fn get_nick_color(&mut self, sender : &str) -> u8 {
        match self.nick_colors.get(sender) {
            Some(color) => {
                return *color;
            }
            None => {},
        }

        let mut rng = rand::thread_rng();
        let ret = {
            if !self.available_colors.is_empty() {
                let ret =
                    *self.available_colors.iter().nth(
                        rng.gen_range(0, self.available_colors.len())).unwrap();
                self.available_colors.remove(&ret);
                ret
            } else {
                rng.gen_range(16, 232)
            }
        };

        self.nick_colors.insert(sender.to_owned(), ret);
        ret
    }
}

////////////////////////////////////////////////////////////////////////////////
// Keeping nick list up-to-date

impl MessagingUI {
    pub fn join(&mut self, nick : &str, tm : Option<&Tm>) {
        self.nicks.insert(nick.to_owned());

        if let Some(tm) = tm {
            let line_idx = self.get_activity_line_idx(tm.tm_hour, tm.tm_min);
            self.msg_area.modify_line(line_idx, |line| {
                line.set_style(&style::JOIN);
                line.add_text(nick);
                line.add_char(' ');
            });
        }
    }

    pub fn part(&mut self, nick : &str, tm : Option<&Tm>) {
        self.nicks.remove(nick);

        if let Some(tm) = tm {
            let line_idx = self.get_activity_line_idx(tm.tm_hour, tm.tm_min);
            self.msg_area.modify_line(line_idx, |line| {
                line.set_style(&style::LEAVE);
                line.add_text(nick);
                line.add_char(' ');
            });
        }
    }

    pub fn has_nick(&self, nick : &str) -> bool {
        self.nicks.contains(nick)
    }

    fn reset_activity_line(&mut self) {
        self.last_activity_line = None;
    }

    fn get_activity_line_idx(&mut self, hour : i32, min : i32) -> usize {
        if let Some(ref mut l) = self.last_activity_line {
            if l.tm_hour == hour && l.tm_min == min {
                l.line_idx
            } else {
                // FIXME: This part is weird. Maybe msg_area should have a
                // `add_line(Line)` method instead of weird `add_text()`, `set_style()`
                // etc.
                self.msg_area.set_style(&style::USER_MSG);
                self.msg_area.add_text(&format!("[{:02}:{:02}] ", hour, min));
                let line_idx = self.msg_area.flush_line();
                l.tm_hour = hour;
                l.tm_min = min;
                l.line_idx = line_idx;
                line_idx
            }
        } else {
            self.msg_area.set_style(&style::USER_MSG);
            self.msg_area.add_text(&format!("[{:02}:{:02}] ", hour, min));
            let line_idx = self.msg_area.flush_line();
            self.last_activity_line = Some(Box::new(ActivityLine {
                tm_hour: hour,
                tm_min: min,
                line_idx: line_idx,
            }));
            line_idx
        }
    }
}

////////////////////////////////////////////////////////////////////////////////

fn translate_irc_colors(str : &str) -> Option<String> {
    // Most messages won't have any colors, so we have this fast path here
    if str.find(style::IRC_COLOR_PREFIX).is_none() {
        return None;
    }

    let mut ret = String::with_capacity(str.len());

    let mut iter = str.chars();
    while let Some(mut char) = iter.next() {
        if char == style::IRC_COLOR_PREFIX {
            let fg1 = to_dec(iter.next().unwrap());
            let fg2 = to_dec(iter.next().unwrap());
            let fg  = fg1 * 10 + fg2;
            if let Some(char_) = iter.next() {
                if char_ == ',' {
                    let bg1 = to_dec(iter.next().unwrap());
                    let bg2 = to_dec(iter.next().unwrap());
                    let bg  = bg1 * 10 + bg2;
                    ret.push(style::TERMBOX_COLOR_PREFIX);
                    ret.push(0 as char); // style
                    ret.push(irc_color_to_termbox(fg) as char);
                    ret.push(irc_color_to_termbox(bg) as char);
                    continue;
                } else {
                    ret.push(style::TERMBOX_COLOR_PREFIX);
                    ret.push(0 as char); // style
                    ret.push(irc_color_to_termbox(fg) as char);
                    ret.push(irc_color_to_termbox(style::USER_MSG.bg as u8) as char);
                    char = char_;
                }
            } else {
                ret.push(style::TERMBOX_COLOR_PREFIX);
                ret.push(0 as char); // style
                ret.push(irc_color_to_termbox(fg) as char);
                ret.push(irc_color_to_termbox(style::USER_MSG.bg as u8) as char);
                break;
            }
        }

        ret.push(char);
    }

    Some(ret)
}

#[inline]
fn to_dec(ch : char) -> u8 {
    ((ch as u32) - ('0' as u32)) as u8
}

// IRC colors: http://en.wikichip.org/wiki/irc/colors
// Termbox colors: http://www.calmar.ws/vim/256-xterm-24bit-rgb-color-chart.html
fn irc_color_to_termbox(irc_color : u8) -> u8 {
    match irc_color {
         0 => 15,  // white
         1 => 0,   // black
         2 => 17,  // navy
         3 => 2,   // green
         4 => 9,   // red
         5 => 88,  // maroon
         6 => 5,   // purple
         7 => 130, // olive
         8 => 11,  // yellow
         9 => 10,  // light green
        10 => 6,   // teal
        11 => 14,  // cyan
        12 => 12,  // awful blue
        13 => 13,  // magenta
        14 => 8,   // gray
        15 => 7,   // light gray
         _ => panic!("Unknown irc color: {}", irc_color)
    }
}

////////////////////////////////////////////////////////////////////////////////

// When highlighting nicks in messages, we search words in the `nicks` set. To
// be able to highlight substrings, we need offsets of chars, but
// `SplitWhitespace` doesn't provide that. Also, our separators are actually a
// set of characters, like {'<', '(', etc} instead of a fixed character or
// whitespace.

struct WordIdxs<'s> {
    /// The whole thing, not a shrinking slice.
    str : &'s str,

    /// Current position.
    idx : usize,
}

impl<'s> WordIdxs<'s> {
    fn new(str : &str) -> WordIdxs {
        WordIdxs {
            str: str,
            idx: 0,
        }
    }
}

impl<'s> Iterator for WordIdxs<'s> {
    type Item = (usize, usize);

    fn next(&mut self) -> Option<(usize, usize)> {
        if self.idx >= self.str.len() {
            return None;
        }

        let slice = unsafe { self.str.slice_unchecked(self.idx, self.str.len()) };

        if let Some(left_ws_idx) = find_word_left(slice) {
            if let Some(right_ws_idx) = find_word_right(unsafe {
                self.str.slice_unchecked(self.idx + left_ws_idx, self.str.len())
            }) {
                let idx = self.idx;
                self.idx += left_ws_idx + right_ws_idx;
                return Some((idx + left_ws_idx, idx + left_ws_idx + right_ws_idx));
            } else {
                let idx = self.idx;
                self.idx += self.str.len();
                return Some((idx + left_ws_idx, idx + left_ws_idx + self.str.len()));
            }
        }

        None
    }
}

/// A word may start with these characters, in addition to alphanumerics.
static LEFT_SEPS  : [char; 8]  = [ '(', '{', '[', '|', '<', '\'', '`', '"' ];

/// A word may end with these characters, in addition to whitespace.
static RIGHT_SEPS : [char; 11] = [ ')', '}', ']', '|', '>', ',', ';', '.', '\'', '`', '"' ];

#[inline]
fn is_left_sep(char : char) -> bool {
    char.is_whitespace() || in_slice(char, &LEFT_SEPS)
}

#[inline]
fn is_right_sep(char : char) -> bool {
    char.is_whitespace() || in_slice(char, &RIGHT_SEPS)
}

fn find_word_left(str : &str) -> Option<usize> {
    let mut iter = str.char_indices();
    while let Some((char_idx, char)) = iter.next() {
        if is_left_sep(char) {
            // consume consecutive separators
            while let Some((char_idx, char)) = iter.next() {
                if !is_left_sep(char) {
                    return Some(char_idx);
                }
            }
        } else if char.is_alphanumeric() {
            return Some(char_idx);
        }
    }

    None
}

fn find_word_right(str : &str) -> Option<usize> {
    if str.is_empty() {
        return None;
    }

    // find_word_left should have consumed this
    assert!(!str.chars().nth(0).unwrap().is_whitespace());

    let mut iter = str.char_indices();
    while let Some((char_idx, char)) = iter.next() {
        if is_right_sep(char) {
            return Some(char_idx);
        }
    }

    Some(str.len())
}

#[test]
fn test_left_ws() {
    assert_eq!(find_word_left("x y"), Some(0));
    assert_eq!(find_word_left(" x y"), Some(1));
    assert_eq!(find_word_left("    y"), Some(4));
    assert_eq!(find_word_left("xy"), Some(0));
    assert_eq!(find_word_left(""), None);
    assert_eq!(find_word_left(" "), None);
    assert_eq!(find_word_left("    "), None);
    assert_eq!(find_word_left("<xyz>"), Some(1));
    assert_eq!(find_word_left("<  xyz>"), Some(3));
    assert_eq!(find_word_left(">"), None);
}

#[test]
fn test_right_ws() {
    assert_eq!(find_word_right(""), None);
    assert_eq!(find_word_right("x"), Some(1));
    assert_eq!(find_word_right("x y"), Some(1));
    assert_eq!(find_word_right("asdf"), Some(4));
    assert_eq!(find_word_right("xyz>"), Some(3));
    assert_eq!(find_word_right("xyz,"), Some(3));
}

#[test]
fn test_word_idxs() {
    assert_eq!(WordIdxs::new("x").into_iter().collect::<Vec<(usize, usize)>>(),
               vec![(0, 1)]);
    assert_eq!(WordIdxs::new("x y").into_iter().collect::<Vec<(usize, usize)>>(),
               vec![(0, 1), (2, 3)]);
    assert_eq!(WordIdxs::new("x y z").into_iter().collect::<Vec<(usize, usize)>>(),
               vec![(0, 1), (2, 3), (4, 5)]);
    assert_eq!(WordIdxs::new("xyz").into_iter().collect::<Vec<(usize, usize)>>(),
               vec![(0, 3)]);
    assert_eq!(WordIdxs::new("xyz foo bar baz").into_iter().collect::<Vec<(usize, usize)>>(),
               vec![(0, 3), (4, 7), (8, 11), (12, 15)]);
    assert_eq!(WordIdxs::new("<xyz>").into_iter().collect::<Vec<(usize, usize)>>(),
               vec![(1, 4)]);
    assert_eq!(WordIdxs::new("<xyz> (foo) [bar] {baz}").into_iter().collect::<Vec<(usize, usize)>>(),
               vec![(1, 4), (7, 10), (13, 16), (19, 22)]);
    assert_eq!(WordIdxs::new("foo, bar; baz: yada").into_iter().collect::<Vec<(usize, usize)>>(),
               vec![(0, 3), (5, 8), (10, 14), (15, 19)]);
    assert_eq!(WordIdxs::new("tiny_test, hey").into_iter().collect::<Vec<(usize, usize)>>(),
               vec![(0, 9), (11, 14)]);
    assert_eq!(WordIdxs::new("foo's bar`s \"baz\"").into_iter().collect::<Vec<(usize, usize)>>(),
               vec![(0, 3), (4, 5), (6, 9), (10, 11), (13, 16)]);
}
