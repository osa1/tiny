//! Provides an abstraction over the standard `TcpStream` and `native_tls`'s `TlsStream`.

/*
use mio::PollOpt;
use mio::Ready;
use mio::Token;
use native_tls as tls;
use net2::TcpBuilder;
use net2::TcpStreamExt;
use std::io::Read;
use std::io::Write;
use std::io;
use std::net::SocketAddr;
use std::net::ToSocketAddrs;
use std::net;
use std::os::unix::io::{AsRawFd, RawFd};
use std::result::Result;
use take_mut::take;
*/

pub mod tcp;
pub mod utils;

/*
enum TlsStream<'poll> {
    Handshake(tls::MidHandshakeTlsStream<TcpStream<'poll>>),
    Connected(tls::TlsStream<TcpStream<'poll>>),
    Broken,
}

impl<'poll> TlsStream<'poll> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, TlsError> {
        let mut ret = Ok(0);
        take(self, |s| match s {
            TlsStream::Handshake(mid) =>
                match mid.handshake() {
                    Ok(stream) =>
                        TlsStream::Connected(stream),
                    Err(tls::HandshakeError::Interrupted(mid)) =>
                        TlsStream::Handshake(mid),
                    Err(tls::HandshakeError::Failure(err)) => {
                        ret = Err(TlsError::TlsError(err));
                        TlsStream::Broken
                    }
                }
            TlsStream::Connected(mut s) => {
                ret = s.read(buf).map_err(|io_err| TlsError::TcpError(TcpError::IoError(io_err)));
                TlsStream::Connected(s)
            }
            TlsStream::Broken =>
                TlsStream::Broken
        });
        ret
    }
}

#[derive(Debug)]
pub enum TlsError {
    TcpError(TcpError),
    TlsError(tls::Error),
}

pub enum Stream<'poll> {
    Tls(TlsStream<'poll>),
    Tcp(TcpStream<'poll>),
}

impl<'poll> Stream<'poll> {
    pub fn new_tls(
        poll: &'poll Poll,
        serv_addr: &str,
        serv_port: u16,
        domain: &str,
    ) -> Result<Stream<'poll>, TlsError> {
        let connector = tls::TlsConnector::builder().unwrap().build().unwrap();
        let tcp_stream: TcpStream<'poll> =
            TcpStream::new(poll, serv_addr, serv_port).map_err(TlsError::TcpError)?;
        match connector.connect(domain, tcp_stream) {
            Ok(tls_stream) =>
                Ok(Stream::Tls(TlsStream::Connected(tls_stream))),
            Err(tls::HandshakeError::Interrupted(mid)) =>
                Ok(Stream::Tls(TlsStream::Handshake(mid))),
            Err(tls::HandshakeError::Failure(err)) =>
                Err(TlsError::TlsError(err)),
        }
    }

    pub fn new_tcp(
        poll: &'poll Poll,
        serv_addr: &str,
        serv_port: u16,
    ) -> Result<Stream<'poll>, TcpError> {
        TcpStream::new(poll, serv_addr, serv_port).map(Stream::Tcp)
    }

    pub fn send(&mut self) -> io::Result<()> {
        match *self {
            Stream::Tcp(ref mut stream) =>
                stream.send(),
            Stream::Tls(TlsStream::Handshake(ref mut mid)) =>
                mid.get_mut().send(),
            Stream::Tls(TlsStream::Connected(ref mut stream)) =>
                stream.get_mut().send(),
            Stream::Tls(TlsStream::Broken) =>
                Ok(()),
        }
    }

    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize, TlsError> {
        match *self {
            Stream::Tls(ref mut stream) =>
                stream.read(buf),
            Stream::Tcp(ref mut stream) =>
                stream.read(buf).map_err(|io_err| TlsError::TcpError(TcpError::IoError(io_err))),
        }
    }
}


////////////////////////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {

    extern crate test;

    use mio::Events;
    use std::time::Duration;
    use std::io::Read;
    use super::*;
    use std::str;

    #[test]
    fn connect_freenode_tcp() {
        let poll = Poll::new().unwrap();
        let mut stream = Stream::new_tcp(&poll, "chat.freenode.net", 6667).unwrap();
        let mut read_buf = Vec::with_capacity(1024);

        let mut events = Events::with_capacity(10);
        'mainloop: loop {
            match poll.poll(&mut events, Some(Duration::from_secs(10))) {
                Err(err) => {
                    panic!("poll error: {:?}", err);
                },
                Ok(_) => {
                    for event in &events {
                        if event.readiness().is_readable() {
                            read_buf.clear();
                            assert!(stream.read(&mut read_buf).is_ok());
                            println!("read: {:?}", str::from_utf8(&read_buf));
                            break 'mainloop;
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn connect_freenode_tls() {
        let poll = Poll::new().unwrap();
        let mut stream = Stream::new_tls(&poll, "chat.freenode.net", 6697, "freenode").unwrap();
        let mut read_buf = Vec::with_capacity(1024);

        let mut events = Events::with_capacity(10);
        'mainloop: loop {
            match poll.poll(&mut events, Some(Duration::from_secs(10))) {
                Err(err) => {
                    panic!("poll error: {:?}", err);
                },
                Ok(_) => {
                    for event in &events {
                        if event.readiness().is_readable() {
                            read_buf.clear();
                            stream.read(&mut read_buf).unwrap();
                            println!("read: {:?}", str::from_utf8(&read_buf));
                            // break 'mainloop;
                        }
                        if event.readiness().is_writable() {
                            stream.send().unwrap();
                        }
                    }
                }
            }
        }
    }
}
*/
