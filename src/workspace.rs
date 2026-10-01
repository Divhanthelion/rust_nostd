//! The learner's workspace: a directory holding editable exercise files,
//! read-only reference copies of lessons and support code, and `.nostd/`
//! (progress + build cache).

use std::fs;
use std::path::{Path, PathBuf};

use crate::curriculum::{self, Exercise, Mode, MODULES};
use crate::toolchain::Toolchain;

pub struct Workspace {
    pub root: PathBuf,
}

const MARKER_DIR: &str = ".nostd";

impl Workspace {
    /// Locate the workspace: `$NOSTD_WORKSPACE`, or the nearest ancestor of the
    /// current directory that contains `.nostd/`.
    pub fn find() -> Option<Workspace> {
        if let Some(p) = std::env::var_os("NOSTD_WORKSPACE") {
            let root = PathBuf::from(p);
            if root.join(MARKER_DIR).is_dir() {
                return Some(Workspace { root });
            }
        }
        let mut dir = std::env::current_dir().ok()?;
        loop {
            if dir.join(MARKER_DIR).is_dir() {
                return Some(Workspace { root: dir });
            }
            if !dir.pop() {
                return None;
            }
        }
    }

    pub fn state_path(&self) -> PathBuf {
        self.root.join(MARKER_DIR).join("state.txt")
    }

    pub fn build_dir(&self, name: &str) -> PathBuf {
        self.root.join(MARKER_DIR).join("build").join(name)
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.root.join(MARKER_DIR)
    }

    pub fn exercise_file(&self, ex: &Exercise) -> PathBuf {
        self.root.join(ex.rel_path())
    }

    /// Make sure the exercise (and its extra files) exist on disk, writing the
    /// original version if missing. Returns the exercise path.
    pub fn ensure_exercise(&self, ex: &Exercise) -> Result<PathBuf, String> {
        let path = self.exercise_file(ex);
        if !path.exists() {
            write_file(&path, ex.source)?;
        }
        for (name, contents) in ex.extra {
            let p = self.root.join(ex.extra_rel_path(name));
            if !p.exists() {
                write_file(&p, contents)?;
            }
        }
        Ok(path)
    }

    pub fn reset_exercise(&self, ex: &Exercise) -> Result<PathBuf, String> {
        let path = self.exercise_file(ex);
        write_file(&path, ex.source)?;
        for (name, contents) in ex.extra {
            write_file(&self.root.join(ex.extra_rel_path(name)), contents)?;
        }
        Ok(path)
    }

    /// Create (or top up) a workspace in `dir`. Never overwrites learner files.
    pub fn init(dir: &Path, tc: Option<&Toolchain>) -> Result<(Workspace, usize), String> {
        fs::create_dir_all(dir.join(MARKER_DIR)).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
        let ws = Workspace { root: dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf()) };
        let mut created = 0;
        for ex in curriculum::exercises() {
            if !ws.exercise_file(ex).exists() {
                created += 1;
            }
            ws.ensure_exercise(ex)?;
        }
        // Reference copies (always refreshed: they are not meant to be edited).
        for m in MODULES {
            if !m.lesson.is_empty() {
                write_file(&ws.root.join(format!("lessons/{:02}-{}.md", m.num, m.slug)), m.lesson)?;
            }
        }
        write_file(&ws.root.join("support/nostd_rt.rs"), curriculum::RUNTIME_SOURCE)?;
        let readme = ws.root.join("README.md");
        if !readme.exists() {
            write_file(&readme, WORKSPACE_README)?;
        }
        let gi = ws.root.join(".gitignore");
        if !gi.exists() {
            write_file(&gi, ".nostd/build/\n.nostd/sysroot*/\n")?;
        }
        let state = ws.state_path();
        if !state.exists() {
            crate::state::State::load(&state).save()?;
        }
        if let Some(tc) = tc {
            let _ = ws.write_rust_project(tc);
        }
        Ok((ws, created))
    }

    /// Write `rust-project.json` so rust-analyzer understands the loose
    /// exercise files (each file is its own crate).
    pub fn write_rust_project(&self, tc: &Toolchain) -> Result<PathBuf, String> {
        let mut crates = Vec::new();
        // crate 0: the runtime support crate
        crates.push(format!(
            "    {{\"display_name\": \"nostd_rt\", \"root_module\": {}, \"edition\": \"2024\", \"deps\": [], \"cfg\": [], \"is_workspace_member\": false}}",
            json_str(&self.root.join("support/nostd_rt.rs").to_string_lossy())
        ));
        for ex in curriculum::exercises() {
            let root_module = self.exercise_file(ex);
            let mut deps = String::new();
            let mut target = String::new();
            let mut cfg = vec!["\"test\"".to_string()];
            match &ex.mode {
                Mode::Bin { rt: true, .. } => deps = "{\"crate\": 0, \"name\": \"nostd_rt\"}".into(),
                Mode::CortexM { .. } => {
                    target = ", \"target\": \"thumbv7m-none-eabi\"".into();
                    cfg.clear();
                }
                Mode::Bin { .. } | Mode::CLib { .. } => cfg.clear(),
                Mode::LibFeatures(sets) => {
                    for set in sets.iter() {
                        for f in set.iter() {
                            let c = format!("\"feature=\\\"{f}\\\"\"");
                            if !cfg.contains(&c) {
                                cfg.push(c);
                            }
                        }
                    }
                }
                Mode::Lib => {}
            }
            crates.push(format!(
                "    {{\"display_name\": {}, \"root_module\": {}, \"edition\": \"2024\", \"deps\": [{}], \"cfg\": [{}], \"is_workspace_member\": true{}}}",
                json_str(ex.name),
                json_str(&root_module.to_string_lossy()),
                deps,
                cfg.join(", "),
                target
            ));
        }
        let json = format!(
            "{{\n  \"sysroot\": {},\n  \"crates\": [\n{}\n  ]\n}}\n",
            json_str(&tc.sysroot.to_string_lossy()),
            crates.join(",\n")
        );
        let path = self.root.join("rust-project.json");
        write_file(&path, &json)?;
        Ok(path)
    }
}

pub fn write_file(path: &Path, contents: &str) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    }
    fs::write(path, contents).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

fn json_str(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

const WORKSPACE_README: &str = r#"# Your no_std workspace

This directory was created by `nostd init`. It holds your copies of the
exercises. Edit them freely; `nostd reset <exercise>` restores the original.

    exercises/   the files you edit (one crate per file)
    lessons/     read-only copies of the lessons (Markdown), for your editor
    support/     the course runtime crate `nostd_rt` (reference copy)
    .nostd/      progress (state.txt) and build cache

Daily loop:

    nostd            where am I? what's next?
    nostd next       open the next lesson / exercise / quiz
    nostd watch      re-check the current exercise every time you save
    nostd hint       reveal the next hint for the current exercise
    nostd learn 5    re-read the lesson of module 5

Editor support: `nostd lsp` writes rust-project.json so rust-analyzer can
check each exercise file as its own crate.
"#;
