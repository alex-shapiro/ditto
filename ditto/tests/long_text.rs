extern crate ditto;
extern crate rand;
extern crate rmp_serde;

use ditto::Text;
use rand::{Rng, ThreadRng};

#[test]
fn test_long_text() {
    perform_ops(1_000);
    perform_ops(10_000);
    perform_ops(100_000);
}

fn perform_ops(insert_count: usize) {
    let mut rng    = rand::thread_rng();
    let mut string = String::new();
    let mut text   = Text::new();
    let mut index  = 0;

    for word in gen_words(insert_count) {
        index = choose_index(&mut rng, index, text.len());
        let word_len = word.len();

        string.insert_str(index, &word);
        let _ = text.insert(index, &word).unwrap();
        index += word_len;

        // every 20ish inserts, execute 10ish deletes
        if rng.gen_weighted_bool(20) {
            while rng.gen_weighted_bool(10) && text.len() > 0 {
                let upper = choose_index(&mut rng, index, text.len());
                let len   = if rng.gen_weighted_bool(20) { std::cmp::min(upper, 10) } else { 1 };
                index     = upper.saturating_sub(len);
                string    = str_remove(&string, index, len);
                let _     = text.remove(index, len).unwrap();
            }
        }
    }

    {
        let string_bytes = string.len();
        let crdt_bytes = rmp_serde::to_vec(&text).unwrap().len();
        let overhead = ((crdt_bytes as f64) * 10. / (string_bytes as f64)).round() / 10.;
        println!("overhead: {}x", overhead);
    }

    assert!(text.local_value() == string);
}

fn gen_words(count: usize) -> Vec<String> {
    let mut rng = rand::thread_rng();
    (0..count).into_iter().map(|_| gen_word(&mut rng)).collect()
}

fn gen_word(rng: &mut ThreadRng) -> String {
    let len = match rng.gen_range(0, 100) {
       0 => 0,
       1 ... 90 => 1,
       number => number - 88,
    };

    if len == 0 {
       String::from("\n")
    } else {
       rng.gen_ascii_chars().take(len).collect()
    }
}

// Probablistically chooses a new insert index.
// 96% of the time it uses the old index
//  2% of the time it chooses the end of the string
//  2% of the time it chooses a random string index.
fn choose_index(rng: &mut ThreadRng, old_index: usize, string_len: usize) -> usize {
    if string_len == 0 { return 0 }

    match rng.gen_range(0, 100) {
        0  ... 95 => old_index,
        96 ... 97 => string_len,
        98 ... 99 => rng.gen_range(0, string_len),
        _ => panic!("UNREACHABLE!!!"),
    }
}

fn str_remove(string: &str, index: usize, len: usize) -> String {
    let (part1, x) = string.split_at(index);
    let (_, part2) = x.split_at(len);
    [part1, part2].join("")
}
