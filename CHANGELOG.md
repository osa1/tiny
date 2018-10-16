# WIP

- A bug when using an invalid nick in `/msg` command fixed (#111).
- Bumped native-tls dependency -- fixes build for newer OpenSSLs (#114).
- A bug when sending multi-line text (via C-x or pasting) fixed (#113).

# 2018/09/01: 0.4.3

- tiny now supports pasting multi-line strings. It runs `$EDITOR` to let you
  edit the paste before sending. After closing the editor the final contents of
  the file (excluding comment lines) are sent. Note: we currently don't support
  commands in paste mode, so none of the lines can start with `/`.
- Ney key binding `C-x` implemented for editing current message in `$EDITOR`.
- Fixed a bug when pasing a string starting with a newline (#86).
- `auto_cmds` config field is gone and nick change and identification handling
  is updated.

  A major pain point for me has been the handling of nick changes when the
  server doesn't support SASL (sigh). We now solve this problem by simplifying
  (removing!) `auto_cmds` field and refactoring nick change logic:

  - We now only consider the nick as changed if we hear a NICK response from the
    server. This way we no longer have to revert a nick change when the request
    fails or is rejected.

  - Config file format changed: auto_cmds is gone, two new fields are added:
    `join` (a list of channels) and `nickserv_ident` (nickserv password to send
    on connecting and nick change).

    Note that `join` is technically old, but it just wasn't advertised as a
    config file field.

  This breaks backwards compatibility, but simplifies the code and nick changes
  and identification are now handled better.

# 2018/04/24: 0.4.2

- Previously tiny showed a `-` line in a private message tab when we got a
  `QUIT` message from the target of the tab. It now shows a `+` line when the
  user quits and then joins to a channel that we participate in.
- A bug that caused tiny to crash when dbus daemon is not configured properly
  fixed (#97).

# 2018/03/24: 0.4.1

- Fixed rendering bugs with ncurses 6.1 (#96).

# 2018/02/24: 0.4.0

- `/switch` command added to quickly switch to a different tab using a
  substring of the tab name.
- `Del` key is now handled. It deletes character under the cursor.
- Some tweaks and a bug fix (#45) in tab bar rendering. Selected tab is now
  stays visible in the tab bar after resizing.
- Connection closure on remote side when TLS is enabled is now handled (#48).
- `alt-char` bindings implemented to switching between tabs.
- Fixed some bugs in `join` command used in `auto_cmds` (#49, #38).
- Tabs can now be moved left/right with `alt-left/right` keys (#52).
- Input field cursor location now preserved after resize.
- `TOPIC` messages are now handled (#58).
- `RPL_AWAY` is now handled (#55). Away message is shown in user tab.
- `/ignore` command added to ignore `join/quit` messages in channels.
- New server config field `pass` added for connecting to password-protected
  servers (e.g. znc).
- Fixed a bug that caused tiny to fail to connect via TLS on some systems
  (#64).
- Fixed some bugs that caused incorrect tab bar rendering in some cases (#76).
- tiny no longer creates `~/logs` directory. This directory was used for debug
  logs in the past (#82).
- `NOTICE` messages (used by services like `NickServ`, `MemoServ`, `Global`
  etc.) are now shown in server tabs unless there's already a tab for the sender
  (#21).
- New command line argument `--config` added for specifying config file
  location (#81).
- tiny can now show desktop notifications for incoming messages. See README for
  notification options. Defaults: show notifications for mentions in channels
  and all private messages.
- Added SASL authentication support. See the configuration section in README
  for how to enable it.

# 2017/11/12: 0.3.0

- Fixed a bug that caused wrong scrolling in input field after changing nick.
- Tab completion now wraps after reaching the end/beginning (when navigating
  with TAB or arrow keys).
- Numeric reply 435 (aka. ERR_BANONCHAN) is now handled (#29).
- tiny now properly renders ACTION messages.
- `/me` command added for sending ACTION messages.
- A bug in the input field that caused crashes fixed.
- tiny now supports TLS! Add `tls: true` to your server setting in
  `.tinyrc.yml` to use. The field is optional and the default is `false`.
- Color code parser now returns default rather than panicking when color code
  is greater than 16 (#34).
- It is now possible to send messages to servers. Any messages sent to a server
  tabs will be sent to the server directly. `/msg <serv_addr> <msg>` can be used
  in `auto_cmds`, where `<serv_addr>` is the `addr` field of the server
  (specified in `.tinyrc.yml`). This can be used for e.g. server-specific login
  methods.

# 2017/10/15: 0.2.5

- `/clear` command implement for clearing tab contents (#22).
- Command line arguments are now considered as patterns to be searched in server
  addressed. tiny only connects to servers that matches at least one of the
  given patterns. Not passing any command line arguments means connecting to all
  servers in the config. Useful for connecting only a subset of servers listed
  in the config. See README as an example use.

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
