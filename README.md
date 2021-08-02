# tiny - Yet another terminal IRC client

tiny is an IRC client written in Rust.

<p float="left" align="middle">
  <img src="/assets/tiny.png" width="430" />
  <img src="/assets/tiny_mac.png" width="430" />
  <img src="https://user-images.githubusercontent.com/448274/73597059-47e71380-4539-11ea-844d-0ec260691911.png" width="430" />
</p>

## Features

- Clean UI: consecutive join/part/quit messages are shown in a single line, time
  stamps for a message is omitted if it's the same as the message before.
  (inspired by [irc-core](https://github.com/glguy/irc-core))

- All mentions to the user are collected in a "mentions" tab, including server
  and channel information. "mentions" tab solves the problem of missing mentions
  to you in channels after hours of inactivity.

- Mentions to the user in a channel is highlighted (the channel tab is also
  highlighted in the tab list)

- Simple config file format for automatically connecting to servers, joining
  channels, registering the nickname etc. See [configuration
  section](#configuration) below.

- Nick tab-completion in channels

- Nicks in channels are colored.

- Disconnect detection and automatic reconnects. You can keep tiny running on
  your laptop and it automatically reconnects after a sleep etc.

- Configurable key bindings inspired by terminal emulators and vim. See [key
  bindings section](#key-bindings) below.

- Configurable colors

- SASL authentication

- Configurable desktop notifications on new messages (opt-in feature behind a
  feature flag, see below)

- znc compatible

- TLS support

## Installation

tiny works on Linux and OSX. Windows users can run it under Windows Subsystem
for Linux.

For pre-built binaries see [releases]. To build from source make sure you have
Rust 1.48 or newer. By default tiny uses [rustls] for TLS support, and desktop
notifications are disabled.

[releases]: https://github.com/osa1/tiny/releases
[rustls]: https://github.com/ctz/rustls

- To use system TLS library (OpenSSL or LibreSSL), add `--no-default-features
  --features=tls-native` to the command you're using. Note that this requires
  OpenSSL or LibreSSL headers and runtime libraries on Linux.

- To enable desktop notifications add `--features=desktop-notifications`. This
  requires libdbus on Linux.

To install in a clone:

```
cargo install --path crates/tiny
```

If you don't want to clone the repo, you can use

```
cargo install --git https://github.com/osa1/tiny
```

If you have an older version installed, add `--force` to the command you're
using.

Arch Linux users can install tiny from [AUR].

[AUR]: https://aur.archlinux.org/packages/tiny-irc-client-git/

tiny is tested on Linux and OSX.

## Configuration

tiny looks for these places for a config file:

- `$XDG_CONFIG_HOME/tiny/config.yml`
- (when `$XDG_CONFIG_HOME` is not defined) `$HOME/.config/tiny/config.yml`
- (deprecated) `$HOME/.tinyrc.yml`

when a config file is not found in one of these locations tiny creates one with
defaults and exists, printing path to the config file. Edit that file before
re-running tiny to change the defaults.

**A note on nick identification:** Some IRC servers such as ircd-seven (used by
Freenode) and InspIRCd (used by Mozilla) support identification via the `PASS`
command. This way of identification (rather than sending a message to a service
like `NickServ`) is better when some of the channels that you automatically
join require identification. To use this method enter your nick password to the
`pass` field in servers.

## Command line arguments

By default (i.e. when no command line arguments passed) tiny connects to all
servers listed in the config. tiny considers command line arguments as patterns
to be matched in server addresses, so you can pass command line arguments to
connect to only a subset of servers specified in the config. For example, in
this config:

```yaml
servers:
    - addr: chat.freenode.net
      ...

    - addr: irc.gnome.org
      ...
```

By default tiny connects to both servers. You can connect to only the first
server by passing `freenode` as a command line argument.

You can use `--config <path>` to specify your config file location.

## Key bindings

Key bindings can be configured in the config file, see the [wiki
page][key-bindings-wiki] for details.

Default key bindings:

- `C-a`/`C-e` move cursor to beginning/end in the input field

- `C-k` delete rest of the line

- `C-w` delete a word backwards

- `C-left`/`C-right` move one word backward/forward

- `page up`/`page down`, `shift-up`/`shift-down`, or `C-u`/`C-d` to scroll

- `C-n`/`C-p` next/previous tab

- `C-c enter` quit (asks for confirmation)

- `alt-{1,9}` switch to nth tab

- `alt-{char}` switch to next tab with underlined `char`

- `alt-0` switch to last tab

- `alt-left/right` move tab to left/right

- `C-x` edit current message in `$EDITOR`

[key-bindings-wiki]: https://github.com/osa1/tiny/wiki/Configuring-key-bindings

## Commands

Commands start with `/` character.

- `/help`: Show help messages of commands listed below.

- `/msg <nick> <message>`: Send a message to a user. Creates a new tab.

- `/join <channel>`: Join to a channel

- `/close`: Close the current tab. Leaves the channel if the current tab is a
  channel. Leaves the server if the tab is a server.

- `/connect <hostname>:<port>`: Connect to a server. Uses `defaults` in the
  config file for nick, realname, hostname and auto cmds.

- `/connect`: Reconnect to the current server. Use if you don't want to wait
  tiny to reconnect automatically after a connectivity problem.

- `/away <msg>`: Set away status

- `/away`: Remove away status

- `/nick <nick>`: Change nick

- `/names`: List all nicks in the current channel. You can use `/names <nick>` to
  check if a specific nick is in the channel.

- `/reload`: Reload TUI configuration

- `/clear`: Clears tab contents

- `/switch <string>`: Switch to the first tab which has the given string in the name.

- `/ignore`: Ignore `join/quit` messages in a channel. Running this command in
  a server tab applies it to all channels of that server. You can check your
  ignore state in the status line.

- `/notify [off|mentions|messages]`: Enable and disable desktop notifications.
  Running this command in a server tab applies it to all channels of that
  server. You can check your notify state in the status line.

- `/quit`: Quit

## Server commands

For commands not supported by tiny as a slash command, sending the command in
the server tab will send the message directly to the server.

### Examples:

- `LIST` will list all channels on the server
- `MOTD` will display the server Message of the Day
- `RULES` will display server rules
- etc...

## Community

Join us at #tiny in [irc.oftc.net][oftc] to chat about anything related to tiny!

[oftc]: https://www.oftc.net/
