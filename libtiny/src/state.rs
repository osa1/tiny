#![allow(clippy::zero_prefixed_literal)]

use crate::{Event, ServerInfo};
use libtiny_wire as wire;
use libtiny_wire::{find_byte, Msg, Pfx};

use std::cell::RefCell;
use std::rc::Rc;
use tokio::sync::mpsc::Sender;

#[derive(Clone)]
pub struct State {
    inner: Rc<RefCell<StateInner>>,
}

impl State {
    pub(crate) fn new(server_info: ServerInfo) -> State {
        State {
            inner: Rc::new(RefCell::new(StateInner::new(server_info))),
        }
    }

    pub(crate) fn reset(&self) {
        self.inner.borrow_mut().reset()
    }

    pub(crate) fn send_ping(&self, snd_irc_msg: &mut Sender<String>) {
        self.inner.borrow_mut().send_ping(snd_irc_msg)
    }

    pub(crate) fn update(
        &self,
        msg: &Msg,
        snd_ev: &mut Sender<Event>,
        snd_irc_msg: &mut Sender<String>,
    ) {
        self.inner.borrow_mut().update(msg, snd_ev, snd_irc_msg)
    }

    pub(crate) fn introduce(&self, snd_irc_msg: &mut Sender<String>) {
        self.inner.borrow_mut().introduce(snd_irc_msg)
    }

    // FIXME: This allocates a new String
    pub(crate) fn get_nick(&self) -> String {
        self.inner.borrow_mut().current_nick.clone()
    }

    // FIXME: Maybe use RwLock instead of Mutex
    pub(crate) fn is_nick_accepted(&self) -> bool {
        self.inner.borrow_mut().nick_accepted
    }

    pub(crate) fn get_usermask(&self) -> Option<String> {
        self.inner.borrow_mut().usermask.clone()
    }

    pub(crate) fn set_away(&self, msg: Option<&str>) {
        self.inner.borrow_mut().away_status = msg.map(str::to_owned);
    }
}

struct StateInner {
    /// Nicks to try, with this order.
    nicks: Vec<String>,

    /// NickServ passowrd.
    nickserv_ident: Option<String>,

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
    server_info: ServerInfo,
}

impl StateInner {
    fn new(server_info: ServerInfo) -> StateInner {
        let current_nick = server_info.nicks[0].to_owned();
        let chans = server_info.auto_join.clone();
        StateInner {
            nicks: server_info.nicks.clone(),
            nickserv_ident: server_info.nickserv_ident.clone(),
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

    fn reset(&mut self) {
        self.nicks = self.server_info.nicks.clone();
        self.current_nick_idx = 0;
        self.current_nick = self.nicks[0].clone();
        self.chans = self.server_info.auto_join.clone();
        self.servername = None;
        self.usermask = None;
    }

    fn send_ping(&mut self, snd_irc_msg: &mut Sender<String>) {
        if let Some(ref servername) = self.servername {
            snd_irc_msg.try_send(wire::ping(servername)).unwrap();
        }
    }

    fn introduce(&mut self, snd_irc_msg: &mut Sender<String>) {
        if let Some(ref pass) = self.server_info.pass {
            snd_irc_msg.try_send(wire::pass(pass)).unwrap();
        }
        snd_irc_msg
            .try_send(wire::nick(&self.current_nick))
            .unwrap();
        snd_irc_msg
            .try_send(wire::user(
                &self.server_info.hostname,
                &self.server_info.realname,
            ))
            .unwrap();
    }

    fn get_next_nick(&mut self) -> &str {
        self.current_nick_idx += 1;
        // println!("current_nick_idx: {}", self.current_nick_idx);
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

    fn update(&mut self, msg: &Msg, snd_ev: &mut Sender<Event>, snd_irc_msg: &mut Sender<String>) {
        let Msg { ref pfx, ref cmd } = msg;

        use wire::Cmd::*;
        match cmd {
            PING { server } => {
                snd_irc_msg.try_send(wire::pong(server)).unwrap();
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
                    .try_send(Event::NickChange(self.current_nick.clone()))
                    .unwrap();
                self.nick_accepted = true;
                if let Some(ref pwd) = self.nickserv_ident {
                    snd_irc_msg
                        .try_send(wire::privmsg("NickServ", &format!("identify {}", pwd)))
                        .unwrap();
                }
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
                    // println!("new nick: {}", new_nick);
                    snd_ev
                        .try_send(Event::NickChange(new_nick.to_owned()))
                        .unwrap();
                    snd_irc_msg.try_send(wire::nick(new_nick)).unwrap();
                }
            }

            //
            // NICK message sent from the server when our nick change request was successful
            //
            NICK { nick: new_nick } => {
                if let Some(Pfx::User { nick: old_nick, .. }) = pfx {
                    if old_nick == &self.current_nick {
                        snd_ev
                            .try_send(Event::NickChange(new_nick.to_owned()))
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

            //
            // SASL authentication
            //
            CAP {
                client: _,
                subcommand,
                params,
            } => {
                match subcommand.as_ref() {
                    "ACK" => {
                        if params.iter().any(|cap| cap.as_str() == "sasl") {
                            snd_irc_msg.try_send(wire::authenticate("PLAIN")).unwrap();
                        }
                    }
                    "NAK" => {
                        snd_irc_msg.try_send(wire::cap_end()).unwrap();
                    }
                    "LS" => {
                        self.introduce(snd_irc_msg);
                        if params.iter().any(|cap| cap == "sasl") {
                            snd_irc_msg.try_send(wire::cap_req(&["sasl"])).unwrap();
                            // Will wait for CAP ... ACK from server before authentication.
                        }
                    }
                    _ => {}
                }
            }

            AUTHENTICATE { ref param } => {
                if param.as_str() == "+" {
                    // Empty AUTHENTICATE response; server accepted the specified SASL mechanism
                    // (PLAIN)
                    if let Some(ref auth) = self.server_info.sasl_auth {
                        let msg = format!(
                            "{}\x00{}\x00{}",
                            auth.username, auth.username, auth.password
                        );
                        snd_irc_msg
                            .try_send(wire::authenticate(&base64::encode(&msg)))
                            .unwrap();
                    }
                }
            }

            Reply { num: 903, .. } | Reply { num: 904, .. } => {
                // 903: RPL_SASLSUCCESS, 904: ERR_SASLFAIL
                snd_irc_msg.try_send(wire::cap_end()).unwrap();
            }

            //
            // Ignore the rest
            //
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
