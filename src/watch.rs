//! `nostd watch`: re-check the current exercise whenever its file changes.

use std::io::BufRead;
use std::sync::mpsc;
use std::time::{Duration, SystemTime};

use crate::commands::{self, Res};
use crate::curriculum::{self, Step};
use crate::term;
use crate::Args;

fn mtime(p: &std::path::Path) -> Option<SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

fn footer(name: &str, advance: bool) {
    if advance {
        println!(
            "  {}",
            term::dim("Enter = next exercise · q = quit")
        );
    } else {
        println!(
            "  {}",
            term::dim(&format!("watching {name} · save the file to re-check · h hint · s show · l list · r re-run · q quit (+Enter)"))
        );
    }
}

fn args_for(name: &str) -> Args {
    Args { positional: vec![name.to_string()], flags: Vec::new(), options: Vec::new() }
}

pub fn watch(args: &Args) -> Res {
    let (ws, mut st) = commands::load()?;
    let mut ex = commands::resolve_exercise(&st, args.pos(0))?;
    st.current = Some(ex.name.to_string());
    st.save()?;

    let (tx, rx) = mpsc::channel::<Option<String>>();
    std::thread::spawn(move || {
        let stdin = std::io::stdin();
        for line in stdin.lock().lines() {
            match line {
                Ok(l) => {
                    if tx.send(Some(l)).is_err() {
                        return;
                    }
                }
                Err(_) => break,
            }
        }
        let _ = tx.send(None);
    });

    let mut last = None;
    let mut force = true;
    let mut advance = false;
    let mut stdin_open = true;
    loop {
        let path = ws.ensure_exercise(ex)?;
        let now = mtime(&path);
        if !advance && (force || now != last) {
            // Editors often write in several steps; give them a moment.
            if !force {
                std::thread::sleep(Duration::from_millis(120));
            }
            last = mtime(&path);
            force = false;
            term::clear_screen();
            let passed = commands::check_and_record(&ws, &mut st, ex, None)?;
            if passed {
                match curriculum::exercises().find(|e| !st.done.contains(e.name)) {
                    Some(n) => {
                        // Point out lessons/quizzes that come before the next exercise.
                        for s in curriculum::path() {
                            match s {
                                Step::Lesson(m) if !st.read.contains(&m.num) && m.exercises.iter().any(|e| e.name == n.name) => {
                                    println!(
                                        "  {} the next exercise belongs to module {:02}; read its lesson first: {}",
                                        term::bold_yellow("note:"),
                                        m.num,
                                        term::bold(&format!("nostd learn {}", m.num))
                                    );
                                }
                                Step::Quiz(m) if !st.quiz.contains_key(&m.num) && m.exercises.iter().any(|e| e.name == ex.name) => {
                                    println!("  {} module {:02} is complete; test yourself with {}", term::bold_cyan("tip:"), m.num, term::bold(&format!("nostd quiz {}", m.num)));
                                }
                                _ => {}
                            }
                        }
                        advance = true;
                        if !stdin_open {
                            ex = n;
                            advance = false;
                            force = true;
                            st.current = Some(ex.name.to_string());
                            st.save()?;
                            continue;
                        }
                    }
                    None => {
                        println!("  {}", term::bold_green("Every exercise is done. Superb work."));
                        return Ok(());
                    }
                }
            }
            footer(ex.name, advance);
        }
        if !stdin_open {
            std::thread::sleep(Duration::from_millis(250));
            continue;
        }
        match rx.recv_timeout(Duration::from_millis(250)) {
            Ok(Some(cmd)) => match cmd.trim() {
                "q" | "quit" | "exit" => return Ok(()),
                "" | "n" | "next" if advance => {
                    if let Some(n) = curriculum::exercises().find(|e| !st.done.contains(e.name)) {
                        ex = n;
                        st.current = Some(ex.name.to_string());
                        st.save()?;
                        term::clear_screen();
                        commands::show(&args_for(ex.name))?;
                        footer(ex.name, false);
                        last = mtime(&ws.exercise_file(ex));
                    }
                    advance = false;
                }
                "h" | "hint" => {
                    commands::hint(&args_for(ex.name))?;
                    st = crate::state::State::load(&ws.state_path());
                    footer(ex.name, advance);
                }
                "s" | "show" => {
                    commands::show(&args_for(ex.name))?;
                    footer(ex.name, advance);
                }
                "l" | "list" => {
                    commands::list(&Args { positional: vec![ex.module().num.to_string()], flags: vec![], options: vec![] })?;
                    footer(ex.name, advance);
                }
                "r" | "run" | "" => {
                    force = true;
                    advance = false;
                }
                other => {
                    println!("  {} unknown command {other:?}", term::dim("?"));
                    footer(ex.name, advance);
                }
            },
            Ok(None) => {
                // stdin closed (e.g. running under a script): keep watching files.
                stdin_open = false;
                if advance {
                    force = true;
                    advance = false;
                    if let Some(n) = curriculum::exercises().find(|e| !st.done.contains(e.name)) {
                        ex = n;
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => stdin_open = false,
        }
    }
}
