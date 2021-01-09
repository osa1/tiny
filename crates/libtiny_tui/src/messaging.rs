use term_input::Key;
use termbox_simple::Termbox;

use std::convert::From;

use time::{self, Tm};

use crate::config::Colors;
use crate::exit_dialogue::ExitDialogue;
use crate::input_area::InputArea;
use crate::msg_area::line::SegStyle;
use crate::msg_area::{Layout, MsgArea};
use crate::trie::Trie;
use crate::widget::WidgetRet;

/// A messaging screen is just a text field to type messages and msg area to
/// show incoming/sent messages.
pub(crate) struct MessagingUI {
    /// Incoming and sent messages appear
    msg_area: MsgArea,

    // exit_dialogue handles input when available.
    // two fields (instead of an enum etc.) to avoid borrowchk problems
    input_field: InputArea,
    exit_dialogue: Option<ExitDialogue>,

    width: i32,
    height: i32,

    // Option to disable status messages ( join/part )
    show_status: bool,

    // All nicks in the channel. Need to keep this up-to-date to be able to
    // properly highlight mentions.
    nicks: Trie,

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
    /// The width of the timestamp plus a space
    pub(crate) const WIDTH: usize = 6;
    fn stamp(self, msg_area: &mut MsgArea) {
        msg_area.add_text(
            &format!("{:02}:{:02} ", self.hour, self.min),
            SegStyle::Timestamp,
        );
    }

    /// Inserts a blank space that is the size of a timestamp
    fn blank(msg_area: &mut MsgArea) {
        msg_area.add_text(
            &format!("{:^width$}", ' ', width = Timestamp::WIDTH),
            SegStyle::Timestamp,
        );
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
    pub(crate) fn new(
        width: i32,
        height: i32,
        status: bool,
        scrollback: usize,
        msg_layout: Layout,
    ) -> MessagingUI {
        MessagingUI {
            msg_area: MsgArea::new(width, height - 1, scrollback, msg_layout),
            input_field: InputArea::new(width, get_input_field_max_height(height)),
            exit_dialogue: None,
            width,
            height,
            show_status: status,
            nicks: Trie::new(),
            last_activity_line: None,
            last_activity_ts: None,
        }
    }

    pub(crate) fn set_nick(&mut self, nick: String) {
        let nick_color = self.get_nick_color(&nick);
        self.input_field.set_nick(nick, nick_color);
        // update text field size
        let w = self.width;
        let h = self.height;
        self.resize(w, h);
    }

    pub(crate) fn get_nick(&self) -> Option<String> {
        self.input_field.get_nick()
    }

    pub(crate) fn draw(&mut self, tb: &mut Termbox, colors: &Colors, pos_x: i32, pos_y: i32) {
        match &self.exit_dialogue {
            Some(exit_dialogue) => {
                exit_dialogue.draw(tb, colors, pos_x, self.height - 1);
            }
            None => {
                // Draw InputArea first because it can trigger a resize of MsgArea
                self.input_field
                    .draw(tb, colors, pos_x, pos_y, self.height, &mut self.msg_area);
            }
        }
        self.msg_area.draw(tb, colors, pos_x, pos_y);
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

    pub(crate) fn resize(&mut self, width: i32, height: i32) {
        self.width = width;
        self.height = height;

        self.input_field
            .resize(width, get_input_field_max_height(height));
        // msg_area should resize based on input_field's rendered height
        let msg_area_height = height - self.input_field.get_height(width);
        self.msg_area.resize(width, msg_area_height);

        // We don't show the nick in exit dialogue, so it has the full width
        for exit_dialogue in &mut self.exit_dialogue {
            exit_dialogue.resize(width);
        }
    }

    /// Get contents of the input field and cursor location and clear it.
    pub(crate) fn flush_input_field(&mut self) -> (String, i32) {
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

    /// Set cursor location in the input field.
    pub(crate) fn set_cursor(&mut self, cursor: i32) {
        self.input_field.set_cursor(cursor);
    }

    fn toggle_exit_dialogue(&mut self) {
        let exit_dialogue = ::std::mem::replace(&mut self.exit_dialogue, None);
        if exit_dialogue.is_none() {
            // We don't show the nick in exit dialogue, so it has the full width
            self.exit_dialogue = Some(ExitDialogue::new(self.width));
        }
    }
}

/// Calculation for input field's maximum height
fn get_input_field_max_height(window_height: i32) -> i32 {
    window_height / 2
}

////////////////////////////////////////////////////////////////////////////////
// Adding new messages

impl MessagingUI {
    fn add_timestamp(&mut self, ts: Timestamp) {
        if let Some(ts_) = self.last_activity_ts {
            let alignment = matches!(self.msg_area.layout(), Layout::Aligned { .. }); // for now
            if ts_ != ts {
                ts.stamp(&mut self.msg_area);
            } else if alignment {
                Timestamp::blank(&mut self.msg_area)
            }
        } else {
            ts.stamp(&mut self.msg_area);
        }
        self.last_activity_ts = Some(ts);
    }

    pub(crate) fn show_topic(&mut self, topic: &str, ts: Timestamp) {
        self.add_timestamp(ts);

        self.msg_area.add_text(topic, SegStyle::Topic);

        self.msg_area.flush_line();
    }

    pub(crate) fn add_client_err_msg(&mut self, msg: &str) {
        self.reset_activity_line();

        self.msg_area.add_text(msg, SegStyle::ErrMsg);
        self.msg_area.flush_line();
    }

    pub(crate) fn add_client_notify_msg(&mut self, msg: &str) {
        self.reset_activity_line();

        self.msg_area.add_text(msg, SegStyle::Faded);
        self.msg_area.flush_line();
        self.reset_activity_line();
    }

    pub(crate) fn add_client_msg(&mut self, msg: &str) {
        self.reset_activity_line();

        self.msg_area.add_text(msg, SegStyle::UserMsg);
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
        // HACK: Some servers (bridges) don't send RPL_NAMREPLY and JOIN/PART messages but we still
        // want to support tab completion on those servers, so when we see a message from someone
        // we add the user to the nick list so that tab completion will complete their nick. See
        // #253 for details.
        self.nicks.insert(sender);

        self.reset_activity_line();
        self.add_timestamp(ts);

        let nick_color = self.get_nick_color(sender);
        let nick_col_style = SegStyle::NickColor(nick_color);

        // actions are /me msgs so they don't show the nick in the nick column, but in the msg
        let layout = self.msg_area.layout();
        let format_nick = |s: &str| -> String {
            if let Layout::Aligned { max_nick_len, .. } = layout {
                format!("{:>padding$.padding$}", s, padding = max_nick_len)
            } else {
                s.to_string()
            }
        };
        if is_action {
            self.msg_area
                .add_text(&format_nick("**"), SegStyle::UserMsg);
            // separator between nick and msg
            self.msg_area.add_text("  ", SegStyle::Faded);
            self.msg_area.add_text(sender, nick_col_style);
            // a space replacing the :
            self.msg_area.add_text(" ", SegStyle::UserMsg);
        } else {
            self.msg_area.add_text(&format_nick(sender), nick_col_style);
            // separator between nick and msg
            self.msg_area.add_text(": ", SegStyle::Faded);
        }

        let msg_style = if highlight {
            SegStyle::Highlight
        } else {
            SegStyle::UserMsg
        };

        self.msg_area.add_text(msg, msg_style);
        self.msg_area.set_current_line_alignment();
        self.msg_area.flush_line();
    }

    pub(crate) fn add_msg(&mut self, msg: &str, ts: Timestamp) {
        self.reset_activity_line();

        self.add_timestamp(ts);
        self.msg_area.add_text(msg, SegStyle::UserMsg);
        self.msg_area.flush_line();
    }

    pub(crate) fn add_err_msg(&mut self, msg: &str, ts: Timestamp) {
        self.reset_activity_line();

        self.add_timestamp(ts);
        self.msg_area.add_text(msg, SegStyle::ErrMsg);
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
        if self.show_status {
            if let Some(ts) = ts {
                let line_idx = self.get_activity_line_idx(ts);
                self.msg_area.modify_line(line_idx, |line| {
                    line.add_char('+', SegStyle::Join);
                    line.add_text(nick, SegStyle::Faded);
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
                    line.add_char('-', SegStyle::Part);
                    line.add_text(nick, SegStyle::Faded);
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

    pub(crate) fn is_showing_status(&self) -> bool {
        self.show_status
    }

    pub(crate) fn nick(&mut self, old_nick: &str, new_nick: &str, ts: Timestamp) {
        self.nicks.remove(old_nick);
        self.nicks.insert(new_nick);

        let line_idx = self.get_activity_line_idx(ts);
        self.msg_area.modify_line(line_idx, |line| {
            line.add_text(old_nick, SegStyle::Faded);
            line.add_char('>', SegStyle::Nick);
            line.add_text(new_nick, SegStyle::Faded);
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
                    .modify_line(line_idx, |line| line.add_char(' ', SegStyle::UserMsg));
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
