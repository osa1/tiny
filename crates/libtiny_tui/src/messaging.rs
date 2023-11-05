use termbox_simple::Termbox;

use std::convert::From;

use time::{self, Tm};

use crate::config::Colors;
use crate::exit_dialogue::ExitDialogue;
use crate::input_area::InputArea;
use crate::key_map::KeyAction;
use crate::msg_area::line::SegStyle;
use crate::msg_area::{Layout, MsgArea};
use crate::trie::Trie;
use crate::widget::WidgetRet;

/// An input field and an area for showing messages and activities of a tab (channel, server,
/// mentions tab).
pub(crate) struct MessagingUI {
    /// The area showing the messages and activities.
    msg_area: MsgArea,

    /// The input field. `exit_dialogue` handles the input when available.
    // Two fields (instead of an enum) to avoid borrowchk problems.
    input_field: InputArea,

    exit_dialogue: Option<ExitDialogue>,

    /// Width of the UI, in characters.
    width: i32,

    /// Height of the UI, in lines.
    height: i32,

    /// All nicks in the channel. Used in autocompletion.
    nicks: Trie,

    /// The last line in `msg_area` that shows join, leave, disconnect activities.
    last_activity_line: Option<ActivityLine>,

    /// Last timestamp added to the UI.
    last_ts: Option<Timestamp>,
}

/// Length of ": " suffix of nicks in messages
pub(crate) const MSG_NICK_SUFFIX_LEN: usize = 2;

/// Like `time::Tm`, but we only care about hour and minute parts.
#[derive(PartialEq, Eq, Clone, Copy)]
pub(crate) struct Timestamp {
    hour: i32,
    min: i32,
}

// 80 characters. TODO: We need to make sure we don't need more whitespace than that. We should
// probably add an upper bound to max_nick_length config field?
static WHITESPACE: &str =
    "                                                                                ";

impl Timestamp {
    /// The width of a timestamp plus a space.
    pub(crate) const WIDTH: usize = 6;

    /// Spaces for a timestamp slot in aligned layout.
    pub(crate) const BLANK: &'static str = "      ";

    fn stamp(&self) -> String {
        format!("{:02}:{:02} ", self.hour, self.min)
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

/// A line showing joins, leaves, and disconnects.
struct ActivityLine {
    /// Timestamp of the line.
    ts: Timestamp,

    /// Index of the line in its `MsgArea`.
    line_idx: usize,
}

impl MessagingUI {
    pub(crate) fn new(
        width: i32,
        height: i32,
        scrollback: usize,
        msg_layout: Layout,
    ) -> MessagingUI {
        MessagingUI {
            msg_area: MsgArea::new(width, height - 1, scrollback, msg_layout),
            input_field: InputArea::new(width, get_input_field_max_height(height)),
            exit_dialogue: None,
            width,
            height,
            nicks: Trie::new(),
            last_activity_line: None,
            last_ts: None,
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

    pub(crate) fn keypressed(&mut self, key_action: &KeyAction) -> WidgetRet {
        match key_action {
            KeyAction::Exit => {
                self.toggle_exit_dialogue();
                WidgetRet::KeyHandled
            }
            KeyAction::MessagesPageUp => {
                self.msg_area.page_up();
                WidgetRet::KeyHandled
            }
            KeyAction::MessagesPageDown => {
                self.msg_area.page_down();
                WidgetRet::KeyHandled
            }
            KeyAction::MessagesScrollUp => {
                self.msg_area.scroll_up();
                WidgetRet::KeyHandled
            }
            KeyAction::MessagesScrollDown => {
                self.msg_area.scroll_down();
                WidgetRet::KeyHandled
            }
            KeyAction::MessagesScrollTop => {
                self.msg_area.scroll_top();
                WidgetRet::KeyHandled
            }
            KeyAction::MessagesScrollBottom => {
                self.msg_area.scroll_bottom();
                WidgetRet::KeyHandled
            }
            KeyAction::InputAutoComplete => {
                if self.exit_dialogue.is_none() {
                    self.input_field.autocomplete(&self.nicks);
                }
                WidgetRet::KeyHandled
            }
            key_action => {
                let ret = {
                    if let Some(exit_dialogue) = self.exit_dialogue.as_ref() {
                        exit_dialogue.keypressed(key_action)
                    } else {
                        self.input_field.keypressed(key_action)
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
        if self.exit_dialogue.take().is_none() {
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
    /// Add a new line with the given timestamp (`ts`) if we're not already showing the timestamp.
    ///
    /// In compact layout this adds the indentation for the timestamp column if we're already
    /// showing the timestamp.
    fn add_timestamp(&mut self, ts: Timestamp) {
        if let Some(ts_) = self.last_ts {
            if ts_ != ts {
                self.msg_area.add_text(&ts.stamp(), SegStyle::Timestamp);
            } else if self.msg_area.layout().is_aligned() {
                self.msg_area
                    .add_text(Timestamp::BLANK, SegStyle::Timestamp);
            }
        } else {
            self.msg_area.add_text(&ts.stamp(), SegStyle::Timestamp);
        }
        self.last_ts = Some(ts);
    }

    pub(crate) fn show_topic(&mut self, topic: &str, ts: Timestamp) {
        self.add_timestamp(ts);

        self.msg_area.add_text(topic, SegStyle::Topic);

        self.msg_area.flush_line();
    }

    pub(crate) fn add_client_err_msg(&mut self, msg: &str) {
        self.msg_area.add_text(msg, SegStyle::ErrMsg);
        self.msg_area.flush_line();
    }

    pub(crate) fn add_client_notify_msg(&mut self, msg: &str) {
        self.msg_area.add_text(msg, SegStyle::Faded);
        self.msg_area.flush_line();
    }

    pub(crate) fn add_client_msg(&mut self, msg: &str) {
        self.msg_area.add_text(msg, SegStyle::UserMsg);
        self.msg_area.flush_line();
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

        self.add_timestamp(ts);

        let nick_color = self.get_nick_color(sender);
        let nick_col_style = SegStyle::NickColor(nick_color);

        // actions are /me msgs so they don't show the nick in the nick column, but in the msg
        let layout = self.msg_area.layout();
        let format_nick = |s: &str| -> String {
            if let Layout::Aligned { max_nick_len, .. } = layout {
                let mut aligned = format!("{:>padding$.padding$}", s, padding = max_nick_len);
                if s.len() > max_nick_len {
                    aligned.pop();
                    aligned.push('…');
                }
                aligned
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
            // a space replacing the usual ':'
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
        self.add_timestamp(ts);
        self.msg_area.add_text(msg, SegStyle::UserMsg);
        self.msg_area.flush_line();
    }

    pub(crate) fn add_err_msg(&mut self, msg: &str, ts: Timestamp) {
        self.add_timestamp(ts);
        self.msg_area.add_text(msg, SegStyle::ErrMsg);
        self.msg_area.flush_line();
    }

    pub(crate) fn clear(&mut self) {
        self.msg_area.clear();
        self.last_activity_line = None;
        self.last_ts = None;
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

    pub(crate) fn join(&mut self, nick: &str, ts: Option<Timestamp>, ignore: bool) {
        self.nicks.insert(nick);

        if !ignore {
            if let Some(ts) = ts {
                let line_idx = self.get_activity_line_idx(ts);
                self.msg_area.modify_line(line_idx, |line| {
                    line.add_char('+', SegStyle::Join);
                    line.add_text(nick, SegStyle::Faded);
                });
            }
        }
    }

    pub(crate) fn part(&mut self, nick: &str, ts: Option<Timestamp>, ignore: bool) {
        self.nicks.remove(nick);

        if !ignore {
            if let Some(ts) = ts {
                let line_idx = self.get_activity_line_idx(ts);
                self.msg_area.modify_line(line_idx, |line| {
                    line.add_char('-', SegStyle::Part);
                    line.add_text(nick, SegStyle::Faded);
                });
            }
        }
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

    fn get_activity_line_idx(&mut self, ts: Timestamp) -> usize {
        match &self.last_activity_line {
            Some(l)
                if l.ts == ts && Some(l.line_idx) == self.msg_area.num_lines().checked_sub(1) =>
            {
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
                if let Layout::Aligned { max_nick_len, .. } = self.msg_area.layout() {
                    self.msg_area.add_text(
                        &WHITESPACE[..max_nick_len + MSG_NICK_SUFFIX_LEN],
                        SegStyle::UserMsg,
                    )
                }
                self.msg_area.set_current_line_alignment();
                let line_idx = self.msg_area.flush_line();
                self.last_activity_line = Some(ActivityLine { ts, line_idx });
                line_idx
            }
        }
    }
}
