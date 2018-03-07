#![feature(test)]

extern crate ditto;
extern crate rand;
extern crate rmp_serde;
extern crate test;

use ditto::text::Text;
use rand::{Rng, ThreadRng};
use std::cmp::min;

#[bench]
fn benchmark_short_text(b: &mut test::Bencher) {
    let mut rng = rand::thread_rng();
    let words   = gen_words(1_000);
    b.iter(|| insert_words(&words, &mut rng))
}

#[bench]
fn benchmark_medium_text(b: &mut test::Bencher) {
    let mut rng = rand::thread_rng();
    let words   = gen_words(10_000);
    b.iter(|| insert_words(&words, &mut rng))
}

#[bench]
fn benchmark_long_text(b: &mut test::Bencher) {
    let mut rng = rand::thread_rng();
    let words   = gen_words(100_000);
    b.iter(|| insert_words(&words, &mut rng))
}

fn insert_words(words: &[String], mut rng: &mut ThreadRng) {
    let mut text = Text::new();
    let mut next_deletes = rng.gen_range(10,30);
    let mut idx = 0;

    for word in words.iter() {
        idx   = choose_index(rng, idx, text.len());
        let _ = text.replace(idx, 0, word).unwrap();
        idx  += word.len();
        next_deletes -= 1;

        if next_deletes == 0 {
            let mut deletes = rng.gen_range(5,15);
            while deletes > 0 && !text.is_empty() {
                idx     = choose_index(&mut rng, idx, text.len()).saturating_sub(1);
                let len = choose_len(&mut rng, idx, text.len());
                let _   = text.replace(idx, len, "").unwrap();
                deletes -= 1;
            }
            next_deletes = rng.gen_range(10,30);
        }
    }
}

fn gen_words(count: usize) -> Vec<String> {
    let mut rng = rand::thread_rng();
    (0..count).into_iter().map(|_| gen_word(&mut rng)).collect()
}

fn gen_word(rng: &mut ThreadRng) -> String {
    let len = match rng.gen_range(0, 100) {
       0 ... 89 => 1,
       number  => number - 88,
    };

   rng.gen_ascii_chars().take(len).collect()
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

fn choose_len(rng: &mut ThreadRng, idx: usize, text_len: usize) -> usize {
    match rng.gen_range(0, 100) {
        0 ... 94 => 1,
        n => min(n, text_len-idx),
    }
}
