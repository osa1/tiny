#![allow(clippy::write_with_newline)]

//! IRC wire protocol message parsers and generators. Incomplete; new messages are added as needed.
//!
//! This library is for implementing clients rather than servers or services, and does not support
//! the IRC message format in full generality.

use std::str;

use libtiny_common::{ChanName, ChanNameRef};

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

pub fn join<'a, I>(chans: I) -> String
where
    I: Iterator<Item = &'a ChanNameRef> + 'a,
{
    let chans = chans.map(|c| c.display()).collect::<Vec<_>>();
    format!("JOIN {}\r\n", chans.join(","))
}

pub fn part(chan: &ChanNameRef) -> String {
    format!("PART {}\r\n", chan.display())
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

/// Sender of a message ("prefix" in the RFC). Instead of returning a `String` we parse prefix part
/// of the message according to the RFC because users of this library sometimes need to distinguish
/// a server from a user. For example, in tiny if a PRIVMSG to us is coming from a server then we
/// show it in the server tab. Otherwise we show it in the sender's (user) tab.
///
/// (Note that the ambiguity in the RFC makes this a best-effort thing. When we get a PRIVMSG from
/// e.g. "foo" it's not possible to know whether "foo" is a server or a user.)
///
/// One alternative here would be to defer parsing to the users so that, for example, if in the
/// context we expect the message to be coming from a user we call `Pfx::parse_user()` which
/// interprets the ambiguous case as "user" and `Pfx::parse_server()` which interprets it as
/// "server". The downside is that'd sometimes means parsing the prefix multiple times. For
/// example, in tiny, a Client would parse the prefix to get the nick to update the channel state,
/// then we'd parse it again in tiny to update the TUI.

// We could still provide `get_server()` and `get_nick()` that interpret the ambiguous case as
// server and nick, respectively, but I don't think it'd be much more convenient that pattern
// matching explicitly. See the commented-out code below.

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Pfx {
    /// Sender is a server.
    Server(String),

    /// Sender is a nick.
    User {
        /// Nick of the sender
        nick: String,
        /// `user@host` part
        user: String,
    },

    /// Server could be a server or nick, it's unclear. According to the RFC if we have something
    /// like "localhost" which doesn't have '!', '@', or a character that 'servername' can have but
    /// 'nickname' cannot, we can't tell whether the sender is a server or a nick. In those cases
    /// we return this variant. See also #247.
    Ambiguous(String),
}

/*
impl Pfx {
    /// Get the server name if the prefix is for a server. Ambiguous case is interpreted as server.
    pub fn get_server(&self) -> Option<&str> {
        match self {
            Pfx::Server(ref server) | Pfx::Ambiguous(ref server) => Some(server),
            Pfx::User { .. } => None,
        }
    }

    /// Get the nick if the prefix is for a user. Ambiguous case is interpreted as a nick.
    pub fn get_nick(&self) -> Option<&str> {
        match self {
            Pfx::User { ref nick, .. } | Pfx::Ambiguous(ref nick) => Some(nick),
            Pfx::Server(_) => None,
        }
    }
}
*/

// RFC 2812 section 2.3.1
fn parse_pfx(pfx: &str) -> Pfx {
    match pfx.find(&['!', '@'][..]) {
        Some(idx) => Pfx::User {
            nick: (&pfx[0..idx]).to_owned(),
            user: (&pfx[idx + 1..]).to_owned(),
        },
        None => {
            // Chars that nicks can have but servernames cannot
            match pfx.find(&['[', ']', '\\', '`', '_', '^', '{', '|', '}'][..]) {
                Some(_) => Pfx::User {
                    nick: pfx.to_owned(),
                    user: "".to_owned(),
                },
                None => {
                    // Nicks can't have '.'
                    match pfx.find('.') {
                        Some(_) => Pfx::Server(pfx.to_owned()),
                        None => Pfx::Ambiguous(pfx.to_owned()),
                    }
                }
            }
        }
    }
}

/// Target of a message
///
/// Masks are not parsed, as rules for masks are not clear in RFC 2818 (for example, `#x.y` can be
/// a channel name or a host mask, there is no way to disambiguate), and in practice servers use
/// masks that are not valid according to the RFC (for example, I've observed Freenode sending
/// PRIVMSGs to `$$*`). The rules we follow is: if a target starts with `#` it's a `Chan`,
/// otherwise it's a `User`.
#[derive(Debug, PartialEq, Eq)]
pub enum MsgTarget {
    Chan(ChanName),
    User(String),
}

/// An IRC message
#[derive(Debug, PartialEq, Eq)]
pub struct Msg {
    /// Sender of a message. According to RFC 2812 it's optional:
    ///
    /// > If the prefix is missing from the message, it is assumed to have originated from the
    /// > connection from which it was received from.
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
        chan: ChanName, // TODO: key field might be useful when joining restricted channels. In
                        // practice I've never needed it.
    },

    PART {
        // TODO: List of channels
        chan: ChanName,
        msg: Option<String>,
    },

    QUIT {
        msg: Option<String>,
        /// Channels of the user that just quit. This is not a part of the IRC message, but
        /// something `libtiny_client` fills in for the users. Currently used to update tabs of the
        /// user in TUI.
        chans: Vec<ChanName>,
    },

    NICK {
        nick: String,
        /// Channels of the user. Channels of the user that just quit. This is not a part of the
        /// IRC message, but something `libtiny_client` fills in for the users. Currently used to
        /// update tabs of the user in TUI.
        chans: Vec<ChanName>,
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
        chan: ChanName,
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
pub fn parse_irc_msg(buf: &mut Vec<u8>) -> Option<Result<Msg, String>> {
    // Find "\r\n" separator. We can't do this *after* generating the lossy UTF-8, as that may have
    // different size than the original buffer after inserting "REPLACEMENT CHARACTER"s.
    let crlf_idx = {
        match buf.windows(2).position(|sub| sub == CRLF) {
            None => return None,
            Some(i) => i,
        }
    };

    let msg_owned: String = String::from_utf8_lossy(&buf[0..crlf_idx]).to_string();
    let msg: &str = &msg_owned;

    let ret = parse_one_message(msg);
    buf.drain(0..crlf_idx + 2);

    Some(ret)
}

// NB. 'msg' does not contain '\r\n' suffix.
fn parse_one_message(mut msg: &str) -> Result<Msg, String> {
    let pfx: Option<Pfx> = {
        if let Some(':') = msg.chars().next() {
            // parse prefix
            let ws_idx = msg.find(' ').ok_or(format!(
                "Can't find prefix terminator (' ') in msg: {:?}",
                msg
            ))?;
            let pfx = &msg[1..ws_idx]; // consume ':'
            msg = &msg[ws_idx + 1..]; // consume ' '
            Some(parse_pfx(pfx))
        } else {
            None
        }
    };

    let msg_ty: MsgType = {
        let ws_idx = msg.find(' ').ok_or(format!(
            "Can't find message type terminator (' ') in msg: {:?}",
            msg
        ))?;
        let cmd = &msg[..ws_idx];
        msg = &msg[ws_idx + 1..]; // Consume ' '
        match cmd.parse::<u16>() {
            Ok(num) => MsgType::Num(num),
            Err(_) => MsgType::Cmd(cmd),
        }
    };

    let params = parse_params(msg);
    let cmd = match msg_ty {
        MsgType::Cmd("PRIVMSG") | MsgType::Cmd("NOTICE") if params.len() == 2 => {
            let is_notice = matches!(msg_ty, MsgType::Cmd("NOTICE"));
            let target = params[0];
            let mut msg = params[1];
            let target = if target.starts_with('#') {
                MsgTarget::Chan(ChanName::new(target.to_owned()))
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
                chan: ChanName::new(chan.to_owned()),
            }
        }
        MsgType::Cmd("PART") if params.len() == 1 || params.len() == 2 => {
            let mb_msg = if params.len() == 2 {
                Some(params[1].to_owned())
            } else {
                None
            };
            Cmd::PART {
                chan: ChanName::new(params[0].to_owned()),
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
            chan: ChanName::new(params[0].to_owned()),
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

    Ok(Msg { pfx, cmd })
}

fn parse_params(chrs: &str) -> Vec<&str> {
    // Spec:
    //
    //     params     =  *14( SPACE middle ) [ SPACE ":" trailing ]
    //                =/ 14( SPACE middle ) [ SPACE [ ":" ] trailing ]
    //
    //     nospcrlfcl =  %x01-09 / %x0B-0C / %x0E-1F / %x21-39 / %x3B-FF
    //                     ; any octet except NUL, CR, LF, " " and ":"
    //     middle     =  nospcrlfcl *( ":" / nospcrlfcl )
    //     trailing   =  *( ":" / " " / nospcrlfcl )
    //
    // The RFC doesn't explain the syntax with `14` here as if it's something standard. I'm
    // guessing it's number of repetitions, and `*14` means "14 or less" repetitions.

    let mut params = Vec::new();
    let mut char_indices = chrs.char_indices();

    while let Some((idx, c)) = char_indices.next() {
        if c == ':' {
            params.push(&chrs[idx + 1..]); // Skip ':'
            break;
        }

        if params.len() == 14 {
            params.push(&chrs[idx..]);
            break;
        }

        if c == ' ' {
            continue;
        }

        loop {
            match char_indices.next() {
                Some((idx_, c)) => {
                    if c == ' ' {
                        params.push(&chrs[idx..idx_]);
                        break;
                    }
                }
                None => {
                    params.push(&chrs[idx..]);
                    break;
                }
            }
        }
    }

    params
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
        let empty: Vec<&str> = vec![];
        assert_eq!(parse_params(""), empty);
        assert_eq!(parse_params(":foo bar baz "), vec!["foo bar baz "]);
        assert_eq!(
            parse_params(":foo : bar : baz :"),
            vec!["foo : bar : baz :"]
        );
        assert_eq!(parse_params(":"), vec![""]);
        assert_eq!(parse_params("x:"), vec!["x:"]);
        assert_eq!(parse_params("x:y"), vec!["x:y"]);
        assert_eq!(parse_params("x:y:z"), vec!["x:y:z"]);
        assert_eq!(parse_params(":::::"), vec!["::::"]);

        let params = parse_params("1 2 3 4 5 6 7 8 9 10 11 12 13 14 blah blah blah");
        assert_eq!(params.len(), 15);
        assert_eq!(params[params.len() - 1], "blah blah blah");

        assert_eq!(parse_params("   "), empty); // Not valid according to the RFC, I think
        assert_eq!(parse_params(":  "), vec!["  "]);
        assert_eq!(parse_params(": : :"), vec![" : :"]);
        assert_eq!(parse_params("x y : : :"), vec!["x", "y", " : :"]);
        assert_eq!(parse_params("aaa://aaa"), vec!["aaa://aaa"]);
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
            parse_irc_msg(&mut buf).unwrap().unwrap(),
            Msg {
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
            }
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
            parse_irc_msg(&mut buf).unwrap().unwrap(),
            Msg {
                pfx: Some(Pfx::Server("barjavel.freenode.net".to_owned())),
                cmd: Cmd::PRIVMSG {
                    target: MsgTarget::User("*".to_owned()),
                    msg: "*** Looking up your hostname...".to_owned(),
                    is_notice: true,
                    ctcp: None,
                },
            }
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
        while let Some(Ok(msg)) = parse_irc_msg(&mut buf) {
            assert_eq!(
                msg.pfx,
                Some(Pfx::Server("barjavel.freenode.net".to_owned()))
            );
            msgs.push(msg);
        }

        assert_eq!(msgs.len(), 4);
    }

    #[test]
    fn test_part_parsing() {
        let mut buf = vec![];
        write!(&mut buf, ":tiny!~tiny@123.123.123.123 PART #haskell\r\n").unwrap();
        assert_eq!(
            parse_irc_msg(&mut buf).unwrap().unwrap(),
            Msg {
                pfx: Some(Pfx::User {
                    nick: "tiny".to_owned(),
                    user: "~tiny@123.123.123.123".to_owned(),
                }),
                cmd: Cmd::PART {
                    chan: ChanName::new("#haskell".to_owned()),
                    msg: None,
                },
            }
        );
    }

    #[test]
    fn test_join_parsing() {
        let mut buf = vec![];
        write!(&mut buf, ":tiny!~tiny@192.168.0.1 JOIN #haskell\r\n").unwrap();
        assert_eq!(
            parse_irc_msg(&mut buf).unwrap().unwrap(),
            Msg {
                pfx: Some(Pfx::User {
                    nick: "tiny".to_owned(),
                    user: "~tiny@192.168.0.1".to_owned(),
                }),
                cmd: Cmd::JOIN {
                    chan: ChanName::new("#haskell".to_owned()),
                },
            }
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
            parse_irc_msg(&mut buf).unwrap().unwrap(),
            Msg {
                pfx: Some(Pfx::User {
                    nick: "dan".to_owned(),
                    user: "u@localhost".to_owned(),
                }),
                cmd: Cmd::PRIVMSG {
                    target: MsgTarget::Chan(ChanName::new("#ircv3".to_owned())),
                    msg: "writes some specs!".to_owned(),
                    is_notice: false,
                    ctcp: Some(CTCP::Action),
                },
            }
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
            parse_irc_msg(&mut buf).unwrap().unwrap().cmd,
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
            parse_irc_msg(&mut buf).unwrap().unwrap().cmd,
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
            parse_irc_msg(&mut buf).unwrap().unwrap().cmd,
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
            parse_irc_msg(&mut buf).unwrap().unwrap().cmd,
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
            parse_irc_msg(&mut buf).unwrap().unwrap().cmd,
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
            parse_irc_msg(&mut buf).unwrap().unwrap().cmd,
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
            parse_irc_msg(&mut buf).unwrap().unwrap().cmd,
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
            parse_irc_msg(&mut buf).unwrap().unwrap(),
            Msg {
                pfx: None,
                cmd: Cmd::ERROR {
                    msg: "Closing Link: 212.252.143.51 (Excess Flood)".to_owned(),
                },
            },
        );
    }

    #[test]
    fn test_parse_pfx() {
        use Pfx::*;
        assert_eq!(parse_pfx("xyz"), Ambiguous("xyz".to_string()));
        assert_eq!(parse_pfx("xy-z"), Ambiguous("xy-z".to_string()),);
        assert_eq!(parse_pfx("xy.z"), Server("xy.z".to_string()));
        assert_eq!(
            parse_pfx("xyz[m]"),
            User {
                nick: "xyz[m]".to_string(),
                user: "".to_string()
            }
        );
        assert_eq!(
            parse_pfx("fe-00106.xyz.net"),
            Server("fe-00106.xyz.net".to_string())
        );
        assert_eq!(
            parse_pfx("osa1!osa1@x.y.im"),
            User {
                nick: "osa1".to_string(),
                user: "osa1@x.y.im".to_string(),
            }
        );
        assert_eq!(
            parse_pfx("IRC!IRC@fe-00106.xyz.net"),
            User {
                nick: "IRC".to_string(),
                user: "IRC@fe-00106.xyz.net".to_string()
            }
        );
    }
}
