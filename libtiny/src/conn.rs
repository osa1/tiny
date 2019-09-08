use futures::future::FutureExt;
use futures::select;
use futures::stream::StreamExt;
use std::net::ToSocketAddrs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::split::{ReadHalf, WriteHalf};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::sync::watch;

pub enum ConnectError {
    CantResolveAddr,
    IoError(std::io::Error),
}

impl From<std::io::Error> for ConnectError {
    fn from(err: std::io::Error) -> ConnectError {
        ConnectError::IoError(err)
    }
}

pub enum ConnEv<A> {
    /// Network error happened
    Err(std::io::Error),

    /// An incoming message
    Msg(A),
}

pub async fn connect<A, F>(
    host: &str,
    port: u16,
    parse: F,
) -> Result<(mpsc::Sender<String>, mpsc::Receiver<ConnEv<A>>), ConnectError>
where
    A: Send + 'static,
    // FIXME: I shouldn't need sync here
    F: Fn(&Vec<u8>) -> Option<(A, usize)> + Send + Sync + 'static,
{
    //
    // Resolve IP address
    //

    let serv_name = host.to_owned();
    let mut addr_iter =
        tokio_executor::blocking::run(move || (serv_name.as_str(), port).to_socket_addrs()).await?;
    let addr = addr_iter.next().ok_or(ConnectError::CantResolveAddr)?;

    //
    // Establish TCP connection to the server
    //

    let stream = TcpStream::connect(&addr).await?;

    //
    // Spawn send/receiver tasks
    //

    let (read_half, write_half) = stream.split();
    let (incoming_snd, incoming_rcv) = mpsc::channel::<ConnEv<A>>(10);
    let (outgoing_snd, outgoing_rcv) = mpsc::channel::<String>(10);
    let (stop_snd, stop_rcv) = watch::channel::<()>(());

    // Spawn task to read the socket
    tokio::spawn(read_task(
        read_half,
        incoming_snd.clone(),
        stop_rcv.clone(),
        parse,
    ));
    // Spawn task to write to the socket
    tokio::spawn(write_task(write_half, outgoing_rcv, incoming_snd, stop_rcv));

    Ok((outgoing_snd, incoming_rcv))
}

async fn read_task<A, F>(
    mut sock: ReadHalf,
    mut ev_chan: mpsc::Sender<ConnEv<A>>,
    mut stop_chan: watch::Receiver<()>,
    parse: F,
) where
    A: Send + 'static,
    F: Fn(&Vec<u8>) -> Option<(A, usize)>,
{
    // TODO we could actually get away with just one buffer
    let mut read_buf: [u8; 512] = [0; 512];
    let mut msg_buf: Vec<u8> = Vec::with_capacity(512);
    let mut stop_chan_fused = stop_chan.next().fuse();
    loop {
        select! {
            stop = stop_chan_fused => {
                return;
            }
            read_ret = sock.read(&mut read_buf).fuse() => {
                match read_ret {
                    Ok(bytes_read) => {
                        msg_buf.extend(&read_buf[0..bytes_read]);
                        while let Some((msg, n_parsed)) = parse(&msg_buf) {
                            msg_buf.drain(0..n_parsed);
                            ev_chan.send(ConnEv::Msg(msg)).await.unwrap();
                        }
                    }
                    Err(io_err) => {
                        // TODO: send error == bug
                        ev_chan.send(ConnEv::Err(io_err)).await.unwrap();
                        return;
                    }
                }
            }
        }
    }
}

async fn write_task<A>(
    mut sock: WriteHalf<S>,
    mut msg_chan: mpsc::Receiver<String>,
    mut ev_chan: mpsc::Sender<ConnEv<A>>,
    mut stop_chan: watch::Receiver<()>,
) {
    let mut stop_chan_fused = stop_chan.next().fuse();
    loop {
        let mut msg_chan_fused = msg_chan.next().fuse();
        select! {
            stop = stop_chan_fused => {
                return;
            }
            msg = msg_chan_fused => {
                match msg {
                    None => {
                        println!("write_task: got None in msg channel");
                        return;
                    }
                    Some(msg) => {
                        // TODO: refactor
                        // TODO: handle send errors
                        if let Err(err) = sock.write_all(msg.as_bytes()).await {
                            ev_chan.send(ConnEv::Err(err)).await.unwrap();
                        }
                        if let Err(err) = sock.flush().await {
                            ev_chan.send(ConnEv::Err(err)).await.unwrap();
                        }
                    }
                }
            }
        }
    }
}
