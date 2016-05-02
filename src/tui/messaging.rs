use rustbox::{RustBox, Key};
use time::Tm;

use std::collections::HashMap;
use std::collections::HashSet;

use rand;
use rand::Rng;

use tui::msg_area::MsgArea;
use tui::style;
use tui::text_field::TextField;
use tui::widget::{WidgetRet};
use utils::in_slice;

/// A messaging screen is just a text field to type messages and msg area to
/// show incoming/sent messages.
pub struct MessagingUI {
    /// Channel topic, user info etc.
    topic      : Option<String>,

    /// Incoming and sent messages appear
    msg_area   : MsgArea,

    /// User input field
    text_field : TextField,

    width      : i32,
    height     : i32,

    // NOTE: Color is encoded in Termbox's 216 colors. (in 256-color mode)
    nick_colors      : HashMap<String, u8>,
    available_colors : HashSet<u8>,

    // All nicks in the channel. Need to keep this up-to-date to be able to
    // properly highlight mentions.
    nicks : HashSet<String>,
}

impl MessagingUI {
    pub fn new(width : i32, height : i32) -> MessagingUI {
        MessagingUI {
            topic: None,
            msg_area: MsgArea::new(width, height - 1),
            text_field: TextField::new(width),
            width: width,
            height: height,
            nick_colors: HashMap::new(),
            available_colors: (16 .. 232).into_iter().collect(),
            nicks: HashSet::new(),
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
        self.text_field.draw(rustbox, pos_x, pos_y + self.height - 1);
    }

    pub fn keypressed(&mut self, key : Key) -> WidgetRet {
        match key {
            Key::Ctrl('p') => {
                self.msg_area.scroll_up();
                WidgetRet::KeyHandled
            },

            Key::Ctrl('n') => {
                self.msg_area.scroll_down();
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
                // TODO: Handle ret
                match self.text_field.keypressed(key) {
                    WidgetRet::KeyIgnored => {
                        // self.show_server_msg("KEY IGNORED", format!("{:?}", key).as_ref());
                        WidgetRet::KeyIgnored
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
        self.text_field.resize(width, 1);
    }
}

////////////////////////////////////////////////////////////////////////////////
// Methods delegeted to the msg_area

impl MessagingUI {
    #[inline]
    pub fn add_client_err_msg(&mut self, msg : &str) {
        self.msg_area.add_text(msg, &style::ERR_MSG_SS);
        self.msg_area.flush_line();
    }

    #[inline]
    pub fn add_client_msg(&mut self, msg : &str) {
        self.msg_area.add_text(msg, &style::USER_MSG_SS);
        self.msg_area.flush_line();
    }

    #[inline]
    pub fn add_privmsg(&mut self, sender : &str, msg : &str, tm : &Tm) {
        self.msg_area.add_text(&format!("[{}] <", tm.strftime("%H:%M").unwrap()),
                               &style::USER_MSG_SS);

        {
            let fg = self.get_nick_color(sender);
            let mut sender_style = String::with_capacity(3);
            sender_style.push(style::TERMBOX_COLOR_PREFIX);
            sender_style.push(fg as char);
            sender_style.push('\x00'); // termbox bg (default)
            self.msg_area.add_text(sender, &style::StyleStr(&sender_style));
        }

        // Need to write this to clear fg/bg. Otherwise we end up ORing old
        // fg/bg with new ones.
        self.msg_area.add_char(style::RESET_PREFIX);
        self.msg_area.add_text("> ", &style::USER_MSG_SS);

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
                self.msg_area.add_text(&msg[ last_idx .. nick_start ], &style::USER_MSG_SS);

                {
                    let nick = unsafe { msg.slice_unchecked(nick_start, nick_end) };
                    let nick_color = self.get_nick_color(nick);
                    let mut nick_style = String::with_capacity(3);
                    nick_style.push(style::TERMBOX_COLOR_PREFIX);
                    nick_style.push(nick_color as char);
                    nick_style.push('\x00'); // default bg
                    self.msg_area.add_text(nick, &style::StyleStr(&nick_style));
                }

                self.msg_area.add_char(style::RESET_PREFIX);

                last_idx = nick_end;
            }

            if last_idx != msg.len() {
                self.msg_area.add_text(&msg[ last_idx .. ], &style::USER_MSG_SS);
            }
        }

        self.msg_area.flush_line();
    }

    #[inline]
    pub fn add_msg(&mut self, msg : &str, tm : &Tm) {
        self.msg_area.add_text(
            &format!("[{}] {}", tm.strftime("%H:%M").unwrap(), msg),
            &style::USER_MSG_SS);
        self.msg_area.flush_line();
    }

    #[inline]
    pub fn add_err_msg(&mut self, msg : &str, tm : &Tm) {
        self.msg_area.add_text(
            &format!("[{}] ", tm.strftime("%H:%M").unwrap()),
            &style::USER_MSG_SS);
        self.msg_area.add_text(msg, &style::ERR_MSG_SS);
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
    pub fn join(&mut self, nick : &str) {
        self.nicks.insert(nick.to_owned());
    }

    pub fn part(&mut self, nick : &str) {
        self.nicks.remove(nick);
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

static LEFT_SEPS  : [char; 5] = [ '(', '{', '[', '|', '<' ];
static RIGHT_SEPS : [char; 8] = [ ')', '}', ']', '|', '>', ',', ';', '.' ];

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
}
