use utils::{find_byte, log_stderr_bytes};

#[derive(Debug)]
pub struct Msg {
    prefix  : Option<Vec<u8>>,
    command : Command,
    params  : Vec<Vec<u8>>,
}

#[derive(Debug)]
pub enum Command {
    Str(Vec<u8>),
    Num(u16),
}

impl Msg {
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
                None => Command::Str(command.to_owned()),
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
        // Consume the thing until \r\n
        let end_idx   = find_byte(chrs, b'\r').unwrap();
        Ok(vec![
           (&chrs[ start_idx .. end_idx ]).to_owned()
        ])
    } else {
        let mut ret : Vec<Vec<u8>> = Vec::new();

        loop {
            match find_byte(chrs, b' ') {
                None => {
                    // Hopefully the rest if just \r\n
                    // TODO: Make sure
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
