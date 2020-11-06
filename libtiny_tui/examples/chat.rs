// In a chat window add dozens of nicks, each printing some random lines.

use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

use futures::future::FutureExt;
use futures::select;
use futures::stream::StreamExt;
use tokio::sync::mpsc;

use libtiny_common::ChanNameRef;
use libtiny_tui::TUI;
use libtiny_ui::*;

static SERV: &str = "debug";
static CHAN: &str = "chan";

fn main() {
    let chan_target = MsgTarget::Chan {
        serv: SERV,
        chan: ChanNameRef::new(CHAN),
    };

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let local = tokio::task::LocalSet::new();

    local.block_on(&runtime, async move {
        let (tui, rcv_ev) = TUI::run(PathBuf::from("../tiny/config.yml"));
        tui.new_server_tab("debug", None);
        tui.new_chan_tab("debug", ChanNameRef::new("chan"));
        tui.set_topic(
            "This is channel topic",
            time::now(),
            SERV,
            ChanNameRef::new(CHAN),
        );
        tui.draw();

        {
            let mut text = String::new();
            let mut file = File::open("test/lipsum.txt").unwrap();
            file.read_to_string(&mut text).unwrap();

            for (line_idx, line) in text.lines().enumerate() {
                let now = time::now();
                let nick = format!("nick_{}", line_idx);
                tui.add_nick(&nick, Some(now), &chan_target);
                tui.add_privmsg(&nick, line, now, &chan_target, false, false);
            }
        }

        tui.set_nick(SERV, "some_long_nick_name____");
        tui.draw();

        // For testing purposes, change the nick between short and long nicks every 5 seconds
        let tui_clone = tui.clone();
        let (snd_abort, rcv_abort) = mpsc::channel(1);
        tokio::task::spawn_local(async move {
            let nicks = ["short", "some_long_nick_name____"];
            let mut nick_idx = 1;
            let mut rcv_abort_fused = rcv_abort.fuse();
            loop {
                let mut timer = tokio::time::sleep(std::time::Duration::from_secs(3)).fuse();
                select! {
                    _ = rcv_abort_fused.next() => {
                        break;
                    },
                    () = timer => {
                        tui_clone.set_nick(SERV, nicks[nick_idx]);
                        tui_clone.draw();
                        nick_idx = (nick_idx + 1) % nicks.len();
                        timer = tokio::time::sleep(std::time::Duration::from_secs(3)).fuse();
                    }
                }
            }
        });

        ui_task(tui, rcv_ev, snd_abort).await;
    });

    runtime.block_on(local);
}

async fn ui_task(ui: TUI, mut rcv_ev: mpsc::Receiver<Event>, mut abort: mpsc::Sender<()>) {
    while let Some(ev) = rcv_ev.next().await {
        handle_input_ev(&ui, ev, &mut abort);
        ui.draw();
    }
}

fn handle_input_ev(ui: &TUI, ev: Event, abort: &mut mpsc::Sender<()>) {
    use libtiny_ui::Event::*;
    match ev {
        Cmd { cmd, .. } => {
            let words: Vec<&str> = cmd.split_whitespace().collect();
            if words.len() == 2 && words[0] == "nick" {
                let new_nick = words[1];
                ui.set_nick(SERV, new_nick);
            }
        }
        Abort => {
            abort.try_send(()).unwrap();
        }
        Msg { .. } | Lines { .. } => {}
    }
}
