//! An echo bot that just repeats stuff sent to it (either in a channel or as PRIVMSG).

use libtiny_client::{Client, Event, ServerInfo};
use libtiny_common::ChanNameRef;
use libtiny_wire::{Cmd, Msg, MsgTarget, Pfx};

use futures::stream::StreamExt;
use std::process::exit;

fn main() {
    // echo <nick> <server> <port> [<chan_1> ... <chan_N>]
    let mut args_vec: Vec<String> = std::env::args().collect();
    if args_vec.len() < 4 {
        show_usage();
        exit(1);
    }

    let nick = args_vec.remove(1);
    let server = args_vec.remove(1);
    let port_str = args_vec.remove(1);
    let port = match port_str.parse::<u16>() {
        Ok(port) => port,
        Err(err) => {
            println!("Can't parse port: {:?}", port_str);
            println!("{}", err);
            exit(1);
        }
    };

    let chans = args_vec[1..]
        .iter()
        .map(|c| ChanNameRef::new(c).to_owned())
        .collect::<Vec<_>>();

    let server_info = ServerInfo {
        addr: server,
        port,
        tls: false,
        pass: None,
        realname: "tiny echo bot".to_owned(),
        nicks: vec![nick],
        auto_join: chans.to_owned(),
        nickserv_ident: None,
        sasl_auth: None,
    };

    println!("{:?}", server_info);

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = tokio::task::LocalSet::new();
    local.block_on(&runtime, echo_bot_task(server_info));
}

fn show_usage() {
    println!("echo <nick> <server> <port> [<chan_1> .. <chan_N>]");
}

static NICK_SEP: [&str; 4] = [": ", ", ", ":", ","];

async fn echo_bot_task(server_info: ServerInfo) {
    let (mut client, mut rcv_ev) = Client::new(server_info);

    while let Some(ev) = rcv_ev.next().await {
        println!("Client event: {:?}", ev);
        if let Event::Msg(Msg {
            pfx: Some(Pfx::User { nick, .. }),
            cmd: Cmd::PRIVMSG { target, msg, .. },
        }) = ev
        {
            let echo_msg = match target {
                MsgTarget::User(_) => {
                    // Message is a PRIVMSG to us, just echo the whole message to the sender
                    Some((nick, msg))
                }
                MsgTarget::Chan(chan) => {
                    // Message was sent to a channel. Only echo if it's directed at us
                    let our_nick = client.get_nick();
                    if msg.starts_with(&our_nick) {
                        let mut msg = &msg[our_nick.len()..];
                        for nick_sep in NICK_SEP.iter() {
                            if msg.starts_with(nick_sep) {
                                msg = &msg[nick_sep.len()..];
                                break;
                            }
                        }
                        Some((chan.display().to_owned(), msg.to_owned()))
                    } else {
                        None
                    }
                }
            };

            if let Some((target, msg)) = echo_msg {
                client.privmsg(&target, &msg, false);
            }
        }
    }
}
