use libtiny_gui::GUI;
use libtiny_ui::*;
use futures_util::stream::StreamExt;

fn main() {
    let mut executor = tokio::runtime::current_thread::Runtime::new().unwrap();

    let (gui, mut rcv_ev) = GUI::run();

    executor.block_on(async move {
        for ev in rcv_ev.next().await {

        }
    });

    executor.run().unwrap(); // unwraps RunError
}
