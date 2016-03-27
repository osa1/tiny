use std::borrow::Borrow;
use std::io::Read;
use std::io::Write;
use std::io;
use std::mem;
use std::net::TcpStream;
use std::os::unix::io::{AsRawFd, RawFd};
use std::str;
use std::time::Duration;

use msg::{Msg, Command};
use utils::find_byte;

pub struct Comms {
    /// The TCP connection to the server.
    stream   : TcpStream,

    /// Prefix used for the messages originated from the server.
    /// Unknown prior to the first server message. (Does NOT include the ':')
    pfx      : Option<String>,

    /// _Partial_ messages collected here until they make a complete message.
    msg_buf  : Vec<u8>,
}

pub enum CommsRet {
    Disconnected,
    Err(String),

    IncomingMsg {
        pfx: String,
        ty: String,
        msg: String,
    },

    /// A message without prefix. From RFC 2812:
    /// > If the prefix is missing from the message, it is assumed to have
    /// > originated from the connection from which it was received from.
    SentMsg {
        ty: String,
        msg: String,
    }
}

impl Comms {
    pub fn try_connect(server : &str, nick : &str, hostname : &str, realname : &str)
                       -> io::Result<Comms> {
        let stream = try!(TcpStream::connect(server));
        try!(stream.set_read_timeout(Some(Duration::from_millis(10))));
        try!(stream.set_write_timeout(None));

        let mut comms = Comms {
            stream: stream,
            pfx: None,
            msg_buf: Vec::new(),
        };
        try!(comms.introduce(nick, hostname, realname));
        Ok(comms)
    }

    fn introduce(&mut self, nick : &str, hostname : &str, realname : &str) -> io::Result<()> {
        try!(Msg::user(hostname, realname, &mut self.stream));
        Msg::nick(nick, &mut self.stream)
    }

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
                    return vec![CommsRet::Disconnected];
                }
            }
        }

        self.add_to_msg_buf(&read_buf);
        self.handle_msgs()
    }

    #[inline]
    fn add_to_msg_buf(&mut self, slice : &[u8]) {
        // Some invisible ASCII characters causing glitches on some terminals,
        // we filter those out here.
        self.msg_buf.extend(slice.iter().filter(|c| **c != 0x1 /* SOH */ || **c != 0x2 /* STX */));
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
                        // Don't include CRLF
                        let msg = Msg::parse(&self.msg_buf[ 0 .. cr_idx ]);
                        // Update the server prefix if we don't know it already
                        for m in msg.iter() {
                            self.pfx = m.pfx.clone().map(|v| {
                                unsafe { String::from_utf8_unchecked(v) }
                            });
                        }
                        // Finally handle the message
                        Comms::handle_msg(&mut self.stream, msg, &mut ret);
                        // Update the buffer (drop CRLF)
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
                ret.push(CommsRet::Err(err_msg));
            },
            Ok(Msg { pfx, command, params }) => {
                match command {
                    Command::Str(str) => Comms::handle_str_command(stream, ret, pfx, str, params),
                    Command::Num(num) => Comms::handle_num_command(stream, ret, pfx, num, params),
                }
            }
        }
    }

    fn handle_str_command(stream : &mut TcpStream, ret : &mut Vec<CommsRet>,
                          prefix : Option<Vec<u8>>, cmd : String, mut params : Vec<Vec<u8>>) {
        if cmd == "PING" {
            debug_assert!(params.len() == 1);
            Msg::pong(unsafe {
                        str::from_utf8_unchecked(params.into_iter().nth(0).unwrap().as_ref())
                      }, stream).unwrap();
        } else {
            match prefix {
                None => {
                    ret.push(CommsRet::SentMsg {
                        ty: cmd,
                        msg: params.into_iter().map(|s| unsafe {
                            String::from_utf8_unchecked(s)
                        }).collect::<Vec<_>>().join(" "), // FIXME: intermediate vector
                    });
                },
                Some(pfx) => {
                    ret.push(CommsRet::IncomingMsg {
                        pfx: unsafe { String::from_utf8_unchecked(pfx) },
                        ty: cmd,
                        msg: params.into_iter().map(|s| unsafe {
                            String::from_utf8_unchecked(s)
                        }).collect::<Vec<_>>().join(" "), // FIXME: intermediate vector
                    });
                }
            }
        }
    }

    fn handle_num_command(stream : &mut TcpStream, ret : &mut Vec<CommsRet>,
                          prefix : Option<Vec<u8>>, num : u16, params : Vec<Vec<u8>>) {
        // TODO
        Comms::handle_str_command(stream, ret, prefix, "UNKNOWN".to_owned(), params)
    }

    /// Get the RawFd, to be used with select() or other I/O multiplexer.
    pub fn get_raw_fd(&self) -> RawFd {
        self.stream.as_raw_fd()
    }

    pub fn send_raw(&mut self, bytes : &[u8]) -> io::Result<()> {
        self.stream.write_all(bytes)
    }
}
