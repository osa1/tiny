// Open a lot of tabs. 10 servers tabs, each one having 3 channels.

use std::path::PathBuf;

use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;

use libtiny_common::{ChanNameRef, Event, MsgSource, MsgTarget, TabStyle};
use libtiny_tui::TUI;

fn main() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let local = tokio::task::LocalSet::new();

    local.block_on(&runtime, async move {
        let (tui, rcv_ev) = TUI::run(PathBuf::from("../tiny/config.yml"));

        for serv_idx in 0..10 {
            let server = format!("server_{}", serv_idx);
            tui.new_server_tab(&server, None);

            tui.new_chan_tab(&server, ChanNameRef::new("chan_0"));
            tui.set_tab_style(
                TabStyle::NewMsg,
                &MsgTarget::Chan {
                    serv: &server,
                    chan: ChanNameRef::new("chan_0"),
                },
            );

            tui.new_chan_tab(&server, ChanNameRef::new("chan_1"));
            tui.set_tab_style(
                TabStyle::Highlight,
                &MsgTarget::Chan {
                    serv: &server,
                    chan: ChanNameRef::new("chan_1"),
                },
            );

            tui.new_chan_tab(&server, ChanNameRef::new("chan_2"));
        }

        tui.draw();

        ui_task(tui, rcv_ev).await;
    });

    runtime.block_on(local);
}

async fn ui_task(ui: TUI, rcv_ev: mpsc::Receiver<Event>) {
    let mut rcv_ev = ReceiverStream::new(rcv_ev);
    while let Some(ev) = rcv_ev.next().await {
        handle_input_ev(&ui, ev);
        ui.draw();
    }
}

fn handle_input_ev(ui: &TUI, ev: Event) {
    use libtiny_common::Event::*;
    match ev {
        Cmd { cmd, source } => {
            if cmd == "close" {
                match source {
                    MsgSource::Serv { serv } => {
                        ui.close_server_tab(&serv);
                    }
                    MsgSource::Chan { serv, chan } => {
                        ui.close_chan_tab(&serv, &chan);
                    }
                    MsgSource::User { serv, nick } => {
                        ui.close_user_tab(&serv, &nick);
                    }
                }
            }
        }
        Abort | Msg { .. } | Lines { .. } => {}
    }
}
