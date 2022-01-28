use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{Read, stdin};
use std::time::Instant;

use rayon::prelude::*;

const FAST_THRESHOLD : usize = 1600;

fn main() {
    println!("enter word len");
    let word_len = read().parse::<usize>().unwrap();
    let mut words_file = File::open("words.txt").unwrap();
    let mut words_string = String::new();
    words_file.read_to_string(&mut words_string).unwrap();

    let mut known_placement_letters: Vec<Option<String>> = vec![None; word_len];
    let mut needed_letters : HashMap<char, HashSet<usize>> = HashMap::new();
    let mut avoid_letters : HashSet<char> = HashSet::new();

    let mut possible_words : Vec<&str> = words_string.lines().collect();
    let alt_word_list = words_string.lines()
        .filter(|word| init_filter(word, word_len))
        .collect::<Vec<&str>>();

    let mut first = true;

    loop {
        let start = Instant::now();

        possible_words = possible_words.par_iter()
            .filter(|&word| check_word(word, word_len, &known_placement_letters, &needed_letters, &avoid_letters))
            .copied()
            .collect::<Vec<&str>>();

        let mut possible_words = &possible_words;
        let mut fast = possible_words.len() > FAST_THRESHOLD;

        if first && fast {
            println!("using culled list: was {}, now: {}", possible_words.len(), alt_word_list.len());
            possible_words = &alt_word_list;
            fast = possible_words.len() > FAST_THRESHOLD;
        } else {
            println!("using full list");
        }

        println!("matches: {}", possible_words.len());

        if fast {
            println!("using fast scoring");
        } else {
            println!("using comprehensive scoring");
        }

        if fast {
            let mut scored_possible_words = possible_words.par_iter()
                .map(|&word| (word, score_word_fast(word, possible_words)))
                .collect::<Vec<(&str, (f64, u32, u32))>>();
            scored_possible_words.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

            for (word, score) in scored_possible_words.iter().take(10) {
                println!("{}, average non avoid: {}, best: {}, worst: {}", word, score.0, score.2, score.1);
            }
        } else {
            let mut scored_possible_words = possible_words.par_iter()
                .map(|&word| (word, score_word(word, possible_words, &known_placement_letters, &needed_letters, &avoid_letters)))
                .collect::<Vec<(&str, (f64, f64, f64))>>();
            scored_possible_words.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

            for (word, score) in scored_possible_words.iter().take(10) {
                println!("{}, average score: {}, best: {}, worst: {}", word, score.0, score.2, score.1);
            }
        }

        let time = start.elapsed();
        println!("computation took {} seconds", time.as_secs_f64());

        println!("enter new info in format place,needed,avoid (a??b?,?c???,d)");
        let input = read().to_lowercase();
        parse_input(&input, &mut known_placement_letters, &mut needed_letters, &mut avoid_letters);

        first = false;
    }
}

fn read() -> String {
    let mut input = String::new();
    stdin().read_line(&mut input).unwrap();
    input.trim_end().to_owned()
}

fn parse_input(input: &str, known_placement_letters: &mut Vec<Option<String>>, needed_letters : &mut HashMap<char, HashSet<usize>>, avoid_letters : &mut HashSet<char>) {
    for (mode, data) in input.split(',').enumerate() {
        for (idx, char) in data.chars().enumerate() {
            match mode {
                0 => {
                    if char != '?' {
                        known_placement_letters[idx] = Some(char.to_string());
                        needed_letters.retain(|letter, _| *letter != char);
                        avoid_letters.retain(|letter| *letter != char);
                    }
                }
                1 => {
                    if char != '?' {
                        match needed_letters.entry(char) {
                            Entry::Occupied(mut occupied) => {
                                occupied.get_mut().insert(idx);
                            }
                            Entry::Vacant(vacant) => {
                                vacant.insert(HashSet::from([idx]));
                            }
                        }
                        avoid_letters.retain(|letter| *letter != char);
                    }
                }
                2 => {
                    if !known_placement_letters.contains(&Some(char.to_string())) && !needed_letters.contains_key(&char) {
                        avoid_letters.insert(char);
                    }
                }
                _ => {
                    panic!("bad input")
                }
            }
        }
    }
}

const COMMON: &[char] = &['e', 't', 'o', 'a', 'i'];
const UNCOMMON: &[char] = &['q', 'z', 'x', 'j', 'v'];

fn init_filter(word: &str, word_len: usize) -> bool {
    if word.len() != word_len {
        return false;
    }

    if !word.contains(COMMON) || word.contains(UNCOMMON) {
        return false;
    }

    for char in word.chars() {
        if word.matches(char).count() > 1 {
            return false;
        }
    }

    true
}

fn check_word(word: &str, word_len: usize, known_placement_letters: &Vec<Option<String>>, needed_letters : &HashMap<char, HashSet<usize>>, avoid_letters : &HashSet<char>) -> bool {
    if word.len() != word_len {
        return false;
    }

   for needed_letter in needed_letters.keys() {
        if !word.contains(*needed_letter) {
            return false;
        }
    }

    for (idx, char) in word.chars().enumerate() {
        if let Some(Some(needed_char)) = known_placement_letters.get(idx) {
            if *needed_char != char.to_string() {
                return false;
            }
        }

        if let Some(avoid_idxs) = needed_letters.get(&char) {
            if avoid_idxs.contains(&idx) {
                return false;
            }
        }

        if avoid_letters.contains(&char) {
            return false;
        }
    }

    true
}

fn score_word(word: &str, words: &Vec<&str>, known_placement_letters: &Vec<Option<String>>, needed_letters : &HashMap<char, HashSet<usize>>, avoid_letters : &HashSet<char>) -> (f64, f64, f64) {
    let mut total_decrease = 0;
    let mut best_decrease = 0;
    let mut worst_decrease = 10000;

    for real_word in words.iter() {
        let mut known_placement_letters2 = known_placement_letters.clone();
        let mut needed_letters2 = needed_letters.clone();
        let mut avoid_letters2 = avoid_letters.clone();

        let mut chars2 = real_word.chars();
        for (idx, char) in word.chars().enumerate() {
            if char == chars2.next().unwrap() {
                known_placement_letters2[idx]= Some(char.to_string());
            } else if real_word.contains(char) {
                match needed_letters2.entry(char) {
                    Entry::Occupied(mut occupied) => {
                        occupied.get_mut().insert(idx);
                    }
                    Entry::Vacant(vacant) => {
                        vacant.insert(HashSet::from([idx]));
                    }
                }
            } else {
                avoid_letters2.insert(char);
            }
        }


        let new_possible_word_count = words.par_iter()
            .filter(|word| check_word(*word, word.len(), &known_placement_letters2, &needed_letters2, &avoid_letters2))
            .count();

        let decrease = words.len() - new_possible_word_count;

        total_decrease += decrease;

        if *real_word != word {
            best_decrease = best_decrease.max(decrease);
            worst_decrease = worst_decrease.min(decrease);
        }
    }

    let count = words.len() as f64;
    (count - total_decrease as f64 / count, count - worst_decrease as f64, count - best_decrease as f64)
}

fn score_word_fast(word: &str, words: &Vec<&str>) -> (f64, u32, u32) {
    // needed, avoid
    let mut non_avoid = 0;
    let mut best_non_avoid = 0;
    let mut worst_non_avoid = 10000;

    for real_word in words.iter() {
        let mut local_non_avoid = 0;

        for char in word.chars() {
            if real_word.contains(char) {
                local_non_avoid += 1;
            }
        }

        non_avoid += local_non_avoid;


        if *real_word != word {
            best_non_avoid = best_non_avoid.max(local_non_avoid);
            worst_non_avoid = worst_non_avoid.min(local_non_avoid);
        }
    }

    (non_avoid as f64 / words.len() as f64, worst_non_avoid, best_non_avoid)
}