# Unreleased

- Fixed handling of CR, LF, and tab characters in IRC format parser. IRC RFCs
  don't allow standalone CR and LF characters, but some servers still them.
  tiny now shows those characters as single space. Tab characters are shown as
  8 spaces, as in tiny 0.9.0.

  This bug was introduced in 0.10.0 with 33df77e. (#366)

# 2021/11/07: 0.10.0

Thanks to @trevarj for contributing to this release.

## New features

- New command `/quit` added for quitting. Key binding for quitting (`C-c
  enter`) works as before.
- Key bindings can be configured in the config file. See the [wiki
  page][key-bindings-wiki] for details. (#328, #336)

## Bug fixes and other improvements

- `/msg <nick> <message>` command now accepts anything as `<nick>` as long as
  it doesn't start with `#`. (#324)
- Error message when sending a message in the mentions tab improved. (#329)
- Logger now prints "Logging ended at ..." lines when you close a tab or exit
  tiny. (8061042)
- Minor improvements in logging (d0505f2, bbb4b81)
- `/join` (without arguments) now rejoins the current channel. (#334)
- Handling of IRC formatting characters (colors etc.) in TUI and logger
  improved: 
  - TUI now handles "reset" control character, to reset the text style to the
    default.
  - Logger now filters out all control characters before writing to the file.
  (#360)

[key-bindings-wiki]: https://github.com/osa1/tiny/wiki/Configuring-key-bindings

# 2021/05/12: 0.9.0

Starting with this release, tarballs on GitHub are now built on Ubuntu 20.04
(instead of 18.04).

- Fixed highlighting tabs when `/ignore` is set. (#291)
- `/statusline` removed. See 6abc671 commit message for the motivation.
- Backspace handling in newer xterms fixed. (#295)
- When NickServ identification is used tiny now send identify messages when
  changing nicks. (#252)
- Tab characters in incoming messages are now rendered as 8 spaces. Previously
  tab characters would be removed, so the message "\thi" would be rendered as
  "hi" instead of "        hi". (#305)
- Fixed a bug when getting `RPL_NAMREPLY` from a server for channels we haven't
  joined. Previously we would create a channel in the TUI for each channel in
  the response. (#302)
- tiny now checks nick lists and realnames in the config file to make sure they
  are not empty. (#314)

# 2020/12/10: 0.8.0

Thanks to @trevarj and @shumvgolove for contributing to this release.

## New features

- Channels with join/leave events are now highlighted with a yellow-ish color.
  Default color can be overridden in the config file. (#262)

## Bug fixes and other improvements

- It's now possible to build tiny with stable Rust 1.48 or newer. Previously
  tiny required nightly toolchain. (#241)
- Fixed a TUI bug when `scrollback` is set. (#265)
- In builds without desktop notification support, `/notify` commands now print a
  helpful message on how to enable it. Previously `/notify` would behave as if
  desktop notification support is enabled but notifications would not work.
  (#270)
- Fixed a bug when showing messages relayed by bouncers. (#271)
- Fixed losing input field contents when pasting a multi-line text and `$EDITOR`
  is not set. (#280)
- Multi-line pastes now inserted into the cursor location. Previously the text
  would be inserted at the end of the current line.
- Fixed a bug when showing messages with host mask as message target. (#278)

# 2020/09/20: 0.7.0

Thanks to @trevarj, @kennylevinsen, and @LordMZTE for contributing to this
release.

## New features

- New command `/help` added. (ec00007)
- `/names` now sorts nicks lexicographically. (#235)
- To make joining channels with +R mode (which usually means joining is only
  allowed after identification via NickServ) more robust, tiny now makes 3
  attempts to join a channel, with a 10-second delay after each attempt, when it
  gets a 477 response and the user has NickServ identification enabled
  (`nickserv_ident` field in the config). Even though we send IDENTIFY messages
  (after `RPL_WELCOME`) before JOIN messages (after `RPL_ENDOFMOTD`), sometimes
  identification takes too long and JOIN command fails with a 477. We now try
  joining again with 10 seconds breaks, up to 3 times. (#236, #240)
- When `$EDITOR` is (n)vim or emacs, `C-x` now places the cursor at its location
  in the input field in the vim/emacs buffer. (#243)

## Bug fixes and other improvements

- Fixed a TUI bug introduced in 0.6.0 when pasting long single-line text using
  `C-x`. (#225)
- Fixed a TUI bug introduced in 0.6.0 when rendering a long line of join/leave
  events. (#227)
- Password fields in the default config file (created automatically on first
  run) are now commented-out, to allow connecting to tiny IRC channel with the
  default config without having to make changes. (2af2357)
- tiny now re-sets current away status on reconnect. Previously the away status
  would be lost. (#234)
- Improved `RPL_YOURHOST` parsing for parsing server names of some
  non-standard-conforming servers. This is not a user-visible change unless
  you're connecting to servers that don't follow IRC standards closely. (#239)
- Fixed a TUI crash when the terminal height is less than two lines.
- Debug logs (enabled with `TINY_LOG` env variable using `env_logger` filter
  syntax) are now printed to `tiny_debug_logs.txt` file in the log directory. If
  logging is disabled then the file is created at tiny's working directory.
  (#238)
- Handling of IRC messages with ambiguous prefix (when it's unclear whether the
  sender is a server or nick) improved. tiny should now work better with some
  bouncers such as soju. (#249)
- Fixed a bug in IRC message parser. (#251)
- Fixed a bug where on spotty connections a server tab (not the entire
  application) would get stuck in "Connecting..." stage (while opening a socket
  to the server) and not respond to user commands like `/connect`. (#255)
- Fixed a bug where tiny would print "Reconnecting in 30 seconds" on connection
  error (or timeout) but would actually reconnect in 60 seconds instead of 30.
  (bfd4e19)
- TUI now adds a nick to the tab completion list of a channel when the user
  posts for the first time. This is to support tab completion on some servers
  that don't implement the RFCs properly. (#253)
- Logger is slightly improved to work better with servers that don't implement
  the RFCs properly. (85051ae)
- Fixed a bug when first argument to `/msg` is a channel rather than a nick. The
  command is supposed to be used for sending a message to a user so we now do
  more error checking and reject the command if the first character is for a
  channel name. (62df491)
- Implemented channel name case sensitivity rules according to RFC 2812. This
  fixes a bug when we join e.g. `#MyChannel` and someone sends a message to
  `#mychannel`. In that case some servers send `PRIVMSG`s to users in the
  channel with the sender's encoding (`#mychannel`), which would previously
  cause tiny to (incorrectly) create a new tab for the channel `#mychannel`
  instead of showing the message in `#MyChannel`. (#248)
- TUI tab bar layout fixed when channel names contain non-ASCII unicode
  characters. (0c86a32)
- tiny binaries are much smaller, thanks to removed features in dependencies
  like tokio, futures, and env_logger. For example, libdbus + libssl build is
  4.9M in 0.6.0 and 4.0M in 0.7.0.

# 2020/06/28: 0.6.0

Thanks to @trevarj, @Kabouik, @meain, and @jbg for contributing to this
release.

## New features

- It's now possible to build tiny with [rustls] instead of [native-tls]. See
  README for instructions. (#172)
- A new optional server field 'alias' added to the configuration file for
  specifying aliases for servers, to be shown in the tab line. This is useful
  when a server address is long, or just an IP address, or you want to show
  something different than the server address in the tab bar (#186).
- tiny now has a proper CLI, supporting `--help` and `--version` arguments.
- Input field now grows vertically on overflow, instead of scrolling. The old
  scrolling behavior is used when there isn't enough space in the window to
  extend input field vertically. (#101)
- A new setting 'scrollback' added to limit max. number of lines in tabs. The
  limit is off by default (old behavior). (#219)

[rustls]: https://github.com/ctz/rustls
[native-tls]: https://github.com/sfackler/rust-native-tls

## Bug fixes and other improvements

- TUI: Tab bar scrolls to left after closing tabs to fit more tabs into the
  visible part of the tab bar (#164). See #164 for an example of previous
  behavior.
- tiny now reads the system cert store (for TLS connections) only once, instead
  of on every new connection. (#172)
- A bug when rendering exit dialogue (shown on `C-c`) fixed.
- TUI: A text field bug is fixed when updating the scroll value after deleting a
  word with `C-w`.
- Fixed a panic when a nick list of a server or the default nick list is empty.
  (#184)
- Fixed handling of invalid UTF-8 sequences in messages. (#194)
- A few crashes when connecting to some IRC servers fixed. tiny is now more
  resilient to non-standard-conforming messages from servers. (#211)
- Fixed a bug in logger when the channel name contains forward slash character.
  (#214)
- Fixed editor support (C-x). Old implementation used to block tiny's event loop
  while an editor is running and cause connection timeouts when it runs for too
  long. Editors are now run in a separate thread without blocking tiny's event
  loop. (#185)
- tiny now breaks long lines without whitespace into multiple lines. Previously
  we'd only break lines at whitespace, so long lines without any whitespace
  would be cut off at the end of the screen. (#202)

# 2020/01/08: 0.5.1

- When a domain name resolves to multiple IP addresses tiny now tries connecting
  to the rest of the addresses when one fails (#144).
- Fixed a bug introduced in 0.5.0 where the client did not update its internal
  state after changing nick, causing TUI and client state disagreeing on what
  the current nick is (#149, introduced with #138).
- tiny no longer needs a 'hostname' field in servers in the config file (#150).
- Version requests are now shown in the server tab if a tab for the requester
  does not exist (#145).
- Fixed a bug where we forgot to reset "nick accepted" state on disconnect,
  causing incorrect nick selection when reconnecting (introduced with #138).
- Fixed a bug that caused the client to loop when the connection is closed on
  the remote end (#153, another bug introduced with #138).
- tiny now uses `$XDG_CONFIG_HOME/tiny/config.yml` for the default config file
  location. The old location `$HOME/.tinyrc.yml` is still used when there isn't
  a config file in the new location, to avoid breakage. `$HOME/.config` is used
  for `$XDG_CONFIG_HOME` when the env variable is not available (#152).
- Fixed a panic when some clients return later than TUI when exiting tiny.

# 2019/10/05: 0.5.0

Starting with this release tiny is no longer distributed on crates.io. Please
get it from the git repo at https://github.com/osa1/tiny.

- With the exception of TUI most of tiny is rewritten for this release. See #138
  for the details. The TLDR is that the code should now be easier to hack on.
- tiny now properly logs all messaging to the `log_dir` specified in tinyrc.
  (#100, #56, #132)
- Address name resolving step no longer blocks the UI, and is interruptible
  (e.g. by issuing a `/connect` command, or by closing the tab/client) (#3).
- Fixed glitches in the TUI when rendering non-visible (0-column) or wide (shown
  in multiple terminal columns) unicode characters (#115).

# 2019/08/06: 0.4.5

- IRC color codes and ASCII control chars are now removed from desktop
  notifications to avoid weird notification rendering and glitches.
- Fixed a bug that caused panics when running tiny with spotty connections (lots
  of disconnects) (#119).
- Fix build with Rust nightly (#133), fix new warnings.

# 2018/12/22: 0.4.4

- A bug when using an invalid nick in `/msg` command fixed (#111).
- Bumped native-tls dependency -- fixes build for newer OpenSSLs (#114).
- A bug when sending multi-line text (via C-x or pasting) fixed (#113).
- Update to `C-x` (paste mode): empty lines are now sent as a space (" ").
  Useful when e.g. sending long text with multiple paragraphs (#112).
- Fixed deprecation warnings for nightly.

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
