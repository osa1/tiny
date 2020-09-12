//! This crate implements common types used by other libtiny crates. These types have their own
//! crate to avoid dependencies between unrelated libtiny crates, like libtiny_tui and
//! libtiny_client.

use std::cmp::Ordering;
use std::fmt;
use std::hash::Hash;

/// Channel names according to RFC 2812, section 1.3. Channel names are case insensitive, so this
/// type defines `Eq`, `Ord`, and `Hash` traits that work in a case-insensitive way. `Display`
/// shows the channel name with the original casing.
///
/// Note that IRC is an ASCII-based protocol and non-ASCII characters in channel names should not
/// be accepted, but some servers like Unreal3.2.10.3 allow unicode channel names but implement
/// case insensitivity incorrectly and only handle case-insensitive comparison in ASCII characters
/// in the channel name. For example, "#Ömer" and "#ömer" are different channels but "#Omer" and
/// "#omer" (correctly) are the same. To play nicely with those servers (which are unfortunately
/// used in the wild) we also implement that buggy behavior. In a standard-conforming server
/// non-ASCII channel names should not be allowed and this behavior should not matter.
#[derive(Debug, Clone)]
pub struct ChanName {
    /// ASCII-lowercase version of the channel name, used when comparing and hashing.
    normalized: String,
    /// Original name of the channel. Can have upper and lower case characters.
    display: String,
}

impl ChanName {
    pub fn new(name: String) -> Self {
        let name_ascii_lower = name.to_ascii_lowercase();
        ChanName {
            normalized: name_ascii_lower,
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
