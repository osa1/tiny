//! Implements two-state "pinger" task that drives sending pings to the server to check liveness of
//! the connection.

use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;

pub(crate) struct Pinger {
    snd_rst: mpsc::Sender<()>,
}

#[derive(Debug)]
pub(crate) enum Event {
    SendPing,
    Disconnect,
}

enum PingerState {
    /// Signal a "ping" on timeout. State moves to `ExpectPong`.
    SendPing,
    /// Signal a "disconnect" on timeout.
    ExpectPong,
}

async fn pinger_task(rcv_rst: mpsc::Receiver<()>, snd_ev: mpsc::Sender<Event>) {
    let mut rcv_rst_fused = ReceiverStream::new(rcv_rst).fuse();
    let mut state = PingerState::SendPing;
    loop {
        match timeout(Duration::from_secs(60), rcv_rst_fused.next()).await {
            Err(_) => match state {
                PingerState::SendPing => {
                    state = PingerState::ExpectPong;
                    snd_ev.try_send(Event::SendPing).unwrap();
                }
                PingerState::ExpectPong => {
                    snd_ev.try_send(Event::Disconnect).unwrap();
                    return;
                }
            },
            Ok(cmd) => match cmd {
                None => {
                    return;
                }
                Some(()) => {
                    state = PingerState::SendPing;
                }
            },
        }
    }
}

impl Pinger {
    pub(crate) fn new() -> (Pinger, mpsc::Receiver<Event>) {
        let (snd_ev, rcv_ev) = mpsc::channel(1);
        // No need for sending another "reset" when there's already one waiting to be processed
        let (snd_rst, rcv_rst) = mpsc::channel(1);
        tokio::task::spawn_local(pinger_task(rcv_rst, snd_ev));
        (Pinger { snd_rst }, rcv_ev)
    }

    pub(crate) fn reset(&mut self) {
        // Ignore errors: no need to send another "reset" when there's already one waiting to be
        // processed
        let _ = self.snd_rst.try_send(());
    }
}
