use std::borrow::Borrow;
use std::io::Read;
use std::io::Write;
use std::io;
use std::mem;
use std::net::TcpStream;
use std::os::unix::io::{AsRawFd, RawFd};
use std::time::Duration;

use msg::{Msg, Command};
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

    /// Some kind of info message from the server (instead of another client)
    ShowServerMsg {
        ty: String,
        msg: String,
    },
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
            Err(_) => {
                // TODO: I don't understand why this happens. I'm ``randomly''
                // getting "temporarily unavailable" errors.
                // return vec![CommsRet::ShowErr(format!("error in read(): {:?}", err))];
                self.read_buf = unsafe { mem::zeroed() };
                return vec![];
            },
            Ok(bytes_read) => {
                if bytes_read == 0 {
                    self.read_buf = unsafe { mem::zeroed() };
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
                                Comms::add_to_msg_buf(&mut self.msg_buf, read_buf_);
                            },
                            Some(slice_end) => {
                                Comms::add_to_msg_buf(
                                    &mut self.msg_buf, &read_buf_[ 0 .. slice_end ]);
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
                            Comms::add_to_msg_buf(&mut self.msg_buf, read_buf_);
                            break;
                        } else {
                            Comms::add_to_msg_buf(&mut self.msg_buf, &read_buf_[ 0 .. cr_idx ]);
                            Comms::handle_msg(&mut self.stream, &mut ret, self.msg_buf.borrow());
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

    #[inline]
    fn add_to_msg_buf(msg_buf : &mut Vec<u8>, slice : &[u8]) {
        // Some invisible ASCII characters causing glitches on some terminals,
        // we filter those out here.
        msg_buf.extend(slice.iter().filter(|c| **c != 0x2 /* STX */));
    }

    // Can't make this a method -- we need TcpStream mut but in the call site
    // msg_buf is borrwed as mutable too.
    fn handle_msg(stream : &mut TcpStream, ret : &mut Vec<CommsRet>, msg_buf : &[u8]) {
        match Msg::parse(msg_buf) {
            Err(err_msg) => {
                ret.push(CommsRet::ShowErr(err_msg));
            },
            Ok(Msg { prefix, command, params }) => {
                match command {
                    Command::Str(str) => Comms::handle_str_command(stream, ret, prefix, str, params),
                    Command::Num(num) => Comms::handle_num_command(stream, ret, prefix, num, params),
                }
            }
        }
    }

    fn handle_str_command(stream : &mut TcpStream, ret : &mut Vec<CommsRet>,
                          prefix : Option<Vec<u8>>, cmd : String, params : Vec<Vec<u8>>) {
        // match cmd.as_str() {
        //     "NOTICE" | "MODE"  => {
        //         let text = params.into_iter().last().unwrap();
        //         ret.push(CommsRet::ShowServerMsg {
        //             ty: cmd,
        //             msg: String::from_utf8(text).unwrap(),
        //         });
        //     },

        //     _ => {},
        // }
        ret.push(CommsRet::ShowServerMsg {
            ty: cmd,
            msg: params.into_iter().map(|s| unsafe {
                String::from_utf8_unchecked(s)
            }).collect::<Vec<String>>().join(" "), // FIXME: intermediate vector
        });
    }

    fn handle_num_command(stream : &mut TcpStream, ret : &mut Vec<CommsRet>,
                          prefix : Option<Vec<u8>>, num : u16, params : Vec<Vec<u8>>) {
        match num {
            // 001 => {
            //     ret.push(CommsRet::ShowServerMsg {
            //         ty: "WELCOME".to_owned(),
            //         msg: unsafe { String::from_utf8_unchecked(params.into_iter().last().unwrap()) },
            //     });
            // },

            // // Info messages with just one parameter
            // 002 | 003 | 004 | 005 | 375 | 372 | 376
            //     // More than more params, but we want to show just the last
            //     // param
            //     | 265 | 266 => {
            //     ret.push(CommsRet::ShowServerMsg {
            //         ty: "INFO".to_owned(),
            //         msg: unsafe { String::from_utf8_unchecked(params.into_iter().last().unwrap()) },
            //     });
            // },

            // // Info messages with more than one parameters to show
            // 251 | 252 | 253 | 254 | 255 => {
            //     ret.push(CommsRet::ShowServerMsg {
            //         ty: "INFO".to_owned(),
            //         msg: params.into_iter().map(|s| unsafe {
            //             String::from_utf8_unchecked(s)
            //         }).collect::<Vec<String>>().join(" "), // FIXME: intermediate vector
            //     });
            // },

            // Just show the rest
            _ => {
                ret.push(CommsRet::ShowServerMsg {
                    ty: "UNKNOWN".to_owned(),
                    msg: params.into_iter().map(|s| unsafe {
                        String::from_utf8_unchecked(s)
                    }).collect::<Vec<String>>().join(" "), // FIXME: intermediate vector
                });
            }
        }
    }

    /// Get the RawFd, to be used with select() or other I/O multiplexer.
    pub fn get_raw_fd(&self) -> RawFd {
        self.stream.as_raw_fd()
    }

    pub fn send_raw(&mut self, bytes : &[u8]) -> io::Result<()> {
        self.stream.write_all(bytes)
    }
}
