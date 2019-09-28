//! UI event handling

use crate::cmd::{parse_cmd, CmdArgs, ParseCmdResult};
use crate::config;
use futures_util::stream::StreamExt;
use libtiny_client::Client;
use libtiny_tui::MsgTarget;
use libtiny_tui::{MsgSource, TUI};
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

pub(crate) async fn task(
    config_path: PathBuf,
    log_dir: Option<PathBuf>,
    defaults: config::Defaults,
    tui: TUI,
    mut clients: Vec<Client>,
    mut rcv_ev: mpsc::Receiver<libtiny_tui::Event>,
) {
    while let Some(ev) = rcv_ev.next().await {
        if handle_input_ev(&config_path, &log_dir, &defaults, &tui, &mut clients, ev) {
            return;
        }
        tui.draw();
    }
}

fn handle_input_ev(
    config_path: &Path,
    log_dir: &Option<PathBuf>,
    defaults: &config::Defaults,
    tui: &TUI,
    clients: &mut Vec<Client>,
    ev: libtiny_tui::Event,
) -> bool {
    use libtiny_tui::Event::*;
    match ev {
        Abort => {
            for client in clients {
                client.quit(None);
            }
            return true; // abort
        }
        Msg { msg, source } => {
            send_msg(tui, clients, &source, msg, false);
        }
        Lines { lines, source } => {
            for line in lines.into_iter() {
                send_msg(tui, clients, &source, line, false)
            }
        }
        Cmd { cmd, source } => {
            handle_cmd(config_path, log_dir, defaults, tui, clients, source, &cmd)
        }
    }

    false // continue
}

fn handle_cmd(
    config_path: &Path,
    log_dir: &Option<PathBuf>,
    defaults: &config::Defaults,
    tui: &TUI,
    clients: &mut Vec<Client>,
    src: MsgSource,
    cmd: &str,
) {
    match parse_cmd(cmd) {
        ParseCmdResult::Ok { cmd, rest } => {
            let cmd_args = CmdArgs {
                args: rest,
                config_path,
                log_dir,
                defaults,
                tui,
                clients,
                src,
            };
            (cmd.cmd_fn)(cmd_args);
        }
        // ParseCmdResult::Ambiguous(vec) => {
        //     self.tui.add_client_err_msg(
        //         &format!("Unsupported command: \"/{}\"", msg),
        //         &MsgTarget::CurrentTab,
        //     );
        //     self.tui.add_client_err_msg(
        //         &format!("Did you mean one of {:?} ?", vec),
        //         &MsgTarget::CurrentTab,
        //     );
        // },
        ParseCmdResult::Unknown => tui.add_client_err_msg(
            &format!("Unsupported command: \"/{}\"", cmd),
            &MsgTarget::CurrentTab,
        ),
    }
}

// TODO: move this somewhere else
pub(crate) fn send_msg(
    tui: &TUI,
    clients: &mut Vec<Client>,
    src: &MsgSource,
    msg: String,
    is_action: bool,
) {
    if src.serv_name() == "mentions" {
        tui.add_client_err_msg(
            "Use `/connect <server>` to connect to a server",
            &MsgTarget::CurrentTab,
        );
        return;
    }

    let client = clients
        .iter_mut()
        .find(|client| client.get_serv_name() == src.serv_name())
        .unwrap();

    // TODO: For errors:
    //
    // tui.add_client_err_msg(
    //     &format!("Can't find server: {}", serv),
    //     &MsgTarget::CurrentTab,
    // );

    // `tui_target`: Where to show the message on TUI
    // `msg_target`: Actual PRIVMSG target to send to the server
    let (tui_target, msg_target) = {
        match src {
            MsgSource::Serv { .. } => {
                // we don't split raw messages to 512-bytes long chunks
                client.raw_msg(&msg);
                return;
            }

            MsgSource::Chan { ref serv, ref chan } => (MsgTarget::Chan { serv, chan }, chan),

            MsgSource::User { ref serv, ref nick } => {
                let msg_target = if nick.eq_ignore_ascii_case("nickserv")
                    || nick.eq_ignore_ascii_case("chanserv")
                {
                    MsgTarget::Server { serv }
                } else {
                    MsgTarget::User { serv, nick }
                };
                (msg_target, nick)
            }
        }
    };

    let ts = time::now();
    let extra_len = msg_target.len()
        + if is_action {
            9 // "\0x1ACTION \0x1".len()
        } else {
            0
        };
    for msg in client.split_privmsg(extra_len, &msg) {
        client.privmsg(msg_target, msg, is_action);
        tui.add_privmsg(&client.get_nick(), msg, ts, &tui_target, false, is_action);
    }
}
