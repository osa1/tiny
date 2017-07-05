//! To see how color numbers map to actual colors in your terminal run
//! `cargo run --example colors`. Use tab to swap fg/bg colors.
use serde::Deserialize;
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
    #[serde(default)]
    pub colors: Colors,
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

# Color theme based on 256 colors (if supported), colors can be defined as color
# index (0-255) or with its name
#
# Accepted color names are:
# default (0), black (0), maroon (1), green (2), olive (3), navy (4),
# purple (5), teal (6), silver (7), gray (8), red (9), lime (10),
# yellow (11), blue (12), magenta (13), cyan (14), white (15)
#
# Attributes can be combined (e.g [bold, underline]), and valid values are bold
# and underline
colors:
    nick: [ 1, 2, 3, 4, 5, 6, 7, 9, 10, 11, 12, 13, 14 ]

    clear:
        fg: default
        bg: default

    user_msg:
        fg: black
        bg: default

    err_msg:
        fg: black
        bg: maroon
        attrs: [bold]

    topic:
        fg: cyan
        bg: default
        attrs: [bold]

    cursor:
        fg: black
        bg: default

    join:
        fg: lime
        bg: default
        attrs: [bold]

    part:
        fg: maroon
        bg: default
        attrs: [bold]

    nick_change:
        fg: lime
        bg: default
        attrs: [bold]

    faded:
        fg: 242
        bg: default

    exit_dialogue:
        fg: default
        bg: navy

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
        fg: default
        bg: default
        attrs: [bold]

    tab_normal:
        fg: gray
        bg: default

    tab_new_msg:
        fg: purple
        bg: default

    tab_highlight:
        fg: red
        bg: default
        attrs: [bold]\n", log_dir.as_path().to_str().unwrap())
}

////////////////////////////////////////////////////////////////////////////////////////////////////
// Colors
////////////////////////////////////////////////////////////////////////////////////////////////////

pub use termbox_simple::*;
use serde::de::{self, Deserializer, Visitor, MapAccess};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Style {
    /// Termbox fg.
    pub fg: u16,

    /// Termbox bg.
    pub bg: u16,
}


#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Colors {
    pub nick: Vec<u8>,
    pub clear: Style,
    pub user_msg: Style,
    pub err_msg: Style,
    pub topic: Style,
    pub cursor: Style,
    pub join: Style,
    pub part: Style,
    pub nick_change: Style,
    pub faded: Style,
    pub exit_dialogue: Style,
    pub highlight: Style,
    pub completion: Style,
    pub timestamp: Style,
    pub tab_active: Style,
    pub tab_normal: Style,
    pub tab_new_msg: Style,
    pub tab_highlight: Style,
}

impl Default for Colors {
    fn default() -> Self {
        Colors {
            nick: vec![ 1, 2, 3, 4, 5, 6, 7, 9, 10, 11, 12, 13, 14 ],
            clear: Style { fg: TB_DEFAULT, bg: TB_DEFAULT },
            user_msg: Style { fg: 0, bg: TB_DEFAULT },
            err_msg: Style { fg: 0 | TB_BOLD, bg: 1 },
            topic: Style { fg: 14 | TB_BOLD, bg: TB_DEFAULT },
            cursor: Style { fg: 0, bg: TB_DEFAULT },
            join: Style { fg: 10 | TB_BOLD, bg: TB_DEFAULT },
            part: Style { fg: 1 | TB_BOLD, bg: TB_DEFAULT },
            nick_change: Style { fg: 10 | TB_BOLD, bg: TB_DEFAULT },
            faded: Style { fg: 242, bg: TB_DEFAULT },
            exit_dialogue: Style { fg: TB_DEFAULT, bg: 4 },
            highlight: Style { fg: 9 | TB_BOLD, bg: TB_DEFAULT },
            completion: Style { fg: 84, bg: TB_DEFAULT },
            timestamp: Style { fg: 242, bg: TB_DEFAULT },
            tab_active: Style { fg: 0 | TB_BOLD, bg: 0 },
            tab_normal: Style { fg: 8, bg: 0 },
            tab_new_msg: Style { fg: 5, bg: 0 },
            tab_highlight: Style { fg: 9 | TB_BOLD, bg: 0 },
        }
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////
// Color parsing
////////////////////////////////////////////////////////////////////////////////////////////////////

// Color names are taken from https://en.wikipedia.org/wiki/List_of_software_palettes
const COLORS: [(&'static str, u16); 17] =
[
    ("default", TB_DEFAULT), // Default fg/bg color of the terminal
    ("black",   0),
    ("maroon",  1),
    ("green",   2),
    ("olive",   3),
    ("navy",    4),
    ("purple",  5),
    ("teal",    6),
    ("silver",  7),
    ("gray",    8),
    ("red",     9),
    ("lime",    10),
    ("yellow",  11),
    ("blue",    12),
    ("magenta", 13),
    ("cyan",    14),
    ("white",   15)
];

const ATTRS: [(&'static str, u16); 2] =
[
    ("bold",      TB_BOLD),
    ("underline", TB_UNDERLINE)
];

fn parse_color(val: String) -> Option<u16> {
    for &(name, color) in &COLORS {
        if val == name {
            return Some(color);
        }
    }

    // If color name doesn't match try get a number
    val.parse().ok()
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

                writeln!(formatter,
                         "fg: 0-255 or color name\n\
                         bg: 0-255 or color name\n\
                         attrs: [{}]\n\n\
                         color names: {}", attrs, colors)
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
                                 .ok_or_else(|| de::Error::invalid_value(de::Unexpected::UnitVariant, &self))?;

                             fg = Some(color);
                        },

                        Field::Bg => {
                            let color = parse_color(map.next_value()?)
                                .ok_or_else(|| de::Error::invalid_value(de::Unexpected::UnitVariant, &self))?;

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

    #[test]
    fn parse_pre_color_config() {
        // Parse config file without a theme field. Important to be able to
        // parse old config files.
        let config = "\
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
log_dir: path";
        match serde_yaml::from_str(config) {
            Err(yaml_err) => {
                println!("{}", yaml_err);
                assert!(false);
            }
            Ok(Config { .. }) => {}
        }
    }
}
