#![recursion_limit = "256"]
#![feature(drain_filter)]

mod state;
pub mod wire;

use state::State;

use futures::{future::FutureExt, select, stream::StreamExt};
use std::{net::ToSocketAddrs, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::mpsc,
};

//
// Public API
//

#[derive(Debug)]
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

/// IRC client events. Returned by `Client` to the users via a channel.
#[derive(Debug)]
pub enum Event {
    /// Client trying to connect
    Connecting,
    /// TCP connection established *and* the introduction sequence with the IRC server started.
    Connected,
    /// Disconnected from the server. Usually sent right after an `Event::IoErr`.
    Disconnected,
    /// An IO error happened.
    IoErr(std::io::Error),
    /// Client couldn't resolve host address.
    CantResolveAddr,
    /// Nick changed.
    NickChange { new_nick: String },
}

/// IRC client.
pub struct Client {
    /// Channel to the send commands to the main loop. Usually just for sending messages to the
    /// server.
    msg_chan: mpsc::Sender<Cmd>,

    // We can't have a channel to the sender task directly here, because when the sender task
    // returns we lose the receiving end of the channel and there's no way to avoid this except
    // with annoying hacks like wrapping it with an `Arc<Mutex<..>>` or something.
}

impl Client {
    /// Create a new client. Spawns two `tokio` tasks.
    pub fn new(server_info: ServerInfo) -> (Client, mpsc::Receiver<Event>) {
        connect(server_info)
    }

    /// Join a channel.
    pub fn join(&mut self, chan: &str) {
        let chans: [&str; 1] = [chan];
        self.msg_chan.try_send(Cmd::Msg(wire::join(&chans))).unwrap()
    }
}

//
// End of public API
//

#[derive(Debug)]
enum Cmd {
    /// Send this IRC message to the server. Note that this needs to be a valid IRC message
    /// (including the trailing "\r\n").
    Msg(String),
}

fn connect(server_info: ServerInfo) -> (Client, mpsc::Receiver<Event>) {
    //
    // Create communication channels
    //

    // Channel for returning IRC events to user.
    let (mut snd_ev, rcv_ev) = mpsc::channel::<Event>(100);
    // Channel for commands from user.
    let (snd_cmd, rcv_cmd) = mpsc::channel::<Cmd>(100);
    let mut rcv_cmd_fused = rcv_cmd.fuse();

    //
    // Create the main loop task
    //

    tokio::spawn(async move {
        // Main loop just tries to (re)connect
        'connect: loop {
            // Channel for the sender task. Messages are complete IRC messages (including the
            // trailing "\r\n") and the task directly sends them to the server.
            let (mut snd_msg, mut rcv_msg) = mpsc::channel::<String>(100);

            snd_ev.try_send(Event::Connecting).unwrap();

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
                    snd_ev.try_send(Event::IoErr(io_err)).unwrap();
                    // Wait 30 seconds before looping
                    tokio::timer::delay(tokio::clock::now() + Duration::from_secs(30)).await;
                    continue;
                }
                Ok(addr_iter) => addr_iter,
            };

            println!("Address resolved");

            let addr = match addr_iter.next() {
                None => {
                    snd_ev.try_send(Event::CantResolveAddr).unwrap();
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
                    snd_ev.try_send(Event::IoErr(io_err)).unwrap();
                    snd_ev.try_send(Event::Disconnected).unwrap();
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
            let mut snd_ev_clone = snd_ev.clone();
            tokio::spawn(async move {
                while let Some(msg) = rcv_msg.next().await {
                    if let Err(io_err) = write_half.write_all(msg.as_str().as_bytes()).await {
                        println!("IO error when writing: {:?}", io_err);
                        snd_ev_clone.try_send(Event::IoErr(io_err)).unwrap();
                        return;
                    }
                }
            });

            let mut parse_buf: Vec<u8> = Vec::with_capacity(1024);
            let mut irc_state = State::new(&server_info, &mut snd_msg);

            loop {
                let mut read_buf: [u8; 1024] = [0; 1024];

                select! {
                    cmd = rcv_cmd_fused.next() => {
                        match cmd {
                            None => {
                                println!("main loop: command channel terminated from the other end");
                                // That's OK, rcv_cmd_fused will never be ready again
                            }
                            Some(Cmd::Msg(irc_msg)) => {
                                snd_msg.try_send(irc_msg).unwrap();
                            }
                        }
                    }
                    bytes = read_half.read(&mut read_buf).fuse() => {
                        match bytes {
                            Err(io_err) => {
                                println!("main loop: error when reading from socket: {:?}", io_err);
                                snd_ev.try_send(Event::IoErr(io_err)).unwrap();
                                snd_ev.try_send(Event::Disconnected).unwrap();
                                continue 'connect;
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

    (Client { msg_chan: snd_cmd }, rcv_ev)
}
