#![recursion_limit = "512"]
#![allow(clippy::unneeded_field_pattern)]
#![allow(clippy::cognitive_complexity)]

mod pinger;
mod state;
mod stream;
mod utils;

use libtiny_common::{ChanName, ChanNameRef};
pub use libtiny_wire as wire;

use pinger::Pinger;
use state::State;
use stream::{Stream, StreamError};

use futures::future::FutureExt;
use futures::stream::{Fuse, StreamExt};
use futures::{pin_mut, select};
use std::net::{SocketAddr, ToSocketAddrs};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;

#[macro_use]
extern crate log;

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

    /// Use TLS?
    pub tls: bool,

    /// Server password.
    pub pass: Option<String>,

    pub realname: String,

    /// Nicks to select when logging in.
    pub nicks: Vec<String>,

    /// Channels to automatically join
    pub auto_join: Vec<ChanName>,

    /// Nickserv password. Sent to NickServ on connecting to the server and nick change, before
    /// join commands.
    pub nickserv_ident: Option<String>,

    /// SASL authentication credentials,
    pub sasl_auth: Option<SASLAuth>,
}

/// SASL authentication credentials
#[derive(Debug, Clone)]
pub struct SASLAuth {
    pub username: String,
    pub password: String,
}

/// IRC client events. Returned by `Client` to the users via a channel.
///
/// Note that Client only returns when it can't resolve the domain name. In all other cases (no
/// matter what the error is) it continues, in case of a connection error either by trying another
/// IP address of the same domain, or by waiting `RECONNECT_SECS` seconds and then trying again.
/// The latter happens after sending a `Disconnected` event.
#[derive(Debug)]
pub enum Event {
    /// Client resolving domain name
    ResolvingHost,
    /// Domain name resolved, client trying to connect to the given IP address
    Connecting(SocketAddr),
    /// TCP connection established *and* the introduction sequence with the IRC server started.
    Connected,
    /// Disconnected from the server. Usually sent right after an `Event::IoErr`. Client tries to
    /// reconnect after `RECONNECT_SECS` seconds after sending this event.
    Disconnected,
    /// An IO error happened.
    IoErr(std::io::Error),
    /// A TLS error happened
    TlsErr(stream::TlsError),
    /// Remote end closed the connection
    ConnectionClosed,
    /// Client couldn't resolve host address. The client stops after sending this event.
    CantResolveAddr,
    /// Nick changed.
    NickChange { new_nick: String },
    /// A message from the server
    Msg(wire::Msg),
    /// A wire-protocol error
    WireError(String),
    /// Channel join error message
    ChannelJoinError { chan: ChanName, msg: String },
}

impl From<StreamError> for Event {
    fn from(err: StreamError) -> Event {
        match err {
            StreamError::TlsError(tls_err) => Event::TlsErr(tls_err),
            StreamError::IoError(io_err) => Event::IoErr(io_err),
        }
    }
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
}

impl Client {
    /// Create a new client. Spawns two `tokio` tasks on the given `runtime`. If not given, tasks
    /// are created on the default executor using `tokio::spawn`.
    pub fn new(server_info: ServerInfo) -> (Client, mpsc::Receiver<Event>) {
        connect(server_info)
    }

    /// Reconnect to the server, possibly using a new port.
    pub fn reconnect(&mut self, port: Option<u16>) {
        debug!("reconnect cmd received, port: {:?}", port);
        self.msg_chan.try_send(Cmd::Reconnect(port)).unwrap()
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

    /// Send a message directly to the server. "\r\n" suffix is added by this method.
    pub fn raw_msg(&mut self, msg: &str) {
        self.msg_chan
            .try_send(Cmd::Msg(format!("{}\r\n", msg)))
            .unwrap();
    }

    /// Split a privmsg to multiple messages so that each message is, when the hostname and nick
    /// prefix added by the server, fits in one IRC message.
    ///
    /// `extra_len`: Size (in bytes) for a prefix/suffix etc. that'll be added to each line.
    pub fn split_privmsg<'a>(
        &self,
        extra_len: usize,
        msg: &'a str,
    ) -> impl Iterator<Item = &'a str> {
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
    pub fn privmsg(&mut self, target: &str, msg: &str, is_action: bool) {
        let wire_fn = if is_action {
            wire::action
        } else {
            wire::privmsg
        };
        self.msg_chan
            .try_send(Cmd::Msg(wire_fn(target, msg)))
            .unwrap();
    }

    /// Join the given list of channels.
    pub fn join(&mut self, chans: &[&ChanNameRef]) {
        self.msg_chan
            .try_send(Cmd::Msg(wire::join(&chans)))
            .unwrap()
    }

    /// Leave a channel.
    pub fn part(&mut self, chan: &ChanNameRef) {
        self.state.leave_channel(&mut self.msg_chan, chan)
    }

    /// Set away status. `None` means not away.
    pub fn away(&mut self, msg: Option<&str>) {
        self.state.set_away(msg);
        self.msg_chan.try_send(Cmd::Msg(wire::away(msg))).unwrap()
    }

    /// Change nick. This may fail (ERR_NICKNAMEINUSE) so wait for confirmation (a NICK message
    /// back from the server, with the old nick as prefix).
    pub fn nick(&mut self, new_nick: &str) {
        self.msg_chan
            .try_send(Cmd::Msg(wire::nick(new_nick)))
            .unwrap()
    }

    /// Send a QUIT message to the server, with optional "reason". This stops the client; so the
    /// sender end of the `Cmd` channel and the receiver end of the IRC message channel (for
    /// outgoing messages) will be dropped.
    pub fn quit(&mut self, reason: Option<String>) {
        debug!("quit cmd received");
        // Kill any retry join tasks to prevent quit delay
        self.state.kill_join_tasks();
        self.msg_chan.try_send(Cmd::Quit(reason)).unwrap();
    }

    /// Get all nicks in a channel.
    pub fn get_chan_nicks(&self, chan: &ChanNameRef) -> Vec<String> {
        self.state.get_chan_nicks(chan)
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
    /// Reconnect to the server, possibly using a new port.
    Reconnect(Option<u16>),
    /// Close the connection. This sends a QUIT message to the server (with optional "reason") and
    /// then all tasks return.
    Quit(Option<String>),
}

fn connect(server_info: ServerInfo) -> (Client, mpsc::Receiver<Event>) {
    let serv_name = server_info.addr.clone();

    //
    // Create communication channels
    //

    // Channel for returning IRC events to user.
    let (snd_ev, rcv_ev) = mpsc::channel::<Event>(100);

    // Channel for commands from user.
    let (snd_cmd, rcv_cmd) = mpsc::channel::<Cmd>(100);

    //
    // Create the main loop task
    //

    let irc_state = State::new(server_info.clone());
    let irc_state_clone = irc_state.clone();

    let task = main_loop(server_info, irc_state_clone, snd_ev, rcv_cmd);
    tokio::task::spawn_local(task);

    (
        Client {
            msg_chan: snd_cmd,
            serv_name,
            state: irc_state,
        },
        rcv_ev,
    )
}

async fn main_loop(
    server_info: ServerInfo,
    irc_state: State,
    mut snd_ev: mpsc::Sender<Event>,
    rcv_cmd: mpsc::Receiver<Cmd>,
) {
    let mut rcv_cmd = rcv_cmd.fuse();

    // We allow changing ports when reconnecting, so `mut`
    let mut port = server_info.port;

    // Whether to wait before trying to (re)connect
    let mut wait = false;

    // Main loop just tries to (re)connect
    'connect: loop {
        if wait {
            match wait_(&mut rcv_cmd).await {
                TaskResult::Done(()) => {}
                TaskResult::Reconnect(mb_port) => {
                    port = mb_port.unwrap_or(port);
                    wait = false;
                    continue;
                }
                TaskResult::Return => {
                    return;
                }
            }
        }

        // Channel for the sender task. Messages are complete IRC messages (including the
        // trailing "\r\n") and the task directly sends them to the server.
        let (mut snd_msg, mut rcv_msg) = mpsc::channel::<String>(100);

        //
        // Resolve IP address
        //

        snd_ev.send(Event::ResolvingHost).await.unwrap();

        let serv_name = server_info.addr.clone();

        debug!("Resolving address");

        let serv_name_clone = serv_name.clone();

        let addr_iter = match resolve_addr(serv_name_clone, port, &mut rcv_cmd).await {
            TaskResult::Done(Ok(addr_iter)) => {
                debug!("resolve_addr: done");
                addr_iter
            }
            TaskResult::Done(Err(err)) => {
                debug!("resolve_addr: {:?}", err);
                snd_ev.send(Event::IoErr(err)).await.unwrap();
                wait = true;
                continue;
            }
            TaskResult::Reconnect(mb_port) => {
                debug!("resolve_addr: try again");
                port = mb_port.unwrap_or(port);
                wait = false;
                continue;
            }
            TaskResult::Return => {
                debug!("resolve_addr: return");
                return;
            }
        };

        let addrs = addr_iter.collect::<Vec<_>>();

        if addrs.is_empty() {
            snd_ev.send(Event::CantResolveAddr).await.unwrap();
            return;
        }

        debug!("Address resolved: {:?}", addrs);

        //
        // Establish TCP connection to the server
        //

        let stream = match try_connect(
            addrs,
            &serv_name,
            server_info.tls,
            &mut rcv_cmd,
            &mut snd_ev,
        )
        .await
        {
            TaskResult::Done(Some(stream)) => stream,
            TaskResult::Done(None) => {
                snd_ev.send(Event::Disconnected).await.unwrap();
                wait = true;
                continue;
            }
            TaskResult::Return => {
                return;
            }
            TaskResult::Reconnect(mb_port) => {
                port = mb_port.unwrap_or(port);
                wait = false;
                continue;
            }
        };

        let (mut read_half, mut write_half) = tokio::io::split(stream);

        debug!("Done");

        //
        // Do the business
        //

        // Reset the connection state
        irc_state.reset();
        // Introduce self
        if server_info.sasl_auth.is_some() {
            // Will introduce self after getting a response to this LS command.
            // This is to avoid getting stuck during nick registration. See the
            // discussion in #91.
            snd_msg.try_send(wire::cap_ls()).unwrap();
        } else {
            irc_state.introduce(&mut snd_msg);
        }

        // Spawn a task for outgoing messages.
        let snd_ev_clone = snd_ev.clone();
        tokio::task::spawn_local(async move {
            while let Some(msg) = rcv_msg.next().await {
                if let Err(io_err) = write_half.write_all(msg.as_str().as_bytes()).await {
                    debug!("IO error when writing: {:?}", io_err);
                    snd_ev_clone.send(Event::IoErr(io_err)).await.unwrap();
                    return;
                }
            }
        });

        // Spawn pinger task
        let (mut pinger, rcv_ping_evs) = Pinger::new();
        let mut rcv_ping_evs = rcv_ping_evs.fuse();

        let mut parse_buf: Vec<u8> = Vec::with_capacity(1024);

        loop {
            let mut read_buf: [u8; 1024] = [0; 1024];

            select! {
                cmd = rcv_cmd.next() => {
                    match cmd {
                        None => {
                            debug!("main loop: command channel terminated from the other end");
                            // That's OK, rcv_cmd will never be ready again
                        }
                        Some(Cmd::Msg(irc_msg)) => {
                            snd_msg.try_send(irc_msg).unwrap();
                        }
                        Some(Cmd::Reconnect(mb_port)) => {
                            if let Some(new_port) = mb_port {
                                port = new_port;
                            }
                            wait = false;
                            continue 'connect;
                        }
                        Some(Cmd::Quit(reason)) => {
                            snd_msg.try_send(wire::quit(reason)).unwrap();
                            // This drops the sender end of the channel that the sender task
                            // uses, which in turn causes the sender task to return. Somewhat
                            // hacky?
                            return;
                        }
                    }
                }
                // It's fine to fuse() the read_half here because we restart main loop with a new
                // stream when this stream ends (either with an error, or when it's closed on the
                // remote end), so we never poll it again after it terminates.
                bytes = read_half.read(&mut read_buf).fuse() => {
                    match bytes {
                        Err(io_err) => {
                            debug!("main loop: error when reading from socket: {:?}", io_err);
                            snd_ev.send(Event::IoErr(io_err)).await.unwrap();
                            snd_ev.send(Event::Disconnected).await.unwrap();
                            wait = true;
                            continue 'connect;
                        }
                        Ok(0) => {
                            debug!("main loop: read 0 bytes");
                            snd_ev.send(Event::ConnectionClosed).await.unwrap();
                            snd_ev.send(Event::Disconnected).await.unwrap();
                            wait = true;
                            continue 'connect;
                        }
                        Ok(bytes) => {
                            parse_buf.extend_from_slice(&read_buf[0..bytes]);
                            while let Some(msg) = wire::parse_irc_msg(&mut parse_buf) {
                                match msg {
                                    Err(err) => {
                                        snd_ev.send(Event::WireError(err)).await.unwrap();
                                    }
                                    Ok(mut msg) => {
                                        debug!("parsed msg: {:?}", msg);
                                        pinger.reset();
                                        irc_state.update(&mut msg, &mut snd_ev, &mut snd_msg);
                                        snd_ev.send(Event::Msg(msg)).await.unwrap();
                                    }
                                }
                            }
                        }
                    }
                }
                ping_ev = rcv_ping_evs.next() => {
                    match ping_ev {
                        None => {
                            debug!("Ping thread terminated unexpectedly???");
                        }
                        Some(pinger::Event::SendPing) => {
                            irc_state.send_ping(&mut snd_msg);
                        }
                        Some(pinger::Event::Disconnect) => {
                            // TODO: indicate that this is a ping timeout
                            snd_ev.send(Event::Disconnected).await.unwrap();
                            // TODO: hopefully dropping the pinger rcv end is enough to stop it?
                            wait = true;
                            continue 'connect;
                        }
                    }
                }
            }
        }
    }
}

enum TaskResult<A> {
    Done(A),
    Return,
    Reconnect(Option<u16>),
}

async fn wait_(rcv_cmd: &mut Fuse<mpsc::Receiver<Cmd>>) -> TaskResult<()> {
    let delay = tokio::time::sleep(Duration::from_secs(RECONNECT_SECS)).fuse();
    pin_mut!(delay);

    loop {
        select! {
            () = delay => {
                return TaskResult::Done(());
            }
            cmd = rcv_cmd.next() => {
                // FIXME: This whole block is duplicated below, but it's hard to reuse because it
                // uses `continue` and `return`.
                match cmd {
                    None => {
                        // Channel closed, return from the main loop
                        return TaskResult::Return;
                    }
                    Some(Cmd::Msg(_)) => {
                        continue;
                    }
                    Some(Cmd::Reconnect(mb_port)) => {
                        return TaskResult::Reconnect(mb_port);
                    }
                    Some(Cmd::Quit(_)) => {
                        return TaskResult::Return;
                    }
                }
            }
        }
    }
}

async fn resolve_addr(
    serv_name: String,
    port: u16,
    rcv_cmd: &mut Fuse<mpsc::Receiver<Cmd>>,
) -> TaskResult<Result<::std::vec::IntoIter<SocketAddr>, ::std::io::Error>> {
    let mut addr_iter_task =
        tokio::task::spawn_blocking(move || (serv_name.as_str(), port).to_socket_addrs()).fuse();

    loop {
        select! {
            addr_iter = addr_iter_task => {
                match addr_iter {
                    Err(join_err) => {
                        // TODO (osa): Not sure about this
                        panic!("DNS thread failed: {:?}", join_err);
                    }
                    Ok(ret) => {
                        return TaskResult::Done(ret);
                    }
                }
            }
            cmd = rcv_cmd.next() => {
                match cmd {
                    None => {
                        // Channel closed, return from the main loop
                        return TaskResult::Return;
                    }
                    Some(Cmd::Msg(_)) => {
                        continue;
                    }
                    Some(Cmd::Reconnect(mb_port)) => {
                        return TaskResult::Reconnect(mb_port);
                    }
                    Some(Cmd::Quit(_)) => {
                        return TaskResult::Return;
                    }
                }
            }
        }
    }
}

async fn try_connect(
    addrs: Vec<SocketAddr>,
    serv_name: &str,
    use_tls: bool,
    rcv_cmd: &mut Fuse<mpsc::Receiver<Cmd>>,
    snd_ev: &mut mpsc::Sender<Event>,
) -> TaskResult<Option<Stream>> {
    let connect_task = async move {
        for addr in addrs {
            snd_ev.send(Event::Connecting(addr)).await.unwrap();
            let mb_stream = if use_tls {
                Stream::new_tls(addr, &serv_name).await
            } else {
                Stream::new_tcp(addr).await
            };
            match mb_stream {
                Err(err) => {
                    snd_ev.send(Event::from(err)).await.unwrap();
                }
                Ok(stream) => {
                    return Some(stream);
                }
            }
        }

        None
    };

    let connect_task = connect_task.fuse();
    pin_mut!(connect_task);

    loop {
        select! {
            stream = connect_task => {
                return TaskResult::Done(stream);
            }
            cmd = rcv_cmd.next() => {
                match cmd {
                    None => {
                        // Channel closed, return from the main loop
                        return TaskResult::Return;
                    }
                    Some(Cmd::Msg(_)) => {
                        continue;
                    }
                    Some(Cmd::Reconnect(mb_port)) => {
                        return TaskResult::Reconnect(mb_port);
                    }
                    Some(Cmd::Quit(_)) => {
                        return TaskResult::Return;
                    }
                }
            }
        }
    }
}
