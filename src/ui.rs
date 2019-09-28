//! UI event handling

use crate::cmd::{parse_cmd, CmdArgs, ParseCmdResult};
use crate::config;
use futures_util::stream::StreamExt;
use libtiny_client::Client;
use libtiny_ui::{MsgSource, MsgTarget, UI};
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

pub(crate) async fn task(
    config_path: PathBuf,
    log_dir: Option<PathBuf>,
    defaults: config::Defaults,
    ui: impl UI,
    mut clients: Vec<Client>,
    mut rcv_ev: mpsc::Receiver<libtiny_ui::Event>,
) {
    while let Some(ev) = rcv_ev.next().await {
        if handle_input_ev(&config_path, &log_dir, &defaults, &ui, &mut clients, ev) {
            return;
        }
        ui.draw();
    }
}

fn handle_input_ev(
    config_path: &Path,
    log_dir: &Option<PathBuf>,
    defaults: &config::Defaults,
    ui: &dyn UI,
    clients: &mut Vec<Client>,
    ev: libtiny_ui::Event,
) -> bool {
    use libtiny_ui::Event::*;
    match ev {
        Abort => {
            for client in clients {
                client.quit(None);
            }
            return true; // abort
        }
        Msg { msg, source } => {
            send_msg(ui, clients, &source, msg, false);
        }
        Lines { lines, source } => {
            for line in lines.into_iter() {
                send_msg(ui, clients, &source, line, false)
            }
        }
        Cmd { cmd, source } => {
            handle_cmd(config_path, log_dir, defaults, ui, clients, source, &cmd)
        }
    }

    false // continue
}

fn handle_cmd(
    config_path: &Path,
    log_dir: &Option<PathBuf>,
    defaults: &config::Defaults,
    ui: &dyn UI,
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
                ui,
                clients,
                src,
            };
            (cmd.cmd_fn)(cmd_args);
        }
        // ParseCmdResult::Ambiguous(vec) => {
        //     self.ui.add_client_err_msg(
        //         &format!("Unsupported command: \"/{}\"", msg),
        //         &MsgTarget::CurrentTab,
        //     );
        //     self.ui.add_client_err_msg(
        //         &format!("Did you mean one of {:?} ?", vec),
        //         &MsgTarget::CurrentTab,
        //     );
        // },
        ParseCmdResult::Unknown => ui.add_client_err_msg(
            &format!("Unsupported command: \"/{}\"", cmd),
            &MsgTarget::CurrentTab,
        ),
    }
}

// TODO: move this somewhere else
pub(crate) fn send_msg(
    ui: &dyn UI,
    clients: &mut Vec<Client>,
    src: &MsgSource,
    msg: String,
    is_action: bool,
) {
    if src.serv_name() == "mentions" {
        ui.add_client_err_msg(
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
    // ui.add_client_err_msg(
    //     &format!("Can't find server: {}", serv),
    //     &MsgTarget::CurrentTab,
    // );

    // `ui_target`: Where to show the message on ui
    // `msg_target`: Actual PRIVMSG target to send to the server
    let (ui_target, msg_target) = {
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
        ui.add_privmsg(&client.get_nick(), msg, ts, &ui_target, false, is_action);
    }
}
