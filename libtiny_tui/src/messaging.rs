use term_input::Key;
use termbox_simple::Termbox;

use std::convert::From;

use time::{self, Tm};

use crate::{
    config::{Colors, Style},
    exit_dialogue::ExitDialogue,
    msg_area::{
        line::{SchemeStyle, SegStyle},
        MsgArea,
    },
    termbox,
    text_field::TextField,
    trie::Trie,
    widget::WidgetRet,
};

/// A messaging screen is just a text field to type messages and msg area to
/// show incoming/sent messages.
pub(crate) struct MessagingUI {
    /// Incoming and sent messages appear
    msg_area: MsgArea,

    // exit_dialogue handles input when available.
    // two fields (instead of an enum etc.) to avoid borrowchk problems
    input_field: TextField,
    exit_dialogue: Option<ExitDialogue>,

    width: i32,
    height: i32,

    // Option to disable status messages ( join/part )
    show_status: bool,

    // All nicks in the channel. Need to keep this up-to-date to be able to
    // properly highlight mentions.
    nicks: Trie,

    current_nick: Option<String>,
    show_current_nick: bool,

    last_activity_line: Option<ActivityLine>,
    last_activity_ts: Option<Timestamp>,
}

/// Like `time::Tm`, but we only care about hour and minute parts.
#[derive(PartialEq, Eq, Clone, Copy)]
pub(crate) struct Timestamp {
    hour: i32,
    min: i32,
}

impl Timestamp {
    fn stamp(self, msg_area: &mut MsgArea) {
        msg_area.set_style(SegStyle::SchemeStyle(SchemeStyle::Timestamp));
        msg_area.add_text(&format!("{:02}:{:02} ", self.hour, self.min));
    }
}

impl From<Tm> for Timestamp {
    fn from(tm: Tm) -> Timestamp {
        Timestamp {
            hour: tm.tm_hour,
            min: tm.tm_min,
        }
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
    pub(crate) fn new(width: i32, height: i32, status: bool, text_field_wrap: bool) -> MessagingUI {
        MessagingUI {
            msg_area: MsgArea::new(width, height - 1),
            input_field: TextField::new(width, height - 1, text_field_wrap),
            exit_dialogue: None,
            width,
            height,
            show_status: status,
            nicks: Trie::new(),
            current_nick: None,
            show_current_nick: true,
            last_activity_line: None,
            last_activity_ts: None,
        }
    }

    pub(crate) fn set_nick(&mut self, nick: String) {
        self.current_nick = Some(nick);
        // update text field size
        let w = self.width;
        let h = self.height;
        self.resize(w, h);
    }

    pub(crate) fn get_nick(&self) -> Option<&str> {
        self.current_nick.as_deref()
    }

    pub(crate) fn draw(&mut self, tb: &mut Termbox, colors: &Colors, pos_x: i32, pos_y: i32) {
        let lines = self.input_field.calculate_lines();
        // make space for textfield expansion
        if self.height - lines >= 5 {
            self.msg_area.resize(self.width, self.height - lines);
        } else {
            let widget_width = self.calculate_input_field_width(self.width);
            self.input_field.resize(widget_width, 1);
        }
        self.msg_area.draw(tb, colors, pos_x, pos_y);

        let input_field_y = pos_y + self.height - lines;
        match &self.exit_dialogue {
            Some(exit_dialogue) => {
                exit_dialogue.draw(tb, colors, pos_x, input_field_y);
            }
            None => {
                if let Some(ref nick) = self.current_nick {
                    if self.show_current_nick {
                        let nick_color = colors.nick[self.get_nick_color(nick) % colors.nick.len()];
                        let style = Style {
                            fg: u16::from(nick_color),
                            bg: colors.user_msg.bg,
                        };
                        termbox::print_chars(tb, pos_x, input_field_y, style, nick.chars());
                        tb.change_cell(
                            pos_x + nick.len() as i32,
                            input_field_y,
                            ':',
                            colors.user_msg.fg,
                            colors.user_msg.bg,
                        );
                        self.input_field.draw(
                            tb,
                            colors,
                            pos_x + nick.len() as i32 + 2,
                            input_field_y,
                        );
                    } else {
                        self.input_field.draw(tb, colors, pos_x, input_field_y);
                    }
                } else {
                    self.input_field.draw(tb, colors, pos_x, input_field_y);
                }
            }
        }
    }

    pub(crate) fn keypressed(&mut self, key: Key) -> WidgetRet {
        match key {
            Key::Ctrl('c') => {
                self.toggle_exit_dialogue();
                WidgetRet::KeyHandled
            }

            Key::Ctrl('u') | Key::PageUp => {
                self.msg_area.page_up();
                WidgetRet::KeyHandled
            }

            Key::Ctrl('d') | Key::PageDown => {
                self.msg_area.page_down();
                WidgetRet::KeyHandled
            }

            Key::ShiftUp => {
                self.msg_area.scroll_up();
                WidgetRet::KeyHandled
            }

            Key::ShiftDown => {
                self.msg_area.scroll_down();
                WidgetRet::KeyHandled
            }

            Key::Tab => {
                if self.exit_dialogue.is_none() {
                    self.input_field.autocomplete(&self.nicks);
                }
                WidgetRet::KeyHandled
            }

            Key::Home => {
                self.msg_area.scroll_top();
                WidgetRet::KeyHandled
            }

            Key::End => {
                self.msg_area.scroll_bottom();
                WidgetRet::KeyHandled
            }

            key => {
                let ret = {
                    if let Some(exit_dialogue) = self.exit_dialogue.as_ref() {
                        exit_dialogue.keypressed(key)
                    } else {
                        self.input_field.keypressed(key)
                    }
                };

                if let WidgetRet::Remove = ret {
                    self.exit_dialogue = None;
                    WidgetRet::KeyHandled
                } else {
                    ret
                }
            }
        }
    }

    fn calculate_input_field_width(&mut self, width: i32) -> i32 {
        let nick_width = match self.current_nick {
            None => 0,
            Some(ref rc) =>
            // +2 for ": "
            {
                rc.len() as i32 + 2
            }
        };

        self.show_current_nick = nick_width as f32 <= width as f32 * (30f32 / 100f32);
        if self.show_current_nick {
            width - nick_width
        } else {
            width
        }
    }

    pub(crate) fn resize(&mut self, width: i32, height: i32) {
        self.width = width;
        self.height = height;
        self.msg_area.resize(width, height - 1);

        let widget_width = self.calculate_input_field_width(width);

        self.input_field.resize(widget_width, height - 1);

        // We don't show the nick in exit dialogue, so it has the full width
        for exit_dialogue in &mut self.exit_dialogue {
            exit_dialogue.resize(width);
        }
    }

    /// Get contents of the input field and clear it.
    pub(crate) fn flush_input_field(&mut self) -> String {
        self.input_field.flush()
    }

    /// Add a line to input field history.
    pub(crate) fn add_input_field_history(&mut self, str: &str) {
        self.input_field.add_history(str)
    }

    /// Set input field contents.
    pub(crate) fn set_input_field(&mut self, str: &str) {
        self.input_field.set(str)
    }

    fn toggle_exit_dialogue(&mut self) {
        let exit_dialogue = ::std::mem::replace(&mut self.exit_dialogue, None);
        if exit_dialogue.is_none() {
            // We don't show the nick in exit dialogue, so it has the full width
            self.exit_dialogue = Some(ExitDialogue::new(self.width));
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

    pub(crate) fn show_topic(&mut self, topic: &str, ts: Timestamp) {
        self.add_timestamp(ts);

        self.msg_area
            .set_style(SegStyle::SchemeStyle(SchemeStyle::Topic));
        self.msg_area.add_text(topic);

        self.msg_area.flush_line();
    }

    pub(crate) fn add_client_err_msg(&mut self, msg: &str) {
        self.reset_activity_line();

        self.msg_area
            .set_style(SegStyle::SchemeStyle(SchemeStyle::ErrMsg));
        self.msg_area.add_text(msg);
        self.msg_area.flush_line();
    }

    pub(crate) fn add_client_notify_msg(&mut self, msg: &str) {
        self.reset_activity_line();

        self.msg_area
            .set_style(SegStyle::SchemeStyle(SchemeStyle::Faded));
        self.msg_area.add_text(msg);
        self.msg_area.flush_line();
        self.reset_activity_line();
    }

    pub(crate) fn add_client_msg(&mut self, msg: &str) {
        self.reset_activity_line();

        self.msg_area
            .set_style(SegStyle::SchemeStyle(SchemeStyle::UserMsg));
        self.msg_area.add_text(msg);
        self.msg_area.flush_line();
        self.reset_activity_line();
    }

    pub(crate) fn add_privmsg(
        &mut self,
        sender: &str,
        msg: &str,
        ts: Timestamp,
        highlight: bool,
        is_action: bool,
    ) {
        self.reset_activity_line();
        self.add_timestamp(ts);

        if is_action {
            self.msg_area
                .set_style(SegStyle::SchemeStyle(SchemeStyle::UserMsg));
            self.msg_area.add_text("** ");
        }

        {
            let nick_color = self.get_nick_color(sender);
            let style = SegStyle::Index(nick_color);
            self.msg_area.set_style(style);
            self.msg_area.add_text(sender);
        }

        self.msg_area
            .set_style(SegStyle::SchemeStyle(SchemeStyle::UserMsg));

        if !is_action {
            self.msg_area.add_char(':');
        }
        self.msg_area.add_char(' ');

        if highlight {
            self.msg_area
                .set_style(SegStyle::SchemeStyle(SchemeStyle::Highlight));
        }

        self.msg_area.add_text(msg);
        self.msg_area.flush_line();
    }

    pub(crate) fn add_msg(&mut self, msg: &str, ts: Timestamp) {
        self.reset_activity_line();

        self.add_timestamp(ts);
        self.msg_area
            .set_style(SegStyle::SchemeStyle(SchemeStyle::UserMsg));
        self.msg_area.add_text(msg);
        self.msg_area.flush_line();
    }

    pub(crate) fn add_err_msg(&mut self, msg: &str, ts: Timestamp) {
        self.reset_activity_line();

        self.add_timestamp(ts);
        self.msg_area
            .set_style(SegStyle::SchemeStyle(SchemeStyle::ErrMsg));
        self.msg_area.add_text(msg);
        self.msg_area.flush_line();
    }

    pub(crate) fn clear(&mut self) {
        self.msg_area.clear();
    }

    fn get_nick_color(&self, sender: &str) -> usize {
        // Anything works as long as it's fast
        let mut hash: usize = 5381;
        for c in sender.chars() {
            hash = hash.wrapping_mul(33).wrapping_add(c as usize);
        }
        hash
    }
}

////////////////////////////////////////////////////////////////////////////////
// Keeping nick list up-to-date

impl MessagingUI {
    pub(crate) fn clear_nicks(&mut self) {
        self.nicks.clear();
    }

    pub(crate) fn join(&mut self, nick: &str, ts: Option<Timestamp>) {
        if self.show_status && !self.nicks.contains(nick) {
            if let Some(ts) = ts {
                let line_idx = self.get_activity_line_idx(ts);
                self.msg_area.modify_line(line_idx, |line| {
                    line.set_style(SegStyle::SchemeStyle(SchemeStyle::Join));
                    line.add_char('+');
                    line.set_style(SegStyle::SchemeStyle(SchemeStyle::Faded));
                    line.add_text(nick);
                });
            }
        }

        self.nicks.insert(nick);
    }

    pub(crate) fn part(&mut self, nick: &str, ts: Option<Timestamp>) {
        self.nicks.remove(nick);

        if self.show_status {
            if let Some(ts) = ts {
                let line_idx = self.get_activity_line_idx(ts);
                self.msg_area.modify_line(line_idx, |line| {
                    line.set_style(SegStyle::SchemeStyle(SchemeStyle::Part));
                    line.add_char('-');
                    line.set_style(SegStyle::SchemeStyle(SchemeStyle::Faded));
                    line.add_text(nick);
                });
            }
        }
    }

    /// `state` == `None` means toggle
    /// `state` == `Some(state)` means set it to `state`
    pub(crate) fn set_or_toggle_ignore(&mut self, state: Option<bool>) {
        self.show_status = state.unwrap_or(!self.show_status);
        if self.show_status {
            self.add_client_notify_msg("Ignore disabled");
        } else {
            self.add_client_notify_msg("Ignore enabled");
        }
    }

    pub(crate) fn get_ignore_state(&self) -> bool {
        self.show_status
    }

    pub(crate) fn nick(&mut self, old_nick: &str, new_nick: &str, ts: Timestamp) {
        self.nicks.remove(old_nick);
        self.nicks.insert(new_nick);

        let line_idx = self.get_activity_line_idx(ts);
        self.msg_area.modify_line(line_idx, |line| {
            line.set_style(SegStyle::SchemeStyle(SchemeStyle::Faded));
            line.add_text(old_nick);
            line.set_style(SegStyle::SchemeStyle(SchemeStyle::Nick));
            line.add_char('>');
            line.set_style(SegStyle::SchemeStyle(SchemeStyle::Faded));
            line.add_text(new_nick);
        });
    }

    fn reset_activity_line(&mut self) {
        self.last_activity_line = None;
    }

    fn get_activity_line_idx(&mut self, ts: Timestamp) -> usize {
        match self.last_activity_line {
            Some(ref l) if l.ts == ts => {
                let line_idx = l.line_idx;
                // FIXME: It's a bit hacky to add a space in this function which from the name
                // looks like a getter.
                // The idea is that we want to add a space *before* adding new stuff, not *after*,
                // to avoid adding redundant spaces. The test `small_screen_1` breaks if we don't
                // get this right.
                self.msg_area
                    .modify_line(line_idx, |line| line.add_char(' '));
                line_idx
            }
            _ => {
                self.add_timestamp(ts);
                let line_idx = self.msg_area.flush_line();
                self.last_activity_line = Some(ActivityLine { ts, line_idx });
                line_idx
            }
        }
    }
}
