// `--target neolyx` end to end, short of the NeolyxOS link.
//
// Every spec example is compiled for NeolyxOS and the C is checked by gcc
// in freestanding mode with no system include path except gcc's own
// (stdint/stdbool/stddef/stdarg): the generated C may reach only rx_core.h
// and the rx_platform.h contract. The reoxc binary is also run to check
// that `--emit obj|exe` is refused for NeolyxOS with exit status 2. gcc
// being unavailable skips the C half with a message, never a silent pass.

use std::path::PathBuf;
use std::process::Command;

use reoxc::cli::{compile_file_for, Target};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn gcc_include_dir() -> Option<String> {
    let out = Command::new("gcc").arg("-print-file-name=include").output().ok()?;
    if !out.status.success() {
        return None;
    }
    let dir = String::from_utf8_lossy(&out.stdout).trim().to_string();
    PathBuf::from(&dir).join("stdint.h").is_file().then_some(dir)
}

#[test]
fn spec_examples_compile_as_freestanding_c_for_neolyx() {
    let Some(gcc_include) = gcc_include_dir() else {
        eprintln!("gcc not found, skipping the freestanding check");
        return;
    };
    let runtime = crate_root().join("runtime");
    let out_dir = std::env::temp_dir().join(format!("reox-neolyx-{}", std::process::id()));
    std::fs::create_dir_all(&out_dir).expect("temp dir");

    let mut checked = 0;
    let dir = crate_root().join("tests/spec");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir).expect("tests/spec")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("rx"))
        .collect();
    files.sort();
    for path in files {
        let (c_code, _) = compile_file_for(&path, Target::Neolyx)
            .unwrap_or_else(|(diags, sources)| panic!("{}", sources.render(&diags)));
        let c_path = out_dir.join(format!("{}.c", path.file_stem().unwrap().to_string_lossy()));
        std::fs::write(&c_path, &c_code).expect("write C");
        let gcc = Command::new("gcc")
            .args(["-std=c11", "-ffreestanding", "-nostdinc", "-fsyntax-only", "-D__NEOLYXOS__"])
            .arg(format!("-I{}", gcc_include))
            .arg(format!("-I{}", runtime.join("core").display()))
            .arg(format!("-I{}", runtime.join("platform").display()))
            .arg(&c_path)
            .output()
            .expect("run gcc");
        assert!(
            gcc.status.success(),
            "{} is not freestanding C for neolyx:\n{}\n--- generated C ---\n{}",
            path.display(), String::from_utf8_lossy(&gcc.stderr), c_code
        );
        checked += 1;
    }
    assert!(checked > 0, "no spec examples found under {}", dir.display());
}

#[test]
fn emit_exe_for_neolyx_is_refused_with_status_2() {
    let src = std::env::temp_dir().join(format!("reox-neolyx-emit-{}.rx", std::process::id()));
    std::fs::write(&src, "fn main() -> int { return 0; }\n").expect("write source");
    for emit in ["obj", "exe"] {
        let out = Command::new(env!("CARGO_BIN_EXE_reoxc"))
            .args(["compile", "--emit", emit, "--target", "neolyx", "-o", "/dev/null"])
            .arg(&src)
            .output()
            .expect("run reoxc");
        assert_eq!(out.status.code(), Some(2), "--emit {} must exit 2", emit);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(stderr.contains("requires the NeolyxOS SDK"), "{}", stderr);
    }
}
