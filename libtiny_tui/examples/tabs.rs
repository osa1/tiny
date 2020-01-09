// Open a lot of tabs. 10 servers tabs, each one having 3 channels.

use futures::stream::StreamExt;
use libtiny_tui::TUI;
use libtiny_ui::*;
use std::path::PathBuf;
use tokio::sync::mpsc;

fn main() {
    let mut runtime = tokio::runtime::Builder::new()
        .basic_scheduler()
        .enable_all()
        .build()
        .unwrap();

    let local = tokio::task::LocalSet::new();

    local.block_on(&mut runtime, async move {
        let (tui, rcv_ev) = TUI::run(PathBuf::from("../tiny/config.yml"));

        for serv_idx in 0..10 {
            let server = format!("server_{}", serv_idx);
            tui.new_server_tab(&server);

            tui.new_chan_tab(&server, "chan_0");
            tui.set_tab_style(
                TabStyle::NewMsg,
                &MsgTarget::Chan {
                    serv: &server,
                    chan: "chan_0",
                },
            );

            tui.new_chan_tab(&server, "chan_1");
            tui.set_tab_style(
                TabStyle::Highlight,
                &MsgTarget::Chan {
                    serv: &server,
                    chan: "chan_1",
                },
            );

            tui.new_chan_tab(&server, "chan_2");
        }

        tui.draw();

        ui_task(tui, rcv_ev).await;
    });

    runtime.block_on(local);
}

async fn ui_task(ui: TUI, mut rcv_ev: mpsc::Receiver<Event>) {
    while let Some(ev) = rcv_ev.next().await {
        handle_input_ev(&ui, ev);
        ui.draw();
    }
}

fn handle_input_ev(ui: &TUI, ev: Event) {
    use libtiny_ui::Event::*;
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
