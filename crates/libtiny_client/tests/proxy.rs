use libtiny_client::{Client, Event, ProxyAuth, ProxyConfig, ProxyProtocol, ServerInfo};
use std::{future::Future, net::SocketAddr, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::mpsc,
    task::LocalSet,
    time::timeout,
};

const DEADLINE: Duration = Duration::from_secs(5);

fn run(f: impl Future<Output = ()> + 'static) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    LocalSet::new().block_on(&rt, async {
        timeout(DEADLINE, f).await.expect("fixture timed out")
    });
}

fn info(host: &str, port: u16, proxy: Option<ProxyConfig>) -> ServerInfo {
    ServerInfo {
        addr: host.into(),
        port,
        tls: false,
        proxy,
        pass: None,
        user: Some("tester".into()),
        realname: "Test User".into(),
        nicks: vec!["testnick".into()],
        auto_join: vec![],
        nickserv_ident: None,
        sasl_auth: None,
    }
}

async fn listener() -> TcpListener {
    TcpListener::bind("127.0.0.1:0").await.unwrap()
}
fn proxy(addr: SocketAddr, protocol: ProxyProtocol, auth: Option<ProxyAuth>) -> ProxyConfig {
    ProxyConfig {
        protocol,
        addr: addr.ip().to_string(),
        port: addr.port(),
        auth,
    }
}
fn credentials() -> Option<ProxyAuth> {
    Some(ProxyAuth::UserPass {
        username: "user".into(),
        password: "secret".into(),
    })
}
async fn read_until(stream: &mut TcpStream, delimiter: &[u8]) -> Vec<u8> {
    let mut data = Vec::new();
    while !data.ends_with(delimiter) {
        let byte = stream.read_u8().await.unwrap();
        data.push(byte);
        assert!(data.len() < 16384);
    }
    data
}
async fn line(stream: &mut TcpStream) -> String {
    String::from_utf8(read_until(stream, b"\n").await).unwrap()
}
async fn registration(stream: &mut TcpStream, early_ping: bool) {
    let mut nick = false;
    let mut user = false;
    let mut pong = false;
    let mut ping_sent = early_ping;
    while !nick || !user || !pong {
        let l = line(stream).await;
        nick |= l.starts_with("NICK testnick");
        user |= l.starts_with("USER tester ");
        if l.starts_with("PONG ") {
            assert!(l.contains("probe"));
            pong = true;
        }
        if nick && user && !ping_sent {
            stream.write_all(b"PING :probe\r\n").await.unwrap();
            ping_sent = true;
        }
    }
}
async fn http_request(stream: &mut TcpStream, authority: &str, basic: bool) {
    let data = read_until(stream, b"\r\n\r\n").await;
    let request = String::from_utf8(data).unwrap();
    assert!(
        request.starts_with(&format!("CONNECT {authority} HTTP/1.")),
        "{request}"
    );
    if basic {
        assert!(
            request
                .to_ascii_lowercase()
                .contains("proxy-authorization: basic dxnlcjpzzwnyzxq=")
        );
    } else {
        assert!(
            !request
                .to_ascii_lowercase()
                .contains("proxy-authorization:")
        );
    }
}
async fn socks4_request(
    stream: &mut TcpStream,
    host: &[u8],
    port: u16,
    userid: &str,
    remote: bool,
) {
    let mut header = [0; 8];
    stream.read_exact(&mut header).await.unwrap();
    assert_eq!(&header[..2], &[4, 1]);
    assert_eq!(u16::from_be_bytes([header[2], header[3]]), port);
    assert_eq!(
        read_until(stream, &[0]).await,
        [userid.as_bytes(), &[0]].concat()
    );
    if remote {
        assert_eq!(&header[4..], &[0, 0, 0, 1]);
        assert_eq!(read_until(stream, &[0]).await, [host, &[0]].concat());
    } else {
        assert_eq!(&header[4..], host);
    }
}
async fn socks5_request(
    stream: &mut TcpStream,
    host: &str,
    port: u16,
    auth: bool,
    supply_auth: bool,
) {
    let version = stream.read_u8().await.unwrap();
    let n = stream.read_u8().await.unwrap();
    let mut methods = vec![0; n as usize];
    stream.read_exact(&mut methods).await.unwrap();
    assert_eq!(version, 5);
    assert!(methods.contains(&if auth { 2 } else { 0 }));
    if supply_auth {
        assert!(methods.contains(&2));
    }
    stream
        .write_all(&[5, if auth { 2 } else { 0 }])
        .await
        .unwrap();
    if auth {
        assert_eq!(stream.read_u8().await.unwrap(), 1);
        let len = stream.read_u8().await.unwrap();
        let mut username = vec![0; len as usize];
        stream.read_exact(&mut username).await.unwrap();
        let len = stream.read_u8().await.unwrap();
        let mut password = vec![0; len as usize];
        stream.read_exact(&mut password).await.unwrap();
        assert_eq!(username, b"user");
        assert_eq!(password, b"secret");
        stream.write_all(&[1, 0]).await.unwrap();
    }
    let mut head = [0; 4];
    stream.read_exact(&mut head).await.unwrap();
    assert_eq!(&head[..3], &[5, 1, 0]); // Also proves no credentials follow a no-auth selection.
    let expected = host.parse::<std::net::IpAddr>();
    match (head[3], expected) {
        (1, Ok(std::net::IpAddr::V4(ip))) => {
            let mut b = [0; 4];
            stream.read_exact(&mut b).await.unwrap();
            assert_eq!(b, ip.octets());
        }
        (4, Ok(std::net::IpAddr::V6(ip))) => {
            let mut b = [0; 16];
            stream.read_exact(&mut b).await.unwrap();
            assert_eq!(b, ip.octets());
        }
        (3, Err(_)) => {
            let len = stream.read_u8().await.unwrap();
            let mut b = vec![0; len as usize];
            stream.read_exact(&mut b).await.unwrap();
            assert_eq!(b, host.as_bytes());
        }
        _ => panic!("incorrect SOCKS5 address type"),
    }
    assert_eq!(stream.read_u16().await.unwrap(), port);
}
async fn success_reply(stream: &mut TcpStream) {
    stream
        .write_all(&[5, 0, 0, 1, 127, 0, 0, 1, 0, 1])
        .await
        .unwrap();
}
async fn proxy_error(events: &mut mpsc::Receiver<Event>, secrets: &[&str]) -> std::io::Error {
    loop {
        match events
            .recv()
            .await
            .expect("event channel closed before proxy error")
        {
            Event::ProxyErr(e) => {
                let text = format!("{e:?} {e}");
                for secret in secrets {
                    assert!(!text.contains(secret), "proxy error leaked sensitive data");
                }
                return e;
            }
            Event::Connected => panic!("unexpected direct fallback"),
            _ => {}
        }
    }
}

#[test]
fn direct_and_tunnel_registration() {
    run(async {
        // Direct TCP, HTTP, SOCKS4a and SOCKS5 each carry real IRC traffic.
        for kind in 0..4 {
            let listener = listener().await;
            let addr = listener.local_addr().unwrap();
            let (host, port, p) = match kind {
                0 => ("127.0.0.1", addr.port(), None),
                1 => (
                    "irc.invalid",
                    6667,
                    Some(proxy(addr, ProxyProtocol::Http, credentials())),
                ),
                2 => (
                    "irc.invalid",
                    6667,
                    Some(proxy(
                        addr,
                        ProxyProtocol::Socks4 { remote_dns: true },
                        Some(ProxyAuth::UserId("userid".into())),
                    )),
                ),
                _ => (
                    "irc.invalid",
                    6667,
                    Some(proxy(addr, ProxyProtocol::Socks5, credentials())),
                ),
            };
            let (mut client, _events) = Client::new(info(host, port, p));
            let (mut socket, _) = listener.accept().await.unwrap();
            match kind {
                1 => {
                    http_request(&mut socket, "irc.invalid:6667", true).await;
                    socket.write_all(b"HTTP/1.1 200 OK\r\n\r\n").await.unwrap();
                }
                2 => {
                    socks4_request(&mut socket, b"irc.invalid", 6667, "userid", true).await;
                    socket.write_all(&[0, 90, 0, 0, 0, 0, 0, 0]).await.unwrap();
                }
                3 => {
                    socks5_request(&mut socket, host, port, true, true).await;
                    success_reply(&mut socket).await;
                }
                _ => {}
            }
            registration(&mut socket, false).await;
            client.quit(None);
        }
    });
}

#[test]
fn socks_address_forms_and_no_auth_selection() {
    run(async {
        for (host, auth) in [
            ("irc.invalid", true),
            ("127.0.0.2", false),
            ("2001:db8::1", false),
        ] {
            let listener = listener().await;
            let addr = listener.local_addr().unwrap();
            let (mut client, _events) = Client::new(info(
                host,
                6669,
                Some(proxy(
                    addr,
                    ProxyProtocol::Socks5,
                    if auth { credentials() } else { None },
                )),
            ));
            let (mut socket, _) = listener.accept().await.unwrap();
            socks5_request(&mut socket, host, 6669, false, auth).await;
            success_reply(&mut socket).await;
            registration(&mut socket, false).await;
            client.quit(None);
        }
        let listener = listener().await;
        let addr = listener.local_addr().unwrap();
        let (mut client, _events) = Client::new(info(
            "localhost",
            6670,
            Some(proxy(
                addr,
                ProxyProtocol::Socks4 { remote_dns: false },
                None,
            )),
        ));
        let (mut socket, _) = listener.accept().await.unwrap();
        socks4_request(&mut socket, &[127, 0, 0, 1], 6670, "", false).await;
        socket.write_all(&[0, 90, 0, 0, 0, 0, 0, 0]).await.unwrap();
        registration(&mut socket, false).await;
        client.quit(None);
    });
}

#[test]
fn http_framing_and_ipv6_authority() {
    run(async {
        let maximum_headers = format!(
            "HTTP/1.1 200 OK\r\n{}\r\nPING :probe\r\n",
            "X: a\r\n".repeat(64)
        );
        for response in [
            b"HTTP/1.1 200 OK\r\nX-Test: yes\r\n\r\nPING :probe\r\n".as_slice(),
            b"HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 200 OK\r\n\r\nPING :probe\r\n".as_slice(),
            b"HTTP/1.0 204 No Content\r\n\r\nPING :probe\r\n".as_slice(),
            maximum_headers.as_bytes(),
        ] {
            let listener = listener().await;
            let addr = listener.local_addr().unwrap();
            let (mut client, _events) = Client::new(info(
                "2001:db8::1",
                6671,
                Some(proxy(addr, ProxyProtocol::Http, None)),
            ));
            let (mut socket, _) = listener.accept().await.unwrap();
            http_request(&mut socket, "[2001:db8::1]:6671", false).await;
            let split = response.len() / 2;
            socket.write_all(&response[..split]).await.unwrap();
            tokio::task::yield_now().await;
            socket.write_all(&response[split..]).await.unwrap();
            registration(&mut socket, true).await;
            client.quit(None);
        }
    });
}

#[test]
fn reconnect_changes_only_target_and_interrupts_handshake() {
    run(async {
        let listener = listener().await;
        let addr = listener.local_addr().unwrap();
        let (mut client, _events) = Client::new(info(
            "irc.invalid",
            6667,
            Some(proxy(addr, ProxyProtocol::Http, None)),
        ));
        let (mut first, _) = listener.accept().await.unwrap();
        http_request(&mut first, "irc.invalid:6667", false).await;
        // The proxy deliberately never replies: reconnect must cancel this negotiation.
        client.reconnect(Some(6698));
        let (mut second, _) = listener.accept().await.unwrap();
        http_request(&mut second, "irc.invalid:6698", false).await;
        let mut byte = [0];
        assert_eq!(first.read(&mut byte).await.unwrap(), 0);
        second.write_all(b"HTTP/1.1 200 OK\r\n\r\n").await.unwrap();
        registration(&mut second, false).await;
        client.reconnect(Some(6700));
        let (mut third, _) = listener.accept().await.unwrap();
        http_request(&mut third, "irc.invalid:6700", false).await;
        client.quit(None);
        assert_eq!(third.read(&mut byte).await.unwrap(), 0);
    });
}

#[test]
fn reconnect_switches_proxy_and_can_return_to_direct() {
    run(async {
        let direct = listener().await;
        let proxy_a = listener().await;
        let proxy_b = listener().await;
        let direct_port = direct.local_addr().unwrap().port();
        let a = proxy(proxy_a.local_addr().unwrap(), ProxyProtocol::Http, None);
        let b = proxy(proxy_b.local_addr().unwrap(), ProxyProtocol::Http, None);
        let (mut client, _events) = Client::new(info("127.0.0.1", direct_port, None));

        let (mut initial, _) = direct.accept().await.unwrap();
        registration(&mut initial, false).await;
        client.reconnect_with_proxy(1111, Some(a));
        let (mut first, _) = proxy_a.accept().await.unwrap();
        http_request(&mut first, "127.0.0.1:1111", false).await;
        let mut byte = [0];
        assert_eq!(initial.read(&mut byte).await.unwrap(), 0);

        // Changing the proxy interrupts an unfinished tunnel negotiation.
        client.reconnect_with_proxy(2222, Some(b));
        let (mut second, _) = proxy_b.accept().await.unwrap();
        http_request(&mut second, "127.0.0.1:2222", false).await;
        assert_eq!(first.read(&mut byte).await.unwrap(), 0);
        second.write_all(b"HTTP/1.1 200 OK\r\n\r\n").await.unwrap();
        registration(&mut second, false).await;

        // The original API retains the selected proxy, even for a direct listener's port.
        client.reconnect(Some(direct_port));
        let (mut retained, _) = proxy_b.accept().await.unwrap();
        http_request(&mut retained, &format!("127.0.0.1:{direct_port}"), false).await;
        assert_eq!(second.read(&mut byte).await.unwrap(), 0);
        assert!(
            timeout(Duration::from_millis(100), direct.accept())
                .await
                .is_err()
        );
        retained
            .write_all(b"HTTP/1.1 200 OK\r\n\r\n")
            .await
            .unwrap();
        registration(&mut retained, false).await;

        client.reconnect_with_proxy(direct_port, None);
        let (mut final_direct, _) = direct.accept().await.unwrap();
        assert_eq!(retained.read(&mut byte).await.unwrap(), 0);
        registration(&mut final_direct, false).await;
        client.quit(None);
    });
}

#[test]
fn reconnect_switches_proxy_while_waiting_after_failure() {
    run(async {
        let proxy_a = listener().await;
        let proxy_b = listener().await;
        let a = proxy(proxy_a.local_addr().unwrap(), ProxyProtocol::Http, None);
        let b = proxy(proxy_b.local_addr().unwrap(), ProxyProtocol::Http, None);
        let (mut client, mut events) = Client::new(info("irc.invalid", 6667, Some(a)));
        let (mut first, _) = proxy_a.accept().await.unwrap();
        http_request(&mut first, "irc.invalid:6667", false).await;
        first
            .write_all(b"HTTP/1.1 407 Forbidden\r\n\r\n")
            .await
            .unwrap();
        while !matches!(events.recv().await.unwrap(), Event::Disconnected) {}

        client.reconnect_with_proxy(6697, Some(b));
        let (mut second, _) = proxy_b.accept().await.unwrap();
        http_request(&mut second, "irc.invalid:6697", false).await;
        second.write_all(b"HTTP/1.1 200 OK\r\n\r\n").await.unwrap();
        registration(&mut second, false).await;
        client.quit(None);
    });
}

#[test]
fn scoped_ipv6_destinations_are_rejected_before_connecting() {
    run(async {
        let direct = match TcpListener::bind("[::1]:0").await {
            Ok(listener) => Some(listener),
            Err(err)
                if matches!(
                    err.kind(),
                    std::io::ErrorKind::AddrNotAvailable | std::io::ErrorKind::Unsupported
                ) || matches!(err.raw_os_error(), Some(93 | 97)) =>
            {
                eprintln!("Skipping IPv6 direct-listener assertion: {err}");
                None
            }
            Err(err) => panic!("IPv6 loopback bind failed: {err}"),
        };
        let proxy_listener = listener().await;
        let proxy_addr = proxy_listener.local_addr().unwrap();
        let direct_port = direct
            .as_ref()
            .map_or(6667, |listener| listener.local_addr().unwrap().port());
        for host in ["::1%1", "[::1%1]"] {
            for protocol in [
                ProxyProtocol::Http,
                ProxyProtocol::Https,
                ProxyProtocol::Socks4 { remote_dns: false },
                ProxyProtocol::Socks4 { remote_dns: true },
                ProxyProtocol::Socks5,
            ] {
                let (mut client, mut events) = Client::new(info(
                    host,
                    direct_port,
                    Some(proxy(proxy_addr, protocol, None)),
                ));
                match events.recv().await.unwrap() {
                    Event::ProxyErr(error) => {
                        assert!(error.to_string().contains("scoped IPv6"));
                    }
                    other => panic!("expected scoped IPv6 proxy error before DNS: {other:?}"),
                }
                assert!(
                    timeout(Duration::from_millis(20), proxy_listener.accept())
                        .await
                        .is_err()
                );
                if let Some(direct) = &direct {
                    assert!(
                        timeout(Duration::from_millis(20), direct.accept())
                            .await
                            .is_err()
                    );
                }
                client.quit(None);
            }
        }
    });
}

#[test]
fn socks4_local_dns_errors_are_distinct_from_ipv6_rejection() {
    run(async {
        let proxy_listener = listener().await;
        let proxy_addr = proxy_listener.local_addr().unwrap();
        let host = "x".repeat(300);
        for (host, expected) in [
            (host.as_str(), "Failed to resolve SOCKS4 destination"),
            ("::1", "SOCKS4 requires an IPv4 destination"),
        ] {
            let (mut client, mut events) = Client::new(info(
                host,
                6667,
                Some(proxy(
                    proxy_addr,
                    ProxyProtocol::Socks4 { remote_dns: false },
                    None,
                )),
            ));
            loop {
                match events.recv().await.unwrap() {
                    Event::ProxyErr(error) => {
                        assert_eq!(error.to_string(), expected);
                        break;
                    }
                    Event::ConnectingProxy(_) => panic!("proxy connected before target validation"),
                    _ => {}
                }
            }
            assert!(
                timeout(Duration::from_millis(20), proxy_listener.accept())
                    .await
                    .is_err()
            );
            client.quit(None);
        }
    });
}

#[test]
fn http_rejections_are_sanitized() {
    run(async {
        let too_many = format!("HTTP/1.1 200 OK\r\n{}\r\n", "X: a\r\n".repeat(65));
        let oversize = format!("HTTP/1.1 200 OK\r\nX: {}\r\n\r\n", "a".repeat(8200));
        let cases: Vec<Vec<u8>> = vec![
            b"HTTP/1.1 101 Switching Protocols\r\n\r\n".to_vec(),
            b"HTTP/1.1 407 Forbidden\r\nContent-Length: 10\r\n\r\nbodysecret".to_vec(),
            b"not HTTP\r\n\r\n".to_vec(),
            too_many.into_bytes(),
            oversize.into_bytes(),
            b"HTTP/1.1 200".to_vec(),
        ];
        for response in cases {
            let listener = listener().await;
            let addr = listener.local_addr().unwrap();
            let (mut client, mut events) = Client::new(info(
                "irc.invalid",
                6667,
                Some(proxy(addr, ProxyProtocol::Http, credentials())),
            ));
            let (mut socket, _) = listener.accept().await.unwrap();
            http_request(&mut socket, "irc.invalid:6667", true).await;
            let _ = socket.write_all(&response).await;
            socket.shutdown().await.unwrap();
            proxy_error(&mut events, &["secret", "bodysecret"]).await;
            client.quit(None);
        }
    });
}

enum SocksRejection {
    Socks4,
    Socks5Auth,
    Socks5Connect,
}

fn check_socks_rejection(kind: SocksRejection) {
    run(async move {
        let listener = listener().await;
        let addr = listener.local_addr().unwrap();
        let p = match kind {
            SocksRejection::Socks4 => proxy(
                addr,
                ProxyProtocol::Socks4 { remote_dns: true },
                Some(ProxyAuth::UserId("userid".into())),
            ),
            SocksRejection::Socks5Auth | SocksRejection::Socks5Connect => {
                proxy(addr, ProxyProtocol::Socks5, credentials())
            }
        };
        let (mut client, mut events) = Client::new(info("irc.invalid", 6667, Some(p)));
        let (mut socket, _) = listener.accept().await.unwrap();
        match kind {
            SocksRejection::Socks4 => {
                socks4_request(&mut socket, b"irc.invalid", 6667, "userid", true).await;
                socket.write_all(&[0, 91, 0, 0, 0, 0, 0, 0]).await.unwrap();
            }
            SocksRejection::Socks5Auth => {
                let mut greeting = [0; 2];
                socket.read_exact(&mut greeting).await.unwrap();
                let mut methods = vec![0; greeting[1] as usize];
                socket.read_exact(&mut methods).await.unwrap();
                socket.write_all(&[5, 2]).await.unwrap();
                assert_eq!(socket.read_u8().await.unwrap(), 1);
                let n = socket.read_u8().await.unwrap();
                let mut user = vec![0; n as usize];
                socket.read_exact(&mut user).await.unwrap();
                let n = socket.read_u8().await.unwrap();
                let mut pass = vec![0; n as usize];
                socket.read_exact(&mut pass).await.unwrap();
                socket.write_all(&[1, 1]).await.unwrap();
            }
            SocksRejection::Socks5Connect => {
                socks5_request(&mut socket, "irc.invalid", 6667, false, true).await;
                socket
                    .write_all(&[5, 5, 0, 1, 0, 0, 0, 0, 0, 0])
                    .await
                    .unwrap();
            }
        }
        proxy_error(&mut events, &["secret", "userid"]).await;
        client.quit(None);
    });
}

#[test]
fn socks4_rejection_is_proxy_error() {
    check_socks_rejection(SocksRejection::Socks4);
}

#[test]
fn socks5_auth_rejection_is_proxy_error() {
    check_socks_rejection(SocksRejection::Socks5Auth);
}

#[test]
fn socks5_connect_rejection_is_proxy_error() {
    check_socks_rejection(SocksRejection::Socks5Connect);
}

#[test]
fn proxy_tcp_connection_failure_is_proxy_error() {
    run(async {
        let direct = listener().await;
        let proxy_listener = listener().await;
        let addr = proxy_listener.local_addr().unwrap();
        drop(proxy_listener);
        let (mut client, mut events) = Client::new(info(
            "127.0.0.1",
            direct.local_addr().unwrap().port(),
            Some(proxy(addr, ProxyProtocol::Http, credentials())),
        ));
        let error = proxy_error(&mut events, &["user", "secret"]).await;
        assert_eq!(error.to_string(), "proxy TCP connection failed");
        assert!(
            timeout(Duration::from_millis(150), direct.accept())
                .await
                .is_err()
        );
        client.quit(None);
    });
}

#[test]
fn invalid_public_proxy_inputs_fail_closed() {
    run(async {
        let listener = listener().await;
        let addr = listener.local_addr().unwrap();
        let invalid = vec![
            (
                "irc.invalid".to_string(),
                ProxyProtocol::Http,
                Some(ProxyAuth::UserId("userid".into())),
            ),
            (
                "irc.invalid".to_string(),
                ProxyProtocol::Socks4 { remote_dns: true },
                credentials(),
            ),
            (
                "irc.invalid".to_string(),
                ProxyProtocol::Socks5,
                Some(ProxyAuth::UserId("userid".into())),
            ),
            (
                "irc.invalid".to_string(),
                ProxyProtocol::Http,
                Some(ProxyAuth::UserPass {
                    username: "bad:user".into(),
                    password: "secret".into(),
                }),
            ),
            (
                "irc.invalid".to_string(),
                ProxyProtocol::Http,
                Some(ProxyAuth::UserPass {
                    username: "user".into(),
                    password: "bad\nsecret".into(),
                }),
            ),
            (
                "irc.invalid".to_string(),
                ProxyProtocol::Socks5,
                Some(ProxyAuth::UserPass {
                    username: "".into(),
                    password: "secret".into(),
                }),
            ),
            (
                "irc.invalid".to_string(),
                ProxyProtocol::Socks5,
                Some(ProxyAuth::UserPass {
                    username: "x".repeat(256),
                    password: "secret".into(),
                }),
            ),
            (
                "irc.invalid".to_string(),
                ProxyProtocol::Socks5,
                Some(ProxyAuth::UserPass {
                    username: "user".into(),
                    password: "x".repeat(256),
                }),
            ),
            (
                "irc.invalid".to_string(),
                ProxyProtocol::Socks4 { remote_dns: true },
                Some(ProxyAuth::UserId("bad\0userid".into())),
            ),
            (
                "irc.invalid".to_string(),
                ProxyProtocol::Socks4 { remote_dns: true },
                Some(ProxyAuth::UserId("x".repeat(256))),
            ),
            (
                "irc.invalid".to_string(),
                ProxyProtocol::Socks4 { remote_dns: true },
                Some(ProxyAuth::UserId(String::new())),
            ),
            (
                "x".repeat(255),
                ProxyProtocol::Socks4 { remote_dns: true },
                Some(ProxyAuth::UserId("u".repeat(249))),
            ),
            (
                "x".repeat(256),
                ProxyProtocol::Socks4 { remote_dns: true },
                None,
            ),
            ("x".repeat(256), ProxyProtocol::Socks5, None),
            ("bad\0host".into(), ProxyProtocol::Socks5, None),
            ("bad host".into(), ProxyProtocol::Http, None),
            ("bad/host".into(), ProxyProtocol::Http, None),
            (
                "2001:db8::1".into(),
                ProxyProtocol::Socks4 { remote_dns: false },
                None,
            ),
        ];
        for (host, protocol, auth) in invalid {
            let (mut client, mut events) =
                Client::new(info(&host, 6667, Some(proxy(addr, protocol, auth))));
            proxy_error(&mut events, &["secret", "userid", "bad:user"]).await;
            client.quit(None);
        }
    });
}

#[test]
fn failed_proxy_never_falls_back_to_irc() {
    run(async {
        let direct = listener().await;
        let proxy_listener = listener().await;
        let p = proxy(
            proxy_listener.local_addr().unwrap(),
            ProxyProtocol::Http,
            None,
        );
        let (mut client, mut events) = Client::new(info(
            "127.0.0.1",
            direct.local_addr().unwrap().port(),
            Some(p),
        ));
        let (mut socket, _) = proxy_listener.accept().await.unwrap();
        http_request(
            &mut socket,
            &format!("127.0.0.1:{}", direct.local_addr().unwrap().port()),
            false,
        )
        .await;
        socket
            .write_all(b"HTTP/1.1 407 Forbidden\r\n\r\n")
            .await
            .unwrap();
        proxy_error(&mut events, &[]).await;
        assert!(
            timeout(Duration::from_millis(150), direct.accept())
                .await
                .is_err()
        );
        client.quit(None);
    });
}

#[test]
fn socks_maximum_credentials_and_domain() {
    run(async {
        let first_listener = listener().await;
        let addr = first_listener.local_addr().unwrap();
        let domain = "d".repeat(255);
        let username = "u".repeat(255);
        let password = "p".repeat(255);
        let auth = Some(ProxyAuth::UserPass {
            username: username.clone(),
            password: password.clone(),
        });
        let (mut client, _events) = Client::new(info(
            &domain,
            6667,
            Some(proxy(addr, ProxyProtocol::Socks5, auth)),
        ));
        let (mut socket, _) = first_listener.accept().await.unwrap();
        let mut greeting = [0; 2];
        socket.read_exact(&mut greeting).await.unwrap();
        let mut methods = vec![0; greeting[1] as usize];
        socket.read_exact(&mut methods).await.unwrap();
        assert!(methods.contains(&2));
        socket.write_all(&[5, 2]).await.unwrap();
        assert_eq!(socket.read_u8().await.unwrap(), 1);
        assert_eq!(socket.read_u8().await.unwrap(), 255);
        let mut value = vec![0; 255];
        socket.read_exact(&mut value).await.unwrap();
        assert_eq!(value, username.as_bytes());
        assert_eq!(socket.read_u8().await.unwrap(), 255);
        socket.read_exact(&mut value).await.unwrap();
        assert_eq!(value, password.as_bytes());
        socket.write_all(&[1, 0]).await.unwrap();
        let mut header = [0; 4];
        socket.read_exact(&mut header).await.unwrap();
        assert_eq!(header, [5, 1, 0, 3]);
        assert_eq!(socket.read_u8().await.unwrap(), 255);
        socket.read_exact(&mut value).await.unwrap();
        assert_eq!(value, domain.as_bytes());
        assert_eq!(socket.read_u16().await.unwrap(), 6667);
        success_reply(&mut socket).await;
        registration(&mut socket, false).await;
        client.quit(None);

        let listener = listener().await;
        let addr = listener.local_addr().unwrap();
        let userid = "u".repeat(248);
        let (mut client, _events) = Client::new(info(
            &domain,
            6667,
            Some(proxy(
                addr,
                ProxyProtocol::Socks4 { remote_dns: true },
                Some(ProxyAuth::UserId(userid.clone())),
            )),
        ));
        let (mut socket, _) = listener.accept().await.unwrap();
        socks4_request(&mut socket, domain.as_bytes(), 6667, &userid, true).await;
        socket.write_all(&[0, 90, 0, 0, 0, 0, 0, 0]).await.unwrap();
        registration(&mut socket, false).await;
        client.quit(None);
    });
}

#[test]
fn proxy_debug_hides_credentials() {
    let userid = ProxyAuth::UserId("secret-userid-marker".into());
    let userpass = ProxyAuth::UserPass {
        username: "secret-username-marker".into(),
        password: "secret-password-marker".into(),
    };
    let debug = format!("{userid:?} {userpass:?}");
    for secret in [
        "secret-userid-marker",
        "secret-username-marker",
        "secret-password-marker",
    ] {
        assert!(!debug.contains(secret));
    }
}
