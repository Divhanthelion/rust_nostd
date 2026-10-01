//! A small Markdown-to-terminal renderer, tuned for the lesson files.
//!
//! Supported: ATX headings, paragraphs, bullet/numbered lists (nested by
//! indentation), fenced code blocks (Rust gets syntax highlighting),
//! blockquote callouts, pipe tables, horizontal rules and inline
//! `code`, **bold**, *italic* and [links](url).

use crate::term::{self, paint, visible_width};

#[derive(Clone, Copy, PartialEq)]
enum Sty {
    Plain,
    Bold,
    Italic,
    BoldItalic,
    Code,
    Link,
    Dim,
}

fn style(s: &str, st: Sty) -> String {
    match st {
        Sty::Plain => s.to_string(),
        Sty::Bold => paint(s, "1"),
        Sty::Italic => paint(s, "3"),
        Sty::BoldItalic => paint(s, "1;3"),
        Sty::Code => paint(s, "36"),
        Sty::Link => paint(s, "4;34"),
        Sty::Dim => paint(s, "2"),
    }
}

/// Parse inline markup into styled spans.
fn spans(text: &str) -> Vec<(String, Sty)> {
    let mut out: Vec<(String, Sty)> = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut bold = false;
    let mut ital = false;
    let mut cur = String::new();
    let cur_style = |b: bool, it: bool| match (b, it) {
        (true, true) => Sty::BoldItalic,
        (true, false) => Sty::Bold,
        (false, true) => Sty::Italic,
        _ => Sty::Plain,
    };
    macro_rules! flush {
        () => {
            if !cur.is_empty() {
                out.push((std::mem::take(&mut cur), cur_style(bold, ital)));
            }
        };
    }
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' && i + 1 < chars.len() && "*`_[]\\|".contains(chars[i + 1]) {
            cur.push(chars[i + 1]);
            i += 2;
            continue;
        }
        if c == '`' {
            if let Some(end) = chars[i + 1..].iter().position(|&x| x == '`') {
                flush!();
                let code: String = chars[i + 1..i + 1 + end].iter().collect();
                out.push((code, Sty::Code));
                i += end + 2;
                continue;
            }
        }
        if c == '*' && i + 1 < chars.len() && chars[i + 1] == '*' {
            flush!();
            bold = !bold;
            i += 2;
            continue;
        }
        if c == '*' {
            let opening = !ital && i + 1 < chars.len() && !chars[i + 1].is_whitespace();
            let closing = ital && i > 0 && !chars[i - 1].is_whitespace();
            let has_close = chars[i + 1..].contains(&'*');
            if (opening && has_close) || closing {
                flush!();
                ital = !ital;
                i += 1;
                continue;
            }
        }
        if c == '[' {
            if let Some(close) = chars[i + 1..].iter().position(|&x| x == ']') {
                let after = i + 1 + close + 1;
                if after < chars.len() && chars[after] == '(' {
                    if let Some(pc) = chars[after + 1..].iter().position(|&x| x == ')') {
                        flush!();
                        let label: String = chars[i + 1..i + 1 + close].iter().collect();
                        let url: String = chars[after + 1..after + 1 + pc].iter().collect();
                        out.push((label.clone(), Sty::Link));
                        if label != url {
                            out.push((format!(" <{url}>"), Sty::Dim));
                        }
                        i = after + 1 + pc + 1;
                        continue;
                    }
                }
            }
        }
        cur.push(c);
        i += 1;
    }
    flush!();
    out
}

/// Render inline markup without wrapping.
pub fn inline(text: &str) -> String {
    spans(text).iter().map(|(s, st)| style(s, *st)).collect()
}

/// Wrap inline-styled text into lines of at most `width` visible columns.
/// `first` is the prefix of the first line, `rest` the prefix of following lines.
fn wrap_styled(text: &str, width: usize, first: &str, rest: &str) -> Vec<String> {
    // Split spans into words; a word is a sequence of styled pieces.
    let mut words: Vec<Vec<(String, Sty)>> = Vec::new();
    let mut current: Vec<(String, Sty)> = Vec::new();
    for (s, st) in spans(text) {
        let mut piece = String::new();
        for ch in s.chars() {
            // Spaces inside `code` spans are kept (as NBSP) so a span never breaks.
            if ch == ' ' && st != Sty::Code {
                if !piece.is_empty() {
                    current.push((std::mem::take(&mut piece), st));
                }
                if !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                }
            } else if ch == ' ' {
                piece.push('\u{a0}');
            } else {
                piece.push(ch);
            }
        }
        if !piece.is_empty() {
            current.push((piece, st));
        }
    }
    if !current.is_empty() {
        words.push(current);
    }

    // Words longer than a line are cut into line-sized chunks; every chunk
    // after the first starts a new line with no space before it.
    let limit = width.saturating_sub(visible_width(rest)).max(1);
    let mut chunks: Vec<(Vec<(String, Sty)>, bool)> = Vec::new();
    for w in words {
        let ww: usize = w.iter().map(|(s, _)| s.chars().count()).sum();
        if ww <= limit {
            chunks.push((w, false));
        } else {
            for (k, c) in split_word(&w, limit).into_iter().enumerate() {
                chunks.push((c, k > 0));
            }
        }
    }

    let mut lines = Vec::new();
    let mut line = String::from(first);
    let mut line_w = visible_width(first);
    let mut empty = true;
    for (w, cont) in chunks {
        let ww: usize = w.iter().map(|(s, _)| s.chars().count()).sum();
        if !empty && (cont || line_w + 1 + ww > width) {
            lines.push(std::mem::take(&mut line));
            line.push_str(rest);
            line_w = visible_width(rest);
            empty = true;
        }
        if !empty {
            line.push(' ');
            line_w += 1;
        }
        for (s, st) in &w {
            let s = if *st == Sty::Code { s.replace('\u{a0}', " ") } else { s.clone() };
            line.push_str(&style(&s, *st));
        }
        line_w += ww;
        empty = false;
    }
    if !empty || lines.is_empty() {
        lines.push(line);
    }
    lines
}

// ---------------------------------------------------------------------------
// Rust syntax highlighting

const KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
    "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type",
    "unsafe", "use", "where", "while", "union", "raw", "safe", "yield", "macro_rules",
];

const PRIMS: &[&str] = &[
    "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize",
    "f32", "f64", "bool", "char", "str",
];

#[derive(Default)]
pub struct HlState {
    in_block_comment: bool,
}

pub fn highlight_rust_line(line: &str, st: &mut HlState) -> String {
    let chars: Vec<char> = line.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    let n = chars.len();
    let take = |a: usize, b: usize| -> String { chars[a..b].iter().collect() };
    while i < n {
        if st.in_block_comment {
            let start = i;
            let mut end = n;
            while i + 1 < n {
                if chars[i] == '*' && chars[i + 1] == '/' {
                    end = i + 2;
                    st.in_block_comment = false;
                    break;
                }
                i += 1;
            }
            if st.in_block_comment {
                end = n;
            }
            out.push_str(&paint(&take(start, end), "2;3"));
            i = end;
            continue;
        }
        let c = chars[i];
        if c == '/' && i + 1 < n && chars[i + 1] == '/' {
            out.push_str(&paint(&take(i, n), "2;3"));
            break;
        }
        if c == '/' && i + 1 < n && chars[i + 1] == '*' {
            st.in_block_comment = true;
            continue;
        }
        if c == '#' && i + 1 < n && (chars[i + 1] == '[' || chars[i + 1] == '!') {
            // attribute: up to matching bracket on this line
            let mut depth = 0;
            let mut j = i;
            while j < n {
                if chars[j] == '[' {
                    depth += 1;
                } else if chars[j] == ']' {
                    depth -= 1;
                    if depth == 0 {
                        j += 1;
                        break;
                    }
                }
                j += 1;
            }
            out.push_str(&paint(&take(i, j), "33"));
            i = j;
            continue;
        }
        if c == '"' || (c == 'b' || c == 'c') && i + 1 < n && chars[i + 1] == '"' {
            let start = i;
            if c != '"' {
                i += 1;
            }
            i += 1;
            while i < n && chars[i] != '"' {
                if chars[i] == '\\' {
                    i += 1;
                }
                i += 1;
            }
            i = (i + 1).min(n);
            out.push_str(&paint(&take(start, i), "32"));
            continue;
        }
        if c == '\'' {
            // char literal 'x' / '\n' / b'x' vs lifetime 'a
            if i + 2 < n && chars[i + 1] != '\\' && chars[i + 2] == '\'' {
                out.push_str(&paint(&take(i, i + 3), "32"));
                i += 3;
                continue;
            }
            if i + 1 < n && chars[i + 1] == '\\' {
                let mut j = i + 2;
                while j < n && chars[j] != '\'' {
                    j += 1;
                }
                let end = (j + 1).min(n);
                out.push_str(&paint(&take(i, end), "32"));
                i = end;
                continue;
            }
            let mut j = i + 1;
            while j < n && (chars[j].is_alphanumeric() || chars[j] == '_') {
                j += 1;
            }
            out.push_str(&paint(&take(i, j), "35"));
            i = j;
            continue;
        }
        if c.is_ascii_digit() {
            let mut j = i;
            while j < n && (chars[j].is_ascii_alphanumeric() || chars[j] == '_' || chars[j] == '.') {
                if chars[j] == '.' && (j + 1 >= n || !chars[j + 1].is_ascii_digit()) {
                    break;
                }
                j += 1;
            }
            out.push_str(&paint(&take(i, j), "35"));
            i = j;
            continue;
        }
        if c.is_alphabetic() || c == '_' {
            let mut j = i;
            while j < n && (chars[j].is_alphanumeric() || chars[j] == '_') {
                j += 1;
            }
            let word = take(i, j);
            if j < n && chars[j] == '!' && !(j + 1 < n && chars[j + 1] == '=') {
                out.push_str(&paint(&format!("{word}!"), "1;34"));
                i = j + 1;
                continue;
            }
            let painted = if KEYWORDS.contains(&word.as_str()) {
                paint(&word, "1;35")
            } else if PRIMS.contains(&word.as_str()) {
                paint(&word, "36")
            } else if word.chars().next().is_some_and(|c| c.is_uppercase()) {
                paint(&word, "36")
            } else if j < n && chars[j] == '(' {
                paint(&word, "34")
            } else {
                word
            };
            out.push_str(&painted);
            i = j;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

pub fn render_code(lang: &str, code: &str, width: usize) -> String {
    let mut out = String::new();
    let is_rust = matches!(lang, "rust" | "rs" | "");
    let is_plain = matches!(lang, "text" | "console" | "sh" | "bash" | "toml" | "c" | "ld" | "asm" | "diff" | "ini" | "json");
    let mut st = HlState::default();
    let label = if lang.is_empty() { String::new() } else { format!(" {lang} ") };
    out.push_str(&format!("  {}{}\n", term::dim("╭─"), term::dim(&label)));
    for line in code.lines() {
        let body = if is_rust && !is_plain {
            highlight_rust_line(line, &mut st)
        } else if lang == "console" || lang == "sh" || lang == "bash" {
            if let Some(rest) = line.strip_prefix("$ ") {
                format!("{} {}", term::dim("$"), term::bold(rest))
            } else {
                term::dim(line)
            }
        } else if lang == "diff" {
            if line.starts_with('+') {
                term::green(line)
            } else if line.starts_with('-') {
                term::red(line)
            } else {
                line.to_string()
            }
        } else if lang == "toml" || lang == "ini" {
            if line.trim_start().starts_with('[') {
                term::yellow(line)
            } else if line.trim_start().starts_with('#') {
                term::dim(line)
            } else {
                line.to_string()
            }
        } else {
            line.to_string()
        };
        let _ = width;
        out.push_str(&format!("  {} {}\n", term::dim("│"), body));
    }
    out.push_str(&format!("  {}\n", term::dim("╰─")));
    out
}

// ---------------------------------------------------------------------------
// Block rendering

fn is_list_item(line: &str) -> Option<(usize, String, String)> {
    let indent = line.len() - line.trim_start().len();
    let t = line.trim_start();
    if let Some(rest) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) {
        return Some((indent, "-".into(), rest.to_string()));
    }
    let digits: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
    if !digits.is_empty() {
        if let Some(rest) = t[digits.len()..].strip_prefix(". ") {
            return Some((indent, format!("{digits}."), rest.to_string()));
        }
    }
    None
}

fn is_block_start(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("```")
        || t.starts_with('#') && t.trim_start_matches('#').starts_with(' ')
        || t.starts_with('>')
        || t.starts_with('|')
        || t == "---"
        || is_list_item(line).is_some()
}

/// Cut one styled word into chunks of at most `limit` characters, preferring
/// to break after `-`, `_`, `:`, `/`, `,`, `.` or `|` in the second half of a chunk.
fn split_word(w: &[(String, Sty)], limit: usize) -> Vec<Vec<(String, Sty)>> {
    let chars: Vec<(char, Sty)> = w.iter().flat_map(|(s, st)| s.chars().map(move |c| (c, *st))).collect();
    let mut out = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        let mut end = (start + limit).min(chars.len());
        if end < chars.len() {
            if let Some(k) = (start + limit / 2..end).rev().find(|&k| matches!(chars[k].0, '-' | '_' | ':' | '/' | ',' | '.' | '|')) {
                end = k + 1;
            }
        }
        let mut chunk: Vec<(String, Sty)> = Vec::new();
        for &(c, st) in &chars[start..end] {
            match chunk.last_mut() {
                Some((s, last)) if *last == st => s.push(c),
                _ => chunk.push((c.to_string(), st)),
            }
        }
        out.push(chunk);
        start = end;
    }
    out
}

/// Wrap one paragraph of inline markdown, with prefixes for the first and
/// following lines (which may contain ANSI styling).
pub fn wrap(text: &str, width: usize, first: &str, rest: &str) -> String {
    let mut out = wrap_styled(text, width, first, rest).join("\n");
    out.push('\n');
    out
}

fn render_table(rows: &[Vec<String>], width: usize) -> String {
    let cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if cols == 0 {
        return String::new();
    }
    // Natural column widths (visible characters, inline markup removed).
    let mut widths = vec![0usize; cols];
    for r in rows {
        for (i, c) in r.iter().enumerate() {
            widths[i] = widths[i].max(visible_width(&inline(c)));
        }
    }
    // Shrink the widest columns until the table fits; cells then wrap.
    let overhead = 2 + 3 * cols + 1;
    let available = width.saturating_sub(overhead).max(cols * 6);
    while widths.iter().sum::<usize>() > available {
        let (imax, wmax) = widths.iter().enumerate().max_by_key(|(_, w)| **w).map(|(i, w)| (i, *w)).unwrap_or((0, 0));
        if wmax <= 6 {
            break;
        }
        widths[imax] -= 1;
    }
    let mut out = String::new();
    let line = |l: &str, m: &str, r: &str| -> String {
        let segs: Vec<String> = widths.iter().map(|w| "─".repeat(w + 2)).collect();
        term::dim(&format!("  {l}{}{r}", segs.join(m)))
    };
    out.push_str(&line("┌", "┬", "┐"));
    out.push('\n');
    for (ri, r) in rows.iter().enumerate() {
        let cells: Vec<Vec<String>> = (0..cols)
            .map(|i| {
                let text = r.get(i).cloned().unwrap_or_default();
                let text = if ri == 0 { format!("**{}**", text.replace("**", "")) } else { text };
                wrap_styled(&text, widths[i], "", "")
            })
            .collect();
        let height = cells.iter().map(|c| c.len()).max().unwrap_or(1);
        for k in 0..height {
            out.push_str("  ");
            out.push_str(&term::dim("│"));
            for (i, w) in widths.iter().enumerate() {
                let c = cells[i].get(k).cloned().unwrap_or_default();
                out.push(' ');
                out.push_str(&term::pad(&c, *w));
                out.push(' ');
                out.push_str(&term::dim("│"));
            }
            out.push('\n');
        }
        if ri == 0 {
            out.push_str(&line("├", "┼", "┤"));
            out.push('\n');
        }
    }
    out.push_str(&line("└", "┴", "┘"));
    out.push('\n');
    out.push('\n');
    out
}

fn callout_color(first_line: &str) -> &'static str {
    let l = first_line.to_ascii_lowercase();
    if l.contains("toyota") || l.contains("automotive") || l.contains("safety") {
        "35"
    } else if l.contains("warning") || l.contains("pitfall") || l.contains("danger") || l.contains("ub") {
        "33"
    } else if l.contains("interview") {
        "36"
    } else if l.contains("try it") || l.contains("exercise") {
        "32"
    } else {
        "34"
    }
}

pub fn render(md: &str, width: usize) -> String {
    let lines: Vec<&str> = md.lines().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].trim_end();
        let t = line.trim_start();
        if t.is_empty() {
            i += 1;
            continue;
        }
        // Fenced code
        if let Some(lang) = t.strip_prefix("```") {
            let lang = lang.trim().to_string();
            let mut code = String::new();
            i += 1;
            while i < lines.len() && !lines[i].trim_start().starts_with("```") {
                code.push_str(lines[i]);
                code.push('\n');
                i += 1;
            }
            i += 1; // closing fence
            out.push_str(&render_code(&lang, &code, width));
            out.push('\n');
            continue;
        }
        // Headings
        if t.starts_with('#') {
            let level = t.chars().take_while(|&c| c == '#').count();
            let text = t[level..].trim();
            let txt = inline(text);
            match level {
                1 => {
                    let plain = term::strip_ansi(&txt);
                    out.push_str(&format!("{}\n", paint(&format!(" {plain} "), "1;7;36")));
                    out.push_str(&format!("{}\n\n", term::dim(&"━".repeat(width.min(plain.chars().count() + 2)))));
                }
                2 => {
                    out.push_str(&format!("{} {}\n\n", term::bold_cyan("▍"), term::bold_cyan(&term::strip_ansi(&txt))));
                }
                _ => {
                    out.push_str(&format!("{}\n\n", term::bold(&term::strip_ansi(&txt))));
                }
            }
            i += 1;
            continue;
        }
        if t == "---" || t == "***" {
            out.push_str(&format!("  {}\n\n", term::dim(&"─".repeat(width.saturating_sub(4)))));
            i += 1;
            continue;
        }
        // Blockquote / callout
        if t.starts_with('>') {
            let mut inner = String::new();
            while i < lines.len() && lines[i].trim_start().starts_with('>') {
                let l = lines[i].trim_start().trim_start_matches('>');
                let l = l.strip_prefix(' ').unwrap_or(l);
                inner.push_str(l);
                inner.push('\n');
                i += 1;
            }
            let color = callout_color(inner.lines().next().unwrap_or(""));
            let body = render(&inner, width.saturating_sub(4));
            let body = body.trim_end_matches('\n');
            for l in body.lines() {
                out.push_str(&format!("  {} {}\n", paint("▌", color), l.strip_prefix("  ").unwrap_or(l)));
            }
            out.push('\n');
            continue;
        }
        // Table
        if t.starts_with('|') {
            let mut rows: Vec<Vec<String>> = Vec::new();
            while i < lines.len() && lines[i].trim_start().starts_with('|') {
                let row = lines[i].trim().trim_matches('|');
                let cells: Vec<String> = split_table_row(row);
                let is_sep = cells.iter().all(|c| !c.is_empty() && c.chars().all(|ch| ch == '-' || ch == ':' || ch == ' '));
                if !is_sep {
                    rows.push(cells);
                }
                i += 1;
            }
            if !rows.is_empty() {
                out.push_str(&render_table(&rows, width));
            }
            continue;
        }
        // Lists
        if is_list_item(line).is_some() {
            let base_indent = line.len() - t.len();
            while i < lines.len() {
                let l = lines[i].trim_end();
                let Some((indent, marker, text)) = is_list_item(l) else { break };
                let mut text = text;
                i += 1;
                // continuation lines
                while i < lines.len() {
                    let nl = lines[i].trim_end();
                    if nl.trim().is_empty() || is_list_item(nl).is_some() || is_block_start(nl) && !nl.starts_with("  ") {
                        break;
                    }
                    if nl.trim_start().starts_with("```") {
                        break;
                    }
                    text.push(' ');
                    text.push_str(nl.trim());
                    i += 1;
                }
                let depth = indent.saturating_sub(base_indent) / 2;
                let pad = "  ".repeat(depth + 1);
                let bullet = if marker == "-" {
                    match depth {
                        0 => term::cyan("•"),
                        1 => term::cyan("◦"),
                        _ => term::cyan("▪"),
                    }
                } else {
                    term::cyan(&marker)
                };
                let bw = visible_width(&bullet);
                let first = format!("{pad}{bullet} ");
                let rest = format!("{pad}{} ", " ".repeat(bw));
                for wl in wrap_styled(&text, width, &first, &rest) {
                    out.push_str(&wl);
                    out.push('\n');
                }
                // allow blank lines between items of the same list
                if i < lines.len() && lines[i].trim().is_empty() && i + 1 < lines.len() && is_list_item(lines[i + 1]).is_some() {
                    i += 1;
                }
            }
            out.push('\n');
            continue;
        }
        // Paragraph
        let mut para = String::new();
        while i < lines.len() {
            let l = lines[i].trim_end();
            if l.trim().is_empty() || (!para.is_empty() && is_block_start(l)) {
                break;
            }
            if !para.is_empty() {
                para.push(' ');
            }
            para.push_str(l.trim());
            i += 1;
            if l.ends_with("  ") {
                break;
            }
        }
        for wl in wrap_styled(&para, width, "  ", "  ") {
            out.push_str(&wl);
            out.push('\n');
        }
        out.push('\n');
    }
    out
}

fn split_table_row(row: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut cur = String::new();
    let mut in_code = false;
    let mut chars = row.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'|') => {
                cur.push('|');
                chars.next();
            }
            '`' => {
                in_code = !in_code;
                cur.push(c);
            }
            '|' if !in_code => {
                cells.push(cur.trim().to_string());
                cur.clear();
            }
            _ => cur.push(c),
        }
    }
    cells.push(cur.trim().to_string());
    cells
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_without_color() {
        crate::term::set_color(false);
        let md = "# Title\n\nSome `code` and **bold** text.\n\n- one\n- two\n\n```rust\nfn main() {}\n```\n";
        let out = render(md, 80);
        assert!(out.contains("Title"));
        assert!(out.contains("Some code and bold text."));
        assert!(out.contains("• one"));
        assert!(out.contains("fn main() {}"));
    }

    #[test]
    fn wraps_long_paragraphs() {
        crate::term::set_color(false);
        let md = "word ".repeat(60);
        let out = render(&md, 40);
        assert!(out.lines().all(|l| l.chars().count() <= 40));
    }

    #[test]
    fn tables() {
        crate::term::set_color(false);
        let md = "| a | b |\n|---|---|\n| `x` | y |\n";
        let out = render(md, 80);
        assert!(out.contains("│ x"));
    }

    #[test]
    fn long_words_never_overflow() {
        crate::term::set_color(false);
        let md = "| a | b |\n|---|---|\n| short | `critical_section::with(|cs| ..)` and critical-section-single-core |\n";
        let out = render(md, 40);
        let widths: Vec<usize> = out.lines().filter(|l| !l.trim().is_empty()).map(crate::term::visible_width).collect();
        assert!(widths.iter().all(|w| *w == widths[0]), "ragged table:\n{out}");
        assert!(wrap("x supercalifragilistic", 10, "", "").lines().all(|l| l.chars().count() <= 10));
    }
}
