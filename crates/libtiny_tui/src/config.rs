// To see how color numbers map to actual colors in your terminal run
// `cargo run --example colors`. Use tab to swap fg/bg colors.

use serde::de::{self, Deserializer, MapAccess, Visitor};
use serde::Deserialize;
use std::fs::File;
use std::io::Read;
use std::path::Path;

pub use termbox_simple::*;

use crate::key_map::KeyMap;

#[derive(Deserialize)]
pub(crate) struct Config {
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

fn default_max_nick_length() -> usize {
    12
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Style {
    /// Termbox fg.
    pub fg: u16,

    /// Termbox bg.
    pub bg: u16,
}

#[derive(Deserialize)]
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
