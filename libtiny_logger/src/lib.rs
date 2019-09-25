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

    pub fn log_incoming_msg(&mut self, msg: &wire::Msg) -> Result<()> {
        // TODO
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
