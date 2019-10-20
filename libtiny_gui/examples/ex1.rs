use futures_util::stream::StreamExt;
use libtiny_gui::GUI;
use libtiny_ui::*;

fn main() {
    let mut executor = tokio::runtime::current_thread::Runtime::new().unwrap();

    let (gui, mut rcv_ev) = GUI::run();

    executor.spawn(async move {
        loop {
            gui.new_server_tab("Server");
            gui.new_chan_tab("Server", "Chan");
            tokio::timer::delay_for(std::time::Duration::from_secs(3)).await;
            gui.add_client_msg("just tesing", &MsgTarget::Server { serv: "Server" });
        }
    });

    executor.block_on(async move {
        while let Some(ev) = rcv_ev.next().await {
            println!("GUI event received: {:?}", ev);
        }
    });

    executor.run().unwrap(); // unwraps RunError
}
