//! IRC wire protocol message parsers and generators. Incomplete; new messages are added as needed.
//!
//! Parsing and message generation are done directly on netbuf buffers to avoid redundant
//! allocations.

use netbuf::Buf;
use std::str;

/// <prefix> ::= <servername> | <nick> [ '!' <user> ] [ '@' <host> ]
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Pfx {
    Server(String),

    User {
        nick: String,
        /// user@host
        user: String,
    },
}

/// A PRIVMSG receiver.
pub enum Receiver {
    Chan(String),
    User(String)
}

pub enum Msg {
    PRIVMSG {
        // TODO: In theory this should be a list of receivers, but in prative I've never
        // encountered that case.
        receivers: Receiver,
        contents: String
    },

    JOIN {
        // TODO: Same as above, this should be a list ...
        chan: String
        // TODO: key field might be useful when joining restricted channels. In practice I've never
        // needed it.
    },

    PART {
        // TODO: List of channels
        chan: String
    },

    QUIT(String),

    NOTICE {
        nick: String,
        msg: String,
    },

    NICK(String),

    /// An IRC message other than the ones listed above.
    Other {
        cmd: String,
        params: Vec<String>
    },

    /// Numeric replies are kept generic as there are just too many replies and we probably only
    /// need to handle a small subset of them.
    Reply {
        num: u16,
        params: Vec<String>,
    }
}

/// An intermediate type used during parsing.
enum MsgType<'a> {
    Cmd(&'a str),
    Num(u16),
}

static CRLF: [u8; 2] = [b'\r', b'\n'];

impl Msg {
    /// Try to read an IRC message off a `netbuf` buffer. Drops the message when parsing is
    /// successful. Otherwise the buffer is left unchanged.
    pub fn read(buf: &mut Buf) -> Option<Msg> {
        // find "\r\n" separator. `IntoSearcher` implementation for slice needs `str` (why??) so
        // using this hacky method instead.
        let crlf_idx = {
            match buf.as_ref().windows(2).position(|sub| sub == CRLF) {
                None => return None,
                Some(i) => i,
            }
        };

        let ret = {
            let mut slice: &[u8] = &buf.as_ref()[ 0 .. crlf_idx ];

            let pfx: Option<Pfx> = {
                if slice[0] == b':' {
                    // parse prefix
                    let ws_idx = find_byte(slice, b' ').unwrap();
                    let (mut pfx, slice_) = slice.split_at(ws_idx);
                    // drop the : from pfx
                    pfx = &pfx[ 1 .. ];
                    slice = &slice_[ 1 .. ]; // drop the space
                    Some(parse_pfx(&pfx))
                } else {
                    None
                }
            };

            let msg_ty: MsgType = {
                let ws_idx = find_byte(slice, b' ').unwrap();
                let (cmd, slice_) = slice.split_at(ws_idx);
                slice = &slice_[ 1 .. ]; // drop the space
                match parse_reply_num(cmd) {
                    None => MsgType::Cmd(unsafe {
                        // Cmd strings are added by the server and they're always ASCII strings, so
                        // this is safe and O(1).
                        str::from_utf8_unchecked(cmd)
                    }),
                    Some(num) => MsgType::Num(num)
                }
            };

            let params = parse_params(slice);

            assert!(slice.is_empty());

            match msg_ty {
                MsgType::Cmd("PRIVMSG") if params.len() == 2 => {
                    // TODO
                },
                _ => {}
            }

            unimplemented!()
        };

        buf.consume(crlf_idx + 2);
        ret
    }
}

fn parse_pfx(pfx: &[u8]) -> Pfx {
    match find_byte(pfx, b'!') {
        None => Pfx::Server(unsafe { str::from_utf8_unchecked(pfx).to_owned() }),
        Some(idx) => {
            Pfx::User {
                nick: unsafe { str::from_utf8_unchecked(&pfx[ 0 .. idx ]) }.to_owned(),
                user: unsafe { str::from_utf8_unchecked(&pfx[ idx + 1 .. ]) }.to_owned()
            }
        }
    }
}

fn parse_reply_num(bs: &[u8]) -> Option<u16> {

    fn is_num_ascii(b : u8) -> bool {
        b >= b'0' && b <= b'9'
    }

    if bs.len() == 3 {
        let n3 = unsafe { *bs.get_unchecked(0) };
        let n2 = unsafe { *bs.get_unchecked(1) };
        let n1 = unsafe { *bs.get_unchecked(2) };
        if is_num_ascii(n3) && is_num_ascii(n2) && is_num_ascii(n1) {
            return Some(((n3 - b'0') as u16) * 100 +
                        ((n2 - b'0') as u16) * 10  +
                        ((n1 - b'0') as u16));
        }
    }
    None
}

fn parse_params(chrs : &[u8]) -> Vec<Vec<u8>> {
    let mut ret : Vec<Vec<u8>> = Vec::new();

    let mut current_param = Vec::new();
    for byte_idx in 0 .. chrs.len() {
        let byte = *unsafe { chrs.get_unchecked(byte_idx) };
        if byte == b':' {
            current_param.extend_from_slice(&chrs[ byte_idx + 1 .. ]);
            ret.push(current_param);
            return ret;
        } else if byte == b' ' {
            ret.push(current_param);
            current_param = Vec::new();
        } else {
            current_param.push(byte);
        }
    }

    if current_param.len() > 0 {
        ret.push(current_param);
    }

    ret
}

pub fn find_byte(buf: &[u8], byte0: u8) -> Option<usize> {
    for (byte_idx, byte) in buf.iter().enumerate() {
        if *byte == byte0 {
            return Some(byte_idx);
        }
    }
    None
}
