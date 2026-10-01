//! Building and checking exercises.
//!
//! Every exercise kind is verified in a way that mirrors real no_std work:
//! libraries are compiled against a sysroot with no `std` in it, binaries are
//! linked without a C runtime and actually executed, FFI exercises are linked
//! into a real C program, and firmware is checked byte-by-byte and booted in
//! QEMU when it is installed.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use crate::curriculum::{self, Case, Exercise, Mode};
use crate::elf::{self, Elf};
use crate::term;
use crate::toolchain::{self, Toolchain};
use crate::workspace::Workspace;

pub struct Ctx<'a> {
    pub ws: &'a Workspace,
    pub tc: &'a Toolchain,
    /// Collect output instead of printing it (used by `nostd dev verify`).
    pub quiet: bool,
    /// Optional extra bare-metal target to cross-check libraries against.
    pub cross_target: Option<String>,
}

#[derive(Debug, PartialEq)]
pub enum Verdict {
    Pass,
    Fail,
    /// The check could not run here (missing target, tool, or OS support).
    Blocked(String),
}

pub struct Report {
    pub verdict: Verdict,
    pub log: String,
}

struct Log {
    quiet: bool,
    buf: String,
}

impl Log {
    fn say(&mut self, s: &str) {
        if !self.quiet {
            print!("{s}");
            let _ = std::io::stdout().flush();
        }
        self.buf.push_str(s);
    }
    fn line(&mut self, s: &str) {
        self.say(s);
        self.say("\n");
    }
}

pub struct Ran {
    pub status: Option<ExitStatus>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub timed_out: bool,
}

impl Ran {
    pub fn ok(&self) -> bool {
        self.status.is_some_and(|s| s.success())
    }
    pub fn code(&self) -> Option<i32> {
        self.status.and_then(|s| s.code())
    }
    pub fn stderr_text(&self) -> String {
        String::from_utf8_lossy(&self.stderr).to_string()
    }
    pub fn stdout_text(&self) -> String {
        String::from_utf8_lossy(&self.stdout).to_string()
    }
}

/// Run a command with optional stdin and a timeout, capturing output.
pub fn run_with_timeout(cmd: &mut Command, stdin: Option<&[u8]>, timeout: Duration) -> Result<Ran, String> {
    cmd.stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| format!("failed to start {:?}: {e}", cmd.get_program()))?;
    let input = stdin.map(|s| s.to_vec());
    let child_in = child.stdin.take();
    let t_in = std::thread::spawn(move || {
        if let (Some(mut pipe), Some(data)) = (child_in, input) {
            let _ = pipe.write_all(&data);
        }
    });
    let mut out_pipe = child.stdout.take();
    let mut err_pipe = child.stderr.take();
    let t_out = std::thread::spawn(move || {
        let mut v = Vec::new();
        if let Some(p) = out_pipe.as_mut() {
            let _ = p.read_to_end(&mut v);
        }
        v
    });
    let t_err = std::thread::spawn(move || {
        let mut v = Vec::new();
        if let Some(p) = err_pipe.as_mut() {
            let _ = p.read_to_end(&mut v);
        }
        v
    });
    let start = Instant::now();
    let mut timed_out = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break Some(st),
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    timed_out = true;
                    break None;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(e) => return Err(e.to_string()),
        }
    };
    let _ = t_in.join();
    let stdout = t_out.join().unwrap_or_default();
    let stderr = t_err.join().unwrap_or_default();
    Ok(Ran { status, stdout, stderr, timed_out })
}

const ALLOWED_LINTS: &[&str] = &["dead_code", "unused_variables", "unreachable_code", "unused_mut"];

impl Ctx<'_> {
    fn rustc(&self) -> Command {
        let mut c = Command::new(&self.tc.rustc);
        c.current_dir(&self.ws.root);
        c.arg("--edition=2024");
        if term::color_enabled() {
            c.arg("--color=always");
        }
        for l in ALLOWED_LINTS {
            c.arg("-A").arg(l);
        }
        c
    }

    fn core_sysroot(&self, log: &mut Log) -> Option<PathBuf> {
        match self.tc.core_sysroot(&self.ws.cache_dir()) {
            Ok(p) => Some(p),
            Err(e) => {
                log.line(&term::yellow(&format!(
                    "  note: could not build the core-only sysroot ({e}); falling back to a source check"
                )));
                None
            }
        }
    }

    /// Compile (if needed) the course runtime crate and return its rlib path.
    fn runtime_rlib(&self, log: &mut Log) -> Result<PathBuf, ()> {
        let dir = self.ws.build_dir("_nostd_rt");
        let src = dir.join("nostd_rt.rs");
        let rlib = dir.join("libnostd_rt.rlib");
        let stamp = dir.join("stamp");
        let stamp_val = format!("{}\n{}", self.tc.version, curriculum::RUNTIME_SOURCE.len());
        let fresh = rlib.exists()
            && fs::read_to_string(&src).is_ok_and(|s| s == curriculum::RUNTIME_SOURCE)
            && fs::read_to_string(&stamp).is_ok_and(|s| s == stamp_val);
        if fresh {
            return Ok(rlib);
        }
        let _ = fs::create_dir_all(&dir);
        if fs::write(&src, curriculum::RUNTIME_SOURCE).is_err() {
            log.line(&term::red("  could not write the runtime source"));
            return Err(());
        }
        let Some(core) = self.core_sysroot(log) else { return Err(()) };
        let mut c = self.rustc();
        c.args(["--crate-type=rlib", "--crate-name=nostd_rt"]).arg("--sysroot").arg(&core);
        bin_codegen_flags(&mut c);
        c.arg("-o").arg(&rlib).arg(&src);
        match run_with_timeout(&mut c, None, Duration::from_secs(120)) {
            Ok(r) if r.ok() => {
                let _ = fs::write(&stamp, stamp_val);
                Ok(rlib)
            }
            Ok(r) => {
                log.line(&term::red("  the course runtime (nostd_rt) failed to build:"));
                log.say(&r.stderr_text());
                Err(())
            }
            Err(e) => {
                log.line(&term::red(&format!("  {e}")));
                Err(())
            }
        }
    }
}

/// Build shared artefacts up front (the core-only sysroot and the runtime
/// crate) so parallel checks don't race to create them.
pub fn prepare(ctx: &Ctx) -> Result<(), String> {
    ctx.tc.core_sysroot(&ctx.ws.cache_dir())?;
    if ctx.tc.can_run_freestanding() {
        let mut log = Log { quiet: true, buf: String::new() };
        ctx.runtime_rlib(&mut log).map_err(|()| log.buf.clone())?;
    }
    Ok(())
}

fn bin_codegen_flags(c: &mut Command) {
    c.args([
        "-C", "panic=abort",
        "-C", "opt-level=s",
        "-C", "overflow-checks=on",
        "-C", "debuginfo=0",
        "-C", "relocation-model=static",
    ]);
}

fn stage_start(log: &mut Log, n: usize, total: usize, label: &str) {
    let tag = term::dim(&format!("[{n}/{total}]"));
    log.say(&format!("  {tag} {label} "));
    let w = 52usize.saturating_sub(label.chars().count());
    log.say(&term::dim(&".".repeat(w)));
    log.say(" ");
}

fn stage_ok(log: &mut Log, detail: &str) {
    if detail.is_empty() {
        log.line(&term::bold_green("ok"));
    } else {
        log.line(&format!("{} {}", term::bold_green("ok"), term::dim(detail)));
    }
}

fn stage_fail(log: &mut Log, detail: &str) {
    if detail.is_empty() {
        log.line(&term::bold_red("failed"));
    } else {
        log.line(&format!("{} {}", term::bold_red("failed"), term::dim(detail)));
    }
}

fn indent_block(text: &str) -> String {
    let mut out = String::new();
    for l in text.trim_end().lines() {
        out.push_str("    ");
        out.push_str(l);
        out.push('\n');
    }
    out
}

/// Check an exercise. The file at `src` is compiled (normally the learner's copy).
pub fn check(ctx: &Ctx, ex: &Exercise, src: &Path) -> Report {
    let mut log = Log { quiet: ctx.quiet, buf: String::new() };
    if !src.exists() {
        log.line(&term::red(&format!("  {} does not exist", src.display())));
        return Report { verdict: Verdict::Fail, log: log.buf };
    }
    let rel = src.strip_prefix(&ctx.ws.root).map(Path::to_path_buf).unwrap_or_else(|_| src.to_path_buf());
    let verdict = match &ex.mode {
        Mode::Lib => check_lib(ctx, ex, &rel, &[&[]], &mut log),
        Mode::LibFeatures(sets) => check_lib(ctx, ex, &rel, sets, &mut log),
        Mode::Bin { rt, cases } => check_bin(ctx, ex, &rel, *rt, cases, &mut log),
        Mode::CLib { harness, cases } => check_clib(ctx, ex, &rel, harness, cases, &mut log),
        Mode::CortexM { link, stdout } => check_cortex_m(ctx, ex, &rel, link, stdout, &mut log),
    };
    Report { verdict, log: log.buf }
}

fn feature_args(c: &mut Command, all: &[&str], set: &[&str]) {
    if !all.is_empty() {
        let values = all.iter().map(|f| format!("\"{f}\"")).collect::<Vec<_>>().join(", ");
        c.arg("--check-cfg").arg(format!("cfg(feature, values({values}))"));
    }
    for f in set {
        c.arg("--cfg").arg(format!("feature=\"{f}\""));
    }
}

fn check_lib(ctx: &Ctx, ex: &Exercise, rel: &Path, sets: &[&[&str]], log: &mut Log) -> Verdict {
    let dir = ctx.ws.build_dir(ex.name);
    let _ = fs::create_dir_all(&dir);
    let mut all: Vec<&str> = Vec::new();
    for s in sets {
        for f in s.iter() {
            if !all.contains(f) {
                all.push(f);
            }
        }
    }
    let total = sets.len() * 2 + usize::from(ctx.cross_target.is_some());
    let mut n = 0;
    for set in sets {
        let uses_std = set.contains(&"std");
        let label_feats = if all.is_empty() {
            String::new()
        } else if set.is_empty() {
            " [no features]".to_string()
        } else {
            format!(" [features: {}]", set.join(", "))
        };
        // Stage A: no_std build.
        n += 1;
        let label = if uses_std {
            format!("library build with std{label_feats}")
        } else {
            format!("no_std build (core + alloc only){label_feats}")
        };
        stage_start(log, n, total, &label);
        let mut c = ctx.rustc();
        c.args(["--crate-type=lib", "--emit=metadata"]).arg(format!("--crate-name={}", ex.name));
        let mut heuristic = false;
        if !uses_std {
            match ctx.core_sysroot(log) {
                Some(core) => {
                    c.arg("--sysroot").arg(core);
                }
                None => heuristic = true,
            }
        }
        feature_args(&mut c, &all, set);
        c.arg("--out-dir").arg(&dir).arg(rel);
        let r = match run_with_timeout(&mut c, None, Duration::from_secs(120)) {
            Ok(r) => r,
            Err(e) => {
                stage_fail(log, &e);
                return Verdict::Fail;
            }
        };
        if !r.ok() {
            stage_fail(log, "");
            log.say(&r.stderr_text());
            explain(log, &r.stderr_text());
            return Verdict::Fail;
        }
        if heuristic && source_uses_std(&fs::read_to_string(ctx.ws.root.join(rel)).unwrap_or_default()) {
            stage_fail(log, "found `extern crate std` outside #[cfg(test)]");
            return Verdict::Fail;
        }
        stage_ok(log, if heuristic { "(source check)" } else { "" });
        let warnings = r.stderr_text();
        if warnings.contains("warning") {
            log.say(&warnings);
        }

        // Stage B: host tests.
        n += 1;
        stage_start(log, n, total, &format!("host tests{label_feats}"));
        let test_bin = dir.join(if set.is_empty() { format!("{}-test", ex.name) } else { format!("{}-test-{}", ex.name, set.join("-")) });
        let mut c = ctx.rustc();
        c.arg("--test").arg(format!("--crate-name={}", ex.name)).arg("--cap-lints=allow");
        feature_args(&mut c, &all, set);
        c.arg("-o").arg(&test_bin).arg(rel);
        let r = match run_with_timeout(&mut c, None, Duration::from_secs(180)) {
            Ok(r) => r,
            Err(e) => {
                stage_fail(log, &e);
                return Verdict::Fail;
            }
        };
        if !r.ok() {
            stage_fail(log, "test build failed");
            log.say(&r.stderr_text());
            explain(log, &r.stderr_text());
            return Verdict::Fail;
        }
        let mut c = Command::new(&test_bin);
        c.current_dir(&ctx.ws.root).env("RUST_BACKTRACE", "0");
        if term::color_enabled() {
            c.arg("--color=always");
        }
        let r = match run_with_timeout(&mut c, None, Duration::from_secs(60)) {
            Ok(r) => r,
            Err(e) => {
                stage_fail(log, &e);
                return Verdict::Fail;
            }
        };
        let out = r.stdout_text();
        let plain = term::strip_ansi(&out);
        if r.timed_out {
            stage_fail(log, "timed out after 60s (deadlock or infinite loop?)");
            log.say(&indent_block(&out));
            return Verdict::Fail;
        }
        let (passed, failed) = count_tests(&plain);
        if !r.ok() || failed > 0 {
            stage_fail(log, &format!("{passed} passed, {failed} failed"));
            log.say(&trim_test_output(&out));
            let err = r.stderr_text();
            if !err.trim().is_empty() {
                log.say(&err);
            }
            explain(log, &format!("{plain}\n{err}"));
            return Verdict::Fail;
        }
        if passed == 0 {
            stage_fail(log, "no tests ran");
            return Verdict::Fail;
        }
        stage_ok(log, &format!("{passed} test{} passed", if passed == 1 { "" } else { "s" }));
        for l in plain.lines().filter(|l| l.starts_with("test ") && l.ends_with(" ok")) {
            let name = l.trim_start_matches("test ").trim_end_matches(" ... ok");
            log.line(&format!("        {} {}", term::green("✓"), term::dim(name)));
        }
    }
    if let Some(t) = &ctx.cross_target {
        n += 1;
        stage_start(log, n, total, &format!("cross-check for {t}"));
        if !ctx.tc.has_target(t) {
            stage_fail(log, &format!("target not installed: rustup target add {t}"));
            return Verdict::Fail;
        }
        let mut c = ctx.rustc();
        c.args(["--crate-type=lib", "--emit=metadata"])
            .arg(format!("--crate-name={}", ex.name))
            .arg(format!("--target={t}"))
            .arg("--out-dir")
            .arg(dir.join(t))
            .arg(rel);
        match run_with_timeout(&mut c, None, Duration::from_secs(120)) {
            Ok(r) if r.ok() => stage_ok(log, ""),
            Ok(r) => {
                stage_fail(log, "");
                log.say(&r.stderr_text());
                explain(log, &r.stderr_text());
                return Verdict::Fail;
            }
            Err(e) => {
                stage_fail(log, &e);
                return Verdict::Fail;
            }
        }
    }
    Verdict::Pass
}

fn source_uses_std(src: &str) -> bool {
    let lines: Vec<&str> = src.lines().collect();
    for (i, l) in lines.iter().enumerate() {
        if l.trim_start().starts_with("extern crate std") {
            let prev = if i > 0 { lines[i - 1] } else { "" };
            if !prev.contains("cfg(test)") && !l.contains("cfg(test)") {
                // inside a #[cfg(test)] mod? look backwards for an enclosing test module
                let in_test_mod = lines[..i].iter().rev().take(40).any(|p| p.contains("mod tests"));
                if !in_test_mod {
                    return true;
                }
            }
        }
    }
    false
}

fn count_tests(out: &str) -> (usize, usize) {
    let mut passed = 0;
    let mut failed = 0;
    for l in out.lines() {
        if l.starts_with("test ") {
            if l.ends_with(" ok") {
                passed += 1;
            } else if l.ends_with("FAILED") {
                failed += 1;
            }
        }
    }
    (passed, failed)
}

fn trim_test_output(out: &str) -> String {
    // Drop the noisy "running N tests" header and blank lines at the edges.
    let mut s = String::new();
    for l in out.lines() {
        let p = term::strip_ansi(l);
        if p.starts_with("running ") && p.ends_with("tests") || p.starts_with("running 1 test") {
            continue;
        }
        s.push_str("    ");
        s.push_str(l);
        s.push('\n');
    }
    s
}

fn show_case(case: &Case) -> String {
    let mut s = String::from("$ ./program");
    for a in case.args {
        if a.contains(' ') || a.is_empty() {
            s.push_str(&format!(" '{a}'"));
        } else {
            s.push(' ');
            s.push_str(a);
        }
    }
    if !case.stdin.is_empty() {
        let preview: String = case.stdin.chars().take(40).collect();
        let more = if case.stdin.chars().count() > 40 { "…" } else { "" };
        s.push_str(&format!("  < {:?}{more}", preview));
    }
    s
}

fn run_cases(ctx: &Ctx, prog: &Path, cases: &[Case], log: &mut Log, n: &mut usize, total: usize) -> bool {
    let mut all_ok = true;
    for case in cases {
        *n += 1;
        stage_start(log, *n, total, &format!("run case {}", *n - (total - cases.len())));
        let mut c = Command::new(prog);
        c.current_dir(&ctx.ws.root).args(case.args);
        let ran = match run_with_timeout(&mut c, Some(case.stdin.as_bytes()), Duration::from_secs(10)) {
            Ok(r) => r,
            Err(e) => {
                stage_fail(log, &e);
                return false;
            }
        };
        let stdout = ran.stdout_text();
        let stderr = ran.stderr_text();
        let code = ran.code();
        let mut problems = Vec::new();
        if ran.timed_out {
            problems.push("timed out after 10s (waiting for input? infinite loop?)".to_string());
        } else if code.is_none() {
            problems.push(signal_description(&ran));
        } else if code != Some(case.exit) {
            problems.push(format!("exit code {} (expected {})", code.unwrap_or(-1), case.exit));
        }
        if !case.stdout.matches(&stdout) {
            problems.push(format!("stdout was {:?}\n      expected {}", stdout, case.stdout.describe()));
        }
        if !case.stderr.matches(&stderr) {
            problems.push(format!("stderr was {:?}\n      expected {}", stderr, case.stderr.describe()));
        }
        if problems.is_empty() {
            stage_ok(log, "");
            log.line(&format!("        {}", term::dim(&show_case(case))));
        } else {
            all_ok = false;
            stage_fail(log, "");
            log.line(&format!("    {}", term::bold(&show_case(case))));
            for p in problems {
                log.line(&format!("    {} {}", term::red("✗"), p));
            }
            if !stderr.is_empty() && !matches!(case.stderr, crate::curriculum::Expect::Any) {
                // already shown
            } else if !stderr.is_empty() {
                log.line(&format!("    stderr: {:?}", stderr));
            }
        }
    }
    all_ok
}

fn signal_description(r: &Ran) -> String {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(sig) = r.status.and_then(|s| s.signal()) {
            let what = match sig {
                4 => "SIGILL: illegal instruction (hit a `ud2`/`brk`, or returned from `_start`)",
                5 => "SIGTRAP: trap instruction reached",
                6 => "SIGABRT: aborted",
                7 => "SIGBUS: bus error (misaligned or invalid memory access)",
                8 => "SIGFPE: arithmetic exception (division by zero in asm?)",
                11 => "SIGSEGV: segmentation fault (bad pointer, stack overflow, or infinite recursion such as a memcpy that calls itself)",
                _ => "killed by a signal",
            };
            return format!("killed by signal {sig} — {what}");
        }
    }
    let _ = r;
    "terminated abnormally".to_string()
}

fn check_bin(ctx: &Ctx, ex: &Exercise, rel: &Path, rt: bool, cases: &[Case], log: &mut Log) -> Verdict {
    let dir = ctx.ws.build_dir(ex.name);
    let _ = fs::create_dir_all(&dir);
    let can_run = ctx.tc.can_run_freestanding();
    let total = 1 + if can_run { cases.len() } else { 0 };
    let mut n = 1;
    stage_start(log, n, total, "link freestanding executable (no libc, no crt0)");
    let Some(core) = ctx.core_sysroot(log) else {
        stage_fail(log, "no core-only sysroot");
        return Verdict::Fail;
    };
    let rt_rlib = if rt {
        match ctx.runtime_rlib(log) {
            Ok(p) => Some(p),
            Err(()) => return Verdict::Fail,
        }
    } else {
        None
    };
    let mut c = ctx.rustc();
    c.args(["--crate-type=bin"]).arg(format!("--crate-name={}", ex.name)).arg("--sysroot").arg(&core);
    bin_codegen_flags(&mut c);
    if let Some(r) = &rt_rlib {
        c.arg("-L").arg(r.parent().unwrap_or(Path::new(".")));
        c.arg("--extern").arg(format!("nostd_rt={}", r.display()));
    }
    let prog = dir.join(ex.name);
    if can_run {
        c.args(["-C", "link-arg=-nostartfiles", "-C", "link-arg=-static"]);
        c.arg("-o").arg(&prog);
    } else {
        // Can't link a Linux freestanding binary here: type-check it instead.
        c.arg("--emit=metadata").arg("--out-dir").arg(&dir);
    }
    c.arg(rel);
    let r = match run_with_timeout(&mut c, None, Duration::from_secs(180)) {
        Ok(r) => r,
        Err(e) => {
            stage_fail(log, &e);
            return Verdict::Fail;
        }
    };
    if !r.ok() {
        stage_fail(log, "");
        log.say(&r.stderr_text());
        explain(log, &r.stderr_text());
        return Verdict::Fail;
    }
    let size = fs::metadata(&prog).map(|m| m.len()).unwrap_or(0);
    stage_ok(log, &if size > 0 { format!("{} bytes", size) } else { String::new() });
    let warnings = r.stderr_text();
    if warnings.contains("warning") {
        log.say(&warnings);
    }
    if !can_run {
        log.line(&term::yellow(&format!(
            "  note: freestanding Linux binaries can only be linked and run on Linux x86_64/aarch64 (host: {}).\n        Your code type-checks; run it for real inside a Linux VM, container or WSL.",
            ctx.tc.host
        )));
        return Verdict::Pass;
    }
    if run_cases(ctx, &prog, cases, log, &mut n, total) { Verdict::Pass } else { Verdict::Fail }
}

fn check_clib(ctx: &Ctx, ex: &Exercise, rel: &Path, harness: &str, cases: &[Case], log: &mut Log) -> Verdict {
    let dir = ctx.ws.build_dir(ex.name);
    let _ = fs::create_dir_all(&dir);
    let total = 2 + cases.len();
    let mut n = 1;
    stage_start(log, n, total, "build no_std staticlib");
    let Some(core) = ctx.core_sysroot(log) else {
        stage_fail(log, "no core-only sysroot");
        return Verdict::Fail;
    };
    let lib = dir.join(format!("lib{}.a", ex.name));
    let mut c = ctx.rustc();
    c.args(["--crate-type=staticlib"]).arg(format!("--crate-name={}", ex.name)).arg("--sysroot").arg(&core);
    c.args(["-C", "panic=abort", "-C", "opt-level=s", "-C", "overflow-checks=on", "-C", "debuginfo=0"]);
    c.arg("-o").arg(&lib).arg(rel);
    let r = match run_with_timeout(&mut c, None, Duration::from_secs(180)) {
        Ok(r) => r,
        Err(e) => {
            stage_fail(log, &e);
            return Verdict::Fail;
        }
    };
    if !r.ok() {
        stage_fail(log, "");
        log.say(&r.stderr_text());
        explain(log, &r.stderr_text());
        return Verdict::Fail;
    }
    stage_ok(log, "");
    let warnings = r.stderr_text();
    if warnings.contains("warning") {
        log.say(&warnings);
    }
    n += 1;
    stage_start(log, n, total, &format!("link C harness ({harness})"));
    let Some(cc) = toolchain::c_compiler() else {
        stage_fail(log, "no C compiler");
        return Verdict::Blocked("this exercise needs a C compiler (cc, gcc or clang) on PATH".into());
    };
    let Some(src) = ex.extra_file(harness) else {
        stage_fail(log, "harness missing from the course");
        return Verdict::Fail;
    };
    let c_path = dir.join(harness);
    let _ = fs::write(&c_path, src);
    let prog = dir.join(ex.name);
    let mut c = Command::new(&cc);
    c.current_dir(&ctx.ws.root).args(["-std=c11", "-Wall", "-Wextra", "-o"]).arg(&prog).arg(&c_path).arg(&lib);
    if cfg!(target_os = "linux") {
        c.arg("-lm");
    }
    let r = match run_with_timeout(&mut c, None, Duration::from_secs(120)) {
        Ok(r) => r,
        Err(e) => {
            stage_fail(log, &e);
            return Verdict::Fail;
        }
    };
    if !r.ok() {
        stage_fail(log, "");
        log.say(&r.stderr_text());
        explain(log, &r.stderr_text());
        return Verdict::Fail;
    }
    stage_ok(log, "");
    if run_cases(ctx, &prog, cases, log, &mut n, total) { Verdict::Pass } else { Verdict::Fail }
}

const CORTEX_M_TARGET: &str = "thumbv7m-none-eabi";

fn check_cortex_m(ctx: &Ctx, ex: &Exercise, rel: &Path, link: &str, expect: &str, log: &mut Log) -> Verdict {
    let dir = ctx.ws.build_dir(ex.name);
    let _ = fs::create_dir_all(&dir);
    let qemu = toolchain::which("qemu-system-arm");
    let total = 2 + usize::from(qemu.is_some());
    let mut n = 1;
    stage_start(log, n, total, &format!("build firmware for {CORTEX_M_TARGET}"));
    if !ctx.tc.has_target(CORTEX_M_TARGET) {
        stage_fail(log, "target not installed");
        log.line(&format!(
            "    This exercise cross-compiles for a Cortex-M3. Install the target once:\n\n        {}\n",
            term::bold(&format!("rustup target add {CORTEX_M_TARGET}"))
        ));
        return Verdict::Blocked(format!("rustup target add {CORTEX_M_TARGET}"));
    }
    let Some(script) = ex.extra_file(link) else {
        stage_fail(log, "linker script missing from the course");
        return Verdict::Fail;
    };
    let script_path = dir.join(link);
    let _ = fs::write(&script_path, script);
    let elf_path = dir.join(format!("{}.elf", ex.name));
    let mut c = ctx.rustc();
    c.args(["--crate-type=bin"])
        .arg(format!("--crate-name={}", ex.name))
        .arg(format!("--target={CORTEX_M_TARGET}"))
        .args(["-C", "panic=abort", "-C", "opt-level=s", "-C", "overflow-checks=on"])
        .arg("-C")
        .arg(format!("link-arg=-T{}", script_path.display()))
        .arg("-o")
        .arg(&elf_path)
        .arg(rel);
    let r = match run_with_timeout(&mut c, None, Duration::from_secs(180)) {
        Ok(r) => r,
        Err(e) => {
            stage_fail(log, &e);
            return Verdict::Fail;
        }
    };
    if !r.ok() {
        stage_fail(log, "");
        log.say(&r.stderr_text());
        explain(log, &r.stderr_text());
        return Verdict::Fail;
    }
    stage_ok(log, &format!("{} bytes", fs::metadata(&elf_path).map(|m| m.len()).unwrap_or(0)));

    n += 1;
    stage_start(log, n, total, "inspect vector table & entry point");
    let elf = match fs::read(&elf_path).map_err(|e| e.to_string()).and_then(Elf::parse) {
        Ok(e) => e,
        Err(e) => {
            stage_fail(log, &e);
            return Verdict::Fail;
        }
    };
    let problems = inspect_vector_table(&elf);
    if !problems.is_empty() {
        stage_fail(log, "");
        for p in &problems {
            log.line(&format!("    {} {p}", term::red("✗")));
        }
        return Verdict::Fail;
    }
    let sp = elf.read_u32(0).unwrap_or(0);
    let reset = elf.read_u32(4).unwrap_or(0);
    stage_ok(log, &format!("SP={sp:#010x} Reset={reset:#010x}"));

    let Some(qemu) = qemu else {
        log.line(&term::yellow(
            "  note: qemu-system-arm not found, so the firmware was not booted.\n        Install QEMU (e.g. `apt install qemu-system-arm`, `brew install qemu`) to run it.",
        ));
        return Verdict::Pass;
    };
    n += 1;
    stage_start(log, n, total, "boot in QEMU (lm3s6965evb, Cortex-M3)");
    let mut c = Command::new(qemu);
    c.args([
        "-cpu", "cortex-m3", "-machine", "lm3s6965evb", "-nographic", "-monitor", "none", "-serial", "none",
        "-semihosting-config", "enable=on,target=native", "-kernel",
    ])
    .arg(&elf_path);
    let r = match run_with_timeout(&mut c, None, Duration::from_secs(10)) {
        Ok(r) => r,
        Err(e) => {
            stage_fail(log, &e);
            return Verdict::Fail;
        }
    };
    let out = r.stdout_text();
    if r.timed_out {
        stage_fail(log, "no exit within 10s");
        log.line(&format!("    output so far: {:?}", out));
        log.line("    Did your firmware call the semihosting exit, or spin in a loop?");
        return Verdict::Fail;
    }
    if !r.ok() || out != expect {
        stage_fail(log, &format!("exit code {}", r.code().unwrap_or(-1)));
        log.line(&format!("    output:   {:?}\n    expected: {:?}", out, expect));
        let err: String = r.stderr_text().lines().filter(|l| !l.contains("Timer with period zero")).collect::<Vec<_>>().join("\n");
        if !err.trim().is_empty() {
            log.line(&format!("    qemu: {err}"));
        }
        return Verdict::Fail;
    }
    stage_ok(log, "");
    for l in out.lines() {
        log.line(&format!("        {} {}", term::dim("│"), l));
    }
    Verdict::Pass
}

fn inspect_vector_table(elf: &Elf) -> Vec<String> {
    let mut p = Vec::new();
    if elf.is64 || elf.machine != elf::EM_ARM {
        p.push("the output is not a 32-bit ARM ELF".into());
        return p;
    }
    let Some(vt) = elf.section(".vector_table") else {
        p.push("no `.vector_table` section in the output (did the linker script keep your table?)".into());
        return p;
    };
    if vt.addr != 0 {
        p.push(format!(".vector_table is at {:#x}; the Cortex-M boots from address 0x0", vt.addr));
    }
    let Some(sp) = elf.read_u32(0) else {
        p.push("nothing readable at address 0x0".into());
        return p;
    };
    let reset = elf.read_u32(4).unwrap_or(0);
    if let Some(stack) = elf.symbol("_stack_start") {
        if u64::from(sp) != stack.value {
            p.push(format!("word 0 (initial stack pointer) is {sp:#x}, expected _stack_start = {:#x}", stack.value));
        }
    }
    if reset == 0 {
        p.push("word 1 (reset vector) is zero".into());
    } else if reset & 1 == 0 {
        p.push(format!("reset vector {reset:#x} has the Thumb bit (bit 0) clear: the CPU would fault"));
    }
    match elf.symbol("Reset") {
        Some(sym) => {
            if u64::from(reset & !1) != sym.value & !1 {
                p.push(format!("reset vector {reset:#x} does not point at `Reset` ({:#x})", sym.value));
            }
        }
        None => p.push("no symbol named `Reset` (use #[unsafe(no_mangle)] on your reset handler)".into()),
    }
    if elf.entry != u64::from(reset) && reset != 0 {
        p.push(format!("ELF entry point {:#x} differs from the reset vector {reset:#x} (ENTRY(Reset) in the linker script)", elf.entry));
    }
    if let Some(hf) = elf.symbol("HardFault") {
        let v = elf.read_u32(12).unwrap_or(0);
        if u64::from(v & !1) != hf.value & !1 || v & 1 == 0 {
            p.push(format!("vector 3 (HardFault) is {v:#x}, expected the address of `HardFault` with the Thumb bit"));
        }
    }
    p
}

// ---------------------------------------------------------------------------
// Explanations for common no_std errors

struct Explainer {
    needles: &'static [&'static str],
    text: &'static str,
}

const EXPLAINERS: &[Explainer] = &[
    Explainer {
        needles: &["can't find crate for `std`"],
        text: "`std` does not exist here. Exercises are built against a sysroot holding only core, alloc and compiler_builtins, which is what a bare-metal target ships. Keep `#![no_std]` at the top of the file and only write `extern crate std;` inside #[cfg(test)] code.",
    },
    Explainer {
        needles: &["unlinked crate `std`", "undeclared crate or module `std`", "could not find `std`"],
        text: "`std::` paths don't resolve in a #![no_std] crate. Most items live at the same path in `core::` (core::fmt, core::mem, core::cmp…). Heap types live in `alloc::` after `extern crate alloc;`. Try `nostd where <Item>`.",
    },
    Explainer {
        needles: &["cannot find macro `println`", "cannot find macro `print`", "cannot find macro `eprintln`", "cannot find macro `eprint`", "cannot find macro `dbg`"],
        text: "println!/print!/eprintln!/dbg! belong to std because they need an OS-provided stdout. In no_std you write to anything implementing core::fmt::Write using write!/writeln!.",
    },
    Explainer {
        needles: &["cannot find macro `vec`", "cannot find macro `format`"],
        text: "vec! and format! live in the alloc crate: add `extern crate alloc;` and call `alloc::vec!`/`alloc::format!` (or `use alloc::{vec, format};`).",
    },
    Explainer {
        needles: &["cannot find type `Vec`", "cannot find type `String`", "cannot find type `Box`", "cannot find type `Rc`", "cannot find type `Arc`", "cannot find type `BTreeMap`", "cannot find struct, variant or union type `String`", "cannot find struct, variant or union type `Vec`"],
        text: "Heap types are not in the core prelude. Either use alloc (`extern crate alloc; use alloc::vec::Vec;`) or a fixed-capacity alternative (arrays, slices, heapless-style containers).",
    },
    Explainer {
        needles: &["cannot find type `HashMap`", "cannot find type `HashSet`"],
        text: "HashMap/HashSet live only in std (their default hasher needs OS randomness). Use alloc::collections::BTreeMap/BTreeSet, the hashbrown crate, or a fixed-capacity map.",
    },
    Explainer {
        needles: &["#[panic_handler]` function required", "`#[panic_handler]` function required"],
        text: "Every no_std *final artifact* (binary or staticlib) must define exactly one `#[panic_handler] fn panic(info: &core::panic::PanicInfo) -> !`. Libraries never define it: the final crate chooses the policy.",
    },
    Explainer {
        needles: &["found duplicate lang item `panic_impl`"],
        text: "Two panic handlers were linked: you defined #[panic_handler] while another crate (std, or another no_std crate) also provides one.",
    },
    Explainer {
        needles: &["undefined symbol: memcpy", "undefined symbol: memset", "undefined symbol: memmove", "undefined symbol: memcmp", "undefined symbol: bcmp", "undefined reference to `memcpy'", "undefined reference to `memset'", "undefined reference to `memcmp'", "undefined reference to `memmove'", "undefined reference to `bcmp'"],
        text: "The compiler assumes the platform provides memcpy, memmove, memset, memcmp and bcmp; it emits calls to them for copies, zeroing and comparisons. Normally libc provides them. Without libc you must (bare-metal targets get them from compiler_builtins). Define them with C signatures, in a crate marked #![no_builtins] so LLVM cannot turn your loops back into calls to themselves.",
    },
    Explainer {
        needles: &["undefined symbol: rust_eh_personality", "undefined reference to `rust_eh_personality'"],
        text: "The precompiled `core` for this host was built with unwinding, so its unwind tables reference `rust_eh_personality`. With panic=abort nothing ever unwinds; an empty #[unsafe(no_mangle)] extern \"C\" fn rust_eh_personality() {} satisfies the linker (bare-metal targets don't need this).",
    },
    Explainer {
        needles: &["undefined symbol: strlen", "undefined reference to `strlen'"],
        text: "CStr::from_ptr calls the C function strlen to find the terminating NUL. Without libc you must provide `strlen` yourself (or scan for the 0 byte manually).",
    },
    Explainer {
        needles: &["undefined symbol: _start", "cannot find entry symbol _start", "entry symbol _start"],
        text: "Without crt0 (-nostartfiles), nothing defines the ELF entry point. Define `_start` yourself: the kernel jumps there with the stack pointer at argc.",
    },
    Explainer {
        needles: &["`main` function not found", "E0601"],
        text: "A #![no_std] binary has no Rust runtime to call `main`. Add #![no_main] and provide the real entry point yourself (`_start` on Linux, the reset handler on a microcontroller).",
    },
    Explainer {
        needles: &["unwinding panics are not supported without std"],
        text: "no_std binaries must use panic=abort (Cargo: `[profile.dev] panic = \"abort\"`; rustc: `-C panic=abort`).",
    },
    Explainer {
        needles: &["reference to mutable static", "shared reference to mutable static", "mutable reference to mutable static"],
        text: "Edition 2024 forbids creating references to a `static mut` (the `static_mut_refs` lint), because any second reference makes it instant UB. Use `&raw const X`/`&raw mut X` with explicit unsafe reads/writes, an atomic, or a Mutex/critical-section wrapper.",
    },
    Explainer {
        needles: &["extern blocks must be unsafe"],
        text: "Edition 2024 requires `unsafe extern \"C\" { ... }`: declaring foreign items is a promise the compiler cannot check. Mark individual items `safe fn` if calling them can never cause UB.",
    },
    Explainer {
        needles: &["unsafe attribute used without unsafe", "usage of unsafe attribute"],
        text: "Edition 2024 marks link-affecting attributes as unsafe: write #[unsafe(no_mangle)], #[unsafe(export_name = \"..\")], #[unsafe(link_section = \"..\")].",
    },
    Explainer {
        needles: &["no method named `sqrt`", "no method named `sin`", "no method named `cos`", "no method named `powf`", "no method named `powi`", "no method named `floor`", "no method named `ceil`", "no method named `round`", "no method named `exp`", "no method named `ln`", "no method named `trunc`", "no method named `tan`", "no method named `atan2`"],
        text: "Most float math (sqrt, sin, floor, powi…) lives in std because it calls the platform libm. In no_std use the `libm` crate, integer/fixed-point math, or write the routine yourself. (abs, signum, copysign, min/max and clamp are available in core.)",
    },
    Explainer {
        needles: &["not yet implemented"],
        text: "Some functions still contain `todo!()`, which panics with \"not yet implemented\". Replace them with real code.",
    },
    Explainer {
        needles: &["with overflow"],
        text: "Integer overflow panics when overflow checks are on (debug builds; this course also enables them in optimized binaries). Choose the semantics explicitly: checked_*, wrapping_*, saturating_* or overflowing_*.",
    },
    Explainer {
        needles: &["index out of bounds"],
        text: "Indexing past the end of a slice/array panics. Use `.get(i)`, iterators, `split_at`/`chunks`, or check lengths first, especially in code that must never panic.",
    },
    Explainer {
        needles: &["cannot be shared between threads safely"],
        text: "A `static` must be `Sync`: it is reachable from every thread and every interrupt handler. Wrap mutable state in an atomic, a Mutex, a critical-section cell, or a type with an `unsafe impl Sync` whose safety argument you can actually make.",
    },
    Explainer {
        needles: &["is unsafe and requires unsafe block", "requires unsafe function or block"],
        text: "Calling an unsafe fn, dereferencing a raw pointer, or touching a `static mut` needs an `unsafe { }` block. In edition 2024 this also applies *inside* an `unsafe fn` (unsafe_op_in_unsafe_fn), so each unsafe operation gets its own block and SAFETY comment.",
    },
    Explainer {
        needles: &["`extern crate alloc` is", "use of unresolved module or unlinked crate `alloc`", "undeclared crate or module `alloc`"],
        text: "`alloc` is not linked automatically in a no_std crate: add `extern crate alloc;` at the crate root.",
    },
];

fn explain(log: &mut Log, output: &str) {
    let plain = term::strip_ansi(output);
    let mut shown = 0;
    for e in EXPLAINERS {
        if e.needles.iter().any(|n| plain.contains(n)) {
            if shown == 0 {
                log.line("");
            }
            log.say(&format!("  {} ", term::bold_cyan("why?")));
            let wrapped = term::wrap_plain(e.text, term::text_width().saturating_sub(2), "       ");
            log.say(wrapped.trim_start());
            shown += 1;
            if shown >= 3 {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_libtest_output() {
        let out = "running 3 tests\ntest a ... ok\ntest b ... FAILED\ntest c ... ok\n";
        assert_eq!(count_tests(out), (2, 1));
    }

    #[test]
    fn detects_std_use() {
        assert!(source_uses_std("#![no_std]\nextern crate std;\n"));
        assert!(!source_uses_std("#![no_std]\n#[cfg(test)]\nextern crate std;\n"));
        assert!(!source_uses_std("#![no_std]\n#[cfg(test)]\nmod tests {\n    extern crate std;\n}\n"));
    }
}
