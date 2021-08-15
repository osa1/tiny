// To see how color numbers map to actual colors in your terminal run
// `cargo run --example colors`. Use tab to swap fg/bg colors.

use libtiny_common::{ChanName, ChanNameRef};
use serde::de::{self, Deserializer, MapAccess, Visitor};
use serde::Deserialize;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::str::FromStr;

pub(crate) use termbox_simple::*;

use crate::key_map::KeyMap;
use crate::notifier::Notifier;

#[derive(Debug, Default, Deserialize)]
pub(crate) struct Config {
    pub(crate) servers: Vec<Server>,

    pub(crate) defaults: Defaults,

    #[serde(default)]
    pub(crate) colors: Colors,

    #[serde(default = "usize::max_value")]
    pub(crate) scrollback: usize,

    pub(crate) layout: Option<Layout>,

    #[serde(default = "default_max_nick_length")]
    pub(crate) max_nick_length: usize,

    #[serde(default)]
    pub(crate) key_map: Option<KeyMap>,
}

impl Config {
    /// Gets tab configs for `server`
    /// Prioritizing configs under the server or using defaults
    pub(crate) fn server_tab_configs(&self, server: &str) -> TabConfig {
        let server_config = self.servers.iter().find_map(|s| {
            if s.addr == server {
                Some(&s.configs)
            } else {
                None
            }
        });
        self.defaults.tab_configs.merge(server_config)
    }

    /// Gets tab configs for `chan` in `server`
    /// Prioritizing configs under the chan, then the server, then the defaults
    pub(crate) fn chan_tab_configs(&self, server: &str, chan: &ChanNameRef) -> TabConfig {
        let tab_config = self
            .servers
            .iter()
            .find(|s| s.addr == server)
            .and_then(|s| {
                s.join.iter().find_map(|c| {
                    if &c.name == chan {
                        Some(c.config)
                    } else {
                        None
                    }
                })
            });
        self.server_tab_configs(server).merge(tab_config.as_ref())
    }

    pub(crate) fn user_tab_configs(&self) -> TabConfig {
        TabConfig {
            ignore: Some(false),
            notifier: Some(Notifier::Messages),
        }
    }
}

#[derive(Debug, Deserialize, PartialEq)]
pub(crate) struct Server {
    pub(crate) addr: String,
    pub(crate) join: Vec<Chan>,
    #[serde(flatten)]
    pub(crate) configs: TabConfig,
}

#[derive(Debug, Default, Deserialize, PartialEq)]
pub(crate) struct Defaults {
    #[serde(flatten)]
    pub(crate) tab_configs: TabConfig,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Chan {
    pub name: ChanName,
    pub config: TabConfig,
}

impl<'de> Deserialize<'de> for Chan {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct ChanVisitor;

        impl<'de> Visitor<'de> for ChanVisitor {
            type Value = Chan;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(formatter, "a channel name with arguments")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Chan::from_str(v).map_err(de::Error::custom)
            }
        }
        deserializer.deserialize_str(ChanVisitor)
    }
}

impl FromStr for Chan {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Make sure channel starts with '#'
        let s = if !s.starts_with('#') {
            format!("#{}", s)
        } else {
            s.to_string()
        };
        // Try to split chan name and args
        match s.split_once(" ") {
            // with args
            Some((name, args)) => {
                let configs = TabConfig::from_str(args)?;
                Ok(Chan {
                    name: ChanName::new(name.to_string()),
                    config: configs,
                })
            }
            // chan name only
            None => Ok(Chan {
                name: ChanName::new(s),
                config: TabConfig::default(),
            }),
        }
    }
}

#[derive(Debug, Default, Copy, Clone, Deserialize, PartialEq)]
pub struct TabConfig {
    /// `true` if tab is ignoring join/part messages
    #[serde(default)]
    pub ignore: Option<bool>,
    /// Notification setting for tab
    #[serde(default)]
    pub notifier: Option<Notifier>,
}

impl TabConfig {
    /// Overwrites `self`'s values with `o`'s if `o`'s are `Some`
    pub(crate) fn merge(&self, o: Option<&TabConfig>) -> TabConfig {
        if let Some(o) = o {
            TabConfig {
                ignore: o.ignore.or(self.ignore),
                notifier: o.notifier.or(self.notifier),
            }
        } else {
            self.to_owned()
        }
    }
}

impl FromStr for TabConfig {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let config = s
            .split('-')
            .filter_map(|arg| (!arg.is_empty()).then(|| arg.trim()))
            .try_fold(TabConfig::default(), |mut tc, arg| match arg {
                // flag
                "ignore" => {
                    tc.ignore = Some(true);
                    Ok(tc)
                }
                arg => match arg.split_once(' ') {
                    // arg with parameter
                    Some(("notify", val)) => {
                        tc.notifier = Some(Notifier::from_str(val)?);
                        Ok(tc)
                    }
                    _ => Err(format!("Unexpected argument: {:?}", arg)),
                },
            })?;
        Ok(config)
    }
}

fn default_max_nick_length() -> usize {
    12
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Style {
    /// Termbox fg
    pub fg: u16,

    /// Termbox bg
    pub bg: u16,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Layout {
    Compact,
    Aligned,
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
    pub tab_joinpart: Style,
}

impl Default for Colors {
    fn default() -> Self {
        Colors {
            nick: vec![1, 2, 3, 4, 5, 6, 7, 9, 10, 11, 12, 13, 14],
            clear: Style {
                fg: TB_DEFAULT,
                bg: TB_DEFAULT,
            },
            user_msg: Style {
                fg: 0,
                bg: TB_DEFAULT,
            },
            err_msg: Style { fg: TB_BOLD, bg: 1 },
            topic: Style {
                fg: 14 | TB_BOLD,
                bg: TB_DEFAULT,
            },
            cursor: Style {
                fg: 0,
                bg: TB_DEFAULT,
            },
            join: Style {
                fg: 10 | TB_BOLD,
                bg: TB_DEFAULT,
            },
            part: Style {
                fg: 1 | TB_BOLD,
                bg: TB_DEFAULT,
            },
            nick_change: Style {
                fg: 10 | TB_BOLD,
                bg: TB_DEFAULT,
            },
            faded: Style {
                fg: 242,
                bg: TB_DEFAULT,
            },
            exit_dialogue: Style {
                fg: TB_DEFAULT,
                bg: 4,
            },
            highlight: Style {
                fg: 9 | TB_BOLD,
                bg: TB_DEFAULT,
            },
            completion: Style {
                fg: 84,
                bg: TB_DEFAULT,
            },
            timestamp: Style {
                fg: 242,
                bg: TB_DEFAULT,
            },
            tab_active: Style { fg: TB_BOLD, bg: 0 },
            tab_normal: Style { fg: 8, bg: 0 },
            tab_new_msg: Style { fg: 5, bg: 0 },
            tab_highlight: Style {
                fg: 9 | TB_BOLD,
                bg: 0,
            },
            tab_joinpart: Style {
                fg: 11,
                bg: TB_DEFAULT,
            },
        }
    }
}

//
// Parsing
//

// Color names are taken from https://en.wikipedia.org/wiki/List_of_software_palettes
const COLORS: [(&str, u16); 17] = [
    ("default", TB_DEFAULT), // Default fg/bg color of the terminal
    ("black", 0),
    ("maroon", 1),
    ("green", 2),
    ("olive", 3),
    ("navy", 4),
    ("purple", 5),
    ("teal", 6),
    ("silver", 7),
    ("gray", 8),
    ("red", 9),
    ("lime", 10),
    ("yellow", 11),
    ("blue", 12),
    ("magenta", 13),
    ("cyan", 14),
    ("white", 15),
];

const ATTRS: [(&str, u16); 2] = [("bold", TB_BOLD), ("underline", TB_UNDERLINE)];

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
    0
}

impl<'de> Deserialize<'de> for Style {
    fn deserialize<D>(d: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "lowercase")]
        enum Field {
            Fg,
            Bg,
            Attrs,
        }

        use std::fmt;

        struct StyleVisitor;
        impl<'de> Visitor<'de> for StyleVisitor {
            type Value = Style;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                let colors = COLORS
                    .iter()
                    .map(|&(name, _)| name)
                    .collect::<Vec<&str>>()
                    .join(", ");
                let attrs = ATTRS
                    .iter()
                    .map(|&(name, _)| name)
                    .collect::<Vec<&str>>()
                    .join(", ");

                writeln!(
                    formatter,
                    "fg: 0-255 or color name\n\
                     bg: 0-255 or color name\n\
                     attrs: [{}]\n\n\
                     color names: {}",
                    attrs, colors
                )
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut fg: Option<u16> = None;
                let mut bg: Option<u16> = None;
                let mut attr: u16 = 0;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Fg => {
                            let color = parse_color(map.next_value()?).ok_or_else(|| {
                                de::Error::invalid_value(de::Unexpected::UnitVariant, &self)
                            })?;

                            fg = Some(color);
                        }

                        Field::Bg => {
                            let color = parse_color(map.next_value()?).ok_or_else(|| {
                                de::Error::invalid_value(de::Unexpected::UnitVariant, &self)
                            })?;

                            bg = Some(color);
                        }

                        Field::Attrs => {
                            let attrs: Vec<String> = map.next_value()?;
                            attr = attrs
                                .into_iter()
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

        d.deserialize_map(StyleVisitor)
    }
}

pub(crate) fn parse_config(config_path: &Path) -> Result<Config, serde_yaml::Error> {
    let contents = {
        let mut str = String::new();
        let mut file = File::open(config_path).unwrap();
        file.read_to_string(&mut str).unwrap();
        str
    };

    serde_yaml::from_str(&contents)
}
