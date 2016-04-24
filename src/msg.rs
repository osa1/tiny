use std::io::Write;
use std::io;
use std::str;

use utils::{find_byte};

#[derive(Debug, PartialEq, Eq)]
pub struct Msg {
    pub pfx     : Option<Pfx>,
    pub cmd     : Cmd,
    pub params  : Vec<Vec<u8>>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Pfx {
    Server(String),

    User {
        nick : String,
        /// user@host
        user : String,
    },
}

impl Pfx {
    #[inline]
    pub fn is_server_pfx(&self) -> bool {
        match *self {
            Pfx::Server(_) => true,
            _ => false,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Cmd {
    Str(String),
    Num(u16),
}

impl Msg {
    /// Parse a complete IRC message. NOTE: The string should NOT have CR-NL.
    pub fn parse(msg : &[u8]) -> Result<Msg, String> {

        writeln!(&mut io::stderr(), "parsing msg: {:?}", String::from_utf8(msg.to_owned())).unwrap();

        if msg.is_empty() {
            return Err("Empty msg".to_owned());
        }

        let mut slice = msg;

        let pfx : Option<Pfx> = {
            if msg[0] == b':' {
                // parse prefix
                let ws_idx = find_byte(slice, b' ').unwrap();
                let (mut pfx, slice_) = slice.split_at(ws_idx);
                // drop the : from pfx
                pfx = &pfx[ 1 .. ];
                slice = &slice_[ 1 .. ]; // drop the space
                Some(parse_pfx(&pfx))
            } else {
                // log_stderr_bytes("Can't parse msg prefix:", msg);
                None
            }
        };

        let cmd : Cmd = {
            let ws_idx = find_byte(slice, b' ').unwrap();
            let (cmd, slice_) = slice.split_at(ws_idx);
            slice = &slice_[ 1 .. ]; // drop the space
            match reply_num(cmd) {
                None => Cmd::Str(unsafe {
                    // Cmd strings are added by the server and they're always
                    // ASCII strings, so this is safe and O(1).
                    String::from_utf8_unchecked(cmd.to_owned())
                }),
                Some(num) => Cmd::Num(num)
            }
        };

        let params = try!(parse_params(slice));

        Ok(Msg {
            pfx: pfx,
            cmd: cmd,
            params: params,
        })
    }

    ////////////////////////////////////////////////////////////////////////////
    // Message generation

    pub fn user<W : Write>(hostname : &str, realname : &str, mut sink : W) -> io::Result<()> {
        write!(sink, "USER {} 0 * :{}\r\n", hostname, realname)
    }

    pub fn nick<W : Write>(arg : &str, mut sink : W) -> io::Result<()> {
        write!(sink, "NICK {}\r\n", arg)
    }

    pub fn pong<W : Write>(arg : &str, mut sink : W) -> io::Result<()> {
        write!(sink, "PONG {}\r\n", arg)
    }

    pub fn join<W : Write>(channel : &str, mut sink : W) -> io::Result<()> {
        write!(sink, "JOIN {}\r\n", channel)
    }

    pub fn privmsg<W : Write>(msgtarget : &str, msg : &str, mut sink : W) -> io::Result<()> {
        write!(sink, "PRIVMSG {} :{}\r\n", msgtarget, msg)
    }

    pub fn quit<W : Write>(msg : Option<&str>, mut sink : W) -> io::Result<()> {
        match msg {
            None => write!(sink, "QUIT\r\n"),
            Some(msg) => write!(sink, "QUIT {}\r\n", msg)
        }
    }
}

fn reply_num(bs : &[u8]) -> Option<u16> {
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

#[inline]
fn is_num_ascii(b : u8) -> bool {
    b >= b'0' && b <= b'9'
}

fn parse_pfx(pfx : &[u8]) -> Pfx {
    match find_byte(pfx, b'!') {
        None => Pfx::Server(unsafe { str::from_utf8_unchecked(pfx).to_owned() }),
        Some(idx) => {
            Pfx::User {
                nick: unsafe { str::from_utf8_unchecked(&pfx[ 0 .. idx ]).to_owned() },
                user: unsafe { str::from_utf8_unchecked(&pfx[ idx + 1 .. ]).to_owned() }
            }
        }
    }
}

fn parse_params(chrs : &[u8]) -> Result<Vec<Vec<u8>>, String> {
    if chrs.len() == 0 {
        return Err("parse_params: Empty slice of chars".to_owned());
    }

    let mut ret : Vec<Vec<u8>> = Vec::new();

    let mut current_param = Vec::new();
    for byte_idx in 0 .. chrs.len() {
        let byte = *unsafe { chrs.get_unchecked(byte_idx) };
        if byte == b':' {
            current_param.extend_from_slice(&chrs[ byte_idx + 1 .. ]);
            ret.push(current_param);
            return Ok(ret);
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

    Ok(ret)
}

////////////////////////////////////////////////////////////////////////////////

#[test]
fn parse_error_1() {
    let msg = b"ERROR :Closing Link: 127.0.0.1 (Client Quit)";
    assert!(Msg::parse(msg).is_ok());
}

#[test]
fn parse_error_2() {
    let msg = b":tiny_test!~tiny@213.153.193.52 JOIN #haskell";
    assert_eq!(Msg::parse(msg), Ok(Msg {
        pfx: Some(Pfx::User {
            nick: "tiny_test".to_string(),
            user: "~tiny@213.153.193.52".to_string(),
        }),
        cmd: Cmd::Str("JOIN".to_string()),
        params: vec![
            (Box::new(*b"#haskell") as Box<[u8]>).into_vec()
        ]
    }));
}

#[test]
fn parse_error_3() {
    let msg = b":verne.freenode.net NOTICE * :*** Couldn\'t look up your hostname";
    assert_eq!(Msg::parse(msg), Ok(Msg {
        pfx: Some(Pfx::Server("verne.freenode.net".to_owned())),
        cmd: Cmd::Str("NOTICE".to_owned()),
        params: vec![
            (Box::new(*b"*") as Box<[u8]>).into_vec(),
            (Box::new(*b"*** Couldn\'t look up your hostname") as Box<[u8]>).into_vec()
        ]
    }));
}
