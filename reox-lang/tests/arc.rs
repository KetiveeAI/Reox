// REOX ARC verification suite
//
// Compiles every program in tests/arc/ to C with the real code generator,
// builds it against the runtime core and the Linux platform layer, runs it,
// checks its output against the `// expect:` lines in the source and runs it
// under valgrind with full leak checking. When valgrind is not installed the
// program is instead built with AddressSanitizer + LeakSanitizer, which
// catch the same classes of error (leaks, invalid reads and writes, double
// frees). Zero leaks and zero invalid accesses are required either way.

use std::path::{Path, PathBuf};
use std::process::Command;

use reoxc::cli::compile_source;

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn runtime_sources() -> Vec<PathBuf> {
    let root = crate_root().join("runtime");
    let mut sources = Vec::new();
    for name in ["reox_runtime.c", "rx_core.c", "rx_object.c", "rx_map.c"] {
        sources.push(root.join("core").join(name));
    }
    sources.push(root.join("platform/linux/rx_platform_linux.c"));
    sources
}

fn expected_lines(source: &str) -> Vec<String> {
    source
        .lines()
        .filter_map(|l| l.trim().strip_prefix("// expect:"))
        .map(|l| l.trim().to_string())
        .collect()
}

fn have(tool: &str) -> bool {
    Command::new(tool).arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

fn build(name: &str, c_code: &str, out_dir: &Path, sanitize: bool) -> PathBuf {
    let c_path = out_dir.join(format!("{}.c", name));
    std::fs::write(&c_path, c_code).expect("write generated C");
    let exe = out_dir.join(if sanitize { format!("{}-asan", name) } else { name.to_string() });
    let runtime = crate_root().join("runtime");
    let mut cmd = Command::new("gcc");
    cmd.arg("-std=gnu11").arg("-g").arg("-O0").arg("-Wall").arg("-Wno-unused-variable").arg("-Wno-unused-but-set-variable").arg("-Wno-unused-label")
        .arg("-Werror=implicit-function-declaration");
    if sanitize {
        cmd.arg("-fsanitize=address,undefined").arg("-fno-omit-frame-pointer");
    }
    cmd
        .arg("-I").arg(runtime.join("core"))
        .arg("-I").arg(&runtime)
        .arg("-o").arg(&exe)
        .arg(&c_path);
    for src in runtime_sources() {
        cmd.arg(src);
    }
    cmd.arg("-lm");
    let output = cmd.output().expect("run gcc");
    assert!(
        output.status.success(),
        "{}: gcc failed:\n{}\n--- generated C ---\n{}",
        name,
        String::from_utf8_lossy(&output.stderr),
        c_code
    );
    exe
}

#[test]
fn arc_programs_run_leak_free() {
    assert!(have("gcc"), "gcc is required for the ARC suite");
    let dir = crate_root().join("tests/arc");
    let mut programs: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("tests/arc exists")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("rx"))
        .collect();
    programs.sort();
    assert!(programs.len() >= 7, "expected the ARC suite to hold at least 7 programs");

    let out_dir = std::env::temp_dir().join(format!("reox-arc-{}", std::process::id()));
    std::fs::create_dir_all(&out_dir).expect("create temp dir");
    let valgrind = have("valgrind");
    if !valgrind {
        eprintln!("valgrind is not installed (install with: sudo apt install valgrind); checking leaks with AddressSanitizer/LeakSanitizer instead");
    }

    for program in &programs {
        let name = program.file_stem().and_then(|s| s.to_str()).unwrap_or("program").to_string();
        let source = std::fs::read_to_string(program).expect("read program");
        let c_code = match compile_source(&source) {
            Ok(c) => c,
            Err(diags) => {
                let rendered: Vec<String> = diags.iter().map(|d| d.render(&name, &source)).collect();
                panic!("{}: compile failed:\n{}", name, rendered.join("\n"));
            }
        };
        assert!(!c_code.contains("setjmp") && !c_code.contains("longjmp"), "{}: longjmp in generated code", name);
        let exe = build(&name, &c_code, &out_dir, false);

        let run = Command::new(&exe).output().expect("run program");
        assert!(run.status.success(), "{}: exited with {}\nstderr: {}", name, run.status, String::from_utf8_lossy(&run.stderr));
        let got: Vec<String> = String::from_utf8_lossy(&run.stdout).lines().map(|l| l.to_string()).collect();
        assert_eq!(got, expected_lines(&source), "{}: output differs", name);

        if valgrind {
            let vg = Command::new("valgrind")
                .arg("--error-exitcode=1")
                .arg("--leak-check=full")
                .arg("--errors-for-leak-kinds=all")
                .arg("--show-leak-kinds=all")
                .arg("-q")
                .arg(&exe)
                .output()
                .expect("run valgrind");
            assert!(
                vg.status.success(),
                "{}: valgrind reported errors:\n{}",
                name,
                String::from_utf8_lossy(&vg.stderr)
            );
        } else {
            let asan_exe = build(&name, &c_code, &out_dir, true);
            let asan = Command::new(&asan_exe)
                .env("ASAN_OPTIONS", "detect_leaks=1:abort_on_error=0")
                .env("UBSAN_OPTIONS", "halt_on_error=1")
                .output()
                .expect("run sanitized program");
            assert!(
                asan.status.success(),
                "{}: sanitizer reported errors:\n{}",
                name,
                String::from_utf8_lossy(&asan.stderr)
            );
            let got: Vec<String> = String::from_utf8_lossy(&asan.stdout).lines().map(|l| l.to_string()).collect();
            assert_eq!(got, expected_lines(&source), "{}: sanitized output differs", name);
        }
    }
    let _ = std::fs::remove_dir_all(&out_dir);
}
