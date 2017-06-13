use termbox_simple::Termbox;
use term_input::Key;

use std::collections::HashMap;
use std::convert::From;
use std::iter::Peekable;
use std::rc::Rc;
use std::str::Chars;

use time::Tm;
use time;

use config;
use config::Style;
use trie::Trie;
use tui::exit_dialogue::ExitDialogue;
use tui::msg_area::line::{TERMBOX_COLOR_PREFIX, IRC_COLOR_PREFIX};
use tui::msg_area::MsgArea;
use tui::text_field::TextField;
use tui::widget::{WidgetRet, Widget};

/// A messaging screen is just a text field to type messages and msg area to
/// show incoming/sent messages.
pub struct MessagingUI {
    /// Incoming and sent messages appear
    msg_area: MsgArea,

    /// Stacked user input fields. Topmost one handles keypresses.
    input_field: Vec<Box<Widget>>,

    width: i32,
    height: i32,

    // NOTE: Color is encoded in Termbox's 216 colors. (in 256-color mode)
    nick_colors: HashMap<String, u8>,
    /// Index of the next color to use when highlighting a new nick. Always a valid index to
    /// `config::NICK_COLORS`.
    next_color_idx: usize,

    // All nicks in the channel. Need to keep this up-to-date to be able to
    // properly highlight mentions.
    //
    // Rc to be able to share with dynamic messages.
    nicks: Rc<Trie>,

    last_activity_line: Option<ActivityLine>,
    last_activity_ts: Option<Timestamp>,
}

/// Like `time::Tm`, but we only care about hour and minute parts.
#[derive(PartialEq, Eq, Clone, Copy)]
pub struct Timestamp {
    hour: i32,
    min: i32,
}

impl Timestamp {
    pub fn now() -> Timestamp {
        Timestamp::from(time::now())
    }

    fn stamp(&self, msg_area: &mut MsgArea) {
        msg_area.set_style(config::TIMESTAMP);
        msg_area.add_text(&format!("{:02}:{:02} ", self.hour, self.min));
    }
}

impl From<Tm> for Timestamp {
    fn from(tm: Tm) -> Timestamp {
        Timestamp { hour: tm.tm_hour, min: tm.tm_min }
    }
}

/// An activity line is just a line that we update on joins / leaves /
/// disconnects. We group activities that happen in the same minute to avoid
/// redundantly showing lines.
struct ActivityLine {
    ts: Timestamp,
    line_idx: usize,
}

impl MessagingUI {
    pub fn new(width : i32, height : i32) -> MessagingUI {
        MessagingUI {
            msg_area: MsgArea::new(width, height - 1),
            input_field: vec![Box::new(TextField::new(width))],
            width: width,
            height: height,
            nick_colors: HashMap::new(),
            next_color_idx: 0,
            nicks: Rc::new(Trie::new()),
            last_activity_line: None,
            last_activity_ts: None,
        }
    }

    pub fn draw(&self, tb: &mut Termbox, pos_x: i32, pos_y: i32) {
        self.msg_area.draw(tb, pos_x, pos_y);
        self.input_field.draw(tb, pos_x, pos_y + self.height - 1);
    }

    pub fn keypressed(&mut self, key : Key) -> WidgetRet {
        match key {
            Key::Ctrl('c') => {
                self.toggle_exit_dialogue();
                WidgetRet::KeyHandled
            },

            Key::Ctrl('u') | Key::PageUp => {
                self.msg_area.page_up();
                WidgetRet::KeyHandled
            },

            Key::Ctrl('d') | Key::PageDown => {
                self.msg_area.page_down();
                WidgetRet::KeyHandled
            },

            Key::ShiftUp => {
                self.msg_area.scroll_up();
                WidgetRet::KeyHandled
            },

            Key::ShiftDown => {
                self.msg_area.scroll_down();
                WidgetRet::KeyHandled
            },

            Key::Tab => {
                self.input_field.event(Box::new(self.nicks.clone()));
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

    pub fn resize(&mut self, width: i32, height: i32) {
        self.width = width;
        self.height = height;
        self.msg_area.resize(width, height - 1);
        for w in &mut self.input_field {
            w.resize(width, 1);
        }
    }

    fn toggle_exit_dialogue(&mut self) {
        assert!(!self.input_field.is_empty());
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

    fn add_timestamp(&mut self, ts: Timestamp) {
        if let Some(ts_) = self.last_activity_ts {
            if ts_ != ts {
                ts.stamp(&mut self.msg_area);
            }
        } else {
            ts.stamp(&mut self.msg_area);

        }
        self.last_activity_ts = Some(ts);
    }

    pub fn show_topic(&mut self, topic: &str, ts: Timestamp) {
        self.add_timestamp(ts);

        self.msg_area.set_style(config::TOPIC);
        self.msg_area.add_text(&format!("Channel topic is: \"{}\"", topic));

        self.msg_area.flush_line();
    }

    pub fn add_client_err_msg(&mut self, msg : &str) {
        self.reset_activity_line();

        self.msg_area.set_style(config::ERR_MSG);
        self.msg_area.add_text(msg);
        self.msg_area.flush_line();
    }

    pub fn add_client_msg(&mut self, msg : &str) {
        self.reset_activity_line();

        self.msg_area.set_style(config::USER_MSG);
        self.msg_area.add_text(msg);
        self.msg_area.flush_line();
        self.reset_activity_line();
    }

    pub fn add_privmsg(&mut self, sender: &str, msg: &str, ts: Timestamp, higlight: bool) {
        self.reset_activity_line();

        let translated = translate_irc_colors(msg);
        let msg = {
            match translated {
                Some(ref str) => str,
                None => msg,
            }
        };

        self.add_timestamp(ts);

        {
            let nick_color = self.get_nick_color(sender);
            let style = Style { fg: nick_color as u16, bg: config::USER_MSG.bg };
            self.msg_area.set_style(style);
            self.msg_area.add_text(sender);
        }

        self.msg_area.set_style(Style { fg: config::USER_MSG.fg | config::TB_BOLD, bg: config::USER_MSG.bg });
        self.msg_area.add_text(": ");

        self.msg_area.set_style(
            if higlight {
                config::HIGHLIGHT
            } else {
                config::USER_MSG
            });

        self.msg_area.add_text(msg);
        self.msg_area.flush_line();
    }

    pub fn add_msg(&mut self, msg: &str, ts: Timestamp) {
        self.reset_activity_line();

        self.add_timestamp(ts);
        self.msg_area.set_style(config::USER_MSG);
        self.msg_area.add_text(msg);
        self.msg_area.flush_line();
    }

    pub fn add_err_msg(&mut self, msg: &str, ts: Timestamp) {
        self.reset_activity_line();

        self.add_timestamp(ts);
        self.msg_area.set_style(config::ERR_MSG);
        self.msg_area.add_text(msg);
        self.msg_area.flush_line();
    }

    fn get_nick_color(&mut self, sender: &str) -> u8 {
        if let Some(color) = self.nick_colors.get(sender) {
            return *color;
        }

        let color = config::NICK_COLORS[self.next_color_idx];
        self.nick_colors.insert(sender.to_owned(), color);
        self.next_color_idx = (self.next_color_idx + 1) % (config::NICK_COLORS.len() - 1);
        color
    }
}

////////////////////////////////////////////////////////////////////////////////
// Keeping nick list up-to-date

impl MessagingUI {
    pub fn join(&mut self, nick: &str, ts: Option<Timestamp>) {
        Rc::get_mut(&mut self.nicks).unwrap().insert(nick);

        if let Some(ts) = ts {
            let line_idx = self.get_activity_line_idx(ts);
            self.msg_area.modify_line(line_idx, |line| {
                line.set_style(config::JOIN);
                line.add_char('+');
                line.add_text(nick);
                line.add_char(' ');
            });
        }
    }

    pub fn part(&mut self, nick: &str, ts: Option<Timestamp>) {
        Rc::get_mut(&mut self.nicks).unwrap().remove(nick);

        if let Some(ts) = ts {
            let line_idx = self.get_activity_line_idx(ts);
            self.msg_area.modify_line(line_idx, |line| {
                line.set_style(config::PART);
                line.add_char('-');
                line.add_text(nick);
                line.add_char(' ');
            });
        }
    }

    pub fn nick(&mut self, old_nick: &str, new_nick: &str, ts: Timestamp) {
        Rc::get_mut(&mut self.nicks).unwrap().remove(old_nick);
        Rc::get_mut(&mut self.nicks).unwrap().insert(new_nick);
        let color = self.nick_colors.remove(old_nick);
        if let Some(color_) = color {
            self.nick_colors.insert(new_nick.to_owned(), color_);
        }

        let line_idx = self.get_activity_line_idx(ts);
        self.msg_area.modify_line(line_idx, |line| {
            line.set_style(config::NICK);
            line.add_text(old_nick);
            line.add_text("->");
            line.add_text(new_nick);
            line.add_char(' ');
        });
    }

    pub fn has_nick(&self, nick : &str) -> bool {
        self.nicks.contains(nick)
    }

    fn reset_activity_line(&mut self) {
        self.last_activity_line = None;
    }

    fn get_activity_line_idx(&mut self, ts: Timestamp) -> usize {
        // borrow checkers strikes again
        if let Some(ref mut l) = self.last_activity_line {
            if l.ts == ts {
                return l.line_idx;
            }
        }

        self.add_timestamp(ts);
        let line_idx = self.msg_area.flush_line();
        self.last_activity_line = Some(ActivityLine {
            ts: ts,
            line_idx: line_idx,
        });
        line_idx
    }
}

////////////////////////////////////////////////////////////////////////////////

/// Parse at least one, at most two digits.
fn parse_color_code(chars: &mut Peekable<Chars>) -> Option<u8> {

    fn to_dec(ch: char) -> Option<u8> {
        ch.to_digit(10).map(|c| c as u8)
    }

    let c1_digit = try_opt!(to_dec(try_opt!(chars.next())));

    // Can't peek() and consume so using this indirection
    let ret;
    let mut consume = false;

    match chars.peek() {
        None => { ret = Some(c1_digit); }
        Some(c2) =>
            match to_dec(*c2) {
                None => { ret = Some(c1_digit); }
                Some(c2_digit) => {
                    ret = Some(c1_digit * 10 + c2_digit);
                    consume = true;
                }
            }
    }

    if consume { chars.next(); }
    ret
}

fn translate_irc_colors(str : &str) -> Option<String> {
    // Most messages won't have any colors, so we have this fast path here
    if str.find(IRC_COLOR_PREFIX).is_none() {
        return None;
    }

    let mut ret = String::with_capacity(str.len());

    let mut iter = str.chars().peekable();
    while let Some(mut char) = iter.next() {
        if char == IRC_COLOR_PREFIX {
            let fg = try_opt!(parse_color_code(&mut iter));
            if let Some(char_) = iter.next() {
                if char_ == ',' {
                    let bg = try_opt!(parse_color_code(&mut iter));
                    ret.push(TERMBOX_COLOR_PREFIX);
                    ret.push(0 as char); // style
                    ret.push(irc_color_to_termbox(fg) as char);
                    ret.push(irc_color_to_termbox(bg) as char);
                    continue;
                } else {
                    ret.push(TERMBOX_COLOR_PREFIX);
                    ret.push(0 as char); // style
                    ret.push(irc_color_to_termbox(fg) as char);
                    ret.push(irc_color_to_termbox(config::USER_MSG.bg as u8) as char);
                    char = char_;
                }
            } else {
                ret.push(TERMBOX_COLOR_PREFIX);
                ret.push(0 as char); // style
                ret.push(irc_color_to_termbox(fg) as char);
                ret.push(irc_color_to_termbox(config::USER_MSG.bg as u8) as char);
                break;
            }
        }

        ret.push(char);
    }

    Some(ret)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_color_code() {
        assert_eq!(parse_color_code(&mut "1".chars().peekable()), Some(1));
        assert_eq!(parse_color_code(&mut "01".chars().peekable()), Some(1));
        assert_eq!(parse_color_code(&mut "1,".chars().peekable()), Some(1));
    }

    #[test]
    fn color_translation() {
        assert!(translate_irc_colors("\x034test").is_some());
        assert!(translate_irc_colors("\x034,8test").is_some());
        assert!(translate_irc_colors("\x034,08test").is_some());
        assert!(translate_irc_colors("\x0304,8test").is_some());
        assert!(translate_irc_colors("\x0304,08test").is_some());
    }
}
