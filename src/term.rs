//! Terminal helpers: colour, sizing, paging and prompts. No dependencies.

use std::io::{self, BufRead, IsTerminal, Write};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU8, Ordering};

// 0 = undecided, 1 = off, 2 = on
static COLOR: AtomicU8 = AtomicU8::new(0);

pub fn color_enabled() -> bool {
    match COLOR.load(Ordering::Relaxed) {
        1 => false,
        2 => true,
        _ => {
            let on = if std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty()) {
                false
            } else {
                match std::env::var("NOSTD_COLOR").as_deref() {
                    Ok("always") | Ok("1") => true,
                    Ok("never") | Ok("0") => false,
                    _ => io::stdout().is_terminal(),
                }
            };
            set_color(on);
            on
        }
    }
}

pub fn set_color(on: bool) {
    COLOR.store(if on { 2 } else { 1 }, Ordering::Relaxed);
}

pub fn paint(s: &str, code: &str) -> String {
    if color_enabled() && !s.is_empty() {
        format!("\x1b[{code}m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

pub fn bold(s: &str) -> String {
    paint(s, "1")
}
pub fn dim(s: &str) -> String {
    paint(s, "2")
}
pub fn italic(s: &str) -> String {
    paint(s, "3")
}
pub fn red(s: &str) -> String {
    paint(s, "31")
}
pub fn green(s: &str) -> String {
    paint(s, "32")
}
pub fn yellow(s: &str) -> String {
    paint(s, "33")
}
pub fn blue(s: &str) -> String {
    paint(s, "34")
}
pub fn magenta(s: &str) -> String {
    paint(s, "35")
}
pub fn cyan(s: &str) -> String {
    paint(s, "36")
}
pub fn bold_green(s: &str) -> String {
    paint(s, "1;32")
}
pub fn bold_red(s: &str) -> String {
    paint(s, "1;31")
}
pub fn bold_yellow(s: &str) -> String {
    paint(s, "1;33")
}
pub fn bold_cyan(s: &str) -> String {
    paint(s, "1;36")
}
pub fn bold_magenta(s: &str) -> String {
    paint(s, "1;35")
}

/// Visible width of a string, ignoring ANSI escape sequences.
pub fn visible_width(s: &str) -> usize {
    let mut n = 0;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // CSI: ESC [ ... final byte in 0x40..=0x7e
            if chars.peek() == Some(&'[') {
                chars.next();
                for c2 in chars.by_ref() {
                    if ('\x40'..='\x7e').contains(&c2) {
                        break;
                    }
                }
            }
            continue;
        }
        n += 1;
    }
    n
}

pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' && chars.peek() == Some(&'[') {
            chars.next();
            for c2 in chars.by_ref() {
                if ('\x40'..='\x7e').contains(&c2) {
                    break;
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

fn stty_size() -> Option<(usize, usize)> {
    #[cfg(unix)]
    {
        let tty = std::fs::File::open("/dev/tty").ok()?;
        let out = Command::new("stty")
            .arg("size")
            .stdin(tty)
            .stderr(Stdio::null())
            .output()
            .ok()?;
        let s = String::from_utf8(out.stdout).ok()?;
        let mut it = s.split_whitespace();
        let rows = it.next()?.parse().ok()?;
        let cols = it.next()?.parse().ok()?;
        Some((rows, cols))
    }
    #[cfg(not(unix))]
    {
        None
    }
}

/// Terminal width in columns (falls back to 80).
pub fn width() -> usize {
    if let Some(c) = std::env::var("COLUMNS").ok().and_then(|v| v.parse::<usize>().ok()) {
        if c >= 20 {
            return c;
        }
    }
    if io::stdout().is_terminal() {
        if let Some((_, c)) = stty_size() {
            if c >= 20 {
                return c;
            }
        }
    }
    80
}

pub fn height() -> usize {
    if let Some(r) = std::env::var("LINES").ok().and_then(|v| v.parse::<usize>().ok()) {
        return r;
    }
    stty_size().map(|(r, _)| r).unwrap_or(24)
}

/// Width used for prose: never wider than 100 columns, for readability.
pub fn text_width() -> usize {
    width().clamp(40, 100)
}

/// Print text through a pager when it does not fit on screen.
pub fn page(text: &str, allow_pager: bool) {
    let stdout = io::stdout();
    let is_tty = stdout.is_terminal();
    let lines = text.lines().count();
    let disabled = std::env::var("NOSTD_PAGER").is_ok_and(|v| v == "0" || v == "never");
    if !allow_pager || !is_tty || disabled || lines + 2 <= height() {
        let mut lock = stdout.lock();
        let _ = lock.write_all(text.as_bytes());
        let _ = lock.flush();
        return;
    }
    let pager = std::env::var("NOSTD_PAGER")
        .or_else(|_| std::env::var("PAGER"))
        .unwrap_or_else(|_| "less".to_string());
    let mut parts = pager.split_whitespace();
    let Some(prog) = parts.next() else {
        print!("{text}");
        return;
    };
    let mut cmd = Command::new(prog);
    cmd.args(parts);
    if prog.ends_with("less") && std::env::var_os("LESS").is_none() {
        // -R: pass colour codes through, -F: quit if one screen, -X: keep text on exit
        cmd.env("LESS", "-RFX");
    }
    match cmd.stdin(Stdio::piped()).spawn() {
        Ok(mut child) => {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            let _ = child.wait();
        }
        Err(_) => {
            print!("{text}");
        }
    }
}

/// Read one line from stdin after printing a prompt. `None` on EOF.
pub fn prompt(msg: &str) -> Option<String> {
    print!("{msg}");
    let _ = io::stdout().flush();
    let mut line = String::new();
    match io::stdin().lock().read_line(&mut line) {
        Ok(0) | Err(_) => None,
        Ok(_) => Some(line.trim().to_string()),
    }
}

pub fn clear_screen() {
    if io::stdout().is_terminal() {
        print!("\x1b[2J\x1b[H");
        let _ = io::stdout().flush();
    }
}

pub fn progress_bar(done: usize, total: usize, width: usize) -> String {
    let total = total.max(1);
    let filled = (done * width + total / 2) / total;
    let filled = filled.min(width);
    let bar_done: String = "█".repeat(filled);
    let bar_todo: String = "░".repeat(width - filled);
    format!("{}{}", green(&bar_done), dim(&bar_todo))
}

/// Pad a (possibly coloured) string to `w` visible columns.
pub fn pad(s: &str, w: usize) -> String {
    let vw = visible_width(s);
    if vw >= w {
        s.to_string()
    } else {
        format!("{s}{}", " ".repeat(w - vw))
    }
}

pub fn rule(w: usize) -> String {
    dim(&"─".repeat(w))
}

/// Word-wrap plain text (no ANSI) to `width`, prefixing each line with `indent`.
pub fn wrap_plain(text: &str, width: usize, indent: &str) -> String {
    let mut out = String::new();
    for para in text.split('\n') {
        let mut line = String::new();
        for word in para.split_whitespace() {
            if !line.is_empty() && indent.len() + line.len() + 1 + word.len() > width {
                out.push_str(indent);
                out.push_str(&line);
                out.push('\n');
                line.clear();
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
        out.push_str(indent);
        out.push_str(&line);
        out.push('\n');
    }
    out
}
