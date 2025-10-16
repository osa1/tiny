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

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::mem::replace;
use std::path::PathBuf;
use std::sync::Mutex;
pub(crate) fn init(path: PathBuf) {

//    let filter = env_logger::Builder::from_env("TINY_LOG").build();
    let sink = Mutex::new(LazyFile::new(path));

    log::set_max_level(log::LevelFilter::max());
    log::set_boxed_logger(Box::new(Logger { sink })).unwrap();
}

struct Logger {
    sink: std::sync::Mutex<LazyFile>,
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        // Aceitar todos os logs
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &log::Record) {
        // Verificar nível do log
        if !self.enabled(record.metadata()) {
            return;
        }

        if let Ok(mut file_guard) = self.sink.lock() {
            if let LazyFile::Open(ref mut file) = *file_guard {
                use std::io::Write;
                let _ = writeln!(
                    file,
                    "[{}] {} [{}:{}] {}",

                    time::OffsetDateTime::now_utc().format(&time::format_description::parse("[year]-[month]-[day] [hour]:[minute]:[second]").unwrap()).unwrap(),
                    record.level(),
                    record.file().unwrap_or_default(),
                    record.line().unwrap_or_default(),
                    record.args()
                );
            }
        }
    }

    fn flush(&self) {
        if let Ok(mut guard) = self.sink.lock() {
            if let LazyFile::Open(ref mut file) = *guard {
                let _ = file.sync_all();
            }
        }
    }
}

#[allow(dead_code)]
enum LazyFile {
    NotOpen(()),
    Open(File),
    Error,
}

impl LazyFile {
    fn new(_path: PathBuf) -> Self {
        LazyFile::NotOpen(())
    }
#[allow(dead_code)]
    fn with_file<F>(&mut self, f: F)
    where
        F: Fn(&mut File),
    {
        let mut file = match replace(self, LazyFile::Error) {
            LazyFile::NotOpen(()) => {
                match OpenOptions::new().create(true).append(true).open("/tmp/tiny.log") {
                    Ok(mut file) => {
                        // Same format used in libtiny_logger
                        let _ = writeln!(
                            file,
                            "\n*** Logging started at {}\n",
                            time::OffsetDateTime::now_utc().format(&time::format_description::parse("[year]-[month]-[day] [hour]:[minute]:[second]").unwrap()).unwrap()
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
