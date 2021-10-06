pub(crate) struct InsertIterator<'iter, A: 'iter> {
    insert_point: usize,
    current_idx: usize,
    iter_orig: &'iter mut dyn Iterator<Item = A>,
    iter_insert: &'iter mut dyn Iterator<Item = A>,
}

impl<'iter, A> Iterator for InsertIterator<'iter, A> {
    type Item = A;

    fn next(&mut self) -> Option<A> {
        if self.current_idx >= self.insert_point {
            if let Some(a) = self.iter_insert.next() {
                Some(a)
            } else {
                self.iter_orig.next()
            }
        } else {
            self.current_idx += 1;
            self.iter_orig.next()
        }
    }
}

pub(crate) fn insert_iter<'iter, A>(
    iter_orig: &'iter mut dyn Iterator<Item = A>,
    iter_insert: &'iter mut dyn Iterator<Item = A>,
    insert_point: usize,
) -> InsertIterator<'iter, A> {
    InsertIterator {
        insert_point,
        current_idx: 0,
        iter_orig,
        iter_insert,
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
