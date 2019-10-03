pub(crate) struct Trie {
    vec: Vec<(char, Box<Trie>)>,
    word: bool,
}

impl Trie {
    pub(crate) fn new() -> Trie {
        Trie {
            vec: vec![],
            word: false,
        }
    }

    pub(crate) fn clear(&mut self) {
        self.vec.clear();
        self.word = false;
    }

    pub(crate) fn insert(&mut self, str: &str) {
        let mut trie_ptr: *mut Trie = &mut *self;
        for char in str.chars() {
            trie_ptr = get_char_node_for_insert(trie_ptr, char);
        }

        unsafe {
            (*trie_ptr).word = true;
        }
    }

    pub(crate) fn contains(&self, str: &str) -> bool {
        let mut trie = self;
        for char in str.chars() {
            if let Some(trie_) = get_char_node_for_lookup(trie, char) {
                trie = trie_;
            } else {
                return false;
            }
        }
        trie.word
    }

    pub(crate) fn remove(&mut self, str: &str) {
        let mut chars = str.chars();
        if let Some(char) = chars.next() {
            if let Ok(idx) = self.vec.binary_search_by(|&(char_, _)| char_.cmp(&char)) {
                let del = {
                    let trie = &mut self.vec[idx].1;
                    trie.remove(chars.as_str());
                    !trie.word && trie.vec.is_empty()
                };
                if del {
                    self.vec.remove(idx);
                };
            }
        } else {
            self.word = false;
        }
    }

    // TODO: We need an Iterator instance instead.
    pub(crate) fn to_strings(&self, prefix: &str) -> Vec<String> {
        let mut ret = {
            if self.word {
                vec![prefix.to_owned()]
            } else {
                vec![]
            }
        };

        for &(c, ref t) in &self.vec {
            let mut prefix_ = prefix.to_owned();
            prefix_.push(c);
            ret.extend(t.to_strings(&prefix_));
        }

        ret
    }

    // TODO: We need an Iterator instance instead.
    pub(crate) fn drop_pfx(&self, prefix: &mut dyn Iterator<Item = char>) -> Vec<String> {
        let mut trie = self;
        for char in prefix {
            if let Some(trie_) = get_char_node_for_lookup(trie, char) {
                trie = trie_;
            } else {
                return vec![];
            }
        }
        trie.to_strings("")
    }
}

fn get_char_node_for_insert(trie: *mut Trie, char: char) -> *mut Trie {
    let trie_ref: &mut Trie = unsafe { &mut *trie };
    match trie_ref
        .vec
        .binary_search_by(|&(char_, _)| char_.cmp(&char))
    {
        Ok(idx) => &mut *trie_ref.vec[idx].1,
        Err(idx) => {
            trie_ref.vec.insert(
                idx,
                (
                    char,
                    Box::new(Trie {
                        vec: vec![],
                        word: false,
                    }),
                ),
            );
            &mut *trie_ref.vec[idx].1
        }
    }
}

fn get_char_node_for_lookup(trie: &Trie, char: char) -> Option<&Trie> {
    match trie.vec.binary_search_by(|&(char_, _)| char_.cmp(&char)) {
        Ok(idx) => Some(&trie.vec[idx].1),
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn trie_test_1() {
        let mut trie = Trie::new();
        trie.insert("yada yada");
        assert_eq!(vec!["yada yada"], trie.to_strings(""));
    }

    #[test]
    fn trie_test_2() {
        let mut trie = Trie::new();
        trie.insert("foo");
        assert!(trie.contains("foo"));
        trie.insert("bar");
        assert!(trie.contains("foo"));
        assert!(trie.contains("bar"));
        trie.insert("baz");
        assert!(trie.contains("foo"));
        assert!(trie.contains("bar"));
        assert!(trie.contains("baz"));
        assert_eq!(vec!["bar", "baz", "foo"], trie.to_strings(""));
    }

    #[test]
    fn trie_test_3() {
        let mut trie = Trie::new();
        trie.insert("foo");
        trie.insert("bar");
        trie.insert("baz");
        assert_eq!(vec!["ar", "az"], trie.drop_pfx(&mut "b".chars()));
    }

    #[test]
    fn trie_test_insert_remove() {
        let mut trie = Trie::new();
        trie.insert("foo");
        trie.insert("bar");
        trie.insert("baz");
        trie.remove("bar");
        assert!(trie.contains("foo"));
        assert!(!trie.contains("bar"));
        assert!(trie.contains("baz"));
        assert_eq!(vec!["az"], trie.drop_pfx(&mut "b".chars()));
    }
} // tests

#[cfg(test)]
mod benchs {

    extern crate test;

    use test::Bencher;
    use super::*;
    use std::{fs::File, io::Read};

    #[bench]
    fn bench_trie_build(b: &mut Bencher) {
        // Total words: 305,089
        // 117,701,680 ns (0.1 seconds)
        // (before reversing the list: 116,795,268 ns (0.1 seconds))

        let mut contents = String::new();
        let mut words: Vec<&str> = vec![];
        {
            match File::open("/usr/share/dict/american") {
                Err(_) => {
                    println!("Can't open dictionary file, aborting benchmark.");
                    return;
                }
                Ok(mut file) => {
                    file.read_to_string(&mut contents).unwrap();
                    words.extend(contents.lines());
                }
            }
        }

        b.iter(|| {
            let mut trie = Trie::new();
            // Note that we insert the words in reverse order here. Since the
            // dictionary is already sorted, we end up benchmarking the best case.
            // Since that best case is never really happens in practice, the number
            // is practically useless. Worst case is at least giving an upper bound.
            for word in words.iter().rev() {
                trie.insert(word);
            }
            trie
        });
    }

    /*
    #[bench]
    fn bench_hashset_build(b : &mut Bencher) {

        // Total words: 305,089
        // 40,292,006 ns (0.04 seconds)

        use std::collections::HashSet;

        let mut contents = String::new();
        let mut words : Vec<&str> = vec![];
        {
            let mut file = File::open("/usr/share/dict/american").unwrap();
            file.read_to_string(&mut contents).unwrap();
            words.extend(contents.lines());
        }

        b.iter(|| {
            let mut set = HashSet::new();
            for word in words.iter() {
                set.insert(word);
            }
            set
        });
    }
    */

    #[bench]
    fn bench_trie_lookup(b: &mut Bencher) {
        // Total:     305,089 words
        // Returning:     235 words
        // 140,717 ns (0.14 ms)

        let mut contents = String::new();
        let mut words: Vec<&str> = vec![];
        {
            match File::open("/usr/share/dict/american") {
                Err(_) => {
                    println!("Can't open dictionary file, aborting benchmark.");
                    return;
                }
                Ok(mut file) => {
                    file.read_to_string(&mut contents).unwrap();
                    words.extend(contents.lines());
                }
            }
        }

        let mut trie = Trie::new();
        for word in words {
            trie.insert(word);
        }

        b.iter(|| trie.drop_pfx(&mut "abs".chars()));
    }

    #[bench]
    fn bench_trie_list_all(b: &mut Bencher) {
        // Total:     305,089 words
        // Returning: 305,089 words
        // 205,946,060 ns (0.2 s)

        let mut contents = String::new();
        let mut words: Vec<&str> = vec![];
        {
            match File::open("/usr/share/dict/american") {
                Err(_) => {
                    println!("Can't open dictionary file, aborting benchmark.");
                    return;
                }
                Ok(mut file) => {
                    file.read_to_string(&mut contents).unwrap();
                    words.extend(contents.lines());
                }
            }
        }

        let mut trie = Trie::new();
        for word in words {
            trie.insert(word);
        }

        b.iter(|| trie.drop_pfx(&mut "".chars()));
    }
} // benchs
