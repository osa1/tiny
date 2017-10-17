use mio::Poll;
use mio::PollOpt;
use mio::Ready;
use mio::Token;
use mio::unix::EventedFd;
use net2::TcpBuilder;
use net2::TcpStreamExt;
use std::collections::HashSet;
use std::io::Read;
use std::io::Write;
use std::io;
use std::net::SocketAddr;
use std::net::TcpStream;
use std::net::ToSocketAddrs;
use std::os::unix::io::{AsRawFd, RawFd};
use std::str;

use config;
use logger::LogFile;
use logger::Logger;
use utils;
use wire::{Cmd, Msg, Pfx};
use wire;

pub struct Conn<'poll> {
    serv_addr: String,
    serv_port: u16,
    hostname: String,
    realname: String,
    nicks: Vec<String>,

    /// Always in range of `nicks`
    current_nick_idx: usize,

    /// Channels to auto-join. Initially empty, every channel we join will be
    /// added here to be able to re-join automatically on reconnect.
    auto_join: HashSet<String>,

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

    /// The TCP connection to the server.
    stream: TcpStream,

    poll: &'poll Poll,

    status: ConnStatus,

    /// Incoming message buffer
    in_buf: Vec<u8>,

    /// Outgoing message buffer. Make sure to register the socket for rw events
    /// after writing to this buffer.
    out_buf: Vec<u8>,
}

fn deregister_fd(poll: &Poll, fd: RawFd) {
    // deregistering multiple times is fine .. I think
    // plus we call this in drop() so we shoudn't panic
    let _ = poll.deregister(&EventedFd(&fd));
}

impl<'poll> Drop for Conn<'poll> {
    fn drop(&mut self) {
        deregister_fd(self.poll, self.get_raw_fd());
    }
}

/// How many ticks to wait before sending a ping to the server.
const PING_TICKS: u8 = 60;
/// How many ticks to wait after sending a ping to the server to consider a
/// disconnect.
const PONG_TICKS: u8 = 60;
/// How many ticks to wait after a disconnect or a socket error.
pub const RECONNECT_TICKS: u8 = 30;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum ConnStatus {
    /// Need to introduce self
    Introduce,
    PingPong {
        /// Ticks passed since last time we've heard from the server. Reset on
        /// each message. After `PING_TICKS` ticks we send a PING message and
        /// move to `WaitPong` state.
        ticks_passed: u8,
    },
    WaitPong {
        /// Ticks passed since we sent a PING to the server. After a message
        /// move to `PingPong` state. On timeout we reset the connection.
        ticks_passed: u8,
    },
    Disconnected {
        ticks_passed: u8,
    },
}

#[derive(Debug)]
pub enum ConnEv {
    /// Connected to the server + registered
    Connected,
    ///
    Disconnected,
    /// Hack to return the main loop that the Conn wants reconnect()
    WantReconnect,
    /// Network error happened
    Err(io::Error),
    /// An incoming message
    Msg(Msg),
    /// Nick changed
    NickChange(String),
}

fn init_stream(serv_addr: &str, serv_port: u16) -> TcpStream {
    // FIXME: This socket will return an error and make the Conn enter into
    // "reconnnect" loop. There should be a more direct way of doing this.
    fn mk_useless_stream() -> TcpStream {
        let stream = TcpBuilder::new_v4().unwrap().to_tcp_stream().unwrap();
        stream.set_nonblocking(true).unwrap();
        stream
    }

    // FIXME: This will block the thread. See issue #3.
    // FIXME: This part is really horrible. No way to report errors. The `Conn`
    // will just try to reconnect in case of an error.
    match (serv_addr, serv_port).to_socket_addrs() {
        Err(_) => {
            mk_useless_stream()
        },
        Ok(mut addr_iter) => {
            match addr_iter.next() {
                None => {
                    mk_useless_stream()
                }
                Some(SocketAddr::V4(addr)) => {
                    let stream = TcpBuilder::new_v4().unwrap().to_tcp_stream().unwrap();
                    stream.set_nonblocking(true).unwrap();
                    // This will fail with EINPROGRESS
                    let _ = stream.connect(SocketAddr::V4(addr));
                    stream
                },
                Some(SocketAddr::V6(addr)) => {
                    let stream = TcpBuilder::new_v6().unwrap().to_tcp_stream().unwrap();
                    stream.set_nonblocking(true).unwrap();
                    // This will fail with EINPROGRESS
                    let _ = stream.connect(SocketAddr::V6(addr));
                    stream
                }
            }
        }
    }
}

fn reregister_for_rw(poll: &Poll, fd: RawFd) {
    // fails when not already registered, ignore result
    // (e.g. between a disconnect and reconnect)
    let _ = poll.reregister(
        &EventedFd(&fd),
        Token(fd as usize),
        Ready::readable() | Ready::writable(),
        PollOpt::level());
}

fn reregister_for_r(poll: &Poll, fd: RawFd) {
    // fails when not already registered, ignore result
    // (e.g. between a disconnect and reconnect)
    let _ = poll.reregister(
        &EventedFd(&fd),
        Token(fd as usize),
        Ready::readable(),
        PollOpt::level());
}

impl<'poll> Conn<'poll> {
    pub fn from_server(server: config::Server, poll: &'poll Poll) -> Conn {
        let stream = init_stream(&server.addr, server.port);
        let ret = Conn {
            serv_addr: server.addr,
            serv_port: server.port,
            hostname: server.hostname,
            realname: server.realname,
            nicks: server.nicks,
            current_nick_idx: 0,
            auto_join: HashSet::new(),
            away_status: None,
            servername: None,
            usermask: None,
            stream: stream,
            poll: poll,
            status: ConnStatus::Introduce,
            in_buf: vec![],
            out_buf: vec![],
        };
        ret.register_for_r();
        ret
    }

    /// Clone an existing connection, but update the server address.
    pub fn from_conn(conn: &Conn<'poll>, new_serv_addr: &str, new_serv_port: u16) -> Conn<'poll> {
        let stream = init_stream(new_serv_addr, new_serv_port);
        let ret = Conn {
            serv_addr: new_serv_addr.to_owned(),
            serv_port: new_serv_port,
            hostname: conn.hostname.clone(),
            realname: conn.realname.clone(),
            nicks: conn.nicks.clone(),
            current_nick_idx: 0,
            auto_join: HashSet::new(),
            away_status: None,
            servername: None,
            usermask: None,
            stream: stream,
            poll: conn.poll,
            status: ConnStatus::Introduce,
            in_buf: vec![],
            out_buf: vec![],
        };
        ret.register_for_r();
        ret
    }

    /// Register self to the Poll for read events.
    fn register_for_r(&self) {
        let fd = self.get_raw_fd();
        self.poll.register(
            &EventedFd(&fd),
            Token(fd as usize),
            Ready::readable(),
            PollOpt::level()).unwrap();
    }

    /// Re-register self to the Poll for read events.
    fn reregister_for_r(&self) {
        reregister_for_r(self.poll, self.get_raw_fd());
    }

    /// Re-register self to the Poll for read and write events.
    fn reregister_for_rw(&self) {
        reregister_for_rw(self.poll, self.get_raw_fd());
    }

    /// De-register self. Do this after a connection error.
    fn deregister(&self) {
        deregister_fd(self.poll, self.get_raw_fd());
    }

    pub fn reconnect(&mut self, new_serv: Option<(&str, u16)>) {
        self.deregister();
        if let Some((new_name, new_port)) = new_serv {
            self.serv_addr = new_name.to_owned();
            self.serv_port = new_port;
        }
        self.stream = init_stream(&self.serv_addr, self.serv_port);
        self.status = ConnStatus::Introduce;
        self.current_nick_idx = 0;
        self.out_buf.clear();
        self.register_for_r();
    }

    /// Get the RawFd, to be used with select() or other I/O multiplexer.
    fn get_raw_fd(&self) -> RawFd {
        self.stream.as_raw_fd()
    }

    pub fn get_conn_tok(&self) -> Token {
        Token(self.get_raw_fd() as usize)
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

    fn reset_nick(&mut self) {
        self.current_nick_idx = 0;
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

    ////////////////////////////////////////////////////////////////////////////
    // Tick handling

    pub fn tick(&mut self, evs: &mut Vec<ConnEv>, mut debug_out: LogFile) {
        match self.status {
            ConnStatus::Introduce => {},
            ConnStatus::PingPong { ticks_passed } => {
                if ticks_passed + 1 == PING_TICKS {
                    match self.servername {
                        None => {
                            debug_out.write_line(
                                format_args!("{}: Can't send PING, servername unknown",
                                             self.serv_addr));
                        }
                        Some(ref host_) => {
                            debug_out.write_line(
                                format_args!("{}: Ping timeout, sending PING",
                                             self.serv_addr));
                            wire::ping(&mut self.out_buf, host_).unwrap();
                            reregister_for_rw(self.poll, self.get_raw_fd());
                        }
                    }
                    self.status = ConnStatus::WaitPong { ticks_passed: 0 };
                } else {
                    self.status = ConnStatus::PingPong { ticks_passed: ticks_passed + 1 };
                }
            }
            ConnStatus::WaitPong { ticks_passed } => {
                if ticks_passed + 1 == PONG_TICKS {
                    evs.push(ConnEv::Disconnected);
                    self.enter_disconnect_state();
                } else {
                    self.status = ConnStatus::WaitPong { ticks_passed: ticks_passed + 1 };
                }
            }
            ConnStatus::Disconnected { ticks_passed } => {
                if ticks_passed + 1 == RECONNECT_TICKS {
                    // *sigh* it's slightly annoying that we can't reconnect here, we need to
                    // update the event loop
                    evs.push(ConnEv::WantReconnect);
                    self.reset_nick();
                }
                self.status = ConnStatus::Disconnected { ticks_passed: ticks_passed + 1 };
            }
        }
    }

    pub fn enter_disconnect_state(&mut self) {
        self.status = ConnStatus::Disconnected { ticks_passed: 0 };
        self.deregister();
    }

    fn reset_ticks(&mut self) {
        match self.status {
            ConnStatus::Introduce => {},
            _ => { self.status = ConnStatus::PingPong { ticks_passed: 0 }; }
        }
    }

    ////////////////////////////////////////////////////////////////////////////
    // Sending messages

    fn introduce(&mut self) {
        wire::user(&mut self.out_buf, &self.hostname, &self.realname).unwrap();
        self.send_nick();
    }

    fn send_nick(&mut self) {
        wire::nick(&mut self.out_buf, &self.nicks[self.current_nick_idx]).unwrap();
        self.reregister_for_rw();
    }

    pub fn split_privmsg<'a>(&self, target: &'a str, msg: &'a str) -> utils::SplitIterator<'a> {
        // Max msg len calculation adapted from hexchat
        // (src/common/outbound.c:split_up_text)
        let mut max: i32 = 512; // RFC 2812
        max -= 3;               // :, !, @
        max -= 13;              // " PRIVMSG ", " ", :, \r, \n
        max -= self.get_nick().len() as i32;
        max -= target.len() as i32;
        match self.usermask {
            None => {
                max -= 9;  // max username
                max -= 64; // max possible hostname (63) + '@'
                           // NOTE(osa): I think hexchat has an error here, it
                           // uses 65
            },
            Some(ref usermask) => {
                max -= usermask.len() as i32;
            },
        }

        assert!(max > 0);

        utils::split_iterator(msg, max as usize)
    }

    // FIXME: This crashes with an assertion error when the message is too long
    // to fit into 512 bytes. Need to make sure `split_privmsg` is called before
    // this.
    pub fn privmsg(&mut self, target: &str, msg: &str) {
        wire::privmsg(&mut self.out_buf, target, msg).unwrap();
        self.reregister_for_rw();
    }

    pub fn join(&mut self, chan: &str) {
        wire::join(&mut self.out_buf, chan).unwrap();
        // the channel will be added to auto-join list on successful join (i.e.
        // after RPL_TOPIC)
        self.reregister_for_rw();
    }

    pub fn part(&mut self, chan: &str) {
        wire::part(&mut self.out_buf, chan).unwrap();
        self.reregister_for_rw();
        self.auto_join.remove(chan);
    }

    pub fn away(&mut self, msg: Option<&str>) {
        self.away_status = msg.map(|s| s.to_string());
        wire::away(&mut self.out_buf, msg).unwrap();
        self.reregister_for_rw();
    }

    ////////////////////////////////////////////////////////////////////////////
    // Sending messages

    pub fn send(&mut self, evs: &mut Vec<ConnEv>) {
        match self.stream.write(&self.out_buf) {
            Err(err) => {
                evs.push(ConnEv::Err(err));
            }
            Ok(bytes_sent) => {
                self.out_buf.drain(0 .. bytes_sent);
                if self.out_buf.is_empty() {
                    self.reregister_for_r();
                }
            }
        }
    }

    ////////////////////////////////////////////////////////////////////////////
    // Receiving messages

    pub fn recv(&mut self, evs: &mut Vec<ConnEv>, logger: &mut Logger) {
        let mut read_buf: [u8; 512] = [0; 512];

        // Handle disconnects
        match self.stream.read(&mut read_buf) {
            Err(err) => {
                evs.push(ConnEv::Err(err));
            }
            Ok(bytes_read) => {
                self.reset_ticks();
                self.in_buf.extend(&read_buf[ 0 .. bytes_read ]);
                self.handle_msgs(evs, logger);
                if bytes_read == 0 {
                    evs.push(ConnEv::Disconnected);
                    self.enter_disconnect_state();
                }
            }
        }
    }

    fn handle_msgs(&mut self, evs: &mut Vec<ConnEv>, logger: &mut Logger) {
        while let Some(msg) = Msg::read(&mut self.in_buf,
                                        Some(logger.get_raw_serv_logs(&self.serv_addr)))
        {
            self.handle_msg(msg, evs, logger);
        }
    }

    fn handle_msg(&mut self, msg: Msg, evs: &mut Vec<ConnEv>, logger: &mut Logger) {
        if let Msg { cmd: Cmd::PING { ref server }, .. } = msg {
            wire::pong(&mut self.out_buf, server).unwrap();
            self.reregister_for_rw();
        }

        if let ConnStatus::Introduce = self.status {
            self.introduce();
            self.status = ConnStatus::PingPong { ticks_passed: 0 };
            evs.push(ConnEv::NickChange(self.get_nick().to_owned()));
        }

        if let Msg { cmd: Cmd::JOIN { .. }, pfx: Some(Pfx::User { ref nick, ref user }) } = msg {
            if nick == self.get_nick() {
                let usermask = format!("{}!{}", nick, user);
                logger.get_debug_logs().write_line(
                    format_args!("usermask set: {}", usermask));
                self.usermask = Some(usermask);
            }
        }

        if let Msg { cmd: Cmd::Reply { num: 396, ref params }, .. } = msg {
            // :hobana.freenode.net 396 osa1 haskell/developer/osa1
            // :is now your hidden host (set by services.)
            if params.len() == 3 {
                let usermask = format!("{}!~{}@{}", self.get_nick(), self.hostname, params[1]);
                logger.get_debug_logs().write_line(format_args!("usermask set: {}", usermask));
                self.usermask = Some(usermask);
            }
        }

        if let Msg { cmd: Cmd::Reply { num: 302, ref params }, .. } = msg {
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
                    logger.get_debug_logs().write_line(
                        format_args!("can't parse RPL_USERHOST: {}", params[1]));
                }
                Some(mut i) => {
                    if param.as_bytes().get(i + 1) == Some(&b'+')
                            || param.as_bytes().get(i + 1) == Some(&b'-') {
                        i += 1;
                    }
                    let usermask = (&param[i ..]).trim();
                    self.usermask = Some(usermask.to_owned());
                    logger.get_debug_logs().write_line(format_args!("usermask set: {}", usermask));
                }
            }
        }

        if let Msg { cmd: Cmd::Reply { num: 001, .. }, .. } = msg {
            // 001 RPL_WELCOME is how we understand that the registration was successful
            evs.push(ConnEv::Connected);
        }

        if let Msg { cmd: Cmd::Reply { num: 002, ref params }, .. } = msg {
            // 002    RPL_YOURHOST
            //        "Your host is <servername>, running version <ver>"

            // An example <servername>: cherryh.freenode.net[149.56.134.238/8001]

            match parse_servername(params) {
                None => {
                    logger.get_debug_logs().write_line(
                        format_args!("{} Can't parse hostname from params: {:?}",
                                     self.serv_addr, params));
                }
                Some(servername) => {
                    logger.get_debug_logs().write_line(
                        format_args!("{} host: {}", self.serv_addr, servername));
                    self.servername = Some(servername);
                }
            }
        }

        if let Msg { cmd: Cmd::Reply { num: 433, .. }, .. } = msg {
            // ERR_NICKNAMEINUSE
            self.next_nick();
            self.send_nick();
            evs.push(ConnEv::NickChange(self.get_nick().to_owned()));
        }

        if let Msg { cmd: Cmd::Reply { num: 376, .. }, .. } = msg {
            // RPL_ENDOFMOTD. Join auto-join channels.
            for chan in &self.auto_join {
                wire::join(&mut self.out_buf, chan).unwrap();
                reregister_for_rw(self.poll, self.get_raw_fd());
            }

            // Set away mode
            if let &Some(ref reason) = &self.away_status {
                wire::away(&mut self.out_buf, Some(reason)).unwrap();
                self.reregister_for_rw();
            }
        }

        if let Msg { cmd: Cmd::Reply { num: 332, ref params }, .. } = msg {
            if params.len() == 2 || params.len() == 3 {
                // RPL_TOPIC. We've successfully joined a channel, add the channel to
                // self.auto_join to be able to auto-join next time we connect
                let chan = &params[params.len() - 2];
                self.auto_join.insert(chan.to_owned());
            }
        }

        evs.push(ConnEv::Msg(msg));
    }
}

/// Try to parse servername in a 002 RPL_YOURHOST reply
fn parse_servername(params: &[String]) -> Option<String> {
    let msg = try_opt!(params.get(1).or_else(|| params.get(0)));
    let slice1 = &msg[13..];
    let servername_ends =
        try_opt!(wire::find_byte(slice1.as_bytes(), b'[')
                 .or_else(|| wire::find_byte(slice1.as_bytes(), b',')));
    Some((&slice1[..servername_ends]).to_owned())
}

////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_servername_1() {
        let args = vec!["tiny_test".to_owned(),
                        "Your host is adams.freenode.net[94.125.182.252/8001], \
                         running version ircd-seven-1.1.4".to_owned()];
        assert_eq!(parse_servername(&args), Some("adams.freenode.net".to_owned()));
    }

    #[test]
    fn test_parse_servername_2() {
        let args =
            vec!["tiny_test".to_owned(),
                 "Your host is belew.mozilla.org, running version InspIRCd-2.0".to_owned()];
        assert_eq!(parse_servername(&args), Some("belew.mozilla.org".to_owned()));
    }
}
