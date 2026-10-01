//! The course content: modules, lessons, exercises and quizzes.
//!
//! Everything is embedded in the binary with `include_str!`, so the tool works
//! offline and can always restore an exercise to its original state.

mod registry;

pub use registry::MODULES;

pub struct Module {
    pub num: u8,
    pub slug: &'static str,
    pub title: &'static str,
    /// One-line description used in listings.
    pub summary: &'static str,
    /// Lesson text (Markdown). Empty if the module has no lesson.
    pub lesson: &'static str,
    pub exercises: &'static [Exercise],
    /// Quiz source (see `quiz.rs` for the format).
    pub quiz: Option<&'static str>,
}

impl Module {
    pub fn dir(&self) -> String {
        format!("{:02}_{}", self.num, self.slug)
    }
}

pub struct Exercise {
    pub name: &'static str,
    pub title: &'static str,
    /// Directory under `exercises/`, e.g. `01_landscape`.
    pub dir: &'static str,
    pub source: &'static str,
    pub solution: &'static str,
    /// Extra files written next to the exercise: (file name, contents).
    pub extra: &'static [(&'static str, &'static str)],
    pub mode: Mode,
    pub hints: &'static [&'static str],
}

impl Exercise {
    pub fn rel_path(&self) -> String {
        format!("exercises/{}/{}.rs", self.dir, self.name)
    }

    pub fn extra_rel_path(&self, file: &str) -> String {
        format!("exercises/{}/{}", self.dir, file)
    }

    pub fn module(&self) -> &'static Module {
        MODULES
            .iter()
            .find(|m| m.exercises.iter().any(|e| e.name == self.name))
            .expect("every exercise belongs to a module")
    }

    pub fn extra_file(&self, name: &str) -> Option<&'static str> {
        self.extra.iter().find(|(n, _)| *n == name).map(|(_, c)| *c)
    }

    pub fn kind_label(&self) -> &'static str {
        match self.mode {
            Mode::Lib => "no_std library + host tests",
            Mode::LibFeatures(_) => "portable library, built with several feature sets",
            Mode::Bin { .. } => "freestanding Linux executable",
            Mode::CLib { .. } => "no_std staticlib called from C",
            Mode::CortexM { .. } => "bare-metal Cortex-M3 firmware (QEMU)",
        }
    }
}

pub enum Mode {
    /// A `#![no_std]` library. Stage 1 builds it against a sysroot that only
    /// contains `core`, `alloc` and `compiler_builtins`; stage 2 runs its
    /// `#[cfg(test)]` tests on the host.
    Lib,
    /// Like `Lib`, once per feature set. Sets containing `"std"` are built
    /// against the full sysroot.
    LibFeatures(&'static [&'static [&'static str]]),
    /// A freestanding (`#![no_main]`) Linux executable. When `rt` is true the
    /// course runtime crate `nostd_rt` is available via `extern crate nostd_rt`.
    Bin { rt: bool, cases: &'static [Case] },
    /// A `#![no_std]` staticlib linked into a C harness program.
    CLib { harness: &'static str, cases: &'static [Case] },
    /// Bare-metal firmware for `thumbv7m-none-eabi`, linked with the given
    /// linker script, checked structurally and run in QEMU when available.
    CortexM { link: &'static str, stdout: &'static str },
}

pub struct Case {
    pub args: &'static [&'static str],
    /// Extra environment variables for the program.
    pub env: &'static [(&'static str, &'static str)],
    pub stdin: &'static str,
    pub stdout: Expect,
    pub stderr: Expect,
    pub exit: i32,
}

pub enum Expect {
    Any,
    Exact(&'static str),
    Contains(&'static [&'static str]),
}

impl Expect {
    pub fn matches(&self, actual: &str) -> bool {
        match self {
            Expect::Any => true,
            Expect::Exact(s) => actual == *s,
            Expect::Contains(parts) => parts.iter().all(|p| actual.contains(p)),
        }
    }

    pub fn describe(&self) -> String {
        match self {
            Expect::Any => "(anything)".into(),
            Expect::Exact(s) => format!("{s:?}"),
            Expect::Contains(parts) => format!("text containing {}", parts.iter().map(|p| format!("{p:?}")).collect::<Vec<_>>().join(" and ")),
        }
    }
}

/// One step on the learning path.
#[derive(Clone, Copy)]
pub enum Step {
    Lesson(&'static Module),
    Exercise(&'static Exercise),
    Quiz(&'static Module),
}

pub fn path() -> Vec<Step> {
    let mut v = Vec::new();
    for m in MODULES {
        if !m.lesson.is_empty() {
            v.push(Step::Lesson(m));
        }
        for e in m.exercises {
            v.push(Step::Exercise(e));
        }
        if m.quiz.is_some() {
            v.push(Step::Quiz(m));
        }
    }
    v
}

pub fn exercises() -> impl Iterator<Item = &'static Exercise> {
    MODULES.iter().flat_map(|m| m.exercises.iter())
}

pub fn find_exercise(name: &str) -> Option<&'static Exercise> {
    let name = name.trim_end_matches(".rs");
    let name = name.rsplit('/').next().unwrap_or(name);
    exercises().find(|e| e.name.eq_ignore_ascii_case(name))
}

pub fn find_module(q: &str) -> Option<&'static Module> {
    if let Ok(n) = q.parse::<u8>() {
        return MODULES.iter().find(|m| m.num == n);
    }
    let q = q.to_ascii_lowercase();
    MODULES
        .iter()
        .find(|m| m.slug == q || m.dir() == q)
        .or_else(|| MODULES.iter().find(|m| m.slug.contains(q.as_str()) || m.title.to_ascii_lowercase().contains(q.as_str())))
}

/// Suggest exercise names close to `q` (for typos).
pub fn suggest(q: &str) -> Vec<&'static str> {
    let q = q.to_ascii_lowercase();
    let mut v: Vec<(usize, &'static str)> = exercises()
        .map(|e| (edit_distance(&q, e.name), e.name))
        .filter(|(d, n)| *d <= 3 || n.starts_with(q.as_str()))
        .collect();
    v.sort();
    v.into_iter().take(4).map(|(_, n)| n).collect()
}

pub fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i; b.len() + 1];
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        prev = cur;
    }
    prev[b.len()]
}

pub const RUNTIME_SOURCE: &str = include_str!("../../curriculum/support/nostd_rt.rs");
pub const DRILLS: &str = include_str!("../../curriculum/drills.txt");
