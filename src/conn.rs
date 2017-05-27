use net2::TcpBuilder;
use net2::TcpStreamExt;
use std::fmt::Arguments;
use std::fs::File;
use std::fs;
use std::io::Read;
use std::io::Write;
use std::io;
use std::mem;
use std::net::TcpStream;
use std::os::unix::io::{AsRawFd, RawFd};
use std::str;

use wire::{Cmd, Msg};
use wire;

pub struct Conn {
    nick: String,
    hostname: String,
    realname: String,

    /// servername to be used in PING messages. Read from 002 RPL_YOURHOST. `None` until 002.
    host: Option<String>,

    /// The TCP connection to the server.
    stream: TcpStream,

    status: ConnStatus,

    serv_name: String,

    /// _Partial_ messages collected here until they make a complete message.
    buf: Vec<u8>,

    /// A file to log incoming messages for debugging purposes. Only available
    /// when `debug_assertions` is available.
    log_file: Option<File>,
}

/// How many ticks to wait before sending a ping to the server.
const PING_TICKS: u8 = 30;
/// How many ticks to wait after sending a ping to the server to consider a disconnect.
const PONG_TICKS: u8 = 10;

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
}

#[derive(Debug)]
pub enum ConnEv {
    Disconnected,
    Err(String),
    Msg(Msg),
}

impl Conn {
    pub fn new(serv_addr: &str, serv_name: &str, nick: &str, hostname: &str, realname: &str) -> Conn {
        let stream = TcpBuilder::new_v4().unwrap().to_tcp_stream().unwrap();
        stream.set_nonblocking(true).unwrap();
        // This will fail with EINPROGRESS
        let _ = stream.connect(serv_addr);

        let log_file = {
            if cfg!(debug_assertions) {
                let _ = fs::create_dir("logs");
                Some(File::create(format!("logs/{}.txt", serv_addr)).unwrap())
            } else {
                None
            }
        };

        Conn {
            nick: nick.to_owned(),
            hostname: hostname.to_owned(),
            realname: realname.to_owned(),
            host: None,
            stream: stream,
            status: ConnStatus::Introduce,
            serv_name: serv_name.to_owned(),
            buf: vec![],
            log_file: log_file,
        }
    }

    /// Get the RawFd, to be used with select() or other I/O multiplexer.
    pub fn get_raw_fd(&self) -> RawFd {
        self.stream.as_raw_fd()
    }

    pub fn get_serv_name(&self) -> &str {
        &self.serv_name
    }

}

impl Conn {

    ////////////////////////////////////////////////////////////////////////////
    // Tick handling

/*
    pub fn tick(&mut self) {
        match self.status {
            ConnStatus::Introduce => {},
            ConnStatus::PingPong { ticks_passed } => {
                if ticks_passed + 1 == PING_TICKS {
                    wire::ping(&self.serv_name, &mut self.stream).unwrap();;
                    self.status = ConnStatus::WaitPong { ticks_passed: 0 };
                } else {
                    self.status = ConnStatus::PingPong { ticks_passed: ticks_passed + 1 };
                }
            }
            ConnStatus::WaitPong { ticks_passed } => {
                if ticks_passed + 1 == PONG_TICKS {
                    // TODO: propagate disconnect event
                    self.stream = init_stream(&self.serv_addr);
                    self.status = ConnStatus::Introduce;
                } else {
                    self.status = ConnStatus::WaitPong { ticks_passed: ticks_passed + 1 };
                }
            }
        }
    }
*/

    ////////////////////////////////////////////////////////////////////////////
    // Sending messages

    fn introduce(&mut self) -> io::Result<()> {
        try!(wire::user(&self.hostname, &self.realname, &mut self.stream));
        wire::nick(&self.nick, &mut self.stream)
    }

    ////////////////////////////////////////////////////////////////////////////
    // Receiving messages

    pub fn read_incoming_msg(&mut self, evs: &mut Vec<ConnEv>) {
        let mut read_buf: [u8; 512] = [0; 512];

        // Handle disconnects
        match self.stream.read(&mut read_buf) {
            Err(err) => {
                // TODO: I don't understand why this happens. I'm ``randomly''
                // getting "temporarily unavailable" errors.
                evs.push(ConnEv::Err(format!("Error in read(): {:?}", err)));
            }
            Ok(bytes_read) => {
                writeln!(&mut io::stderr(), "read: {:?} bytes", bytes_read).unwrap();
                self.add_to_msg_buf(&read_buf[ 0 .. bytes_read ]);
                self.handle_msgs(evs);
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

    fn handle_msgs(&mut self, evs: &mut Vec<ConnEv>) {
        while let Some(msg) = Msg::read(&mut self.buf, &self.log_file) {
            self.handle_msg(msg, evs);
        }
    }

    fn handle_msg(&mut self, msg: Msg, evs: &mut Vec<ConnEv>) {
        if let &Msg { cmd: Cmd::PING { ref server }, .. } = &msg {
            wire::pong(server, &mut self.stream).unwrap();
        }

        if let ConnStatus::Introduce = self.status {
            if let Err(err) = self.introduce() {
                evs.push(ConnEv::Err(format!("Error: {:?}", err)));
            } else {
                self.status = ConnStatus::PingPong { ticks_passed: 0 };
            }
        }

        if let &Msg { cmd: Cmd::Reply { num: 002, ref params }, .. } = &msg {
            // 002    RPL_YOURHOST
            //        "Your host is <servername>, running version <ver>"

            // An example <servername>: cherryh.freenode.net[149.56.134.238/8001]

            match parse_servername(params) {
                None => {
                    evs.push(ConnEv::Err(format!("Can't parse hostname from params: {:?}", params)));
                }
                Some(host) => {
                    self.host = Some(host);
                }
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
    let msg = try_opt!(params.get(1));
    let slice1 = &msg[13..];
    let servername_ends = try_opt!(wire::find_byte(slice1.as_bytes(), b'['));
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
