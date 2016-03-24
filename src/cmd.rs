pub enum Cmd {
    Connect(String),
}

impl Cmd {
    pub fn parse(cmd : &[char]) -> Result<Cmd, String> {
        if cmd[0] != '/' {
            return Err(format!("Not a cmd: \"{}\"", cmd.iter().cloned().collect::<String>()));
        }

        // Drop '/'
        let cmd = &cmd[ 1 .. ];

        let words : Vec<String> = cmd.split(|c| *c == ' ')
                                     .map(|cs| cs.iter().cloned().collect::<String>())
                                     .collect();

        if words.len() == 2 && words[0] == "connect" {
            Ok(Cmd::Connect(words.into_iter().nth(1).unwrap()))
        } else {
            Err(format!("Can't parse cmd: \"{}\"", cmd.iter().cloned().collect::<String>()))
        }
    }
}
