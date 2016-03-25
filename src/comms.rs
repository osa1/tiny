use std::io::Read;
use std::io::Write;
use std::io;
use std::mem;
use std::net::TcpStream;
use std::os::unix::io::{AsRawFd, RawFd};
use std::time::Duration;

use utils::find_byte;

pub struct Comms {
    /// The TCP connection to the server.
    stream : TcpStream,

    /// Buffer used to read bytes from the socket.
    read_buf : [u8; 512],

    /// _Partial_ messages collected here until they make a complete message.
    msg_buf  : Vec<u8>,
}

pub enum CommsRet {
    Disconnected,
    ShowErr(String),
    ShowIncomingMsg(String),
}

impl Comms {
    pub fn new(stream : TcpStream) -> Comms {
        stream.set_read_timeout(Some(Duration::from_millis(1))).unwrap();
        stream.set_write_timeout(None).unwrap();
        stream.set_nodelay(true).unwrap();
        Comms {
            stream: stream,
            read_buf: [0; 512],
            msg_buf: Vec::new(),
        }
    }

    pub fn try_connect(hostname : &str) -> io::Result<Comms> {
        TcpStream::connect(hostname).map(Comms::new)
    }

    pub fn read_incoming_msg(&mut self) -> Vec<CommsRet> {
        // Handle disconnects
        match self.stream.read(&mut self.read_buf) {
            Err(err) => {
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

        let mut ret : Vec<CommsRet> = Vec::with_capacity(2);

        // Have we read any CRLFs? In that case just process the message and
        // update the buffers. Otherwise just push the partial message to the
        // buffer.
        {
            // (Creating a new scope for read_buf_)
            let mut read_buf_ : &[u8] = &self.read_buf;
            loop {
                match find_byte(read_buf_, b'\r') {
                    None => {
                        // Push the partial message to the message buffer, keep
                        // reading until a complete message is read.
                        match find_byte(read_buf_, 0) {
                            None => {
                                self.msg_buf.extend_from_slice(read_buf_);
                            },
                            Some(slice_end) => {
                                self.msg_buf.extend_from_slice(&read_buf_[ 0 .. slice_end ]);
                            }
                        }
                        break;
                    },
                    Some(cr_idx) => {
                        // We have a CR, however, we don't have any guarantees
                        // that a single read() will read both CR and LF. So if
                        // we have a CR, but that's the last byte, we should
                        // just push the whole thing to the msg_buf so that when
                        // we read NL in the next read() we get a whole mssage.
                        if cr_idx == read_buf_.len() - 1 {
                            self.msg_buf.extend_from_slice(read_buf_);
                            break;
                        } else {
                            self.msg_buf.extend_from_slice(&read_buf_[ 0 .. cr_idx ]);
                            match String::from_utf8(mem::replace(&mut self.msg_buf, Vec::new())) {
                                Err(err) => {
                                    ret.push(
                                        CommsRet::ShowErr(
                                            format!("Can't parse incoming message: {}", err)));
                                },
                                Ok(str) => {
                                    ret.push(CommsRet::ShowIncomingMsg(str));
                                }
                            }

                            self.msg_buf.clear();

                            // Next char is NL, drop that too.
                            read_buf_ = &read_buf_[ cr_idx + 2 .. ];
                        }
                    }
                }
            }
        }

        self.read_buf = unsafe { mem::zeroed() };

        ret
    }

    /// Get the RawFd, to be used with select() or other I/O multiplexer.
    pub fn get_raw_fd(&self) -> RawFd {
        self.stream.as_raw_fd()
    }

    pub fn send_raw(&mut self, bytes : &[u8]) -> io::Result<()> {
        self.stream.write_all(bytes)
    }
}
