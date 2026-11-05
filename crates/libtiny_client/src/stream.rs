use lazy_static::lazy_static;
use std::{
    net::SocketAddr,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    net::TcpStream,
};

#[cfg(feature = "tls-native")]
use tokio_native_tls::TlsStream;
#[cfg(feature = "tls-rustls")]
use tokio_rustls::client::TlsStream;

fn tls_server_name(host_name: &str) -> &str {
    match host_name.split_once('%') {
        Some((ip, scope))
            if !scope.is_empty()
                && !scope.contains('%')
                && ip.parse::<std::net::Ipv6Addr>().is_ok() =>
        {
            ip
        }
        _ => host_name,
    }
}

#[cfg(feature = "tls-native")]
lazy_static! {
    static ref TLS_CONNECTOR: tokio_native_tls::TlsConnector = tls_connector(None);
}

#[cfg(feature = "tls-native")]
fn tls_connector(pem: Option<&Vec<u8>>) -> tokio_native_tls::TlsConnector {
    use native_tls::Identity;

    let mut builder = native_tls::TlsConnector::builder();
    if let Some(pem) = pem {
        let identity = Identity::from_pkcs8(pem, pem).expect("X509 Cert and private key");
        builder.identity(identity);
    }
    tokio_native_tls::TlsConnector::from(builder.build().unwrap())
}

#[cfg(feature = "tls-rustls")]
lazy_static! {
    static ref TLS_CONNECTOR: tokio_rustls::TlsConnector = tls_connector(None);
}

#[cfg(feature = "tls-rustls")]
fn tls_connector(sasl: Option<&Vec<u8>>) -> tokio_rustls::TlsConnector {
    use std::io::{Cursor, Seek, SeekFrom};
    use tokio_rustls::rustls::{Certificate, ClientConfig, PrivateKey, RootCertStore};

    let mut roots = RootCertStore::empty();
    for cert in rustls_native_certs::load_native_certs().expect("could not load platform certs") {
        roots.add(&Certificate(cert.0)).unwrap();
    }

    let builder = ClientConfig::builder()
        .with_safe_defaults()
        .with_root_certificates(roots);

    let config = if let Some(pem) = sasl {
        let mut buf = Cursor::new(pem);
        // extract certificate
        let cert = rustls_pemfile::certs(&mut buf)
            .expect("Could not parse PKCS8 PEM")
            .pop()
            .expect("Cert PEM must have at least one cert");

        // extract private key
        buf.seek(SeekFrom::Start(0)).unwrap();
        let key = rustls_pemfile::pkcs8_private_keys(&mut buf)
            .expect("Could not parse PKCS8 PEM")
            .pop()
            .expect("Cert PEM must have at least one private key");

        builder
            .with_client_auth_cert(vec![Certificate(cert)], PrivateKey(key))
            .expect("Client auth cert")
    } else {
        builder.with_no_client_auth()
    };
    tokio_rustls::TlsConnector::from(std::sync::Arc::new(config))
}

// We box the fields to reduce type size. Without boxing the type size is 64 with native-tls and
// 1288 with native-tls. With boxing it's 16 in both. More importantly, there's a large size
// difference between the variants when using rustls, see #189.
pub(crate) enum Stream {
    TcpStream(Box<TcpStream>),
    TlsStream(Box<TlsStream<TcpStream>>),
}

#[cfg(feature = "tls-native")]
pub(crate) type TlsError = native_tls::Error;
#[cfg(feature = "tls-rustls")]
pub(crate) type TlsError = tokio_rustls::rustls::Error;

pub(crate) enum StreamError {
    TlsError(TlsError),
    IoError(std::io::Error),
}

impl From<TlsError> for StreamError {
    fn from(err: TlsError) -> Self {
        StreamError::TlsError(err)
    }
}

impl From<std::io::Error> for StreamError {
    fn from(err: std::io::Error) -> Self {
        StreamError::IoError(err)
    }
}

impl Stream {
    pub(crate) async fn new_tcp(addr: SocketAddr) -> Result<Stream, StreamError> {
        Ok(Stream::TcpStream(TcpStream::connect(addr).await?.into()))
    }

    #[cfg(feature = "tls-native")]
    pub(crate) async fn new_tls(
        addr: SocketAddr,
        host_name: &str,
        sasl: Option<&Vec<u8>>,
    ) -> Result<Stream, StreamError> {
        let tcp_stream = TcpStream::connect(addr).await?;
        let host_name = tls_server_name(host_name);
        // If SASL EXTERNAL is enabled create a new TLS connector with client auth cert
        let tls_stream = if sasl.is_some() {
            tls_connector(sasl).connect(host_name, tcp_stream).await?
        } else {
            TLS_CONNECTOR.connect(host_name, tcp_stream).await?
        };
        Ok(Stream::TlsStream(tls_stream.into()))
    }

    #[cfg(feature = "tls-rustls")]
    pub(crate) async fn new_tls(
        addr: SocketAddr,
        host_name: &str,
        sasl: Option<&Vec<u8>>,
    ) -> Result<Stream, StreamError> {
        use tokio_rustls::rustls::ServerName;

        let tcp_stream = TcpStream::connect(addr).await?;
        let name = ServerName::try_from(tls_server_name(host_name)).unwrap();
        // If SASL EXTERNAL is enabled create a new TLS connector with client auth cert
        let tls_stream = if sasl.is_some() {
            tls_connector(sasl).connect(name, tcp_stream).await?
        } else {
            TLS_CONNECTOR.connect(name, tcp_stream).await?
        };
        Ok(Stream::TlsStream(tls_stream.into()))
    }
}

//
// Boilerplate
//

impl AsyncRead for Stream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &mut ReadBuf,
    ) -> Poll<Result<(), std::io::Error>> {
        match *self {
            Stream::TcpStream(ref mut tcp_stream) => Pin::new(tcp_stream).poll_read(cx, buf),
            Stream::TlsStream(ref mut tls_stream) => Pin::new(tls_stream).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for Stream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        match *self {
            Stream::TcpStream(ref mut tcp_stream) => Pin::new(tcp_stream).poll_write(cx, buf),
            Stream::TlsStream(ref mut tls_stream) => Pin::new(tls_stream).poll_write(cx, buf),
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), std::io::Error>> {
        match *self {
            Stream::TcpStream(ref mut tcp_stream) => Pin::new(tcp_stream).poll_flush(cx),
            Stream::TlsStream(ref mut tls_stream) => Pin::new(tls_stream).poll_flush(cx),
        }
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
    ) -> Poll<Result<(), std::io::Error>> {
        match *self {
            Stream::TcpStream(ref mut tcp_stream) => Pin::new(tcp_stream).poll_shutdown(cx),
            Stream::TlsStream(ref mut tls_stream) => Pin::new(tls_stream).poll_shutdown(cx),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Stream, StreamError, tls_server_name};
    use std::process::Command;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio_rustls::TlsAcceptor;
    use tokio_rustls::rustls::{Certificate, PrivateKey, ServerConfig};

    #[test]
    fn scoped_server_names() {
        for (input, expected) in [
            ("::1%1", "::1"),
            ("fe80::1%lo", "fe80::1"),
            ("::ffff:192.0.2.1%lo", "::ffff:192.0.2.1"),
            ("::1", "::1"),
            ("localhost", "localhost"),
            ("localhost%lo", "localhost%lo"),
            ("127.0.0.1%lo", "127.0.0.1%lo"),
            ("::1%", "::1%"),
            ("::1%lo%extra", "::1%lo%extra"),
            ("not-ip%lo", "not-ip%lo"),
        ] {
            assert_eq!(tls_server_name(input), expected);
        }
    }

    #[test]
    fn scoped_ipv6_tls() {
        const CHILD: &str = "TINY_TEST_IPV6_TLS_CHILD";
        if !matches!(std::env::var(CHILD).as_deref(), Ok("1")) {
            let output = Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "stream::tests::scoped_ipv6_tls", "--nocapture"])
                .env(CHILD, "1")
                .env(
                    "SSL_CERT_FILE",
                    concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/ipv6-ca.pem"),
                )
                .output()
                .expect("failed to run isolated TLS test");
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                output.status.success() && stdout.contains("1 passed"),
                "TLS test child failed: {}\nstdout:\n{}\nstderr:\n{}",
                output.status,
                stdout,
                stderr,
            );
            eprint!("{stderr}");
            return;
        }

        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let listener = match TcpListener::bind("[::1]:0").await {
                    Ok(listener) => listener,
                    Err(err)
                        if matches!(
                            err.kind(),
                            std::io::ErrorKind::AddrNotAvailable | std::io::ErrorKind::Unsupported
                        ) || matches!(err.raw_os_error(), Some(93 | 97)) =>
                    {
                        eprintln!("Skipping IPv6 TLS test: {err}");
                        return;
                    }
                    Err(err) => panic!("IPv6 loopback bind failed: {err}"),
                };
                let cert =
                    Certificate(include_bytes!("../tests/fixtures/ipv6-server.der").to_vec());
                let key =
                    PrivateKey(include_bytes!("../tests/fixtures/ipv6-server-key.der").to_vec());
                let config = ServerConfig::builder()
                    .with_safe_defaults()
                    .with_no_client_auth()
                    .with_single_cert(vec![cert], key)
                    .unwrap();
                let acceptor = TlsAcceptor::from(Arc::new(config));

                for (name, succeeds) in [("::1%1", true), ("localhost", true), ("::2%1", false)] {
                    tokio::time::timeout(Duration::from_secs(5), async {
                        let server = async {
                            let (tcp, _) = listener.accept().await.unwrap();
                            let handshake = acceptor.accept(tcp).await;
                            if succeeds {
                                let mut tls = handshake.expect("server TLS handshake failed");
                                let mut ping = [0; 4];
                                tls.read_exact(&mut ping).await.unwrap();
                                assert_eq!(&ping, b"ping");
                                tls.write_all(b"pong").await.unwrap();
                            }
                        };
                        let client = async {
                            let connection =
                                Stream::new_tls(listener.local_addr().unwrap(), name, None).await;
                            if succeeds {
                                let mut tls = match connection {
                                    Ok(tls) => tls,
                                    Err(StreamError::TlsError(err)) => {
                                        panic!("TLS handshake failed: {err}")
                                    }
                                    Err(StreamError::IoError(err)) => {
                                        panic!("TLS handshake failed: {err}")
                                    }
                                };
                                tls.write_all(b"ping").await.unwrap();
                                let mut pong = [0; 4];
                                tls.read_exact(&mut pong).await.unwrap();
                                assert_eq!(&pong, b"pong");
                            } else {
                                let error = match connection {
                                    Ok(_) => panic!("certificate mismatch unexpectedly accepted"),
                                    Err(StreamError::TlsError(err)) => err.to_string(),
                                    Err(StreamError::IoError(err)) => err.to_string(),
                                };
                                assert!(error.to_lowercase().contains("certificate"), "{error}");
                            }
                        };
                        tokio::join!(server, client);
                    })
                    .await
                    .unwrap_or_else(|_| panic!("timed out testing TLS name {name}"));
                }
            });
    }
}
