// REOX spec examples, end to end.
//
// Every program under tests/spec/ (the "Planned" examples of reox-spec.md)
// is compiled to C, built with gcc against runtime/ and run; its output is
// compared with the expected text. Programs that need no C-only runtime
// symbols are also run in the interpreter. gcc or the runtime library being
// unavailable skips the C half with a message, never a silent pass.

use std::path::{Path, PathBuf};
use std::process::Command;

use reoxc::cli::{check_file, compile_file};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn have_gcc() -> bool {
    Command::new("gcc").arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

/// Build librx_linux.a if it is not there yet.
fn runtime_lib() -> Option<PathBuf> {
    let runtime = crate_root().join("runtime");
    let lib = runtime.join("librx_linux.a");
    if !lib.is_file() {
        let status = Command::new("make").arg("PLATFORM=linux").current_dir(&runtime).status().ok()?;
        if !status.success() {
            return None;
        }
    }
    lib.is_file().then_some(lib)
}

fn sdl_libs() -> Vec<String> {
    Command::new("pkg-config").args(["--libs", "sdl2"]).output().ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).split_whitespace().map(str::to_string).collect())
        .unwrap_or_default()
}

/// Compile `name` to C, build it and return the program's stdout.
fn run_in_c(name: &str) -> Option<String> {
    if !have_gcc() {
        eprintln!("gcc not found, skipping the C build of {}", name);
        return None;
    }
    let lib = match runtime_lib() {
        Some(lib) => lib,
        None => {
            eprintln!("runtime library could not be built, skipping the C build of {}", name);
            return None;
        }
    };
    let source = crate_root().join("tests/spec").join(name);
    let (c_code, _) = compile_file(&source).unwrap_or_else(|(diags, sources)| panic!("{}", sources.render(&diags)));

    let out_dir = std::env::temp_dir().join(format!("reox-spec-{}-{}", std::process::id(), name.replace('.', "_")));
    std::fs::create_dir_all(&out_dir).expect("temp dir");
    let c_path = out_dir.join(format!("{}.c", name));
    let exe = out_dir.join("program");
    std::fs::write(&c_path, &c_code).expect("write C");

    let mut gcc = Command::new("gcc");
    gcc.args(["-std=gnu11", "-w", "-I"]).arg(crate_root().join("runtime"))
        .arg("-o").arg(&exe).arg(&c_path).arg(&lib).arg("-lm");
    for flag in sdl_libs() {
        gcc.arg(flag);
    }
    let build = gcc.output().expect("run gcc");
    assert!(
        build.status.success(),
        "gcc failed for {}:\n{}\n--- generated C ---\n{}",
        name, String::from_utf8_lossy(&build.stderr), c_code
    );
    let run = Command::new(&exe).output().expect("run program");
    assert!(run.status.success(), "{} exited with {}: {}", name, run.status, String::from_utf8_lossy(&run.stderr));
    Some(String::from_utf8_lossy(&run.stdout).into_owned())
}

/// Run `name` in the interpreter; `main` must return normally.
fn run_in_interpreter(name: &str) {
    let source = crate_root().join("tests/spec").join(name);
    let (ast, sources) = check_file(&source).unwrap_or_else(|(diags, sources)| panic!("{}", sources.render(&diags)));
    if let Err(e) = reoxc::interpreter::eval(&ast) {
        panic!("{}: {}", name, sources.render(&[e.into()]));
    }
}

fn every_spec_file() -> Vec<PathBuf> {
    let dir = crate_root().join("tests/spec");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir).expect("tests/spec")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("rx"))
        .collect();
    files.sort();
    files
}

#[test]
fn every_spec_example_type_checks() {
    for path in every_spec_file() {
        if let Err((diags, sources)) = check_file(&path) {
            panic!("{}", sources.render(&diags));
        }
    }
}

#[test]
fn variant_result_example_runs() {
    run_in_interpreter("variant_result.rx");
    if let Some(out) = run_in_c("variant_result.rx") {
        assert_eq!(out, "42\nfailed\n3\n");
    }
}

#[test]
fn layer_button_example_builds_and_runs_against_the_runtime() {
    // button_view and friends live in the C runtime only.
    if let Some(out) = run_in_c("layer_button.rx") {
        assert!(out.ends_with("Press me\nbuilt\n"), "unexpected output: {:?}", out);
    }
}

#[test]
fn protocol_drawable_example_runs() {
    run_in_interpreter("protocol_drawable.rx");
    if let Some(out) = run_in_c("protocol_drawable.rx") {
        assert_eq!(out, "drawing circle\n3\n");
    }
}

#[test]
fn spec_example_sources_are_in_the_corpus_directory() {
    assert!(Path::new(&crate_root().join("tests/spec/shapes.rx")).is_file());
}
