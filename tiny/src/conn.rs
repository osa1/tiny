#![allow(clippy::cognitive_complexity)]
#![allow(clippy::unneeded_field_pattern)]

//! IRC event handling

use futures::stream::StreamExt;
use tokio::sync::mpsc;

use libtiny_client::Client;
use libtiny_common::ChanNameRef;
use libtiny_ui::{MsgTarget, TabStyle, UI};
use libtiny_wire as wire;

pub(crate) async fn task(
    mut rcv_ev: mpsc::Receiver<libtiny_client::Event>,
    ui: Box<dyn UI>,
    client: Client,
) {
    while let Some(ev) = rcv_ev.next().await {
        handle_conn_ev(&*ui, &client, ev);
        ui.draw();
    }
}

fn handle_conn_ev(ui: &dyn UI, client: &Client, ev: libtiny_client::Event) {
    use libtiny_client::Event::*;
    match ev {
        ResolvingHost => {
            ui.add_client_msg(
                "Resolving host...",
                &MsgTarget::AllServTabs {
                    serv: client.get_serv_name(),
                },
            );
        }
        Connecting(sock_addr) => {
            ui.add_client_msg(
                &format!("Connecting to {}", sock_addr),
                &MsgTarget::AllServTabs {
                    serv: client.get_serv_name(),
                },
            );
        }
        Connected => {
            ui.add_msg(
                "Connected.",
                time::now(),
                &MsgTarget::AllServTabs {
                    serv: client.get_serv_name(),
                },
            );
        }
        Disconnected => {
            let serv = client.get_serv_name();
            ui.add_err_msg(
                &format!(
                    "Disconnected. Will try to reconnect in {} seconds.",
                    libtiny_client::RECONNECT_SECS
                ),
                time::now(),
                &MsgTarget::AllServTabs { serv },
            );
            ui.clear_nicks(serv);
        }
        IoErr(err) => {
            ui.add_err_msg(
                &format!("Connection error: {}", err),
                time::now(),
                &MsgTarget::AllServTabs {
                    serv: client.get_serv_name(),
                },
            );
        }
        ConnectionClosed => {
            ui.add_err_msg(
                "Connection closed on the remote end",
                time::now(),
                &MsgTarget::AllServTabs {
                    serv: client.get_serv_name(),
                },
            );
        }
        TlsErr(err) => {
            ui.add_err_msg(
                &format!("TLS error: {}", err),
                time::now(),
                &MsgTarget::AllServTabs {
                    serv: client.get_serv_name(),
                },
            );
        }
        CantResolveAddr => {
            ui.add_err_msg(
                "Can't resolve address",
                time::now(),
                &MsgTarget::AllServTabs {
                    serv: client.get_serv_name(),
                },
            );
        }
        NickChange(new_nick) => {
            ui.set_nick(client.get_serv_name(), &new_nick);
        }
        Msg(msg) => {
            handle_irc_msg(ui, client, msg);
        }
        WireError(err) => {
            ui.add_err_msg(
                &format!("Wire protocol error: {}", err),
                time::now(),
                &MsgTarget::Server {
                    serv: client.get_serv_name(),
                },
            );
        }
        ChannelJoinError { chan, msg } => ui.add_client_msg(
            &msg,
            &MsgTarget::Chan {
                serv: client.get_serv_name(),
                chan: &chan,
            },
        ),
    }
}

fn handle_irc_msg(ui: &dyn UI, client: &Client, msg: wire::Msg) {
    use wire::Cmd::*;
    use wire::Pfx::*;

    let wire::Msg { pfx, cmd } = msg;
    let ts = time::now();
    let serv = client.get_serv_name();
    match cmd {
        PRIVMSG {
            target,
            msg,
            is_notice,
            ctcp,
        } => {
            let pfx = match pfx {
                Some(pfx) => pfx,
                None => {
                    debug!("PRIVMSG without prefix: {:?}", msg);
                    return;
                }
            };

            // Sender to be shown in the UI
            let sender = match pfx {
                Server(ref serv) => serv,
                User { ref nick, .. } | Ambiguous(ref nick) => nick,
            };

            if ctcp == Some(wire::CTCP::Version) {
                let msg_target = if ui.user_tab_exists(serv, sender) {
                    MsgTarget::User { serv, nick: sender }
                } else {
                    MsgTarget::Server { serv }
                };
                ui.add_client_msg(
                    &format!("Received version request from {}", sender),
                    &msg_target,
                );
                return;
            }

            let is_action = ctcp == Some(wire::CTCP::Action);

            match target {
                wire::MsgTarget::Chan(chan) => {
                    let ui_msg_target = MsgTarget::Chan { serv, chan: &chan };
                    // highlight the message if it mentions us
                    if msg.find(&client.get_nick()).is_some() {
                        ui.add_privmsg(sender, &msg, ts, &ui_msg_target, true, is_action);
                        ui.set_tab_style(TabStyle::Highlight, &ui_msg_target);
                        let mentions_target = MsgTarget::Server { serv: "mentions" };
                        ui.add_msg(
                            &format!("{} in {}:{}: {}", sender, serv, chan.display(), msg),
                            ts,
                            &mentions_target,
                        );
                        ui.set_tab_style(TabStyle::Highlight, &mentions_target);
                    } else {
                        ui.add_privmsg(sender, &msg, ts, &ui_msg_target, false, is_action);
                        ui.set_tab_style(TabStyle::NewMsg, &ui_msg_target);
                    }
                }
                wire::MsgTarget::User(target) => {
                    if target == client.get_nick() {
                        // Message is sent to us. If the sender is a server we show the message in
                        // the server tab, otherwise we show it in a private tab. In case of an
                        // ambiguity we again show in a private tab, as some bouncers send PRIVMSGs
                        // from users with ambiguous prefix without a `user@host` part. See #247.
                        let msg_target = {
                            match pfx {
                                Server(_) => MsgTarget::Server { serv },
                                User { ref nick, .. } | Ambiguous(ref nick) => {
                                    // Show NOTICE messages in server tabs if we don't have a tab for
                                    // the sender already (see #21)
                                    if is_notice && !ui.user_tab_exists(serv, nick) {
                                        MsgTarget::Server { serv }
                                    } else {
                                        MsgTarget::User { serv, nick }
                                    }
                                }
                            }
                        };
                        ui.add_privmsg(sender, &msg, ts, &msg_target, false, is_action);
                        ui.set_tab_style(TabStyle::Highlight, &msg_target);
                    } else {
                        // PRIVMSG not sent to us. This case can happen when using a bouncer, see
                        // #271. When multiple clients connect to the same bouncer and one of them
                        // sends a PRIVMSG, the message is relayed to the other clients. E.g.
                        //
                        //     PRIVMSG <our_nick> <target> :...
                        //
                        // When this happens the prefix should be our nick. We check this below:
                        if log_enabled!(log::Level::Warn) {
                            match pfx {
                                Server(_) => {
                                    warn!(
                                        "PRIVMSG target is not us, but prefix is server: {:?}",
                                        msg
                                    );
                                }
                                User { ref nick, .. } | Ambiguous(ref nick) => {
                                    if *nick != client.get_nick() {
                                        warn!("PRIVMSG sender or target is not us: {:?}", msg);
                                    }
                                }
                            }
                        }
                        let msg_target = MsgTarget::User {
                            serv,
                            nick: &target,
                        };
                        ui.add_privmsg(&client.get_nick(), &msg, ts, &msg_target, false, is_action);
                        // Don't highlight the tab as `Highlight`: the message was sent by us so
                        // the tab probably doesn't need that much attention. Highlight as `NewMsg`
                        // instead.
                        ui.set_tab_style(TabStyle::NewMsg, &msg_target);
                    }
                }
            }
        }

        JOIN { chan } => {
            let nick = match pfx {
                Some(User { nick, .. }) | Some(Ambiguous(nick)) => nick,
                Some(Server(_)) | None => {
                    debug!(
                        "JOIN with weird prefix: pfx={:?}, cmd={:?}",
                        pfx,
                        JOIN { chan }
                    );
                    return;
                }
            };

            if nick == client.get_nick() {
                ui.new_chan_tab(serv, &chan);
            } else {
                let nick = wire::drop_nick_prefix(&nick);
                let ts = Some(time::now());
                ui.add_nick(nick, ts, &MsgTarget::Chan { serv, chan: &chan });
                // Also update the private message tab if it exists
                // Nothing will be shown if the user already known to be online by the tab
                if ui.user_tab_exists(serv, nick) {
                    ui.add_nick(nick, ts, &MsgTarget::User { serv, nick });
                }
                ui.set_tab_style(TabStyle::JoinOrPart, &MsgTarget::Chan { serv, chan: &chan })
            }
        }

        PART { chan, msg } => {
            let nick = match pfx {
                Some(User { nick, .. }) | Some(Ambiguous(nick)) => nick,
                Some(Server(_)) | None => {
                    debug!(
                        "PART with weird prefix: pfx={:?}, cmd={:?}",
                        pfx,
                        PART { chan, msg }
                    );
                    return;
                }
            };
            if nick != client.get_nick() {
                ui.remove_nick(
                    &nick,
                    Some(time::now()),
                    &MsgTarget::Chan { serv, chan: &chan },
                );
                ui.set_tab_style(TabStyle::JoinOrPart, &MsgTarget::Chan { serv, chan: &chan })
            }
        }

        QUIT { chans, msg } => {
            let nick = match pfx {
                Some(User { ref nick, .. }) | Some(Ambiguous(ref nick)) => nick,
                Some(Server(_)) | None => {
                    debug!(
                        "QUIT with weird prefix: pfx={:?}, cmd={:?}",
                        pfx,
                        QUIT { chans, msg }
                    );
                    return;
                }
            };

            for chan in &chans {
                ui.remove_nick(nick, Some(time::now()), &MsgTarget::Chan { serv, chan });
            }
            if ui.user_tab_exists(serv, nick) {
                ui.remove_nick(nick, Some(time::now()), &MsgTarget::User { serv, nick });
            }
        }

        NICK { nick, chans } => {
            let old_nick = match pfx {
                Some(User { nick, .. }) | Some(Ambiguous(nick)) => nick,
                Some(Server(_)) | None => {
                    debug!(
                        "NICK with weird prefix: pfx={:?}, cmd={:?}",
                        pfx,
                        NICK { nick, chans }
                    );
                    return;
                }
            };

            for chan in &chans {
                ui.rename_nick(
                    &old_nick,
                    &nick,
                    time::now(),
                    &MsgTarget::Chan { serv, chan },
                );
            }
            if ui.user_tab_exists(serv, &old_nick) {
                ui.rename_nick(
                    &old_nick,
                    &nick,
                    time::now(),
                    &MsgTarget::User {
                        serv,
                        nick: &old_nick,
                    },
                );
            }
        }

        Reply { num: 433, .. } => {
            // ERR_NICKNAMEINUSE
            if client.is_nick_accepted() {
                // Nick change request from user failed. Just show an error message.
                ui.add_err_msg(
                    "Nickname is already in use",
                    time::now(),
                    &MsgTarget::AllServTabs { serv },
                );
            }
        }

        PING { .. } | PONG { .. } => {
            // Ignore
        }

        ERROR { msg } => {
            ui.add_err_msg(&msg, time::now(), &MsgTarget::AllServTabs { serv });
        }

        TOPIC { chan, topic } => {
            ui.set_topic(&topic, time::now(), serv, &chan);
        }

        CAP {
            client: _,
            subcommand,
            params,
        } => match subcommand.as_ref() {
            "NAK" => {
                if params.iter().any(|cap| cap.as_str() == "sasl") {
                    let msg_target = MsgTarget::Server { serv };
                    ui.add_err_msg(
                        "Server rejected using SASL authenication capability",
                        time::now(),
                        &msg_target,
                    );
                }
            }
            "LS" => {
                if !params.iter().any(|cap| cap.as_str() == "sasl") {
                    let msg_target = MsgTarget::Server { serv };
                    ui.add_err_msg(
                        "Server does not support SASL authenication",
                        time::now(),
                        &msg_target,
                    );
                }
            }
            "ACK" => {}
            cmd => {
                debug!("Ignoring CAP subcommand {}: params={:?}", cmd, params);
            }
        },

        AUTHENTICATE { .. } => {
            // Ignore
        }

        Reply { num: n, params } => {
            let n_params = params.len();
            if (
                n <= 003 // RPL_WELCOME, RPL_YOURHOST, RPL_CREATED
                    || n == 251 // RPL_LUSERCLIENT
                    || n == 255 // RPL_LUSERME
                    || n == 372 // RPL_MOTD
                    || n == 375 // RPL_MOTDSTART
                    || n == 376
                // RPL_ENDOFMOTD
            ) && n_params == 2
            {
                let msg = &params[1];
                ui.add_msg(msg, time::now(), &MsgTarget::Server { serv });
            } else if n == 4 // RPL_MYINFO
                    || n == 5 // RPL_BOUNCE
                    || (n >= 252 && n <= 254)
            // RPL_LUSEROP, RPL_LUSERUNKNOWN, RPL_LUSERCHANNELS
            {
                let msg = params.into_iter().collect::<Vec<String>>().join(" ");
                ui.add_msg(&msg, time::now(), &MsgTarget::Server { serv });
            } else if (n == 265 || n == 266 || n == 250) && n_params > 0 {
                let msg = &params[n_params - 1];
                ui.add_msg(msg, time::now(), &MsgTarget::Server { serv });
            }
            // RPL_TOPIC
            else if n == 332 && (n_params == 3 || n_params == 2) {
                // RFC 2812 says this will have 2 arguments, but freenode sends 3 arguments (extra
                // one being our nick).
                let chan = &params[n_params - 2];
                let topic = &params[n_params - 1];
                ui.set_topic(topic, time::now(), serv, ChanNameRef::new(chan));
            }
            // RPL_NAMREPLY: List of users in a channel
            else if n == 353 && n_params > 3 {
                let chan = &params[2];
                let chan_target = MsgTarget::Chan {
                    serv,
                    chan: ChanNameRef::new(chan),
                };

                for nick in params[3].split_whitespace() {
                    ui.add_nick(wire::drop_nick_prefix(nick), None, &chan_target);
                }
            }
            // RPL_ENDOFNAMES: End of NAMES list
            else if n == 366 {
            }
            // RPL_UNAWAY or RPL_NOWAWAY
            else if (n == 305 || n == 306) && n_params > 1 {
                let msg = &params[1];
                ui.add_client_msg(msg, &MsgTarget::AllServTabs { serv });
            }
            // ERR_NOSUCHNICK
            else if n == 401 && n_params > 2 {
                let nick = &params[1];
                let msg = &params[2];
                ui.add_client_msg(msg, &MsgTarget::User { serv, nick });
            // RPL_AWAY
            } else if n == 301 && n_params > 2 {
                let nick = &params[1];
                let msg = &params[2];
                ui.add_client_msg(
                    &format!("{} is away: {}", nick, msg),
                    &MsgTarget::User { serv, nick },
                );
            } else {
                match pfx {
                    Some(Server(msg_serv)) | Some(Ambiguous(msg_serv)) => {
                        let msg_target = MsgTarget::Server { serv };
                        ui.add_privmsg(
                            &msg_serv,
                            &params.join(" "),
                            time::now(),
                            &msg_target,
                            false,
                            false,
                        );
                        ui.set_tab_style(TabStyle::NewMsg, &msg_target);
                    }
                    Some(User { .. }) | None => {
                        debug!(
                            "Ignoring numeric reply {}: pfx={:?}, params={:?}",
                            n, pfx, params
                        );
                    }
                }
            }
        }

        Other { cmd, params } => match pfx {
            Some(Server(msg_serv)) => {
                let msg_target = MsgTarget::Server { serv };
                ui.add_privmsg(
                    &msg_serv,
                    &params.join(" "),
                    time::now(),
                    &msg_target,
                    false,
                    false,
                );
                ui.set_tab_style(TabStyle::NewMsg, &msg_target);
            }
            Some(User { .. }) | Some(Ambiguous(_)) | None => {
                debug!(
                    "Ignoring command {}: pfx={:?}, params={:?}",
                    cmd, pfx, params
                );
            }
        },
    }
}
