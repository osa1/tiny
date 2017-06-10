//! To see how color numbers map to actual colors in your terminal run
//! `cargo run --example colors`. Use tab to swap fg/bg colors.

/// A server to auto-connect on startup.
pub struct Server {
    /// Address of the server
    pub server_addr: &'static str,

    /// Port of the server
    pub server_port: u16,

    /// Hostname to be used in connection registration
    pub hostname: &'static str,

    /// Real name to be used in connection registration
    pub real_name: &'static str,

    /// Nicks to try when connecting to this server. tiny tries these sequentially, and starts
    /// adding trailing underscores to the last one if none of the nicks are available.
    pub nicks: &'static [&'static str],

    /// Commands to automatically run after joining to the server. Useful for e.g. registering the
    /// nick via nickserv or joining channels. Uses tiny command syntax.
    pub auto_cmds: &'static [&'static str],
}

pub static SERVERS: [Server; 2] =
    [ Server {
          server_addr: "chat.freenode.net",
          server_port: 6667,
          hostname: "tiny",
          real_name: "tiny",
          nicks: &["tiny_user"],
          auto_cmds: &["msg NickServ identify hunter2",
                       "join #haskell"],
      },

      Server {
          server_addr: "irc.mozilla.org",
          server_port: 6667,
          hostname: "tiny",
          real_name: "tiny",
          nicks: &["tiny_user"],
          auto_cmds: &["msg NickServ identify hunter2",
                       "join #rust"],
      }
    ];

////////////////////////////////////////////////////////////////////////////////////////////////////
// Colors
////////////////////////////////////////////////////////////////////////////////////////////////////

/// Colors used to highlight nicks
pub static NICK_COLORS: [u8; 15] =
    [ 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15 ];

pub use termbox_simple::*;

#[derive(Debug, Clone, Copy)]
pub struct Style {
    /// Termbox fg.
    pub fg: u16,

    /// Termbox bg.
    pub bg: u16,
}

pub const CLEAR: Style =
    Style {
        fg: TB_DEFAULT,
        bg: TB_DEFAULT,
    };

pub const USER_MSG: Style =
    Style {
        fg: 15,
        bg: TB_DEFAULT,
    };

pub const ERR_MSG: Style =
    Style {
        fg: 15 | TB_BOLD,
        bg: 1,
    };

pub const TOPIC: Style =
    Style {
        fg: 14 | TB_BOLD,
        bg: TB_DEFAULT,
    };

pub const CURSOR: Style =
    USER_MSG;

pub const JOIN: Style =
    Style {
        fg: 242,
        bg: TB_DEFAULT,
    };

pub const PART: Style =
    Style {
        fg: 242,
        bg: TB_DEFAULT,
    };

pub const NICK: Style =
    Style {
        fg: 242,
        bg: TB_DEFAULT,
    };

pub const EXIT_DIALOGUE: Style =
    Style {
        fg: TB_DEFAULT,
        bg: 4,
    };

pub const HIGHLIGHT: Style =
    Style {
        fg: 161,
        bg: TB_DEFAULT,
    };

// Currently unused
// pub const MENTION: Style =
//     Style {
//         fg: 220,
//         bg: TB_DEFAULT,
//     };

pub const COMPLETION: Style =
    Style {
        fg: 84,
        bg: TB_DEFAULT,
    };

pub const TIMESTAMP: Style =
    Style {
        fg: 15 | TB_BOLD,
        bg: TB_DEFAULT,
    };

pub const TAB_ACTIVE: Style =
    Style {
        fg: 15 | TB_BOLD,
        bg: 0,
    };

pub const TAB_NORMAL: Style =
    Style {
        fg: 8,
        bg: 0,
    };

pub const TAB_IMPORTANT: Style =
    Style {
        fg: 9 | TB_BOLD,
        bg: 0,
    };

pub const TAB_HIGHLIGHT: Style =
    Style {
        fg: 5,
        bg: 0,
    };
