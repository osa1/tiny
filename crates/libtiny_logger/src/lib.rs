use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::fs::{File, OpenOptions};
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use time::Tm;

use libtiny_common::{ChanName, ChanNameRef, MsgTarget};

#[macro_use]
extern crate log;

#[derive(Clone)]
pub struct Logger {
    inner: Rc<RefCell<LoggerInner>>,
}

#[derive(Debug)]
pub enum LoggerInitError {
    CouldNotCreateDir { dir_path: PathBuf, err: io::Error },
}

impl Logger {
    pub fn new(
        log_dir: PathBuf,
        report_err: Box<dyn Fn(String)>,
    ) -> Result<Logger, LoggerInitError> {
        Ok(Logger {
            inner: Rc::new(RefCell::new(LoggerInner::new(log_dir, report_err)?)),
        })
    }
}

macro_rules! delegate {
    ( $name:ident ( $( $x:ident: $t:ty, )* ) ) => {
        pub fn $name(&self, $($x: $t,)*) {
            self.inner.borrow_mut().$name( $( $x, )* )
        }
    }
}

impl Logger {
    delegate!(new_server_tab(serv: &str,));
    delegate!(close_server_tab(serv: &str,));
    delegate!(new_chan_tab(serv: &str, chan: &ChanNameRef,));
    delegate!(close_chan_tab(serv: &str, chan: &ChanNameRef,));
    delegate!(close_user_tab(serv: &str, nick: &str,));
    delegate!(add_client_msg(msg: &str, target: &MsgTarget,));
    delegate!(add_msg(msg: &str, ts: Tm, target: &MsgTarget,));
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
    delegate!(set_topic(
        topic: &str,
        ts: Tm,
        serv: &str,
        chan: &ChanNameRef,
    ));
}

struct LoggerInner {
    /// Log file directory
    log_dir: PathBuf,

    /// Maps server names to their fds
    servers: HashMap<String, ServerLogs>,

    /// Callback used when reporting errors
    report_err: Box<dyn Fn(String)>,
}

impl Drop for LoggerInner {
    fn drop(&mut self) {
        for (_, server) in self.servers.drain() {
            close_server_tabs(server, &self.report_err);
        }
    }
}

struct ServerLogs {
    fd: File,
    chans: HashMap<ChanName, File>,
    users: HashMap<String, File>,
}

fn print_header(fd: &mut File) -> io::Result<()> {
    writeln!(fd)?;
    writeln!(
        fd,
        "*** Logging started at {}",
        time::strftime("%Y-%m-%d %H:%M:%S", &time::now()).unwrap()
    )?;
    writeln!(fd)
}

fn print_footer(fd: &mut File) -> io::Result<()> {
    writeln!(fd)?;
    writeln!(
        fd,
        "*** Logging ended at {}",
        time::strftime("%Y-%m-%d %H:%M:%S", &time::now()).unwrap()
    )?;
    writeln!(fd)
}

macro_rules! report_io_err {
    ( $f:expr, $e:expr ) => {
        match $e {
            Err(err) => {
                info!("{:?}", err);
                $f(format!("{:?}", err));
                return;
            }
            Ok(ok) => ok,
        }
    };
}

// '/' is valid in channel names but we can't use it in file names, so we replace it with '-'.
// According to RFC 2812 nick names can't contain '/', but we still use this in nicks just to be
// safe. Other special characters mentioned in the RFC ("[]\`^{|}") can be used in file names so we
// don't replace those.
fn replace_forward_slash(path: &str) -> String {
    path.replace('/', "-")
}

fn try_open_log_file(path: &Path, report_err: &dyn Fn(String)) -> Option<File> {
    match OpenOptions::new().create(true).append(true).open(path) {
        Ok(fd) => Some(fd),
        Err(err) => {
            report_err(format!("Couldn't open file {:?}: {}", path, err));
            None
        }
    }
}

fn close_server_tabs(server: ServerLogs, report_err: &dyn Fn(String)) {
    let ServerLogs {
        mut fd,
        chans,
        users,
    } = server;
    report_io_err!(report_err, print_footer(&mut fd));
    for (_, mut fd) in chans.into_iter() {
        report_io_err!(report_err, print_footer(&mut fd));
    }
    for (_, mut fd) in users.into_iter() {
        report_io_err!(report_err, print_footer(&mut fd));
    }
}

impl LoggerInner {
    fn new(
        log_dir: PathBuf,
        report_err: Box<dyn Fn(String)>,
    ) -> Result<LoggerInner, LoggerInitError> {
        if let Err(err) = fs::create_dir_all(&log_dir) {
            if err.kind() != io::ErrorKind::AlreadyExists {
                return Err(LoggerInitError::CouldNotCreateDir {
                    dir_path: log_dir,
                    err,
                });
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
        if let Some(mut fd) = try_open_log_file(&path, &*self.report_err) {
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
    }

    fn close_server_tab(&mut self, serv: &str) {
        match self.servers.remove(serv) {
            None => {
                info!("close_server_tab: can't find server: {:?}", serv);
            }
            Some(server) => {
                close_server_tabs(server, &self.report_err);
            }
        }
    }

    fn new_chan_tab(&mut self, serv: &str, chan: &ChanNameRef) {
        match self.servers.get_mut(serv) {
            None => {
                info!("new_chan_tab: can't find server: {:?}", serv);
            }
            Some(server) => {
                let chan_name_normalized = chan.normalized();
                if server
                    .chans
                    .contains_key(ChanNameRef::new(&chan_name_normalized))
                {
                    return;
                }

                let mut path = self.log_dir.clone();
                path.push(&format!(
                    "{}_{}.txt",
                    serv,
                    replace_forward_slash(&chan_name_normalized)
                ));
                if let Some(mut fd) = try_open_log_file(&path, &*self.report_err) {
                    report_io_err!(self.report_err, print_header(&mut fd));
                    server.chans.insert(ChanName::new(chan_name_normalized), fd);
                }
            }
        }
    }

    fn close_chan_tab(&mut self, serv: &str, chan: &ChanNameRef) {
        match self.servers.get_mut(serv) {
            None => {
                info!("close_chan_tab: can't find server: {:?}", serv);
            }
            Some(server) => match server.chans.remove(chan) {
                None => {
                    info!(
                        "close_chan_tab: can't find chan {:?} in server {:?}",
                        chan.display(),
                        serv
                    );
                }
                Some(mut fd) => {
                    report_io_err!(self.report_err, print_footer(&mut fd));
                }
            },
        }
    }

    fn close_user_tab(&mut self, serv: &str, nick: &str) {
        match self.servers.get_mut(serv) {
            None => {
                info!("close_user_tab: can't find server: {:?}", serv);
            }
            Some(server) => match server.users.remove(nick) {
                None => {
                    info!(
                        "close_user_tab: can't find user {:?} in server {:?}",
                        nick, serv
                    );
                }
                Some(mut fd) => {
                    report_io_err!(self.report_err, print_footer(&mut fd));
                }
            },
        }
    }

    fn add_client_msg(&mut self, msg: &str, target: &MsgTarget) {
        let now = now();
        self.apply_to_target(target, |fd: &mut File, report_err: &dyn Fn(String)| {
            report_io_err!(report_err, writeln!(fd, "[{}] [client] {}", now, msg));
        });
    }

    fn add_msg(&mut self, msg: &str, ts: Tm, target: &MsgTarget) {
        self.apply_to_target(target, |fd: &mut File, report_err: &dyn Fn(String)| {
            report_io_err!(report_err, writeln!(fd, "[{}] {}", strf(&ts), msg));
        });
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
        self.apply_to_target(target, |fd: &mut File, report_err: &dyn Fn(String)| {
            let io_ret = if is_action {
                writeln!(fd, "[{}] {} {}", strf(&ts), sender, msg)
            } else {
                writeln!(fd, "[{}] {}: {}", strf(&ts), sender, msg)
            };
            report_io_err!(report_err, io_ret);
        });
    }

    fn add_nick(&mut self, nick: &str, ts: Option<Tm>, target: &MsgTarget) {
        if let Some(_ts) = ts {
            // This method is only called when a user joins a chan
            self.apply_to_target(target, |fd: &mut File, report_err: &dyn Fn(String)| {
                report_io_err!(
                    report_err,
                    writeln!(fd, "[{}] {} joined the channel.", now(), nick)
                );
            });
        }
    }

    fn remove_nick(&mut self, nick: &str, ts: Option<Tm>, target: &MsgTarget) {
        if let Some(_ts) = ts {
            // TODO: Did the user leave a channel or the server? Currently we can't tell.
            self.apply_to_target(target, |fd: &mut File, report_err: &dyn Fn(String)| {
                report_io_err!(report_err, writeln!(fd, "[{}] {} left.", now(), nick));
            });
        }
    }

    fn rename_nick(&mut self, old_nick: &str, new_nick: &str, ts: Tm, target: &MsgTarget) {
        self.apply_to_target(target, |fd: &mut File, report_err: &dyn Fn(String)| {
            report_io_err!(
                report_err,
                writeln!(
                    fd,
                    "[{}] {} is now known as {}.",
                    strf(&ts),
                    old_nick,
                    new_nick
                )
            );
        });
    }

    fn set_topic(&mut self, topic: &str, ts: Tm, serv: &str, chan: &ChanNameRef) {
        let target = MsgTarget::Chan { serv, chan };
        self.apply_to_target(&target, |fd: &mut File, report_err: &dyn Fn(String)| {
            report_io_err!(
                report_err,
                writeln!(fd, "[{}] Channel topic: {}.", strf(&ts), topic)
            );
        });
    }

    fn apply_to_target(&mut self, target: &MsgTarget, f: impl Fn(&mut File, &dyn Fn(String))) {
        match *target {
            MsgTarget::Server { serv } => match self.servers.get_mut(serv) {
                None => {
                    info!("Can't find server: {:?}", serv);
                }
                Some(ServerLogs { ref mut fd, .. }) => {
                    f(fd, &*self.report_err);
                }
            },
            MsgTarget::Chan { serv, chan } => match self.servers.get_mut(serv) {
                None => {
                    info!("Can't find server: {:?}", serv);
                }
                Some(ServerLogs { ref mut chans, .. }) => match chans.get_mut(chan) {
                    None => {
                        // Create a file for the channel. FIXME Code copied from new_chan_tab:
                        // can't reuse it because of borrowchk issues.
                        let mut path = self.log_dir.clone();
                        let chan_name_normalized = chan.normalized();
                        path.push(&format!(
                            "{}_{}.txt",
                            serv,
                            replace_forward_slash(&chan_name_normalized)
                        ));
                        if let Some(mut fd) = try_open_log_file(&path, &*self.report_err) {
                            report_io_err!(self.report_err, print_header(&mut fd));
                            f(&mut fd, &*self.report_err);
                            chans.insert(ChanName::new(chan_name_normalized), fd);
                        }
                    }
                    Some(fd) => {
                        f(fd, &*self.report_err);
                    }
                },
            },
            MsgTarget::User { serv, nick } => {
                match self.servers.get_mut(serv) {
                    None => {
                        info!("Can't find server: {:?}", serv);
                    }
                    Some(ServerLogs { ref mut users, .. }) => {
                        match users.get_mut(nick) {
                            Some(fd) => {
                                f(fd, &*self.report_err);
                            }
                            None => {
                                // We don't have a `new_user_tab` trait method so user log files
                                // are created here
                                let mut path = self.log_dir.clone();
                                path.push(&format!("{}_{}.txt", serv, replace_forward_slash(nick)));
                                if let Some(mut fd) = try_open_log_file(&path, &*self.report_err) {
                                    report_io_err!(self.report_err, print_header(&mut fd));
                                    f(&mut fd, &*self.report_err);
                                    users.insert(nick.to_owned(), fd);
                                }
                            }
                        }
                    }
                }
            }
            MsgTarget::AllServTabs { serv } => match self.servers.get_mut(serv) {
                None => {
                    info!("Can't find server: {:?}", serv);
                }
                Some(ServerLogs {
                    ref mut fd,
                    ref mut chans,
                    ref mut users,
                    ..
                }) => {
                    f(fd, &*self.report_err);
                    for (_, fd) in chans.iter_mut() {
                        f(fd, &*self.report_err);
                    }
                    for (_, fd) in users.iter_mut() {
                        f(fd, &*self.report_err);
                    }
                }
            },
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
