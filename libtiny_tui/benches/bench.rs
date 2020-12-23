#[macro_use]
extern crate bencher;

use bencher::Bencher;
use libtiny_common::MsgTarget;
use libtiny_tui::tui::TUI;
use std::{fs::File, io::BufRead, io::BufReader, io::Read};
use time::Tm;

use libtiny_tui::trie::Trie;

static DICT_FILE: &str = "/usr/share/dict/american-english";

fn trie_build(b: &mut Bencher) {
    // Total words: 305,089
    // 117,701,680 ns (0.1 seconds)
    // (before reversing the list: 116,795,268 ns (0.1 seconds))

    let mut contents = String::new();
    let mut words: Vec<&str> = vec![];
    {
        match File::open(DICT_FILE) {
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

fn hashset_build(b: &mut Bencher) {
    // Total words: 305,089
    // 40,292,006 ns (0.04 seconds)

    use std::collections::HashSet;

    let mut contents = String::new();
    let mut words: Vec<&str> = vec![];
    {
        let mut file = File::open(DICT_FILE).unwrap();
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

fn trie_lookup(b: &mut Bencher) {
    // Total:     305,089 words
    // Returning:     235 words
    // 140,717 ns (0.14 ms)

    let mut contents = String::new();
    let mut words: Vec<&str> = vec![];
    {
        match File::open(DICT_FILE) {
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

fn trie_list_all(b: &mut Bencher) {
    // Total:     305,089 words
    // Returning: 305,089 words
    // 205,946,060 ns (0.2 s)

    let mut contents = String::new();
    let mut words: Vec<&str> = vec![];
    {
        match File::open(DICT_FILE) {
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

fn tui_resize(b: &mut Bencher) {
    let mut tui = TUI::new_test(80, 50);

    let server = "<server>";
    tui.new_server_tab(server, None);

    let ts: Tm = time::empty_tm();
    let target = MsgTarget::CurrentTab;

    let f = File::open("test/lipsum.txt").unwrap();
    let f = BufReader::new(f);
    for line in f.lines() {
        let line = line.unwrap();
        tui.add_msg(&line, ts, &target);
    }

    b.iter(|| {
        let mut w = 80;
        let mut h = 50;

        for _ in 0..50 {
            w -= 1;
            h -= 1;
            tui.set_size(w, h);
            tui.draw();
        }

        for _ in 0..50 {
            w += 1;
            h += 1;
            tui.set_size(w, h);
            tui.draw();
        }
    });
}

benchmark_group!(
    benches,
    trie_build,
    hashset_build,
    trie_lookup,
    trie_list_all,
    tui_resize
);
benchmark_main!(benches);
