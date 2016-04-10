use std::borrow::Borrow;
use std::fmt::Arguments;
use std::fs::File;
use std::fs;
use std::io::Read;
use std::io::Write;
use std::io;
use std::net::TcpStream;
use std::os::unix::io::{AsRawFd, RawFd};
use std::str;
use std::time::Duration;

use msg::{Pfx, Cmd, Msg};
use utils::find_byte;

pub struct Comms {
    /// The TCP connection to the server.
    stream        : TcpStream,

    pub serv_name : String,

    /// _Partial_ messages collected here until they make a complete message.
    msg_buf       : Vec<u8>,

    /// A file to log incoming messages for debugging purposes. Only available
    /// when `debug_assertions` is available.
    log_file      : Option<File>,
}

#[derive(Debug)]
pub enum CommsRet {
    Disconnected {
        fd        : RawFd,
    },

    Err {
        err_msg   : String,
    },

    IncomingMsg {
        pfx       : Pfx,
        cmd       : Cmd,
        args      : Vec<String>,
    },

    /// A message without prefix. From RFC 2812:
    /// > If the prefix is missing from the message, it is assumed to have
    /// > originated from the connection from which it was received from.
    SentMsg {
        cmd       : Cmd,
        args      : Vec<String>,
    }
}

impl Comms {
    pub fn try_connect(serv_addr : &str, serv_name : &str,
                       nick : &str, hostname : &str, realname : &str)
                       -> io::Result<Comms> {
        let stream = try!(TcpStream::connect(serv_addr));
        try!(stream.set_read_timeout(Some(Duration::from_millis(10))));
        try!(stream.set_write_timeout(None));
        // try!(stream.set_nonblocking(true));

        let log_file = {
            if cfg!(debug_assertions) {
                let _ = fs::create_dir("logs");
                Some(File::create(format!("logs/{}.txt", serv_addr)).unwrap())
            } else {
                None
            }
        };

        let mut comms = Comms {
            stream:     stream,
            serv_name:  serv_name.to_owned(),
            msg_buf:    Vec::new(),
            log_file:   log_file,
        };

        try!(comms.introduce(nick, hostname, realname));
        Ok(comms)
    }

    /// Get the RawFd, to be used with select() or other I/O multiplexer.
    pub fn get_raw_fd(&self) -> RawFd {
        self.stream.as_raw_fd()
    }

    ////////////////////////////////////////////////////////////////////////////
    // Sending messages

    fn introduce(&mut self, nick : &str, hostname : &str, realname : &str) -> io::Result<()> {
        try!(Msg::user(hostname, realname, &mut self.stream));
        Msg::nick(nick, &mut self.stream)
    }

    ////////////////////////////////////////////////////////////////////////////
    // Receiving messages

    pub fn read_incoming_msg(&mut self) -> Vec<CommsRet> {
        let mut read_buf : [u8; 512] = [0; 512];

        // Handle disconnects
        match self.stream.read(&mut read_buf) {
            Err(_) => {
                // TODO: I don't understand why this happens. I'm ``randomly''
                // getting "temporarily unavailable" errors.
                // return vec![CommsRet::ShowErr(format!("error in read(): {:?}", err))];
                return vec![];
            },
            Ok(bytes_read) => {
                if bytes_read == 0 {
                    vec![CommsRet::Disconnected { fd: self.get_raw_fd() }]
                } else {
                    self.add_to_msg_buf(&read_buf[ 0 .. bytes_read ]);
                    self.handle_msgs()
                }
            }
        }
    }

    #[inline]
    fn add_to_msg_buf(&mut self, slice : &[u8]) {
        // Some invisible ASCII characters causing glitches on some terminals,
        // we filter those out here.
        self.msg_buf.extend(slice.iter().filter(|c| **c != 0x1 /* SOH */ ||
                                                    **c != 0x2 /* STX */ ||
                                                    **c != 0x0 /* NUL */ ||
                                                    **c != 0x4 /* EOT */));
    }

    fn handle_msgs(&mut self) -> Vec<CommsRet> {
        let mut ret = Vec::with_capacity(1);

        // Have we read any CRLFs? In that case just process the message and
        // update buffers. Otherwise just leave the partial message in the
        // buffer.
        loop {
            match find_byte(self.msg_buf.borrow(), b'\r') {
                None => { break; },
                Some(cr_idx) => {
                    // We have a CR, however, we don't have any guarantees that
                    // a single read() will read both CR and LF. So if we have a
                    // CR, but that's the last byte, we should just wait until
                    // we read NL too.
                    if cr_idx == self.msg_buf.len() - 1 {
                        break;
                    } else {
                        assert!(self.msg_buf[cr_idx + 1] == b'\n');

                        let msg = {
                            // Don't include CRLF
                            let msg_slice = &self.msg_buf[ 0 .. cr_idx ];
                            // Log the message in debug mode
                            if cfg!(debug_assertions) {
                                writeln!(self.log_file.as_ref().unwrap(), "{}",
                                         unsafe { str::from_utf8_unchecked(msg_slice) }).unwrap();
                            }
                            Msg::parse(msg_slice)
                        };

                        Comms::handle_msg(&mut self.stream, msg, &mut ret);
                        // Update the buffer (drop CRLF too)
                        self.msg_buf.drain(0 .. cr_idx + 2);
                    }
                }
            }
        }

        ret
    }

    fn handle_msg(stream : &mut TcpStream, msg : Result<Msg, String>, ret : &mut Vec<CommsRet>) {
        match msg {
            Err(err_msg) => {
                ret.push(CommsRet::Err { err_msg: err_msg });
            },
            Ok(Msg { pfx, cmd, params }) => {
                Comms::handle_cmd(stream, ret, pfx, cmd, params);
            }
        }
    }

    fn handle_cmd(stream : &mut TcpStream, ret : &mut Vec<CommsRet>,
                  pfx : Option<Pfx>, cmd : Cmd, params : Vec<Vec<u8>>) {
        match &cmd {
            &Cmd::Str(ref str) if str == "PING" => {
                debug_assert!(params.len() == 1);
                Msg::pong(unsafe {
                            str::from_utf8_unchecked(params.into_iter().nth(0).unwrap().as_ref())
                          }, stream).unwrap();
                return;
            }
            _ => {}
        }

        match pfx {
            None => {
                ret.push(CommsRet::SentMsg {
                    cmd: cmd,
                    args: params.into_iter().map(|s| unsafe {
                        String::from_utf8_unchecked(s)
                    }).collect(),
                });
            },
            Some(pfx) => {
                ret.push(CommsRet::IncomingMsg {
                    pfx: pfx,
                    cmd: cmd,
                    args: params.into_iter().map(|s| unsafe {
                        String::from_utf8_unchecked(s)
                    }).collect(),
                });
            }
        }
    }
}

impl Write for Comms {
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
    fn by_ref(&mut self) -> &mut Comms {
        self
    }
}
