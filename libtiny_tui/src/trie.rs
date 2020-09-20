pub struct Trie {
    vec: Vec<(char, Box<Trie>)>,
    word: bool,
}

impl Default for Trie {
    fn default() -> Self {
        Trie::new()
    }
}

impl Trie {
    pub fn new() -> Trie {
        Trie {
            vec: vec![],
            word: false,
        }
    }

    pub fn clear(&mut self) {
        self.vec.clear();
        self.word = false;
    }

    pub fn insert(&mut self, str: &str) {
        let mut node = self;
        for char in str.chars() {
            node = get_char_node_for_insert(node, char);
        }
        node.word = true;
    }

    #[cfg(test)]
    fn contains(&self, str: &str) -> bool {
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

    pub fn remove(&mut self, str: &str) {
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
    pub fn to_strings(&self, prefix: &str) -> Vec<String> {
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
    pub fn drop_pfx(&self, prefix: &mut dyn Iterator<Item = char>) -> Vec<String> {
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

fn get_char_node_for_insert(trie: &mut Trie, char: char) -> &mut Trie {
    match trie.vec.binary_search_by(|&(char_, _)| char_.cmp(&char)) {
        Ok(idx) => &mut trie.vec[idx].1,
        Err(idx) => {
            trie.vec.insert(
                idx,
                (
                    char,
                    Box::new(Trie {
                        vec: vec![],
                        word: false,
                    }),
                ),
            );
            &mut trie.vec[idx].1
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
