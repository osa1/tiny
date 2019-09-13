use crate::wire::{Cmd, Msg, Pfx};
use crate::IrcEv;
use crate::ServerInfo;

use tokio::sync::mpsc::Sender;

pub struct IrcState<'a> {
    /// Nicks to try, with this order.
    nicks: Vec<String>,

    /// An index to `nicks`. When out of range we add `current_nick_idx - nicks.length()`
    /// underscores to the last nick in `nicks`
    current_nick_idx: usize,

    /// A cache of current nick, to avoid allocating new nicks when inventing new nicks with
    /// underscores.
    current_nick: String,

    /// Currently joined channels. Every channel we join will be added here to be able to re-join
    /// automatically on reconnect and channels we leave will be removed.
    ///
    /// Technically a set but we want to join channels in the order given by the user, so using
    /// `Vec` here.
    chans: Vec<String>,

    /// Away reason if away mode is on. `None` otherwise. TODO: I don't think the message is used?
    away_status: Option<String>,

    /// servername to be used in PING messages. Read from 002 RPL_YOURHOST. `None` until 002.
    servername: Option<String>,

    /// Our usermask given by the server. Currently only parsed after a JOIN, reply 396.
    ///
    /// Note that RPL_USERHOST (302) does not take cloaks into account, so we don't parse USERHOST
    /// responses to set this field.
    usermask: Option<String>,

    /// Do we have a nick yet? Try another nick on ERR_NICKNAMEINUSE (433) until we've got a nick.
    nick_accepted: bool,

    /// Server information
    server_info: &'a ServerInfo,
}

impl<'a> IrcState<'a> {
    pub fn new(server_info: &'a ServerInfo) -> IrcState<'a> {
        let current_nick = server_info.nicks[0].to_owned();
        let chans = server_info.auto_join.clone();
        IrcState {
            nicks: server_info.nicks.clone(),
            current_nick_idx: 0,
            current_nick,
            chans,
            away_status: None,
            servername: None,
            usermask: None,
            nick_accepted: false,
            server_info,
        }
    }

    pub fn get_next_nick(&mut self) -> &str {
        self.current_nick_idx += 1;
        println!("current_nick_idx: {}", self.current_nick_idx);
        if self.current_nick_idx >= self.nicks.len() {
            let n_underscores = self.current_nick_idx - self.nicks.len() + 1;
            let mut new_nick = self.nicks.last().unwrap().to_string();
            for _ in 0..n_underscores {
                new_nick.push('_');
            }
            self.current_nick = new_nick;
        } else {
            self.current_nick = self.nicks[self.current_nick_idx].clone();
        }
        &self.current_nick
    }

    pub fn update(
        &mut self,
        msg: &Msg,
        snd_ev: &mut Sender<IrcEv>,
        snd_irc_msg: &mut Sender<String>,
    ) {
        let Msg { ref pfx, ref cmd } = msg;

        use Cmd::*;
        match cmd {
            JOIN { chan } => {
                if let Some(Pfx::User { nick, .. }) = pfx {
                    if nick == &self.current_nick {
                        if !self.chans.contains(chan) {
                            self.chans.push(chan.to_owned());
                        }
                    }
                }
            }
            PART { chan, .. } => {
                if let Some(Pfx::User { nick, .. }) = pfx {
                    if nick == &self.current_nick {
                        self.chans.drain_filter(|chan_| chan_ == chan);
                    }
                }
            }
            NICK { nick: ref new_nick } => {
                if let Some(Pfx::User {
                    nick: ref old_nick, ..
                }) = pfx
                {
                    // if old_nick == &self.current_nick {
                    //     snd_ev.try_send(IrcEv::NickChange(new_nick.to_owned())); // TODO panic on error
                    //     if !self.nicks.contains(new_nick) {
                    //         self.nicks.push(new_nick.to_owned());
                    //         self.current_nick_idx = self.nicks.len() - 1;
                    //     }
                    // }
                }
            }
            Reply { num: 433, .. } => {
                // ERR_NICKNAMEINUSE. If we don't have a nick already try next nick.
                if !self.nick_accepted {
                    let new_nick = self.get_next_nick();
                    println!("new nick: {}", new_nick);
                    snd_ev.try_send(IrcEv::NickChange(new_nick.to_owned()));
                    snd_irc_msg
                        .try_send(format!("NICK {}\r\n", new_nick))
                        .unwrap();
                }
            }
            Reply {
                num: 396,
                ref params,
            } => {
                if params.len() == 3 {
                    let usermask = format!(
                        "{}!~{}@{}",
                        self.current_nick, self.server_info.hostname, params[1]
                    );
                    self.usermask = Some(usermask);
                }
            }
            Reply { num: 376, .. } => {
                // End of MOTD, join channels
                if !self.chans.is_empty() {
                    snd_irc_msg.try_send(format!("JOIN {}\r\n", self.chans.join(","))).unwrap();
                }
            }
            _ => {}
        }
    }
}
