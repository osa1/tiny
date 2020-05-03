use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::fs::{File, OpenOptions};
use std::io;
use std::io::Result;
use std::io::Write;
use std::path::PathBuf;
use std::rc::Rc;
use time::Tm;

use libtiny_ui::*;

#[macro_use]
extern crate log;

#[derive(Clone)]
pub struct Logger {
    inner: Rc<RefCell<LoggerInner>>,
}

impl Logger {
    pub fn new(log_dir: PathBuf, report_err: Box<dyn Fn(String)>) -> Result<Logger> {
        Ok(Logger {
            inner: Rc::new(RefCell::new(LoggerInner::new(log_dir, report_err)?)),
        })
    }
}

macro_rules! delegate {
    ( $name:ident ( $( $x:ident: $t:ty, )* ) ) => {
        fn $name(&self, $($x: $t,)*) {
            self.inner.borrow_mut().$name( $( $x, )* )
        }
    }
}

impl UI for Logger {
    fn draw(&self) {}
    delegate!(new_server_tab(serv: &str,));
    delegate!(close_server_tab(serv: &str,));
    delegate!(new_chan_tab(serv: &str, chan: &str,));
    delegate!(close_chan_tab(serv: &str, chan: &str,));
    delegate!(close_user_tab(serv: &str, nick: &str,));
    delegate!(add_client_msg(msg: &str, target: &MsgTarget,));
    delegate!(add_msg(msg: &str, ts: Tm, target: &MsgTarget,));
    delegate!(add_err_msg(msg: &str, ts: Tm, target: &MsgTarget,));
    delegate!(add_client_err_msg(msg: &str, target: &MsgTarget,));
    delegate!(clear_nicks(serv: &str,));
    delegate!(set_nick(serv: &str, nick: &str,));
    delegate!(add_privmsg(
        sender: &str,
        msg: &str,
        ts: Tm,
        target: &MsgTarget,
        highlight: bool,
        is_action: bool,
    ));
    delegate!(add_nick(nick: &str, ts: Option<Tm>, target: &MsgTarget,));
    delegate!(remove_nick(nick: &str, ts: Option<Tm>, target: &MsgTarget,));
    delegate!(rename_nick(
        old_nick: &str,
        new_nick: &str,
        ts: Tm,
        target: &MsgTarget,
    ));
    delegate!(set_topic(topic: &str, ts: Tm, serv: &str, chan: &str,));
    delegate!(set_tab_style(style: TabStyle, target: &MsgTarget,));

    // TODO: Maybe just return true?
    fn user_tab_exists(&self, _serv: &str, _nick: &str) -> bool {
        false
    }
}

struct LoggerInner {
    /// Log file directory
    log_dir: PathBuf,

    /// Maps server names to their fds
    servers: HashMap<String, ServerLogs>,

    /// Callback used when reporting errors
    report_err: Box<dyn Fn(String)>,
}

struct ServerLogs {
    fd: File,
    chans: HashMap<String, File>,
    users: HashMap<String, File>,
}

fn print_header(fd: &mut File) -> Result<()> {
    writeln!(fd)?;
    writeln!(
        fd,
        "*** Logging started at {}",
        time::strftime("%Y-%m-%d %H:%M:%S", &time::now()).unwrap()
    )?;
    writeln!(fd)
}

macro_rules! report_io_err {
    ( $f:expr, $e:expr ) => {
        match $e {
            Err(err) => {
                $f(format!("{:?}", err));
                return;
            }
            Ok(ok) => ok,
        }
    };
}

impl LoggerInner {
    fn new(log_dir: PathBuf, report_err: Box<dyn Fn(String)>) -> Result<LoggerInner> {
        if let Err(err) = fs::create_dir(&log_dir) {
            if err.kind() != io::ErrorKind::AlreadyExists {
                return Err(err);
            }
        }

        Ok(LoggerInner {
            log_dir,
            servers: HashMap::new(),
            report_err,
        })
    }

    fn new_server_tab(&mut self, serv: &str) {
        if self.servers.contains_key(serv) {
            return;
        }

        let mut path = self.log_dir.clone();
        path.push(&format!("{}.txt", serv));
        debug!("Trying to open log file: {:?}", path);
        let mut fd = report_io_err!(
            self.report_err,
            OpenOptions::new().create(true).append(true).open(path)
        );
        report_io_err!(self.report_err, print_header(&mut fd));

        self.servers.insert(
            serv.to_string(),
            ServerLogs {
                fd,
                chans: HashMap::new(),
                users: HashMap::new(),
            },
        );
    }

    fn close_server_tab(&mut self, serv: &str) {
        self.servers.remove(serv);
    }

    fn new_chan_tab(&mut self, serv: &str, chan: &str) {
        if !self.servers.contains_key(serv) {
            (self.report_err)(format!("Logger::new_chan_tab: can't find server: {}", serv));
            return;
        }

        let server = self.servers.get_mut(serv).unwrap();
        let mut path = self.log_dir.clone();
        path.push(&format!("{}_{}.txt", serv, chan));
        debug!("Trying to open log file: {:?}", path);
        let mut fd = report_io_err!(
            self.report_err,
            OpenOptions::new().create(true).append(true).open(path)
        );
        report_io_err!(self.report_err, print_header(&mut fd));
        server.chans.insert(chan.to_string(), fd);
    }

    fn close_chan_tab(&mut self, serv: &str, chan: &str) {
        if !self.servers.contains_key(serv) {
            (self.report_err)(format!(
                "Logger::close_chan_tab: can't find server: {}",
                serv
            ));
            return;
        }

        let server = self.servers.get_mut(serv).unwrap();
        server.chans.remove(chan);
    }

    // TODO: Where's new_user_tab?

    fn close_user_tab(&mut self, serv: &str, nick: &str) {
        if !self.servers.contains_key(serv) {
            (self.report_err)(format!(
                "Logger::close_user_tab: can't find server: {}",
                serv
            ));
            return;
        }

        let server = self.servers.get_mut(serv).unwrap();
        server.users.remove(nick);
    }

    fn add_client_msg(&mut self, msg: &str, target: &MsgTarget) {
        let now = now();
        self.apply_to_target(target, move |fd: &mut File| {
            // TODO: Report errors?
            let _ = writeln!(fd, "[{}] [client] {}", now, msg);
        });
    }

    fn add_msg(&mut self, msg: &str, ts: Tm, target: &MsgTarget) {
        self.apply_to_target(target, |fd: &mut File| {
            // TODO: Report errors?
            let _ = writeln!(fd, "[{}] {}", strf(&ts), msg);
        });
    }

    fn add_err_msg(&self, _msg: &str, _ts: Tm, _target: &MsgTarget) {
        // IRC error messages are ignored
    }

    fn add_client_err_msg(&self, _msg: &str, _target: &MsgTarget) {
        // Ditto with client error messages
    }

    fn clear_nicks(&self, _serv: &str) {
        // Nothing to do here
    }

    fn set_nick(&self, _serv: &str, _nick: &str) {
        // Ditto
    }

    fn add_privmsg(
        &mut self,
        sender: &str,
        msg: &str,
        ts: Tm,
        target: &MsgTarget,
        _highlight: bool,
        is_action: bool,
    ) {
        self.apply_to_target(target, |fd: &mut File| {
            // TODO: Report errors?
            if is_action {
                let _ = writeln!(fd, "[{}] {} {}", strf(&ts), sender, msg);
            } else {
                let _ = writeln!(fd, "[{}] {}: {}", strf(&ts), sender, msg);
            }
        });
    }

    fn add_nick(&mut self, nick: &str, ts: Option<Tm>, target: &MsgTarget) {
        if let Some(_ts) = ts {
            // This method is only called when a user joins a chan
            self.apply_to_target(target, |fd: &mut File| {
                // TODO: Report errors?
                let _ = writeln!(fd, "[{}] {} joined the channel.", now(), nick);
            });
        }
    }

    fn remove_nick(&mut self, nick: &str, ts: Option<Tm>, target: &MsgTarget) {
        if let Some(_ts) = ts {
            // TODO: Did the user leave a channel or the server? Currently we can't tell.
            self.apply_to_target(target, |fd: &mut File| {
                // TODO: Report errors?
                let _ = writeln!(fd, "[{}] {} left.", now(), nick);
            });
        }
    }

    fn rename_nick(&mut self, old_nick: &str, new_nick: &str, ts: Tm, target: &MsgTarget) {
        self.apply_to_target(target, |fd: &mut File| {
            // TODO: Report errors?
            let _ = writeln!(
                fd,
                "[{}] {} is now known as {}.",
                strf(&ts),
                old_nick,
                new_nick
            );
        });
    }

    fn set_topic(&mut self, topic: &str, ts: Tm, serv: &str, chan: &str) {
        let target = MsgTarget::Chan { serv, chan };
        self.apply_to_target(&target, |fd: &mut File| {
            // TODO: Report errors?
            let _ = writeln!(fd, "[{}] Channel topic: {}.", strf(&ts), topic);
        });
    }

    fn set_tab_style(&self, _: TabStyle, _: &MsgTarget) {
        // Nothing to do here
    }

    fn apply_to_target(&mut self, target: &MsgTarget, f: impl Fn(&mut File)) {
        match *target {
            MsgTarget::Server { serv } => {
                if !self.servers.contains_key(serv) {
                    (self.report_err)(format!("Logger: can't find server: {}", serv));
                    return;
                }
                let ServerLogs { ref mut fd, .. } = self.servers.get_mut(serv).unwrap();
                f(fd);
            }
            MsgTarget::Chan { serv, chan } => {
                if !self.servers.contains_key(serv) {
                    (self.report_err)(format!("Logger: can't find server: {}", serv));
                    return;
                }
                let ServerLogs { ref mut chans, .. } = self.servers.get_mut(serv).unwrap();
                if !chans.contains_key(chan) {
                    (self.report_err)(format!(
                        "Logger: can't find chan {} in server {}",
                        chan, serv
                    ));
                    return;
                }
                let fd = chans.get_mut(chan).unwrap();
                f(fd);
            }
            MsgTarget::User { serv, nick } => {
                if !self.servers.contains_key(serv) {
                    (self.report_err)(format!("Logger: can't find server: {}", serv));
                    return;
                }
                let ServerLogs { ref mut users, .. } = self.servers.get_mut(serv).unwrap();
                if !users.contains_key(nick) {
                    // We don't have a `new_user_tab` trait method so user log files are created
                    // here
                    let mut path = self.log_dir.clone();
                    path.push(&format!("{}_{}.txt", serv, nick));
                    debug!("Trying to open log file: {:?}", path);
                    let mut fd = report_io_err!(
                        self.report_err,
                        OpenOptions::new().create(true).append(true).open(path)
                    );
                    report_io_err!(self.report_err, print_header(&mut fd));
                    users.insert(nick.to_owned(), fd);
                }
                let fd = users.get_mut(nick).unwrap();
                f(fd);
            }
            MsgTarget::AllServTabs { serv } => {
                if !self.servers.contains_key(serv) {
                    (self.report_err)(format!("Logger: can't find server: {}", serv));
                    return;
                }
                let ServerLogs {
                    ref mut fd,
                    ref mut chans,
                    ref mut users,
                    ..
                } = self.servers.get_mut(serv).unwrap();
                f(fd);
                for (_, fd) in chans.iter_mut() {
                    f(fd);
                }
                for (_, fd) in users.iter_mut() {
                    f(fd);
                }
            }
            MsgTarget::CurrentTab => {
                // Probably a cmd error; these are ignored
            }
        }
    }
}

fn now() -> String {
    time::strftime("%H:%M:%S", &time::now()).unwrap()
}

fn strf(tm: &Tm) -> String {
    time::strftime("%H:%M:%S", tm).unwrap()
}
