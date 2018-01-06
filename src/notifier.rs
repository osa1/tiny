extern crate notify_rust;

pub use tui::messaging::Timestamp;
use tui::MsgTarget;

use self::notify_rust::Notification;


// Destktop notification handler
pub struct Notifier {
    // [off,mentions,messages]
    notify_for: String,
}

impl Notifier {
    pub fn init(notify_for_: &str) -> Notifier {
        return Notifier { notify_for: notify_for_.to_string() };
    }

    pub fn set_notify_for(&mut self, notify_for_: &str) {
        self.notify_for = notify_for_.to_string();
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
                if self.notify_for == "messages" || (self.notify_for == "mentions" && mention) {
                    if nick != sender {
                        self.notify(&format!("{} in {}", sender, chan_name), &format!("{}", msg))
                    }
                }
            }
            MsgTarget::User { nick: ref nick_sender, .. } => {
                if self.notify_for != "off" {
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
