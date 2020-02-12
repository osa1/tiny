#![allow(clippy::write_with_newline)]

//! IRC wire protocol message parsers and generators. Incomplete; new messages are added as needed.

use std::str;

pub fn pass(pass: &str) -> String {
    format!("PASS {}\r\n", pass)
}

// FIXME: Option<String> because going from Option<String> to Option<&str> is too painful...
pub fn quit(reason: Option<String>) -> String {
    match reason {
        None => "QUIT\r\n".to_string(),
        Some(reason) => format!("QUIT :{}\r\n", reason),
    }
}

pub fn user(hostname: &str, realname: &str) -> String {
    format!("USER {} 8 * :{}\r\n", hostname, realname)
}

pub fn nick(arg: &str) -> String {
    format!("NICK {}\r\n", arg)
}

pub fn ping(arg: &str) -> String {
    format!("PING {}\r\n", arg)
}

pub fn pong(arg: &str) -> String {
    format!("PONG {}\r\n", arg)
}

pub fn join(chans: &[&str]) -> String {
    format!("JOIN {}\r\n", chans.join(","))
}

pub fn part(channel: &str) -> String {
    format!("PART {}\r\n", channel)
}

pub fn list(arg: &str) -> String {
    format!("LIST {}\r\n", arg)
}

pub fn privmsg(msgtarget: &str, msg: &str) -> String {
    // IRC messages need to be shorter than 512 bytes (see RFC 1459 or 2812). This should be dealt
    // with at call sites as we can't show how we split messages into multiple messages in the UI
    // at this point.
    assert!(msgtarget.len() + msg.len() + 12 <= 512);
    format!("PRIVMSG {} :{}\r\n", msgtarget, msg)
}

pub fn action(msgtarget: &str, msg: &str) -> String {
    assert!(msgtarget.len() + msg.len() + 21 <= 512); // See comments in `privmsg`
    format!("PRIVMSG {} :\x01ACTION {}\x01\r\n", msgtarget, msg)
}

pub fn away(msg: Option<&str>) -> String {
    match msg {
        None => "AWAY\r\n".to_string(),
        Some(msg) => format!("AWAY :{}\r\n", msg),
    }
}

pub fn cap_ls() -> String {
    "CAP LS\r\n".to_string()
}

pub fn cap_req(cap_identifiers: &[&str]) -> String {
    format!("CAP REQ :{}\r\n", cap_identifiers.join(" "))
}

pub fn cap_end() -> String {
    "CAP END\r\n".to_string()
}

pub fn authenticate(msg: &str) -> String {
    format!("AUTHENTICATE {}\r\n", msg)
}

/// Sender of a message
///
/// `<prefix> ::= <servername> | <nick> [ '!' <user> ] [ '@' <host> ]`
///
/// From RFC 2812:
///
/// > If the prefix is missing from the message, it is assumed to have originated from the
/// > connection from which it was received from.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Pfx {
    Server(String),

    /// <nick>!<user>@<host>
    User {
        nick: String,
        /// user@host
        user: String,
    },
}

/// Target of a message
#[derive(Debug, PartialEq, Eq)]
pub enum MsgTarget {
    Chan(String),
    User(String),
}

/// An IRC message
#[derive(Debug, PartialEq, Eq)]
pub struct Msg {
    pub pfx: Option<Pfx>,
    pub cmd: Cmd,
}

/// A client-to-client protocol message. See https://defs.ircdocs.horse/defs/ctcp.html
#[derive(Debug, PartialEq, Eq)]
pub enum CTCP {
    Version,
    Action,
    Other(String),
}

impl CTCP {
    fn parse(s: &str) -> CTCP {
        match s {
            "VERSION" => CTCP::Version,
            "ACTION" => CTCP::Action,
            _ => CTCP::Other(s.to_owned()),
        }
    }
}

/// An IRC command or reply
#[derive(Debug, PartialEq, Eq)]
pub enum Cmd {
    /// A PRIVMSG or NOTICE. Check `is_notice` field.
    PRIVMSG {
        // TODO: In theory this should be a list of targets, but in practice I've never
        // encountered that case.
        target: MsgTarget,
        msg: String,
        is_notice: bool,
        ctcp: Option<CTCP>,
    },

    JOIN {
        // TODO: Same as above, this should be a list ...
        chan: String, // TODO: key field might be useful when joining restricted channels. In practice I've never
                      // needed it.
    },

    PART {
        // TODO: List of channels
        chan: String,
        msg: Option<String>,
    },

    QUIT {
        msg: Option<String>,
        /// Channels of the user that just quit.
        chans: Vec<String>,
    },

    NICK {
        nick: String,
        /// Channels of the user.
        chans: Vec<String>,
    },

    PING {
        server: String,
    },

    PONG {
        server: String,
    },

    ERROR {
        msg: String,
    },

    TOPIC {
        chan: String,
        topic: String,
    },

    CAP {
        client: String,
        subcommand: String,
        params: Vec<String>,
    },

    AUTHENTICATE {
        param: String,
    },

    /// An IRC message other than the ones listed above.
    Other {
        cmd: String,
        params: Vec<String>,
    },

    /// Numeric replies are kept generic as there are just too many replies and we probably only
    /// need to handle a small subset of them.
    Reply {
        num: u16,
        params: Vec<String>,
    },
}

/// An intermediate type used during parsing.
enum MsgType<'a> {
    Cmd(&'a str),
    Num(u16),
}

static CRLF: [u8; 2] = [b'\r', b'\n'];

/// Try to read an IRC message off a buffer. Drops the message when parsing is successful.
/// Otherwise the buffer is left unchanged.
pub fn parse_irc_msg(buf: &mut Vec<u8>) -> Option<Msg> {
    // find "\r\n" separator. `IntoSearcher` implementation for slice needs `str` (why??) so
    // using this hacky method instead.
    let crlf_idx = {
        match buf.windows(2).position(|sub| sub == CRLF) {
            None => return None,
            Some(i) => i,
        }
    };

    let ret = {
        let mut slice: &[u8] = &buf[0..crlf_idx];

        let pfx: Option<Pfx> = {
            if slice[0] == b':' {
                // parse prefix
                let ws_idx = find_byte(slice, b' ').unwrap();
                let (mut pfx, slice_) = slice.split_at(ws_idx);
                // drop the : from pfx
                pfx = &pfx[1..];
                slice = &slice_[1..]; // drop the space
                Some(parse_pfx(pfx))
            } else {
                None
            }
        };

        let msg_ty: MsgType = {
            let ws_idx = find_byte(slice, b' ').unwrap();
            let (cmd, slice_) = slice.split_at(ws_idx);
            slice = &slice_[1..]; // drop the space
            match parse_reply_num(cmd) {
                None => MsgType::Cmd(unsafe {
                    // Cmd strings are added by the server and they're always ASCII strings, so
                    // this is safe and O(1).
                    str::from_utf8_unchecked(cmd)
                }),
                Some(num) => MsgType::Num(num),
            }
        };

        let params: Vec<&str> = parse_params(unsafe { str::from_utf8_unchecked(slice) });
        let cmd = match msg_ty {
            MsgType::Cmd("PRIVMSG") | MsgType::Cmd("NOTICE") if params.len() == 2 => {
                let is_notice = if let MsgType::Cmd("NOTICE") = msg_ty {
                    true
                } else {
                    false
                };
                let target = params[0];
                let mut msg = params[1];
                let target = if target.starts_with('#') {
                    MsgTarget::Chan(target.to_owned())
                } else {
                    MsgTarget::User(target.to_owned())
                };

                let mut ctcp: Option<CTCP> = None;
                if !msg.is_empty() && msg.as_bytes()[0] == 0x01 {
                    // Drop 0x01
                    msg = &msg[1..];
                    // Parse message type
                    for (byte_idx, byte) in msg.as_bytes().iter().enumerate() {
                        if *byte == 0x01 {
                            let ctcp_type = &msg[0..byte_idx];
                            ctcp = Some(CTCP::parse(ctcp_type));
                            msg = &msg[byte_idx + 1..];
                            break;
                        } else if *byte == b' ' {
                            let ctcp_type = &msg[0..byte_idx];
                            ctcp = Some(CTCP::parse(ctcp_type));
                            msg = &msg[byte_idx + 1..];
                            if !msg.is_empty() && msg.as_bytes()[msg.len() - 1] == 0x01 {
                                msg = &msg[..msg.len() - 1];
                            }
                            break;
                        }
                    }
                }

                Cmd::PRIVMSG {
                    target,
                    msg: msg.to_owned(),
                    is_notice,
                    ctcp,
                }
            }
            MsgType::Cmd("JOIN") if params.len() == 1 => {
                let chan = params[0];
                Cmd::JOIN {
                    chan: chan.to_owned(),
                }
            }
            MsgType::Cmd("PART") if params.len() == 1 || params.len() == 2 => {
                let mb_msg = if params.len() == 2 {
                    Some(params[1].to_owned())
                } else {
                    None
                };
                Cmd::PART {
                    chan: params[0].to_owned(),
                    msg: mb_msg,
                }
            }
            MsgType::Cmd("QUIT") if params.is_empty() || params.len() == 1 => {
                let mb_msg = params.get(1).map(|s| (*s).to_owned());

                Cmd::QUIT {
                    msg: mb_msg,
                    chans: Vec::new(),
                }
            }
            MsgType::Cmd("NICK") if params.len() == 1 => {
                let nick = params[0];
                Cmd::NICK {
                    nick: nick.to_owned(),
                    chans: Vec::new(),
                }
            }
            MsgType::Cmd("PING") if params.len() == 1 => Cmd::PING {
                server: params[0].to_owned(),
            },
            MsgType::Cmd("PONG") if !params.is_empty() => Cmd::PONG {
                server: params[0].to_owned(),
            },
            MsgType::Cmd("ERROR") if params.len() == 1 => Cmd::ERROR {
                msg: params[0].to_owned(),
            },
            MsgType::Cmd("TOPIC") if params.len() == 2 => Cmd::TOPIC {
                chan: params[0].to_owned(),
                topic: params[1].to_owned(),
            },
            MsgType::Cmd("CAP") if params.len() == 3 => Cmd::CAP {
                client: params[0].to_owned(),
                subcommand: params[1].to_owned(),
                params: params[2].split(' ').map(|s| s.to_owned()).collect(),
            },
            MsgType::Cmd("AUTHENTICATE") if params.len() == 1 => Cmd::AUTHENTICATE {
                param: params[0].to_owned(),
            },
            MsgType::Num(n) => Cmd::Reply {
                num: n,
                params: params.into_iter().map(|s| s.to_owned()).collect(),
            },
            MsgType::Cmd(cmd) => Cmd::Other {
                cmd: cmd.to_owned(),
                params: params.into_iter().map(|s| s.to_owned()).collect(),
            },
        };

        Msg { pfx, cmd }
    };

    buf.drain(0..crlf_idx + 2);
    Some(ret)
}

fn parse_pfx(pfx: &[u8]) -> Pfx {
    match find_byte(pfx, b'!') {
        None => Pfx::Server(unsafe { str::from_utf8_unchecked(pfx).to_owned() }),
        Some(idx) => Pfx::User {
            nick: unsafe { str::from_utf8_unchecked(&pfx[0..idx]) }.to_owned(),
            user: unsafe { str::from_utf8_unchecked(&pfx[idx + 1..]) }.to_owned(),
        },
    }
}

fn parse_reply_num(bs: &[u8]) -> Option<u16> {
    fn is_num_ascii(b: u8) -> bool {
        b >= b'0' && b <= b'9'
    }

    if bs.len() == 3 {
        let n3 = bs[0];
        let n2 = bs[1];
        let n1 = bs[2];
        if is_num_ascii(n3) && is_num_ascii(n2) && is_num_ascii(n1) {
            return Some(
                u16::from(n3 - b'0') * 100 + u16::from(n2 - b'0') * 10 + u16::from(n1 - b'0'),
            );
        }
    }
    None
}

fn parse_params(chrs: &str) -> Vec<&str> {
    debug_assert!(!chrs.starts_with(' '));

    let mut ret: Vec<&str> = Vec::new();

    let mut slice_begins = 0;
    for (char_idx, char) in chrs.char_indices() {
        if char == ':' {
            ret.push(unsafe { chrs.get_unchecked(char_idx + 1..chrs.len()) });
            return ret;
        } else if char == ' ' {
            ret.push(unsafe { chrs.get_unchecked(slice_begins..char_idx) });
            slice_begins = char_idx + 1;
        }
    }

    if slice_begins != chrs.len() {
        ret.push(unsafe { chrs.get_unchecked(slice_begins..chrs.len()) });
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

/// Nicks may have prefixes, indicating it is a operator, founder, or something else.
///
/// Channel Membership Prefixes: http://modern.ircdocs.horse/#channel-membership-prefixes
///
/// Returns the nick without prefix.
pub fn drop_nick_prefix(nick: &str) -> &str {
    static PREFIXES: [char; 5] = ['~', '&', '@', '%', '+'];

    if PREFIXES.contains(&nick.chars().next().unwrap()) {
        &nick[1..]
    } else {
        nick
    }
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
        let mut buf = vec![];
        write!(
            &mut buf,
            ":nick!~nick@unaffiliated/nick PRIVMSG tiny :a b c\r\n"
        )
        .unwrap();
        assert_eq!(
            parse_irc_msg(&mut buf),
            Some(Msg {
                pfx: Some(Pfx::User {
                    nick: "nick".to_owned(),
                    user: "~nick@unaffiliated/nick".to_owned(),
                }),
                cmd: Cmd::PRIVMSG {
                    target: MsgTarget::User("tiny".to_owned()),
                    msg: "a b c".to_owned(),
                    is_notice: false,
                    ctcp: None,
                },
            })
        );
        assert_eq!(buf.len(), 0);
    }

    #[test]
    fn test_notice_parsing() {
        let mut buf = vec![];
        write!(
            &mut buf,
            ":barjavel.freenode.net NOTICE * :*** Looking up your hostname...\r\n"
        )
        .unwrap();
        assert_eq!(
            parse_irc_msg(&mut buf),
            Some(Msg {
                pfx: Some(Pfx::Server("barjavel.freenode.net".to_owned())),
                cmd: Cmd::PRIVMSG {
                    target: MsgTarget::User("*".to_owned()),
                    msg: "*** Looking up your hostname...".to_owned(),
                    is_notice: true,
                    ctcp: None,
                },
            })
        );
    }

    #[test]
    fn test_numeric_parsing() {
        let mut buf = vec![];
        write!(
            &mut buf,
            ":barjavel.freenode.net 001 tiny :Welcome to the freenode Internet Relay Chat Network tiny\r\n"
        ).unwrap();
        write!(
            &mut buf,
            ":barjavel.freenode.net 002 tiny :Your host is barjavel.freenode.net[123.123.123.123/8001], \
             running version ircd-seven-1.1.4\r\n"
        ).unwrap();
        write!(
            &mut buf,
            ":barjavel.freenode.net 004 tiny_test barjavel.freenode.net \
             ircd-seven-1.1.4 DOQRSZaghilopswz \
             CFILMPQSbcefgijklmnopqrstvz bkloveqjfI\r\n"
        )
        .unwrap();
        write!(
            &mut buf,
            ":barjavel.freenode.net 005 tiny_test CHANTYPES=# EXCEPTS INVEX \
             CHANMODES=eIbq,k,flj,CFLMPQScgimnprstz CHANLIMIT=#:120 PREFIX=(ov)@+ \
             MAXLIST=bqeI:100 MODES=4 NETWORK=freenode STATUSMSG=@+ CALLERID=g \
             CASEMAPPING=rfc1459 :are supported by this server\r\n"
        )
        .unwrap();

        let mut msgs = vec![];
        while let Some(msg) = parse_irc_msg(&mut buf) {
            msgs.push(msg);
        }

        assert_eq!(msgs.len(), 4);
    }

    #[test]
    fn test_part_parsing() {
        let mut buf = vec![];
        write!(&mut buf, ":tiny!~tiny@123.123.123.123 PART #haskell\r\n").unwrap();
        assert_eq!(
            parse_irc_msg(&mut buf),
            Some(Msg {
                pfx: Some(Pfx::User {
                    nick: "tiny".to_owned(),
                    user: "~tiny@123.123.123.123".to_owned(),
                }),
                cmd: Cmd::PART {
                    chan: "#haskell".to_owned(),
                    msg: None,
                },
            })
        );
    }

    #[test]
    fn test_join_parsing() {
        let mut buf = vec![];
        write!(&mut buf, ":tiny!~tiny@192.168.0.1 JOIN #haskell\r\n").unwrap();
        assert_eq!(
            parse_irc_msg(&mut buf),
            Some(Msg {
                pfx: Some(Pfx::User {
                    nick: "tiny".to_owned(),
                    user: "~tiny@192.168.0.1".to_owned(),
                }),
                cmd: Cmd::JOIN {
                    chan: "#haskell".to_owned(),
                },
            })
        );
        assert_eq!(buf.len(), 0);
    }

    // Example from https://tools.ietf.org/id/draft-oakley-irc-ctcp-01.html
    #[test]
    fn test_ctcp_action_parsing_1() {
        let mut buf = vec![];
        write!(
            &mut buf,
            ":dan!u@localhost PRIVMSG #ircv3 :\x01ACTION writes some specs!\x01\r\n"
        )
        .unwrap();
        assert_eq!(
            parse_irc_msg(&mut buf),
            Some(Msg {
                pfx: Some(Pfx::User {
                    nick: "dan".to_owned(),
                    user: "u@localhost".to_owned(),
                }),
                cmd: Cmd::PRIVMSG {
                    target: MsgTarget::Chan("#ircv3".to_owned()),
                    msg: "writes some specs!".to_owned(),
                    is_notice: false,
                    ctcp: Some(CTCP::Action),
                },
            })
        );
        assert_eq!(buf.len(), 0);
    }

    #[test]
    fn test_ctcp_action_parsing_2() {
        // From https://modern.ircdocs.horse/ctcp.html:
        //
        // > The final <delim> MUST be sent, but parsers SHOULD accept incoming messages which lack
        // > it (particularly for CTCP ACTION). This is due to how some software incorrectly
        // > implements message splitting.
        let mut buf = vec![];
        write!(
            &mut buf,
            ":a!b@c PRIVMSG target :\x01ACTION msg contents\r\n"
        )
        .unwrap();
        assert_eq!(
            parse_irc_msg(&mut buf).unwrap().cmd,
            Cmd::PRIVMSG {
                target: MsgTarget::User("target".to_owned()),
                msg: "msg contents".to_owned(),
                is_notice: false,
                ctcp: Some(CTCP::Action),
            }
        );
        assert_eq!(buf.len(), 0);

        let mut buf = vec![];
        write!(&mut buf, ":a!b@c PRIVMSG target :\x01ACTION \r\n").unwrap();
        assert_eq!(
            parse_irc_msg(&mut buf).unwrap().cmd,
            Cmd::PRIVMSG {
                target: MsgTarget::User("target".to_owned()),
                msg: "".to_owned(),
                is_notice: false,
                ctcp: Some(CTCP::Action),
            }
        );
        assert_eq!(buf.len(), 0);

        // This is a regression test: the slice [..8] takes the substring with only a part of one
        // of the '’'s.
        let mut buf = vec![];
        write!(&mut buf, ":a!b@c PRIVMSG target :’’’’’’’\r\n").unwrap();
        assert_eq!(
            parse_irc_msg(&mut buf).unwrap().cmd,
            Cmd::PRIVMSG {
                target: MsgTarget::User("target".to_owned()),
                msg: "’’’’’’’".to_owned(),
                is_notice: false,
                ctcp: None,
            }
        );
        assert_eq!(buf.len(), 0);
    }

    #[test]
    fn test_ctcp_version_parsing() {
        let mut buf = vec![];
        write!(&mut buf, ":a!b@c PRIVMSG target :\x01VERSION\x01\r\n").unwrap();
        assert_eq!(
            parse_irc_msg(&mut buf).unwrap().cmd,
            Cmd::PRIVMSG {
                target: MsgTarget::User("target".to_owned()),
                msg: "".to_owned(),
                is_notice: false,
                ctcp: Some(CTCP::Version),
            }
        );

        let mut buf = vec![];
        write!(&mut buf, ":a!b@c PRIVMSG target :\x01VERSION \x01\r\n").unwrap();
        assert_eq!(
            parse_irc_msg(&mut buf).unwrap().cmd,
            Cmd::PRIVMSG {
                target: MsgTarget::User("target".to_owned()),
                msg: "".to_owned(),
                is_notice: false,
                ctcp: Some(CTCP::Version),
            }
        );
    }

    #[test]
    fn other_ctcp_parsing() {
        let mut buf = vec![];
        write!(&mut buf, ":a!b@c PRIVMSG target :\x01blah blah \x01\r\n").unwrap();
        assert_eq!(
            parse_irc_msg(&mut buf).unwrap().cmd,
            Cmd::PRIVMSG {
                target: MsgTarget::User("target".to_owned()),
                msg: "blah ".to_owned(),
                is_notice: false,
                ctcp: Some(CTCP::Other("blah".to_owned())),
            }
        );

        let mut buf = vec![];
        write!(&mut buf, ":a!b@c PRIVMSG target :\x01blah blah \r\n").unwrap();
        assert_eq!(
            parse_irc_msg(&mut buf).unwrap().cmd,
            Cmd::PRIVMSG {
                target: MsgTarget::User("target".to_owned()),
                msg: "blah ".to_owned(),
                is_notice: false,
                ctcp: Some(CTCP::Other("blah".to_owned())),
            }
        );
    }

    #[test]
    fn test_error_parsing() {
        let mut buf = vec![];
        write!(
            &mut buf,
            "ERROR :Closing Link: 212.252.143.51 (Excess Flood)\r\n"
        )
        .unwrap();
        assert_eq!(
            parse_irc_msg(&mut buf),
            Some(Msg {
                pfx: None,
                cmd: Cmd::ERROR {
                    msg: "Closing Link: 212.252.143.51 (Excess Flood)".to_owned(),
                },
            }),
        );
    }
}
