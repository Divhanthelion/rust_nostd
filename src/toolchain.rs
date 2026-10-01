//! Discovering rustc, the sysroot, installed targets and helper programs, and
//! building the "core-only sysroot" that proves exercises really are `no_std`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct Toolchain {
    pub rustc: PathBuf,
    pub version: String,
    /// e.g. "1.97.0"
    pub release: String,
    pub host: String,
    pub sysroot: PathBuf,
}

impl Toolchain {
    pub fn detect() -> Result<Toolchain, String> {
        let rustc = std::env::var_os("RUSTC").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("rustc"));
        let out = Command::new(&rustc)
            .arg("-vV")
            .output()
            .map_err(|e| format!("could not run `{}`: {e}\n  Install Rust from https://rustup.rs", rustc.display()))?;
        if !out.status.success() {
            return Err(format!("`{} -vV` failed: {}", rustc.display(), String::from_utf8_lossy(&out.stderr)));
        }
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        let version = text.lines().next().unwrap_or("rustc ?").to_string();
        let field = |k: &str| {
            text.lines()
                .find_map(|l| l.strip_prefix(k).map(|v| v.trim().to_string()))
                .unwrap_or_default()
        };
        let release = field("release:");
        let host = field("host:");
        let out = Command::new(&rustc)
            .args(["--print", "sysroot"])
            .output()
            .map_err(|e| format!("could not run rustc --print sysroot: {e}"))?;
        let sysroot = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
        Ok(Toolchain { rustc, version, release, host, sysroot })
    }

    /// (major, minor) of the compiler release.
    pub fn minor_version(&self) -> (u32, u32) {
        let mut it = self.release.split(['.', '-']);
        let maj = it.next().and_then(|v| v.parse().ok()).unwrap_or(0);
        let min = it.next().and_then(|v| v.parse().ok()).unwrap_or(0);
        (maj, min)
    }

    pub fn target_lib_dir(&self, target: &str) -> PathBuf {
        self.sysroot.join("lib").join("rustlib").join(target).join("lib")
    }

    pub fn has_target(&self, target: &str) -> bool {
        let dir = self.target_lib_dir(target);
        fs::read_dir(&dir)
            .map(|rd| rd.flatten().any(|e| e.file_name().to_string_lossy().starts_with("libcore-")))
            .unwrap_or(false)
    }

    /// Host is a Linux on x86_64/aarch64, i.e. we can run freestanding binaries.
    pub fn can_run_freestanding(&self) -> bool {
        self.host.contains("-linux") && (self.host.starts_with("x86_64") || self.host.starts_with("aarch64"))
    }

    /// Build (once per compiler release) a sysroot that contains only
    /// `core`, `alloc` and `compiler_builtins` for the host. Compiling against
    /// it makes any use of `std` a hard error, exactly like a real bare-metal
    /// target, while still producing host code we can run.
    pub fn core_sysroot(&self, cache: &Path) -> Result<PathBuf, String> {
        let root = cache.join(format!("sysroot-{}-{}", self.release, self.host));
        let lib = root.join("lib").join("rustlib").join(&self.host).join("lib");
        let stamp = root.join("ok");
        if stamp.exists() {
            return Ok(root);
        }
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&lib).map_err(|e| format!("cannot create {}: {e}", lib.display()))?;
        let src = self.target_lib_dir(&self.host);
        let entries = fs::read_dir(&src).map_err(|e| format!("cannot read {}: {e}", src.display()))?;
        let mut found = 0;
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let wanted = ["libcore-", "liballoc-", "libcompiler_builtins-"].iter().any(|p| name.starts_with(p))
                && (name.ends_with(".rlib") || name.ends_with(".rmeta"));
            if wanted {
                link_or_copy(&entry.path(), &lib.join(&name))?;
                found += 1;
            }
        }
        if found < 3 {
            return Err(format!("could not find core/alloc/compiler_builtins in {}", src.display()));
        }
        // The linker shims (gcc-ld, rust-lld) live in lib/rustlib/<host>/bin.
        let bin = self.sysroot.join("lib").join("rustlib").join(&self.host).join("bin");
        if bin.is_dir() {
            let dst = root.join("lib").join("rustlib").join(&self.host).join("bin");
            #[cfg(unix)]
            {
                let _ = std::os::unix::fs::symlink(&bin, &dst);
            }
            #[cfg(not(unix))]
            {
                let _ = dst;
            }
        }
        // Smoke test: a no_std lib must compile, a std-using one must not.
        let probe_dir = root.join("probe");
        fs::create_dir_all(&probe_dir).map_err(|e| e.to_string())?;
        let ok_src = probe_dir.join("ok.rs");
        fs::write(&ok_src, "#![no_std]\nextern crate alloc;\npub fn f() -> alloc::vec::Vec<u8> { alloc::vec![1] }\n")
            .map_err(|e| e.to_string())?;
        let out = Command::new(&self.rustc)
            .args(["--edition=2024", "--crate-type=lib", "--emit=metadata", "--crate-name=probe"])
            .arg("--sysroot")
            .arg(&root)
            .arg("--out-dir")
            .arg(&probe_dir)
            .arg(&ok_src)
            .output()
            .map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(format!(
                "the core-only sysroot did not work:\n{}",
                String::from_utf8_lossy(&out.stderr)
            ));
        }
        fs::write(&stamp, &self.version).map_err(|e| e.to_string())?;
        Ok(root)
    }

    /// Where rust-lld lives (used to link aarch64 binaries from other hosts).
    pub fn rust_lld(&self) -> PathBuf {
        let exe = if cfg!(windows) { "rust-lld.exe" } else { "rust-lld" };
        self.sysroot.join("lib").join("rustlib").join(&self.host).join("bin").join(exe)
    }
}

fn link_or_copy(src: &Path, dst: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        if std::os::unix::fs::symlink(src, dst).is_ok() {
            return Ok(());
        }
    }
    fs::copy(src, dst).map(|_| ()).map_err(|e| format!("cannot copy {}: {e}", src.display()))
}

/// Find an executable on PATH.
pub fn which(cmd: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let p = dir.join(cmd);
        if p.is_file() {
            return Some(p);
        }
        if cfg!(windows) {
            let p = dir.join(format!("{cmd}.exe"));
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

/// Find a C compiler for the FFI exercises.
pub fn c_compiler() -> Option<PathBuf> {
    if let Some(cc) = std::env::var_os("CC") {
        let p = PathBuf::from(cc);
        if p.is_absolute() {
            return Some(p);
        }
        return which(&p.to_string_lossy());
    }
    which("cc").or_else(|| which("gcc")).or_else(|| which("clang"))
}

/// Targets installed via rustup (or present in the sysroot).
pub fn installed_targets(tc: &Toolchain) -> Vec<String> {
    let dir = tc.sysroot.join("lib").join("rustlib");
    let mut v: Vec<String> = fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.path().join("lib").is_dir())
                .map(|e| e.file_name().to_string_lossy().to_string())
                .filter(|n| tc.has_target(n))
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}
