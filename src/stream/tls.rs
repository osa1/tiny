use mio::Poll;
use mio::Token;
use take_mut::take;
use native_tls as tls;
use std::io::Read;
use std::io::Write;
use std::io;
use std::result::Result;

use stream::tcp::{TcpError, TcpStream};

pub enum TlsStream<'poll> {
    Handshake {
        stream: tls::MidHandshakeTlsStream<TcpStream<'poll>>,
        // send buffer to be able to provide `Write` impl
        out_buf: Vec<u8>,
    },
    Connected {
        stream: tls::TlsStream<TcpStream<'poll>>,
    },
    Broken,
}

#[derive(Debug)]
pub enum TlsError {
    TcpError(TcpError),
    TlsError(tls::Error),
    ConnectionClosed,
}

impl<'poll> TlsStream<'poll> {
    pub fn new(
        poll: &'poll Poll,
        serv_addr: &str,
        serv_port: u16,
        domain: &str,
    ) -> Result<TlsStream<'poll>, TlsError> {
        let connector = tls::TlsConnector::builder()
            .map_err(TlsError::TlsError)?
            .build()
            .map_err(TlsError::TlsError)?;
        let tcp_stream = TcpStream::new(poll, serv_addr, serv_port).map_err(TlsError::TcpError)?;
        match connector.connect(domain, tcp_stream) {
            Ok(tls_stream) =>
                Ok(TlsStream::Connected { stream: tls_stream }),
            Err(tls::HandshakeError::Interrupted(mid)) =>
                Ok(TlsStream::Handshake {
                    stream: mid,
                    out_buf: Vec::with_capacity(1024),
                }),
            Err(tls::HandshakeError::Failure(err)) =>
                Err(TlsError::TlsError(err)),
        }
    }

    pub fn write_ready(&mut self) -> io::Result<()> {
        match *self {
            TlsStream::Handshake { ref mut stream, .. } =>
                stream.get_mut().write_ready(),
            TlsStream::Connected { ref mut stream } =>
                stream.get_mut().write_ready(),
            TlsStream::Broken =>
                panic!("write_ready() called on broken tls stream"),
        }
    }

    pub fn read_ready(&mut self, buf: &mut [u8]) -> Result<usize, TlsError> {
        let mut ret = Ok(0);
        take(self, |s| match s {
            TlsStream::Handshake { stream, out_buf } =>
                match stream.handshake() {
                    Ok(mut tls_stream) => {
                        ret = tls_stream
                            .write(&out_buf)
                            .map_err(|err| TlsError::TcpError(TcpError::IoError(err)));
                        TlsStream::Connected { stream: tls_stream }
                    }
                    Err(tls::HandshakeError::Interrupted(mid)) =>
                        TlsStream::Handshake {
                            stream: mid,
                            out_buf,
                        },
                    Err(tls::HandshakeError::Failure(err)) => {
                        ret = Err(TlsError::TlsError(err));
                        TlsStream::Broken
                    }
                },
            TlsStream::Connected { mut stream } => {
                match stream.read(buf) {
                    Ok(0) => {
                        ret = Err(TlsError::ConnectionClosed);
                    }
                    Ok(n) => {
                        ret = Ok(n);
                    }
                    Err(err) => {
                        ret = Err(TlsError::TcpError(TcpError::IoError(err)));
                    }
                }
                TlsStream::Connected { stream }
            }
            TlsStream::Broken =>
                panic!("read_ready() called on broken tls stream"),
        });
        ret
    }

    pub fn get_tok(&self) -> Token {
        match *self {
            TlsStream::Handshake { ref stream, .. } =>
                stream.get_ref().get_tok(),
            TlsStream::Connected { ref stream } =>
                stream.get_ref().get_tok(),
            TlsStream::Broken =>
                panic!("get_tok() called on broken tls stream"),
        }
    }
}

impl<'poll> Write for TlsStream<'poll> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match *self {
            TlsStream::Handshake {
                ref mut out_buf, ..
            } => {
                out_buf.extend(buf);
                Ok(buf.len())
            }
            TlsStream::Connected { ref mut stream } =>
                stream.write(buf),
            TlsStream::Broken =>
                Ok(0),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match *self {
            TlsStream::Handshake { .. } | TlsStream::Broken =>
                Ok(()),
            TlsStream::Connected { ref mut stream } =>
                stream.flush(),
        }
    }
}
