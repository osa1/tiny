use std::fs::File;
use std::fs::OpenOptions;
use std::fs;
use std::io::Write;
use std::fmt;
use std::path::PathBuf;
use time;

// Using Vec in `fds` for custom lookup functions that don't allocate. This is probably also
// possible with HashMap if the internals are exposed. Alternatively I guess we could implement a
// `Borrow` instance for `LogDest`.

pub struct Logger {
    log_dir: PathBuf,
    fds: Vec<(LogDest, File)>,
    debug_fd: File,
}

pub struct LogFile<'a> {
    fd: &'a File,
}

impl<'a> LogFile<'a> {
    pub fn write_line(&mut self, args: fmt::Arguments) {
        let now = time::now();
        write!(self.fd, "[{}] ", now.rfc822()).unwrap();
        self.fd.write_fmt(args).unwrap();
        writeln!(self.fd, "").unwrap();
    }
}

/// Log message destination
enum LogDest {
    // Server(String),
    Chan { serv: String, chan: String },
    /// For logging raw messages
    ServerRaw(String),
}

fn init_log_file(file: &mut Write) {
    let now = time::now();
    writeln!(file, "\nLogs started on {}\n", now.rfc822()).unwrap();
}

impl Logger {
    pub fn new(log_dir: PathBuf) -> Logger {
        let _ = fs::create_dir(&log_dir);

        let debug_logs = {
            let mut log_dir = log_dir.clone();
            log_dir.push("debug.log");
            let mut file = OpenOptions::new()
                .append(true)
                .create(true)
                .open(log_dir)
                .unwrap();
            init_log_file(&mut file);
            file
        };

        Logger {
            log_dir: log_dir,
            fds: vec![],
            debug_fd: debug_logs,
        }
    }

    // Stupid code below because of
    // https://users.rust-lang.org/t/weird-borrow-checker-error-for-loop-keeps-references-after-its-scope-ends/10929

    /*
    pub fn get_serv_logs(&mut self, serv_: &str) -> LogFile {
        let pos = self.fds.iter().position(|&(ref dest, _)| {
            if let &LogDest::Server(ref serv) = dest {
                serv == serv_
            } else {
                false
            }
        });

        match pos {
            Some(idx) => LogFile { fd: &mut self.fds[idx].1 },
            None => {
                let mut log_path = self.log_dir.clone();
                log_path.push(format!("{}.log", serv_));
                let mut file = OpenOptions::new().append(true).create(true).open(log_path).unwrap();
                init_log_file(&mut file);
                let idx = self.fds.len();
                self.fds.push((LogDest::Server(serv_.to_owned()), file));
                LogFile { fd: &mut self.fds[idx].1 }
            }
        }
    }
*/

    pub fn get_chan_logs(&mut self, serv_: &str, chan_: &str) -> LogFile {
        let pos = self.fds.iter().position(|&(ref dest, _)| {
            if let LogDest::Chan { ref serv, ref chan } = *dest {
                serv == serv_ && chan == chan_
            } else {
                false
            }
        });

        match pos {
            Some(idx) =>
                LogFile {
                    fd: &mut self.fds[idx].1,
                },
            None => {
                let mut log_path = self.log_dir.clone();
                log_path.push(format!("{}_{}.log", serv_, chan_));
                let mut file = OpenOptions::new()
                    .append(true)
                    .create(true)
                    .open(log_path)
                    .unwrap();
                init_log_file(&mut file);
                let idx = self.fds.len();
                self.fds.push((
                    LogDest::Chan {
                        serv: serv_.to_owned(),
                        chan: chan_.to_owned(),
                    },
                    file,
                ));
                LogFile {
                    fd: &mut self.fds[idx].1,
                }
            }
        }
    }

    pub fn get_raw_serv_logs(&mut self, serv_: &str) -> LogFile {
        let pos = self.fds.iter().position(|&(ref dest, _)| {
            if let LogDest::ServerRaw(ref serv) = *dest {
                serv == serv_
            } else {
                false
            }
        });

        match pos {
            Some(idx) =>
                LogFile {
                    fd: &mut self.fds[idx].1,
                },
            None => {
                let mut log_path = self.log_dir.clone();
                log_path.push(format!("{}_raw.log", serv_));
                let mut file = OpenOptions::new()
                    .append(true)
                    .create(true)
                    .open(log_path)
                    .unwrap();
                init_log_file(&mut file);
                let idx = self.fds.len();
                self.fds.push((LogDest::ServerRaw(serv_.to_owned()), file));
                LogFile {
                    fd: &mut self.fds[idx].1,
                }
            }
        }
    }

    pub fn get_debug_logs(&mut self) -> LogFile {
        LogFile {
            fd: &mut self.debug_fd,
        }
    }
}
