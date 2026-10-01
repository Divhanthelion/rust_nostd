//! Module quizzes.
//!
//! Format (one question per block, blocks separated by blank lines):
//!
//! ```text
//! ? Question text (may span several `?` lines)
//! | code line shown under the question
//! - a wrong answer
//! * the right answer
//! > explanation shown after answering (may span several `>` lines)
//! ```

use crate::markdown;
use crate::term;

pub const PASS_PERCENT: usize = 80;

pub struct Question {
    pub prompt: String,
    pub code: Vec<String>,
    pub options: Vec<(String, bool)>,
    pub explanation: String,
}

pub fn parse(src: &str) -> Result<Vec<Question>, String> {
    let mut qs = Vec::new();
    let mut cur: Option<Question> = None;
    let finish = |q: Question, qs: &mut Vec<Question>| -> Result<(), String> {
        let right = q.options.iter().filter(|(_, c)| *c).count();
        if right != 1 {
            return Err(format!("question {:?} has {right} correct answers (need exactly 1)", q.prompt));
        }
        if q.options.len() < 2 {
            return Err(format!("question {:?} needs at least two options", q.prompt));
        }
        qs.push(q);
        Ok(())
    };
    for (lineno, raw) in src.lines().enumerate() {
        let line = raw.trim_end();
        if line.starts_with('#') {
            continue;
        }
        if line.is_empty() {
            if let Some(q) = cur.take() {
                finish(q, &mut qs)?;
            }
            continue;
        }
        let (tag, rest) = line.split_at(1);
        let rest = rest.strip_prefix(' ').unwrap_or(rest);
        match tag {
            "?" => {
                let q = cur.get_or_insert_with(|| Question {
                    prompt: String::new(),
                    code: Vec::new(),
                    options: Vec::new(),
                    explanation: String::new(),
                });
                if !q.prompt.is_empty() {
                    q.prompt.push(' ');
                }
                q.prompt.push_str(rest);
            }
            "|" => cur
                .as_mut()
                .ok_or(format!("line {}: code before question", lineno + 1))?
                .code
                .push(rest.to_string()),
            "-" | "*" => cur
                .as_mut()
                .ok_or(format!("line {}: option before question", lineno + 1))?
                .options
                .push((rest.to_string(), tag == "*")),
            ">" => {
                let q = cur.as_mut().ok_or(format!("line {}: explanation before question", lineno + 1))?;
                if !q.explanation.is_empty() {
                    q.explanation.push(' ');
                }
                q.explanation.push_str(rest);
            }
            _ => return Err(format!("line {}: unexpected {:?}", lineno + 1, line)),
        }
    }
    if let Some(q) = cur.take() {
        finish(q, &mut qs)?;
    }
    Ok(qs)
}

/// Tiny xorshift PRNG so option order varies between attempts.
pub struct Rng(u64);

impl Rng {
    pub fn seeded() -> Rng {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E37_79B9_7F4A_7C15);
        Rng(t | 1)
    }
    pub fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    pub fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let j = (self.next() % (i as u64 + 1)) as usize;
            v.swap(i, j);
        }
    }
}

/// Run a quiz interactively. Returns Some((correct, total)) or None if aborted.
pub fn run(title: &str, questions: &[Question]) -> Option<(usize, usize)> {
    let width = term::text_width();
    let mut rng = Rng::seeded();
    let total = questions.len();
    println!();
    println!("  {} {}", term::bold_magenta("QUIZ"), term::bold(title));
    println!(
        "  {}",
        term::dim(&format!("{total} questions · pass mark {PASS_PERCENT}% · answer with a letter, `q` to quit"))
    );
    println!();
    let mut correct = 0;
    for (i, q) in questions.iter().enumerate() {
        let header = format!("Q{}/{}", i + 1, total);
        let prompt = markdown::inline(&q.prompt);
        let first = format!("  {} ", term::bold_cyan(&header));
        print!("{first}");
        let wrapped = term::wrap_plain(&term::strip_ansi(&prompt), width.saturating_sub(header.len() + 3), "");
        if term::color_enabled() {
            // keep inline styling when it fits on one line
            if wrapped.lines().count() == 1 {
                println!("{prompt}");
            } else {
                let mut lines = wrapped.lines();
                println!("{}", lines.next().unwrap_or(""));
                for l in lines {
                    println!("  {}{l}", " ".repeat(header.len() + 1));
                }
            }
        } else {
            let mut lines = wrapped.lines();
            println!("{}", lines.next().unwrap_or(""));
            for l in lines {
                println!("  {}{l}", " ".repeat(header.len() + 1));
            }
        }
        if !q.code.is_empty() {
            let code = q.code.join("\n");
            print!("{}", markdown::render_code("rust", &code, width));
        }
        let mut order: Vec<usize> = (0..q.options.len()).collect();
        rng.shuffle(&mut order);
        let letters = "abcdefgh";
        for (k, &oi) in order.iter().enumerate() {
            let letter = &letters[k..k + 1];
            let first = format!("     {} ", term::bold(&format!("{letter})")));
            print!("{}", markdown::wrap(&q.options[oi].0, width, &first, "        "));
        }
        let right_k = order.iter().position(|&oi| q.options[oi].1).unwrap_or(0);
        let answer = loop {
            let Some(a) = term::prompt(&format!("  {} ", term::dim("answer ›"))) else {
                return None;
            };
            let a = a.to_ascii_lowercase();
            if a == "q" || a == "quit" {
                return None;
            }
            if a.len() == 1 && letters[..order.len()].contains(&a) {
                break letters.find(&a).unwrap_or(0);
            }
            println!("  {}", term::dim(&format!("type a letter between a and {}", &letters[order.len() - 1..order.len()])));
        };
        if answer == right_k {
            correct += 1;
            println!("  {} {}", term::bold_green("✓ correct."), "");
        } else {
            let letter = &letters[right_k..right_k + 1];
            println!(
                "  {} the answer is {}) {}",
                term::bold_red("✗ not quite:"),
                letter,
                markdown::inline(&q.options[order[right_k]].0)
            );
        }
        if !q.explanation.is_empty() {
            let text = term::wrap_plain(&term::strip_ansi(&markdown::inline(&q.explanation)), width - 4, "    ");
            print!("{}", term::dim(&text));
        }
        println!();
    }
    Some((correct, total))
}
