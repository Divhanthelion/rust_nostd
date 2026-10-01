//! Implementations of the user-facing commands.

use std::path::PathBuf;

use crate::curriculum::{self, Exercise, Module, Step, MODULES};
use crate::drill;
use crate::markdown;
use crate::quiz;
use crate::reference;
use crate::runner::{self, Ctx, Verdict};
use crate::state::State;
use crate::term;
use crate::toolchain::{self, Toolchain};
use crate::workspace::Workspace;
use crate::Args;

pub type Res = Result<(), String>;

pub fn workspace() -> Result<Workspace, String> {
    Workspace::find().ok_or_else(|| {
        format!(
            "no workspace found here.\n\n  Create one with:  {}\n  (or cd into an existing one, or set NOSTD_WORKSPACE)",
            term::bold("nostd init")
        )
    })
}

pub fn load() -> Result<(Workspace, State), String> {
    let ws = workspace()?;
    let st = State::load(&ws.state_path());
    Ok((ws, st))
}

fn step_done(st: &State, s: &Step) -> bool {
    match s {
        Step::Lesson(m) => st.read.contains(&m.num),
        Step::Exercise(e) => st.done.contains(e.name),
        Step::Quiz(m) => st.quiz.contains_key(&m.num),
    }
}

pub fn next_step(st: &State) -> Option<Step> {
    curriculum::path().into_iter().find(|s| !step_done(st, s))
}

/// The exercise to act on: explicit name, else current, else next unfinished.
pub fn resolve_exercise(st: &State, name: Option<&str>) -> Result<&'static Exercise, String> {
    if let Some(n) = name {
        return curriculum::find_exercise(n).ok_or_else(|| {
            let s = curriculum::suggest(n);
            if s.is_empty() {
                format!("no exercise named `{n}` (see `nostd list`)")
            } else {
                format!("no exercise named `{n}`. Did you mean: {}?", s.join(", "))
            }
        });
    }
    if let Some(c) = st.current.as_deref().and_then(curriculum::find_exercise) {
        if !st.done.contains(c.name) {
            return Ok(c);
        }
    }
    curriculum::exercises()
        .find(|e| !st.done.contains(e.name))
        .ok_or_else(|| "every exercise is done! Name one explicitly to revisit it.".to_string())
}

fn counts(st: &State) -> (usize, usize, usize, usize, usize, usize) {
    let ex_total = curriculum::exercises().count();
    let ex_done = curriculum::exercises().filter(|e| st.done.contains(e.name)).count();
    let les_total = MODULES.iter().filter(|m| !m.lesson.is_empty()).count();
    let les_done = MODULES.iter().filter(|m| !m.lesson.is_empty() && st.read.contains(&m.num)).count();
    let quiz_total = MODULES.iter().filter(|m| m.quiz.is_some()).count();
    let quiz_done = MODULES.iter().filter(|m| m.quiz.is_some() && st.quiz_passed(m.num)).count();
    (ex_done, ex_total, les_done, les_total, quiz_done, quiz_total)
}

fn banner() {
    println!();
    println!(
        "  {}  {}",
        term::paint(" nostd ", "1;7;36"),
        term::bold("no_std Rust · from `core` to bare metal and automotive patterns")
    );
    println!();
}

fn describe_step(s: &Step) -> String {
    match s {
        Step::Lesson(m) => format!("{} lesson {:02}: {}", term::bold_cyan("▸"), m.num, term::bold(m.title)),
        Step::Exercise(e) => format!(
            "{} exercise {}: {}  {}",
            term::bold_cyan("▸"),
            term::bold(e.name),
            e.title,
            term::dim(&e.rel_path())
        ),
        Step::Quiz(m) => format!("{} quiz for module {:02}: {}", term::bold_cyan("▸"), m.num, term::bold(m.title)),
    }
}

pub fn status() -> Res {
    banner();
    let Some(ws) = Workspace::find() else {
        println!("  Welcome! This tool teaches you to write Rust without the standard library:");
        println!("  core & alloc, fixed-capacity data structures, panics, atomics, unsafe & MMIO,");
        println!("  freestanding Linux binaries, FFI with C, bare-metal Cortex-M boot code,");
        println!("  embedded-hal drivers, async executors, CAN, and functional-safety patterns.");
        println!();
        println!("  Start by creating a workspace (a folder with your exercise files):");
        println!();
        println!("      {}", term::bold("nostd init            # creates ./nostd-workspace"));
        println!("      {}", term::bold("cd nostd-workspace && nostd next"));
        println!();
        println!("  Check your toolchain first with {}.", term::bold("nostd doctor"));
        println!();
        return Ok(());
    };
    let st = State::load(&ws.state_path());
    let (ed, et, ld, lt, qd, qt) = counts(&st);
    println!(
        "  {}  {}  {}",
        term::dim("Progress"),
        term::progress_bar(ed, et, 30),
        term::bold(&format!("{ed}/{et} exercises"))
    );
    println!("            {}", term::dim(&format!("lessons read {ld}/{lt} · quizzes passed {qd}/{qt}")));
    println!();
    match next_step(&st) {
        Some(s) => {
            let m = match s {
                Step::Lesson(m) | Step::Quiz(m) => m,
                Step::Exercise(e) => e.module(),
            };
            println!("  {}  Module {:02} · {}", term::dim("Now     "), m.num, term::bold(m.title));
            println!("  {}  {}", term::dim("Next    "), describe_step(&s));
        }
        None => {
            println!("  {}", term::bold_green("You have completed the whole course. Outstanding."));
            println!("  Keep sharp with `nostd drill`, and start a real project with `nostd new`.");
        }
    }
    println!();
    println!("  {}", term::dim("nostd next   continue        nostd watch   auto-check on save    nostd hint   get unstuck"));
    println!("  {}", term::dim("nostd list   curriculum      nostd where X  is X in core/alloc?   nostd help   all commands"));
    println!();
    Ok(())
}

pub fn init(args: &Args) -> Res {
    let dir = PathBuf::from(args.pos(0).unwrap_or("nostd-workspace"));
    let tc = Toolchain::detect().ok();
    let existed = dir.join(".nostd").is_dir();
    let (ws, created) = Workspace::init(&dir, tc.as_ref())?;
    banner();
    if existed {
        println!("  Updated workspace at {} ({created} new exercise files).", term::bold(&ws.root.display().to_string()));
    } else {
        println!("  Created workspace at {}", term::bold(&ws.root.display().to_string()));
        println!("  {} exercise files, {} lessons, the course runtime, and rust-project.json.", created, MODULES.iter().filter(|m| !m.lesson.is_empty()).count());
    }
    if let Some(tc) = &tc {
        if let Err(e) = tc.core_sysroot(&ws.cache_dir()) {
            println!("  {} {e}", term::yellow("warning:"));
        }
    } else {
        println!("  {} rustc was not found; install Rust from https://rustup.rs", term::yellow("warning:"));
    }
    println!();
    println!("  Next steps:");
    if args.pos(0).is_some() || dir != std::path::Path::new(".") {
        println!("      {}", term::bold(&format!("cd {}", dir.display())));
    }
    println!("      {}       {}", term::bold("nostd doctor"), term::dim("check compilers, targets and QEMU"));
    println!("      {}         {}", term::bold("nostd next"), term::dim("start lesson 00"));
    println!();
    Ok(())
}

fn print_exercise_card(ws: &Workspace, st: &State, ex: &Exercise) {
    let m = ex.module();
    let width = term::text_width();
    println!();
    println!("  {} {} {}", term::paint(" EXERCISE ", "1;7;32"), term::bold(ex.name), term::bold(&format!("· {}", ex.title)));
    println!("  {}", term::dim(&format!("module {:02} · {} · {}", m.num, m.title, ex.kind_label())));
    let status = if st.done.contains(ex.name) { term::green("done ✓") } else { term::yellow("not done") };
    let hints = st.hints.get(ex.name).copied().unwrap_or(0);
    println!(
        "  {} {}   {} {}   {} {}/{}",
        term::dim("file"),
        term::cyan(&ws.exercise_file(ex).strip_prefix(&ws.root).unwrap_or(&ws.exercise_file(ex)).display().to_string()),
        term::dim("status"),
        status,
        term::dim("hints"),
        hints,
        ex.hints.len()
    );
    for (name, _) in ex.extra {
        println!("  {} {}", term::dim("also"), term::cyan(&ex.extra_rel_path(name)));
    }
    println!();
    let doc = leading_doc(ex.source);
    if !doc.is_empty() {
        print!("{}", markdown::render(&doc, width));
    }
    println!(
        "  {}  {}",
        term::dim("Edit the file, then"),
        term::bold(&format!("nostd run {}", ex.name)),
    );
    println!("  {}", term::dim(&format!("or `nostd watch` to re-check on every save · `nostd hint {}` when stuck", ex.name)));
    println!();
}

/// The `//!` block at the top of an exercise: its instructions.
pub fn leading_doc(src: &str) -> String {
    let mut out = String::new();
    for line in src.lines() {
        let t = line.trim_start();
        if let Some(rest) = t.strip_prefix("//!") {
            out.push_str(rest.strip_prefix(' ').unwrap_or(rest));
            out.push('\n');
        } else if t.is_empty() && out.is_empty() {
            continue;
        } else {
            break;
        }
    }
    out
}

pub fn next(args: &Args) -> Res {
    let (ws, mut st) = load()?;
    let Some(step) = next_step(&st) else {
        banner();
        println!("  {}", term::bold_green("Course complete!"));
        println!("  Ideas: `nostd drill` for interview practice, `nostd new cortex-m my-fw` for a real project.");
        return Ok(());
    };
    match step {
        Step::Lesson(m) => show_lesson(&ws, &mut st, m, args)?,
        Step::Exercise(e) => {
            ws.ensure_exercise(e)?;
            st.current = Some(e.name.to_string());
            st.save()?;
            print_exercise_card(&ws, &st, e);
        }
        Step::Quiz(m) => run_quiz(&mut st, m)?,
    }
    Ok(())
}

fn show_lesson(_ws: &Workspace, st: &mut State, m: &Module, args: &Args) -> Res {
    let width = term::text_width();
    let mut text = String::new();
    text.push('\n');
    text.push_str(&format!("  {}\n\n", term::dim(&format!("LESSON {:02} · {}", m.num, m.summary))));
    text.push_str(&markdown::render(m.lesson, width));
    let after: Vec<String> = m.exercises.iter().map(|e| format!("{} ({})", e.name, e.title)).collect();
    if !after.is_empty() {
        text.push_str(&format!("  {} {}\n", term::bold("Exercises for this module:"), after.join(", ")));
    }
    if m.quiz.is_some() {
        text.push_str(&format!("  {} nostd quiz {}\n", term::bold("Quiz:"), m.num));
    }
    text.push('\n');
    term::page(&text, !args.flag(&["--no-pager"]));
    st.read.insert(m.num);
    st.save()?;
    if let Some(s) = next_step(st) {
        println!("  {} {}", term::dim("next:"), describe_step(&s));
        println!("  {}", term::dim("run `nostd next` to continue"));
        println!();
    }
    Ok(())
}

pub fn list(args: &Args) -> Res {
    let (_ws, st) = load()?;
    banner();
    let only = args.opt(&["--module", "-m"]).or(args.pos(0)).and_then(curriculum::find_module);
    let current = st.current.clone();
    for m in MODULES {
        if let Some(o) = only {
            if o.num != m.num {
                continue;
            }
        }
        let lesson = if m.lesson.is_empty() {
            String::new()
        } else if st.read.contains(&m.num) {
            term::green("lesson ✓")
        } else {
            term::dim("lesson ·")
        };
        let quiz = match (m.quiz, st.quiz.get(&m.num)) {
            (None, _) => String::new(),
            (Some(_), None) => term::dim("quiz ·"),
            (Some(_), Some(&(c, t))) => {
                let s = format!("quiz {c}/{t}");
                if st.quiz_passed(m.num) { term::green(&format!("{s} ✓")) } else { term::yellow(&s) }
            }
        };
        let done = m.exercises.iter().filter(|e| st.done.contains(e.name)).count();
        let head = format!("{:02} {}", m.num, m.title);
        println!(
            "  {}  {}  {}  {}",
            term::pad(&term::bold(&head), 50),
            term::pad(&lesson, 9),
            term::pad(&quiz, 12),
            term::dim(&format!("{done}/{}", m.exercises.len()))
        );
        for e in m.exercises {
            let mark = if st.done.contains(e.name) {
                term::green("✓")
            } else if current.as_deref() == Some(e.name) {
                term::bold_cyan("▸")
            } else {
                term::dim("·")
            };
            println!("       {} {}  {}", mark, term::pad(e.name, 11), term::dim(e.title));
        }
    }
    println!();
    Ok(())
}

pub fn learn(args: &Args) -> Res {
    let (ws, mut st) = load()?;
    let m = match args.pos(0) {
        Some(q) => curriculum::find_module(q).ok_or_else(|| format!("no module matching `{q}` (try a number 0-{})", MODULES.len() - 1))?,
        None => match MODULES.iter().find(|m| !m.lesson.is_empty() && !st.read.contains(&m.num)) {
            Some(m) => m,
            None => {
                println!("  All lessons read. Re-read one with `nostd learn <number>`:");
                for m in MODULES.iter().filter(|m| !m.lesson.is_empty()) {
                    println!("    {:02}  {}", m.num, m.title);
                }
                return Ok(());
            }
        },
    };
    if m.lesson.is_empty() {
        return Err(format!("module {} has no lesson", m.num));
    }
    show_lesson(&ws, &mut st, m, args)
}

pub fn show(args: &Args) -> Res {
    let (ws, st) = load()?;
    let ex = resolve_exercise(&st, args.pos(0))?;
    ws.ensure_exercise(ex)?;
    print_exercise_card(&ws, &st, ex);
    Ok(())
}

pub fn check_and_record(ws: &Workspace, st: &mut State, ex: &'static Exercise, cross: Option<String>) -> Result<bool, String> {
    let tc = Toolchain::detect()?;
    let path = ws.ensure_exercise(ex)?;
    println!();
    println!(
        "  {} {} {}",
        term::bold("checking"),
        term::bold_cyan(ex.name),
        term::dim(&format!("· {} · {}", ex.title, ex.kind_label()))
    );
    let ctx = Ctx { ws, tc: &tc, quiet: false, cross_target: cross };
    let report = runner::check(&ctx, ex, &path);
    println!();
    match report.verdict {
        Verdict::Pass => {
            let first_time = st.done.insert(ex.name.to_string());
            let next_ex = curriculum::exercises().find(|e| !st.done.contains(e.name));
            st.current = next_ex.map(|e| e.name.to_string());
            st.save()?;
            println!("  {} {}", term::bold_green("✓ passed:"), term::bold(ex.title));
            if first_time {
                let (ed, et, ..) = counts(st);
                println!("  {} {}", term::progress_bar(ed, et, 30), term::dim(&format!("{ed}/{et} exercises")));
            }
            if let Some(s) = next_step(st) {
                println!("  {} {}", term::dim("next:"), describe_step(&s));
            }
            println!();
            Ok(true)
        }
        Verdict::Fail => {
            let used = st.hints.get(ex.name).copied().unwrap_or(0);
            let tip = if used < ex.hints.len() {
                format!("`nostd hint {}` reveals hint {}/{}", ex.name, used + 1, ex.hints.len())
            } else {
                format!("all hints revealed; `nostd learn {}` to re-read the lesson", ex.module().num)
            };
            println!("  {} {}", term::bold_yellow("✗ not yet."), term::dim(&format!("Fix the issue above and re-run. {tip}.")));
            println!();
            Ok(false)
        }
        Verdict::Blocked(why) => {
            println!("  {} {}", term::bold_yellow("⚠ can't check this here:"), why);
            println!();
            Ok(false)
        }
    }
}

pub fn run(args: &Args) -> Res {
    let (ws, mut st) = load()?;
    let ex = resolve_exercise(&st, args.pos(0))?;
    if st.current.as_deref() != Some(ex.name) && !st.done.contains(ex.name) {
        st.current = Some(ex.name.to_string());
        st.save()?;
    }
    let ok = check_and_record(&ws, &mut st, ex, args.opt(&["--target"]).map(String::from))?;
    if ok { Ok(()) } else { Err(String::new()) }
}

pub fn hint(args: &Args) -> Res {
    let (_ws, mut st) = load()?;
    let ex = resolve_exercise(&st, args.pos(0))?;
    let width = term::text_width();
    let used = st.hints.get(ex.name).copied().unwrap_or(0);
    let show_all = args.flag(&["--all", "-a"]);
    let upto = if show_all { ex.hints.len() } else { (used + 1).min(ex.hints.len()) };
    println!();
    if ex.hints.is_empty() {
        println!("  No hints for {} — re-read the lesson with `nostd learn {}`.", ex.name, ex.module().num);
        return Ok(());
    }
    for (i, h) in ex.hints.iter().enumerate().take(upto) {
        let fresh = i >= used;
        let label = format!("hint {}/{}", i + 1, ex.hints.len());
        let label = if fresh { term::bold_yellow(&label) } else { term::dim(&label) };
        println!("  {label}");
        print!("{}", markdown::render(h, width - 2));
    }
    if upto < ex.hints.len() {
        println!("  {}", term::dim(&format!("run `nostd hint {}` again for the next hint", ex.name)));
    } else {
        println!("  {}", term::dim("that's every hint. Still stuck? `nostd solution --force` shows the reference solution."));
    }
    println!();
    st.hints.insert(ex.name.to_string(), upto.max(used));
    st.save()
}

pub fn solution(args: &Args) -> Res {
    let (_ws, st) = load()?;
    let ex = match args.pos(0) {
        Some(n) => resolve_exercise(&st, Some(n))?,
        None => match st.current.as_deref().and_then(curriculum::find_exercise) {
            Some(e) => e,
            None => resolve_exercise(&st, None)?,
        },
    };
    if !st.done.contains(ex.name) && !args.flag(&["--force", "-f"]) {
        return Err(format!(
            "you haven't solved {} yet. Struggle is where the learning happens!\n  Try `nostd hint {}` first, or pass --force to see the solution anyway.",
            ex.name, ex.name
        ));
    }
    let width = term::text_width();
    let mut text = format!("\n  {} {} · {}\n\n", term::paint(" SOLUTION ", "1;7;33"), term::bold(ex.name), ex.title);
    text.push_str(&markdown::render_code("rust", ex.solution, width));
    text.push('\n');
    term::page(&text, !args.flag(&["--no-pager"]));
    Ok(())
}

pub fn reset(args: &Args) -> Res {
    let (ws, st) = load()?;
    let ex = resolve_exercise(&st, args.pos(0))?;
    if args.pos(0).is_none() {
        return Err("name the exercise to reset, e.g. `nostd reset core1`".into());
    }
    if !args.flag(&["--yes", "-y"]) {
        let a = term::prompt(&format!(
            "  Overwrite {} with the original? Your changes will be lost. [y/N] ",
            ex.rel_path()
        ))
        .unwrap_or_default();
        if !a.eq_ignore_ascii_case("y") && !a.eq_ignore_ascii_case("yes") {
            println!("  left unchanged.");
            return Ok(());
        }
    }
    let p = ws.reset_exercise(ex)?;
    println!("  restored {}", p.display());
    Ok(())
}

pub fn skip(args: &Args) -> Res {
    let (_ws, mut st) = load()?;
    let ex = resolve_exercise(&st, args.pos(0))?;
    st.done.insert(ex.name.to_string());
    let next_ex = curriculum::exercises().find(|e| !st.done.contains(e.name));
    st.current = next_ex.map(|e| e.name.to_string());
    st.save()?;
    println!("  marked {} as done (skipped). Come back any time with `nostd run {}`.", ex.name, ex.name);
    Ok(())
}

fn run_quiz(st: &mut State, m: &Module) -> Res {
    let Some(src) = m.quiz else {
        return Err(format!("module {} has no quiz", m.num));
    };
    let qs = quiz::parse(src).map_err(|e| format!("quiz for module {} is malformed: {e}", m.num))?;
    let title = format!("Module {:02} · {}", m.num, m.title);
    let Some((c, t)) = quiz::run(&title, &qs) else {
        println!("  quiz abandoned; nothing recorded.");
        return Ok(());
    };
    let pct = c * 100 / t.max(1);
    let best = st.quiz.get(&m.num).copied();
    if best.is_none_or(|(bc, _)| c > bc) {
        st.quiz.insert(m.num, (c, t));
    }
    st.save()?;
    if pct >= quiz::PASS_PERCENT {
        println!("  {} {c}/{t} ({pct}%). Passed.", term::bold_green("Score:"));
    } else {
        println!(
            "  {} {c}/{t} ({pct}%). Pass mark is {}%. Re-read with `nostd learn {}` and retry with `nostd quiz {}`.",
            term::bold_yellow("Score:"),
            quiz::PASS_PERCENT,
            m.num,
            m.num
        );
    }
    println!();
    Ok(())
}

pub fn quiz(args: &Args) -> Res {
    let (_ws, mut st) = load()?;
    let m = match args.pos(0) {
        Some(q) => curriculum::find_module(q).ok_or_else(|| format!("no module matching `{q}`"))?,
        None => MODULES
            .iter()
            .find(|m| m.quiz.is_some() && st.read.contains(&m.num) && !st.quiz_passed(m.num))
            .or_else(|| MODULES.iter().find(|m| m.quiz.is_some() && !st.quiz_passed(m.num)))
            .ok_or("all quizzes passed! Take one again with `nostd quiz <module>`.")?,
    };
    run_quiz(&mut st, m)
}

pub fn drill(args: &Args) -> Res {
    let cards = drill::parse(curriculum::DRILLS).map_err(|e| format!("drill file malformed: {e}"))?;
    if args.flag(&["--topics"]) {
        println!();
        for t in drill::topics(&cards) {
            let n = cards.iter().filter(|c| c.topic == t).count();
            println!("  {}  {}", term::pad(&term::bold(&t), 28), term::dim(&format!("{n} cards")));
        }
        println!();
        return Ok(());
    }
    let n = args.pos(0).and_then(|v| v.parse().ok()).unwrap_or(10);
    let mut st = match Workspace::find() {
        Some(ws) => State::load(&ws.state_path()),
        None => State::default(),
    };
    drill::run(&cards, &mut st, n, args.opt(&["--topic"]));
    Ok(())
}

pub fn where_(args: &Args) -> Res {
    if args.positional.is_empty() {
        return Err("usage: nostd where <item>   e.g. `nostd where HashMap`, `nostd where sqrt`, `nostd where Mutex`".into());
    }
    for q in &args.positional {
        let hits = reference::lookup(q);
        if hits.is_empty() {
            println!();
            println!("  {} isn't in the reference table.", term::bold(q));
            println!("  Rule of thumb: if it needs an OS (files, threads, time, stdio) it's std-only;");
            println!("  if it allocates it's in alloc; everything else is probably in core under the same path.");
        } else {
            reference::print(&hits);
        }
    }
    Ok(())
}

pub fn lsp(_args: &Args) -> Res {
    let ws = workspace()?;
    let tc = Toolchain::detect()?;
    let p = ws.write_rust_project(&tc)?;
    println!("  wrote {}", p.display());
    println!("  Open the workspace folder in your editor; rust-analyzer will treat each exercise as its own crate.");
    Ok(())
}

pub fn doctor(_args: &Args) -> Res {
    banner();
    let ok = |b: bool| if b { term::bold_green("✓") } else { term::bold_red("✗") };
    let warn = term::bold_yellow("!");
    let tc = match Toolchain::detect() {
        Ok(tc) => tc,
        Err(e) => {
            println!("  {} rustc: {e}", ok(false));
            return Err("install Rust with rustup (https://rustup.rs) and re-run `nostd doctor`".into());
        }
    };
    let (maj, min) = tc.minor_version();
    let recent = maj > 1 || min >= 85;
    println!("  {} {}  {}", ok(recent), tc.version, term::dim(&format!("host {}", tc.host)));
    if !recent {
        println!("      the course uses edition 2024, which needs Rust 1.85 or newer: `rustup update stable`");
    }
    println!("  {} sysroot {}", ok(tc.sysroot.exists()), term::dim(&tc.sysroot.display().to_string()));
    let cache = match Workspace::find() {
        Some(ws) => ws.cache_dir(),
        None => std::env::temp_dir().join("nostd-doctor"),
    };
    match tc.core_sysroot(&cache) {
        Ok(_) => println!("  {} core-only sysroot works {}", ok(true), term::dim("(std is physically absent when exercises are checked)")),
        Err(e) => println!("  {} core-only sysroot: {e}\n      library exercises fall back to a source-level check", warn),
    }
    let free = tc.can_run_freestanding();
    println!(
        "  {} freestanding Linux binaries {}",
        if free { ok(true) } else { warn.clone() },
        term::dim(if free { "(modules 09-10 run natively)" } else { "(not Linux x86_64/aarch64: modules 09-10 are type-checked only; use a Linux VM/WSL/container to run them)" })
    );
    let cc = toolchain::c_compiler();
    println!(
        "  {} C compiler {}",
        if cc.is_some() { ok(true) } else { warn.clone() },
        term::dim(&cc.map(|p| p.display().to_string()).unwrap_or_else(|| "not found: needed for module 11 (FFI) and for linking on Linux".into()))
    );
    let t = "thumbv7m-none-eabi";
    let has = tc.has_target(t);
    println!(
        "  {} target {t} {}",
        if has { ok(true) } else { warn.clone() },
        term::dim(if has { "(module 12 firmware)" } else { "missing: `rustup target add thumbv7m-none-eabi` (needed in module 12)" })
    );
    let q = toolchain::which("qemu-system-arm");
    println!(
        "  {} qemu-system-arm {}",
        if q.is_some() { ok(true) } else { warn.clone() },
        term::dim(if q.is_some() { "(firmware will actually boot)" } else { "optional: install QEMU to boot the Cortex-M exercise (apt install qemu-system-arm / brew install qemu)" })
    );
    let optional = ["thumbv6m-none-eabi", "thumbv7em-none-eabihf", "riscv32imac-unknown-none-elf", "aarch64-unknown-none", "x86_64-unknown-none"];
    let installed = toolchain::installed_targets(&tc);
    let extra: Vec<&str> = optional.iter().copied().filter(|t| installed.iter().any(|i| i == t)).collect();
    println!(
        "  {} extra bare-metal targets: {}",
        term::dim("·"),
        if extra.is_empty() { term::dim("none (optional: try `rustup target add thumbv6m-none-eabi` and `nostd run <ex> --target thumbv6m-none-eabi`)") } else { extra.join(", ") }
    );
    let ra = toolchain::which("rust-analyzer");
    println!(
        "  {} rust-analyzer {}",
        term::dim("·"),
        term::dim(if ra.is_some() { "found: run `nostd lsp` in your workspace for editor support" } else { "not on PATH (optional; most editors bundle it): `nostd lsp` writes rust-project.json" })
    );
    println!();
    Ok(())
}

pub fn help(cmd: Option<&str>) {
    banner();
    let w = term::text_width();
    let rows: &[(&str, &str)] = &[
        ("nostd", "Show progress and what to do next."),
        ("nostd init [DIR]", "Create a workspace (default ./nostd-workspace). Re-run to add new exercises after upgrading."),
        ("nostd next", "Open the next step: lesson, exercise or quiz."),
        ("nostd learn [MODULE]", "Read a lesson (default: the next unread one). MODULE is a number or name."),
        ("nostd list [MODULE]", "Show the curriculum and your progress."),
        ("nostd show [EXERCISE]", "Show an exercise's instructions and file."),
        ("nostd run [EXERCISE] [--target T]", "Check an exercise (default: the current one). --target cross-checks a library for a bare-metal target, e.g. thumbv6m-none-eabi."),
        ("nostd watch [EXERCISE]", "Re-check on every save and advance automatically. Type h(int), l(ist), r(un), q(uit) + Enter."),
        ("nostd hint [EXERCISE] [--all]", "Reveal the next hint."),
        ("nostd solution [EXERCISE] [--force]", "Show the reference solution (after you've passed, or with --force)."),
        ("nostd reset EXERCISE [--yes]", "Restore an exercise file to its original state."),
        ("nostd skip [EXERCISE]", "Mark an exercise as done without solving it."),
        ("nostd quiz [MODULE]", "Take a module quiz (pass mark 80%)."),
        ("nostd drill [N] [--topic T] [--topics]", "Interview flashcards with spaced repetition."),
        ("nostd where ITEM", "Is ITEM in core, alloc, std only, or a crate? With alternatives."),
        ("nostd new TEMPLATE NAME", "Scaffold a real Cargo project: lib, linux-bin, cortex-m, cortex-m-rt."),
        ("nostd doctor", "Check rustc, targets, C compiler and QEMU."),
        ("nostd lsp", "Write rust-project.json for rust-analyzer."),
        ("--no-color, --no-pager", "Global flags. NO_COLOR, NOSTD_PAGER=0 and PAGER are honoured too."),
    ];
    let filtered: Vec<&(&str, &str)> = match cmd {
        Some(c) => rows.iter().filter(|(u, _)| u.split_whitespace().nth(1) == Some(c)).collect(),
        None => rows.iter().collect(),
    };
    for (usage, desc) in filtered {
        println!("  {}", term::bold(usage));
        print!("{}", term::wrap_plain(desc, w.saturating_sub(4), "      "));
    }
    println!();
    if cmd.is_none() {
        println!("  {}", term::dim("Tip: `nostd <exercise>` is short for `nostd run <exercise>`."));
        println!();
    }
}

