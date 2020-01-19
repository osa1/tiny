use std::{
    net::SocketAddr,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};

#[cfg(feature = "tls-native")]
use tokio_tls::TlsStream;
#[cfg(feature = "tls-rustls")]
use tokio_rustls::client::TlsStream;

#[derive(Debug)]
pub(crate) enum Stream {
    TcpStream(TcpStream),
    TlsStream(TlsStream<TcpStream>),
}

#[cfg(feature = "tls-native")]
pub(crate) type TlsError = native_tls::Error;
#[cfg(feature = "tls-rustls")]
pub(crate) type TlsError = rustls::TLSError;

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
        Ok(Stream::TcpStream(TcpStream::connect(addr).await?))
    }

    #[cfg(feature = "tls-native")]
    pub(crate) async fn new_tls(addr: SocketAddr, host_name: &str) -> Result<Stream, StreamError> {
        let tcp_stream = TcpStream::connect(addr).await?;
        let tls_connector =
            tokio_tls::TlsConnector::from(native_tls::TlsConnector::builder().build()?);
        let tls_stream = tls_connector.connect(host_name, tcp_stream).await?;
        Ok(Stream::TlsStream(tls_stream))
    }

    #[cfg(feature = "tls-rustls")]
    pub(crate) async fn new_tls(addr: SocketAddr, host_name: &str) -> Result<Stream, StreamError> {
        let tcp_stream = TcpStream::connect(addr).await?;
        let config = std::sync::Arc::new(rustls::ClientConfig::default());
        let tls_connector = tokio_rustls::TlsConnector::from(config);
        let name = webpki::DNSNameRef::try_from_ascii_str(host_name)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        let tls_stream = tls_connector.connect(name, tcp_stream).await?;
        Ok(Stream::TlsStream(tls_stream))
    }
}

//
// Boilerplate
//

impl AsyncRead for Stream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &mut [u8],
    ) -> Poll<Result<usize, std::io::Error>> {
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
