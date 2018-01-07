extern crate notify_rust;

pub use tui::messaging::Timestamp;
use tui::MsgTarget;

use self::notify_rust::Notification;

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum NotifyFor { Off, Mentions, Messages }

// Destktop notification handler
pub struct Notifier {
    notify_for: NotifyFor,
}

impl Notifier {
    pub fn init(notify_for_: NotifyFor) -> Notifier {
        return Notifier { notify_for: notify_for_ };
    }

    pub fn set_notify_for(&mut self, notify_for_: NotifyFor) {
        self.notify_for = notify_for_;
    }

    fn notify(&mut self, summary: &str, body: &str) {
        Notification::new()
            .summary(summary)
            .body(body)
            .show()
            .unwrap();
    }

    pub fn notify_privmsg(
        &mut self,
        sender: &str,
        msg: &str,
        target: &MsgTarget,
        nick: &str,
        mention: bool,
    ) {
        match *target {
            MsgTarget::Chan { chan_name, .. } => {
                if self.notify_for == NotifyFor::Messages || (self.notify_for == NotifyFor::Mentions && mention) {
                    if nick != sender {
                        self.notify(&format!("{} in {}", sender, chan_name), &format!("{}", msg))
                    }
                }
            }
            MsgTarget::User { nick: ref nick_sender, .. } => {
                if self.notify_for != NotifyFor::Off {
                    if nick != sender {
                        self.notify(
                            &format!("{} sent a private message", nick_sender),
                            &format!("{}", msg),
                        )
                    }
                }
            }
            _ => {}
        }
    }
}
