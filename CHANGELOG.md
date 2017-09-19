# 2017/09/19: 0.2.4

- tiny can now connect to IPv6 servers.

# 2017/09/18: 0.2.3

- Channel name list is now reset on `RPL_NAMREPLY`. This fixes #23.
- A bug that caused "highlight" state of tabs fixed.
  (new messages in channel tabs no longer cause a tab in "highlight" state to
  move to "new message" style, #24)
- Fixed a bug that caused not updating channel status (highlight etc.) after
  `alt-{0,9}` keys (#26).

# 2017/08/05: 0.2.2

- Scrolling now scrolls one visible line rather than one complete line in the
  widget (which can be rendered as multiple lines).
- `/names` command implemented (see README).
- Key bindings `alt-{0,9}` added for switching tabs (see README).

# 2017/08/04: 0.2.1

- User tab names are now updated when the user changes their nick.
- Reverted a change made on termbox to be able to run on systems without
  terminfo files (#19).

# 2017/08/01: 0.2.0

- A bug triggered by single-digit color codes fixed.
- NickServ messages now shown in server tabs (previously: shown in privmsg tabs)
- Messages with non-visible characters are now logged without any modification.
  These characters are now filtered by the UI.
- A `msg_area` bug that caused not scrolling automatically when a new message
  arrived fixed.
- Switched to `mio` from in-house event loop for OSX support. tiny now runs on
  OSX!
- `/away` command implemented.
- `/nick` command implemented for changing nicks.
- HOME/END keys now scroll to top/bottom of a chat window.
- Colors are now fully configurable! You can live reload config changes via the
  `/reload` command. Thanks @umurgdk for the contribution!
- Fixed a bug that caused re-joining `/close`d channels on reconnect.
- tiny can now split long messages into smaller messages to make sure the
  command will fit into 512 bytes on the receiving side. (#15)
- tiny now buffers outgoing messages and only write to sockets when they're
  ready for writing. This fixes some crashes and/or losing messages.

# 2017/06/11: First announcement
