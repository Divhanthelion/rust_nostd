//! Learner progress, persisted as a small line-oriented text file so it is
//! easy to inspect, diff, or edit by hand.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Default, Debug)]
pub struct State {
    path: PathBuf,
    /// Modules whose lesson has been read.
    pub read: BTreeSet<u8>,
    /// Exercises that passed.
    pub done: BTreeSet<String>,
    /// How many hints were revealed per exercise.
    pub hints: BTreeMap<String, usize>,
    /// Best quiz score per module: (correct, total).
    pub quiz: BTreeMap<u8, (usize, usize)>,
    /// Exercise currently being worked on.
    pub current: Option<String>,
    /// Flashcard Leitner box per card id (1..=5).
    pub drill: BTreeMap<String, u8>,
}

impl State {
    pub fn load(path: &Path) -> State {
        let mut st = State { path: path.to_path_buf(), ..Default::default() };
        let Ok(text) = fs::read_to_string(path) else { return st };
        for line in text.lines() {
            let mut it = line.split_whitespace();
            match (it.next(), it.next(), it.next(), it.next()) {
                (Some("read"), Some(m), _, _) => {
                    if let Ok(m) = m.parse() {
                        st.read.insert(m);
                    }
                }
                (Some("done"), Some(e), _, _) => {
                    st.done.insert(e.to_string());
                }
                (Some("hint"), Some(e), Some(n), _) => {
                    if let Ok(n) = n.parse() {
                        st.hints.insert(e.to_string(), n);
                    }
                }
                (Some("quiz"), Some(m), Some(c), Some(t)) => {
                    if let (Ok(m), Ok(c), Ok(t)) = (m.parse(), c.parse(), t.parse()) {
                        st.quiz.insert(m, (c, t));
                    }
                }
                (Some("current"), Some(e), _, _) => st.current = Some(e.to_string()),
                (Some("drill"), Some(id), Some(b), _) => {
                    if let Ok(b) = b.parse() {
                        st.drill.insert(id.to_string(), b);
                    }
                }
                _ => {}
            }
        }
        st
    }

    pub fn save(&self) -> Result<(), String> {
        let mut out = String::from("# nostd progress file. Safe to edit; one record per line.\n");
        if let Some(c) = &self.current {
            out.push_str(&format!("current {c}\n"));
        }
        for m in &self.read {
            out.push_str(&format!("read {m}\n"));
        }
        for e in &self.done {
            out.push_str(&format!("done {e}\n"));
        }
        for (e, n) in &self.hints {
            out.push_str(&format!("hint {e} {n}\n"));
        }
        for (m, (c, t)) in &self.quiz {
            out.push_str(&format!("quiz {m} {c} {t}\n"));
        }
        for (id, b) in &self.drill {
            out.push_str(&format!("drill {id} {b}\n"));
        }
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
        }
        fs::write(&self.path, out).map_err(|e| format!("cannot write {}: {e}", self.path.display()))
    }

    pub fn quiz_passed(&self, module: u8) -> bool {
        self.quiz.get(&module).is_some_and(|&(c, t)| t > 0 && c * 100 >= t * crate::quiz::PASS_PERCENT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let dir = std::env::temp_dir().join(format!("nostd-state-test-{}", std::process::id()));
        let path = dir.join("state.txt");
        let mut st = State::load(&path);
        st.read.insert(3);
        st.done.insert("core1".into());
        st.hints.insert("core2".into(), 2);
        st.quiz.insert(1, (7, 8));
        st.current = Some("core2".into());
        st.drill.insert("abc".into(), 3);
        st.save().unwrap();
        let st2 = State::load(&path);
        assert!(st2.read.contains(&3));
        assert!(st2.done.contains("core1"));
        assert_eq!(st2.hints.get("core2"), Some(&2));
        assert_eq!(st2.quiz.get(&1), Some(&(7, 8)));
        assert_eq!(st2.current.as_deref(), Some("core2"));
        assert_eq!(st2.drill.get("abc"), Some(&3));
        let _ = fs::remove_dir_all(dir);
    }
}
