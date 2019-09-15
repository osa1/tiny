#![recursion_limit = "256"]
#![feature(drain_filter)]
#![feature(test)]

mod state;
pub mod utils;
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

/// `Client` tries to reconnect on error after this many seconds.
pub const RECONNECT_SECS: u64 = 30;

#[derive(Debug, Clone)]
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
    NickChange(String),
    /// A message from the server
    Msg(wire::Msg),
}

/// IRC client.
#[derive(Clone)]
pub struct Client {
    /// Channel to the send commands to the main loop. Usually just for sending messages to the
    /// server.
    msg_chan: mpsc::Sender<Cmd>,

    // TODO: This is mostly here to make switching from the old `conn.rs` easier; it may be
    // possible to remove this and maybe have a unique usize in each client as id.
    serv_name: String,

    /// Reference to the state, to be able to provide methods like `get_nick` and
    /// `is_nick_accepted`.
    state: State,
    // We can't have a channel to the sender task directly here, because when the sender task
    // returns we lose the receiving end of the channel and there's no way to avoid this except
    // with annoying hacks like wrapping it with an `Arc<Mutex<..>>` or something.
}

impl Client {
    /// Create a new client. Spawns two `tokio` tasks.
    pub fn new(server_info: ServerInfo) -> (Client, mpsc::Receiver<Event>) {
        connect(server_info)
    }

    /// Get host name of this connection.
    pub fn get_serv_name(&self) -> &str {
        &self.serv_name
    }

    /// Get current nick. Not that this returns the nick we're currently trying when the nick is
    /// not yet accepted. See `is_nick_accepted`.
    // FIXME: This allocates a String
    pub fn get_nick(&self) -> String {
        self.state.get_nick()
    }

    /// Is current nick accepted by the server?
    // TODO: Do we really need this?
    pub fn is_nick_accepted(&self) -> bool {
        self.state.is_nick_accepted()
    }

    /// Send a message directly to the server.
    pub fn raw_msg(&mut self, msg: String) {
        self.msg_chan.try_send(Cmd::Msg(msg)).unwrap()
    }

    /// Split a privmsg to multiple messages so that each message is, when the hostname and nick
    /// prefix added by the server, fits in one IRC message.
    ///
    /// `extra_len`: Size (in bytes) for a prefix/suffix etc. that'll be added to each line.
    pub fn split_privmsg<'a>(&self, extra_len: usize, msg: &'a str) -> utils::SplitIterator<'a> {
        // Max msg len calculation adapted from hexchat
        // (src/common/outbound.c:split_up_text)
        let mut max = 512; // RFC 2812
        max -= 3; // :, !, @
        max -= 13; // " PRIVMSG ", " ", :, \r, \n
        max -= self.get_nick().len();
        max -= extra_len;
        match self.state.get_usermask() {
            None => {
                max -= 9; // max username
                max -= 64; // max possible hostname (63) + '@'
                           // NOTE(osa): I think hexchat has an error here, it
                           // uses 65
            }
            Some(ref usermask) => {
                max -= usermask.len();
            }
        }

        assert!(max > 0);

        utils::split_iterator(msg, max)
    }

    /// Send a privmsg. Note that this method does not split long messages into smaller messages;
    /// use `split_privmsg` for that.
    pub fn privmsg(&mut self, target: &str, msg: &str, ctcp_action: bool) {
        let wire_fn = if ctcp_action {
            wire::privmsg
        } else {
            wire::action
        };
        self.msg_chan
            .try_send(Cmd::Msg(wire_fn(target, msg)))
            .unwrap();
    }

    /// Join a channel.
    pub fn join(&mut self, chan: &str) {
        let chans: [&str; 1] = [chan];
        self.msg_chan
            .try_send(Cmd::Msg(wire::join(&chans)))
            .unwrap()
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
    let serv_name = server_info.addr.clone();

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

    let irc_state = State::new(server_info.clone());
    let irc_state_clone = irc_state.clone();

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
                    tokio::timer::delay(tokio::clock::now() + Duration::from_secs(RECONNECT_SECS))
                        .await;
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

            // Reset the connection state
            irc_state.reset();
            // Introduce self
            snd_msg.try_send(wire::nick(&irc_state.get_nick())).unwrap();
            snd_msg
                .try_send(wire::user(&server_info.hostname, &server_info.realname))
                .unwrap();

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

            // TODO: Introduce self here

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

    (
        Client {
            msg_chan: snd_cmd,
            serv_name,
            state: irc_state_clone,
        },
        rcv_ev,
    )
}
