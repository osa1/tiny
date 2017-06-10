use net2::TcpBuilder;
use net2::TcpStreamExt;
use std::collections::HashSet;
use std::fmt::Arguments;
use std::io::Read;
use std::io::Write;
use std::io;
use std::net::TcpStream;
use std::os::unix::io::{AsRawFd, RawFd};
use std::str;

use config;
use logger::Logger;
use logger::LogFile;
use wire::{Cmd, Msg};
use wire;

pub struct Conn {
    serv_addr: String,
    serv_port: u16,
    hostname: String,
    realname: String,
    nicks: Vec<String>,

    /// Always in range of `nicks`
    current_nick_idx: usize,

    /// Channels to auto-join. Initially empty, every channel we join will be added here to be able
    /// to re-join automatically on reconnect.
    auto_join: HashSet<String>,

    /// servername to be used in PING messages. Read from 002 RPL_YOURHOST. `None` until 002.
    host: Option<String>,

    /// The TCP connection to the server.
    stream: TcpStream,

    status: ConnStatus,

    /// _Partial_ messages collected here until they make a complete message.
    buf: Vec<u8>,
}

/// How many ticks to wait before sending a ping to the server.
const PING_TICKS: u8 = 60;
/// How many ticks to wait after sending a ping to the server to consider a disconnect.
const PONG_TICKS: u8 = 60;
/// How many ticks to wait after a disconnect or a socket error.
pub const RECONNECT_TICKS: u8 = 30;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum ConnStatus {
    /// Need to introduce self
    Introduce,
    PingPong {
        /// Ticks passed since last time we've heard from the server.
        /// Reset on each message. After `PING_TICKS` ticks we send a PING message and move to
        /// `WaitPong` state.
        ticks_passed: u8,
    },
    WaitPong {
        /// Ticks passed since we sent a PING to the server.
        /// After a message move to `PingPong` state. On timeout we reset the connection.
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
}

fn init_stream(serv_addr: &str, serv_port: u16) -> TcpStream {
    let stream = TcpBuilder::new_v4().unwrap().to_tcp_stream().unwrap();
    stream.set_nonblocking(true).unwrap();
    // This will fail with EINPROGRESS
    let _ = stream.connect((serv_addr, serv_port));
    stream
}

impl Conn {
    pub fn from_server(server: config::Server) -> Conn {
        let stream = init_stream(&server.server_addr, server.server_port);
        Conn {
            serv_addr: server.server_addr,
            serv_port: server.server_port,
            hostname: server.hostname,
            realname: server.real_name,
            nicks: server.nicks,
            current_nick_idx: 0,
            auto_join: HashSet::new(),
            host: None,
            stream: stream,
            status: ConnStatus::Introduce,
            buf: vec![],
        }
    }

    /// Clone an existing connection, but update the server address.
    pub fn from_conn(conn: &Conn, new_serv_addr: &str, new_serv_port: u16) -> Conn {
        let stream = init_stream(new_serv_addr, new_serv_port);
        Conn {
            serv_addr: new_serv_addr.to_owned(),
            serv_port: new_serv_port,
            hostname: conn.hostname.clone(),
            realname: conn.realname.clone(),
            nicks: conn.nicks.clone(),
            current_nick_idx: 0,
            auto_join: HashSet::new(),
            host: None,
            stream: stream,
            status: ConnStatus::Introduce,
            buf: vec![],
        }
    }

    pub fn reconnect(&mut self, new_serv: Option<(&str, u16)>) {
        if let Some((new_name, new_port)) = new_serv {
            self.serv_addr = new_name.to_owned();
            self.serv_port = new_port;
        }
        self.stream = init_stream(&self.serv_addr, self.serv_port);
        self.status = ConnStatus::Introduce;
        self.current_nick_idx = 0;
    }

    /// Get the RawFd, to be used with select() or other I/O multiplexer.
    pub fn get_raw_fd(&self) -> RawFd {
        self.stream.as_raw_fd()
    }

    pub fn get_serv_name(&self) -> &str {
        &self.serv_addr
    }

    pub fn get_nick(&self) -> &str {
        &self.nicks[self.current_nick_idx]
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

impl Conn {

    ////////////////////////////////////////////////////////////////////////////
    // Tick handling

    pub fn tick(&mut self, evs: &mut Vec<ConnEv>, mut debug_out: LogFile) {
        match self.status {
            ConnStatus::Introduce => {},
            ConnStatus::PingPong { ticks_passed } => {
                if ticks_passed + 1 == PING_TICKS {
                    match self.host {
                        None => {
                            debug_out.write_line(
                                format_args!("{}: Can't send PING, host unknown",
                                             self.serv_addr));
                        }
                        Some(ref host_) => {
                            debug_out.write_line(
                                format_args!("{}: Ping timeout, sending PING",
                                             self.serv_addr));
                            wire::ping(&mut self.stream, host_).unwrap();;
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
                    self.status = ConnStatus::Disconnected { ticks_passed: 0 };
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
                    self.status = ConnStatus::Introduce;
                } else {
                    self.status = ConnStatus::Disconnected { ticks_passed: ticks_passed + 1 };
                }
            }
        }
    }
    pub fn enter_disconnect_state(&mut self) {
        self.status = ConnStatus::Disconnected { ticks_passed: 0 };
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
        wire::user(&self.stream, &self.hostname, &self.realname).unwrap();
        self.send_nick();
    }

    fn send_nick(&mut self) {
        wire::nick(&self.stream, self.get_nick()).unwrap();
    }

    pub fn privmsg(&self, target: &str, msg: &str) {
        wire::privmsg(&self.stream, target, msg).unwrap();
    }

    pub fn join(&self, chan: &str) {
        wire::join(&self.stream, chan).unwrap();
    }

    ////////////////////////////////////////////////////////////////////////////
    // Receiving messages

    pub fn read_incoming_msg(&mut self, evs: &mut Vec<ConnEv>, logger: &mut Logger) {
        let mut read_buf: [u8; 512] = [0; 512];

        // Handle disconnects
        match self.stream.read(&mut read_buf) {
            Err(err) => {
                evs.push(ConnEv::Err(err));
            }
            Ok(bytes_read) => {
                self.reset_ticks();
                self.add_to_msg_buf(&read_buf[ 0 .. bytes_read ]);
                self.handle_msgs(evs, logger);
                if bytes_read == 0 {
                    evs.push(ConnEv::Disconnected);
                }
            }
        }
    }

    fn add_to_msg_buf(&mut self, slice: &[u8]) {
        // Some invisible ASCII characters causing glitches on some terminals,
        // we filter those out here.
        self.buf.extend(slice.iter().filter(|c| **c != 0x1 /* SOH */ ||
                                                **c != 0x2 /* STX */ ||
                                                **c != 0x0 /* NUL */ ||
                                                **c != 0x4 /* EOT */ ));
    }

    fn handle_msgs(&mut self, evs: &mut Vec<ConnEv>, logger: &mut Logger) {
        while let Some(msg) = Msg::read(&mut self.buf, Some(logger.get_raw_serv_logs(&self.serv_addr))) {
            self.handle_msg(msg, evs, logger);
        }
    }

    fn handle_msg(&mut self, msg: Msg, evs: &mut Vec<ConnEv>, logger: &mut Logger) {
        if let &Msg { cmd: Cmd::PING { ref server }, .. } = &msg {
            wire::pong(&self.stream, server).unwrap();
        }

        if let ConnStatus::Introduce = self.status {
            self.introduce();
            self.status = ConnStatus::PingPong { ticks_passed: 0 };
        }

        if let &Msg { cmd: Cmd::Reply { num: 001, .. }, .. } = &msg {
            // 001 RPL_WELCOME is how we understand that the registration was successful
            evs.push(ConnEv::Connected);
        }

        if let &Msg { cmd: Cmd::Reply { num: 002, ref params }, .. } = &msg {
            // 002    RPL_YOURHOST
            //        "Your host is <servername>, running version <ver>"

            // An example <servername>: cherryh.freenode.net[149.56.134.238/8001]

            match parse_servername(params) {
                None => {
                    logger.get_debug_logs().write_line(
                        format_args!("{} Can't parse hostname from params: {:?}",
                                     self.serv_addr, params));
                }
                Some(host) => {
                    logger.get_debug_logs().write_line(
                        format_args!("{} host: {}", self.serv_addr, host));
                    self.host = Some(host);
                }
            }
        }

        if let &Msg { cmd: Cmd::Reply { num: 433, .. }, .. } = &msg {
            // ERR_NICKNAMEINUSE
            self.next_nick();
            self.send_nick();
        }

        if let &Msg { cmd: Cmd::Reply { num: 376, .. }, .. } = &msg {
            // RPL_ENDOFMOTD. Join auto-join channels.
            for chan in &self.auto_join {
                self.join(chan);
            }
        }

        if let &Msg { cmd: Cmd::Reply { num: 332, ref params }, .. } = &msg {
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

macro_rules! try_opt {
    ($expr:expr) => (match $expr {
        Option::Some(val) => val,
        Option::None => {
            return Option::None
        }
    })
}

/// Try to parse servername in a 002 RPL_YOURHOST reply
fn parse_servername(params: &[String]) -> Option<String> {
    let msg = try_opt!(params.get(1).or(params.get(0)));
    let slice1 = &msg[13..];
    let servername_ends =
        try_opt!(wire::find_byte(slice1.as_bytes(), b'[')
                 .or(wire::find_byte(slice1.as_bytes(), b',')));
    Some((&slice1[..servername_ends]).to_owned())
}

impl Write for Conn {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stream.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }

    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.stream.write_all(buf)
    }

    fn write_fmt(&mut self, fmt: Arguments) -> io::Result<()> {
        self.stream.write_fmt(fmt)
    }

    fn by_ref(&mut self) -> &mut Conn {
        self
    }
}

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
