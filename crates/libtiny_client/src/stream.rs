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

pub(crate) enum Stream {
    TcpStream(Box<TcpStream>),
    TlsStream(Box<TlsStream<TcpStream>>),
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
    TlsError(TlsError),
    IoError(io::Error),
    ProxyError(io::Error),
}

impl From<TlsError> for StreamError {
    fn from(err: TlsError) -> Self {
        Self::TlsError(err)
    }
}
impl From<io::Error> for StreamError {
    fn from(err: io::Error) -> Self {
        Self::IoError(err)
    }
}

fn proxy_error(message: &'static str) -> StreamError {
    StreamError::ProxyError(io::Error::new(io::ErrorKind::Other, message))
}

pub(crate) fn validate_proxy_destination(host: &str) -> Result<(), StreamError> {
    let host = host
        .strip_prefix('[')
        .and_then(|host| host.strip_suffix(']'))
        .unwrap_or(host);
    if let Some((ip, _scope)) = host.split_once('%') {
        if ip.parse::<std::net::Ipv6Addr>().is_ok() {
            return Err(proxy_error(
                "scoped IPv6 destinations are not supported by proxies",
            ));
        }
    }
    Ok(())
}

#[cfg(feature = "tls-native")]
fn tls_connector(
    identity: Option<&Vec<u8>>,
    root: Option<&[u8]>,
) -> Result<tokio_native_tls::TlsConnector, StreamError> {
    let mut builder = native_tls::TlsConnector::builder();
    if let Some(pem) = identity {
        const KEY: &[u8] = b"-----BEGIN PRIVATE KEY-----";
        let key_start = pem
            .windows(KEY.len())
            .position(|window| window == KEY)
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "missing client private key")
            })?;
        builder.identity(native_tls::Identity::from_pkcs8(pem, &pem[key_start..])?);
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
    Ok(connector.connect(host, stream).await?)
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
    let name = ServerName::try_from(host)
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
            && !password.chars().any(char::is_control) =>
        {
            ()
        }
        (ProxyProtocol::Socks4 { .. }, None) => (),
        (ProxyProtocol::Socks4 { .. }, Some(ProxyAuth::UserId(id)))
            if (1..=255).contains(&id.len()) && !id.contains('\0') =>
        {
            ()
        }
        (ProxyProtocol::Socks5, None) => (),
        (ProxyProtocol::Socks5, Some(ProxyAuth::UserPass { username, password }))
            if (1..=255).contains(&username.len()) && (1..=255).contains(&password.len()) =>
        {
            ()
        }
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
                StreamError::IoError(e)
            }
        })?;
        let identity = match &info.sasl_auth {
            Some(SASLAuth::External { pem }) => Some(pem),
            _ => None,
        };
        match &info.proxy {
            None => {
                if info.tls {
                    Ok(Stream::TlsStream(Box::new(
                        tls_connect(tcp, &info.addr, identity, root).await?,
                    )))
                } else {
                    Ok(Stream::TcpStream(Box::new(tcp)))
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
                    if let TargetAddr::Domain(domain, _) = &target {
                        if domain.len() + id.map_or(0, str::len) > 503 {
                            return Err(proxy_error("SOCKS4 target too long"));
                        }
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
                        Ok(Stream::TlsStream(Box::new(
                            tls_connect(tcp, &info.addr, identity, root).await?,
                        )))
                    } else {
                        Ok(Stream::TcpStream(Box::new(tcp)))
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
                        Ok(Stream::TlsStream(Box::new(
                            tls_connect(tcp, &info.addr, identity, root).await?,
                        )))
                    } else {
                        Ok(Stream::TcpStream(Box::new(tcp)))
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
            Self::TcpStream(s) => Pin::new(s).poll_read(cx, buf),
            Self::TlsStream(s) => Pin::new(s).poll_read(cx, buf),
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
            Self::TcpStream(s) => Pin::new(s).poll_write(cx, buf),
            Self::TlsStream(s) => Pin::new(s).poll_write(cx, buf),
            Self::Http(s) => Pin::new(s).poll_write(cx, buf),
            Self::HttpTls(s) => Pin::new(s).poll_write(cx, buf),
            Self::Https(s) => Pin::new(s).poll_write(cx, buf),
            Self::HttpsTls(s) => Pin::new(s).poll_write(cx, buf),
        }
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match &mut *self {
            Self::TcpStream(s) => Pin::new(s).poll_flush(cx),
            Self::TlsStream(s) => Pin::new(s).poll_flush(cx),
            Self::Http(s) => Pin::new(s).poll_flush(cx),
            Self::HttpTls(s) => Pin::new(s).poll_flush(cx),
            Self::Https(s) => Pin::new(s).poll_flush(cx),
            Self::HttpsTls(s) => Pin::new(s).poll_flush(cx),
        }
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match &mut *self {
            Self::TcpStream(s) => Pin::new(s).poll_shutdown(cx),
            Self::TlsStream(s) => Pin::new(s).poll_shutdown(cx),
            Self::Http(s) => Pin::new(s).poll_shutdown(cx),
            Self::HttpTls(s) => Pin::new(s).poll_shutdown(cx),
            Self::Https(s) => Pin::new(s).poll_shutdown(cx),
            Self::HttpsTls(s) => Pin::new(s).poll_shutdown(cx),
        }
    }
}
