// I think borrowed boxing is necessary for objekt::clone_box to work
#![allow(clippy::borrowed_box)]

//! UI event handling

use crate::cmd::{parse_cmd, CmdArgs, ParseCmdResult};
use crate::config;
use futures::stream::StreamExt;
use libtiny_client::Client;
use libtiny_common::{ChanNameRef, MsgSource, MsgTarget, TabStyle};
use libtiny_logger::Logger;
use libtiny_tui::TUI;
use std::path::{Path, PathBuf};
use time::Tm;
use tokio::sync::mpsc;

macro_rules! delegate {
    ( $name:ident ( $( $x:ident: $t:ty, )* )) => {
        pub(crate) fn $name(&self, $($x: $t,)*) {
            self.ui.$name( $( $x, )* );
            if let Some(logger) = &self.logger {
                logger.$name( $( $x, )* );
            }
        }
    }
}

macro_rules! delegate_ui {
    ( $name:ident ( $( $x:ident: $t:ty, )* ) $(->$ret:ty )? ) => {
        pub(crate) fn $name(&self, $($x: $t,)*) $(-> $ret)? {
            self.ui.$name( $( $x, )* )
        }
    }
}

#[derive(Clone)]
pub(crate) struct UI {
    ui: TUI,
    logger: Option<Logger>,
}

impl UI {
    pub(crate) fn new(ui: TUI, logger: Option<Logger>) -> UI {
        UI { ui, logger }
    }

    pub(crate) fn new_server_tab(&self, serv_name: &str, alias: Option<String>) {
        self.ui.new_server_tab(serv_name, alias);
        if let Some(logger) = &self.logger {
            logger.new_server_tab(serv_name);
        }
    }

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

    delegate_ui!(draw());
    delegate_ui!(add_err_msg(msg: &str, ts: Tm, target: &MsgTarget,));
    delegate_ui!(add_client_err_msg(msg: &str, target: &MsgTarget,));
    delegate_ui!(clear_nicks(serv: &str,));
    delegate_ui!(set_nick(serv: &str, nick: &str,));
    delegate_ui!(set_tab_style(style: TabStyle, target: &MsgTarget,));
    delegate_ui!(user_tab_exists(serv_name: &str, nick: &str,) -> bool);
}

pub(crate) async fn task(
    config_path: PathBuf,
    defaults: config::Defaults,
    ui: UI,
    mut clients: Vec<Client>,
    mut rcv_ev: mpsc::Receiver<libtiny_common::Event>,
) {
    while let Some(ev) = rcv_ev.next().await {
        handle_input_ev(&config_path, &defaults, &ui, &mut clients, ev);
        ui.draw();
    }
}

fn handle_input_ev(
    config_path: &Path,
    defaults: &config::Defaults,
    ui: &UI,
    clients: &mut Vec<Client>,
    ev: libtiny_common::Event,
) {
    use libtiny_common::Event::*;
    match ev {
        Abort => {
            for client in clients {
                client.quit(None);
            }
        }
        Msg { msg, source } => {
            send_msg(ui, clients, &source, msg, false);
        }
        Lines { lines, source } => {
            for line in lines.into_iter() {
                send_msg(ui, clients, &source, line, false)
            }
        }
        Cmd { cmd, source } => handle_cmd(config_path, defaults, ui, clients, source, &cmd),
    }
}

fn handle_cmd(
    config_path: &Path,
    defaults: &config::Defaults,
    ui: &UI,
    clients: &mut Vec<Client>,
    src: MsgSource,
    cmd: &str,
) {
    match parse_cmd(cmd) {
        ParseCmdResult::Ok { cmd, rest } => {
            let cmd_args = CmdArgs {
                args: rest,
                config_path,
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
    ui: &UI,
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
    let (ui_target, msg_target): (MsgTarget, &str) = {
        match src {
            MsgSource::Serv { .. } => {
                // we don't split raw messages to 512-bytes long chunks
                client.raw_msg(&msg);
                return;
            }

            MsgSource::Chan { ref serv, ref chan } => {
                (MsgTarget::Chan { serv, chan }, chan.display())
            }

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
