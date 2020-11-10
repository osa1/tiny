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

#[cfg(feature = "tls-native")]
lazy_static! {
    static ref TLS_CONNECTOR: tokio_native_tls::TlsConnector =
        tokio_native_tls::TlsConnector::from(native_tls::TlsConnector::builder().build().unwrap());
}

#[cfg(feature = "tls-rustls")]
lazy_static! {
    static ref TLS_CONNECTOR: tokio_rustls::TlsConnector = {
        let mut config = tokio_rustls::rustls::ClientConfig::default();
        config.root_store = rustls_native_certs::load_native_certs().unwrap();
        tokio_rustls::TlsConnector::from(std::sync::Arc::new(config))
    };
}

#[derive(Debug)]
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
pub(crate) type TlsError = tokio_rustls::rustls::TLSError;

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
    pub(crate) async fn new_tls(addr: SocketAddr, host_name: &str) -> Result<Stream, StreamError> {
        let tcp_stream = TcpStream::connect(addr).await?;
        let tls_stream = TLS_CONNECTOR.connect(host_name, tcp_stream).await?;
        Ok(Stream::TlsStream(tls_stream.into()))
    }

    #[cfg(feature = "tls-rustls")]
    pub(crate) async fn new_tls(addr: SocketAddr, host_name: &str) -> Result<Stream, StreamError> {
        let tcp_stream = TcpStream::connect(addr).await?;
        let name = tokio_rustls::webpki::DNSNameRef::try_from_ascii_str(host_name)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        let tls_stream = TLS_CONNECTOR.connect(name, tcp_stream).await?;
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
