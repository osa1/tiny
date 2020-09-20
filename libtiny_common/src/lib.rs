//! This crate implements common types used by other libtiny crates.

use std::borrow::Borrow;
use std::hash::{Hash, Hasher};
use std::ops::Deref;

/// Channel names according to RFC 2812, section 1.3. Channel names are case insensitive, so this
/// type defines `Eq`, and `Hash` traits that work in a case-insensitive way. `ChanName::display`
/// method shows the channel name with the original casing.
#[derive(Debug, Clone)]
pub struct ChanName(String);

/// Slice version of `ChanName`
#[derive(Debug)]
pub struct ChanNameRef(str);

impl Deref for ChanName {
    type Target = ChanNameRef;

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

// https://github.com/rust-lang/rust/blob/10b3595ba6a4c658c9dea105488fc562c815e434/library/std/src/path.rs#L1735
impl AsRef<ChanNameRef> for ChanName {
    fn as_ref(&self) -> &ChanNameRef {
        ChanNameRef::new(self.0.as_ref())
    }
}

impl<'a> Borrow<ChanNameRef> for ChanName {
    fn borrow(&self) -> &ChanNameRef {
        self.as_ref()
    }
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
        ChanName(name)
    }

    pub fn display(&self) -> &str {
        &self.0
    }
}

impl ChanNameRef {
    pub fn new(name: &str) -> &Self {
        unsafe { &*(name as *const str as *const ChanNameRef) }
    }

    pub fn display(&self) -> &str {
        &self.0
    }

    pub fn normalized(&self) -> String {
        self.0.chars().map(to_lower).collect()
    }
}

impl ToOwned for ChanNameRef {
    type Owned = ChanName;

    fn to_owned(&self) -> Self::Owned {
        ChanName(self.0.to_owned())
    }
}

impl PartialEq for ChanName {
    fn eq(&self, other: &Self) -> bool {
        let self_borrowed: &ChanNameRef = self.borrow();
        let other_borrowed: &ChanNameRef = other.borrow();
        self_borrowed.eq(other_borrowed)
    }
}

impl Eq for ChanName {}

impl PartialEq<ChanNameRef> for ChanName {
    fn eq(&self, other: &ChanNameRef) -> bool {
        let self_borrowed: &ChanNameRef = self.borrow();
        self_borrowed.eq(other)
    }
}

impl Hash for ChanName {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let self_borrowed: &ChanNameRef = self.borrow();
        self_borrowed.hash(state)
    }
}

impl PartialEq for ChanNameRef {
    fn eq(&self, other: &Self) -> bool {
        // https://github.com/rust-lang/rust/blob/b4acb110333392ecdaf890fce080e4b576106aae/library/core/src/slice/mod.rs#L6678-L6684

        // All characters in ASCII have the same encoding length so we can compare byte lenghts.
        if self.0.as_bytes().len() != other.0.as_bytes().len() {
            return false;
        }

        self.0
            .chars()
            .map(to_lower)
            .zip(other.0.chars().map(to_lower))
            .all(|(a, b)| a == b)
    }
}

impl Eq for ChanNameRef {}

impl PartialEq<ChanName> for ChanNameRef {
    fn eq(&self, other: &ChanName) -> bool {
        let other_borrowed: &ChanNameRef = other.borrow();
        self.eq(other_borrowed)
    }
}

impl Hash for ChanNameRef {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // https://github.com/rust-lang/rust/blob/b4acb110333392ecdaf890fce080e4b576106aae/library/core/src/hash/mod.rs#L653-L656
        self.0.len().hash(state);
        for c in self.0.chars() {
            to_lower(c).hash(state);
        }
    }
}
