use utils::find_char;

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
    pub fn parse(msg : Vec<char>) -> Result<Msg, String> {
        if msg.len() == 0 {
            return Err("Empty msg".to_owned());
        }

        let mut slice = msg.as_slice();

        let prefix : Option<String> = {
            if msg[0] == ':' {
                // parse prefix
                let ws_idx = find_char(slice, ' ').unwrap();
                let (prefix, slice_) = slice.split_at(ws_idx);
                slice = &slice_[ 1 .. ]; // drop the space
                Some(prefix.into_iter().cloned().collect())
            } else {
                None
            }
        };

        let command : Command = {
            let ws_idx = find_char(slice, ' ').unwrap();
            let (command, slice_) = slice.split_at(ws_idx);
            slice = &slice_[ 1 .. ]; // drop the space
            Command::Str(command.into_iter().cloned().collect())
        };

        let params = try!(parse_params(slice));

        Ok(Msg {
            prefix: prefix,
            command: command,
            params: params,
        })
    }
}

fn parse_params(mut chrs : &[char]) -> Result<Vec<String>, String> {
    if chrs.len() == 0 {
        return Err("parse_params: Empty slice of chars".to_owned());
    }

    if chrs[0] == ':' {
        // Consume the thing until \r\n
        let start_idx = 1; // drop the colon
        let end_idx   = find_char(chrs, '\r').unwrap();
        Ok(vec![
           chrs[ start_idx .. end_idx ].into_iter().cloned().collect()
        ])
    } else {
        let mut ret : Vec<String> = Vec::new();

        loop {
            match find_char(chrs, ' ') {
                None => {
                    // Hopefully the rest if just \r\n
                    // TODO: Make sure
                    break;
                },
                Some(end_idx) => {
                    ret.push(chrs[ 0 .. end_idx ].into_iter().cloned().collect());
                    chrs = &chrs[ end_idx + 1 .. ]; // +1 to drop the space
                }
            }
        }

        Ok(ret)
    }
}
