use termbox_simple::Termbox;
use term_input::Key;

use std::convert::From;
use std::rc::Rc;

use time::Tm;
use time;

use config::Style;
use config;
use trie::Trie;
use tui::exit_dialogue::ExitDialogue;
use tui::msg_area::MsgArea;
use tui::termbox;
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

    // All nicks in the channel. Need to keep this up-to-date to be able to
    // properly highlight mentions.
    //
    // Rc to be able to share with dynamic messages.
    nicks: Rc<Trie>,

    current_nick: Option<Rc<String>>,
    draw_current_nick: bool,

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
            nicks: Rc::new(Trie::new()),
            current_nick: None,
            draw_current_nick: true,
            last_activity_line: None,
            last_activity_ts: None,
        }
    }

    pub fn set_nick(&mut self, nick: Rc<String>) {
        self.current_nick = Some(nick);
    }

    pub fn get_nick(&self) -> Option<Rc<String>> {
        self.current_nick.clone()
    }

    pub fn draw(&self, tb: &mut Termbox, pos_x: i32, pos_y: i32) {
        self.msg_area.draw(tb, pos_x, pos_y);

        if let &Some(ref nick) = &self.current_nick {
            if self.draw_current_nick {
                let nick_color = self.get_nick_color(nick);
                let style = Style { fg: nick_color as u16, bg: config::USER_MSG.bg };
                termbox::print_chars(
                    tb,
                    pos_x,
                    pos_y + self.height - 1,
                    style,
                    &mut nick.chars());
                tb.change_cell(
                    pos_x + nick.len() as i32,
                    pos_y + self.height - 1,
                    ':',
                    config::USER_MSG.fg | config::TB_BOLD,
                    config::USER_MSG.bg);
                self.input_field.draw(
                    tb,
                    pos_x + nick.len() as i32 + 2,
                    pos_y + self.height - 1);
            } else {
                self.input_field.draw(tb, pos_x, pos_y + self.height - 1);
            }
        } else {
            self.input_field.draw(tb, pos_x, pos_y + self.height - 1);
        }
    }

    pub fn keypressed(&mut self, key : Key) -> WidgetRet {
        match key {
            Key::Ctrl('c') => {
                self.toggle_exit_dialogue();
                WidgetRet::KeyHandled
            },

            Key::Ctrl('u') => {
                self.msg_area.page_up();
                WidgetRet::KeyHandled
            },

            Key::Ctrl('d') => {
                self.msg_area.page_down();
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

            Key::Home => {
                self.msg_area.scroll_top();
                WidgetRet::KeyHandled
            },

            Key::End => {
                self.msg_area.scroll_bottom();
                WidgetRet::KeyHandled
            },

            key => {
                match self.input_field.keypressed(key) {
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

        let nick_width = match &self.current_nick {
            &None =>
                0,
            &Some(ref rc) =>
                // +2 for ": "
                rc.len() as i32 + 2,
        };

        self.draw_current_nick =
            (nick_width as f32) <= (width as f32) * (30f32 / 100f32);

        let widget_width =
            if self.draw_current_nick { width - nick_width } else { width };

        for w in &mut self.input_field {
            w.resize(widget_width, 1);
        }
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
        self.msg_area.add_text(topic);

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

    pub fn add_privmsg(&mut self, sender: &str, msg: &str, ts: Timestamp, highlight: bool) {
        self.reset_activity_line();
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
            if highlight {
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

    fn get_nick_color(&self, sender: &str) -> u8 {
        // Anything works as long as it's fast
        let mut hash: usize = 5381;
        for c in sender.chars() {
            hash = hash.wrapping_mul(33).wrapping_add(c as usize);
        }
        config::NICK_COLORS[hash % config::NICK_COLORS.len()]
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
                line.set_style(config::FADED);
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
                line.set_style(config::FADED);
                line.add_text(nick);
                line.add_char(' ');
            });
        }
    }

    pub fn nick(&mut self, old_nick: &str, new_nick: &str, ts: Timestamp) {
        Rc::get_mut(&mut self.nicks).unwrap().remove(old_nick);
        Rc::get_mut(&mut self.nicks).unwrap().insert(new_nick);

        let line_idx = self.get_activity_line_idx(ts);
        self.msg_area.modify_line(line_idx, |line| {
            line.set_style(config::FADED);
            line.add_text(old_nick);
            line.set_style(config::NICK);
            line.add_text(">");
            line.set_style(config::FADED);
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
        // borrow checker strikes again
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
