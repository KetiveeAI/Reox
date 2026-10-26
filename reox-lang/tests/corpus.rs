// REOX crash corpus
//
// Runs the front end (lexer, parser, type checker) over every REOX source
// file in the repository and in the NeolyxOS desktop apps, plus a set of
// deliberately malformed inputs. A file may produce diagnostics; it must
// never panic, overflow the stack or hang.

use std::path::{Path, PathBuf};

use reoxc::cli::check_source;
use reoxc::diagnostic::Diagnostic;

/// Directories scanned for `*.rx` and `*.reox`, relative to the crate root
/// unless absolute. Directories that do not exist are skipped, so the corpus
/// still runs on a machine without the NeolyxOS checkout.
const CORPUS_DIRS: &[&str] = &[
    "tests",
    "examples",
    "stdlib",
    "../reox/stdlib",
    "../Calculator.app",
    "/home/swa/NEOLYXOS/neolyx-os/desktop/apps",
];

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn is_reox_source(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("rx") | Some("reox")
    )
}

fn collect_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_sources(&path, out);
        } else if is_reox_source(&path) {
            out.push(path);
        }
    }
}

fn corpus_files() -> Vec<PathBuf> {
    let root = crate_root();
    let mut files = Vec::new();
    for dir in CORPUS_DIRS {
        let dir = if Path::new(dir).is_absolute() {
            PathBuf::from(dir)
        } else {
            root.join(dir)
        };
        collect_sources(&dir, &mut files);
    }
    files.sort();
    files.dedup();
    files
}

/// Outcome of running the front end over one input.
enum Outcome {
    Accepted,
    Diagnostics(Vec<Diagnostic>),
    Panicked(String),
}

fn run_front_end(source: &str) -> Outcome {
    let source = source.to_string();
    let result = std::panic::catch_unwind(move || check_source(&source));
    match result {
        Ok(Ok(_)) => Outcome::Accepted,
        Ok(Err(diagnostics)) => Outcome::Diagnostics(diagnostics),
        Err(payload) => {
            let message = payload
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "non-string panic payload".to_string());
            Outcome::Panicked(message)
        }
    }
}

fn check_no_panic(name: &str, source: &str) -> Vec<Diagnostic> {
    match run_front_end(source) {
        Outcome::Accepted => Vec::new(),
        Outcome::Diagnostics(d) => {
            assert!(!d.is_empty(), "{}: front end failed without diagnostics", name);
            for diag in &d {
                assert!(!diag.message.is_empty(), "{}: empty diagnostic message", name);
            }
            d
        }
        Outcome::Panicked(message) => panic!("{}: front end panicked: {}", name, message),
    }
}

#[test]
fn every_corpus_file_lexes_parses_and_typechecks_without_panicking() {
    let files = corpus_files();
    assert!(
        files.len() >= 20,
        "expected the repository corpus to hold at least 20 REOX files, found {}",
        files.len()
    );

    let mut accepted = 0usize;
    let mut with_diagnostics = Vec::new();
    for path in &files {
        let source = match std::fs::read(path) {
            Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            Err(e) => panic!("cannot read corpus file {}: {}", path.display(), e),
        };
        let name = path.display().to_string();
        let diagnostics = check_no_panic(&name, &source);
        if diagnostics.is_empty() {
            accepted += 1;
        } else {
            // Every diagnostic must render without panicking as well.
            for d in &diagnostics {
                let _ = d.render(&name, &source);
            }
            with_diagnostics.push((name, diagnostics.len()));
        }
    }

    eprintln!(
        "corpus: {} files, {} accepted, {} with diagnostics",
        files.len(),
        accepted,
        with_diagnostics.len()
    );
    for (name, count) in &with_diagnostics {
        eprintln!("  {}: {} diagnostic(s)", name, count);
    }
}

#[test]
fn neolyx_desktop_apps_are_in_the_corpus_when_present() {
    let apps = Path::new("/home/swa/NEOLYXOS/neolyx-os/desktop/apps");
    if !apps.is_dir() {
        eprintln!("NeolyxOS checkout not present, skipping");
        return;
    }
    let mut files = Vec::new();
    collect_sources(apps, &mut files);
    assert!(
        !files.is_empty(),
        "expected .reox files under {}",
        apps.display()
    );
    for path in files {
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {}", path.display(), e));
        check_no_panic(&path.display().to_string(), &source);
    }
}

/// Deliberately malformed inputs. Each must produce at least one diagnostic
/// and must never panic.
fn malformed_inputs() -> Vec<(&'static str, String)> {
    let deep_parens = format!("fn main() {{ let x = {}1{}; }}", "(".repeat(5000), ")".repeat(5000));
    let deep_blocks = format!("fn main() {}{}", "{".repeat(5000), "}".repeat(5000));
    let deep_unary = format!("fn main() {{ let x = {}1; }}", "-".repeat(5000));
    let deep_arrays = format!("fn main() {{ let x = {}1{}; }}", "[".repeat(5000), "]".repeat(5000));
    let deep_types = format!("fn f(a: {}int{}) {{ }}", "[".repeat(5000), "]".repeat(5000));
    let deep_else_if = format!("fn main() {{ if true {{ }} {} }}", "else if true { }".repeat(5000));
    let deep_calls = format!("fn main() {{ {}f{}; }}", "f(".repeat(5000), ")".repeat(5000));
    let long_ident = format!("fn main() {{ let {} = 1; }}", "a".repeat(100_000));
    let many_tokens = format!("fn main() {{ let x = {}1; }}", "1 + ".repeat(50_000));

    vec![
        ("empty", String::new()),
        ("whitespace only", "   \n\t  \n".to_string()),
        ("unterminated string", "fn main() { let s = \"abc; }".to_string()),
        ("unterminated string at eof", "fn main() { let s = \"".to_string()),
        ("unterminated interpolation", "fn main() { let s = \"a \\(b\"; }".to_string()),
        ("unterminated block comment", "fn main() { } /* never closed".to_string()),
        ("nested unterminated block comment", "/* outer /* inner */ still open".to_string()),
        ("bad escape", "fn main() { let s = \"\\q\"; }".to_string()),
        ("unbalanced open brace", "fn main() {".to_string()),
        ("unbalanced close brace", "fn main() { } }".to_string()),
        ("unbalanced parens", "fn main() { let x = (1 + 2; }".to_string()),
        ("unbalanced brackets", "fn main() { let x = [1, 2; }".to_string()),
        ("bad token", "fn main() { let x = 1 $ 2; }".to_string()),
        ("bad token backtick", "fn main() { `x }".to_string()),
        ("non-ascii identifier", "fn main() { let café = 1; let naïve = café; }".to_string()),
        ("non-ascii identifier at eof", "fn main() { let x = café".to_string()),
        ("unexpected unicode character", "fn main() { let x = 1 → 2; }".to_string()),
        ("emoji in source", "fn main() { let 😀 = 1; }".to_string()),
        ("huge integer literal", "fn main() { let x = 99999999999999999999999999; }".to_string()),
        ("hex without digits", "fn main() { let x = 0x; }".to_string()),
        ("binary without digits", "fn main() { let x = 0b; }".to_string()),
        ("float with trailing dot", "fn main() { let x = 1.; }".to_string()),
        ("keyword as identifier", "fn fn() { }".to_string()),
        ("missing semicolon", "fn main() { let x = 1 let y = 2; }".to_string()),
        ("break without semicolon", "fn main() { while true { break } }".to_string()),
        ("stray else", "fn main() { else { } }".to_string()),
        ("stray operator", "fn main() { let x = * 2; }".to_string()),
        ("assignment to literal", "fn main() { 1 = 2; }".to_string()),
        ("struct without body", "struct S".to_string()),
        ("struct literal unclosed", "fn main() { let p = Point { x: 1, }".to_string()),
        ("match unclosed", "fn main() { let v = match 1 { 1 => 2 }".to_string()),
        ("match bad pattern", "fn main() { let v = match 1 { + => 2 }; }".to_string()),
        ("variant unclosed", "variant V { A, B(".to_string()),
        ("protocol bad body", "protocol P { let x = 1; }".to_string()),
        ("layer bad body", "layer L { 123 }".to_string()),
        ("panel bad body", "panel P { + }".to_string()),
        ("pub alone", "pub".to_string()),
        ("static alone", "static".to_string()),
        ("async without fn", "async struct S { }".to_string()),
        ("import bad path", "import a::;".to_string()),
        ("extern unclosed", "extern fn f(".to_string()),
        ("const without value", "const X: int;".to_string()),
        ("typealias unclosed", "typealias T =".to_string()),
        ("bind without let", "fn main() { @Bind x = 1; }".to_string()),
        ("at without bind", "fn main() { @Other let x = 1; }".to_string()),
        ("emit unclosed", "fn main() { emit sig(1; }".to_string()),
        ("guard without else", "fn main() { guard true { } }".to_string()),
        ("try without catch", "fn main() { try { } }".to_string()),
        ("throw without value", "fn main() { throw; }".to_string()),
        ("for without in", "fn main() { for x 1..3 { } }".to_string()),
        ("call undefined", "fn main() { undefined_fn(1, 2); }".to_string()),
        ("type mismatch", "fn main() { let x: int = \"s\"; }".to_string()),
        ("undefined variable", "fn main() { let y = x + 1; }".to_string()),
        ("duplicate function", "fn f() { } fn f() { }".to_string()),
        ("duplicate struct", "struct S { } struct S { }".to_string()),
        ("if non-bool", "fn main() { if 1 { } }".to_string()),
        ("return type mismatch", "fn f() -> int { return \"s\"; }".to_string()),
        ("index non-array", "fn main() { let x = 1; let y = x[0]; }".to_string()),
        ("member of int", "fn main() { let x = 1; let y = x.field; }".to_string()),
        ("iterate over string", "fn main() { for c in \"abc\" { } }".to_string()),
        ("deep parens", deep_parens),
        ("deep blocks", deep_blocks),
        ("deep unary", deep_unary),
        ("deep arrays", deep_arrays),
        ("deep types", deep_types),
        ("deep else if", deep_else_if),
        ("deep calls", deep_calls),
        ("very long identifier", long_ident),
        ("many tokens", many_tokens),
        ("null bytes", "fn main() { \0\0\0 }".to_string()),
        ("only closing braces", "}}}}}}}}".to_string()),
        ("only operators", "+ - * / % == != < > <= >= && || ! & | ^ ~ << >> ?? ?. .. -> =>".to_string()),
        ("truncated keyword", "f".to_string()),
        ("lonely at", "@".to_string()),
        ("crlf line endings", "fn main() {\r\n  let x = ;\r\n}\r\n".to_string()),
        ("tabs before error", "fn main() {\n\t\tlet x = ;\n}\n".to_string()),
    ]
}

#[test]
fn malformed_inputs_produce_diagnostics_not_panics() {
    for (name, source) in malformed_inputs() {
        let outcome = run_front_end(&source);
        match outcome {
            Outcome::Accepted => {
                // Some inputs (e.g. non-ASCII identifiers, empty files) are valid programs.
                // The point of this test is that nothing panics.
            }
            Outcome::Diagnostics(diagnostics) => {
                assert!(!diagnostics.is_empty(), "{}: failed without diagnostics", name);
                for d in &diagnostics {
                    assert!(!d.message.is_empty(), "{}: empty message", name);
                    let rendered = d.render("input.rx", &source);
                    assert!(rendered.starts_with("input.rx:"), "{}: bad render {}", name, rendered);
                }
            }
            Outcome::Panicked(message) => panic!("{}: front end panicked: {}", name, message),
        }
    }
}

#[test]
fn malformed_syntax_is_rejected() {
    // Inputs that must be rejected, not merely survive.
    let must_fail = [
        "fn main() { let s = \"abc; }",
        "fn main() {",
        "fn main() { } }",
        "fn main() { let x = 1 $ 2; }",
        "fn main() { } /* never closed",
        &format!("fn main() {{ let x = {}1{}; }}", "(".repeat(5000), ")".repeat(5000)),
        "fn main() { let x: int = \"s\"; }",
    ];
    for source in must_fail {
        match run_front_end(source) {
            Outcome::Diagnostics(d) => assert!(!d.is_empty()),
            Outcome::Accepted => panic!("accepted malformed input: {:?}", &source[..source.len().min(60)]),
            Outcome::Panicked(m) => panic!("panicked on {:?}: {}", &source[..source.len().min(60)], m),
        }
    }
}

#[test]
fn deep_nesting_is_reported_as_a_nesting_error() {
    let source = format!("fn main() {{ let x = {}1{}; }}", "(".repeat(5000), ")".repeat(5000));
    match run_front_end(&source) {
        Outcome::Diagnostics(d) => {
            assert!(d[0].message.contains("nested deeper than"), "{}", d[0].message);
            assert!(d[0].hint.is_some());
        }
        Outcome::Accepted => panic!("deeply nested input was accepted"),
        Outcome::Panicked(m) => panic!("deeply nested input panicked: {}", m),
    }
}
