//! This crate implements common types used by other libtiny crates. These types have their own
//! crate to avoid dependencies between unrelated libtiny crates, like libtiny_tui and
//! libtiny_client.

use std::cmp::Ordering;
use std::fmt;
use std::hash::Hash;

/// Channel names according to RFC 2812, section 1.3. Channel names are case insensitive, so this
/// type defines `Eq`, `Ord`, and `Hash` traits that work in a case-insensitive way. `Display`
/// shows the channel name with the original casing.
#[derive(Debug, Clone)]
pub struct ChanName {
    /// ASCII-lowercase version of the channel name, used when comparing and hashing.
    normalized: String,
    /// Original name of the channel. Can have upper and lower case characters.
    display: String,
}

// Used to normalize channel names. Rules are:
//
// - ASCII characters are mapped to their lowercase versions
// - '[', ']', '\\', '~' are mapped to '{', '}', '|', '^', respectively. See RFC 2812 section 2.2.
// - Non-ASCII characters are left unchanged.
fn to_lower(c: char) -> char {
    match c {
        '[' => '{',
        ']' => '}',
        '\\' => '|',
        '~' => '^',
        _ => c.to_ascii_lowercase(),
    }
}

impl ChanName {
    pub fn new(name: String) -> Self {
        let name_lower = name.chars().map(to_lower).collect();
        ChanName {
            normalized: name_lower,
            display: name,
        }
    }

    pub fn display(&self) -> &str {
        self.display.as_str()
    }

    pub fn normalized(&self) -> &str {
        self.normalized.as_str()
    }
}

impl PartialEq for ChanName {
    fn eq(&self, other: &Self) -> bool {
        self.normalized.as_str().eq(other.normalized.as_str())
    }
}

impl PartialEq<str> for ChanName {
    fn eq(&self, other: &str) -> bool {
        self.normalized.as_str().eq(other)
    }
}

impl PartialEq<String> for ChanName {
    fn eq(&self, other: &String) -> bool {
        self.normalized.eq(other)
    }
}

impl Eq for ChanName {}

impl PartialOrd for ChanName {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.normalized
            .as_str()
            .partial_cmp(other.normalized.as_str())
    }
}

impl PartialOrd<str> for ChanName {
    fn partial_cmp(&self, other: &str) -> Option<Ordering> {
        self.normalized.as_str().partial_cmp(other)
    }
}

impl PartialOrd<String> for ChanName {
    fn partial_cmp(&self, other: &String) -> Option<Ordering> {
        self.normalized.partial_cmp(other)
    }
}

impl Ord for ChanName {
    fn cmp(&self, other: &Self) -> Ordering {
        self.normalized.as_str().cmp(other.normalized.as_str())
    }
}

impl Hash for ChanName {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.normalized.hash(state)
    }
}

impl fmt::Display for ChanName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.display.fmt(f)
    }
}
