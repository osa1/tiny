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

enum ConnStatus {
    /// Need to introduce self
    Introduce,
    PingPong,
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
                                                **c != 0x4 /* EOT */));
    }

    fn handle_msgs(&mut self, evs: &mut Vec<ConnEv>) {
        while let Some(msg) = Msg::read(&mut self.buf, &self.log_file) {
            self.handle_msg(msg, evs);
        }
    }

    fn handle_msg(&mut self, msg: Msg, evs: &mut Vec<ConnEv>) {
        match &msg {
            &Msg { cmd: Cmd::PING { ref server }, .. } => {
                wire::pong(server, &mut self.stream).unwrap()
            }
            _ => {}
        }

        let status = mem::replace(&mut self.status, ConnStatus::PingPong);
        if let ConnStatus::Introduce = status {
            if let Err(err) = self.introduce() {
                evs.push(ConnEv::Err(format!("Error: {:?}", err)));
            }
        }

        evs.push(ConnEv::Msg(msg));
    }
}

impl Write for Conn {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stream.write(buf)
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }

    #[inline]
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.stream.write_all(buf)
    }

    #[inline]
    fn write_fmt(&mut self, fmt: Arguments) -> io::Result<()> {
        self.stream.write_fmt(fmt)
    }

    #[inline]
    fn by_ref(&mut self) -> &mut Conn {
        self
    }
}
