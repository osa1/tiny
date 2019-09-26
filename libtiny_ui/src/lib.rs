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
