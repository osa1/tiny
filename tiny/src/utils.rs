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

pub(crate) fn is_nick_char(c: char) -> bool {
    // from RFC 2812:
    //
    // nickname   =  ( letter / special ) *8( letter / digit / special / "-" )
    // special    =  %x5B-60 / %x7B-7D
    //                  ; "[", "]", "\", "`", "_", "^", "{", "|", "}"
    //
    // we use a simpler check here (allows strictly more nicks)

    c.is_alphanumeric()
        || (c as i32 >= 0x5B && c as i32 <= 0x60)
        || (c as i32 >= 0x7B && c as i32 <= 0x7D)
        || c == '-' // not valid according to RFC 2812 but servers accept it and I've seen nicks with
                    // this char in the wild
}

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
