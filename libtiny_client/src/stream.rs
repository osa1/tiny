use native_tls;
use std::{
    net::SocketAddr,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};
use tokio_tls::TlsStream;

#[derive(Debug)]
pub(crate) enum Stream {
    TcpStream(TcpStream),
    TlsStream(TlsStream<TcpStream>),
}

#[derive(Debug)]
pub(crate) enum StreamError {
    TlsError(native_tls::Error),
    IoError(std::io::Error),
}

impl From<native_tls::Error> for StreamError {
    fn from(err: native_tls::Error) -> Self {
        StreamError::TlsError(err)
    }
}

impl From<std::io::Error> for StreamError {
    fn from(err: std::io::Error) -> Self {
        StreamError::IoError(err)
    }
}

impl Stream {
    pub(crate) async fn new(
        addr: SocketAddr,
        host_name: &str,
        use_tls: bool,
    ) -> Result<Stream, StreamError> {
        if use_tls {
            Stream::new_tls(addr, host_name).await
        } else {
            Stream::new_tcp(addr).await
        }
    }

    async fn new_tcp(addr: SocketAddr) -> Result<Stream, StreamError> {
        Ok(Stream::TcpStream(TcpStream::connect(addr).await?))
    }

    async fn new_tls(addr: SocketAddr, host_name: &str) -> Result<Stream, StreamError> {
        let tcp_stream = TcpStream::connect(addr).await?;
        let tls_connector =
            tokio_tls::TlsConnector::from(native_tls::TlsConnector::builder().build()?);
        let tls_stream = tls_connector.connect(host_name, tcp_stream).await?;
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
