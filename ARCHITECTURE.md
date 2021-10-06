# Architecture

This document describes tiny's high-level architecture, the parts that are
unlikely to change often, with the rationale behind some of the design
decisions. If you are interested in contributing to tiny, this is the best
place to start.

## Crates

tiny consists of 9 crates. The dependencies between these crates are as
follows:

![crate dependencies](/assets/crate_deps.png)

Separation to smaller crates was mainly motivated by two things:

- The crates `libtiny_client`, `term_input`, and `termbox_simple` are reusable
  outside of tiny. More details are in the individual sections of the crates
  below.

- Rust allows recursive imports within a crate and over time tiny started to
  become a big crate where everything depended on everything else. Separation
  to crates makes separating concerns easier and dependencies between
  modules/crates and crate interfaces become clear.

Unfortunately this makes it difficult to publish tiny on crates.io (#257). We
currently do not publish tiny on crates.io.

Below are the crates in more detail:

### tiny

`tiny` is the main crate that provides the `tiny` executable. It brings
everything together.

- Implements command-line interface (CLI) and command-line argument parsing
- Implements config file parsing
- Initializes loggers (both debug logging and message logging)
- Initializes tokio runtime and creates clients
- Initializes TUI
- Implements updating the TUI on client events (e.g. to show an incoming
  message on TUI). This is implemented in module `conn`.
- Implements updating clients on TUI events (e.g. to send a message when the
  user enters a message in the TUI). This is implemented in module `ui`.

#### Dependencies of `tiny`:

| Dependency     | Used for      |
| -------------- | ------------- |
| libtiny_client | Managing IRC connections (maintaining conns, getting incoming msgs, sending msgs, ...) |
| libtiny_tui    | Drawing the UI on the terminal |
| libtiny_logger | User and server message logging (not debug logging) |
| libtiny_wire   | For IRC message definitions (types), used to handle incoming IRC messages |
| libtiny_common | The "channel name" type |

### libtiny_client

Provides three key types to create and maintain an IRC connection:

- `ServerInfo`: encapsulates information to connect to an IRC server and
  maintain the connection (server address/port, NickServ/SASL/etc.
  credentials, nicks, ...).

- `Event`: an enum type for IRC events ("connected", "message received" etc.)

- `Client`: the connection handle type. Users create a `Client` by providing a
  `ServerInfo`. `Client` then maintains the connection (handles timeouts,
  disconnects, nick selection and identification etc. everything needed to keep
  the connection alive).

  Note that a client maintains one connection. If you want to connect to N
  servers you need N `Client`s.

  `Client::new()` also returns a tokio channel receiver for `Event`s. IRC
  events are passed to users via this channel. `tiny`'s `conn` module
  implements the handler for these events.

  The `Client` itself can be used to send messages, joining channels etc.

#### Dependencies of `libtiny_client`:

| Dependency     | Used for      |
| -------------- | ------------- |
| libtiny_wire   | IRC message parsing and generation|
| libtiny_common | The "channel name" type |

### libtiny_tui

Handles user input and drawing the terminal UI (TUI). At a high-level the API
is very similar to `libtiny_client`: on initialization the user passes a config
(file path for the tiny config file), `libtiny_tui` returns two tokio channel
for user input events and a TUI handle to update the TUI. The types are:

- `Event`: Enum for TUI events like a message submitted by the user, or an exit
  request.

- `TUI`: The type to modify TUI (create tabs, show messages etc.)

#### Dependencies of `libtiny_tui`:

| Dependency     | Used for      |
| -------------- | ------------- |
| term_input     | Input handling (reading events from `stdin`) |
| termbox_simple | Terminal manipulation (drawing) |
| libtiny_common | The "channel name" type |
| libtiny_wire   | Parsing IRC message formatting characters (colors etc.) |

### libtiny_logger

Implements logging IRC events (incoming messages, user left/joined etc.) to
user-specified log directory.

#### Dependencies of `libtiny_logger`:

| Dependency     | Used for      |
| -------------- | ------------- |
| libtiny_common | The "channel name" type |
| libtiny_wire   | Filtering out IRC message formatting characters (colors etc.) |

### libtiny_wire

Implements IRC message parsing and generation. Entry point for parsing is
`parse_irc_msg`, which returns at most one `Msg`, which is the type for IRC
messages.

For message generation, we only have a few functions like `privmsg`, `join`
etc. for the messages we need in tiny.

#### Dependencies of `libtiny_wire`:

| Dependency     | Used for      |
| -------------- | ------------- |
| libtiny_common | The "channel name" type |

### libtiny_common

This crate currently has just one type: `ChanName`, which is the type for
channel names.

RFC 2812 has two rules related to character names:

- Channel names are case insensitive
- The characters "{}|^" are considered lowercase, and their uppercase
  equivalents are "[]\\~".

There's also a rule implemented by servers:

- For non-ASCII characters the comparison is *case sensitive*.

The `ChanName` type provides a newtype with comparison implementation that
follows these rules.

Because channel names are widely used in the implementation of tiny, all other
tiny crates need the `ChanName` type. To avoid introducing dependencies between
large crates just for one type, we have this crate.

`libtiny_common` does not depend on other tiny crates.

### term_input

Parses stdin contents to key events. On initialization sets stdin to
non-blocking mode, to work around a WSL bug (#269). The main type is `Input`,
which implements `Stream` (from `futures`) to allow asynchronously reading
input events.

`term_input` allows creating multiple `Input`s, but you should only have one at
a time.

`term_input` does not use `terminfo`. Instead it has hard-coded byte sequences
for xterm key events. Most terminals I tried use xterm byte sequences so this
is mostly fine. However we've seen a case where a recent version of xterm
updated one of its byte sequences, causing tiny to not work properly, see #295.

For efficiently mapping bytes read from stdin to key events `term_input`
generates a parser from hard-coded xterm byte sequences using
`term_input_macros`.

#### Dependencies of `term_input`:

| Dependency        | Used for      |
| ----------------- | ------------- |
| term_input_macros | Generating parser for xterm byte sequences |

### term_input_macros

Provides a single procedural macro called `byte_seq_parser`. Given a list of
byte array to expression mappings, the macro returns a parser function that
internally uses a decision tree to parse the argument (a byte slice) to a value
by spending `O(n)` time on the input, where `n` is the size of the input.

Example:

```rust
byte_seq_parser! {
    my_parser -> &'static str,

    [1, 2, 3] => "first",
    [1, 3, 4] => "second",
    [2] => "third",
}
```

generates this function:

```rust
fn my_parser(buf: &[u8]) -> Option<(&'static str, usize)> {
    match buf.get(0usize) {
        None => None,
        Some(byte) => match byte {
            1u8 => match buf.get(1usize) {
                None => None,
                Some(byte) => match byte {
                    3u8 => match buf.get(2usize) {
                        None => None,
                        Some(byte) => match byte {
                            4u8 => Some(("second", 3usize)),
                            _ => None,
                        },
                    },
                    2u8 => match buf.get(2usize) {
                        None => None,
                        Some(byte) => match byte {
                            3u8 => Some(("first", 3usize)),
                            _ => None,
                        },
                    },
                    _ => None,
                },
            },
            2u8 => Some(("third", 1usize)),
            _ => None,
        },
    }
}
```

Second return value is the number of bytes consumed.

`term_input_macros` does not depend on other tiny crates.

### termbox_simple

Implements drawing to terminal and suspend/activate functions to temporarily
release the terminal (by resetting the attributes) while still allowing users
to update the internal buffer.

The main type is `Termbox` which allows updating cells on the terminal and
drawing the updates to the terminal. Internally the terminal is just a grid of
"cells", where a cell has a character, background color, and foreground color.
Character attributes like "underline" or "bold" are applied to the foreground
color.

After manipulating the internal buffer with `change_cell`, updates are rendered
with `present`. When `Termbox` is suspended with `suspend`, `present` doesn't
do anything. `change_cell` calls still update the internal buffer so after
`activate` the most recent changes are shown.

`termbox_simple` tries to be efficient by avoiding updates for cells on the
terminal that are not changed. This is done by comparing contents of the back
buffer (updates done to the last rendered buffer) and the front buffer
(currently drawn stuff). `present`, after updating the terminal, moves the back
buffer to front buffer.

`suspend` and `activate` are used to implement "edit in $EDITOR" function of
tiny, where when the user types `C-x` (or pastes a multi-line text) and tiny
runs `$EDITOR` (if the variable is set) with the contents of the input field as
the editor's buffer contents. On exit `activate` called to show tiny again.

`termbox_simple` does not depend on other tiny crates.
