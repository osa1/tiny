use crate::config::Defaults;
use crate::ui::UI;
use crate::utils;
use libtiny_client::{Client, ProxyConfig, ServerInfo};
use libtiny_common::{ChanNameRef, MsgSource, MsgTarget};
use libtiny_tui::config::Chan;

use std::borrow::Borrow;

pub(crate) fn register_completions(tui: &libtiny_tui::TUI) {
    for cmd in &CMDS {
        tui.add_command_completion(cmd.name);
    }
}

pub(crate) fn run_cmd(
    cmd: &str,
    src: MsgSource,
    defaults: &Defaults,
    server_proxies: &[(String, u16, Option<ProxyConfig>)],
    ui: &UI,
    clients: &mut Vec<Client>,
) {
    match parse_cmd(cmd) {
        Some(ParsedCmd { cmd, args }) => {
            let cmd_args = CmdArgs {
                args,
                defaults,
                server_proxies,
                ui,
                clients,
                src,
            };
            (cmd.cmd_fn)(cmd_args);
        }

        None => {
            ui.add_client_err_msg(
                &format!("Unsupported command: \"/{cmd}\""),
                &MsgTarget::CurrentTab,
            );
        }
    }
}

struct ParsedCmd<'a> {
    cmd: &'static Cmd,

    /// Rest of the command after extracting command name.
    args: &'a str,
}

fn parse_cmd(cmd: &str) -> Option<ParsedCmd<'_>> {
    let cmd_name = cmd.split_whitespace().next()?;
    let mut ws_idxs = utils::split_whitespace_indices(cmd);
    ws_idxs.next(); // cmd_name
    let rest = match ws_idxs.next() {
        None => "",
        Some(rest_idx) => &cmd[rest_idx..],
    };
    for cmd in &CMDS {
        if cmd_name == cmd.name {
            return Some(ParsedCmd { cmd, args: rest });
        }
    }
    None
}

struct CmdArgs<'a> {
    args: &'a str,
    defaults: &'a Defaults,
    server_proxies: &'a [(String, u16, Option<ProxyConfig>)],
    ui: &'a UI,
    clients: &'a mut Vec<Client>,
    src: MsgSource,
}

struct Cmd {
    /// Command name. If this is `"cmd"`, `/cmd ...` will call this command.
    name: &'static str,

    /// Command function.
    cmd_fn: fn(CmdArgs),

    /// Command description. Shown in `/help` and error messages.
    description: &'static str,

    /// Command usage. Shown in `/help` and error messages.
    usage: &'static str,
}

fn find_client_idx(clients: &[Client], serv_name: &str) -> Option<usize> {
    for (client_idx, client) in clients.iter().enumerate() {
        if client.get_serv_name() == serv_name {
            return Some(client_idx);
        }
    }
    None
}

fn find_client<'a>(clients: &'a mut [Client], serv_name: &str) -> Option<&'a mut Client> {
    match find_client_idx(clients, serv_name) {
        None => None,
        Some(idx) => Some(&mut clients[idx]),
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

static CMDS: [&Cmd; 9] = [
    &AWAY_CMD,
    &CLOSE_CMD,
    &CONNECT_CMD,
    &JOIN_CMD,
    &ME_CMD,
    &MSG_CMD,
    &NAMES_CMD,
    &NICK_CMD,
    &HELP_CMD,
];

////////////////////////////////////////////////////////////////////////////////////////////////////

static AWAY_CMD: Cmd = Cmd {
    name: "away",
    cmd_fn: away,
    description: "Sets/removes away message",
    usage: "`/away` or `/away <message>`",
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
    description: "Closes current tab",
    usage: "`/close` or `/close <reason>`",
};

fn close(args: CmdArgs) {
    let CmdArgs {
        args,
        ui,
        clients,
        src,
        ..
    } = args;
    match src {
        MsgSource::Serv { ref serv } if serv == "mentions" => {
            // ignore
        }
        MsgSource::Serv { serv } => {
            ui.close_server_tab(&serv);
            let client_idx = find_client_idx(clients, &serv).unwrap();
            // TODO: this probably won't close the connection?
            let mut client = clients.remove(client_idx);
            if args.is_empty() {
                client.quit(None);
            } else {
                client.quit(Some(args.to_string()));
            }
        }
        MsgSource::Chan { serv, chan } => {
            ui.close_chan_tab(&serv, chan.borrow());
            let client_idx = find_client_idx(clients, &serv).unwrap();
            if args.is_empty() {
                clients[client_idx].part(&chan, None);
            } else {
                clients[client_idx].part(&chan, Some(args.to_string()));
            }
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
    description: "Connects to a server",
    usage: "`/connect <host>:<port>` or `/connect [<IPv6>]:<port>` or `/connect` to reconnect",
};

fn connect(args: CmdArgs) {
    let CmdArgs {
        args,
        defaults,
        server_proxies,
        ui,
        clients,
        src,
        ..
    } = args;
    let words: Vec<&str> = args.split_whitespace().collect();

    match words.len() {
        0 => reconnect(ui, clients, src),
        1 => connect_(words[0], None, defaults, server_proxies, ui, clients),
        2 => connect_(
            words[0],
            Some(words[1]),
            defaults,
            server_proxies,
            ui,
            clients,
        ),
        _ => ui.add_client_err_msg(
            &format!("Usage: {}", CONNECT_CMD.usage),
            &MsgTarget::CurrentTab,
        ),
    }
}

fn reconnect(ui: &UI, clients: &mut [Client], src: MsgSource) {
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

#[derive(Debug)]
enum ProxySelection<'a> {
    Unconfigured,
    Configured(Option<&'a ProxyConfig>),
}

fn same_server_host(left: &str, right: &str) -> bool {
    fn literal(host: &str) -> Option<(std::net::IpAddr, Option<&str>)> {
        let host = host
            .strip_prefix('[')
            .and_then(|host| host.strip_suffix(']'))
            .unwrap_or(host);
        if let Some((ip, scope)) = host.split_once('%') {
            return ip
                .parse::<std::net::Ipv6Addr>()
                .ok()
                .map(|ip| (ip.into(), Some(scope)));
        }
        host.parse().ok().map(|ip| (ip, None))
    }

    match (literal(left), literal(right)) {
        (Some(left), Some(right)) => left == right,
        (None, None) => left
            .strip_suffix('.')
            .unwrap_or(left)
            .eq_ignore_ascii_case(right.strip_suffix('.').unwrap_or(right)),
        _ => false,
    }
}

fn select_proxy<'a>(
    servers: &'a [(String, u16, Option<ProxyConfig>)],
    host: &str,
    port: u16,
) -> Result<ProxySelection<'a>, ()> {
    let mut hosts = servers
        .iter()
        .filter(|(addr, _, _)| same_server_host(addr, host));
    let Some(first) = hosts.next() else {
        return Ok(ProxySelection::Unconfigured);
    };
    if hosts.clone().next().is_none() {
        return Ok(ProxySelection::Configured(first.2.as_ref()));
    }
    let mut matching_ports = std::iter::once(first)
        .chain(hosts)
        .filter(|(_, configured_port, _)| *configured_port == port);
    match (matching_ports.next(), matching_ports.next()) {
        (Some((_, _, proxy)), None) => Ok(ProxySelection::Configured(proxy.as_ref())),
        _ => Err(()),
    }
}

fn split_port(s: &str) -> Option<(&str, &str)> {
    if let Some(bracketed) = s.strip_prefix('[') {
        let (host, rest) = bracketed.split_once(']')?;
        let port = rest.strip_prefix(':')?;
        let ip = match host.split_once('%') {
            Some((ip, scope)) if !scope.is_empty() && !scope.contains('%') => ip,
            Some(_) => return None,
            None => host,
        };
        ip.parse::<std::net::Ipv6Addr>().ok()?;
        Some((host, port))
    } else {
        s.split_once(':')
    }
}

fn connect_(
    serv_addr: &str,
    pass: Option<&str>,
    defaults: &Defaults,
    server_proxies: &[(String, u16, Option<ProxyConfig>)],
    ui: &UI,
    clients: &mut Vec<Client>,
) {
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
                        &format!("connect: Can't parse port {serv_port}: {err}"),
                        &MsgTarget::CurrentTab,
                    );
                }
                Ok(serv_port) => (serv_name, serv_port),
            },
        }
    };

    let proxy = match select_proxy(server_proxies, serv_name, serv_port) {
        Ok(proxy) => proxy,
        Err(()) => {
            ui.add_client_err_msg(
                &format!("connect: ambiguous server configuration for {serv_name}"),
                &MsgTarget::CurrentTab,
            );
            return;
        }
    };

    if let Some(client) = find_client(clients, serv_name) {
        ui.add_client_msg("Connecting...", &MsgTarget::AllServTabs { serv: serv_name });
        match proxy {
            ProxySelection::Unconfigured => client.reconnect(Some(serv_port)),
            ProxySelection::Configured(proxy) => {
                client.reconnect_with_proxy(serv_port, proxy.cloned());
            }
        }
        return;
    }

    let proxy = match proxy {
        ProxySelection::Unconfigured => None,
        ProxySelection::Configured(proxy) => proxy.cloned(),
    };

    // otherwise create a new connection
    // can't move the rest to an else branch because of borrowchk

    // otherwise create a new Conn, tab etc.
    ui.new_server_tab(serv_name, None);
    let msg_target = MsgTarget::Server { serv: serv_name };
    ui.add_client_msg("Connecting...", &msg_target);

    let (client, rcv_ev) = Client::new(ServerInfo {
        addr: serv_name.to_owned(),
        port: serv_port,
        tls: defaults.tls,
        proxy,
        user: None,
        realname: defaults.realname.clone(),
        pass: pass.map(str::to_owned),
        nicks: defaults.nicks.clone(),
        auto_join: defaults
            .join
            .iter()
            .map(|c| ChanNameRef::new(c).to_owned())
            .collect(),
        nickserv_ident: None,
        sasl_auth: None,
    });

    // Spawn UI task
    let ui_clone = ui.clone();
    let client_clone = client.clone();
    tokio::task::spawn_local(crate::conn::task(rcv_ev, ui_clone, Box::new(client_clone)));

    clients.push(client);
}

////////////////////////////////////////////////////////////////////////////////////////////////////

static JOIN_CMD: Cmd = Cmd {
    name: "join",
    cmd_fn: join,
    description: "Joins a channel",
    usage: "`/join <chan1> [-ignore] [-notify [off|mentions|messages]],<chan2>...` or `/join` in a channel tab to rejoin",
};

fn join(args: CmdArgs) {
    let CmdArgs {
        args,
        ui,
        clients,
        src,
        ..
    } = args;

    if let MsgSource::Serv { serv } = &src
        && serv == "mentions"
    {
        return ui.add_client_err_msg(
            "Switch to a server tab to join a channel",
            &MsgTarget::CurrentTab,
        );
    }

    let chans = args
        .split(',')
        .map(str::trim)
        .filter_map(|c| match Chan::from_cmd_args(c) {
            Ok(c) => Some(c),
            Err(err) => {
                ui.add_client_err_msg(&err, &MsgTarget::CurrentTab);
                None
            }
        })
        .collect::<Vec<Chan>>();

    let chans = if chans.is_empty() {
        match ui.current_tab() {
            None => return,
            Some(MsgSource::Chan { serv, chan }) => {
                // Rejoin current tab's channel.
                let config = ui.get_tab_config(&serv, Some(chan.as_ref()));
                vec![Chan::WithConfig { name: chan, config }]
            }
            Some(MsgSource::Serv { .. } | MsgSource::User { .. }) => {
                return ui.add_client_err_msg(
                    &format!("Usage: {}", JOIN_CMD.usage),
                    &MsgTarget::CurrentTab,
                );
            }
        }
    } else {
        chans
    };

    let serv = src.serv_name();
    match find_client(clients, serv) {
        Some(client) => {
            let iter_ref = chans.iter().map(|c| c.name());
            // set tab configs of new channel tabs (creates new tab)
            for chan in &chans {
                match chan {
                    Chan::Name(name) => {
                        let config = ui.get_tab_config(serv, Some(name.as_ref()));
                        ui.set_tab_config(serv, Some(name), config)
                    }
                    Chan::WithConfig { name, config } => {
                        ui.set_tab_config(serv, Some(name), config.to_owned())
                    }
                }
            }
            client.join(iter_ref);
        }
        None => ui.add_client_err_msg(
            &format!("Can't join: Not connected to server {}", src.serv_name()),
            &MsgTarget::CurrentTab,
        ),
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

static ME_CMD: Cmd = Cmd {
    name: "me",
    cmd_fn: me,
    description: "Sends emote message",
    usage: "`/me <message>`",
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
        return ui.add_client_err_msg(&format!("Usage: {}", ME_CMD.usage), &MsgTarget::CurrentTab);
    }
    crate::ui::send_msg(ui, clients, &src, args.to_string(), true);
}

////////////////////////////////////////////////////////////////////////////////////////////////////

static MSG_CMD: Cmd = Cmd {
    name: "msg",
    cmd_fn: msg,
    description: "Sends a message to a user",
    usage: "`/msg <nick> <message>`",
};

fn split_msg_args(args: &str) -> Option<(&str, &str)> {
    let mut char_indices = args.char_indices();

    // We could check for validity of the nick according to RFC 2812 but we do the simple thing for
    // now and and only check the first character, to avoid confusing the UI by returning a
    // `MsgSource::User` with a channel name as `nick`.
    match char_indices.next() {
        None => {
            return None;
        }
        Some((_, c)) => {
            if !utils::is_nick_first_char(c) {
                return None;
            }
        }
    }

    for (i, c) in char_indices {
        if c.is_whitespace() {
            return Some((&args[0..i], &args[i + 1..]));
        }
    }

    None
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
        ui.add_client_err_msg(&format!("Usage: {}", MSG_CMD.usage), &MsgTarget::CurrentTab);
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

    crate::ui::send_msg(ui, clients, &src, msg.to_owned(), false);
}

////////////////////////////////////////////////////////////////////////////////////////////////////

static NAMES_CMD: Cmd = Cmd {
    name: "names",
    cmd_fn: names,
    description: "Shows users in channel",
    usage: "`/names`",
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
                ui.add_client_msg(&format!("{nick} is online"), &target);
            } else {
                ui.add_client_msg(&format!("{nick} is not in the channel"), &target);
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
    description: "Sets your nick",
    usage: "`/nick <nick>`",
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
        ui.add_client_err_msg(
            &format!("Usage: {}", NICK_CMD.usage),
            &MsgTarget::CurrentTab,
        );
    }
}

static HELP_CMD: Cmd = Cmd {
    name: "help",
    cmd_fn: help,
    description: "Displays this message",
    usage: "`/help`",
};

fn help(args: CmdArgs) {
    let CmdArgs { ui, .. } = args;
    ui.add_client_msg("Client Commands:", &MsgTarget::CurrentTab);
    for cmd in CMDS.iter() {
        ui.add_client_msg(
            &format!(
                "/{:<10} - {:<25} - Usage: {}",
                cmd.name, cmd.description, cmd.usage
            ),
            &MsgTarget::CurrentTab,
        )
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////

#[test]
fn test_parse_cmd() {
    let ParsedCmd { cmd, args } = parse_cmd("msg NickServ identify notMyPassword").unwrap();
    assert_eq!(cmd.name, "msg");
    assert_eq!(args, "NickServ identify notMyPassword");

    let ParsedCmd { cmd, args } = parse_cmd("join #foo").unwrap();
    assert_eq!(cmd.name, "join");
    assert_eq!(args, "#foo");
}

#[test]
fn test_split_port() {
    for (input, host, port) in [
        ("irc.example:6697", "irc.example", 6697),
        ("127.0.0.1:6667", "127.0.0.1", 6667),
        ("[::1]:6667", "::1", 6667),
        ("[2001:db8::1]:6697", "2001:db8::1", 6697),
        ("[::ffff:192.0.2.1]:6667", "::ffff:192.0.2.1", 6667),
        ("[fe80::1%lo]:6667", "fe80::1%lo", 6667),
        ("host:0", "host", 0),
        ("host:65535", "host", 65535),
    ] {
        let (actual_host, actual_port) = split_port(input).unwrap();
        assert_eq!(actual_host, host, "{input}");
        assert_eq!(actual_port.parse::<u16>(), Ok(port), "{input}");
    }

    for input in [
        "[::1",
        "[::1:6667",
        "[::1]]:6667",
        "[]:6667",
        "[not-ipv6]:6667",
        "[127.0.0.1]:6667",
        "[::1%]:6667",
        "[::1%lo%extra]:6667",
        "[::1]",
        "[::1]6667",
    ] {
        assert_eq!(split_port(input), None, "{input}");
    }

    for input in ["[::1]:", "[::1]:abc", "[::1]:65536", "::1:6667"] {
        let (_, port) = split_port(input).unwrap();
        assert!(port.parse::<u16>().is_err(), "{input}");
    }
}

#[test]
fn test_msg_args() {
    assert_eq!(split_msg_args("foo,bar"), None);
    assert_eq!(split_msg_args("foo bar"), Some(("foo", "bar")));
    assert_eq!(split_msg_args("foo, bar"), Some(("foo,", "bar"))); // nick not valid according to RFC but whatever
    assert_eq!(split_msg_args("foo ,bar"), Some(("foo", ",bar")));
    assert_eq!(split_msg_args("#blah blah"), None);
}

#[test]
fn selects_proxy_for_manual_connections() {
    use libtiny_client::ProxyProtocol;

    let proxy = |addr: &str| {
        Some(ProxyConfig {
            protocol: ProxyProtocol::Socks5,
            addr: addr.to_owned(),
            port: 1080,
            auth: None,
        })
    };
    let one = vec![("irc.example.".to_owned(), 6667, proxy("first"))];
    assert!(matches!(
        select_proxy(&one, "IRC.EXAMPLE", 6697),
        Ok(ProxySelection::Configured(Some(proxy))) if proxy.addr == "first"
    ));
    assert!(matches!(
        select_proxy(&one, "other.example", 6667),
        Ok(ProxySelection::Unconfigured)
    ));

    let multiple = vec![
        ("irc.example".to_owned(), 6667, proxy("first")),
        ("IRC.EXAMPLE.".to_owned(), 6697, proxy("second")),
        ("irc.example".to_owned(), 7000, None),
    ];
    assert!(matches!(
        select_proxy(&multiple, "irc.example", 6697),
        Ok(ProxySelection::Configured(Some(proxy))) if proxy.addr == "second"
    ));
    assert!(matches!(
        select_proxy(&multiple, "irc.example", 7000),
        Ok(ProxySelection::Configured(None))
    ));
    assert!(select_proxy(&multiple, "irc.example", 1234).is_err());

    let duplicate_port = vec![
        ("irc.example".to_owned(), 6667, proxy("first")),
        ("IRC.EXAMPLE".to_owned(), 6667, None),
    ];
    assert!(select_proxy(&duplicate_port, "irc.example", 6667).is_err());

    let ipv6 = vec![("::1".to_owned(), 6667, proxy("ipv6"))];
    assert!(matches!(
        select_proxy(&ipv6, "0:0:0:0:0:0:0:1", 6667),
        Ok(ProxySelection::Configured(Some(proxy))) if proxy.addr == "ipv6"
    ));
    let ipv6_ports = vec![
        ("::1".to_owned(), 6667, proxy("ipv6")),
        ("0:0:0:0:0:0:0:1".to_owned(), 6697, None),
    ];
    assert!(matches!(
        select_proxy(&ipv6_ports, "::1", 6697),
        Ok(ProxySelection::Configured(None))
    ));
    assert!(select_proxy(&ipv6_ports, "::1", 7000).is_err());
    let ipv4 = vec![("127.0.0.1".to_owned(), 6667, proxy("ipv4"))];
    assert!(matches!(
        select_proxy(&ipv4, "127.0.0.1", 6667),
        Ok(ProxySelection::Configured(Some(proxy))) if proxy.addr == "ipv4"
    ));
    let scoped = vec![("fe80::1%ethA".to_owned(), 6667, proxy("scoped"))];
    assert!(matches!(
        select_proxy(&scoped, "[fe80:0:0:0:0:0:0:1%ethA]", 6667),
        Ok(ProxySelection::Configured(Some(proxy))) if proxy.addr == "scoped"
    ));
    for different in ["fe80::1%etha", "fe80::1%ethB", "fe80::1"] {
        assert!(matches!(
            select_proxy(&scoped, different, 6667),
            Ok(ProxySelection::Unconfigured)
        ));
    }
}

#[test]
fn manual_reconnect_uses_selected_proxy_and_rejects_ambiguity() {
    use libtiny_client::ProxyProtocol;
    use libtiny_tui::TUI;
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};
    use tokio::sync::mpsc;
    use tokio::task::LocalSet;
    use tokio::time::timeout;
    use tokio_stream::{StreamExt, wrappers::ReceiverStream};

    async fn request(socket: &mut TcpStream, port: u16) {
        let mut bytes = Vec::new();
        while !bytes.ends_with(b"\r\n\r\n") {
            bytes.push(socket.read_u8().await.unwrap());
        }
        assert!(
            String::from_utf8(bytes)
                .unwrap()
                .starts_with(&format!("CONNECT 127.0.0.1:{port} HTTP/1."))
        );
    }

    async fn registration(socket: &mut TcpStream) {
        let mut bytes = Vec::new();
        while !bytes.ends_with(b"\r\n") || !bytes.windows(5).any(|window| window == b"USER ") {
            bytes.push(socket.read_u8().await.unwrap());
        }
        assert!(bytes.windows(5).any(|window| window == b"NICK "));
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    LocalSet::new().block_on(&runtime, async {
        timeout(Duration::from_secs(5), async {
            let direct = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let a = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let b = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let make_proxy = |listener: &TcpListener| ProxyConfig {
                protocol: ProxyProtocol::Http,
                addr: "127.0.0.1".to_owned(),
                port: listener.local_addr().unwrap().port(),
                auth: None,
            };
            let proxy_a = make_proxy(&a);
            let proxy_b = make_proxy(&b);
            let direct_port = direct.local_addr().unwrap().port();
            let port_a = if direct_port == 1111 { 1112 } else { 1111 };
            let port_b = if direct_port == 2222 { 2223 } else { 2222 };
            let servers = vec![
                ("127.0.0.1".to_owned(), port_a, Some(proxy_a.clone())),
                ("127.0.0.1".to_owned(), port_b, Some(proxy_b.clone())),
                ("127.0.0.1".to_owned(), direct_port, None),
            ];
            let defaults = Defaults {
                nicks: vec!["smoke".to_owned()],
                realname: "Smoke".to_owned(),
                join: vec![],
                tls: false,
            };
            let (_input_tx, input_rx) = mpsc::channel::<term_input::Event>(1);
            let (tui, _events) = TUI::run_test(80, 10, ReceiverStream::new(input_rx).map(Ok));
            let ui = UI::new(tui, None);
            let mut clients = Vec::new();

            connect_(
                &format!("127.0.0.1:{port_a}"),
                None,
                &defaults,
                &servers,
                &ui,
                &mut clients,
            );
            let (mut first, _) = a.accept().await.unwrap();
            request(&mut first, port_a).await;
            connect_(
                &format!("127.0.0.1:{port_b}"),
                None,
                &defaults,
                &servers,
                &ui,
                &mut clients,
            );
            let (mut second, _) = b.accept().await.unwrap();
            request(&mut second, port_b).await;
            let mut byte = [0];
            assert_eq!(first.read(&mut byte).await.unwrap(), 0);
            second.write_all(b"HTTP/1.1 200 OK\r\n\r\n").await.unwrap();
            registration(&mut second).await;

            connect_("127.0.0.1:4444", None, &defaults, &[], &ui, &mut clients);
            let (mut retained, _) = b.accept().await.unwrap();
            request(&mut retained, 4444).await;
            assert_eq!(second.read(&mut byte).await.unwrap(), 0);
            retained
                .write_all(b"HTTP/1.1 200 OK\r\n\r\n")
                .await
                .unwrap();
            registration(&mut retained).await;

            let ambiguous = vec![
                ("127.0.0.1".to_owned(), 3333, Some(proxy_a)),
                ("127.0.0.1".to_owned(), 3333, Some(proxy_b)),
            ];
            connect_(
                "127.0.0.1:3333",
                None,
                &defaults,
                &ambiguous,
                &ui,
                &mut clients,
            );
            assert!(
                timeout(Duration::from_millis(100), b.accept())
                    .await
                    .is_err()
            );
            assert!(
                timeout(Duration::from_millis(100), a.accept())
                    .await
                    .is_err()
            );

            connect_(
                &format!("127.0.0.1:{direct_port}"),
                None,
                &defaults,
                &servers,
                &ui,
                &mut clients,
            );
            let (mut final_direct, _) = direct.accept().await.unwrap();
            assert_eq!(retained.read(&mut byte).await.unwrap(), 0);
            registration(&mut final_direct).await;
            assert_eq!(clients.len(), 1);
            clients[0].quit(None);
        })
        .await
        .expect("manual reconnect timed out");
    });
}
