// To see how color numbers map to actual colors in your terminal run
// `cargo run --example colors`. Use tab to swap fg/bg colors.

use libtiny_common::{ChanName, ChanNameRef};
use serde::de::{self, Deserializer, MapAccess, Visitor};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

use termbox_simple::*;

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

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub(crate) struct Server {
    pub(crate) addr: String,
    pub(crate) join: Vec<Chan>,
    #[serde(flatten)]
    pub(crate) config: TabConfig,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub(crate) struct Defaults {
    #[serde(default, flatten)]
    pub(crate) tab_config: TabConfig,
}

impl Default for Defaults {
    fn default() -> Self {
        Defaults {
            tab_config: TabConfig {
                ignore: Some(false),
                notify: Some(Notifier::default()),
            },
        }
    }
}

#[derive(Clone, Deserialize, Debug, PartialEq, Eq)]
#[serde(untagged)]
pub enum Chan {
    #[serde(deserialize_with = "deser_chan_name")]
    Name(ChanName),
    WithConfig {
        #[serde(deserialize_with = "deser_chan_name")]
        name: ChanName,
        #[serde(flatten)]
        config: TabConfig,
    },
}

impl Chan {
    pub fn from_cmd_args(s: &str) -> Result<Chan, String> {
        // Make sure channel starts with '#'
        let s = if !s.starts_with('#') {
            format!("#{}", s)
        } else {
            s.to_string()
        };
        // Try to split chan name and args
        match s.split_once(' ') {
            // with args
            Some((name, args)) => {
                let config = TabConfig::from_cmd_args(args)?;
                Ok(Chan::WithConfig {
                    name: ChanName::new(name.to_string()),
                    config,
                })
            }
            // chan name only
            None => Ok(Chan::Name(ChanName::new(s))),
        }
    }

    pub fn name(&self) -> &ChanNameRef {
        match self {
            Chan::Name(name) => name,
            Chan::WithConfig { name, .. } => name,
        }
        .as_ref()
    }
}

fn deser_chan_name<'de, D>(d: D) -> Result<ChanName, D::Error>
where
    D: Deserializer<'de>,
{
    let name: String = serde::de::Deserialize::deserialize(d)?;
    Ok(ChanName::new(name))
}

/// Map of TabConfigs by tab names
#[derive(Debug, Default)]
pub(crate) struct TabConfigs(HashMap<String, TabConfig>);

impl TabConfigs {
    pub(crate) fn get(
        &self,
        serv_name: &str,
        chan_name: Option<&ChanNameRef>,
    ) -> Option<TabConfig> {
        let key = if let Some(chan) = chan_name {
            format!("{}_{}", serv_name, chan.display())
        } else {
            serv_name.to_string()
        };
        self.0.get(&key).cloned()
    }

    pub(crate) fn get_mut(
        &mut self,
        serv_name: &str,
        chan_name: Option<&ChanNameRef>,
    ) -> Option<&mut TabConfig> {
        let key = if let Some(chan) = chan_name {
            format!("{}_{}", serv_name, chan.display())
        } else {
            serv_name.to_string()
        };
        self.0.get_mut(&key)
    }

    pub(crate) fn set(
        &mut self,
        serv_name: &str,
        chan_name: Option<&ChanNameRef>,
        config: TabConfig,
    ) {
        let key = if let Some(chan) = chan_name {
            format!("{}_{}", serv_name, chan.display())
        } else {
            serv_name.to_string()
        };
        self.0.insert(key, config);
    }

    pub(crate) fn set_by_server(&mut self, serv_name: &str, config: TabConfig) {
        for c in self
            .0
            .iter_mut()
            .filter(|entry| entry.0.starts_with(serv_name))
        {
            *c.1 = config;
        }
    }
}

impl From<&Config> for TabConfigs {
    fn from(config: &Config) -> Self {
        let mut tab_configs = HashMap::new();
        for server in &config.servers {
            let serv_tc = server.config.or_use(&config.defaults.tab_config);
            tab_configs.insert(server.addr.clone(), serv_tc);
            for chan in &server.join {
                let (name, tc) = match chan {
                    Chan::Name(name) => (name, serv_tc),
                    Chan::WithConfig { name, config } => (name, config.or_use(&serv_tc)),
                };
                tab_configs.insert(format!("{}_{}", server.addr, name.display()), tc);
            }
        }
        tab_configs.insert("_defaults".to_string(), config.defaults.tab_config);
        debug!("new {tab_configs:?}");
        Self(tab_configs)
    }
}

#[derive(Debug, Default, Copy, Clone, Deserialize, PartialEq, Eq)]
pub struct TabConfig {
    /// Whether the join/part messages are ignored.
    #[serde(default)]
    pub ignore: Option<bool>,

    /// Notification setting for tab.
    #[serde(default)]
    pub notify: Option<Notifier>,
}

impl TabConfig {
    pub(crate) fn from_cmd_args(s: &str) -> Result<TabConfig, String> {
        let mut config = TabConfig::default();
        let mut words = s.trim().split(' ').map(str::trim);

        while let Some(word) = words.next() {
            // `"".split(' ')` yields one empty string.
            if word.is_empty() {
                continue;
            }
            match word {
                "-ignore" => config.ignore = Some(true),
                "-notify" => match words.next() {
                    Some(notify_setting) => {
                        config.notify = Some(Notifier::from_cmd_args(notify_setting)?)
                    }
                    None => return Err("-notify parameter missing".to_string()),
                },
                other => return Err(format!("Unexpected channel parameter: {:?}", other)),
            }
        }

        Ok(config)
    }

    pub(crate) fn or_use(&self, config: &TabConfig) -> TabConfig {
        TabConfig {
            ignore: self.ignore.or(config.ignore),
            notify: self.notify.or(config.notify),
        }
    }

    pub(crate) fn toggle_ignore(&mut self) -> bool {
        let ignore = self.ignore.get_or_insert(false);
        *ignore = !&*ignore;
        *ignore
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

const ATTRS: [(&str, u16); 4] = [
    ("bold", TB_BOLD),
    ("underline", TB_UNDERLINE),
    ("italic", TB_ITALIC),
    ("strikethrough", TB_STRIKETHROUGH),
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
    // tiny creates a config file with the defaults when it can't find one, but the config file can
    // be deleted before a `/reload`.
    let contents = std::fs::read_to_string(config_path).map_err(|err| {
        <serde_yaml::Error as de::Error>::custom(format!(
            "Can't read config file '{}': {}",
            config_path.to_string_lossy(),
            err
        ))
    })?;
    serde_yaml::from_str(&contents)
}
