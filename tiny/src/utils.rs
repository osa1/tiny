use std::str::SplitWhitespace;

/// Like `std::str::SplitWhitespace`, but returns beginning indices rather than slices.
pub(crate) struct SplitWhitespaceIndices<'a> {
    inner: SplitWhitespace<'a>,
    str: &'a str,
}

impl<'a> Iterator for SplitWhitespaceIndices<'a> {
    type Item = usize;

    fn next(&mut self) -> Option<usize> {
        self.inner
            .next()
            .map(|str| unsafe { str.as_ptr().offset_from(self.str.as_ptr()) as usize })
    }
}

pub(crate) fn split_whitespace_indices(str: &str) -> SplitWhitespaceIndices {
    SplitWhitespaceIndices {
        inner: str.split_whitespace(),
        str,
    }
}

////////////////////////////////////////////////////////////////////////////////

// RFC 2812:
//
// nickname   =  ( letter / special ) *8( letter / digit / special / "-" )
// letter     =  %x41-5A / %x61-7A ; A-Z / a-z
// special    =  %x5B-60 / %x7B-7D ; "[", "]", "\", "`", "_", "^", "{", "|", "}"
//
// we use a simpler check here (allows strictly more nicks)

pub(crate) fn is_nick_first_char(c: char) -> bool {
    c.is_alphabetic() || "[]\\`_^{|}".contains(c)
}

/*
pub(crate) fn is_nick_char(c: char) -> bool {
    c.is_alphanumeric() // 'letter' or 'digit'
        || (c as i32 >= 0x5B && c as i32 <= 0x60)
        || (c as i32 >= 0x7B && c as i32 <= 0x7D)
        || "[]\\`_^{|}-".contains(c)
}

pub(crate) fn is_chan_first_char(c: char) -> bool {
    // RFC 2812 section 1.3
    //
    // > Channels names are strings (beginning with a '&', '#', '+' or '!' character) of length up
    // > to fifty (50) characters.
    c == '#' || c == '&' || c == '+' || c == '!'
}
*/

////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_ws_idx() {
        let str = "x y z";
        let idxs: Vec<usize> = split_whitespace_indices(str).collect();
        assert_eq!(idxs, vec![0, 2, 4]);

        let str = "       ";
        let idxs: Vec<usize> = split_whitespace_indices(str).collect();
        let expected: Vec<usize> = vec![];
        // Next line fails when I inline `expected`. Probably a rustc bug.
        assert_eq!(idxs, expected);

        let str = "  foo    bar  \n\r   baz     ";
        let idxs: Vec<usize> = split_whitespace_indices(str).collect();
        assert_eq!(idxs, vec![2, 9, 19]);
    }
}
