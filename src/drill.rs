//! Interview-style flashcards with a simple Leitner schedule.
//!
//! Format of `curriculum/drills.txt`:
//!
//! ```text
//! ## topic
//! Q: question (one or more lines)
//! A: answer (one or more lines; continuation lines are indented)
//! ```

use crate::markdown;
use crate::quiz::Rng;
use crate::state::State;
use crate::term;

pub struct Card {
    pub id: String,
    pub topic: String,
    pub question: String,
    pub answer: String,
}

pub fn parse(src: &str) -> Result<Vec<Card>, String> {
    let mut cards = Vec::new();
    let mut topic = String::from("general");
    let mut q = String::new();
    let mut a = String::new();
    let mut in_answer = false;
    let push = |q: &mut String, a: &mut String, topic: &str, cards: &mut Vec<Card>| -> Result<(), String> {
        if q.is_empty() && a.is_empty() {
            return Ok(());
        }
        if q.is_empty() || a.is_empty() {
            return Err(format!("incomplete card near {:?}", if q.is_empty() { &*a } else { &*q }));
        }
        cards.push(Card { id: card_id(q), topic: topic.to_string(), question: q.trim().to_string(), answer: a.trim_end().to_string() });
        q.clear();
        a.clear();
        Ok(())
    };
    for line in src.lines() {
        if let Some(t) = line.strip_prefix("## ") {
            push(&mut q, &mut a, &topic, &mut cards)?;
            topic = t.trim().to_string();
            in_answer = false;
        } else if let Some(rest) = line.strip_prefix("Q:") {
            push(&mut q, &mut a, &topic, &mut cards)?;
            q.push_str(rest.trim());
            in_answer = false;
        } else if let Some(rest) = line.strip_prefix("A:") {
            a.push_str(rest.trim());
            a.push('\n');
            in_answer = true;
        } else if line.trim().is_empty() {
            if in_answer {
                a.push('\n');
            }
        } else if in_answer {
            a.push_str(line.strip_prefix("   ").unwrap_or(line));
            a.push('\n');
        } else if !q.is_empty() {
            q.push(' ');
            q.push_str(line.trim());
        } else if !line.starts_with('#') {
            return Err(format!("unexpected line {line:?}"));
        }
    }
    push(&mut q, &mut a, &topic, &mut cards)?;
    Ok(cards)
}

fn card_id(q: &str) -> String {
    // FNV-1a, stable across releases as long as the question text is unchanged.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in q.trim().bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{:08x}", h >> 32)
}

pub fn topics(cards: &[Card]) -> Vec<String> {
    let mut t: Vec<String> = Vec::new();
    for c in cards {
        if !t.contains(&c.topic) {
            t.push(c.topic.clone());
        }
    }
    t
}

/// Run a session of `n` cards. Lower Leitner boxes come up more often.
pub fn run(cards: &[Card], st: &mut State, n: usize, topic: Option<&str>) {
    let pool: Vec<&Card> = cards
        .iter()
        .filter(|c| topic.is_none_or(|t| c.topic.to_ascii_lowercase().contains(&t.to_ascii_lowercase())))
        .collect();
    if pool.is_empty() {
        println!("  no cards match that topic. Topics: {}", topics(cards).join(", "));
        return;
    }
    let mut rng = Rng::seeded();
    // Weighted pick without replacement: weight = 2^(5 - box).
    let mut chosen: Vec<&Card> = Vec::new();
    let mut remaining = pool.clone();
    while chosen.len() < n && !remaining.is_empty() {
        let weights: Vec<u64> = remaining
            .iter()
            .map(|c| 1u64 << (5 - st.drill.get(&c.id).copied().unwrap_or(1).clamp(1, 5)))
            .collect();
        let total: u64 = weights.iter().sum();
        let mut r = rng.next() % total;
        let mut idx = 0;
        for (i, w) in weights.iter().enumerate() {
            if r < *w {
                idx = i;
                break;
            }
            r -= w;
        }
        chosen.push(remaining.swap_remove(idx));
    }
    let width = term::text_width();
    println!();
    println!(
        "  {} {}",
        term::bold_magenta("DRILL"),
        term::dim(&format!("{} cards · answer out loud or on paper, then reveal · y = knew it, n = review again, q = stop", chosen.len()))
    );
    let mut knew = 0;
    let mut seen = 0;
    for (i, c) in chosen.iter().enumerate() {
        println!();
        let bx = st.drill.get(&c.id).copied().unwrap_or(1);
        println!("  {} {} {}", term::bold_cyan(&format!("{}/{}", i + 1, chosen.len())), term::dim(&format!("[{}]", c.topic)), term::dim(&format!("box {bx}")));
        let q = term::wrap_plain(&term::strip_ansi(&markdown::inline(&c.question)), width - 4, "  ");
        print!("{}", term::bold(&q));
        let Some(cmd) = term::prompt(&format!("  {} ", term::dim("[enter] reveal ›"))) else { break };
        if cmd == "q" {
            break;
        }
        seen += 1;
        print!("{}", markdown::render(&c.answer, width - 2));
        let verdict = loop {
            let Some(v) = term::prompt(&format!("  {} ", term::dim("knew it? [y/n/q] ›"))) else { return finish(st, knew, seen - 1) };
            match v.as_str() {
                "y" | "Y" | "yes" => break Some(true),
                "n" | "N" | "no" => break Some(false),
                "q" => break None,
                _ => {}
            }
        };
        match verdict {
            Some(true) => {
                knew += 1;
                st.drill.insert(c.id.clone(), (bx + 1).min(5));
            }
            Some(false) => {
                st.drill.insert(c.id.clone(), 1);
            }
            None => return finish(st, knew, seen - 1),
        }
    }
    finish(st, knew, seen);
}

fn finish(st: &mut State, knew: usize, seen: usize) {
    let _ = st.save();
    println!();
    println!("  {} knew {knew} of {seen}. Cards you missed will come back sooner.", term::bold("Session done:"));
}
