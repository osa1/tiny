use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

use libtiny_common::MsgTarget;
use libtiny_tui::TUI;

fn main() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let local = tokio::task::LocalSet::new();

    local.block_on(&runtime, async move {
        let (tui, _) = TUI::run(PathBuf::from("../tiny/config.yml"));
        tui.new_server_tab("debug", None);
        let debug_tab = MsgTarget::Server { serv: "debug" };

        tui.add_msg(
            "Loading word list for auto-completion ...",
            time::now(),
            &debug_tab,
        );
        tui.draw();

        {
            let mut contents = String::new();
            let mut file = File::open("/usr/share/dict/american-english").unwrap();
            file.read_to_string(&mut contents).unwrap();
            for word in contents.lines() {
                tui.add_nick(word, None, &debug_tab);
            }
        }

        tui.add_msg("Done.", time::now(), &debug_tab);
        tui.draw();
    });

    runtime.block_on(local);
}
