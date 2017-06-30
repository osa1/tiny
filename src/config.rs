//! To see how color numbers map to actual colors in your terminal run
//! `cargo run --example colors`. Use tab to swap fg/bg colors.

use std::env::home_dir;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
pub struct Server {
    /// Address of the server
    pub addr: String,

    /// Port of the server
    pub port: u16,

    /// Hostname to be used in connection registration
    pub hostname: String,

    /// Real name to be used in connection registration
    pub realname: String,

    /// Nicks to try when connecting to this server. tiny tries these sequentially, and starts
    /// adding trailing underscores to the last one if none of the nicks are available.
    pub nicks: Vec<String>,

    /// Commands to automatically run after joining to the server. Useful for e.g. registering the
    /// nick via nickserv or joining channels. Uses tiny command syntax.
    pub auto_cmds: Vec<String>,
}

/// Similar to `Server`, but used when connecting via the `/connect` command.
#[derive(Debug, Clone, Deserialize)]
pub struct Defaults {
    pub nicks: Vec<String>,
    pub hostname: String,
    pub realname: String,
    pub auto_cmds: Vec<String>
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub servers: Vec<Server>,
    pub defaults: Defaults,
    pub theme: Theme,
    pub log_dir: String,
}

pub fn get_config_path() -> PathBuf {
    let mut config_path = home_dir().unwrap();
    config_path.push(".tinyrc.yml");
    config_path
}

pub fn get_default_config_yaml() -> String {
    let mut log_dir = home_dir().unwrap();
    log_dir.push("tiny_logs");
    format!("\
# Servers to auto-connect
servers:
    - addr: irc.mozilla.org
      port: 6667
      hostname: yourhost
      realname: yourname
      nicks: [tiny_user]
      auto_cmds:
          - 'msg NickServ identify hunter2'
          - 'join #tiny'

# Defaults used when connecting to servers via the /connect command
defaults:
    nicks: [tiny_user]
    hostname: yourhost
    realname: yourname
    auto_cmds: []

# Where to put log files
log_dir: '{}'

# Color theme based on 256 colors (if supported), colors can be defined as color index (0-255) or with it's name
# 
# Accepted color names are:
# default, black, darkred, darkgreen, darkyellow, darkblue, darkmagenta, darkcyan, lightgray, darkgray, 
# red, green, yellow, blue, magenta, cyan, white
#
# Attributes can be combined (e.g [bold, underline]), and valid values are bold, underline, reverse
theme:
    nick_colors: [ 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14 ]

    # Used for whitespace
    clear: 
        fg: default
        bg: default

    user_msg:
        fg: black
        bg: default

    err_msg:
        fg: black
        bg: darkred
        attrs: [underline]

    topic:
        fg: lightgray
        bg: default
        attrs: [bold]

    cursor:
        fg: black
        bg: default

    join:
        fg: green
        bg: default
        attrs: [bold]

    part:
        fg: darkred
        bg: default
        attrs: [bold]

    nick:
        fg: green
        bg: default
        attrs: [bold]

    faded:
        fg: 242
        bg: default

    exit_dialogue:
        fg: default
        bg: darkblue

    highlight:
        fg: red
        bg: default
        attrs: [bold]

    completion:
        fg: 84
        bg: default

    timestamp:
        fg: 242
        bg: default

    tab_active:
        fg: darkred
        bg: black
        attrs: [bold]

    tab_normal:
        fg: darkgray
        bg: black

    tab_new_msg:
        fg: darkmagenta
        bg: black

    tab_highlight:
        fg: red
        bg: black
        attrs: [bold]\n", log_dir.as_path().to_str().unwrap())
}

////////////////////////////////////////////////////////////////////////////////////////////////////
// Theme
////////////////////////////////////////////////////////////////////////////////////////////////////
use serde::de::{self, Deserialize, Deserializer, Visitor, MapAccess};
pub use termbox_simple::*;

#[derive(Debug, Clone, Copy)]
pub struct Style {
    /// Termbox fg.
    pub fg: u16,

    /// Termbox bg.
    pub bg: u16,
}

const COLORS: [(&'static str, u16); 17] = 
[
    // Default bg color of the terminal
    ("default",     TB_DEFAULT),

    // Dark variants
    ("black",       0),
    ("darkred",     1),
    ("darkgreen",   2),
    ("darkyellow",  3),
    ("darkblue",    4),
    ("darkmagenta", 5),
    ("darkcyan",    6),
    ("lightgray",   7),
    
    // Bright variants
    ("darkgray",    8),
    ("red",         9),
    ("green",       10),
    ("yellow",      11),
    ("blue",        12),
    ("magenta",     13),
    ("cyan",        14),
    ("white",       15)
];

const ATTRS: [(&'static str, u16); 3] =
[
    ("bold",      TB_BOLD),
    ("underline", TB_UNDERLINE),
    ("reverse",   TB_REVERSE)
];

fn parse_color(val: String) -> Result<u16, ()> {
    for &(name, color) in &COLORS {
        if val == name {
            return Ok(color);
        }
    }

    // If color name doesn't match try get a number
    val.parse().map_err(|_| ())
}

fn parse_attr(val: String) -> u16 {
    for &(name, attr) in &ATTRS {
        if name == val {
            return attr;
        }
    }

    return 0;
}

impl<'de> Deserialize<'de> for Style {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer<'de>
    {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "lowercase")]
        enum Field { Fg, Bg, Attrs }

        use std::fmt;

        struct StyleVisitor;
        impl<'de> Visitor<'de> for StyleVisitor {
            type Value = Style;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                let colors = COLORS.iter().map(|&(name, _)| name).collect::<Vec<&str>>().join(", ");
                let attrs = ATTRS.iter().map(|&(name, _)| name).collect::<Vec<&str>>().join(", ");

                let expected_format = format!("Expected format:\nfg: 0-255 | colorname\nbg: 0-255 | colorname\nattrs: [{}]\n\nColor Names: {}", attrs, colors);
                let msg = format!("color style\n{}", expected_format);

                formatter.write_str(msg.as_str())
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
                where M: MapAccess<'de>
            {
                let mut fg: Option<u16> = None;
                let mut bg: Option<u16> = None;
                let mut attr: u16 = 0;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Fg => {
                             let color = parse_color(map.next_value()?) 
                                 .map_err(|_| de::Error::invalid_value(de::Unexpected::UnitVariant, &self))?;

                             fg = Some(color);
                        },

                        Field::Bg => {
                            let color = parse_color(map.next_value()?)
                                .map_err(|_| de::Error::invalid_value(de::Unexpected::UnitVariant, &self))?;

                            bg = Some(color);
                        },

                        Field::Attrs => {
                            let attrs: Vec<String> = map.next_value()?;
                            attr = attrs.into_iter()
                                .map(parse_attr)
                                .fold(0, |style, a| style | a);
                        }
                    }
                }

                let fg = fg.ok_or_else(|| de::Error::missing_field("fg"))?;
                let bg = bg.ok_or_else(|| de::Error::missing_field("bg"))?;

                Ok(Style { fg: fg | attr, bg })
            }
        }

        deserializer.deserialize_map(StyleVisitor)
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Theme {
    /// Colors used to highlight nicks
    pub nick_colors: [u8; 14],
    pub clear: Style,
    pub user_msg: Style,
    pub err_msg: Style,
    pub topic: Style,
    pub cursor: Style,
    pub join: Style,
    pub part: Style,
    pub nick: Style,
    pub faded: Style,
    pub exit_dialogue: Style,
    pub highlight: Style,
    pub completion: Style,
    pub timestamp: Style,
    pub tab_active: Style,
    pub tab_normal: Style,
    pub tab_new_msg: Style,
    pub tab_highlight: Style
}

const fn default_theme() -> Theme {
    Theme {
        nick_colors: [ 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14 ],
        clear: Style { fg: TB_DEFAULT, bg: TB_DEFAULT },
        user_msg: Style { fg: 0, bg: TB_DEFAULT },
        err_msg: Style { fg: 0 | TB_BOLD, bg: 1 },
        topic: Style { fg: 14 | TB_BOLD, bg: TB_DEFAULT },
        cursor: Style { fg: 0, bg: TB_DEFAULT },
        join: Style { fg: 10 | TB_BOLD, bg: TB_DEFAULT },
        part: Style { fg: 1 | TB_BOLD, bg: TB_DEFAULT },
        nick: Style { fg: 10 | TB_BOLD, bg: TB_DEFAULT },
        faded: Style { fg: 242, bg: TB_DEFAULT },
        exit_dialogue: Style { fg: TB_DEFAULT, bg: 4 },
        highlight: Style { fg: 9 | TB_BOLD, bg: TB_DEFAULT },
        completion: Style { fg: 84, bg: TB_DEFAULT },
        timestamp: Style { fg: 242, bg: TB_DEFAULT },
        tab_active: Style { fg: 1 | TB_BOLD, bg: 0 },
        tab_normal: Style { fg: 8, bg: 0 },
        tab_new_msg: Style { fg: 5, bg: 0 },
        tab_highlight: Style { fg: 9 | TB_BOLD, bg: 0 },
    }
}

impl Default for Theme {
    fn default() -> Self {
        default_theme()
    }
}

static mut THEME: Theme = default_theme();

pub fn get_theme() -> &'static Theme {
    unsafe { &THEME }
}
pub fn set_theme(theme: Theme) {
    unsafe { THEME = theme; }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_yaml;

    #[test]
    fn parse_default_config() {
        match serde_yaml::from_str(&get_default_config_yaml()) {
            Err(yaml_err) => {
                println!("{}", yaml_err);
                assert!(false);
            }
            Ok(Config { .. }) => {}
        }
    }
}
