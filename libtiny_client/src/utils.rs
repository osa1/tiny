pub(crate) struct SplitIterator<'a> {
    s: Option<&'a str>,
    max: usize,
}

/// Iterate over subslices that are at most `max` long (in bytes). Splits are
/// made on whitespace characters when possible.
pub(crate) fn split_iterator(s: &str, max: usize) -> SplitIterator {
    SplitIterator { s: Some(s), max }
}

impl<'a> Iterator for SplitIterator<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        if self.max == 0 {
            return None;
        }

        match self.s {
            None => None,
            Some(s) => {
                if s.len() <= self.max {
                    let ret = Some(s);
                    self.s = None;
                    ret
                } else {
                    let mut split = 0;

                    // try to split at a whitespace character
                    for (ws_idx, ws_char) in s.rmatch_indices(char::is_whitespace) {
                        if ws_idx <= self.max {
                            // should we include ws char?
                            if ws_idx + ws_char.len() <= self.max {
                                split = ws_idx + ws_char.len();
                            } else {
                                split = ws_idx;
                            }
                            break;
                        }
                    }

                    if split == 0 {
                        // couldn't split at a whitespace, just split at any char
                        for i in 0..4 {
                            if s.is_char_boundary(self.max - i) {
                                split = self.max - i;
                                break;
                            }
                        }
                    }

                    if split == 0 {
                        panic!("Can't split long msg: {:?}", s);
                    } else {
                        let ret = Some(&s[0..split]);
                        self.s = Some(&s[split..]);
                        ret
                    }
                }
            }
        }
    }
}

pub(crate) fn find_idx<A, F: Fn(&A) -> bool>(slice: &[A], f: F) -> Option<usize> {
    for (idx, a) in slice.iter().enumerate() {
        if f(a) {
            return Some(idx);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_iterator_1() {
        let iter = split_iterator("yada yada yada", 5);
        assert_eq!(iter.collect::<Vec<&str>>(), vec!["yada ", "yada ", "yada"]);
    }

    #[test]
    fn test_split_iterator_2() {
        let iter = split_iterator("yada yada yada", 4);
        assert_eq!(
            iter.collect::<Vec<&str>>(),
            // weird but OK
            vec!["yada", " ", "yada", " ", "yada"]
        );
    }

    #[test]
    fn test_split_iterator_3() {
        let iter = split_iterator("yada yada yada", 3);
        assert_eq!(
            iter.collect::<Vec<&str>>(),
            vec!["yad", "a ", "yad", "a ", "yad", "a"]
        );
    }

    #[test]
    fn test_split_iterator_4() {
        let iter = split_iterator("longwordislong", 3);
        assert_eq!(
            iter.collect::<Vec<&str>>(),
            vec!["lon", "gwo", "rdi", "slo", "ng"]
        );
    }

    #[test]
    fn test_split_iterator_5() {
        let iter = split_iterator("", 3);
        assert_eq!(iter.collect::<Vec<&str>>(), vec![""]);
    }

    #[test]
    fn test_split_iterator_6() {
        let iter = split_iterator("", 0);
        let ret: Vec<&str> = vec![];
        assert_eq!(iter.collect::<Vec<&str>>(), ret);
    }
}
