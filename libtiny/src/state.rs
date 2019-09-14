#![allow(clippy::zero_prefixed_literal)]

use crate::{
    wire,
    wire::{find_byte, Cmd, Msg, Pfx},
    Event, ServerInfo,
};

use tokio::sync::mpsc::Sender;

pub(crate) struct State<'a> {
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
    // away_status: Option<String>,

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

impl<'a> State<'a> {
    pub(crate) fn new(server_info: &'a ServerInfo, snd_irc_msg: &mut Sender<String>) -> State<'a> {
        // Introduce self
        snd_irc_msg
            .try_send(wire::nick(&server_info.nicks[0]))
            .unwrap();
        snd_irc_msg
            .try_send(wire::user(&server_info.hostname, &server_info.realname))
            .unwrap();

        let current_nick = server_info.nicks[0].to_owned();
        let chans = server_info.auto_join.clone();
        State {
            nicks: server_info.nicks.clone(),
            current_nick_idx: 0,
            current_nick,
            chans,
            // away_status: None,
            servername: None,
            usermask: None,
            nick_accepted: false,
            server_info,
        }
    }

    fn get_next_nick(&mut self) -> &str {
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

    pub(crate) fn update(
        &mut self,
        msg: &Msg,
        snd_ev: &mut Sender<Event>,
        snd_irc_msg: &mut Sender<String>,
    ) {
        let Msg { ref pfx, ref cmd } = msg;

        use Cmd::*;
        match cmd {
            PING { server } => {
                snd_irc_msg
                    .try_send(wire::pong(server))
                    .unwrap();
            }

            //
            // Setting usermask using JOIN, RPL_USERHOST and 396 (?)
            //
            JOIN { .. } => {
                if let Some(Pfx::User { nick, user }) = pfx {
                    if nick == &self.current_nick {
                        let usermask = format!("{}!{}", nick, user);
                        self.usermask = Some(usermask);
                    }
                }
            }

            Reply { num: 396, params } => {
                // :hobana.freenode.net 396 osa1 haskell/developer/osa1
                // :is now your hidden host (set by services.)
                if params.len() == 3 {
                    let usermask = format!(
                        "{}!~{}@{}",
                        self.current_nick, self.server_info.hostname, params[1]
                    );
                    self.usermask = Some(usermask);
                }
            }

            Reply { num: 302, params } => {
                // 302 RPL_USERHOST
                // :ircd.stealth.net 302 yournick :syrk=+syrk@millennium.stealth.net
                //
                // We know there will be only one nick because /userhost cmd sends
                // one parameter (our nick)
                //
                // Example args: ["osa1", "osa1=+omer@moz-s8a.9ac.93.91.IP "]

                let param = &params[1];
                match find_byte(param.as_bytes(), b'=') {
                    None => {
                        // TODO: Log this
                    }
                    Some(mut i) => {
                        if param.as_bytes().get(i + 1) == Some(&b'+')
                            || param.as_bytes().get(i + 1) == Some(&b'-')
                        {
                            i += 1;
                        }
                        let usermask = (&param[i..]).trim();
                        self.usermask = Some(usermask.to_owned());
                    }
                }
            }

            //
            // RPL_WELCOME
            //
            Reply { num: 001, .. } => {
                snd_ev.try_send(Event::Connected).unwrap();
                snd_ev
                    .try_send(Event::NickChange {
                        new_nick: self.current_nick.clone(),
                    })
                    .unwrap();
                // TODO: identify via nickserv
                self.nick_accepted = true;
            }

            //
            // RPL_YOURHOST
            //
            Reply { num: 002, params } => {
                // 002    RPL_YOURHOST
                //        "Your host is <servername>, running version <ver>"

                // An example <servername>: cherryh.freenode.net[149.56.134.238/8001]

                match parse_servername(params) {
                    None => {
                        // TODO: Log
                    }
                    Some(servername) => {
                        self.servername = Some(servername);
                    }
                }
            }

            //
            // ERR_NICKNAMEINUSE
            //
            Reply { num: 433, .. } => {
                // ERR_NICKNAMEINUSE. If we don't have a nick already try next nick.
                if !self.nick_accepted {
                    let new_nick = self.get_next_nick();
                    println!("new nick: {}", new_nick);
                    snd_ev
                        .try_send(Event::NickChange {
                            new_nick: new_nick.to_owned(),
                        })
                        .unwrap();
                    snd_irc_msg
                        .try_send(wire::nick(new_nick))
                        .unwrap();
                }
            }

            //
            // NICK message sent from the server when our nick change request was successful
            //
            NICK { nick: new_nick } => {
                if let Some(Pfx::User { nick: old_nick, .. }) = pfx {
                    if old_nick == &self.current_nick {
                        snd_ev
                            .try_send(Event::NickChange {
                                new_nick: new_nick.to_owned(),
                            })
                            .unwrap();
                        if !self.nicks.contains(new_nick) {
                            self.nicks.push(new_nick.to_owned());
                            self.current_nick_idx = self.nicks.len() - 1;
                        }
                    }
                }
            }

            //
            // RPL_ENDOFMOTD, join channels, set away status (TODO)
            //
            Reply { num: 376, .. } => {
                if !self.chans.is_empty() {
                    snd_irc_msg
                        .try_send(format!("JOIN {}\r\n", self.chans.join(",")))
                        .unwrap();
                }
            }

            //
            // RPL_TOPIC, we've successfully joined a channel
            //
            Reply { num: 332, params } => {
                if params.len() == 2 || params.len() == 3 {
                    let chan = &params[params.len() - 2];
                    if !self.chans.contains(chan) {
                        self.chans.push(chan.to_owned());
                    }
                }
            }

            _ => {}
        }
    }
}

/// Try to parse servername in a 002 RPL_YOURHOST reply
fn parse_servername(params: &[String]) -> Option<String> {
    let msg = params.get(1).or_else(|| params.get(0))?;
    let slice1 = &msg[13..];
    let servername_ends =
        find_byte(slice1.as_bytes(), b'[').or_else(|| find_byte(slice1.as_bytes(), b','))?;
    Some((&slice1[..servername_ends]).to_owned())
}

////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_servername_1() {
        let args = vec![
            "tiny_test".to_owned(),
            "Your host is adams.freenode.net[94.125.182.252/8001], \
             running version ircd-seven-1.1.4"
                .to_owned(),
        ];
        assert_eq!(
            parse_servername(&args),
            Some("adams.freenode.net".to_owned())
        );
    }

    #[test]
    fn test_parse_servername_2() {
        let args = vec![
            "tiny_test".to_owned(),
            "Your host is belew.mozilla.org, running version InspIRCd-2.0".to_owned(),
        ];
        assert_eq!(
            parse_servername(&args),
            Some("belew.mozilla.org".to_owned())
        );
    }
}
