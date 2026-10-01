//! `nostd new`: scaffold real Cargo projects that apply what the course teaches.

use std::path::PathBuf;

use crate::commands::Res;
use crate::curriculum::RUNTIME_SOURCE;
use crate::term;
use crate::workspace::write_file;
use crate::Args;

struct Template {
    name: &'static str,
    about: &'static str,
    files: &'static [(&'static str, &'static str)],
    next: &'static [&'static str],
}

macro_rules! t {
    ($dir:literal, $path:literal) => {
        ($path, include_str!(concat!("../curriculum/templates/", $dir, "/", $path)))
    };
}

const GITIGNORE: &str = "/target\n";

const TEMPLATES: &[Template] = &[
    Template {
        name: "lib",
        about: "portable #![no_std] library with alloc/std features, safety lints and a cross-compiling CI workflow",
        files: &[
            t!("lib", "Cargo.toml"),
            t!("lib", "clippy.toml"),
            t!("lib", "README.md"),
            t!("lib", "src/lib.rs"),
            t!("lib", ".github/workflows/ci.yml"),
        ],
        next: &["cargo test --all-features", "cargo build --target thumbv7em-none-eabihf"],
    },
    Template {
        name: "linux-bin",
        about: "freestanding Linux executable (no libc, no crt0) with its own runtime crates",
        files: &[
            t!("linux-bin", "Cargo.toml"),
            t!("linux-bin", "build.rs"),
            t!("linux-bin", "README.md"),
            t!("linux-bin", "src/main.rs"),
            t!("linux-bin", "rt/Cargo.toml"),
            ("mem/Cargo.toml", MEM_CARGO),
        ],
        next: &["cargo build --release", "echo hi | ./target/release/{{name}} a b"],
    },
    Template {
        name: "cortex-m",
        about: "Cortex-M3 firmware with zero dependencies: hand-written vector table, reset handler, SysTick, semihosting; runs in QEMU",
        files: &[
            t!("cortex-m", "Cargo.toml"),
            t!("cortex-m", ".cargo/config.toml"),
            t!("cortex-m", "build.rs"),
            t!("cortex-m", "link.x"),
            t!("cortex-m", "README.md"),
            t!("cortex-m", "src/main.rs"),
        ],
        next: &["rustup target add thumbv7m-none-eabi", "cargo run"],
    },
    Template {
        name: "cortex-m-rt",
        about: "the same firmware on the ecosystem stack: cortex-m-rt, cortex-m, semihosting (needs crates.io)",
        files: &[
            t!("cortex-m-rt", "Cargo.toml"),
            t!("cortex-m-rt", ".cargo/config.toml"),
            t!("cortex-m-rt", "build.rs"),
            t!("cortex-m-rt", "memory.x"),
            t!("cortex-m-rt", "README.md"),
            t!("cortex-m-rt", "src/main.rs"),
        ],
        next: &["rustup target add thumbv7m-none-eabi", "cargo run"],
    },
];

const MEM_CARGO: &str = r#"[package]
name = "nostd_mem"
version = "0.1.0"
edition = "2024"
description = "memcpy & co for a libc-free Linux program (#![no_builtins])"

[lib]
path = "src/lib.rs"
"#;

/// Split the course runtime into (runtime-without-mem, mem-only) crates.
///
/// `#![no_builtins]` crates are excluded from LTO. If such a crate calls into
/// `core`, LTO may internalise the `core` functions it needs and the link
/// fails. So the template keeps the libc-replacement symbols in a tiny
/// no_builtins crate that calls nothing, and the rest in a normal crate.
fn split_runtime() -> (String, String) {
    let start = RUNTIME_SOURCE.find("// >>> mem").unwrap_or(RUNTIME_SOURCE.len());
    let end = RUNTIME_SOURCE.find("// <<< mem").unwrap_or(RUNTIME_SOURCE.len());
    let mem_body = &RUNTIME_SOURCE[start..end];
    let mut rt = String::new();
    rt.push_str(&RUNTIME_SOURCE[..start]);
    rt.push_str("// memcpy, memset, memcmp, bcmp, memmove, strlen and rust_eh_personality\n");
    rt.push_str("// live in the `nostd_mem` crate (see mem/src/lib.rs).\n");
    let rt = rt
        .replace(
            "// Stop LLVM from recognising the byte loops below as \"memcpy idioms\" and\n// replacing them with calls to... memcpy. That would recurse forever.\n#![no_builtins]\n",
            "// Link the libc-replacement symbols (a separate #![no_builtins] crate).\nextern crate nostd_mem;\n",
        )
        .replace("use core::ffi::{c_char, c_int, c_void};", "use core::ffi::c_char;");
    let mem = format!(
        "//! `memcpy` & co for a program without libc.\n//!\n//! `#![no_builtins]` stops LLVM from turning these loops back into calls to\n//! themselves. Keep this crate free of calls into `core`: no_builtins crates\n//! are left out of LTO.\n\n#![no_std]\n#![no_builtins]\n\nuse core::ffi::{{c_char, c_int, c_void}};\n\n{}",
        mem_body
    );
    (rt, mem)
}

pub fn new(args: &Args) -> Res {
    let (Some(kind), Some(name)) = (args.pos(0), args.pos(1)) else {
        println!();
        println!("  usage: {}", term::bold("nostd new <template> <name> [--dir PATH]"));
        println!();
        for t in TEMPLATES {
            println!("  {}  {}", term::pad(&term::bold(t.name), 14), term::dim(t.about));
        }
        println!();
        return if args.positional.is_empty() { Ok(()) } else { Err("missing project name".into()) };
    };
    let tpl = TEMPLATES
        .iter()
        .find(|t| t.name == kind)
        .ok_or_else(|| format!("unknown template `{kind}`; choose one of: {}", TEMPLATES.iter().map(|t| t.name).collect::<Vec<_>>().join(", ")))?;
    let valid = !name.is_empty()
        && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        && !name.starts_with(|c: char| c.is_ascii_digit());
    if !valid {
        return Err(format!("`{name}` is not a good package name: use lowercase letters, digits, - and _"));
    }
    let dir = PathBuf::from(args.opt(&["--dir"]).unwrap_or(name));
    if dir.exists() && std::fs::read_dir(&dir).map(|mut d| d.next().is_some()).unwrap_or(true) {
        return Err(format!("{} already exists and is not empty", dir.display()));
    }
    let crate_name = name.replace('-', "_");
    let subst = |s: &str| s.replace("{{name}}", name).replace("{{crate}}", &crate_name);
    for (path, contents) in tpl.files {
        write_file(&dir.join(path), &subst(contents))?;
    }
    if tpl.name == "linux-bin" {
        let (rt, mem) = split_runtime();
        write_file(&dir.join("rt/src/lib.rs"), &rt)?;
        write_file(&dir.join("mem/src/lib.rs"), &mem)?;
        let rt_cargo = std::fs::read_to_string(dir.join("rt/Cargo.toml")).unwrap_or_default();
        write_file(&dir.join("rt/Cargo.toml"), &format!("{rt_cargo}\n[dependencies]\nnostd_mem = {{ path = \"../mem\" }}\n"))?;
    }
    write_file(&dir.join(".gitignore"), GITIGNORE)?;
    println!();
    println!("  {} {} project in {}", term::bold_green("created"), tpl.name, term::bold(&dir.display().to_string()));
    println!("  {}", term::dim(tpl.about));
    println!();
    println!("      cd {}", dir.display());
    for n in tpl.next {
        println!("      {}", subst(n));
    }
    println!();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_split_is_clean() {
        let (rt, mem) = split_runtime();
        assert!(rt.contains("extern crate nostd_mem;"));
        assert!(!rt.contains("#![no_builtins]"));
        assert!(!rt.contains("fn memcpy"));
        assert!(mem.contains("#![no_builtins]"));
        assert!(mem.contains("fn memcpy"));
        assert!(mem.contains("fn rust_eh_personality"));
    }
}
