use std::collections::HashMap;
use std::fmt::Display;

use serde::de::{MapAccess, Unexpected, Visitor};
use serde::{Deserialize, Deserializer};
use term_input::{Arrow, FKey, Key};

#[derive(Debug, PartialEq)]
pub(crate) struct KeyMap(HashMap<Key, KeyAction>);

#[derive(Debug, Copy, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum KeyAction {
    Cancel,
    Disable,
    Exit,

    RunEditor,

    TabNext,
    TabPrev,
    TabMoveLeft,
    TabMoveRight,
    TabGoto(char),

    MessagesPageUp,
    MessagesPageDown,
    MessagesScrollUp,
    MessagesScrollDown,
    MessagesScrollTop,
    MessagesScrollBottom,

    Input(char),
    InputAutoComplete,
    InputNextEntry,
    InputPrevEntry,
    InputSend,
    InputDeletePrevChar,
    InputDeleteNextChar,
    InputDeleteToStart,
    InputDeleteToEnd,
    InputDeletePrevWord,
    InputMoveCursEnd,
    InputMoveCursStart,
    InputMoveCursLeft,
    InputMoveCursRight,
    InputMoveWordLeft,
    InputMoveWordRight,
}

impl Display for KeyAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            KeyAction::Cancel => "cancel",
            KeyAction::Disable => "disable",
            KeyAction::Exit => "exit",
            KeyAction::RunEditor => "run_editor",
            KeyAction::TabNext => "tab_next",
            KeyAction::TabPrev => "tab_prev",
            KeyAction::TabMoveLeft => "tab_move_left",
            KeyAction::TabMoveRight => "tab_move_right",
            KeyAction::TabGoto(c) => return writeln!(f, "tab_goto: {}", c),
            KeyAction::MessagesPageUp => "messages_page_up",
            KeyAction::MessagesPageDown => "messages_page_down",
            KeyAction::MessagesScrollUp => "messages_scroll_up",
            KeyAction::MessagesScrollDown => "messages_scroll_down",
            KeyAction::MessagesScrollTop => "messages_scroll_top",
            KeyAction::MessagesScrollBottom => "messages_scroll_bottom",
            KeyAction::Input(c) => return writeln!(f, "input_{}", c),
            KeyAction::InputAutoComplete => "input_auto_complete",
            KeyAction::InputNextEntry => "input_next_entry",
            KeyAction::InputPrevEntry => "input_prev_entry",
            KeyAction::InputSend => "input_send",
            KeyAction::InputDeletePrevChar => "input_delete_prev_char",
            KeyAction::InputDeleteNextChar => "input_delete_next_char",
            KeyAction::InputDeleteToStart => "input_delete_to_start",
            KeyAction::InputDeleteToEnd => "input_delete_to_end",
            KeyAction::InputDeletePrevWord => "input_delete_prev_word",
            KeyAction::InputMoveCursEnd => "input_move_curs_end",
            KeyAction::InputMoveCursStart => "input_move_curs_start",
            KeyAction::InputMoveCursLeft => "input_move_curs_left",
            KeyAction::InputMoveCursRight => "input_move_curs_right",
            KeyAction::InputMoveWordLeft => "input_move_word_left",
            KeyAction::InputMoveWordRight => "input_move_word_right",
        };
        writeln!(f, "{}", s)
    }
}

impl Default for KeyMap {
    fn default() -> Self {
        let map = vec![
            (Key::Esc, KeyAction::Cancel),
            (Key::Ctrl('c'), KeyAction::Exit),
            (Key::Ctrl('x'), KeyAction::RunEditor),
            (Key::Ctrl('n'), KeyAction::TabNext),
            (Key::Ctrl('p'), KeyAction::TabPrev),
            (Key::AltArrow(Arrow::Left), KeyAction::TabMoveLeft),
            (Key::AltArrow(Arrow::Right), KeyAction::TabMoveRight),
            (Key::AltChar('1'), KeyAction::TabGoto('1')),
            (Key::AltChar('2'), KeyAction::TabGoto('2')),
            (Key::AltChar('3'), KeyAction::TabGoto('3')),
            (Key::AltChar('4'), KeyAction::TabGoto('4')),
            (Key::AltChar('5'), KeyAction::TabGoto('5')),
            (Key::AltChar('6'), KeyAction::TabGoto('6')),
            (Key::AltChar('7'), KeyAction::TabGoto('7')),
            (Key::AltChar('8'), KeyAction::TabGoto('8')),
            (Key::AltChar('9'), KeyAction::TabGoto('9')),
            (Key::AltChar('0'), KeyAction::TabGoto('0')),
            (Key::Ctrl('u'), KeyAction::MessagesPageUp),
            (Key::Ctrl('d'), KeyAction::MessagesPageDown),
            (Key::PageUp, KeyAction::MessagesPageUp),
            (Key::PageDown, KeyAction::MessagesPageDown),
            (Key::ShiftArrow(Arrow::Up), KeyAction::MessagesScrollUp),
            (Key::ShiftArrow(Arrow::Down), KeyAction::MessagesScrollDown),
            (Key::Home, KeyAction::MessagesScrollTop),
            (Key::End, KeyAction::MessagesScrollBottom),
            (Key::Tab, KeyAction::InputAutoComplete),
            (Key::Arrow(Arrow::Up), KeyAction::InputPrevEntry),
            (Key::Arrow(Arrow::Down), KeyAction::InputNextEntry),
            (Key::Char('\r'), KeyAction::InputSend),
            (Key::Backspace, KeyAction::InputDeletePrevChar),
            (Key::Del, KeyAction::InputDeleteNextChar),
            (Key::Ctrl('a'), KeyAction::InputMoveCursStart),
            (Key::Ctrl('e'), KeyAction::InputMoveCursEnd),
            (Key::Ctrl('k'), KeyAction::InputDeleteToEnd),
            (Key::Ctrl('w'), KeyAction::InputDeletePrevWord),
            (Key::Arrow(Arrow::Left), KeyAction::InputMoveCursLeft),
            (Key::Arrow(Arrow::Right), KeyAction::InputMoveCursRight),
            (Key::CtrlArrow(Arrow::Left), KeyAction::InputMoveWordLeft),
            (Key::CtrlArrow(Arrow::Right), KeyAction::InputMoveWordRight),
        ];
        let hash_map = map.into_iter().collect::<HashMap<_, _>>();
        KeyMap(hash_map)
    }
}

impl Display for KeyMap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (key, action) in self.0.iter() {
            writeln!(f, "key: {:?}, action: {}", key, action)?;
        }
        Ok(())
    }
}

impl KeyMap {
    pub(crate) fn get(&self, key: &Key) -> Option<KeyAction> {
        self.0.get(key).cloned()
    }

    pub(crate) fn load(&mut self, key_map: &KeyMap) {
        self.0.extend(key_map.0.iter())
    }
}

impl<'de> Deserialize<'de> for KeyMap {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct KeyMapVisitor;

        impl<'de> Visitor<'de> for KeyMapVisitor {
            type Value = KeyMap;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(
                    formatter,
                    "a map of key bindings. ex. 'ctrl_a: input_move_curs_start'"
                )?;
                write!(formatter, "defaults: \n{}", KeyMap::default())
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut key_map = HashMap::new();
                while let Some((key, action)) = map.next_entry::<MappedKey, KeyAction>()? {
                    key_map.insert(key.0, action);
                }
                Ok(KeyMap(key_map))
            }
        }
        deserializer.deserialize_map(KeyMapVisitor)
    }
}

#[derive(Debug)]
pub(crate) struct MappedKey(Key);

impl<'de> Deserialize<'de> for MappedKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct MappedKeyVisitor;

        impl<'de> Visitor<'de> for MappedKeyVisitor {
            type Value = MappedKey;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(formatter, "single keys: backspace, del, end, esc, home, pgdown, pgup, tab, up, down, left right, [a-z], [0-9], [f1-f12]. ")?;
                write!(
                    formatter,
                    "modifiers with an arrow key, function key, or single characters:  alt, shift, ctrl"
                )
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                let key_combo = v.split_once('_');
                let single_key = |s: &str| {
                    let mut chars = s.chars();
                    if let (Some(c), None) = (chars.next(), chars.next()) {
                        Ok(c)
                    } else {
                        Err(E::invalid_value(Unexpected::Str(s), &Self))
                    }
                };

                let key = match key_combo {
                    None => match v {
                        "backspace" => Key::Backspace,
                        "del" => Key::Del,
                        "end" => Key::End,
                        "esc" => Key::Esc,
                        "home" => Key::Home,
                        "pgdown" => Key::PageDown,
                        "pgup" => Key::PageUp,
                        "tab" => Key::Tab,
                        other => {
                            if let Some(arrow) = parse_arrow(other) {
                                Key::Arrow(arrow)
                            } else if let Some(f_key) = parse_f_key(other) {
                                Key::FKey(f_key)
                            } else {
                                Key::Char(single_key(other)?)
                            }
                        }
                    },
                    Some((k1, k2)) => match k1 {
                        "alt" => {
                            if let Some(arrow) = parse_arrow(k2) {
                                Key::AltArrow(arrow)
                            } else if let Some(f_key) = parse_f_key(k2) {
                                Key::AltF(f_key)
                            } else {
                                Key::AltChar(single_key(k2)?)
                            }
                        }
                        "ctrl" => {
                            if let Some(arrow) = parse_arrow(k2) {
                                Key::CtrlArrow(arrow)
                            } else if let Some(f_key) = parse_f_key(k2) {
                                Key::CtrlF(f_key)
                            } else {
                                Key::Ctrl(single_key(k2)?)
                            }
                        }
                        "shift" => {
                            if let Some(arrow) = parse_arrow(k2) {
                                Key::ShiftArrow(arrow)
                            } else if let Some(f_key) = parse_f_key(k2) {
                                Key::ShiftF(f_key)
                            } else {
                                return Err(E::invalid_value(Unexpected::Str(k2), &Self));
                            }
                        }
                        unexp => return Err(E::invalid_value(Unexpected::Str(unexp), &Self)),
                    },
                };

                if term_input::is_valid_key(key) {
                    Ok(MappedKey(key))
                } else {
                    Err(E::invalid_value(Unexpected::Str(v), &Self))
                }
            }
        }

        deserializer.deserialize_str(MappedKeyVisitor)
    }
}

fn parse_arrow(s: &str) -> Option<Arrow> {
    match s {
        "up" => Some(Arrow::Up),
        "down" => Some(Arrow::Down),
        "left" => Some(Arrow::Left),
        "right" => Some(Arrow::Right),
        _ => None,
    }
}

fn parse_f_key(s: &str) -> Option<FKey> {
    match s {
        "f1" => Some(FKey::F1),
        "f2" => Some(FKey::F2),
        "f3" => Some(FKey::F3),
        "f4" => Some(FKey::F4),
        "f5" => Some(FKey::F5),
        "f6" => Some(FKey::F6),
        "f7" => Some(FKey::F7),
        "f8" => Some(FKey::F8),
        "f9" => Some(FKey::F9),
        "f10" => Some(FKey::F10),
        "f11" => Some(FKey::F11),
        "f12" => Some(FKey::F12),
        _ => None,
    }
}

#[test]
fn deser_key() {
    let s = "alt_p";
    let key: MappedKey = serde_yaml::from_str(s).unwrap();
    assert_eq!(Key::AltChar('p'), key.0);
    let s = "alt_è";
    let key: MappedKey = serde_yaml::from_str(s).unwrap();
    assert_eq!(Key::AltChar('è'), key.0);
    let s = "alt__";
    let key: MappedKey = serde_yaml::from_str(s).unwrap();
    assert_eq!(Key::AltChar('_'), key.0);
    let s = "ctrl_/";
    let key: MappedKey = serde_yaml::from_str(s).unwrap();
    assert_eq!(Key::Ctrl('/'), key.0);
    let s = "ctrl_f2";
    let key: MappedKey = serde_yaml::from_str(s).unwrap();
    assert_eq!(Key::CtrlF(FKey::F2), key.0);
    let s = "alt_f2";
    let key: MappedKey = serde_yaml::from_str(s).unwrap();
    assert_eq!(Key::AltF(FKey::F2), key.0);
    let s = "shift_f2";
    let key: MappedKey = serde_yaml::from_str(s).unwrap();
    assert_eq!(Key::ShiftF(FKey::F2), key.0);
    let s = "shift_left";
    let key: MappedKey = serde_yaml::from_str(s).unwrap();
    assert_eq!(Key::ShiftArrow(Arrow::Left), key.0);
}

#[test]
fn deser_key_action_goto_tab() {
    let s = "tab_goto: 1";
    let a = serde_yaml::from_str::<KeyAction>(s).unwrap();
    assert_eq!(KeyAction::TabGoto('1'), a);
}

#[test]
fn deser_keymap() {
    let s = r#"
    ctrl_a: input_move_curs_start
    ctrl_e: 
      tab_goto: 1
    a: 
      input: b
    "#;
    let mut expect = KeyMap(HashMap::new());
    expect
        .0
        .insert(Key::Ctrl('a'), KeyAction::InputMoveCursStart);
    expect.0.insert(Key::Ctrl('e'), KeyAction::TabGoto('1'));
    expect.0.insert(Key::Char('a'), KeyAction::Input('b'));

    let key_map: KeyMap = serde_yaml::from_str(s).unwrap();
    assert_eq!(expect, key_map);
}
