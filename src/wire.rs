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
#[derive(Debug, PartialEq, Eq)]
pub enum Receiver {
    Chan(String),
    User(String)
}

#[derive(Debug, PartialEq, Eq)]
pub enum Msg {
    PRIVMSG {
        pfx: Option<Pfx>,
        // TODO: In theory this should be a list of receivers, but in practice I've never
        // encountered that case.
        receivers: Receiver,
        contents: String
    },

    JOIN {
        pfx: Option<Pfx>,
        // TODO: Same as above, this should be a list ...
        chan: String
        // TODO: key field might be useful when joining restricted channels. In practice I've never
        // needed it.
    },

    PART {
        pfx: Option<Pfx>,
        // TODO: List of channels
        chan: String
    },

    QUIT {
        pfx: Option<Pfx>,
        msg: String,
    },

    NOTICE {
        nick: String,
        msg: String,
    },

    NICK(String),

    /// An IRC message other than the ones listed above.
    Other {
        pfx: Option<Pfx>,
        cmd: String,
        params: Vec<String>
    },

    /// Numeric replies are kept generic as there are just too many replies and we probably only
    /// need to handle a small subset of them.
    Reply {
        pfx: Option<Pfx>,
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

            let params: Vec<&str> = parse_params(unsafe { str::from_utf8_unchecked(slice) });
            match msg_ty {
                MsgType::Cmd("PRIVMSG") if params.len() == 2 => {
                    let target = params[0];
                    let msg = params[1];
                    let receiver =
                        if target.chars().nth(0) == Some('#') {
                            Receiver::Chan(target.to_owned())
                        } else {
                            Receiver::User(target.to_owned())
                        };
                    Msg::PRIVMSG {
                        pfx: pfx,
                        receivers: receiver,
                        contents: msg.to_owned(),
                    }
                },
                MsgType::Cmd("JOIN") if params.len() == 1 => {
                    let chan = params[0];
                    Msg::JOIN {
                        pfx: pfx,
                        chan: chan.to_owned(),
                    }
                },
                _ => {
                    unimplemented!()
                }
            }
        };

        buf.consume(crlf_idx + 2);
        Some(ret)
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

fn parse_params(chrs: &str) -> Vec<&str> {
    debug_assert!(chrs.chars().nth(0) != Some(' '));

    let mut ret: Vec<&str> = Vec::new();

    let mut slice_begins = 0;
    for (char_idx, char) in chrs.char_indices() {
        if char == ':' {
            ret.push(unsafe { chrs.slice_unchecked(char_idx + 1, chrs.len()) });
            return ret;
        } else if char == ' ' {
            ret.push(unsafe { chrs.slice_unchecked(slice_begins, char_idx) });
            slice_begins = char_idx + 1;
        }
    }

    if slice_begins != chrs.len() {
        ret.push(unsafe { chrs.slice_unchecked(slice_begins, chrs.len()) });
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_parse_params() {
        assert_eq!(parse_params("p1 p2 p3"), vec!["p1", "p2", "p3"]);
        let v: Vec<&str> = vec![];
        assert_eq!(parse_params(""), v);
        assert_eq!(parse_params(":foo bar baz "), vec!["foo bar baz "]);
        assert_eq!(parse_params(":"), vec![""]);
    }

    #[test]
    fn test_privmsg_parsing() {
        let mut buf = Buf::new();
        write!(&mut buf, ":nick!~nick@unaffiliated/nick PRIVMSG tiny :a b c\r\n").unwrap();
        assert_eq!(
            Msg::read(&mut buf),
            Some(Msg::PRIVMSG {
                pfx: Some(Pfx::User {
                    nick: "nick".to_owned(),
                    user: "~nick@unaffiliated/nick".to_owned(),
                }),
                receivers: Receiver::User("tiny".to_owned()),
                contents: "a b c".to_owned()
            }));
        assert_eq!(buf.len(), 0);
    }

    #[test]
    fn test_join_parsing() {
        let mut buf = Buf::new();
        write!(&mut buf, ":tiny!~tiny@192.168.0.1 JOIN #haskell\r\n").unwrap();
        assert_eq!(
            Msg::read(&mut buf),
            Some(Msg::JOIN {
                pfx: Some(Pfx::User {
                    nick: "tiny".to_owned(),
                    user: "~tiny@192.168.0.1".to_owned(),
                }),
                chan: "#haskell".to_owned(),
            }));
        assert_eq!(buf.len(), 0);
    }
}
