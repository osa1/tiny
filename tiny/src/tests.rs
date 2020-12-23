use crate::conn;
use crate::ui::UI;
use libtiny_common::ChanName;
use libtiny_tui::test_utils::expect_screen;
use libtiny_tui::TUI;
use libtiny_wire::{Cmd, Msg, MsgTarget, Pfx};
use term_input;
use termbox_simple::CellBuf;

use libtiny_client as client;
use term_input as input;

use tokio::stream::StreamExt;
use tokio::sync::mpsc;

use std::future::Future;
use std::panic::Location;

struct TestClient {
    nick: String,
}

impl conn::Client for TestClient {
    fn get_serv_name(&self) -> &str {
        SERV_NAME
    }

    fn get_nick(&self) -> String {
        self.nick.clone()
    }

    fn is_nick_accepted(&self) -> bool {
        true
    }
}

static SERV_NAME: &str = "x.y.z";
const DEFAULT_TUI_WIDTH: u16 = 40;
const DEFAULT_TUI_HEIGHT: u16 = 5;

struct TestSetup {
    /// TUI test instance
    tui: TUI,
    /// Send input events to the TUI using this channel
    snd_input_ev: mpsc::Sender<input::Event>,
    /// Send connection events to connection handler (`conn::task`) using this channel
    snd_conn_ev: mpsc::Sender<client::Event>,
}

fn run_test<F, Fut>(nick: String, test: F)
where
    F: Fn(TestSetup) -> Fut,
    Fut: Future<Output = ()>,
{
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = tokio::task::LocalSet::new();

    local.block_on(&runtime, async move {
        // Create test TUI
        let (snd_input_ev, rcv_input_ev) = mpsc::channel::<term_input::Event>(100);
        let (tui, _rcv_tui_ev) = TUI::run_test(
            DEFAULT_TUI_WIDTH,
            DEFAULT_TUI_HEIGHT,
            rcv_input_ev.map(|ev| Ok(ev)),
        );

        let tiny_ui = UI::new(tui.clone(), None);

        // Create test connection event channel
        let (snd_conn_ev, rcv_conn_ev) = mpsc::channel::<client::Event>(100);

        // Spawn connection event handler task
        tokio::task::spawn_local(conn::task(
            rcv_conn_ev,
            tiny_ui,
            Box::new(TestClient { nick }),
        ));

        tui.new_server_tab(SERV_NAME, None);
        tui.draw();

        test(TestSetup {
            tui,
            snd_input_ev,
            snd_conn_ev,
        })
        .await;
    });
}

#[test]
fn test_privmsg_from_user_without_user_or_host_part_issue_247() {
    run_test(
        "osa1".to_owned(),
        |TestSetup {
             tui,
             snd_input_ev,
             snd_conn_ev,
         }| async move {
            snd_conn_ev.send(client::Event::Connected).await.unwrap();
            snd_conn_ev
                .send(client::Event::NickChange {
                    new_nick: "osa1".to_owned(),
                })
                .await
                .unwrap();
            yield_(5).await;

            // Join a channel to test msg sent to channel
            let join = Msg {
                pfx: Some(Pfx::User {
                    nick: "osa1".to_owned(),
                    user: "a@b".to_owned(),
                }),
                cmd: Cmd::JOIN {
                    chan: ChanName::new("#chan".to_owned()),
                },
            };
            snd_conn_ev.send(client::Event::Msg(join)).await.unwrap();
            yield_(5).await;

            // Send a PRIVMSG to the channel
            let chan_msg = Msg {
                pfx: Some(Pfx::Ambiguous("blah".to_owned())),
                cmd: Cmd::PRIVMSG {
                    target: MsgTarget::Chan(ChanName::new("#chan".to_owned())),
                    msg: "msg to chan".to_owned(),
                    is_notice: false,
                    ctcp: None,
                },
            };
            snd_conn_ev
                .send(client::Event::Msg(chan_msg))
                .await
                .unwrap();
            yield_(5).await;

            // Send a PRIVMSG to current nick
            let msg = Msg {
                pfx: Some(Pfx::Ambiguous("blah".to_owned())),
                cmd: Cmd::PRIVMSG {
                    target: MsgTarget::User("osa1".to_owned()),
                    msg: "msg to user".to_owned(),
                    is_notice: false,
                    ctcp: None,
                },
            };
            snd_conn_ev.send(client::Event::Msg(msg)).await.unwrap();
            yield_(5).await;

            // Check channel tab
            next_tab(&snd_input_ev).await; // server tab
            next_tab(&snd_input_ev).await; // channel tab
            yield_(5).await;
            tui.draw();

            #[rustfmt::skip]
            let screen =
            "|                                        |
             |                                        |
             |00:00 blah: msg to chan                 |
             |osa1:                                   |
             |mentions x.y.z #chan blah               |";

            let mut front_buffer = tui.get_front_buffer();
            normalize_timestamps(&mut front_buffer, DEFAULT_TUI_WIDTH, DEFAULT_TUI_HEIGHT);
            expect_screen(
                screen,
                &front_buffer,
                DEFAULT_TUI_WIDTH,
                DEFAULT_TUI_HEIGHT,
                Location::caller(),
            );

            // Check privmsg tab
            next_tab(&snd_input_ev).await; // privmsg tab
            yield_(5).await;
            tui.draw();

            #[rustfmt::skip]
            let screen =
            "|                                        |
             |                                        |
             |00:00 blah: msg to user                 |
             |osa1:                                   |
             |mentions x.y.z #chan blah               |";

            let mut front_buffer = tui.get_front_buffer();
            normalize_timestamps(&mut front_buffer, DEFAULT_TUI_WIDTH, DEFAULT_TUI_HEIGHT);
            expect_screen(
                screen,
                &front_buffer,
                DEFAULT_TUI_WIDTH,
                DEFAULT_TUI_HEIGHT,
                Location::caller(),
            );
        },
    )
}

#[test]
fn test_bouncer_relay_issue_271() {
    run_test(
        "osa1-soju".to_owned(),
        |TestSetup {
             tui,
             snd_input_ev,
             snd_conn_ev,
         }| async move {
            snd_conn_ev.send(client::Event::Connected).await.unwrap();
            snd_conn_ev
                .send(client::Event::NickChange {
                    new_nick: "osa1-soju".to_owned(),
                })
                .await
                .unwrap();

            let msg = Msg {
                pfx: Some(Pfx::User {
                    nick: "osa1-soju".to_owned(),
                    user: "osa1-soju@127.0.0.1".to_owned(),
                }),
                cmd: Cmd::PRIVMSG {
                    target: MsgTarget::User("osa1/oftc".to_owned()),
                    msg: "blah blah".to_owned(),
                    is_notice: false,
                    ctcp: None,
                },
            };

            snd_conn_ev.send(client::Event::Msg(msg)).await.unwrap();

            yield_(5).await;
            tui.draw();

            next_tab(&snd_input_ev).await; // server tab
            next_tab(&snd_input_ev).await; // privmsg tab
            yield_(5).await;

            tui.draw();

            #[rustfmt::skip]
            let screen =
            "|                                        |
             |                                        |
             |00:00 osa1-soju: blah blah              |
             |osa1-soju:                              |
             |mentions x.y.z osa1/oftc                |";

            let mut front_buffer = tui.get_front_buffer();
            normalize_timestamps(&mut front_buffer, DEFAULT_TUI_WIDTH, DEFAULT_TUI_HEIGHT);
            expect_screen(
                screen,
                &front_buffer,
                DEFAULT_TUI_WIDTH,
                DEFAULT_TUI_HEIGHT,
                Location::caller(),
            );
        },
    )
}

#[test]
fn test_privmsg_targetmask_issue_278() {
    run_test(
        "osa1".to_owned(),
        |TestSetup {
             tui,
             snd_input_ev,
             snd_conn_ev,
         }| async move {
            next_tab(&snd_input_ev).await;
            snd_conn_ev.send(client::Event::Connected).await.unwrap();
            snd_conn_ev
                .send(client::Event::NickChange {
                    new_nick: "osa1".to_owned(),
                })
                .await
                .unwrap();

            snd_conn_ev
                .send(client::Event::Msg(Msg {
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

            yield_(3).await;

            next_tab(&snd_input_ev).await;

            tui.draw();

            yield_(3).await;

            #[rustfmt::skip]
            let screen =
            "|                                        |
             |                                        |
             |00:00 e: blah blah blah                 |
             |osa1:                                   |
             |mentions x.y.z e                        |";

            let mut front_buffer = tui.get_front_buffer();
            normalize_timestamps(&mut front_buffer, DEFAULT_TUI_WIDTH, DEFAULT_TUI_HEIGHT);
            expect_screen(
                screen,
                &front_buffer,
                DEFAULT_TUI_WIDTH,
                DEFAULT_TUI_HEIGHT,
                Location::caller(),
            );
        },
    )
}

async fn next_tab(snd_input_ev: &mpsc::Sender<input::Event>) {
    snd_input_ev
        .send(term_input::Event::Key(term_input::Key::Ctrl('n')))
        .await
        .unwrap();
}

async fn yield_(n: usize) {
    for _ in 0..n {
        tokio::task::yield_now().await;
    }
}

/// Makes all timestamps 00:00
fn normalize_timestamps(cells: &mut CellBuf, w: u16, h: u16) {
    let cells = &mut cells.cells;
    for y in 0..h {
        let x = (w * y) as usize;
        if cells[x].ch.is_ascii_digit()
            && cells[x + 1].ch.is_ascii_digit()
            && cells[x + 2].ch == ':'
            && cells[x + 3].ch.is_ascii_digit()
            && cells[x + 4].ch.is_ascii_digit()
        {
            cells[x].ch = '0';
            cells[x + 1].ch = '0';
            cells[x + 2].ch = ':';
            cells[x + 3].ch = '0';
            cells[x + 4].ch = '0';
        }
    }
}
