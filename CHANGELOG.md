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
