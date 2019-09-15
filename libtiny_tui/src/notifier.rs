extern crate notify_rust;

use self::notify_rust::Notification;
use crate::{utils::remove_irc_control_chars, MsgTarget};

/// Destktop notification handler
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum Notifier {
    /// Notifications are disabled.
    Off,
    /// Generate notifications only for mentions.
    Mentions,
    /// Generate notificastions for all messages.
    Messages,
}

fn notify(summary: &str, body: &str) {
    // TODO: Report errors somehow
    let _ = Notification::new().summary(summary).body(body).show();
}

impl Notifier {
    pub(crate) fn notify_privmsg(
        &mut self,
        sender: &str,
        msg: &str,
        target: &MsgTarget,
        our_nick: &str,
        mention: bool,
    ) {
        if our_nick == sender {
            return;
        }

        let msg = remove_irc_control_chars(msg);

        match *target {
            MsgTarget::Chan { chan_name, .. } => {
                if *self == Notifier::Messages || (*self == Notifier::Mentions && mention) {
                    notify(&format!("{} in {}", sender, chan_name), &msg)
                }
            }
            MsgTarget::User {
                nick: ref nick_sender,
                ..
            } => {
                if *self != Notifier::Off {
                    notify(&format!("{} sent a private message", nick_sender), &msg)
                }
            }
            _ => {}
        }
    }
}
