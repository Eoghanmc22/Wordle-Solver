use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{Read, stdin};
use std::time::Instant;

use rayon::prelude::*;
use time::UtcOffset;

const FAST_THRESHOLD : usize = 1600;

fn main() {
    println!("enter word list");
    let list = format!("{}.txt", read());

    let mut words_file = File::open(list.clone()).unwrap();
    let mut words_string = String::new();
    words_file.read_to_string(&mut words_string).unwrap();
    let mut possible_answers = words_string.lines().collect::<Vec<&str>>();
    let mut possible_guesses = words_string.lines().collect::<Vec<&str>>();

    if list == "wordle.txt" {
        println!("would you like to include possible guesses");
        if read().parse::<bool>().unwrap() {
            let mut guesses_file = File::open("wordle_guesses.txt").unwrap();
            let mut guesses_string = String::new();
            guesses_file.read_to_string(&mut guesses_string).unwrap();
            possible_guesses.extend(guesses_string.lines());
        }

        println!("would you like to calculate the answer based on the current time (true/false)");
        if read().parse::<bool>().unwrap() {
            let wordle_start = time::Date::from_calendar_date(2021, time::Month::June, 19).unwrap();
            let now = time::OffsetDateTime::now_utc().to_offset(UtcOffset::from_hms(-5, 0, 0).unwrap()).date();

            println!("enter day offset");
            let day_offset = read().parse::<f64>().unwrap();

            let difference = now - wordle_start;
            let word_idx = ((difference.whole_milliseconds() as f64 / 86400000.0).round() + day_offset) as usize;

            println!("The word should be {}", possible_answers[word_idx % possible_answers.len()]);

            return;
        }
    }

    println!("enter word len");
    let word_len = read().parse::<usize>().unwrap();

    let mut context = Context { know_placements: vec![None; word_len], letter_data: HashMap::new() };

    let alt_word_list = words_string.lines()
        .filter(|word| init_filter(word, word_len))
        .collect::<Vec<&str>>();

    let mut first = true;

    println!("Skip first");
    if read().parse::<bool>().unwrap() {
        println!("enter new info in format place,needed,avoid (a??b?,?c???,d)");
        let input = read().to_lowercase();
        parse_input(&input, &mut context);

        first = false;
    }

    loop {
        let start = Instant::now();

        possible_answers = possible_answers.par_iter()
            .filter(|&word| check_word(word, word_len, &context))
            .copied()
            .collect::<Vec<&str>>();

        let mut possible_words = &possible_answers;
        let mut fast = false; //possible_words.len() > FAST_THRESHOLD;

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
            scored_possible_words.dedup_by(|a, b| a.1.0 == b.1.0);

            for (word, score) in scored_possible_words.iter().take(10) {
                println!("{}, average non avoid: {}, best: {}, worst: {}", word, score.0, score.2, score.1);
            }
        } else {
            let mut scored_possible_words = possible_words.par_iter()
                .map(|&word| (word, score_word(word, possible_words, &context)))
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
        parse_input(&input, &mut context);

        first = false;
    }
}

fn read() -> String {
    let mut input = String::new();
    stdin().read_line(&mut input).unwrap();
    input.trim_end().to_owned()
}

#[derive(Clone, Debug)]
struct Context {
    know_placements: Vec<Option<String>>,
    // map from char to (set of indices to avoid, min occurrences, max occurrences)
    letter_data : HashMap<char, (HashSet<usize>, u32, Option<u32>)>,
}

fn parse_input(input: &str, ctx: &mut Context) {
    let mut seen = HashMap::new();
    ctx.know_placements.iter()
        .filter_map(|entry| entry.as_ref())
        .for_each(|entry| *seen.entry(entry.chars().next().unwrap()).or_insert(0) += 1);

    for (mode, data) in input.split(',').enumerate() {
        for (idx, char) in data.chars().enumerate() {
            match mode {
                0 => {
                    if char != '?' {
                        ctx.know_placements.push(Some(char.to_string()));
                        if ctx.know_placements.swap_remove(idx).is_none() {
                            *seen.entry(char).or_insert(0) += 1;
                        }
                    }
                }
                1 => {
                    if char != '?' {
                        let count = seen.entry(char).or_insert(0);
                        *count += 1;

                        match ctx.letter_data.entry(char) {
                            Entry::Occupied(mut occupied) => {
                                let val = occupied.get_mut();
                                val.0.insert(idx);
                                val.1 = val.1.max(*count);
                                if let Some(max) = &mut val.2 {
                                    *max = (*max).max(*count);
                                }
                            }
                            Entry::Vacant(vacant) => {
                                vacant.insert((HashSet::from([idx]), *count, None));
                            }
                        }
                    }
                }
                2 => {
                    let count = *seen.entry(char).or_insert(0);

                    match ctx.letter_data.entry(char) {
                        Entry::Occupied(mut occupied) => {
                            let val = occupied.get_mut();

                            if let Some(max) = &mut val.2 {
                                *max = (*max).max(count);
                            } else {
                                val.2 = Some(count);
                            }
                        }
                        Entry::Vacant(vacant) => {
                            vacant.insert((HashSet::new(), count, Some(count)));
                        }
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

fn check_word(word: &str, word_len: usize, ctx: &Context) -> bool {
    if word.len() != word_len {
        return false;
    }

    let mut seen = HashMap::new();

    for (idx, char) in word.chars().enumerate() {
        if let Some(Some(needed_char)) = ctx.know_placements.get(idx) {
            if *needed_char != char.to_string() {
                return false;
            }
        }

        if let Some((avoid_idxs, _, max_count)) = ctx.letter_data.get(&char) {
            if avoid_idxs.contains(&idx) {
                return false;
            }

            if let Some(0) = max_count {
                return false;
            }
        }

        *seen.entry(char).or_insert(0) += 1;
    }

    for (letter, (_, min_count, max_count)) in ctx.letter_data.iter() {
        let actual_count = seen.get(letter).unwrap_or(&0);

        if actual_count < min_count {
            return false;
        }

        if let Some(max_count) = max_count {
            if actual_count > max_count {
                return false;
            }
        }
    }

    true
}

fn score_word(word: &str, words: &Vec<&str>, ctx: &Context) -> (f64, f64, f64) {
    let mut total_decrease = 0;
    let mut best_decrease = 0;
    let mut worst_decrease = 10000;

    for real_word in words.iter() {
        let mut alternate_ctx = ctx.clone();

        let mut seen = HashMap::new();

        let mut chars2 = real_word.chars();
        for (idx, char) in word.chars().enumerate() {
            if char == chars2.next().unwrap() {
                *seen.entry(char).or_insert(0) += 1;

                alternate_ctx.know_placements[idx] = Some(char.to_string());
            } else {
                let count = seen.entry(char).or_insert(0);

                if real_word.matches(char).count() as u32 > *count {
                    *count += 1;

                    match alternate_ctx.letter_data.entry(char) {
                        Entry::Occupied(mut occupied) => {
                            let val = occupied.get_mut();
                            val.0.insert(idx);
                            val.1 = val.1.max(*count);
                            if let Some(max) = &mut val.2 {
                                *max = (*max).max(*count);
                            }
                        }
                        Entry::Vacant(vacant) => {
                            vacant.insert((HashSet::from([idx]), *count, None));
                        }
                    }
                } else {
                    let count = *count;

                    match alternate_ctx.letter_data.entry(char) {
                        Entry::Occupied(mut occupied) => {
                            let val = occupied.get_mut();

                            if let Some(max) = &mut val.2 {
                                *max = (*max).max(count);
                            } else {
                                val.2 = Some(count);
                            }
                        }
                        Entry::Vacant(vacant) => {
                            vacant.insert((HashSet::new(), count, Some(count)));
                        }
                    }
                }
            }
        }

        let new_possible_word_count = words.par_iter()
            .filter(|word| check_word(*word, word.len(), &alternate_ctx))
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