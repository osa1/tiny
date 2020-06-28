use futures::stream::StreamExt;
use libtiny_tui::TUI;
use libtiny_ui::*;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use tokio::sync::mpsc;

fn main() {
    let mut runtime = tokio::runtime::Builder::new()
        .basic_scheduler()
        .enable_all()
        .build()
        .unwrap();

    let local = tokio::task::LocalSet::new();

    let tab = MsgTarget::Server { serv: "mentions" };

    local.block_on(&mut runtime, async move {
        let (tui, rcv_ev) = TUI::run(PathBuf::from("../tiny/config.yml"));

        let mut text = String::new();
        let mut file = File::open("test/lipsum.txt").unwrap();
        file.read_to_string(&mut text).unwrap();

        let single_line_text = text.lines().collect::<Vec<&str>>().join("");
        tui.add_client_msg(&single_line_text, &tab);

        for line in text.lines() {
            tui.add_client_msg(&format!(">>>  {}", line), &tab);
        }

        tui.draw();

        ui_task(tui, rcv_ev).await;
    });

    runtime.block_on(local);
}

async fn ui_task(ui: TUI, mut rcv_ev: mpsc::Receiver<Event>) {
    while let Some(_) = rcv_ev.next().await {
        ui.draw();
    }
}
