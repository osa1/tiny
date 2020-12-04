use crate::conn;
use libtiny_client::Event;
use libtiny_tui::tui::CellBuf;
use libtiny_tui::{TUI, UI};
use libtiny_wire::{Cmd, Msg, MsgTarget, Pfx};
use term_input;

use tokio::stream::StreamExt;
use tokio::sync::mpsc;

struct TestClient;

impl conn::Client for TestClient {
    fn get_serv_name(&self) -> &str {
        "chat.myserver.net"
    }

    fn get_nick(&self) -> String {
        "osa1".to_owned()
    }

    fn is_nick_accepted(&self) -> bool {
        true
    }
}

#[test]
fn test_setup() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = tokio::task::LocalSet::new();

    local.block_on(&runtime, async move {
        // Create test TUI
        let (snd_input_ev, rcv_input_ev) = mpsc::channel::<term_input::Event>(100);
        let (tui, _rcv_tui_ev) = TUI::run_test(40, 20, rcv_input_ev.map(|ev| Ok(ev)));

        let tui = Box::new(tui);

        // Create test connection event channel
        let (snd_conn_ev, rcv_conn_ev) = mpsc::channel::<Event>(100);

        // Spawn connection event handler task
        tokio::task::spawn_local(conn::task(rcv_conn_ev, tui.clone(), Box::new(TestClient)));

        tui.new_server_tab("chat.myserver.net", None);
        tui.draw();

        snd_input_ev
            .send(term_input::Event::Key(term_input::Key::Ctrl('n')))
            .await
            .unwrap();

        snd_conn_ev.send(Event::Connected).await.unwrap();

        snd_conn_ev
            .send(Event::Msg(Msg {
                pfx: Some(Pfx::User {
                    nick: "e".to_owned(),
                    user: "e@a/b/c.d".to_owned(),
                }),
                cmd: Cmd::PRIVMSG {
                    target: MsgTarget::User("$$*".to_owned()),
                    msg: "blah blah blah".to_owned(),
                    is_notice: true,
                    ctcp: None,
                },
            }))
            .await
            .unwrap();

        tokio::task::yield_now().await;

        tui.draw();

        let tui_buf = tui.get_front_buffer();
        println!("{}", buffer_str(&tui_buf, 40, 20));
    });
}

// TODO: Copied from TUI tests
fn buffer_str(buf: &CellBuf, w: u16, h: u16) -> String {
    let w = usize::from(w);
    let h = usize::from(h);

    let mut ret = String::with_capacity(w * h);

    for y in 0..h {
        for x in 0..w {
            let ch = buf.cells[(y * usize::from(w)) + x].ch;
            ret.push(ch);
        }
        if y != h - 1 {
            ret.push('\n');
        }
    }

    ret
}
