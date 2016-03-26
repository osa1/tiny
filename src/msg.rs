use std::io::Write;
use std::io;

use utils::{find_byte, log_stderr_bytes};

#[derive(Debug)]
pub struct Msg {
    // Does not include the ':' prefix
    pub prefix  : Option<Vec<u8>>,
    pub command : Command,
    pub params  : Vec<Vec<u8>>,
}

#[derive(Debug)]
pub enum Command {
    Str(String),
    Num(u16),
}

impl Msg {
    /// Parse a complete IRC message. NOTE: The string should NOT have CR-NL.
    pub fn parse(msg : &[u8]) -> Result<Msg, String> {
        if msg.len() == 0 {
            return Err("Empty msg".to_owned());
        }

        let mut slice = msg;

        let prefix : Option<Vec<u8>> = {
            if msg[0] == b':' {
                // parse prefix
                let ws_idx = find_byte(slice, b' ').unwrap();
                let (prefix, slice_) = slice.split_at(ws_idx);
                slice = &slice_[ 1 .. ]; // drop the space
                Some(prefix.to_owned())
            } else {
                log_stderr_bytes("Can't parse msg prefix:", msg);
                None
            }
        };

        let command : Command = {
            let ws_idx = find_byte(slice, b' ').unwrap();
            let (command, slice_) = slice.split_at(ws_idx);
            slice = &slice_[ 1 .. ]; // drop the space
            match reply_num(command) {
                None => Command::Str(unsafe {
                    // Command strings are added by the server and they're
                    // always ASCII strings, so this is safe and O(1).
                    String::from_utf8_unchecked(command.to_owned())
                }),
                Some(num) => Command::Num(num)
            }
        };

        let params = try!(parse_params(slice));

        Ok(Msg {
            prefix: prefix,
            command: command,
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
        write!(sink, "PRIVMSG {} {}\r\n", msgtarget, msg)
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

fn parse_params(mut chrs : &[u8]) -> Result<Vec<Vec<u8>>, String> {
    if chrs.len() == 0 {
        return Err("parse_params: Empty slice of chars".to_owned());
    }

    if chrs[0] == b':' {
        let start_idx = 1; // drop the colon
        Ok(vec![
           (&chrs[ start_idx .. ]).to_owned()
        ])
    } else {
        let mut ret : Vec<Vec<u8>> = Vec::new();

        loop {
            match find_byte(chrs, b' ') {
                None => {
                    ret.push(chrs.to_owned());
                    break;
                },
                Some(end_idx) => {
                    ret.push((&chrs[ 0 .. end_idx ]).to_owned());
                    chrs = &chrs[ end_idx + 1 .. ]; // +1 to drop the space
                }
            }
        }

        Ok(ret)
    }
}

////////////////////////////////////////////////////////////////////////////////

#[test]
fn parse_error_1() {
    let msg = b"ERROR :Closing Link: 127.0.0.1 (Client Quit)";
    assert!(Msg::parse(msg).is_ok());
}
