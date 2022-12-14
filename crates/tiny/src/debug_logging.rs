//! This module provides a logger (as in `log` and `env_logger` crates, rather than
//! `libtiny_logger`) implementation for logging to a file rather than stdout/stderr (which is what
//! `env_logger` provides).
//!
//! Some notes regarding implementation:
//!
//! - All IO errors ignored. Once initialized the logger never panics.
//! - TINY_LOG is the env variable used for setting filters.
//! - Filter syntax is unchanged (same as `env_logger` syntax).
//! - Log file is created when logging for the first time.

use env_logger::filter::{self, Filter};
use log::{Log, Record};

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::mem::replace;
use std::path::PathBuf;
use std::sync::Mutex;

pub(crate) fn init(path: PathBuf) {
    let filter = filter::Builder::from_env("TINY_LOG").build();
    let sink = Mutex::new(LazyFile::new(path));

    log::set_max_level(filter.filter());
    log::set_boxed_logger(Box::new(Logger { sink, filter })).unwrap();
}

struct Logger {
    sink: Mutex<LazyFile>,
    filter: Filter,
}

impl Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        self.filter.enabled(metadata)
    }

    fn log(&self, record: &Record) {
        if !self.filter.matches(record) {
            return;
        }

        self.sink.lock().unwrap().with_file(|file| {
            let _ = writeln!(
                file,
                "[{}] {} [{}:{}] {}",
                time::strftime("%F %T", &time::now()).unwrap(),
                record.level(),
                record.file().unwrap_or_default(),
                record.line().unwrap_or_default(),
                record.args()
            );
        });
    }

    fn flush(&self) {}
}

enum LazyFile {
    NotOpen(PathBuf),
    Open(File),
    Error,
}

impl LazyFile {
    fn new(path: PathBuf) -> Self {
        LazyFile::NotOpen(path)
    }

    fn with_file<F>(&mut self, f: F)
    where
        F: Fn(&mut File),
    {
        let mut file = match replace(self, LazyFile::Error) {
            LazyFile::NotOpen(path) => {
                match OpenOptions::new().create(true).append(true).open(path) {
                    Ok(mut file) => {
                        // Same format used in libtiny_logger
                        let _ = writeln!(
                            file,
                            "\n*** Logging started at {}\n",
                            time::strftime("%Y-%m-%d %H:%M:%S", &time::now()).unwrap()
                        );
                        file
                    }
                    Err(_) => {
                        return;
                    }
                }
            }
            LazyFile::Open(file) => file,
            LazyFile::Error => {
                return;
            }
        };

        f(&mut file);
        *self = LazyFile::Open(file);
    }
}
