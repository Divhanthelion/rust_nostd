//! `nostd`: an interactive course on `#![no_std]` Rust.
//!
//! Run `nostd help` for usage.

mod commands;
mod curriculum;
mod dev;
mod drill;
mod elf;
mod markdown;
mod quiz;
mod reference;
mod runner;
mod state;
mod templates;
mod term;
mod toolchain;
mod watch;
mod workspace;

use std::process::ExitCode;

pub struct Args {
    pub positional: Vec<String>,
    pub flags: Vec<String>,
    pub options: Vec<(String, String)>,
}

impl Args {
    pub fn flag(&self, names: &[&str]) -> bool {
        self.flags.iter().any(|f| names.contains(&f.as_str()))
    }
    pub fn opt(&self, names: &[&str]) -> Option<&str> {
        self.options.iter().find(|(k, _)| names.contains(&k.as_str())).map(|(_, v)| v.as_str())
    }
    pub fn pos(&self, i: usize) -> Option<&str> {
        self.positional.get(i).map(String::as_str)
    }
}

/// Options that take a value (everything else starting with `-` is a flag).
const VALUE_OPTIONS: &[&str] = &["--target", "--topic", "-j", "--jobs", "--dir", "--module", "-m"];

fn parse_args(raw: Vec<String>) -> Args {
    let mut a = Args { positional: Vec::new(), flags: Vec::new(), options: Vec::new() };
    let mut it = raw.into_iter();
    while let Some(arg) = it.next() {
        if arg == "--" {
            a.positional.extend(it.by_ref());
            break;
        }
        if arg.starts_with('-') && arg.len() > 1 && arg.parse::<i64>().is_err() {
            if let Some((k, v)) = arg.split_once('=') {
                a.options.push((k.to_string(), v.to_string()));
            } else if VALUE_OPTIONS.contains(&arg.as_str()) {
                let v = it.next().unwrap_or_default();
                a.options.push((arg, v));
            } else {
                a.flags.push(arg);
            }
        } else {
            a.positional.push(arg);
        }
    }
    a
}

fn main() -> ExitCode {
    // `nostd learn 3 | head` closes the pipe early; exit quietly like other CLIs.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let msg = info.to_string();
        if msg.contains("Broken pipe") || msg.contains("failed printing to stdout") {
            std::process::exit(0);
        }
        default_hook(info);
    }));
    let mut raw: Vec<String> = std::env::args().skip(1).collect();
    // Global flags.
    raw.retain(|a| match a.as_str() {
        "--no-color" | "--no-colour" => {
            term::set_color(false);
            false
        }
        "--color" | "--colour" => {
            term::set_color(true);
            false
        }
        "--no-pager" => {
            // SAFETY: single-threaded at this point; nothing else reads the environment concurrently.
            unsafe { std::env::set_var("NOSTD_PAGER", "0") };
            false
        }
        _ => true,
    });
    let cmd = if raw.is_empty() { String::new() } else { raw.remove(0) };
    let args = parse_args(raw);
    let result = match cmd.as_str() {
        "" | "status" => commands::status(),
        "init" => commands::init(&args),
        "next" | "n" => commands::next(&args),
        "list" | "ls" | "progress" => commands::list(&args),
        "learn" | "lesson" | "read" => commands::learn(&args),
        "show" | "info" => commands::show(&args),
        "run" | "check" | "verify" => commands::run(&args),
        "watch" | "w" => watch::watch(&args),
        "hint" | "h" => commands::hint(&args),
        "solution" | "sol" => commands::solution(&args),
        "reset" => commands::reset(&args),
        "skip" => commands::skip(&args),
        "quiz" | "q" => commands::quiz(&args),
        "drill" | "flashcards" => commands::drill(&args),
        "where" | "whereis" | "find" => commands::where_(&args),
        "new" | "scaffold" => templates::new(&args),
        "doctor" => commands::doctor(&args),
        "lsp" => commands::lsp(&args),
        "help" | "--help" | "-h" => {
            commands::help(args.pos(0));
            Ok(())
        }
        "--version" | "-V" | "version" => {
            println!("nostd {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        "dev" => dev::dev(&args),
        other => {
            if let Some(ex) = curriculum::find_exercise(other) {
                // `nostd core1` is a shortcut for `nostd run core1`.
                let mut a = args;
                a.positional.insert(0, ex.name.to_string());
                commands::run(&a)
            } else {
                Err(format!("unknown command `{other}`. Run `nostd help` for the list of commands."))
            }
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) if e.is_empty() => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("{} {e}", term::bold_red("error:"));
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arg_parsing() {
        let a = parse_args(vec!["core1".into(), "--force".into(), "--target".into(), "thumbv6m-none-eabi".into(), "-j=4".into()]);
        assert_eq!(a.pos(0), Some("core1"));
        assert!(a.flag(&["--force"]));
        assert_eq!(a.opt(&["--target"]), Some("thumbv6m-none-eabi"));
        assert_eq!(a.opt(&["-j"]), Some("4"));
    }
}
