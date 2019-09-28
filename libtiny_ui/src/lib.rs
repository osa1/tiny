use time::Tm;

/// Target of a message to be shown in a UI.
pub enum MsgTarget<'a> {
    /// Show the message in the server tab.
    Server { serv_name: &'a str },

    /// Show the message in the channel tab.
    Chan {
        serv_name: &'a str,
        chan_name: &'a str,
    },

    /// Show the message in the privmsg tab.
    User { serv_name: &'a str, nick: &'a str },

    /// Show the message in all tabs of a server.
    AllServTabs { serv_name: &'a str },

    /// Show the message in currently active tab.
    CurrentTab,
}

/// Source of a message from the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MsgSource {
    /// Message sent in a server tab.
    Serv { serv_name: String },

    /// Message sent in a channel tab.
    Chan {
        serv_name: String,
        chan_name: String,
    },

    /// Message sent in a privmsg tab.
    User { serv_name: String, nick: String },
}

impl MsgSource {
    pub fn serv_name(&self) -> &str {
        match *self {
            MsgSource::Serv { ref serv_name }
            | MsgSource::Chan { ref serv_name, .. }
            | MsgSource::User { ref serv_name, .. } => serv_name,
        }
    }

    pub fn to_target(&self) -> MsgTarget {
        match *self {
            MsgSource::Serv { ref serv_name } => MsgTarget::Server { serv_name },
            MsgSource::Chan {
                ref serv_name,
                ref chan_name,
            } => MsgTarget::Chan {
                serv_name,
                chan_name,
            },
            MsgSource::User {
                ref serv_name,
                ref nick,
            } => MsgTarget::User { serv_name, nick },
        }
    }

    pub fn visible_name(&self) -> &str {
        match *self {
            MsgSource::Serv { ref serv_name, .. } => serv_name,
            MsgSource::Chan { ref chan_name, .. } => chan_name,
            MsgSource::User { ref nick, .. } => nick,
        }
    }
}

pub trait UI {
    /// Create a new server tab.
    fn new_server_tab(&self, serv: &str);

    /// Close a server tab and all channel and user tabs for that server.
    fn close_server_tab(&self, serv: &str);

    /// Create a channel tab in the given server.
    fn new_chan_tab(&self, serv: &str, chan: &str);

    /// Close a channel tab.
    fn close_chan_tab(&self, serv: &str, chan: &str);

    /// Close a user tab.
    fn close_user_tab(&self, serv: &str, nick: &str);

    /// Show a message coming from a client in the given tabs.
    fn add_client_msg(&self, msg: &str, target: &MsgTarget);

    /// Show a plain message with a timestamp in the given tabs.
    // TODO: What is a "plain message"? Not descriptive.
    fn add_msg(&self, msg: &str, ts: Tm, target: &MsgTarget);

    /// Show an IRC error message in the given tabs.
    fn add_err_msg(&self, msg: &str, ts: Tm, target: &MsgTarget);

    /// Show a client error message in the given tabs. Usuallys generated in case of a command
    /// error, e.g. "unknown command".
    fn add_client_err_msg(&self, msg: &str, target: &MsgTarget);

    /// Clear all nicks in a server from the UI's cache. Does not show anything.
    fn clear_nicks(&self, serv: &str);

    /// Set the client's nick in the given server.
    fn set_nick(&self, serv: &str, nick: &str);

    /// Show a user message sent to the client or to a channel.
    ///
    /// - highlight: Whether to highlight the message. Usually set `true` when the message mentions
    ///   the client's nick.
    ///
    /// - is_action: `true` when this is a CTCP ACTION message.
    ///
    fn add_privmsg(
        &self,
        sender: &str,
        msg: &str,
        ts: Tm,
        target: &MsgTarget,
        highlight: bool,
        is_action: bool,
    );

    /// Add a nick to the given tabs. When `ts` is not provided this does not show anything; just
    /// updated the channel nick list etc. Otherwise this shows a line like "foo joined channel".
    fn add_nick(&self, nick: &str, ts: Option<Tm>, target: &MsgTarget);

    /// Remove a nick from given tabs. Similar to `add_nick`, when `ts` is not provided this does
    /// not show a "foo left channel" line.
    fn remove_nick(&self, nick: &str, ts: Option<Tm>, target: &MsgTarget);

    /// Rename a nick in the given tabs.
    fn rename_nick(&self, old_nick: &str, new_nick: &str, ts: Tm, target: &MsgTarget);

    /// Set topic of given tabs.
    fn set_topic(&self, topic: &str, ts: Tm, serv_name: &str, chan_name: &str);

    /// Do we have a tab for the given user? This is useful for deciding where to show a PRIVMSG
    /// coming from server; e.g. messages from services sometimes shown in their own tabs,
    /// sometimes in the server tab.
    fn user_tab_exists(&self, serv: &str, nick: &str) -> bool;
}
