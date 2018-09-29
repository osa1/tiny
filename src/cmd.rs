use config;
use conn::Conn;
use mio::Poll;
use std::error::Error;
use super::Tiny;
use tui::{MsgTarget, MsgSource, Timestamp};
use utils;

use notifier::Notifier;

pub struct Cmd {
    /// Command name. E.g. if this is `"cmd"`, `/cmd ...` will call this command.
    pub name: &'static str,

    // Command help message. Shown in `/help`.
    // pub help: &'static str,

    /// Command function.
    pub cmd_fn: for<'a, 'b> fn(&str, poll: &'b Poll, &'a mut Tiny<'b>, MsgSource),
}

////////////////////////////////////////////////////////////////////////////////////////////////////

pub enum ParseCmdResult<'a> {
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

pub fn parse_cmd(cmd: &str) -> ParseCmdResult {
    match cmd.split_whitespace().next() {
        None =>
            ParseCmdResult::Unknown,
        Some(cmd_name) => {
            let mut ws_idxs = utils::split_whitespace_indices(cmd);
            ws_idxs.next(); // cmd_name
            let rest = {
                match ws_idxs.next() {
                    None =>
                        "",
                    Some(rest_idx) =>
                        &cmd[rest_idx..],
                }
            };
            // let mut possibilities: Vec<&'static Cmd> = vec![];
            for cmd in &CMDS {
                if cmd_name == cmd.name {
                    // exact match, return
                    return ParseCmdResult::Ok { cmd, rest }
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

////////////////////////////////////////////////////////////////////////////////////////////////////

static CMDS: [&'static Cmd; 14] = [
    &AWAY_CMD,
    &CLEAR_CMD,
    &CLOSE_CMD,
    &CONNECT_CMD,
    &HELP_CMD,
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

fn away(args: &str, _: &Poll, tiny: &mut Tiny, src: MsgSource) {
    let msg =
        if args.is_empty() {
            None
        } else {
            Some(args)
        };
    if let Some(conn) = super::find_conn(&mut tiny.conns, src.serv_name()) {
        conn.away(msg);
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

static CLEAR_CMD: Cmd = Cmd {
    name: "clear",
    cmd_fn: clear,
};

fn clear(_: &str, _: &Poll, tiny: &mut Tiny, src: MsgSource) {
    tiny.tui.clear(&src.to_target());
}

////////////////////////////////////////////////////////////////////////////////////////////////////

static CLOSE_CMD: Cmd = Cmd {
    name: "close",
    cmd_fn: close,
};

fn close(_: &str, _: &Poll, tiny: &mut Tiny, src: MsgSource) {
    match src {
        MsgSource::Serv { ref serv_name } if serv_name == "mentions" => {
            // ignore
        }
        MsgSource::Serv { serv_name } => {
            tiny.tui.close_server_tab(&serv_name);
            let conn_idx = super::find_conn_idx(&tiny.conns, &serv_name).unwrap();
            tiny.conns.remove(conn_idx);
        }
        MsgSource::Chan {
            serv_name,
            chan_name,
        } => {
            tiny.tui.close_chan_tab(&serv_name, &chan_name);
            tiny.part(&serv_name, &chan_name);
        }
        MsgSource::User { serv_name, nick } => {
            tiny.tui.close_user_tab(&serv_name, &nick);
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

static CONNECT_CMD: Cmd = Cmd {
    name: "connect",
    cmd_fn: connect,
};

fn connect<'a, 'b>(args: &str, poll: &'b Poll, tiny: &'a mut Tiny<'b>, src: MsgSource) {
    let words: Vec<&str> = args.split_whitespace().into_iter().collect();

    match words.len() {
        0 =>
            reconnect(tiny, src),
        1 =>
            connect_(words[0], None, poll, tiny),
        2 =>
            connect_(words[0], Some(words[1]), poll, tiny),
        _ =>
            // wat
            tiny.tui.add_client_err_msg(
                "/connect usage: /connect <host>:<port> or /connect (to reconnect)",
                &MsgTarget::CurrentTab,
            ),
    }
}

fn reconnect(tiny: &mut Tiny, src: MsgSource) {
    tiny.tui.add_client_msg(
        "Reconnecting...",
        &MsgTarget::AllServTabs {
            serv_name: src.serv_name(),
        },
    );
    match super::find_conn(&mut tiny.conns, src.serv_name()) {
        Some(conn) =>
            match conn.reconnect(None) {
                Ok(()) =>
                    {}
                Err(err) => {
                    tiny.tui.add_err_msg(
                        &super::reconnect_err_msg(&err),
                        Timestamp::now(),
                        &MsgTarget::AllServTabs {
                            serv_name: conn.get_serv_name(),
                        },
                    );
                }
            },
        None => {
            tiny.logger
                .get_debug_logs()
                .write_line(format_args!("Can't reconnect to {}", src.serv_name()));
        }
    }
}

fn connect_<'a, 'b>(serv_addr: &str, pass: Option<&str>, poll: &'b Poll, tiny: &'a mut Tiny<'b>) {
    fn split_port(s: &str) -> Option<(&str, &str)> {
        s.find(':').map(|split| (&s[0..split], &s[split + 1..]))
    }

    // parse host name and port
    let (serv_name, serv_port) = {
        match split_port(serv_addr) {
            None => {
                return tiny.tui
                    .add_client_err_msg("connect: Need a <host>:<port>", &MsgTarget::CurrentTab);
            }
            Some((serv_name, serv_port)) =>
                match serv_port.parse::<u16>() {
                    Err(err) => {
                        return tiny.tui.add_client_err_msg(
                            &format!("connect: Can't parse port {}: {}", serv_port, err),
                            &MsgTarget::CurrentTab,
                        );
                    }
                    Ok(serv_port) =>
                        (serv_name, serv_port),
                },
        }
    };

    // if we already connected to this server reconnect using new port
    if let Some(conn) = super::find_conn(&mut tiny.conns, serv_name) {
        tiny.tui.add_client_msg(
            "Connecting...",
            &MsgTarget::AllServTabs { serv_name },
        );
        match conn.reconnect(Some((serv_name, serv_port))) {
            Ok(()) =>
                {}
            Err(err) => {
                tiny.tui.add_err_msg(
                    &super::reconnect_err_msg(&err),
                    Timestamp::now(),
                    &MsgTarget::AllServTabs {
                        serv_name: conn.get_serv_name(),
                    },
                );
            }
        }
        return;
    }

    // otherwise create a new connection
    // can't move the rest to an else branch because of borrowchk

    // otherwise create a new Conn, tab etc.
    tiny.tui.new_server_tab(serv_name);
    let msg_target = MsgTarget::Server { serv_name };
    tiny.tui.add_client_msg("Connecting...", &msg_target);

    let conn_ret = Conn::new(
        config::Server {
            addr: serv_name.to_owned(),
            port: serv_port,
            tls: tiny.defaults.tls,
            hostname: tiny.defaults.hostname.clone(),
            realname: tiny.defaults.realname.clone(),
            pass: pass.map(str::to_owned),
            nicks: tiny.defaults.nicks.clone(),
            join: tiny.defaults.join.clone(),
            nickserv_ident: None,
            sasl_auth: None,
        },
        poll,
    );

    match conn_ret {
        Ok(conn) => {
            tiny.conns.push(conn);
        }
        Err(err) => {
            tiny.tui
                .add_err_msg(&super::connect_err_msg(&err), Timestamp::now(), &msg_target);
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

static HELP_CMD: Cmd = Cmd {
    name: "help",
    cmd_fn: help,
};

fn help(_: &str, _: &Poll, _: &mut Tiny, _: MsgSource) {
    // TODO
}

////////////////////////////////////////////////////////////////////////////////////////////////////

static IGNORE_CMD: Cmd = Cmd {
    name: "ignore",
    cmd_fn: ignore,
};

fn ignore(_: &str, _: &Poll, tiny: &mut Tiny, src: MsgSource) {
    match src {
        MsgSource::Serv { serv_name } => {
            tiny.tui.toggle_ignore(&MsgTarget::AllServTabs {
                serv_name: &serv_name,
            });
        }
        MsgSource::Chan {
            serv_name,
            chan_name,
        } => {
            tiny.tui.toggle_ignore(&MsgTarget::Chan {
                serv_name: &serv_name,
                chan_name: &chan_name,
            });
        }
        MsgSource::User { serv_name, nick } => {
            tiny.tui.toggle_ignore(&MsgTarget::User {
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

fn join(args: &str, _: &Poll, tiny: &mut Tiny, src: MsgSource) {
    let words = args.split_whitespace().collect::<Vec<_>>();
    if words.is_empty() {
        return tiny.tui.add_client_err_msg(
            "/join usage: /join chan1[,chan2...]", &MsgTarget::CurrentTab);
    }

    match super::find_conn(&mut tiny.conns, src.serv_name()) {
        Some(conn) =>
            conn.join(&words),
        None =>
            tiny.tui.add_client_err_msg(
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

fn me(args: &str, _: &Poll, tiny: &mut Tiny, src: MsgSource) {
    if args.len() == 0 {
        return tiny.tui
            .add_client_err_msg("/me usage: /me message", &MsgTarget::CurrentTab);
    }
    tiny.send_msg(&src, args, true);
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

fn msg(args: &str, _: &Poll, tiny: &mut Tiny, src: MsgSource) {
    let mut fail = || {
        tiny.tui.add_client_err_msg(
            "/msg usage: /msg target message", &MsgTarget::CurrentTab);
    };

    let (target, msg) = match split_msg_args(args) {
        None => { return fail() }
        Some((target, msg)) => {
            if msg.is_empty() {
                return fail();
            } else {
                (target, msg)
            }
        }
    };

    let src = if tiny.conns.iter().any(|conn| conn.get_serv_name() == target) {
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

    tiny.send_msg(&src, msg, false);
}

////////////////////////////////////////////////////////////////////////////////////////////////////

static NAMES_CMD: Cmd = Cmd {
    name: "names",
    cmd_fn: names,
};

fn names(args: &str, _: &Poll, tiny: &mut Tiny, src: MsgSource) {
    let words: Vec<&str> = args.split_whitespace().collect();

    if let MsgSource::Chan {
        ref serv_name,
        ref chan_name,
    } = src
    {
        let nicks_vec = tiny.tui
            .get_nicks(serv_name, chan_name)
            .map(|nicks| nicks.to_strings(""));
        if let Some(nicks_vec) = nicks_vec {
            let target = MsgTarget::Chan { serv_name, chan_name };
            if words.is_empty() {
                tiny.tui.add_client_msg(
                    &format!("{} users: {}", nicks_vec.len(), nicks_vec.join(", ")),
                    &target,
                );
            } else {
                let nick = words[0];
                if nicks_vec.iter().any(|v| v == nick) {
                    tiny.tui.add_client_msg(&format!("{} is online", nick), &target);
                } else {
                    tiny.tui.add_client_msg(&format!("{} is not in the channel", nick), &target);
                }
            }
        }
    } else {
        tiny.tui.add_client_err_msg(
            "/names only supported in chan tabs",
            &MsgTarget::CurrentTab,
        );
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

static NICK_CMD: Cmd = Cmd {
    name: "nick",
    cmd_fn: nick,
};

fn nick(args: &str, _: &Poll, tiny: &mut Tiny, src: MsgSource) {
    let words: Vec<&str> = args.split_whitespace().collect();
    if words.len() == 1 {
        if let Some(conn) = super::find_conn(&mut tiny.conns, src.serv_name()) {
            let new_nick = words[0];
            conn.send_nick(new_nick);
        }
    } else {
        tiny.tui.add_client_err_msg(
            "/nick usage: /nick <nick>",
            &MsgTarget::CurrentTab,
        );
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

static RELOAD_CMD: Cmd = Cmd {
    name: "reload",
    cmd_fn: reload,
};

fn reload(_: &str, _: &Poll, tiny: &mut Tiny, _: MsgSource) {
    match config::parse_config(&tiny.config_path) {
        Ok(config::Config { colors, .. }) =>
            tiny.tui.set_colors(colors),
        Err(err) => {
            tiny.tui
                .add_client_err_msg("Can't parse config file:", &MsgTarget::CurrentTab);
            for line in err.description().lines() {
                tiny.tui.add_client_err_msg(line, &MsgTarget::CurrentTab);
            }
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

static SWITCH_CMD: Cmd = Cmd {
    name: "switch",
    cmd_fn: switch,
};

fn switch(args: &str, _: &Poll, tiny: &mut Tiny, _: MsgSource) {
    let words: Vec<&str> = args.split_whitespace().collect();
    if words.len() != 1 {
        return tiny.tui.add_client_err_msg(
            "/switch usage: /switch <tab name>",
            &MsgTarget::CurrentTab,
        );
    }
    tiny.tui.switch(words[0]);
}

////////////////////////////////////////////////////////////////////////////////////////////////////

static NOTIFY_CMD: Cmd = Cmd {
    name: "notify",
    cmd_fn: notify,
};

fn notify(args: &str, _: &Poll, tiny: &mut Tiny, src: MsgSource) {
    let words: Vec<&str> = args.split_whitespace().collect();

    let mut show_usage = || {
        tiny.tui.add_client_err_msg(
            "/notify usage: /notify [off|mentions|messages]",
            &MsgTarget::CurrentTab,
        )
    };

    if words.is_empty() {
        tiny.tui.show_notify_mode(&MsgTarget::CurrentTab);
    } else if words.len() != 1 {
        show_usage();
    } else {
        let notifier =
            match words[0] {
                "off" => {
                    tiny.tui.add_client_notify_msg(
                        "Notifications turned off",
                        &MsgTarget::CurrentTab,
                    );
                    Notifier::Off
                }
                "mentions" => {
                    tiny.tui.add_client_notify_msg(
                        "Notifications enabled for mentions",
                        &MsgTarget::CurrentTab,
                    );
                    Notifier::Mentions
                }
                "messages" => {
                    tiny.tui.add_client_notify_msg(
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
        let tab_target =
            match src {
                MsgSource::Serv { ref serv_name } =>
                    MsgTarget::AllServTabs { serv_name },
                MsgSource::Chan { ref serv_name, ref chan_name } =>
                    MsgTarget::Chan { serv_name, chan_name },
                MsgSource::User { ref serv_name, ref nick } =>
                    MsgTarget::User { serv_name, nick },
            };
        tiny.tui.set_notifier(notifier, &tab_target);
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
            },
            _ => {
                panic!("Can't parse cmd");
            }
        }

        let ret = parse_cmd("join #foo");
        match ret {
            ParseCmdResult::Ok { cmd, rest } => {
                assert_eq!(cmd.name, "join");
                assert_eq!(rest, "#foo");
            },
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
