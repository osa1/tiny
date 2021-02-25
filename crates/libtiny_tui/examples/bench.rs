// A program that initializes the TUI and adds lines in the file (given as first argument) to it,
// with a draw() call after every line added.
//
// After adding all lines the program just quits.
//
// Useful for benchmarking TUI::draw().

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use libtiny_common::MsgTarget;
use libtiny_tui::TUI;

fn main() {
    run_bench();

    let mut rusage: libc::rusage = unsafe { ::std::mem::zeroed() };
    match unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut rusage as *mut _) } {
        0 => {
            println!("Max RSS (ru_maxrss): {} kb", rusage.ru_maxrss);
        }
        i => {
            println!("getrusage() returned {}", i);
        }
    }
}

fn run_bench() {
    let args = std::env::args().collect::<Vec<_>>();
    let file_path = &args[1];
    let file = File::open(file_path).unwrap();
    let file_buffered = BufReader::new(file);
    let lines = file_buffered.lines().map(Result::unwrap).collect();

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let local = tokio::task::LocalSet::new();

    local.block_on(&runtime, async move {
        let (tui, _) = TUI::run(PathBuf::from("../tiny/config.yml"));
        tui.draw();
        bench_task(tui, lines).await;
    });
}

async fn bench_task(tui: TUI, lines: Vec<String>) {
    let msg_target = MsgTarget::Server { serv: "mentions" };
    let time = time::now();

    for line in &lines {
        tui.add_privmsg("server", line, time, &msg_target, false, false);
        tui.draw();
    }
}
