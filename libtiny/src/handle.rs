use tokio::sync::mpsc;

pub struct ConnHandle {
    // Channel to send IRC messages to send to the server to the sender task.
    chan: mpsc::Sender<String>,
}

pub type SendResult = Result<(), mpsc::error::SendError>;

impl ConnHandle {
    pub fn new(chan: mpsc::Sender<String>) -> ConnHandle {
        ConnHandle { chan }
    }

    //
    // Messaging
    //

    pub async fn send_pass(&mut self, pass: &str) -> SendResult {
        self.chan.send(format!("PASS {}\r\n", pass)).await
    }

    pub async fn send_user(&mut self, hostname: &str, realname: &str) -> SendResult {
        self.chan
            .send(format!("USER {} 8 * :{}\r\n", hostname, realname))
            .await
    }

    pub async fn send_nick(&mut self, nick: &str) -> SendResult {
        self.chan.send(format!("NICK {}\r\n", nick)).await
    }

    pub async fn send_ping(&mut self, arg: &str) -> SendResult {
        self.chan.send(format!("PING {}\r\n", arg)).await
    }

    pub async fn send_pong(&mut self, arg: &str) -> SendResult {
        self.chan.send(format!("PONG {}\r\n", arg)).await
    }

    pub async fn send_join(&mut self, chans: &[&str]) -> SendResult {
        self.chan
            .send(format!("JOIN {}\r\n", chans.join(",")))
            .await
    }

    pub async fn send_part(&mut self, chan: &str) -> SendResult {
        self.chan.send(format!("PART {}\r\n", chan)).await
    }

    pub async fn send_privmsg(&mut self, target: &str, msg: &str) -> SendResult {
        // IRC messages need to be shorter than 512 bytes (see RFC 1459 or 2812). This should be
        // dealt with at call sites as we can't show how we split messages into multiple messages
        // in the UI at this point.
        assert!(target.len() + msg.len() + 12 <= 512);
        self.chan
            .send(format!("PRIVMSG {} :{}\r\n", target, msg))
            .await
    }

    pub async fn send_ctcp_action(&mut self, target: &str, msg: &str) -> SendResult {
        assert!(target.len() + msg.len() + 21 <= 512); // See comments in `privmsg`
        self.chan
            .send(format!("PRIVMSG {} :\x01ACTION {}\x01\r\n", target, msg))
            .await
    }

    pub async fn send_away(&mut self, msg: Option<&str>) -> SendResult {
        self.chan
            .send(match msg {
                None => "AWAY\r\n".to_string(),
                Some(msg) => format!("AWAY :{}\r\n", msg),
            })
            .await
    }

    pub async fn send_cap_ls(&mut self) -> SendResult {
        self.chan.send("CAP LS\r\n".to_string()).await
    }

    pub async fn send_cap_req(&mut self, cap_identifiers: &[&str]) -> SendResult {
        self.chan
            .send(format!("CAP REQ :{}\r\n", cap_identifiers.join(" ")))
            .await
    }

    pub async fn send_cap_end(&mut self) -> SendResult {
        self.chan.send("CAP END\r\n".to_string()).await
    }

    pub async fn send_authenticate(&mut self, msg: &str) -> SendResult {
        self.chan.send(format!("AUTHENTICATE {}\r\n", msg)).await
    }
}
