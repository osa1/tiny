use crate::stream::{Stream, StreamError};
use crate::{Event, ProxyAuth, ProxyConfig, ProxyProtocol, SASLAuth, ServerInfo};
use rcgen::{
    BasicConstraints, Certificate, CertificateParams, DistinguishedName, DnType,
    ExtendedKeyUsagePurpose, IsCa, KeyUsagePurpose,
};
use std::io;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_rustls::TlsAcceptor;
use tokio_rustls::rustls::{
    self, RootCertStore,
    server::{AllowAnyAnonymousOrAuthenticatedClient, AllowAnyAuthenticatedClient},
};

const PROXY_NAME: &str = "proxy.test";
const IRC_NAME: &str = "irc.test";
const DEADLINE: Duration = Duration::from_secs(10);

trait Io: AsyncRead + AsyncWrite + Unpin {}
impl<T: AsyncRead + AsyncWrite + Unpin> Io for T {}
type BoxIo = Box<dyn Io>;

fn ca(name: &str) -> Certificate {
    let mut params = CertificateParams::new(Vec::new());
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    params.key_usages = vec![
        KeyUsagePurpose::KeyCertSign,
        KeyUsagePurpose::DigitalSignature,
    ];
    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, name);
    params.distinguished_name = dn;
    Certificate::from_params(params).unwrap()
}

fn leaf(name: &str, client: bool) -> Certificate {
    let mut params = CertificateParams::new(vec![name.to_owned()]);
    params.extended_key_usages = vec![if client {
        ExtendedKeyUsagePurpose::ClientAuth
    } else {
        ExtendedKeyUsagePurpose::ServerAuth
    }];
    Certificate::from_params(params).unwrap()
}

fn server_config(
    cert: &Certificate,
    signer: &Certificate,
    client_ca: &Certificate,
    require_client: bool,
) -> Arc<rustls::ServerConfig> {
    let mut roots = RootCertStore::empty();
    roots
        .add(&rustls::Certificate(client_ca.serialize_der().unwrap()))
        .unwrap();
    let builder = rustls::ServerConfig::builder().with_safe_defaults();
    let builder = if require_client {
        builder.with_client_cert_verifier(Arc::new(AllowAnyAuthenticatedClient::new(roots)))
    } else {
        builder
            .with_client_cert_verifier(Arc::new(AllowAnyAnonymousOrAuthenticatedClient::new(roots)))
    };
    Arc::new(
        builder
            .with_single_cert(
                vec![rustls::Certificate(
                    cert.serialize_der_with_signer(signer).unwrap(),
                )],
                rustls::PrivateKey(cert.serialize_private_key_der()),
            )
            .unwrap(),
    )
}

#[derive(Clone, Copy)]
enum Fault {
    None,
    ProxyName,
    IrcName,
    ProxyCa,
    IrcCa,
}

// The proxy and IRC server share a socket but negotiate separate TLS sessions.
// Reporting the completed CONNECT distinguishes inner failures from outer failures.
async fn fixture(
    listener: TcpListener,
    proxy_tls: bool,
    irc_tls: bool,
    proxy_config: Arc<rustls::ServerConfig>,
    irc_config: Arc<rustls::ServerConfig>,
    entered_tunnel: mpsc::UnboundedSender<()>,
) -> io::Result<()> {
    let (socket, _) = listener.accept().await?;
    let mut transport: BoxIo = if proxy_tls {
        let tls = TlsAcceptor::from(proxy_config).accept(socket).await?;
        assert!(
            tls.get_ref()
                .1
                .peer_certificates()
                .map_or(true, |certs| certs.is_empty()),
            "SASL client identity must not be sent to the proxy"
        );
        Box::new(tls)
    } else {
        Box::new(socket)
    };

    let mut request = Vec::new();
    while !request.ends_with(b"\r\n\r\n") {
        let byte = transport.read_u8().await?;
        request.push(byte);
        if request.len() > 8192 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "CONNECT too long",
            ));
        }
    }
    assert!(request.starts_with(b"CONNECT irc.test:6697 HTTP/1."));
    transport
        .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
        .await?;
    entered_tunnel.send(()).unwrap();

    let mut irc: BoxIo = if irc_tls {
        let tls = TlsAcceptor::from(irc_config).accept(transport).await?;
        assert!(
            !tls.get_ref().1.peer_certificates().unwrap().is_empty(),
            "IRC server must receive the SASL EXTERNAL identity"
        );
        Box::new(tls)
    } else {
        transport
    };
    let mut registration = vec![0; b"NICK tiny\r\nUSER tiny 0 * :Tiny\r\n".len()];
    irc.read_exact(&mut registration).await?;
    assert_eq!(registration, b"NICK tiny\r\nUSER tiny 0 * :Tiny\r\n");
    irc.write_all(b":server 001 tiny :Welcome\r\nPING :probe\r\n")
        .await?;
    let mut pong = vec![0; b"PONG :probe\r\n".len()];
    irc.read_exact(&mut pong).await?;
    assert_eq!(pong, b"PONG :probe\r\n");
    Ok(())
}

async fn exercise(proxy_tls: bool, irc_tls: bool, fault: Fault) {
    let root = ca("trusted fixture CA");
    let other = ca("untrusted fixture CA");
    let proxy = leaf(
        if matches!(fault, Fault::ProxyName) {
            "wrong-proxy.test"
        } else {
            PROXY_NAME
        },
        false,
    );
    let irc = leaf(
        if matches!(fault, Fault::IrcName) {
            "wrong-irc.test"
        } else {
            IRC_NAME
        },
        false,
    );
    let client = leaf("client.test", true);
    let proxy_signer = if matches!(fault, Fault::ProxyCa) {
        &other
    } else {
        &root
    };
    let irc_signer = if matches!(fault, Fault::IrcCa) {
        &other
    } else {
        &root
    };
    let proxy_config = server_config(&proxy, proxy_signer, &root, false);
    let irc_config = server_config(&irc, irc_signer, &root, true);
    let root_der = root.serialize_der().unwrap();
    let client_pem = format!(
        "{}{}",
        client.serialize_pem_with_signer(&root).unwrap(),
        client.serialize_private_key_pem()
    )
    .into_bytes();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = listener.local_addr().unwrap();
    let (tunnel_tx, mut tunnel_rx) = mpsc::unbounded_channel();
    let server = tokio::task::spawn_local(fixture(
        listener,
        proxy_tls,
        irc_tls,
        proxy_config,
        irc_config,
        tunnel_tx,
    ));
    let info = ServerInfo {
        addr: IRC_NAME.into(),
        port: 6697,
        tls: irc_tls,
        proxy: Some(ProxyConfig {
            protocol: if proxy_tls {
                ProxyProtocol::Https
            } else {
                ProxyProtocol::Http
            },
            addr: PROXY_NAME.into(),
            port: proxy_addr.port(),
            auth: None,
        }),
        pass: None,
        user: None,
        realname: "Tiny".into(),
        nicks: vec!["tiny".into()],
        auto_join: vec![],
        nickserv_ident: None,
        sasl_auth: Some(SASLAuth::External { pem: client_pem }),
    };
    let result = timeout(
        DEADLINE,
        Stream::connect_with_test_root(proxy_addr, &info, 6697, None, &root_der),
    )
    .await
    .expect("connection timed out");
    match fault {
        Fault::None => {
            let mut stream = match result {
                Ok(stream) => stream,
                Err(error) => panic!("valid TLS connection rejected: {error:?}"),
            };
            timeout(DEADLINE, async {
                stream
                    .write_all(b"NICK tiny\r\nUSER tiny 0 * :Tiny\r\n")
                    .await
                    .unwrap();
                let mut greeting = vec![0; b":server 001 tiny :Welcome\r\nPING :probe\r\n".len()];
                stream.read_exact(&mut greeting).await.unwrap();
                assert_eq!(greeting, b":server 001 tiny :Welcome\r\nPING :probe\r\n");
                stream.write_all(b"PONG :probe\r\n").await.unwrap();
            })
            .await
            .expect("IRC exchange timed out");
            timeout(DEADLINE, server)
                .await
                .expect("fixture timed out")
                .unwrap()
                .unwrap();
        }
        Fault::ProxyName | Fault::ProxyCa => {
            assert!(
                matches!(result, Err(StreamError::ProxyError(_))),
                "proxy TLS failure must be a proxy error"
            );
            assert!(
                tunnel_rx.try_recv().is_err(),
                "invalid proxy must not establish a tunnel"
            );
            let _ = timeout(DEADLINE, server)
                .await
                .expect("fixture timed out")
                .unwrap();
        }
        Fault::IrcName | Fault::IrcCa => {
            assert!(
                matches!(&result, Err(error) if !matches!(error, StreamError::ProxyError(_))),
                "IRC TLS verification must fail as an IRC error"
            );
            timeout(DEADLINE, tunnel_rx.recv())
                .await
                .expect("CONNECT timed out")
                .expect("CONNECT did not complete");
            let _ = timeout(DEADLINE, server)
                .await
                .expect("fixture timed out")
                .unwrap();
        }
    }
}

#[tokio::test(flavor = "current_thread")]
async fn nested_tls_and_certificate_validation() {
    tokio::task::LocalSet::new()
        .run_until(async {
            exercise(false, true, Fault::None).await;
            exercise(true, false, Fault::None).await;
            exercise(true, true, Fault::None).await;
            exercise(true, true, Fault::ProxyName).await;
            exercise(true, true, Fault::ProxyCa).await;
            exercise(true, true, Fault::IrcName).await;
            exercise(true, true, Fault::IrcCa).await;
        })
        .await;
}

async fn read_line<S: AsyncRead + Unpin>(stream: &mut S) -> Vec<u8> {
    let mut line = Vec::new();
    loop {
        line.push(stream.read_u8().await.unwrap());
        assert!(line.len() <= 8192);
        if line.ends_with(b"\r\n") {
            return line;
        }
    }
}

async fn https_basic_client(reject: bool) {
    let root = ca("HTTPS client fixture CA");
    let proxy_cert = leaf("127.0.0.1", false);
    let config = server_config(&proxy_cert, &root, &root, false);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::task::spawn_local(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let mut socket = TlsAcceptor::from(config).accept(socket).await.unwrap();
        assert!(
            socket
                .get_ref()
                .1
                .peer_certificates()
                .map_or(true, |certs| certs.is_empty())
        );

        let mut headers = Vec::new();
        loop {
            let line = read_line(&mut socket).await;
            headers.extend_from_slice(&line);
            if line == b"\r\n" {
                break;
            }
        }
        let headers = String::from_utf8(headers).unwrap();
        assert!(headers.starts_with("CONNECT irc.invalid:6667 HTTP/1.1\r\n"));
        assert!(headers.contains("Proxy-Authorization: Basic dXNlcjpzZWNyZXQ=\r\n"));
        if reject {
            socket
                .write_all(b"HTTP/1.1 407 Forbidden\r\n\r\nprivate-proxy-body")
                .await
                .unwrap();
            return;
        }
        socket.write_all(b"HTTP/1.1 200 OK\r\n\r\n").await.unwrap();
        let mut nick = false;
        let mut user = false;
        while !nick || !user {
            let line = read_line(&mut socket).await;
            nick |= line.starts_with(b"NICK tiny");
            user |= line.starts_with(b"USER ");
        }
        socket
            .write_all(b":irc.invalid 001 tiny :welcome\r\nPING :probe\r\n")
            .await
            .unwrap();
        assert_eq!(read_line(&mut socket).await, b"PONG probe\r\n");
    });
    let info = ServerInfo {
        addr: "irc.invalid".into(),
        port: 6667,
        tls: false,
        proxy: Some(ProxyConfig {
            protocol: ProxyProtocol::Https,
            addr: "127.0.0.1".into(),
            port: address.port(),
            auth: Some(ProxyAuth::UserPass {
                username: "user".into(),
                password: "secret".into(),
            }),
        }),
        pass: None,
        user: None,
        realname: "Tiny".into(),
        nicks: vec!["tiny".into()],
        auto_join: vec![],
        nickserv_ident: None,
        sasl_auth: None,
    };
    let (mut client, mut events) =
        crate::connect_with_test_root(info, root.serialize_der().unwrap());
    timeout(DEADLINE, async {
        loop {
            match events.recv().await.expect("client event channel closed") {
                Event::Connected if !reject => break,
                Event::ProxyErr(error) if reject => {
                    let message = error.to_string();
                    assert!(!message.contains("secret"));
                    assert!(!message.contains("private-proxy-body"));
                    break;
                }
                Event::ProxyErr(error) => panic!("unexpected HTTPS proxy error: {error}"),
                Event::Connected => panic!("rejected proxy tunnel connected"),
                _ => {}
            }
        }
    })
    .await
    .expect("client event timed out");
    timeout(DEADLINE, server)
        .await
        .expect("proxy fixture timed out")
        .unwrap();
    client.quit(None);
}

#[tokio::test(flavor = "current_thread")]
async fn https_basic_registration_and_rejection() {
    tokio::task::LocalSet::new()
        .run_until(async {
            https_basic_client(false).await;
            https_basic_client(true).await;
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn stream_rejects_scoped_ipv6_before_connecting_to_proxy() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    for protocol in [
        ProxyProtocol::Http,
        ProxyProtocol::Https,
        ProxyProtocol::Socks4 { remote_dns: false },
        ProxyProtocol::Socks4 { remote_dns: true },
        ProxyProtocol::Socks5,
    ] {
        let info = ServerInfo {
            addr: "::1%1".into(),
            port: 6667,
            tls: false,
            proxy: Some(ProxyConfig {
                protocol,
                addr: address.ip().to_string(),
                port: address.port(),
                auth: None,
            }),
            pass: None,
            user: None,
            realname: "Tiny".into(),
            nicks: vec!["tiny".into()],
            auto_join: vec![],
            nickserv_ident: None,
            sasl_auth: None,
        };
        match Stream::connect(address, &info, info.port, None).await {
            Err(StreamError::ProxyError(error)) => {
                assert!(error.to_string().contains("scoped IPv6"));
            }
            _ => panic!("expected a scoped IPv6 proxy error"),
        }
        assert!(
            timeout(Duration::from_millis(20), listener.accept())
                .await
                .is_err()
        );
    }
}
