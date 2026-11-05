use crate::conn;
use crate::ui::UI;
use libtiny_common::ChanName;
use libtiny_common::MsgSource;
use libtiny_tui::TUI;
use libtiny_tui::test_utils::{buffer_str, expect_screen};
use libtiny_wire::{Cmd, Msg, MsgTarget, Pfx};

use termbox_simple::CellBuf;

use libtiny_client as client;
use term_input as input;

use std::future::Future;
use std::panic::Location;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream;

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
        let rcv_input_ev = ReceiverStream::new(rcv_input_ev);
        let (tui, _rcv_tui_ev) =
            TUI::run_test(DEFAULT_TUI_WIDTH, DEFAULT_TUI_HEIGHT, rcv_input_ev.map(Ok));

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
fn ipv6_connect_and_reconnect() {
    run_test(
        "testnick".to_owned(),
        |TestSetup {
             tui, snd_input_ev, ..
         }| async move {
            let first = match TcpListener::bind("[::1]:0").await {
                Ok(listener) => listener,
                Err(err)
                    if matches!(
                        err.kind(),
                        std::io::ErrorKind::AddrNotAvailable | std::io::ErrorKind::Unsupported
                    ) || matches!(err.raw_os_error(), Some(93 | 97)) =>
                {
                    eprintln!("Skipping IPv6 loopback test: {err}");
                    return;
                }
                Err(err) => panic!("IPv6 loopback bind failed: {err}"),
            };
            let second = TcpListener::bind("[::1]:0").await.unwrap();
            let defaults = crate::config::Defaults {
                nicks: vec!["testnick".to_owned()],
                realname: "Test User".to_owned(),
                join: vec![],
                tls: false,
            };
            let ui = UI::new(tui.clone(), None);
            let mut clients = Vec::new();
            let source = MsgSource::Serv {
                serv: SERV_NAME.to_owned(),
            };

            crate::cmd::run_cmd(
                &format!(
                    "connect [::1]:{} testpass",
                    first.local_addr().unwrap().port()
                ),
                source.clone(),
                &defaults,
                &[],
                &ui,
                &mut clients,
            );
            let first_stream = accept_registration(&first).await;
            assert_eq!(clients.len(), 1);
            assert_eq!(clients[0].get_serv_name(), "::1");

            crate::cmd::run_cmd(
                &format!("connect [::1]:{}", second.local_addr().unwrap().port()),
                source,
                &defaults,
                &[],
                &ui,
                &mut clients,
            );
            let _second_stream = accept_registration(&second).await;
            assert_eq!(clients.len(), 1);
            assert_eq!(clients[0].get_serv_name(), "::1");

            tui.draw();
            let cells = tui.get_front_buffer();
            let start = usize::from(DEFAULT_TUI_WIDTH * (DEFAULT_TUI_HEIGHT - 1));
            let tab_line: String = cells.cells[start..].iter().map(|cell| cell.ch).collect();
            assert_eq!(tab_line.matches("::1").count(), 1, "{tab_line:?}");
            drop(first_stream);
            drop(snd_input_ev);
        },
    );
}

#[test]
fn registered_client_command_completes_in_tui() {
    run_test(
        "nixcheck".to_owned(),
        |TestSetup {
             tui, snd_input_ev, ..
         }| async move {
            crate::cmd::register_completions(&tui);
            for key in [
                input::Key::Char('/'),
                input::Key::Char('j'),
                input::Key::Char('o'),
                input::Key::Tab,
            ] {
                snd_input_ev.send(input::Event::Key(key)).await.unwrap();
            }
            yield_(5).await;
            tui.draw();

            let screen = buffer_str(
                &tui.get_front_buffer(),
                DEFAULT_TUI_WIDTH,
                DEFAULT_TUI_HEIGHT,
            );
            assert_eq!(screen.lines().nth(3).unwrap().trim_end(), "/join");

            for key in [
                input::Key::Ctrl('a'),
                input::Key::Ctrl('k'),
                input::Key::Char('/'),
                input::Key::Char('w'),
                input::Key::Char('h'),
                input::Key::Tab,
            ] {
                snd_input_ev.send(input::Event::Key(key)).await.unwrap();
            }
            yield_(5).await;
            tui.draw();
            let screen = buffer_str(
                &tui.get_front_buffer(),
                DEFAULT_TUI_WIDTH,
                DEFAULT_TUI_HEIGHT,
            );
            assert_eq!(screen.lines().nth(3).unwrap().trim_end(), "/wh");
        },
    );
}

#[test]
fn expanded_ipv6_reconnect_uses_the_proxy_for_each_configured_port() {
    run_test("testnick".to_owned(), |TestSetup { tui, .. }| async move {
        let direct_a = TcpListener::bind("[::1]:0").await.unwrap();
        let direct_b = TcpListener::bind("[::1]:0").await.unwrap();
        let proxy_a = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_b = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port_a = direct_a.local_addr().unwrap().port();
        let port_b = direct_b.local_addr().unwrap().port();
        let make_proxy = |listener: &TcpListener| {
            Some(client::ProxyConfig {
                protocol: client::ProxyProtocol::Http,
                addr: "127.0.0.1".to_owned(),
                port: listener.local_addr().unwrap().port(),
                auth: None,
            })
        };
        let proxies = vec![
            ("::1".to_owned(), port_a, make_proxy(&proxy_a)),
            ("::1".to_owned(), port_b, make_proxy(&proxy_b)),
        ];
        let defaults = crate::config::Defaults {
            nicks: vec!["testnick".to_owned()],
            realname: "Test User".to_owned(),
            join: vec![],
            tls: false,
        };
        let ui = UI::new(tui, None);
        let mut clients = Vec::new();
        let source = MsgSource::Serv {
            serv: SERV_NAME.to_owned(),
        };

        crate::cmd::run_cmd(
            &format!("connect [0:0:0:0:0:0:0:1]:{port_a} testpass"),
            source.clone(),
            &defaults,
            &proxies,
            &ui,
            &mut clients,
        );
        let mut first = accept_proxy_registration(&proxy_a, port_a).await;
        assert!(
            tokio::time::timeout(Duration::from_millis(100), direct_a.accept())
                .await
                .is_err()
        );

        crate::cmd::run_cmd(
            &format!("connect [::1]:{port_b}"),
            source,
            &defaults,
            &proxies,
            &ui,
            &mut clients,
        );
        let _second = accept_proxy_registration(&proxy_b, port_b).await;
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(2), first.read_line(&mut String::new()))
                .await
                .unwrap()
                .unwrap(),
            0
        );
        assert!(
            tokio::time::timeout(Duration::from_millis(100), direct_b.accept())
                .await
                .is_err()
        );
        assert_eq!(clients.len(), 1);
    });
}

async fn accept_proxy_registration(listener: &TcpListener, port: u16) -> BufReader<TcpStream> {
    tokio::time::timeout(Duration::from_secs(5), async {
        let (stream, _) = listener.accept().await.unwrap();
        let mut reader = BufReader::new(stream);
        let mut request = String::new();
        loop {
            let mut line = String::new();
            assert_ne!(reader.read_line(&mut line).await.unwrap(), 0);
            request.push_str(&line);
            if line == "\r\n" {
                break;
            }
        }
        assert!(request.starts_with(&format!("CONNECT [::1]:{port} HTTP/1.")));
        reader
            .get_mut()
            .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            .await
            .unwrap();
        let mut lines = Vec::new();
        while lines.len() < 3 {
            let mut line = String::new();
            assert_ne!(reader.read_line(&mut line).await.unwrap(), 0);
            lines.push(line);
        }
        assert!(lines.iter().any(|line| line == "PASS testpass\r\n"));
        assert!(lines.iter().any(|line| line == "NICK testnick\r\n"));
        assert!(lines.iter().any(|line| line.starts_with("USER ")));
        reader
    })
    .await
    .expect("timed out waiting for proxied IPv6 IRC registration")
}

async fn accept_registration(listener: &TcpListener) -> BufReader<TcpStream> {
    tokio::time::timeout(Duration::from_secs(5), async {
        let (stream, _) = listener.accept().await.unwrap();
        let mut reader = BufReader::new(stream);
        let mut lines = Vec::new();
        while lines.len() < 3 {
            let mut line = String::new();
            assert_ne!(reader.read_line(&mut line).await.unwrap(), 0);
            lines.push(line);
        }
        assert!(
            lines.iter().any(|line| line == "PASS testpass\r\n"),
            "{lines:?}"
        );
        assert!(
            lines.iter().any(|line| line == "NICK testnick\r\n"),
            "{lines:?}"
        );
        assert!(
            lines.iter().any(|line| line.starts_with("USER ")),
            "{lines:?}"
        );
        reader
    })
    .await
    .expect("timed out waiting for IPv6 IRC registration")
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
                pfx: Some(Pfx::Ambiguous("tiny_test_user".to_owned())),
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
                pfx: Some(Pfx::Ambiguous("tiny_test_user".to_owned())),
                cmd: Cmd::PRIVMSG {
                    target: MsgTarget::User("osa1".to_owned()),
                    // This generates a notification when the test is run with
                    // desktop-notifications feature, so show a helpful message to not confuse
                    // users (#371)
                    msg: "this is a test in tiny IRC client -- please ignore".to_owned(),
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
             |00:00 tiny_test_user: msg to chan       |
             |osa1:                                   |
             |mentions x.y.z #chan tiny_test_user     |";

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
             |00:00 tiny_test_user: this is a test in |
             |tiny IRC client -- please ignore        |
             |osa1:                                   |
             |mentions x.y.z #chan tiny_test_user     |";

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
                        nick: "tiny_test_user".to_owned(),
                        user: "e@a/b/c.d".to_owned(),
                    }),
                    cmd: Cmd::PRIVMSG {
                        target: MsgTarget::User("$$*".to_owned()),
                        // This generates a notification when the test is run with
                        // desktop-notifications feature, so show a helpful message to not confuse
                        // users (#371)
                        msg: "this is a test in tiny IRC client -- please ignore".to_owned(),
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
             |00:00 tiny_test_user: this is a test in |
             |tiny IRC client -- please ignore        |
             |osa1:                                   |
             |mentions x.y.z tiny_test_user           |";

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
