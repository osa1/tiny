use futures_util::stream::StreamExt;
use libtiny_gui::GUI;
use libtiny_ui::*;

fn main() {
    let mut executor = tokio::runtime::current_thread::Runtime::new().unwrap();

    let (gui, mut rcv_ev) = GUI::run();

    executor.spawn(async move {
        loop {
            tokio::timer::delay_for(std::time::Duration::from_secs(1)).await;
            gui.new_server_tab("Just testing");
        }
    });

    executor.block_on(async move { for ev in rcv_ev.next().await {} });

    executor.run().unwrap(); // unwraps RunError
}
