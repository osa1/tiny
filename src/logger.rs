use std::fs::File;
use std::fs::OpenOptions;
use std::fs;
use std::path::PathBuf;

pub struct Logger {
    log_path: PathBuf,
    // server, optional channel for logging a channel, file handle
    fds: Vec<(String, Option<String>, File)>,
}

impl Logger {

    pub fn new(log_dir: PathBuf) -> Logger {
        let _ = fs::create_dir(&log_dir);
        Logger {
            log_path: log_dir,
            fds: vec![],
        }
    }

    // Stupid code below  because of
    // https://users.rust-lang.org/t/weird-borrow-checker-error-for-loop-keeps-references-after-its-scope-ends/10929

    pub fn get_chan_file(&mut self, serv: &str, chan: &str) -> &mut File {
        let pos = self.fds.iter().position(|&(ref serv_, ref chan_, _)| {
            serv_ == serv && chan_.as_ref().map(|s| s.as_str()) == Some(chan)
        });

        match pos {
            Some(idx) => &mut self.fds[idx].2,
            None => {
                let mut log_path = self.log_path.clone();
                log_path.push(format!("{}_{}.log", serv, chan));
                let file = OpenOptions::new().append(true).create(true).open(log_path).unwrap();
                let idx = self.fds.len();
                self.fds.push((serv.to_owned(), Some(chan.to_owned()), file));
                &mut self.fds[idx].2
            }
        }
    }

    pub fn get_serv_file(&mut self, serv: &str) -> &mut File {
        let pos = self.fds.iter().position(|&(ref serv_, ref chan_, _)| {
            serv_ == serv && chan_ == &None
        });

        match pos {
            Some(idx) => &mut self.fds[idx].2,
            None => {
                let mut log_path = self.log_path.clone();
                log_path.push(format!("{}.log", serv));
                let file = OpenOptions::new().append(true).create(true).open(log_path).unwrap();
                let idx = self.fds.len();
                self.fds.push((serv.to_owned(), None, file));
                &mut self.fds[idx].2
            }
        }
    }
}
