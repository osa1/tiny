use crate::config;
use crate::utils;
use libtiny::{Client, IrcClient, ServerInfo};
use libtiny_tui::{MsgSource, MsgTarget, Notifier, TUI};
use std::cell::RefCell;
use std::error::Error;
use std::path::PathBuf;
use std::rc::Rc;

type TUIRef = Rc<RefCell<TUI>>;

pub(crate) struct Cmd {
    /// Command name. E.g. if this is `"cmd"`, `/cmd ...` will call this command.
    pub(crate) name: &'static str,

    // Command help message. Shown in `/help`.
    // pub(crate) help: &'static str,
    /// Command function.
    pub(crate) cmd_fn:
        for<'a, 'b> fn(&str, &PathBuf, &config::Defaults, TUIRef, &mut Vec<IrcClient>, MsgSource),
}

////////////////////////////////////////////////////////////////////////////////////////////////////

pub(crate) enum ParseCmdResult<'a> {
    /// Command name parsing successful
    Ok {
        cmd: &'static Cmd,

        /// Rest of the command after extracting command name
        rest: &'a str,
    },

    // Command name is ambiguous, here are possible values
    // Ambiguous(Vec<&'static str>),
    /// Unknown command
    Unknown,
}

pub(crate) fn parse_cmd(cmd: &str) -> ParseCmdResult {
    match cmd.split_whitespace().next() {
        None => ParseCmdResult::Unknown,
        Some(cmd_name) => {
            let mut ws_idxs = utils::split_whitespace_indices(cmd);
            ws_idxs.next(); // cmd_name
            let rest = {
                match ws_idxs.next() {
                    None => "",
                    Some(rest_idx) => &cmd[rest_idx..],
                }
            };
            // let mut possibilities: Vec<&'static Cmd> = vec![];
            for cmd in &CMDS {
                if cmd_name == cmd.name {
                    // exact match, return
                    return ParseCmdResult::Ok { cmd, rest };
                }
            }
            ParseCmdResult::Unknown
            // match possibilities.len() {
            //     0 =>
            //         ParseCmdResult::Unknown,
            //     1 =>
            //         ParseCmdResult::Ok {
            //             cmd: possibilities[0],
            //             rest,
            //         },
            //     _ =>
            //         ParseCmdResult::Ambiguous(possibilities.into_iter().map(|cmd| cmd.name).collect()),
            // }
        }
    }
}

fn find_client_idx(clients: &[IrcClient], serv_name: &str) -> Option<usize> {
    for (client_idx, client) in clients.iter().enumerate() {
        if client.get_serv_name() == serv_name {
            return Some(client_idx);
        }
    }
    None
}

fn find_client<'a>(clients: &'a mut Vec<IrcClient>, serv_name: &str) -> Option<&'a mut IrcClient> {
    match find_client_idx(clients, serv_name) {
        None => None,
        Some(idx) => Some(&mut clients[idx]),
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

static CMDS: [&Cmd; 13] = [
    &AWAY_CMD,
    &CLEAR_CMD,
    &CLOSE_CMD,
    &CONNECT_CMD,
    &IGNORE_CMD,
    &JOIN_CMD,
    &ME_CMD,
    &MSG_CMD,
    &NAMES_CMD,
    &NICK_CMD,
    &NOTIFY_CMD,
    &RELOAD_CMD,
    &SWITCH_CMD,
];

////////////////////////////////////////////////////////////////////////////////////////////////////

static AWAY_CMD: Cmd = Cmd {
    name: "away",
    cmd_fn: away,
};

fn away(
    args: &str,
    _: &PathBuf,
    _: &config::Defaults,
    _: TUIRef,
    clients: &mut Vec<IrcClient>,
    src: MsgSource,
) {
    let msg = if args.is_empty() { None } else { Some(args) };
    if let Some(client) = find_client(clients, src.serv_name()) {
        client.away(msg);
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

static CLEAR_CMD: Cmd = Cmd {
    name: "clear",
    cmd_fn: clear,
};

fn clear(
    _: &str,
    _: &PathBuf,
    _: &config::Defaults,
    tui: TUIRef,
    _: &mut Vec<IrcClient>,
    src: MsgSource,
) {
    tui.borrow_mut().clear(&src.to_target());
}

////////////////////////////////////////////////////////////////////////////////////////////////////

static CLOSE_CMD: Cmd = Cmd {
    name: "close",
    cmd_fn: close,
};

fn close(
    _: &str,
    _: &PathBuf,
    _: &config::Defaults,
    tui: TUIRef,
    clients: &mut Vec<IrcClient>,
    src: MsgSource,
) {
    let mut tui = tui.borrow_mut();
    match src {
        MsgSource::Serv { ref serv_name } if serv_name == "mentions" => {
            // ignore
        }
        MsgSource::Serv { serv_name } => {
            tui.close_server_tab(&serv_name);
            let client_idx = find_client_idx(&clients, &serv_name).unwrap();
            // TODO: this probably won't close the connection?
            let mut client = clients.remove(client_idx);
            client.quit(None);
        }
        MsgSource::Chan {
            serv_name,
            chan_name,
        } => {
            tui.close_chan_tab(&serv_name, &chan_name);
            // tiny.part(&serv_name, &chan_name); FIXME
        }
        MsgSource::User { serv_name, nick } => {
            tui.close_user_tab(&serv_name, &nick);
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

static CONNECT_CMD: Cmd = Cmd {
    name: "connect",
    cmd_fn: connect,
};

fn connect(
    args: &str,
    _: &PathBuf,
    defaults: &config::Defaults,
    tui: TUIRef,
    clients: &mut Vec<IrcClient>,
    src: MsgSource,
) {
    let words: Vec<&str> = args.split_whitespace().collect();

    match words.len() {
        0 => reconnect(&mut *tui.borrow_mut(), clients, src),
        1 => connect_(words[0], None, defaults, tui, clients),
        2 => connect_(words[0], Some(words[1]), defaults, tui, clients),
        _ =>
        // wat
        {
            tui.borrow_mut().add_client_err_msg(
                "/connect usage: /connect <host>:<port> or /connect (to reconnect)",
                &MsgTarget::CurrentTab,
            )
        }
    }
}

fn reconnect(tui: &mut TUI, clients: &mut Vec<IrcClient>, src: MsgSource) {
    tui.add_client_msg(
        "Reconnecting...",
        &MsgTarget::AllServTabs {
            serv_name: src.serv_name(),
        },
    );
    match find_client(clients, src.serv_name()) {
        Some(client) => client.reconnect(None),
        None => {
            // tiny.logger
            //     .get_debug_logs()
            //     .write_line(format_args!("Can't reconnect to {}", src.serv_name()));
        }
    }
}

fn connect_(
    serv_addr: &str,
    pass: Option<&str>,
    defaults: &config::Defaults,
    tui_ref: TUIRef,
    clients: &mut Vec<IrcClient>,
) {
    let mut tui = tui_ref.borrow_mut();

    fn split_port(s: &str) -> Option<(&str, &str)> {
        s.find(':').map(|split| (&s[0..split], &s[split + 1..]))
    }

    // parse host name and port
    let (serv_name, serv_port) = {
        match split_port(serv_addr) {
            None => {
                return tui
                    .add_client_err_msg("connect: Need a <host>:<port>", &MsgTarget::CurrentTab);
            }
            Some((serv_name, serv_port)) => match serv_port.parse::<u16>() {
                Err(err) => {
                    return tui.add_client_err_msg(
                        &format!("connect: Can't parse port {}: {}", serv_port, err),
                        &MsgTarget::CurrentTab,
                    );
                }
                Ok(serv_port) => (serv_name, serv_port),
            },
        }
    };

    // if we already connected to this server reconnect using new port
    if let Some(client) = find_client(clients, serv_name) {
        tui.add_client_msg("Connecting...", &MsgTarget::AllServTabs { serv_name });
        client.reconnect(Some(serv_port));
        return;
    }

    // otherwise create a new connection
    // can't move the rest to an else branch because of borrowchk

    // otherwise create a new Conn, tab etc.
    tui.new_server_tab(serv_name);
    let msg_target = MsgTarget::Server { serv_name };
    tui.add_client_msg("Connecting...", &msg_target);

    let (client, rcv_ev) = IrcClient::new(
        ServerInfo {
            addr: serv_name.to_owned(),
            port: serv_port,
            tls: defaults.tls,
            hostname: defaults.hostname.clone(),
            realname: defaults.realname.clone(),
            pass: pass.map(str::to_owned),
            nicks: defaults.nicks.clone(),
            auto_join: defaults.join.clone(),
            nickserv_ident: None,
            sasl_auth: None,
        },
        None,
    );

    // Spawn TUI task
    let tui_clone = tui_ref.clone();
    let client_clone = client.clone();
    tokio::runtime::current_thread::spawn(crate::tui_task(rcv_ev, tui_clone, client_clone));

    clients.push(client);
}

////////////////////////////////////////////////////////////////////////////////////////////////////

static IGNORE_CMD: Cmd = Cmd {
    name: "ignore",
    cmd_fn: ignore,
};

fn ignore(
    _: &str,
    _: &PathBuf,
    _: &config::Defaults,
    tui: TUIRef,
    _: &mut Vec<IrcClient>,
    src: MsgSource,
) {
    match src {
        MsgSource::Serv { serv_name } => {
            tui.borrow_mut().toggle_ignore(&MsgTarget::AllServTabs {
                serv_name: &serv_name,
            });
        }
        MsgSource::Chan {
            serv_name,
            chan_name,
        } => {
            tui.borrow_mut().toggle_ignore(&MsgTarget::Chan {
                serv_name: &serv_name,
                chan_name: &chan_name,
            });
        }
        MsgSource::User { serv_name, nick } => {
            tui.borrow_mut().toggle_ignore(&MsgTarget::User {
                serv_name: &serv_name,
                nick: &nick,
            });
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

static JOIN_CMD: Cmd = Cmd {
    name: "join",
    cmd_fn: join,
};

fn join(
    args: &str,
    _: &PathBuf,
    _: &config::Defaults,
    tui: TUIRef,
    clients: &mut Vec<IrcClient>,
    src: MsgSource,
) {
    let words = args.split_whitespace().collect::<Vec<_>>();
    if words.is_empty() {
        return tui.borrow_mut().add_client_err_msg(
            "/join usage: /join chan1[,chan2...]",
            &MsgTarget::CurrentTab,
        );
    }

    match find_client(clients, src.serv_name()) {
        Some(client) => client.join(&words),
        None => tui.borrow_mut().add_client_err_msg(
            &format!("Can't JOIN: Not connected to server {}", src.serv_name()),
            &MsgTarget::CurrentTab,
        ),
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

static ME_CMD: Cmd = Cmd {
    name: "me",
    cmd_fn: me,
};

fn me(
    args: &str,
    _: &PathBuf,
    _: &config::Defaults,
    tui: TUIRef,
    clients: &mut Vec<IrcClient>,
    src: MsgSource,
) {
    if args.is_empty() {
        return tui
            .borrow_mut()
            .add_client_err_msg("/me usage: /me message", &MsgTarget::CurrentTab);
    }
    crate::send_msg(
        &mut *tui.borrow_mut(),
        clients,
        &src,
        args.to_string(),
        true,
    );
}

////////////////////////////////////////////////////////////////////////////////////////////////////

static MSG_CMD: Cmd = Cmd {
    name: "msg",
    cmd_fn: msg,
};

fn split_msg_args(args: &str) -> Option<(&str, &str)> {
    // Apparently we can't break with a val in a for loop yet so using mut var
    let mut target_msg: Option<(&str, &str)> = None;
    for (i, c) in args.char_indices() {
        if !utils::is_nick_char(c) {
            // This is where we split the message into target and actual message, however if the
            // current char is a whitespace then we don't include it in the message, otherwise most
            // messages would start with a whitespace. See `test_msg_args` below for some examples.
            let target = &args[0..i];
            let i = if c.is_whitespace() { i + 1 } else { i };
            let msg = &args[i..];
            target_msg = Some((target, msg));
            break;
        }
    }
    target_msg
}

fn msg(
    args: &str,
    _: &PathBuf,
    _: &config::Defaults,
    tui: TUIRef,
    clients: &mut Vec<IrcClient>,
    src: MsgSource,
) {
    let fail = || {
        tui.borrow_mut()
            .add_client_err_msg("/msg usage: /msg target message", &MsgTarget::CurrentTab);
    };

    let (target, msg) = match split_msg_args(args) {
        None => return fail(),
        Some((target, msg)) => {
            if msg.is_empty() {
                return fail();
            } else {
                (target, msg)
            }
        }
    };

    let src = if clients
        .iter()
        .any(|client| client.get_serv_name() == target)
    {
        MsgSource::Serv {
            serv_name: target.to_owned(),
        }
    } else {
        let serv = src.serv_name();
        MsgSource::User {
            serv_name: serv.to_owned(),
            nick: target.to_owned(),
        }
    };

    crate::send_msg(&mut *tui.borrow_mut(), clients, &src, msg.to_owned(), false);
}

////////////////////////////////////////////////////////////////////////////////////////////////////

static NAMES_CMD: Cmd = Cmd {
    name: "names",
    cmd_fn: names,
};

fn names(
    args: &str,
    _: &PathBuf,
    _: &config::Defaults,
    tui: TUIRef,
    _: &mut Vec<IrcClient>,
    src: MsgSource,
) {
    let mut tui = tui.borrow_mut();
    let words: Vec<&str> = args.split_whitespace().collect();

    if let MsgSource::Chan {
        ref serv_name,
        ref chan_name,
    } = src
    {
        let nicks_vec = tui.get_nicks(serv_name, chan_name);
        if let Some(nicks_vec) = nicks_vec {
            let target = MsgTarget::Chan {
                serv_name,
                chan_name,
            };
            if words.is_empty() {
                tui.add_client_msg(
                    &format!("{} users: {}", nicks_vec.len(), nicks_vec.join(", ")),
                    &target,
                );
            } else {
                let nick = words[0];
                if nicks_vec.iter().any(|v| v == nick) {
                    tui.add_client_msg(&format!("{} is online", nick), &target);
                } else {
                    tui.add_client_msg(&format!("{} is not in the channel", nick), &target);
                }
            }
        }
    } else {
        tui.add_client_err_msg("/names only supported in chan tabs", &MsgTarget::CurrentTab);
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

static NICK_CMD: Cmd = Cmd {
    name: "nick",
    cmd_fn: nick,
};

fn nick(
    args: &str,
    _: &PathBuf,
    _: &config::Defaults,
    tui: TUIRef,
    clients: &mut Vec<IrcClient>,
    src: MsgSource,
) {
    let words: Vec<&str> = args.split_whitespace().collect();
    if words.len() == 1 {
        if let Some(client) = find_client(clients, src.serv_name()) {
            let new_nick = words[0];
            client.nick(new_nick);
        }
    } else {
        tui.borrow_mut()
            .add_client_err_msg("/nick usage: /nick <nick>", &MsgTarget::CurrentTab);
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

static RELOAD_CMD: Cmd = Cmd {
    name: "reload",
    cmd_fn: reload,
};

fn reload(
    _: &str,
    config_path: &PathBuf,
    _: &config::Defaults,
    tui: TUIRef,
    _: &mut Vec<IrcClient>,
    _: MsgSource,
) {
    let mut tui = tui.borrow_mut();
    match config::parse_config(config_path) {
        Ok(config::Config { colors, .. }) => tui.set_colors(colors),
        Err(err) => {
            tui.add_client_err_msg("Can't parse config file:", &MsgTarget::CurrentTab);
            for line in err.description().lines() {
                tui.add_client_err_msg(line, &MsgTarget::CurrentTab);
            }
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

static SWITCH_CMD: Cmd = Cmd {
    name: "switch",
    cmd_fn: switch,
};

fn switch(
    args: &str,
    _: &PathBuf,
    _: &config::Defaults,
    tui: TUIRef,
    _: &mut Vec<IrcClient>,
    _: MsgSource,
) {
    let words: Vec<&str> = args.split_whitespace().collect();
    if words.len() != 1 {
        return tui
            .borrow_mut()
            .add_client_err_msg("/switch usage: /switch <tab name>", &MsgTarget::CurrentTab);
    }
    tui.borrow_mut().switch(words[0]);
}

////////////////////////////////////////////////////////////////////////////////////////////////////

static NOTIFY_CMD: Cmd = Cmd {
    name: "notify",
    cmd_fn: notify,
};

fn notify(
    args: &str,
    _: &PathBuf,
    _: &config::Defaults,
    tui: TUIRef,
    _: &mut Vec<IrcClient>,
    src: MsgSource,
) {
    let mut tui = tui.borrow_mut();

    let words: Vec<&str> = args.split_whitespace().collect();

    let mut show_usage = || {
        tui.add_client_err_msg(
            "/notify usage: /notify [off|mentions|messages]",
            &MsgTarget::CurrentTab,
        )
    };

    if words.is_empty() {
        tui.show_notify_mode(&MsgTarget::CurrentTab);
    } else if words.len() != 1 {
        show_usage();
    } else {
        let notifier = match words[0] {
            "off" => {
                tui.add_client_notify_msg("Notifications turned off", &MsgTarget::CurrentTab);
                Notifier::Off
            }
            "mentions" => {
                tui.add_client_notify_msg(
                    "Notifications enabled for mentions",
                    &MsgTarget::CurrentTab,
                );
                Notifier::Mentions
            }
            "messages" => {
                tui.add_client_notify_msg(
                    "Notifications enabled for all messages",
                    &MsgTarget::CurrentTab,
                );
                Notifier::Messages
            }
            _ => {
                return show_usage();
            }
        };
        // can't use `MsgSource::to_target` here, `Serv` case is different
        let tab_target = match src {
            MsgSource::Serv { ref serv_name } => MsgTarget::AllServTabs { serv_name },
            MsgSource::Chan {
                ref serv_name,
                ref chan_name,
            } => MsgTarget::Chan {
                serv_name,
                chan_name,
            },
            MsgSource::User {
                ref serv_name,
                ref nick,
            } => MsgTarget::User { serv_name, nick },
        };
        tui.set_notifier(notifier, &tab_target);
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cmd() {
        let ret = parse_cmd("msg NickServ identify notMyPassword");
        match ret {
            ParseCmdResult::Ok { cmd, rest } => {
                assert_eq!(cmd.name, "msg");
                assert_eq!(rest, "NickServ identify notMyPassword");
            }
            _ => {
                panic!("Can't parse cmd");
            }
        }

        let ret = parse_cmd("join #foo");
        match ret {
            ParseCmdResult::Ok { cmd, rest } => {
                assert_eq!(cmd.name, "join");
                assert_eq!(rest, "#foo");
            }
            _ => {
                panic!("Can't parse cmd");
            }
        }
    }

    #[test]
    fn test_msg_args() {
        assert_eq!(split_msg_args("foo,bar"), Some(("foo", ",bar")));
        assert_eq!(split_msg_args("foo bar"), Some(("foo", "bar")));
        assert_eq!(split_msg_args("foo, bar"), Some(("foo", ", bar")));
        assert_eq!(split_msg_args("foo ,bar"), Some(("foo", ",bar")));
    }
}
