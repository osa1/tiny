# tiny - Yet another console IRC client

[![Travis Build Status](https://travis-ci.org/osa1/tiny.svg?branch=master)](https://travis-ci.org/osa1/tiny)
[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)

tiny is an IRC client written in Rust.

![tiny in action](tiny.png)

![tiny in action](tiny_mac.png)

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

- Key bindings inspired by terminal emulators and vim. See [key bindings
  section](#key-bindings) below.

- Configurable colors

- SASL authentication

- Configurable desktop notifications on new messages

- znc compatible

- TLS support

## Installation

Install the Rust nightly toolchain, clone the repo, and run

```
cargo install --path tiny
```

If you don't want to clone the repo, you can use

```
cargo install --git https://github.com/osa1/tiny
```

If you have an older version installed, add `--force` to the command you're
using.

Arch Linux users can install tiny from the [AUR].

[AUR]: https://aur.archlinux.org/packages/tiny-irc-client-git/

Since version 0.3.0 tiny needs OpenSSL or LibreSSL headers and runtime
libraries. See [rust-openssl documentation] for installation instructions.

[rust-openssl documentation]: https://docs.rs/openssl/0.10.26/openssl/#automatic

### Using rustls instead of native TLS library

To use [rustls], add `--no-default-features --features tls-rustls` to the
install command you're using.

When building with rustls tiny doesn't require a native SSL library (OpenSSL or
LibreSSL).

[rustls]: https://github.com/ctz/rustls

### Dependencies

* OpenSSL or LibreSSL (default, not when rustls is used)
* libdbus (Linux only)

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
    - addr: irc.mozilla.org
      ...

    - addr: chat.freenode.net
      ...
```

By default tiny connects to both servers. You can connect to only the second
server by passing `freenode` as a command line argument.

You can use `--config <path>` to specify your config file location.

## Key bindings

- `C-a`/`C-e` to move cursor beginning/end in the input field

- `C-k` for deleting rest of the line

- `C-w` for deleting a word

- `C-left`/`C-right` for moving one word backward/forward

- `page up`/`page down` or `shift-up`/`shift-down` or  for scrolling

- `C-n`/`C-p` for next/previous tab

- `C-c enter` to quit.

- `alt-{1,9}` switch to nth tab

- `alt-{char}` switch to next tab with underlined `char`

- `alt-0` switch to last tab

- `alt-left/right` move tab to left/right

- `C-x` edit current message in `$EDITOR`

## Commands

Commands start with `/` character.

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

- `/reload`: Reload configuration

- `/clear`: Clears tab contents

- `/switch <string>`: Switch to the first tab which has the given string in the name.

- `/statusline`: Enable or disable status line on top which gives you info about
  current settings of a tab.

- `/ignore`: Ignore `join/quit` messages in a channel. Running this command in
  a server tab applies it to all channels of that server. You can check your
  ignore state in the status line.

- `/notify [off|mentions|messages]`: Enable and disable desktop notifications.
  Running this command in a server tab applies it to all channels of that
  server. You can check your notify state in the status line.

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md).

## Contributors

- [Ömer Sinan Ağacan](https://github.com/osa1)
- [Abin Simon](https://github.com/meain)
- [Umur Gedik](https://github.com/umurgdk)
- [Chiu Yue Chun](https://github.com/BrianOn99)
- [@wammies](https://github.com/wammies)
- [Nick Econopouly](https://github.com/wrycode)
