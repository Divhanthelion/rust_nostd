//! Maintainer commands. `nostd dev verify` proves the course is consistent:
//! every reference solution passes, every untouched exercise fails, quizzes
//! and flashcards parse, and the `where` table's paths really exist.

use std::sync::Mutex;
use std::time::Instant;

use crate::commands::{leading_doc, Res};
use crate::curriculum::{self, Exercise, MODULES};
use crate::runner::{self, Ctx, Verdict};
use crate::toolchain::Toolchain;
use crate::workspace::{write_file, Workspace};
use crate::{drill, quiz, reference, term, Args};

pub fn dev(args: &Args) -> Res {
    match args.pos(0) {
        Some("verify") => verify(args),
        Some("solve") => solve(args),
        _ => Err("usage: nostd dev verify [EXERCISE...] [-j N] [--keep] [--strict]\n       nostd dev solve EXERCISE   (write the solution into your workspace)".into()),
    }
}

fn solve(args: &Args) -> Res {
    let ws = crate::commands::workspace()?;
    let name = args.pos(1).ok_or("name an exercise")?;
    let ex = curriculum::find_exercise(name).ok_or("no such exercise")?;
    write_file(&ws.exercise_file(ex), ex.solution)?;
    println!("  wrote the reference solution to {}", ex.rel_path());
    Ok(())
}

fn content_checks(tc: &Toolchain, ws: &Workspace) -> Vec<String> {
    let mut problems = Vec::new();
    let mut names = std::collections::BTreeSet::new();
    for (i, m) in MODULES.iter().enumerate() {
        if m.num as usize != i {
            problems.push(format!("module {} is at index {i}", m.num));
        }
        if let Some(q) = m.quiz {
            match quiz::parse(q) {
                Ok(qs) if qs.len() < 5 => problems.push(format!("quiz {} has only {} questions", m.num, qs.len())),
                Ok(_) => {}
                Err(e) => problems.push(format!("quiz {}: {e}", m.num)),
            }
        }
        for e in m.exercises {
            if !names.insert(e.name) {
                problems.push(format!("duplicate exercise name {}", e.name));
            }
            if e.dir != m.dir() {
                problems.push(format!("{}: dir {} should be {}", e.name, e.dir, m.dir()));
            }
            if e.hints.is_empty() {
                problems.push(format!("{}: no hints", e.name));
            }
            if leading_doc(e.source).trim().is_empty() {
                problems.push(format!("{}: missing //! instructions", e.name));
            }
            if e.source == e.solution {
                problems.push(format!("{}: solution identical to exercise", e.name));
            }
        }
    }
    match drill::parse(curriculum::DRILLS) {
        Ok(c) if c.len() < 20 => problems.push(format!("only {} flashcards", c.len())),
        Ok(cards) => {
            let mut ids = std::collections::BTreeSet::new();
            for c in &cards {
                if !ids.insert(c.id.clone()) {
                    problems.push(format!("duplicate flashcard id for {:?}", c.question));
                }
            }
        }
        Err(e) => problems.push(format!("drills: {e}")),
    }
    // The `where` table must only reference paths that exist in core/alloc.
    let dir = ws.build_dir("_where_check");
    let src = dir.join("where_check.rs");
    if write_file(&src, &reference::verification_source()).is_ok() {
        match tc.core_sysroot(&ws.cache_dir()) {
            Ok(core) => {
                let out = std::process::Command::new(&tc.rustc)
                    .args(["--edition=2024", "--crate-type=lib", "--emit=metadata", "--crate-name=where_check"])
                    .arg("--sysroot")
                    .arg(core)
                    .arg("--out-dir")
                    .arg(&dir)
                    .arg(&src)
                    .output();
                match out {
                    Ok(o) if o.status.success() => {}
                    Ok(o) => problems.push(format!("`where` table has invalid paths:\n{}", String::from_utf8_lossy(&o.stderr))),
                    Err(e) => problems.push(format!("could not run rustc: {e}")),
                }
            }
            Err(e) => problems.push(format!("core sysroot: {e}")),
        }
    }
    problems
}

struct Row {
    name: &'static str,
    solution: Verdict,
    original: Verdict,
    log: String,
    secs: f32,
}

fn verify_one(ctx: &Ctx, ex: &'static Exercise) -> Row {
    let start = Instant::now();
    let path = ctx.ws.exercise_file(ex);
    let mut log = String::new();
    let _ = ctx.ws.reset_exercise(ex);
    let _ = write_file(&path, ex.solution);
    let sol = runner::check(ctx, ex, &path);
    if sol.verdict != Verdict::Pass {
        log.push_str(&format!("--- solution log for {} ---\n{}", ex.name, sol.log));
    }
    let _ = write_file(&path, ex.source);
    let orig = runner::check(ctx, ex, &path);
    if orig.verdict == Verdict::Pass {
        log.push_str(&format!("--- original exercise {} passed but should fail ---\n{}", ex.name, orig.log));
    }
    Row { name: ex.name, solution: sol.verdict, original: orig.verdict, log, secs: start.elapsed().as_secs_f32() }
}

fn verify(args: &Args) -> Res {
    let tc = Toolchain::detect()?;
    if args.flag(&["--strict"]) {
        // CI mode: every optional tool must be present, so nothing is skipped silently.
        let missing: Vec<&str> = [
            ("freestanding Linux (x86_64/aarch64)", tc.can_run_freestanding()),
            ("a C compiler", crate::toolchain::c_compiler().is_some()),
            ("target thumbv7m-none-eabi", tc.has_target("thumbv7m-none-eabi")),
            ("qemu-system-arm", crate::toolchain::which("qemu-system-arm").is_some()),
        ]
        .into_iter()
        .filter(|(_, ok)| !ok)
        .map(|(name, _)| name)
        .collect();
        if !missing.is_empty() {
            return Err(format!("--strict: missing {}", missing.join(", ")));
        }
    }
    let dir = std::env::temp_dir().join(format!("nostd-verify-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (ws, _) = Workspace::init(&dir, Some(&tc))?;
    println!("  verifying in {}", term::dim(&ws.root.display().to_string()));

    let problems = content_checks(&tc, &ws);
    for p in &problems {
        println!("  {} {p}", term::bold_red("content:"));
    }
    if problems.is_empty() {
        println!("  {} lessons, quizzes, flashcards and the `where` table", term::bold_green("ok"));
    }

    let selected: Vec<&'static Exercise> = if args.positional.len() > 1 {
        let mut v = Vec::new();
        for n in &args.positional[1..] {
            if let Some(m) = curriculum::find_module(n).filter(|_| n.parse::<u8>().is_ok()) {
                v.extend(m.exercises.iter());
            } else {
                v.push(curriculum::find_exercise(n).ok_or(format!("no exercise {n}"))?);
            }
        }
        v
    } else {
        curriculum::exercises().collect()
    };
    let jobs = args
        .opt(&["-j", "--jobs"])
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2))
        .max(1);
    let ctx = Ctx { ws: &ws, tc: &tc, quiet: true, cross_target: None };
    runner::prepare(&ctx)?;

    let queue = Mutex::new(selected.clone());
    let rows = Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..jobs {
            s.spawn(|| loop {
                let next = queue.lock().ok().and_then(|mut q| q.pop());
                let Some(ex) = next else { break };
                let row = verify_one(&ctx, ex);
                let mark = |v: &Verdict, want_pass: bool| match (v, want_pass) {
                    (Verdict::Pass, true) | (Verdict::Fail, false) => term::green("✓"),
                    (Verdict::Blocked(_), _) => term::yellow("-"),
                    _ => term::bold_red("✗"),
                };
                println!(
                    "  {} solution passes {} exercise fails  {}  {}",
                    mark(&row.solution, true),
                    mark(&row.original, false),
                    term::pad(row.name, 12),
                    term::dim(&format!("{:.1}s", row.secs))
                );
                if let Ok(mut r) = rows.lock() {
                    r.push(row);
                }
            });
        }
    });
    let rows = rows.into_inner().unwrap_or_default();
    let mut bad = 0;
    let mut blocked = 0;
    for r in &rows {
        let ok = r.solution == Verdict::Pass && r.original == Verdict::Fail;
        if matches!(r.solution, Verdict::Blocked(_)) {
            blocked += 1;
        } else if !ok {
            bad += 1;
            println!("\n{} {}", term::bold_red("FAILED:"), r.name);
            print!("{}", r.log);
        }
    }
    println!();
    println!(
        "  {} exercises verified, {} problems, {} blocked (missing tools/targets), {} content problems",
        rows.len(),
        bad,
        blocked,
        problems.len()
    );
    if !args.flag(&["--keep"]) {
        let _ = std::fs::remove_dir_all(&dir);
    } else {
        println!("  kept {}", dir.display());
    }
    if bad > 0 || !problems.is_empty() || (blocked > 0 && args.flag(&["--strict"])) {
        Err(String::new())
    } else {
        Ok(())
    }
}
