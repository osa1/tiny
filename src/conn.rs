use mio::Poll;
use mio::Token;
use std::io::Write;
use std::result;
use std::str;

use config;
use logger::LogFile;
use logger::Logger;
use utils;
use wire::{Cmd, Msg, Pfx};
use wire;
use stream::{Stream, StreamErr};

pub struct Conn<'poll> {
    serv_addr: String,
    serv_port: u16,
    tls: bool,
    hostname: String,
    realname: String,

    /// Server password
    pass: Option<String>,

    nicks: Vec<String>,

    /// Always in range of `nicks`
    current_nick_idx: usize,

    /// Channels to auto-join. Every channel we join will be added here to be able to re-join
    /// automatically on reconnect and channels we leave will be removed.
    auto_join: Vec<String>,

    /// Away reason if away mode is on. `None` otherwise.
    away_status: Option<String>,

    /// servername to be used in PING messages. Read from 002 RPL_YOURHOST.
    /// `None` until 002.
    servername: Option<String>,

    /// Our usermask given by the server. Currently only parsed after a JOIN,
    /// reply 396.
    ///
    /// Note that RPL_USERHOST (302) does not take cloaks into account, so we
    /// don't parse USERHOST responses to set this field.
    usermask: Option<String>,

    poll: &'poll Poll,

    status: ConnStatus<'poll>,

    /// Incoming message buffer
    in_buf: Vec<u8>,
}

pub type ConnErr = StreamErr;

/// How many ticks to wait before sending a ping to the server.
const PING_TICKS: u8 = 60;
/// How many ticks to wait after sending a ping to the server to consider a
/// disconnect.
const PONG_TICKS: u8 = 60;
/// How many ticks to wait after a disconnect or a socket error.
pub const RECONNECT_TICKS: u8 = 30;

enum ConnStatus<'poll> {
    PingPong {
        /// Ticks passed since last time we've heard from the server. Reset on
        /// each message. After `PING_TICKS` ticks we send a PING message and
        /// move to `WaitPong` state.
        ticks_passed: u8,
        stream: Stream<'poll>,
    },
    WaitPong {
        /// Ticks passed since we sent a PING to the server. After a message
        /// move to `PingPong` state. On timeout we reset the connection.
        ticks_passed: u8,
        stream: Stream<'poll>,
    },
    Disconnected {
        ticks_passed: u8,
    },
}

macro_rules! update_status {
    ($self:ident, $v:ident, $code:expr) => {{
        // temporarily putting `Disconnected` to `self.status`
        let $v = ::std::mem::replace(&mut $self.status, ConnStatus::Disconnected { ticks_passed: 0 });
        let new_status = $code;
        $self.status = new_status;
    }}
}

impl<'poll> ConnStatus<'poll> {
    fn get_stream(&self) -> Option<&Stream<'poll>> {
        use self::ConnStatus::*;
        match *self {
            PingPong { ref stream, .. }
            | WaitPong { ref stream, .. } =>
                Some(stream),
            Disconnected { .. } =>
                None,
        }
    }

    fn get_stream_mut(&mut self) -> Option<&mut Stream<'poll>> {
        use self::ConnStatus::*;
        match *self {
            PingPong { ref mut stream, .. }
            | WaitPong { ref mut stream, .. } =>
                Some(stream),
            Disconnected { .. } =>
                None,
        }
    }
}

pub type Result<T> = result::Result<T, StreamErr>;

#[derive(Debug)]
pub enum ConnEv {
    /// Connected to the server + registered
    Connected,
    ///
    Disconnected,
    /// Hack to return the main loop that the Conn wants reconnect()
    WantReconnect,
    /// Network error happened
    Err(StreamErr),
    /// An incoming message
    Msg(Msg),
    /// Nick changed
    NickChange(String),
}

fn introduce<W: Write>(stream: &mut W, pass: Option<&str>, hostname: &str, realname: &str, nick: &str) {
    if let Some(pass) = pass {
        wire::pass(stream, pass).unwrap();
    }
    wire::nick(stream, nick).unwrap();
    wire::user(stream, hostname, realname).unwrap();
}

impl<'poll> Conn<'poll> {
    pub fn new(server: config::Server, poll: &'poll Poll) -> Result<Conn<'poll>> {
        let mut stream =
            Stream::new(poll, &server.addr, server.port, server.tls).map_err(StreamErr::from)?;

        introduce(
            &mut stream,
            server.pass.as_ref().map(String::as_str),
            &server.hostname,
            &server.realname,
            &server.nicks[0],
        );

        Ok(Conn {
            serv_addr: server.addr,
            serv_port: server.port,
            tls: server.tls,
            hostname: server.hostname,
            realname: server.realname,
            pass: server.pass,
            nicks: server.nicks,
            current_nick_idx: 0,
            auto_join: server.join,
            away_status: None,
            servername: None,
            usermask: None,
            poll,
            status: ConnStatus::PingPong {
                ticks_passed: 0,
                stream: stream,
            },
            in_buf: vec![],
        })
    }

    pub fn reconnect(&mut self, new_serv: Option<(&str, u16)>) -> Result<()> {
        // drop existing connection first
        let old_stream = ::std::mem::replace(
            &mut self.status,
            ConnStatus::Disconnected { ticks_passed: 0 },
        );
        drop(old_stream);

        if let Some((new_name, new_port)) = new_serv {
            self.serv_addr = new_name.to_owned();
            self.serv_port = new_port;
        }
        match Stream::new(self.poll, &self.serv_addr, self.serv_port, self.tls) {
            Err(err) =>
                Err(StreamErr::from(err)),
            Ok(mut stream) => {
                introduce(
                    &mut stream,
                    self.pass.as_ref().map(String::as_str),
                    &self.hostname,
                    &self.realname,
                    self.get_nick()
                );
                self.status = ConnStatus::PingPong {
                    ticks_passed: 0,
                    stream: stream,
                };
                self.current_nick_idx = 0;
                Ok(())
            }
        }
    }

    pub fn get_conn_tok(&self) -> Option<Token> {
        self.status.get_stream().map(|s| s.get_tok())
    }

    pub fn get_serv_name(&self) -> &str {
        &self.serv_addr
    }

    pub fn get_nick(&self) -> &str {
        &self.nicks[self.current_nick_idx]
    }

    pub fn set_nick(&mut self, nick: &str) {
        if let Some(nick_idx) = self.nicks.iter().position(|n| n == nick) {
            self.current_nick_idx = nick_idx;
        } else {
            self.nicks.push(nick.to_owned());
            self.current_nick_idx = self.nicks.len() - 1;
        }
        self.send_nick();
    }

    fn next_nick(&mut self) {
        if self.current_nick_idx + 1 == self.nicks.len() {
            let mut new_nick = self.nicks.last().unwrap().to_string();
            new_nick.push('_');
            self.nicks.push(new_nick);
        }
        self.current_nick_idx += 1;
    }
}

impl<'poll> Conn<'poll> {
    pub fn enter_disconnect_state(&mut self) {
        self.status = ConnStatus::Disconnected { ticks_passed: 0 };
    }

    ////////////////////////////////////////////////////////////////////////////
    // Tick handling

    pub fn tick(&mut self, evs: &mut Vec<ConnEv>, mut debug_out: LogFile) {
        update_status!(
            self,
            status,
            match status {
                ConnStatus::PingPong {
                    mut stream,
                    ticks_passed,
                } => {
                    let ticks = ticks_passed + 1;
                    if ticks == PING_TICKS {
                        match self.servername {
                            None => {
                                debug_out.write_line(format_args!(
                                    "{}: Can't send PING, servername unknown",
                                    self.serv_addr
                                ));
                            }
                            Some(ref host_) => {
                                debug_out.write_line(format_args!(
                                    "{}: Ping timeout, sending PING",
                                    self.serv_addr
                                ));
                                wire::ping(&mut stream, host_).unwrap();
                            }
                        }
                        ConnStatus::WaitPong {
                            stream,
                            ticks_passed: 0,
                        }
                    } else {
                        ConnStatus::PingPong {
                            stream,
                            ticks_passed: ticks,
                        }
                    }
                }
                ConnStatus::WaitPong {
                    stream,
                    ticks_passed,
                } => {
                    let ticks = ticks_passed + 1;
                    if ticks == PONG_TICKS {
                        evs.push(ConnEv::Disconnected);
                        ConnStatus::Disconnected { ticks_passed: 0 }
                    } else {
                        ConnStatus::WaitPong {
                            stream,
                            ticks_passed: ticks,
                        }
                    }
                }
                ConnStatus::Disconnected { ticks_passed } => {
                    let ticks = ticks_passed + 1;
                    if ticks_passed + 1 == RECONNECT_TICKS {
                        // *sigh* it's slightly annoying that we can't reconnect here, we need to
                        // update the event loop
                        evs.push(ConnEv::WantReconnect);
                        self.current_nick_idx = 0;
                    }
                    ConnStatus::Disconnected {
                        ticks_passed: ticks,
                    }
                }
            }
        );
    }

    fn reset_ticks(&mut self) {
        update_status!(
            self,
            status,
            match status {
                ConnStatus::PingPong { stream, .. } =>
                    ConnStatus::PingPong { ticks_passed: 0, stream },
                ConnStatus::WaitPong { stream, .. } =>
                    // no bug: we heard something from the server, whether it was a pong or not
                    // doesn't matter that much, connectivity is fine.
                    ConnStatus::PingPong { ticks_passed: 0, stream },
                ConnStatus::Disconnected { .. } =>
                    status,
            }
        );
    }

    ////////////////////////////////////////////////////////////////////////////
    // Sending messages

    fn send_nick(&mut self) {
        let nick = &self.nicks[self.current_nick_idx];
        self.status.get_stream_mut().map(|stream| {
            wire::nick(stream, nick).unwrap();
        });
    }

    /// `extra_len`: Size (in bytes) for a prefix/suffix etc. that'll be added to each line.
    /// Strings returned by the iterator will have enough room for that.
    pub fn split_privmsg<'a>(&self, extra_len: i32, msg: &'a str) -> utils::SplitIterator<'a> {
        // Max msg len calculation adapted from hexchat
        // (src/common/outbound.c:split_up_text)
        let mut max: i32 = 512; // RFC 2812
        max -= 3; // :, !, @
        max -= 13; // " PRIVMSG ", " ", :, \r, \n
        max -= self.get_nick().len() as i32;
        max -= extra_len;
        match self.usermask {
            None => {
                max -= 9; // max username
                max -= 64; // max possible hostname (63) + '@'
                           // NOTE(osa): I think hexchat has an error here, it
                           // uses 65
            }
            Some(ref usermask) => {
                max -= usermask.len() as i32;
            }
        }

        assert!(max > 0);

        utils::split_iterator(msg, max as usize)
    }

    // FIXME: This crashes with an assertion error when the message is too long
    // to fit into 512 bytes. Need to make sure `split_privmsg` is called before
    // this.
    pub fn privmsg(&mut self, target: &str, msg: &str) {
        self.status.get_stream_mut().map(|stream| {
            wire::privmsg(stream, target, msg).unwrap();
        });
    }

    pub fn ctcp_action(&mut self, target: &str, msg: &str) {
        self.status.get_stream_mut().map(|stream| {
            wire::ctcp_action(stream, target, msg).unwrap();
        });
    }

    pub fn join(&mut self, chans: &[&str]) {
        self.status.get_stream_mut().map(|stream| {
            wire::join(stream, chans).unwrap();
        });
        // the channel will be added to auto-join list on successful join (i.e.
        // after RPL_TOPIC)
    }

    pub fn part(&mut self, chan: &str) {
        self.status.get_stream_mut().map(|stream| {
            wire::part(stream, chan).unwrap();
        });
        self.auto_join.drain_filter(|chan_| chan_ == chan);
    }

    pub fn away(&mut self, msg: Option<&str>) {
        self.away_status = msg.map(|s| s.to_string());
        self.status.get_stream_mut().map(|stream| {
            wire::away(stream, msg).unwrap();
        });
    }

    pub fn raw_msg(&mut self, msg: &str) {
        self.status.get_stream_mut().map(|stream| {
            use std::io::Write;
            write!(stream, "{}\r\n", msg).unwrap();
        });
    }

    ////////////////////////////////////////////////////////////////////////////
    // Sending messages

    pub fn write_ready(&mut self, evs: &mut Vec<ConnEv>) {
        if let Some(stream) = self.status.get_stream_mut() {
            match stream.write_ready() {
                Err(err) =>
                    if !err.is_would_block() {
                        evs.push(ConnEv::Err(StreamErr::from(err)));
                    },
                Ok(()) =>
                    {}
            }
        }
    }

    ////////////////////////////////////////////////////////////////////////////
    // Receiving messages

    pub fn read_ready(&mut self, evs: &mut Vec<ConnEv>, logger: &mut Logger) {
        let mut read_buf: [u8; 512] = [0; 512];

        if let Some(stream) = self.status.get_stream_mut() {
            match stream.read_ready(&mut read_buf) {
                Err(err) => {
                    if !err.is_would_block() {
                        evs.push(ConnEv::Err(StreamErr::from(err)));
                    }
                }
                Ok(bytes_read) => {
                    self.reset_ticks();
                    self.in_buf.extend(&read_buf[0..bytes_read]);
                    self.handle_msgs(evs, logger);
                }
            }
        }
    }

    fn handle_msgs(&mut self, evs: &mut Vec<ConnEv>, logger: &mut Logger) {
        while let Some(msg) = Msg::read(
            &mut self.in_buf,
            Some(logger.get_raw_serv_logs(&self.serv_addr)),
        ) {
            self.handle_msg(msg, evs, logger);
        }
    }

    fn handle_msg(&mut self, msg: Msg, evs: &mut Vec<ConnEv>, logger: &mut Logger) {
        if let Msg {
            cmd: Cmd::PING { ref server },
            ..
        } = msg
        {
            self.status.get_stream_mut().map(|stream| {
                wire::pong(stream, server).unwrap();
            });
        }

        if let Msg {
            cmd: Cmd::JOIN { .. },
            pfx: Some(Pfx::User { ref nick, ref user }),
        } = msg
        {
            if nick == self.get_nick() {
                let usermask = format!("{}!{}", nick, user);
                logger
                    .get_debug_logs()
                    .write_line(format_args!("usermask set: {}", usermask));
                self.usermask = Some(usermask);
            }
        }

        if let Msg {
            cmd: Cmd::Reply {
                num: 396,
                ref params,
            },
            ..
        } = msg
        {
            // :hobana.freenode.net 396 osa1 haskell/developer/osa1
            // :is now your hidden host (set by services.)
            if params.len() == 3 {
                let usermask = format!("{}!~{}@{}", self.get_nick(), self.hostname, params[1]);
                logger
                    .get_debug_logs()
                    .write_line(format_args!("usermask set: {}", usermask));
                self.usermask = Some(usermask);
            }
        }

        if let Msg {
            cmd: Cmd::Reply {
                num: 302,
                ref params,
            },
            ..
        } = msg
        {
            // 302 RPL_USERHOST
            // :ircd.stealth.net 302 yournick :syrk=+syrk@millennium.stealth.net
            //
            // We know there will be only one nick because /userhost cmd sends
            // one parameter (our nick)
            //
            // Example args: ["osa1", "osa1=+omer@moz-s8a.9ac.93.91.IP "]

            let param = &params[1];
            match wire::find_byte(param.as_bytes(), b'=') {
                None => {
                    logger
                        .get_debug_logs()
                        .write_line(format_args!("can't parse RPL_USERHOST: {}", params[1]));
                }
                Some(mut i) => {
                    if param.as_bytes().get(i + 1) == Some(&b'+')
                        || param.as_bytes().get(i + 1) == Some(&b'-')
                    {
                        i += 1;
                    }
                    let usermask = (&param[i..]).trim();
                    self.usermask = Some(usermask.to_owned());
                    logger
                        .get_debug_logs()
                        .write_line(format_args!("usermask set: {}", usermask));
                }
            }
        }

        if let Msg {
            cmd: Cmd::Reply { num: 001, .. },
            ..
        } = msg
        {
            // 001 RPL_WELCOME is how we understand that the registration was successful
            evs.push(ConnEv::Connected);
            evs.push(ConnEv::NickChange(self.get_nick().to_owned()));
        }

        if let Msg {
            cmd: Cmd::Reply {
                num: 002,
                ref params,
            },
            ..
        } = msg
        {
            // 002    RPL_YOURHOST
            //        "Your host is <servername>, running version <ver>"

            // An example <servername>: cherryh.freenode.net[149.56.134.238/8001]

            match parse_servername(params) {
                None => {
                    logger.get_debug_logs().write_line(format_args!(
                        "{} Can't parse hostname from params: {:?}",
                        self.serv_addr,
                        params
                    ));
                }
                Some(servername) => {
                    logger
                        .get_debug_logs()
                        .write_line(format_args!("{} host: {}", self.serv_addr, servername));
                    self.servername = Some(servername);
                }
            }
        }

        if let Msg {
            cmd: Cmd::Reply { num: 433, .. },
            ..
        } = msg
        {
            // ERR_NICKNAMEINUSE
            self.next_nick();
            self.send_nick();
            evs.push(ConnEv::NickChange(self.get_nick().to_owned()));
        }

        // Not in any of the RFCs. Also known as ERR_BANONCHAN on the internets.
        // Sent by freenode when nick change failed. See issue #29.
        if let Msg {
            cmd: Cmd::Reply {
                num: 435,
                ref params,
            },
            ..
        } = msg
        {
            if params.len() == 4 {
                // args: [old_nick, new_nick, chan, msg]
                let old_nick = &params[0];
                // make current nick 'old_nick'
                for (nick_idx, nick) in self.nicks.iter().enumerate() {
                    if nick == old_nick {
                        self.current_nick_idx = nick_idx;
                        evs.push(ConnEv::NickChange(self.get_nick().to_owned()));
                        break;
                    }
                }
            }
        }

        if let Msg {
            cmd: Cmd::Reply { num: 376, .. },
            ..
        } = msg
        {
            if let Some(mut stream) = self.status.get_stream_mut() {
                // RPL_ENDOFMOTD. Join auto-join channels.
                if !self.auto_join.is_empty() {
                    wire::join(
                        &mut stream,
                        self.auto_join
                            .iter()
                            .map(String::as_str)
                            .collect::<Vec<&str>>()
                            .as_slice(),
                    ).unwrap();
                }

                // Set away mode
                if let Some(ref reason) = self.away_status {
                    wire::away(stream, Some(reason)).unwrap();
                }
            }
        }

        if let Msg {
            cmd: Cmd::Reply {
                num: 332,
                ref params,
            },
            ..
        } = msg
        {
            if params.len() == 2 || params.len() == 3 {
                // RPL_TOPIC. We've successfully joined a channel, add the channel to
                // self.auto_join to be able to auto-join next time we connect
                let chan = &params[params.len() - 2];
                if !self.auto_join.contains(chan) {
                    self.auto_join.push(chan.to_owned());
                }
            }
        }

        evs.push(ConnEv::Msg(msg));
    }
}

/// Try to parse servername in a 002 RPL_YOURHOST reply
fn parse_servername(params: &[String]) -> Option<String> {
    let msg = params.get(1).or_else(|| params.get(0))?;
    let slice1 = &msg[13..];
    let servername_ends =
        wire::find_byte(slice1.as_bytes(), b'[')
            .or_else(|| wire::find_byte(slice1.as_bytes(), b','))?;
    Some((&slice1[..servername_ends]).to_owned())
}

////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_servername_1() {
        let args = vec![
            "tiny_test".to_owned(),
            "Your host is adams.freenode.net[94.125.182.252/8001], \
             running version ircd-seven-1.1.4"
                .to_owned(),
        ];
        assert_eq!(
            parse_servername(&args),
            Some("adams.freenode.net".to_owned())
        );
    }

    #[test]
    fn test_parse_servername_2() {
        let args = vec![
            "tiny_test".to_owned(),
            "Your host is belew.mozilla.org, running version InspIRCd-2.0".to_owned(),
        ];
        assert_eq!(
            parse_servername(&args),
            Some("belew.mozilla.org".to_owned())
        );
    }
}
