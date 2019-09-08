use crate::conn::*;
use crate::sasl;
use crate::wire::*;

use std::{
    pin::Pin,
    task::{Context, Poll},
};

use tokio::stream;
use tokio::stream::Stream;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Receiver;

pub struct ServerInfo {
    /// Server address
    pub addr: String,

    /// Server port
    pub port: u16,

    /// Use TLS?
    pub tls: bool,

    /// Server password.
    pub pass: Option<String>,

    pub hostname: String,

    pub realname: String,

    /// Nicks to select when logging in.
    pub nicks: Vec<String>,

    /// Channels to automatically join
    pub auto_join: Vec<String>,

    /// Nickserv password. Sent to NickServ on connecting to the server and nick change, before
    /// join commands.
    pub nickserv_ident: Option<String>,

    /// SASL credentials.
    pub sasl_auth: Option<sasl::Auth>,
}

struct IrcState {
    /// An index to `server_info->nicks`. When out of range we add `current_nick_idx -
    /// server_info.nicks.length()` underscores to the last nick in `server_info.nicks`
    current_nick_idx: usize,

    /// Currently joined channels. Every channel we join will be added here to be able to re-join
    /// automatically on reconnect and channels we leave will be removed.
    ///
    /// Technically a set but we want to join channels in the order given by the user, so using
    /// `Vec` here.
    auto_join: Vec<String>,

    /// Away reason if away mode is on. `None` otherwise. TODO: I don't think the message is used?
    away_status: Option<String>,

    /// servername to be used in PING messages. Read from 002 RPL_YOURHOST. `None` until 002.
    servername: Option<String>,

    /// Our usermask given by the server. Currently only parsed after a JOIN, reply 396.
    ///
    /// Note that RPL_USERHOST (302) does not take cloaks into account, so we don't parse USERHOST
    /// responses to set this field.
    usermask: Option<String>,

    /// Do we have a nick yet? Try another nick on ERR_NICKNAMEINUSE (433) until we've got a nick.
    nick_accepted: bool,
}

// pub struct IrcConn {
//     server_info: ServerInfo,
//     state: IrcState,
//     sender: mpsc::channel<String>,
// }

// Use to send messages or
pub struct IrcConnHandle {
    chan: mpsc::Sender<String>,
}

// Consume this stream to drive the connection.
pub struct IrcConnStream {
    chan: mpsc::Sender<String>,
    rcv_ev: mpsc::Receiver<ConnEv<Msg>>,
}

pub async fn irc_connect(
    server_info: ServerInfo,
) -> Result<(IrcConnHandle, IrcConnStream), ConnectError> {
    let (mut chan, rcv_ev) = connect(&server_info.addr, server_info.port, parse_irc_msg).await?;

    // TODO sasl cap stuff

    //
    // Introduce self
    //

    if let Some(ref pass) = server_info.pass {
        send_pass(&mut chan, pass).await;
    }
    send_nick(&mut chan, &server_info.nicks[0]).await;
    send_user(&mut chan, &server_info.hostname, &server_info.realname);

    let handle = IrcConnHandle { chan: chan.clone() };
    let stream = IrcConnStream { chan, rcv_ev };
    Ok((handle, stream))
}

impl Stream for IrcConnStream {
    type Item = ConnEv<Msg>;

    fn poll_next(mut self: Pin<&mut IrcConnStream>, cx: &mut Context) -> Poll<Option<ConnEv<Msg>>> {
        let poll_ret = {
            // sigh
            let pinned_rcv_ev = unsafe { self.map_unchecked_mut(|self_| &mut self_.rcv_ev) };
            pinned_rcv_ev.poll_next(cx)
        };

        match poll_ret {
            Poll::Pending => Poll::Pending,
            Poll::Ready(None) => Poll::Ready(None), // TODO: wat?
            Poll::Ready(Some(msg)) => {
                // TODO: do the business here
                Poll::Ready(Some(msg))
            }
        }
    }
}

//
// Sending messages
//

type SendResult = Result<(), mpsc::error::SendError>;

async fn send_pass(chan: &mut mpsc::Sender<String>, pass: &str) -> SendResult {
    chan.send(format!("PASS {}\r\n", pass)).await
}

async fn send_user(chan: &mut mpsc::Sender<String>, hostname: &str, realname: &str) -> SendResult {
    chan.send(format!("USER {} 8 * :{}\r\n", hostname, realname))
        .await
}

async fn send_nick(chan: &mut mpsc::Sender<String>, nick: &str) -> SendResult {
    chan.send(format!("NICK {}\r\n", nick)).await
}

async fn send_ping(chan: &mut mpsc::Sender<String>, arg: &str) -> SendResult {
    chan.send(format!("PING {}\r\n", arg)).await
}

async fn send_pong(chan: &mut mpsc::Sender<String>, arg: &str) -> SendResult {
    chan.send(format!("PONG {}\r\n", arg)).await
}

async fn send_join(chan: &mut mpsc::Sender<String>, chans: &[&str]) -> SendResult {
    chan.send(format!("JOIN {}\r\n", chans.join(","))).await
}

async fn send_part(chan: &mut mpsc::Sender<String>, chan_name: &str) -> SendResult {
    chan.send(format!("PART {}\r\n", chan_name)).await
}

async fn send_privmsg(chan: &mut mpsc::Sender<String>, target: &str, msg: &str) -> SendResult {
    // IRC messages need to be shorter than 512 bytes (see RFC 1459 or 2812). This should be
    // dealt with at call sites as we can't show how we split messages into multiple messages
    // in the UI at this point.
    assert!(target.len() + msg.len() + 12 <= 512);
    chan.send(format!("PRIVMSG {} :{}\r\n", target, msg)).await
}

async fn send_ctcp_action(chan: &mut mpsc::Sender<String>, target: &str, msg: &str) -> SendResult {
    assert!(target.len() + msg.len() + 21 <= 512); // See comments in `privmsg`
    chan.send(format!("PRIVMSG {} :\x01ACTION {}\x01\r\n", target, msg))
        .await
}

async fn send_away(chan: &mut mpsc::Sender<String>, msg: Option<&str>) -> SendResult {
    chan.send(match msg {
        None => "AWAY\r\n".to_string(),
        Some(msg) => format!("AWAY :{}\r\n", msg),
    })
    .await
}

async fn send_cap_ls(chan: &mut mpsc::Sender<String>) -> SendResult {
    chan.send("CAP LS\r\n".to_string()).await
}

async fn send_cap_req(chan: &mut mpsc::Sender<String>, cap_identifiers: &[&str]) -> SendResult {
    chan.send(format!("CAP REQ :{}\r\n", cap_identifiers.join(" ")))
        .await
}

async fn send_cap_end(chan: &mut mpsc::Sender<String>) -> SendResult {
    chan.send("CAP END\r\n".to_string()).await
}

async fn send_authenticate(chan: &mut mpsc::Sender<String>, msg: &str) -> SendResult {
    chan.send(format!("AUTHENTICATE {}\r\n", msg)).await
}
