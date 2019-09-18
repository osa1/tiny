//! Defines the `Client` trait.

use crate::utils::SplitIterator;

pub trait Client {
    /// Get host name of this connection.
    fn get_serv_name(&self) -> &str;

    /// Get current nick. Not that this returns the nick we're currently trying when the nick is
    /// not yet accepted. See `is_nick_accepted`.
    fn get_nick(&self) -> String;

    /// Is current nick accepted by the server?
    fn is_nick_accepted(&self) -> bool;

    /// Send a message directly to the server. "\r\n" suffix is added by this method.
    fn raw_msg(&mut self, msg: String);

    /// Split a privmsg to multiple messages so that each message is, when the hostname and nick
    /// prefix added by the server, fits in one IRC message.
    ///
    /// `extra_len`: Size (in bytes) for a prefix/suffix etc. that'll be added to each line.
    fn split_privmsg<'a>(&self, extra_len: usize, msg: &'a str) -> SplitIterator<'a>;

    /// Send a privmsg. Note that this method does not split long messages into smaller messages;
    /// use `split_privmsg` for that.
    fn privmsg(&mut self, target: &str, msg: &str, ctcp_action: bool);

    /// Join the given list of channels.
    fn join(&mut self, chans: &[&str]);

    /// Set away status. `None` means not away.
    fn away(&mut self, msg: Option<&str>);

    /// Change nick. This may fail (ERR_NICKNAMEINUSE) so wait for confirmation (a NICK message
    /// back from the server, with the old nick as prefix).
    fn nick(&mut self, new_nick: &str);

    /// Send a QUIT message to the server, with optional "reason". This stops the client; so the
    /// sender end of the `Cmd` channel and the receiver end of the IRC message channel (for
    /// outgoing messages) will be dropped.
    fn quit(&mut self, reason: Option<String>);
}
