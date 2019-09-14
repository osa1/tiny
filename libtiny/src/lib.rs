#![recursion_limit = "256"]
#![feature(drain_filter)]

pub mod wire;
pub mod irc_state;

use futures::stream::StreamExt;
use std::net::ToSocketAddrs;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;

use futures::future::FutureExt;
use futures::select;

pub struct ServerInfo {
    /// Server address
    pub addr: String,

    /// Server port
    pub port: u16,

    /// Server password.
    pub pass: Option<String>,

    pub hostname: String,

    pub realname: String,

    /// Nicks to select when logging in.
    pub nicks: Vec<String>,

    /// Channels to automatically join
    pub auto_join: Vec<String>,

    /// Nickserv password. Sent to NickServ on connecting to the server and nick change, before
    /// join commands.
    pub nickserv_ident: Option<String>,
}

#[derive(Debug)]
pub enum IrcEv {
    Connecting,
    Connected,
    Disconnected,
    IoErr(std::io::Error),
    CantResolveAddr,
    NickChange(String),
}

#[derive(Debug)]
enum IrcCmd {
    Msg(String),
}

pub struct IrcClient {
    msg_chan: mpsc::Sender<IrcCmd>,
}

impl IrcClient {
    pub fn join(&mut self, chan: &str) {
        self.msg_chan
            .try_send(IrcCmd::Msg(format!("JOIN {}\r\n", chan)))
            .unwrap();
    }
}

#[derive(Debug)]
pub enum ConnectError {
    CantResolveAddr,
    IoError(std::io::Error),
}

impl From<std::io::Error> for ConnectError {
    fn from(err: std::io::Error) -> ConnectError {
        ConnectError::IoError(err)
    }
}

pub async fn connect(
    server_info: ServerInfo,
) -> Result<(IrcClient, mpsc::Receiver<IrcEv>), ConnectError> {
    //
    // Create communication channels
    //

    // Channel for returning IRC events to user.
    let (mut snd_ev, rcv_ev) = mpsc::channel::<IrcEv>(100);
    // Channel for commands from user.
    let (snd_cmd, rcv_cmd) = mpsc::channel::<IrcCmd>(100);
    // Channel for the sender task. TODO: Find a way to remove this.
    let (mut snd_msg, mut rcv_msg) = mpsc::channel::<String>(100);

    //
    // Create the main loop task
    //

    tokio::spawn(async move {
        // Main loop just tries to (re)connect
        /* 'connect: */ loop {
            snd_ev.send(IrcEv::Connecting).await.unwrap();

            //
            // Resolve IP address
            //

            let serv_name = server_info.addr.clone();
            let port = server_info.port;

            println!("Resolving address");

            let mut addr_iter = match tokio_executor::blocking::run(move || {
                (serv_name.as_str(), port).to_socket_addrs()
            })
            .await
            {
                Err(io_err) => {
                    snd_ev.send(IrcEv::IoErr(io_err)).await.unwrap();
                    // Wait 30 seconds before looping
                    tokio::timer::delay(tokio::clock::now() + Duration::from_secs(30)).await;
                    continue;
                }
                Ok(addr_iter) => addr_iter,
            };

            println!("Address resolved");

            let addr = match addr_iter.next() {
                None => {
                    snd_ev.send(IrcEv::CantResolveAddr).await.unwrap();
                    break;
                }
                Some(addr) => addr,
            };

            //
            // Establish TCP connection to the server
            //

            println!("Establishing connection ...");

            let stream = match TcpStream::connect(&addr).await {
                Err(io_err) => {
                    snd_ev.send(IrcEv::IoErr(io_err)).await.unwrap();
                    // Wait 30 seconds before looping
                    tokio::timer::delay(tokio::clock::now() + Duration::from_secs(30)).await;
                    continue;
                }
                Ok(stream) => stream,
            };

            println!("Done");

            let (mut read_half, mut write_half) = stream.split();

            //
            // Do the business
            //

            // Spawn a task for outgoing messages.
            tokio::spawn(async move {
                // XXX JUST TESING
                write_half
                    .write_all("NICK osa1\r\n".as_bytes())
                    .await
                    .unwrap();
                write_half
                    .write_all("USER omer 8 * :omer\r\n".as_bytes())
                    .await
                    .unwrap();

                while let Some(msg) = rcv_msg.next().await {
                    if let Err(io_err) = write_half.write_all(msg.as_str().as_bytes()).await {
                        println!("IO error when writing: {:?}", io_err);
                        return;
                    }
                }
            });

            let mut parse_buf: Vec<u8> = Vec::with_capacity(1024);
            let mut irc_state = irc_state::IrcState::new(&server_info);

            let mut rcv_cmd_fused = rcv_cmd.fuse();

            loop {
                let mut read_buf: [u8; 1024] = [0; 1024];

                select! {
                    cmd = rcv_cmd_fused.next() => {
                        match cmd {
                            None => {
                                println!("main loop: command channel terminated from the other end");
                                // That's OK, rcv_cmd_fused will never be ready again
                            }
                            Some(IrcCmd::Msg(msg)) => {
                                // FIXME something like this
                                println!(">>> {}", &msg[..msg.len()-2]);
                            }
                        }
                    }
                    bytes = read_half.read(&mut read_buf).fuse() => {
                        match bytes {
                            Err(io_err) => {
                                println!("main loop: error when reading from socket: {:?}", io_err);
                                return;
                            }
                            Ok(bytes) => {
                                parse_buf.extend_from_slice(&read_buf[0..bytes]);
                                while let Some(msg) = wire::parse_irc_msg(&mut parse_buf) {
                                    println!("parsed msg: {:?}", msg);
                                    irc_state.update(&msg, &mut snd_ev, &mut snd_msg);
                                }
                            }
                        }
                    }
                }
            }
        }
    });

    Ok((IrcClient { msg_chan: snd_cmd }, rcv_ev))
}
