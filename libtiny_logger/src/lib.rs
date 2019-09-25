use std::collections::HashMap;
use std::fs;
use std::fs::{File, OpenOptions};
use std::io;
use std::io::Result;
use std::path::PathBuf;

use libtiny_wire as wire;

pub struct Logger {
    /// Log file directory
    log_dir: PathBuf,

    /// Server name for this logger. All file names will be prefixed with this.
    serv_name: String,

    /// File for the server logs.
    server_fd: File,

    /// Maps channels/users to their files.
    fds: HashMap<String, File>,
}

impl Logger {
    pub fn new(log_dir: PathBuf, serv_name: String) -> Result<Logger> {
        if let Err(err) = fs::create_dir(&log_dir) {
            if err.kind() != io::ErrorKind::AlreadyExists {
                return Err(err);
            }
        }

        let mut server_fd_path = log_dir.clone();
        server_fd_path.push(&format!("{}.txt", serv_name));
        let server_fd = OpenOptions::new().append(true).open(server_fd_path)?;

        // TODO: Write a "logs started" lines

        Ok(Logger {
            log_dir,
            serv_name,
            server_fd,
            fds: HashMap::new(),
        })
    }

    fn get_file(&mut self, target: &str) -> Result<&mut File> {
        // *sigh* Double lookup to make borrowchk happy
        if self.fds.contains_key(target) {
            return Ok(self.fds.get_mut(target).unwrap());
        }

        let mut file_path = self.log_dir.clone();
        file_path.push(&format!("{}.txt", target));
        let fd = OpenOptions::new().append(true).open(file_path)?;
        self.fds.insert(target.to_owned(), fd);
        self.get_file(target)
    }

    pub fn log_incoming_msg(&mut self, msg: &wire::Msg) -> Result<()> {
        let wire::Msg { pfx, cmd } = msg;
        let sender = match pfx {
            Some(sender) => sender,
            None => { return Ok(()); }
        };

        use wire::Cmd::*;
        use wire::MsgTarget;
        match cmd {
            PRIVMSG { target, msg, is_notice, is_action } => {
                let target_file = match target {
                    MsgTarget::Chan(target) | MsgTarget::User(target) =>
                        self.get_file(target)?
                };
            }
            _ => {}
        }

        Ok(())
    }

    pub fn log_outgoing_msg(&mut self, target: &str, msg: &str, is_action: bool) -> Result<()> {
        // TODO
        Ok(())
    }

    pub fn log_outgoing_raw_msg(&mut self, msg: &str) -> Result<()> {
        // TODO
        Ok(())
    }
}
