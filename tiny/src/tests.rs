use libtiny_client::Event;
use libtiny_tui::TUI;

use tokio::sync::mpsc;

#[test]
fn test_setup() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = tokio::task::LocalSet::new();

    local.block_on(&runtime, async move {
        // Create test TUI
        let (tui, rcv_tui_ev) = TUI::run_test(20, 20);

        // Create test connection event channel
        let (snd_conn_ev, rcv_conn_ev) = mpsc::channel::<Event>(100);

        // Spawn connection event handler task
        // TODO: We can't do the without creating a Client. We'll probably need a trait for
        // clients and make `conn:task` parametric on clients.
    });
}
