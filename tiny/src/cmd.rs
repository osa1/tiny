// I think borrowed boxing is necessary for objekt::clone_box to work
#![allow(clippy::borrowed_box)]

use crate::config;
use crate::utils;
use libtiny_client::{Client, ServerInfo};
use libtiny_ui::{MsgSource, MsgTarget, UI};
use std::path::Path;

pub(crate) struct CmdArgs<'a> {
    pub args: &'a str,
    pub config_path: &'a Path,
    pub defaults: &'a config::Defaults,
    pub ui: &'a Box<dyn UI>,
    pub clients: &'a mut Vec<Client>,
    pub src: MsgSource,
}

pub(crate) struct Cmd {
    /// Command name. E.g. if this is `"cmd"`, `/cmd ...` will call this command.
    pub(crate) name: &'static str,

    // Command help message. Shown in `/help`.
    // pub(crate) help: &'static str,
    /// Command function.
    pub(crate) cmd_fn: fn(CmdArgs),
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

fn find_client_idx(clients: &[Client], serv_name: &str) -> Option<usize> {
    for (client_idx, client) in clients.iter().enumerate() {
        if client.get_serv_name() == serv_name {
            return Some(client_idx);
        }
    }
    None
}

fn find_client<'a>(clients: &'a mut Vec<Client>, serv_name: &str) -> Option<&'a mut Client> {
    match find_client_idx(clients, serv_name) {
        None => None,
        Some(idx) => Some(&mut clients[idx]),
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

static CMDS: [&Cmd; 8] = [
    &AWAY_CMD,
    &CLOSE_CMD,
    &CONNECT_CMD,
    &JOIN_CMD,
    &ME_CMD,
    &MSG_CMD,
    &NAMES_CMD,
    &NICK_CMD,
];

////////////////////////////////////////////////////////////////////////////////////////////////////

static AWAY_CMD: Cmd = Cmd {
    name: "away",
    cmd_fn: away,
};

fn away(args: CmdArgs) {
    let msg = if args.args.is_empty() {
        None
    } else {
        Some(args.args)
    };
    if let Some(client) = find_client(args.clients, args.src.serv_name()) {
        client.away(msg);
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

static CLOSE_CMD: Cmd = Cmd {
    name: "close",
    cmd_fn: close,
};

fn close(args: CmdArgs) {
    let CmdArgs {
        ui, clients, src, ..
    } = args;
    match src {
        MsgSource::Serv { ref serv } if serv == "mentions" => {
            // ignore
        }
        MsgSource::Serv { serv } => {
            ui.close_server_tab(&serv);
            let client_idx = find_client_idx(&clients, &serv).unwrap();
            // TODO: this probably won't close the connection?
            let mut client = clients.remove(client_idx);
            client.quit(None);
        }
        MsgSource::Chan { serv, chan } => {
            ui.close_chan_tab(&serv, &chan);
            let client_idx = find_client_idx(&clients, &serv).unwrap();
            clients[client_idx].part(&chan);
        }
        MsgSource::User { serv, nick } => {
            ui.close_user_tab(&serv, &nick);
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

static CONNECT_CMD: Cmd = Cmd {
    name: "connect",
    cmd_fn: connect,
};

fn connect(args: CmdArgs) {
    let CmdArgs {
        args,
        defaults,
        ui,
        clients,
        src,
        ..
    } = args;
    let words: Vec<&str> = args.split_whitespace().collect();

    match words.len() {
        0 => reconnect(ui, clients, src),
        1 => connect_(words[0], None, defaults, ui, clients),
        2 => connect_(words[0], Some(words[1]), defaults, ui, clients),
        _ =>
        // wat
        {
            ui.add_client_err_msg(
                "/connect usage: /connect <host>:<port> or /connect (to reconnect)",
                &MsgTarget::CurrentTab,
            )
        }
    }
}

fn reconnect(ui: &Box<dyn UI>, clients: &mut Vec<Client>, src: MsgSource) {
    if let Some(client) = find_client(clients, src.serv_name()) {
        ui.add_client_msg(
            "Reconnecting...",
            &MsgTarget::AllServTabs {
                serv: src.serv_name(),
            },
        );
        client.reconnect(None);
    }
}

fn connect_(
    serv_addr: &str,
    pass: Option<&str>,
    defaults: &config::Defaults,
    ui: &Box<dyn UI>,
    clients: &mut Vec<Client>,
) {
    fn split_port(s: &str) -> Option<(&str, &str)> {
        s.find(':').map(|split| (&s[0..split], &s[split + 1..]))
    }

    // parse host name and port
    let (serv_name, serv_port) = {
        match split_port(serv_addr) {
            None => {
                return ui
                    .add_client_err_msg("connect: Need a <host>:<port>", &MsgTarget::CurrentTab);
            }
            Some((serv_name, serv_port)) => match serv_port.parse::<u16>() {
                Err(err) => {
                    return ui.add_client_err_msg(
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
        ui.add_client_msg("Connecting...", &MsgTarget::AllServTabs { serv: serv_name });
        client.reconnect(Some(serv_port));
        return;
    }

    // otherwise create a new connection
    // can't move the rest to an else branch because of borrowchk

    // otherwise create a new Conn, tab etc.
    ui.new_server_tab(serv_name);
    let msg_target = MsgTarget::Server { serv: serv_name };
    ui.add_client_msg("Connecting...", &msg_target);

    let (client, rcv_ev) = Client::new(ServerInfo {
        addr: serv_name.to_owned(),
        port: serv_port,
        tls: defaults.tls,
        realname: defaults.realname.clone(),
        pass: pass.map(str::to_owned),
        nicks: defaults.nicks.clone(),
        auto_join: defaults.join.clone(),
        nickserv_ident: None,
        sasl_auth: None,
    });

    // Spawn UI task
    let ui_clone = libtiny_ui::clone_box(&**ui);
    let client_clone = client.clone();
    tokio::runtime::current_thread::spawn(crate::conn::task(rcv_ev, ui_clone, client_clone));

    clients.push(client);
}

////////////////////////////////////////////////////////////////////////////////////////////////////

static JOIN_CMD: Cmd = Cmd {
    name: "join",
    cmd_fn: join,
};

fn join(args: CmdArgs) {
    let CmdArgs {
        args,
        ui,
        clients,
        src,
        ..
    } = args;
    let words = args.split_whitespace().collect::<Vec<_>>();
    if words.is_empty() {
        return ui.add_client_err_msg(
            "/join usage: /join chan1[,chan2...]",
            &MsgTarget::CurrentTab,
        );
    }

    match find_client(clients, src.serv_name()) {
        Some(client) => client.join(&words),
        None => ui.add_client_err_msg(
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

fn me(args: CmdArgs) {
    let CmdArgs {
        args,
        ui,
        clients,
        src,
        ..
    } = args;
    if args.is_empty() {
        return ui.add_client_err_msg("/me usage: /me message", &MsgTarget::CurrentTab);
    }
    crate::ui::send_msg(&**ui, clients, &src, args.to_string(), true);
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

fn msg(args: CmdArgs) {
    let CmdArgs {
        args,
        ui,
        clients,
        src,
        ..
    } = args;
    let fail = || {
        ui.add_client_err_msg("/msg usage: /msg target message", &MsgTarget::CurrentTab);
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
            serv: target.to_owned(),
        }
    } else {
        let serv = src.serv_name();
        MsgSource::User {
            serv: serv.to_owned(),
            nick: target.to_owned(),
        }
    };

    crate::ui::send_msg(&**ui, clients, &src, msg.to_owned(), false);
}

////////////////////////////////////////////////////////////////////////////////////////////////////

static NAMES_CMD: Cmd = Cmd {
    name: "names",
    cmd_fn: names,
};

fn names(args: CmdArgs) {
    let CmdArgs {
        args,
        ui,
        src,
        clients,
        ..
    } = args;
    let words: Vec<&str> = args.split_whitespace().collect();

    let client = match find_client(clients, src.serv_name()) {
        None => {
            return;
        }
        Some(client) => client,
    };

    if let MsgSource::Chan { ref serv, ref chan } = src {
        let nicks_vec = client.get_chan_nicks(chan);
        let target = MsgTarget::Chan { serv, chan };
        if words.is_empty() {
            ui.add_client_msg(
                &format!("{} users: {}", nicks_vec.len(), nicks_vec.join(", ")),
                &target,
            );
        } else {
            let nick = words[0];
            if nicks_vec.iter().any(|v| v == nick) {
                ui.add_client_msg(&format!("{} is online", nick), &target);
            } else {
                ui.add_client_msg(&format!("{} is not in the channel", nick), &target);
            }
        }
    } else {
        ui.add_client_err_msg("/names only supported in chan tabs", &MsgTarget::CurrentTab);
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

static NICK_CMD: Cmd = Cmd {
    name: "nick",
    cmd_fn: nick,
};

fn nick(args: CmdArgs) {
    let CmdArgs {
        args,
        ui,
        clients,
        src,
        ..
    } = args;
    let words: Vec<&str> = args.split_whitespace().collect();
    if words.len() == 1 {
        if let Some(client) = find_client(clients, src.serv_name()) {
            let new_nick = words[0];
            client.nick(new_nick);
        }
    } else {
        ui.add_client_err_msg("/nick usage: /nick <nick>", &MsgTarget::CurrentTab);
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
