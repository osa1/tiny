pub use objekt::clone_box;
use time::Tm;

/// Target of a message to be shown in a UI.
#[derive(Debug)]
pub enum MsgTarget<'a> {
    /// Show the message in the server tab.
    Server { serv: &'a str },

    /// Show the message in the channel tab.
    Chan { serv: &'a str, chan: &'a str },

    /// Show the message in the privmsg tab.
    User { serv: &'a str, nick: &'a str },

    /// Show the message in all tabs of a server.
    AllServTabs { serv: &'a str },

    /// Show the message in currently active tab.
    CurrentTab,
}

/// Source of a message from the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MsgSource {
    /// Message sent in a server tab.
    Serv { serv: String },

    /// Message sent in a channel tab.
    Chan { serv: String, chan: String },

    /// Message sent in a privmsg tab.
    User { serv: String, nick: String },
}

impl MsgSource {
    pub fn serv_name(&self) -> &str {
        match self {
            MsgSource::Serv { serv }
            | MsgSource::Chan { serv, .. }
            | MsgSource::User { serv, .. } => serv,
        }
    }

    pub fn to_target(&self) -> MsgTarget {
        match self {
            MsgSource::Serv { serv } => MsgTarget::Server { serv },
            MsgSource::Chan { serv, chan } => MsgTarget::Chan { serv, chan },
            MsgSource::User { serv, nick } => MsgTarget::User { serv, nick },
        }
    }

    pub fn visible_name(&self) -> &str {
        match self {
            MsgSource::Serv { serv, .. } => serv,
            MsgSource::Chan { chan, .. } => chan,
            MsgSource::User { nick, .. } => nick,
        }
    }
}

// NOTE: Keep the variants sorted in increasing significance, to avoid updating
// style with higher significance for a less significant style (e.g. updating
// from `Highlight` to `NewMsg` in `set_tab_style`).
// TODO: This shouldn't be a part of the API
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum TabStyle {
    Normal,
    NewMsg,
    Highlight,
}

/// UI events
#[derive(Debug)]
pub enum Event {
    Abort,
    Msg {
        msg: String,
        source: MsgSource,
    },
    Lines {
        lines: Vec<String>,
        source: MsgSource,
    },
    Cmd {
        cmd: String,
        source: MsgSource,
    },
}

pub trait UI: objekt::Clone {
    fn draw(&self);

    /// Create a new server tab.
    fn new_server_tab(&self, serv: &str, alias: Option<String>);

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
    fn set_topic(&self, topic: &str, ts: Tm, serv: &str, chan: &str);

    /// Set style of the given tabs.
    fn set_tab_style(&self, style: TabStyle, target: &MsgTarget);

    /// Do we have a tab for the given user? This is useful for deciding where to show a PRIVMSG
    /// coming from server; e.g. messages from services sometimes shown in their own tabs,
    /// sometimes in the server tab.
    fn user_tab_exists(&self, serv: &str, nick: &str) -> bool;
}

#[derive(Clone)]
pub struct CombinedUIs<UI1, UI2> {
    ui1: UI1,
    ui2: UI2,
}

impl<UI1: UI + Clone, UI2: UI + Clone> UI for CombinedUIs<UI1, UI2> {
    fn draw(&self) {
        self.ui1.draw();
        self.ui2.draw();
    }

    fn new_server_tab(&self, serv: &str, alias: Option<String>) {
        self.ui1.new_server_tab(serv, alias.clone());
        self.ui2.new_server_tab(serv, alias);
    }

    fn close_server_tab(&self, serv: &str) {
        self.ui1.close_server_tab(serv);
        self.ui2.close_server_tab(serv);
    }

    fn new_chan_tab(&self, serv: &str, chan: &str) {
        self.ui1.new_chan_tab(serv, chan);
        self.ui2.new_chan_tab(serv, chan);
    }

    fn close_chan_tab(&self, serv: &str, chan: &str) {
        self.ui1.close_chan_tab(serv, chan);
        self.ui2.close_chan_tab(serv, chan);
    }

    fn close_user_tab(&self, serv: &str, nick: &str) {
        self.ui1.close_user_tab(serv, nick);
        self.ui2.close_user_tab(serv, nick);
    }

    fn add_client_msg(&self, msg: &str, target: &MsgTarget) {
        self.ui1.add_client_msg(msg, target);
        self.ui2.add_client_msg(msg, target);
    }

    fn add_msg(&self, msg: &str, ts: Tm, target: &MsgTarget) {
        self.ui1.add_msg(msg, ts, target);
        self.ui2.add_msg(msg, ts, target);
    }

    fn add_err_msg(&self, msg: &str, ts: Tm, target: &MsgTarget) {
        self.ui1.add_err_msg(msg, ts, target);
        self.ui2.add_err_msg(msg, ts, target);
    }

    fn add_client_err_msg(&self, msg: &str, target: &MsgTarget) {
        self.ui1.add_client_err_msg(msg, target);
        self.ui2.add_client_err_msg(msg, target);
    }

    fn clear_nicks(&self, serv: &str) {
        self.ui1.clear_nicks(serv);
        self.ui2.clear_nicks(serv);
    }

    fn set_nick(&self, serv: &str, nick: &str) {
        self.ui1.set_nick(serv, nick);
        self.ui2.set_nick(serv, nick);
    }

    fn add_privmsg(
        &self,
        sender: &str,
        msg: &str,
        ts: Tm,
        target: &MsgTarget,
        highlight: bool,
        is_action: bool,
    ) {
        self.ui1
            .add_privmsg(sender, msg, ts, target, highlight, is_action);
        self.ui2
            .add_privmsg(sender, msg, ts, target, highlight, is_action);
    }

    fn add_nick(&self, nick: &str, ts: Option<Tm>, target: &MsgTarget) {
        self.ui1.add_nick(nick, ts, target);
        self.ui2.add_nick(nick, ts, target);
    }

    fn remove_nick(&self, nick: &str, ts: Option<Tm>, target: &MsgTarget) {
        self.ui1.remove_nick(nick, ts, target);
        self.ui2.remove_nick(nick, ts, target);
    }

    fn rename_nick(&self, old_nick: &str, new_nick: &str, ts: Tm, target: &MsgTarget) {
        self.ui1.rename_nick(old_nick, new_nick, ts, target);
        self.ui2.rename_nick(old_nick, new_nick, ts, target);
    }

    fn set_topic(&self, topic: &str, ts: Tm, serv: &str, chan: &str) {
        self.ui1.set_topic(topic, ts, serv, chan);
        self.ui2.set_topic(topic, ts, serv, chan);
    }

    fn set_tab_style(&self, style: TabStyle, target: &MsgTarget) {
        self.ui1.set_tab_style(style, target);
        self.ui2.set_tab_style(style, target);
    }

    fn user_tab_exists(&self, serv: &str, nick: &str) -> bool {
        // TODO weird
        self.ui1.user_tab_exists(serv, nick)
    }
}

pub fn combine<UI1: UI, UI2: UI>(ui1: UI1, ui2: UI2) -> CombinedUIs<UI1, UI2> {
    CombinedUIs { ui1, ui2 }
}
