use crate::{ProxyAuth, ProxyConfig, ProxyProtocol, SASLAuth, ServerInfo};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::sync::LazyLock;
use std::{
    io,
    net::{IpAddr, SocketAddr, SocketAddrV4},
    pin::Pin,
    task::{Context, Poll},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader, ReadBuf},
    net::TcpStream,
};
use tokio_socks::{
    TargetAddr,
    tcp::{Socks4Stream, Socks5Stream},
};

#[cfg(feature = "tls-native")]
use tokio_native_tls::TlsStream;
#[cfg(feature = "tls-rustls")]
use tokio_rustls::client::TlsStream;

type HttpStream = BufReader<TcpStream>;
type HttpsStream = BufReader<TlsStream<TcpStream>>;

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

pub(crate) enum Stream {
    Tcp(Box<TcpStream>),
    Tls(Box<TlsStream<TcpStream>>),
    Http(Box<HttpStream>),
    HttpTls(Box<TlsStream<HttpStream>>),
    Https(Box<HttpsStream>),
    HttpsTls(Box<TlsStream<HttpsStream>>),
}

#[cfg(feature = "tls-native")]
pub(crate) type TlsError = native_tls::Error;
#[cfg(feature = "tls-rustls")]
pub(crate) type TlsError = tokio_rustls::rustls::Error;

#[derive(Debug)]
pub(crate) enum StreamError {
    Tls(TlsError),
    Io(io::Error),
    Proxy(io::Error),
}

impl From<TlsError> for StreamError {
    fn from(err: TlsError) -> Self {
        Self::Tls(err)
    }
}
impl From<io::Error> for StreamError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

fn proxy_error(message: &'static str) -> StreamError {
    StreamError::Proxy(io::Error::other(message))
}

pub(crate) fn validate_proxy_destination(host: &str) -> Result<(), StreamError> {
    let host = host
        .strip_prefix('[')
        .and_then(|host| host.strip_suffix(']'))
        .unwrap_or(host);
    if let Some((ip, _scope)) = host.split_once('%')
        && ip.parse::<std::net::Ipv6Addr>().is_ok()
    {
        return Err(proxy_error(
            "scoped IPv6 destinations are not supported by proxies",
        ));
    }
    Ok(())
}

#[cfg(feature = "tls-native")]
fn split_identity_pem(pem: &[u8]) -> io::Result<(Vec<u8>, Vec<u8>)> {
    fn append_pem(output: &mut Vec<u8>, der: &[u8], header: &[u8], footer: &[u8]) {
        output.extend_from_slice(header);
        for chunk in der.chunks(48) {
            let mut encoded = [0; 64];
            let len = STANDARD
                .encode_slice(chunk, &mut encoded)
                .expect("48 bytes fit in a 64-byte base64 line");
            output.extend_from_slice(&encoded[..len]);
            output.push(b'\n');
        }
        output.extend_from_slice(footer);
    }

    let mut input = pem;
    let mut certificates = Vec::new();
    let mut key = Vec::new();
    while let Some(item) = rustls_pemfile::read_one(&mut input)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid client identity PEM"))?
    {
        match item {
            rustls_pemfile::Item::X509Certificate(der) => append_pem(
                &mut certificates,
                &der,
                b"-----BEGIN CERTIFICATE-----\n",
                b"-----END CERTIFICATE-----\n",
            ),
            rustls_pemfile::Item::PKCS8Key(der) if key.is_empty() => append_pem(
                &mut key,
                &der,
                b"-----BEGIN PRIVATE KEY-----\n",
                b"-----END PRIVATE KEY-----\n",
            ),
            _ => {}
        }
    }
    if key.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "missing client private key",
        ));
    }
    if certificates.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "missing client certificate",
        ));
    }
    Ok((certificates, key))
}

#[cfg(feature = "tls-native")]
fn tls_connector(
    identity: Option<&Vec<u8>>,
    root: Option<&[u8]>,
) -> Result<tokio_native_tls::TlsConnector, StreamError> {
    let mut builder = native_tls::TlsConnector::builder();
    if let Some(pem) = identity {
        let (certificates, key) = split_identity_pem(pem)?;
        builder.identity(native_tls::Identity::from_pkcs8(&certificates, &key)?);
    }
    if let Some(der) = root {
        builder.add_root_certificate(native_tls::Certificate::from_der(der)?);
    }
    Ok(tokio_native_tls::TlsConnector::from(builder.build()?))
}

#[cfg(feature = "tls-native")]
async fn tls_connect<S: AsyncRead + AsyncWrite + Unpin>(
    stream: S,
    host: &str,
    identity: Option<&Vec<u8>>,
    root: Option<&[u8]>,
) -> Result<TlsStream<S>, StreamError> {
    static DEFAULT: LazyLock<Result<tokio_native_tls::TlsConnector, ()>> =
        LazyLock::new(|| tls_connector(None, None).map_err(|_| ()));
    let custom;
    let connector = if identity.is_none() && root.is_none() {
        DEFAULT
            .as_ref()
            .map_err(|_| io::Error::other("could not initialize TLS connector"))?
    } else {
        custom = tls_connector(identity, root)?;
        &custom
    };
    Ok(connector.connect(tls_server_name(host), stream).await?)
}

#[cfg(feature = "tls-rustls")]
fn tls_connector(
    identity: Option<&Vec<u8>>,
    root: Option<&[u8]>,
) -> Result<tokio_rustls::TlsConnector, StreamError> {
    use std::io::{Cursor, Seek, SeekFrom};
    use tokio_rustls::rustls::{Certificate, ClientConfig, PrivateKey, RootCertStore};
    let mut roots = RootCertStore::empty();
    for cert in rustls_native_certs::load_native_certs()? {
        roots.add(&Certificate(cert.0))?;
    }
    if let Some(der) = root {
        roots.add(&Certificate(der.to_vec()))?;
    }
    let builder = ClientConfig::builder()
        .with_safe_defaults()
        .with_root_certificates(roots);
    let config = if let Some(pem) = identity {
        let mut buf = Cursor::new(pem);
        let certs = rustls_pemfile::certs(&mut buf)?
            .into_iter()
            .map(Certificate)
            .collect();
        buf.seek(SeekFrom::Start(0))?;
        let key = rustls_pemfile::pkcs8_private_keys(&mut buf)?
            .pop()
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "missing client private key")
            })?;
        builder.with_client_auth_cert(certs, PrivateKey(key))?
    } else {
        builder.with_no_client_auth()
    };
    Ok(tokio_rustls::TlsConnector::from(std::sync::Arc::new(
        config,
    )))
}

#[cfg(feature = "tls-rustls")]
async fn tls_connect<S: AsyncRead + AsyncWrite + Unpin>(
    stream: S,
    host: &str,
    identity: Option<&Vec<u8>>,
    root: Option<&[u8]>,
) -> Result<TlsStream<S>, StreamError> {
    use tokio_rustls::rustls::ServerName;
    static DEFAULT: LazyLock<Result<tokio_rustls::TlsConnector, ()>> =
        LazyLock::new(|| tls_connector(None, None).map_err(|_| ()));
    let name = ServerName::try_from(tls_server_name(host))
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid TLS server name"))?;
    let custom;
    let connector = if identity.is_none() && root.is_none() {
        DEFAULT
            .as_ref()
            .map_err(|_| io::Error::other("could not initialize TLS connector"))?
    } else {
        custom = tls_connector(identity, root)?;
        &custom
    };
    Ok(connector.connect(name, stream).await?)
}

fn validate_proxy(proxy: &ProxyConfig, host: &str) -> Result<(), StreamError> {
    if proxy.addr.trim().is_empty() || proxy.addr.chars().any(char::is_control) || proxy.port == 0 {
        return Err(proxy_error("invalid proxy address"));
    }
    match (&proxy.protocol, &proxy.auth) {
        (ProxyProtocol::Http | ProxyProtocol::Https, None) => (),
        (
            ProxyProtocol::Http | ProxyProtocol::Https,
            Some(ProxyAuth::UserPass { username, password }),
        ) if !username.chars().any(|c| c == ':' || c.is_control())
            && !password.chars().any(char::is_control) => {}
        (ProxyProtocol::Socks4 { .. }, None) => (),
        (ProxyProtocol::Socks4 { .. }, Some(ProxyAuth::UserId(id)))
            if (1..=255).contains(&id.len()) && !id.contains('\0') => {}
        (ProxyProtocol::Socks5, None) => (),
        (ProxyProtocol::Socks5, Some(ProxyAuth::UserPass { username, password }))
            if (1..=255).contains(&username.len()) && (1..=255).contains(&password.len()) => {}
        _ => return Err(proxy_error("invalid proxy authentication")),
    }
    match proxy.protocol {
        ProxyProtocol::Http | ProxyProtocol::Https => validate_http_host(host)?,
        ProxyProtocol::Socks4 { .. } | ProxyProtocol::Socks5 => {
            if host.parse::<IpAddr>().is_err()
                && (host.is_empty() || host.len() > 255 || host.contains('\0'))
            {
                return Err(proxy_error("invalid SOCKS target"));
            }
        }
    }
    Ok(())
}

fn validate_http_host(host: &str) -> Result<(), StreamError> {
    if host.parse::<IpAddr>().is_ok() {
        return Ok(());
    }
    if host.is_empty()
        || host
            .chars()
            .any(|c| c.is_control() || c.is_whitespace() || ":/?#@[]\\!$&'()*+,;=".contains(c))
    {
        return Err(proxy_error("invalid HTTP CONNECT target"));
    }
    Ok(())
}

fn http_authority(host: &str, port: u16) -> Result<String, StreamError> {
    validate_http_host(host)?;
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(match ip {
            IpAddr::V4(_) => format!("{ip}:{port}"),
            IpAddr::V6(_) => format!("[{ip}]:{port}"),
        });
    }
    Ok(format!("{host}:{port}"))
}

async fn http_connect<S: AsyncRead + AsyncWrite + Unpin>(
    stream: S,
    host: &str,
    port: u16,
    auth: Option<&ProxyAuth>,
) -> Result<BufReader<S>, StreamError> {
    let authority = http_authority(host, port)?;
    let mut request = format!("CONNECT {authority} HTTP/1.1\r\nHost: {authority}\r\n");
    if let Some(ProxyAuth::UserPass { username, password }) = auth {
        request.push_str("Proxy-Authorization: Basic ");
        STANDARD.encode_string(format!("{username}:{password}"), &mut request);
        request.push_str("\r\n");
    }
    request.push_str("\r\n");
    let mut stream = BufReader::new(stream);
    stream
        .get_mut()
        .write_all(request.as_bytes())
        .await
        .map_err(|_| proxy_error("HTTP proxy request failed"))?;
    let mut total = 0;
    loop {
        let mut header = Vec::new();
        loop {
            let available = stream
                .fill_buf()
                .await
                .map_err(|_| proxy_error("HTTP proxy response failed"))?;
            if available.is_empty() {
                return Err(proxy_error("incomplete HTTP proxy response"));
            }
            let mut used = 0;
            for &byte in available {
                header.push(byte);
                used += 1;
                total += 1;
                if total > 8192 {
                    return Err(proxy_error("HTTP proxy response too large"));
                }
                if header.ends_with(b"\r\n\r\n") {
                    break;
                }
            }
            let complete = header.ends_with(b"\r\n\r\n");
            stream.consume(used);
            if complete {
                break;
            }
        }
        let mut headers = [httparse::EMPTY_HEADER; 64];
        let mut response = httparse::Response::new(&mut headers);
        match response.parse(&header) {
            Ok(httparse::Status::Complete(_)) if matches!(response.version, Some(0 | 1)) => (),
            _ => return Err(proxy_error("invalid HTTP proxy response")),
        }
        match response.code {
            Some(101) => return Err(proxy_error("HTTP proxy rejected tunnel")),
            Some(100..=199) => continue,
            Some(200..=299) => return Ok(stream),
            _ => return Err(proxy_error("HTTP proxy rejected tunnel")),
        }
    }
}

impl Stream {
    pub(crate) async fn connect(
        tcp_addr: SocketAddr,
        info: &ServerInfo,
        irc_port: u16,
        socks4_target: Option<SocketAddrV4>,
    ) -> Result<Stream, StreamError> {
        Self::connect_inner(tcp_addr, info, irc_port, socks4_target, None).await
    }

    #[cfg(test)]
    pub(crate) async fn connect_with_test_root(
        tcp_addr: SocketAddr,
        info: &ServerInfo,
        irc_port: u16,
        socks4_target: Option<SocketAddrV4>,
        root_cert_der: &[u8],
    ) -> Result<Stream, StreamError> {
        Self::connect_inner(tcp_addr, info, irc_port, socks4_target, Some(root_cert_der)).await
    }

    async fn connect_inner(
        tcp_addr: SocketAddr,
        info: &ServerInfo,
        irc_port: u16,
        socks4_target: Option<SocketAddrV4>,
        root: Option<&[u8]>,
    ) -> Result<Stream, StreamError> {
        if let Some(proxy) = &info.proxy {
            validate_proxy_destination(&info.addr)?;
            validate_proxy(proxy, &info.addr)?;
        }
        let tcp = TcpStream::connect(tcp_addr).await.map_err(|e| {
            if info.proxy.is_some() {
                proxy_error("proxy TCP connection failed")
            } else {
                StreamError::Io(e)
            }
        })?;
        let identity = match &info.sasl_auth {
            Some(SASLAuth::External { pem }) => Some(pem),
            _ => None,
        };
        match &info.proxy {
            None => {
                if info.tls {
                    Ok(Stream::Tls(Box::new(
                        tls_connect(tcp, &info.addr, identity, root).await?,
                    )))
                } else {
                    Ok(Stream::Tcp(Box::new(tcp)))
                }
            }
            Some(proxy) => match proxy.protocol {
                ProxyProtocol::Http => {
                    let stream =
                        http_connect(tcp, &info.addr, irc_port, proxy.auth.as_ref()).await?;
                    if info.tls {
                        Ok(Stream::HttpTls(Box::new(
                            tls_connect(stream, &info.addr, identity, root).await?,
                        )))
                    } else {
                        Ok(Stream::Http(Box::new(stream)))
                    }
                }
                ProxyProtocol::Https => {
                    let outer = tls_connect(tcp, &proxy.addr, None, root)
                        .await
                        .map_err(|_| proxy_error("HTTPS proxy TLS failed"))?;
                    let stream =
                        http_connect(outer, &info.addr, irc_port, proxy.auth.as_ref()).await?;
                    if info.tls {
                        Ok(Stream::HttpsTls(Box::new(
                            tls_connect(stream, &info.addr, identity, root).await?,
                        )))
                    } else {
                        Ok(Stream::Https(Box::new(stream)))
                    }
                }
                ProxyProtocol::Socks4 { remote_dns } => {
                    let target = if let Some(addr) = socks4_target {
                        TargetAddr::Ip(SocketAddr::V4(addr))
                    } else if let Ok(ip) = info.addr.parse::<IpAddr>() {
                        match ip {
                            IpAddr::V4(ip) => {
                                TargetAddr::Ip(SocketAddr::new(IpAddr::V4(ip), irc_port))
                            }
                            IpAddr::V6(_) => {
                                return Err(proxy_error("SOCKS4 does not support IPv6"));
                            }
                        }
                    } else if remote_dns {
                        TargetAddr::Domain(info.addr.as_str().into(), irc_port)
                    } else {
                        return Err(proxy_error("SOCKS4 requires an IPv4 target"));
                    };
                    let id = match &proxy.auth {
                        Some(ProxyAuth::UserId(id)) => Some(id.as_str()),
                        _ => None,
                    };
                    if let TargetAddr::Domain(domain, _) = &target
                        && domain.len() + id.map_or(0, str::len) > 503
                    {
                        return Err(proxy_error("SOCKS4 target too long"));
                    }
                    let tcp = match id {
                        Some(id) => {
                            Socks4Stream::connect_with_userid_and_socket(tcp, target, id).await
                        }
                        None => Socks4Stream::connect_with_socket(tcp, target).await,
                    }
                    .map_err(|_| proxy_error("SOCKS4 proxy negotiation failed"))?
                    .into_inner();
                    if info.tls {
                        Ok(Stream::Tls(Box::new(
                            tls_connect(tcp, &info.addr, identity, root).await?,
                        )))
                    } else {
                        Ok(Stream::Tcp(Box::new(tcp)))
                    }
                }
                ProxyProtocol::Socks5 => {
                    let target = match info.addr.parse::<IpAddr>() {
                        Ok(ip) => TargetAddr::Ip(SocketAddr::new(ip, irc_port)),
                        Err(_) => TargetAddr::Domain(info.addr.as_str().into(), irc_port),
                    };
                    let tcp = match &proxy.auth {
                        Some(ProxyAuth::UserPass { username, password }) => {
                            Socks5Stream::connect_with_password_and_socket(
                                tcp, target, username, password,
                            )
                            .await
                        }
                        _ => Socks5Stream::connect_with_socket(tcp, target).await,
                    }
                    .map_err(|_| proxy_error("SOCKS5 proxy negotiation failed"))?
                    .into_inner();
                    if info.tls {
                        Ok(Stream::Tls(Box::new(
                            tls_connect(tcp, &info.addr, identity, root).await?,
                        )))
                    } else {
                        Ok(Stream::Tcp(Box::new(tcp)))
                    }
                }
            },
        }
    }
}

impl AsyncRead for Stream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match &mut *self {
            Self::Tcp(s) => Pin::new(s).poll_read(cx, buf),
            Self::Tls(s) => Pin::new(s).poll_read(cx, buf),
            Self::Http(s) => Pin::new(s).poll_read(cx, buf),
            Self::HttpTls(s) => Pin::new(s).poll_read(cx, buf),
            Self::Https(s) => Pin::new(s).poll_read(cx, buf),
            Self::HttpsTls(s) => Pin::new(s).poll_read(cx, buf),
        }
    }
}
impl AsyncWrite for Stream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match &mut *self {
            Self::Tcp(s) => Pin::new(s).poll_write(cx, buf),
            Self::Tls(s) => Pin::new(s).poll_write(cx, buf),
            Self::Http(s) => Pin::new(s).poll_write(cx, buf),
            Self::HttpTls(s) => Pin::new(s).poll_write(cx, buf),
            Self::Https(s) => Pin::new(s).poll_write(cx, buf),
            Self::HttpsTls(s) => Pin::new(s).poll_write(cx, buf),
        }
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match &mut *self {
            Self::Tcp(s) => Pin::new(s).poll_flush(cx),
            Self::Tls(s) => Pin::new(s).poll_flush(cx),
            Self::Http(s) => Pin::new(s).poll_flush(cx),
            Self::HttpTls(s) => Pin::new(s).poll_flush(cx),
            Self::Https(s) => Pin::new(s).poll_flush(cx),
            Self::HttpsTls(s) => Pin::new(s).poll_flush(cx),
        }
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match &mut *self {
            Self::Tcp(s) => Pin::new(s).poll_shutdown(cx),
            Self::Tls(s) => Pin::new(s).poll_shutdown(cx),
            Self::Http(s) => Pin::new(s).poll_shutdown(cx),
            Self::HttpTls(s) => Pin::new(s).poll_shutdown(cx),
            Self::Https(s) => Pin::new(s).poll_shutdown(cx),
            Self::HttpsTls(s) => Pin::new(s).poll_shutdown(cx),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Stream, StreamError, tls_server_name};
    use crate::ServerInfo;
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

    #[cfg(feature = "tls-native")]
    #[test]
    fn invalid_client_identity_pem_is_rejected() {
        let client = rcgen::generate_simple_self_signed(vec!["client.test".into()]).unwrap();
        let certificate = client.serialize_pem().unwrap();
        let key = client.serialize_private_key_pem();
        let cases = [
            (
                certificate.into_bytes(),
                std::io::ErrorKind::InvalidInput,
                "missing client private key",
            ),
            (
                key.as_bytes().to_vec(),
                std::io::ErrorKind::InvalidInput,
                "missing client certificate",
            ),
            (
                format!(
                    "{key}-----BEGIN CERTIFICATE-----\nsecret!invalid\n-----END CERTIFICATE-----\n"
                )
                .into_bytes(),
                std::io::ErrorKind::InvalidData,
                "invalid client identity PEM",
            ),
        ];
        for (pem, kind, message) in cases {
            let error = match super::tls_connector(Some(&pem), None) {
                Err(StreamError::Io(error)) => error,
                Err(error) => panic!("unexpected identity error: {error:?}"),
                Ok(_) => panic!("invalid client identity was accepted"),
            };
            assert_eq!(error.kind(), kind);
            assert_eq!(error.to_string(), message);
            assert!(!format!("{error:?} {error}").contains("secret"));
        }
    }

    #[test]
    fn scoped_ipv6_tls() {
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
                let mut roots = rustls_pemfile::certs(
                    &mut &include_bytes!("../tests/fixtures/ipv6-ca.pem")[..],
                )
                .expect("invalid IPv6 test CA");
                assert_eq!(roots.len(), 1, "expected exactly one IPv6 test CA");
                let root = roots.pop().unwrap();
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
                            let address = listener.local_addr().unwrap();
                            let info = ServerInfo {
                                addr: name.to_owned(),
                                port: address.port(),
                                tls: true,
                                proxy: None,
                                user: None,
                                realname: "Test User".to_owned(),
                                pass: None,
                                nicks: vec!["testnick".to_owned()],
                                auto_join: vec![],
                                nickserv_ident: None,
                                sasl_auth: None,
                            };
                            let connection = Stream::connect_with_test_root(
                                address, &info, info.port, None, &root,
                            )
                            .await;
                            if succeeds {
                                let mut tls = match connection {
                                    Ok(tls) => tls,
                                    Err(StreamError::Tls(err)) => {
                                        panic!("TLS handshake failed: {err}")
                                    }
                                    Err(StreamError::Io(err)) => {
                                        panic!("TLS handshake failed: {err}")
                                    }
                                    Err(StreamError::Proxy(err)) => {
                                        panic!("unexpected proxy error: {err}")
                                    }
                                };
                                tls.write_all(b"ping").await.unwrap();
                                let mut pong = [0; 4];
                                tls.read_exact(&mut pong).await.unwrap();
                                assert_eq!(&pong, b"pong");
                            } else {
                                match connection {
                                    Ok(_) => panic!("certificate mismatch unexpectedly accepted"),
                                    #[cfg(feature = "tls-native")]
                                    Err(StreamError::Tls(error)) => {
                                        let message = error.to_string();
                                        assert!(
                                            message.to_lowercase().contains("certificate"),
                                            "{message}"
                                        );
                                    }
                                    #[cfg(feature = "tls-rustls")]
                                    Err(StreamError::Io(error)) => {
                                        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
                                        let tls_error = error.get_ref().and_then(|source| {
                                            source.downcast_ref::<tokio_rustls::rustls::Error>()
                                        });
                                        assert!(
                                            matches!(
                                                tls_error,
                                                Some(tokio_rustls::rustls::Error::InvalidCertificate(
                                                    tokio_rustls::rustls::CertificateError::NotValidForName
                                                ))
                                            ),
                                            "unexpected TLS error: {error:?}"
                                        );
                                    }
                                    #[cfg(feature = "tls-rustls")]
                                    Err(StreamError::Tls(error)) => {
                                        panic!("unexpected TLS error: {error}")
                                    }
                                    #[cfg(feature = "tls-native")]
                                    Err(StreamError::Io(error)) => {
                                        panic!("unexpected I/O error: {error}")
                                    }
                                    Err(StreamError::Proxy(error)) => {
                                        panic!("unexpected proxy error: {error}")
                                    }
                                }
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
