use std::str;

use utils::find_byte;

#[derive(Debug)]
pub struct Msg {
    prefix  : Option<String>,
    command : Command,
    params  : Vec<String>,
}

#[derive(Debug)]
pub enum Command {
    Str(String),
    Num(i16),
}

impl Msg {
    pub fn parse(msg : &Vec<u8>) -> Result<Msg, String> {
        if msg.len() == 0 {
            return Err("Empty msg".to_owned());
        }

        let mut slice = msg.as_slice();

        let prefix : Option<String> = {
            if msg[0] == b':' {
                // parse prefix
                let ws_idx = find_byte(slice, b' ').unwrap();
                let (prefix, slice_) = slice.split_at(ws_idx);
                slice = &slice_[ 1 .. ]; // drop the space
                str::from_utf8(prefix).ok().map(|s| s.to_owned())
            } else {
                None
            }
        };

        let command : Command = {
            let ws_idx = find_byte(slice, b' ').unwrap();
            let (command, slice_) = slice.split_at(ws_idx);
            slice = &slice_[ 1 .. ]; // drop the space
            Command::Str(str::from_utf8(command).unwrap().to_owned())
        };

        let params = try!(parse_params(slice));

        Ok(Msg {
            prefix: prefix,
            command: command,
            params: params,
        })
    }
}

fn parse_params(mut chrs : &[u8]) -> Result<Vec<String>, String> {
    if chrs.len() == 0 {
        return Err("parse_params: Empty slice of chars".to_owned());
    }

    if chrs[0] == b':' {
        // Consume the thing until \r\n
        let start_idx = 1; // drop the colon
        let end_idx   = find_byte(chrs, b'\r').unwrap();
        Ok(vec![
           str::from_utf8(&chrs[ start_idx .. end_idx ]).unwrap().to_owned()
        ])
    } else {
        let mut ret : Vec<String> = Vec::new();

        loop {
            match find_byte(chrs, b' ') {
                None => {
                    // Hopefully the rest if just \r\n
                    // TODO: Make sure
                    break;
                },
                Some(end_idx) => {
                    ret.push(str::from_utf8(&chrs[ 0 .. end_idx ]).unwrap().to_owned());
                    chrs = &chrs[ end_idx + 1 .. ]; // +1 to drop the space
                }
            }
        }

        Ok(ret)
    }
}
