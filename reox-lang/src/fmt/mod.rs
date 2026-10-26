// REOX Formatter - `reoxc fmt`.
//
// Token-based, comment-preserving, idempotent. See docs/REOX_STYLE.md for the
// style it enforces. The entry points are `format_source` (library) and `run`
// (CLI driver used by `reoxc fmt`).

mod layout;
mod tokens;

use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use crate::diagnostic::Diagnostic;

/// Format REOX source text. Fails with a diagnostic when the input does not
/// lex or its brackets are unbalanced; the text is left untouched in that case.
pub fn format_source(source: &str) -> Result<String, Diagnostic> {
    let toks = tokens::scan(source)?;
    layout::format_tokens(toks)
}

/// Options parsed from `reoxc fmt` arguments.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FmtOptions {
    /// Report files that would change and exit non-zero instead of writing.
    pub check: bool,
    /// Read one file from stdin and write the result to stdout.
    pub stdin: bool,
    /// Files or directories (directories are searched for .rx/.reox).
    pub paths: Vec<String>,
}

pub fn parse_fmt_args(args: &[String]) -> Result<FmtOptions, String> {
    let mut opts = FmtOptions::default();
    for arg in args {
        match arg.as_str() {
            "--check" | "-c" => opts.check = true,
            "--stdin" | "-" => opts.stdin = true,
            "-h" | "--help" => return Err(usage()),
            other if other.starts_with('-') => return Err(format!("unknown fmt option: {}\n\n{}", other, usage())),
            path => opts.paths.push(path.to_string()),
        }
    }
    if opts.stdin && !opts.paths.is_empty() {
        return Err("fmt: --stdin cannot be combined with file paths".to_string());
    }
    if !opts.stdin && opts.paths.is_empty() {
        return Err(format!("fmt: no input files\n\n{}", usage()));
    }
    Ok(opts)
}

pub fn usage() -> String {
    [
        "reoxc fmt - format REOX source files",
        "",
        "USAGE:",
        "    reoxc fmt [--check] <FILE|DIR>...",
        "    reoxc fmt --stdin < input.rx",
        "",
        "OPTIONS:",
        "    --check, -c    Do not write; list files that would change and exit 1",
        "    --stdin, -     Format stdin to stdout",
        "    -h, --help     Show this help",
    ]
    .join("\n")
}

/// Run `reoxc fmt`. Returns the process exit code: 0 when every file is
/// formatted (or, with --check, already formatted), 1 otherwise.
pub fn run(args: &[String]) -> Result<i32, String> {
    let opts = parse_fmt_args(args)?;
    if opts.stdin {
        let mut source = String::new();
        io::stdin().read_to_string(&mut source).map_err(|e| format!("fmt: cannot read stdin: {}", e))?;
        return match format_source(&source) {
            Ok(formatted) => {
                if opts.check {
                    return Ok(if formatted == source { 0 } else { 1 });
                }
                io::stdout()
                    .write_all(formatted.as_bytes())
                    .map_err(|e| format!("fmt: cannot write stdout: {}", e))?;
                Ok(0)
            }
            Err(d) => {
                crate::cli::report_diagnostics("<stdin>", &source, &[d]);
                Ok(1)
            }
        };
    }

    let files = collect_files(&opts.paths)?;
    if files.is_empty() {
        return Err("fmt: no .rx or .reox files found".to_string());
    }
    let mut failures = 0usize;
    let mut changed = 0usize;
    for file in &files {
        let display = file.display().to_string();
        let source = match fs::read_to_string(file) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("fmt: cannot read {}: {}", display, e);
                failures += 1;
                continue;
            }
        };
        match format_source(&source) {
            Ok(formatted) => {
                if formatted == source {
                    continue;
                }
                changed += 1;
                if opts.check {
                    println!("would reformat: {}", display);
                } else if let Err(e) = fs::write(file, formatted) {
                    eprintln!("fmt: cannot write {}: {}", display, e);
                    failures += 1;
                }
            }
            Err(d) => {
                crate::cli::report_diagnostics(&display, &source, &[d]);
                failures += 1;
            }
        }
    }
    if failures > 0 {
        return Ok(1);
    }
    if opts.check && changed > 0 {
        eprintln!("{} file{} would be reformatted", changed, if changed == 1 { "" } else { "s" });
        return Ok(1);
    }
    Ok(0)
}

/// Expand paths: files are taken as given, directories are walked for
/// `.rx` and `.reox` files (sorted, `target`/`node_modules`/dot-dirs skipped).
pub fn collect_files(paths: &[String]) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    for p in paths {
        let path = Path::new(p);
        if path.is_dir() {
            walk(path, &mut out)?;
        } else if path.is_file() {
            out.push(path.to_path_buf());
        } else {
            return Err(format!("fmt: no such file or directory: {}", p));
        }
    }
    Ok(out)
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .map_err(|e| format!("fmt: cannot read directory {}: {}", dir.display(), e))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    entries.sort();
    for entry in entries {
        let name = entry.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if entry.is_dir() {
            if name.starts_with('.') || name == "target" || name == "node_modules" {
                continue;
            }
            walk(&entry, out)?;
        } else if name.ends_with(".rx") || name.ends_with(".reox") {
            out.push(entry);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fmt(src: &str) -> String {
        format_source(src).unwrap_or_else(|d| panic!("{}", d.render("test.rx", src)))
    }

    fn assert_fmt(src: &str, expected: &str) {
        let once = fmt(src);
        assert_eq!(once, expected, "\n--- formatted ---\n{}\n--- expected ---\n{}", once, expected);
        let twice = fmt(&once);
        assert_eq!(twice, once, "not idempotent:\n--- once ---\n{}\n--- twice ---\n{}", once, twice);
    }

    #[test]
    fn empty_and_whitespace_only_sources_format_to_nothing() {
        assert_eq!(fmt(""), "");
        assert_eq!(fmt("   \n\n\t\n"), "");
    }

    #[test]
    fn indents_blocks_with_four_spaces_and_kr_braces() {
        assert_fmt(
            "fn main()\n{\nlet x = 1;\n  if x > 0\n  {\n  println(\"pos\");\n  }\n  else {\n  println(\"neg\"); }\n}\n",
            "fn main() {\n    let x = 1;\n    if x > 0 {\n        println(\"pos\");\n    } else {\n        println(\"neg\");\n    }\n}\n",
        );
    }

    #[test]
    fn one_statement_per_line() {
        assert_fmt(
            "fn f() { a(); b(); return 1; }\n",
            "fn f() {\n    a();\n    b();\n    return 1;\n}\n",
        );
    }

    #[test]
    fn empty_block_stays_compact() {
        assert_fmt("fn f() {}\nfn g() {\n}\n", "fn f() {}\nfn g() {}\n");
    }

    #[test]
    fn operator_spacing() {
        assert_fmt(
            "fn f(){let a=1+2*3;let b=a==1&&!c||d!=-e;let r=0..n;a+=1;b-=1;x=y%2;z=a<<2|b&c^~d;}\n",
            "fn f() {\n    let a = 1 + 2 * 3;\n    let b = a == 1 && !c || d != -e;\n    let r = 0..n;\n    a += 1;\n    b -= 1;\n    x = y % 2;\n    z = a << 2 | b & c ^ ~d;\n}\n",
        );
    }

    #[test]
    fn unary_minus_and_postfix_operators() {
        assert_fmt(
            "fn f() { return - 1; }\nfn g() { let x = a - - b; i ++; -- j; let o: int ? = nil; let v = p ?. q ?? 3; }\n",
            "fn f() {\n    return -1;\n}\nfn g() {\n    let x = a - -b;\n    i++;\n    --j;\n    let o: int? = nil;\n    let v = p?.q ?? 3;\n}\n",
        );
    }

    #[test]
    fn calls_members_and_indexing_have_no_inner_spaces() {
        assert_fmt(
            "fn f() { foo ( a , b ) ; x . y . z ( ) ; arr [ 0 ] = arr [ i + 1 ] ; if (a) { } ; return (a + b) ; }\n",
            "fn f() {\n    foo(a, b);\n    x.y.z();\n    arr[0] = arr[i + 1];\n    if (a) {};\n    return (a + b);\n}\n",
        );
    }

    #[test]
    fn declarations_and_types() {
        assert_fmt(
            "struct Point{x:int,y:int}\nextern fn len(xs:[int])->int;\nextern fn each(xs: [int], f: fn (int) -> void) -> void;\nfn id(p:Point)->Point{return p;}\ntypealias Id=int;\nconst MAX:int=10;\nimport ui.widgets;\n",
            "struct Point { x: int, y: int }\nextern fn len(xs: [int]) -> int;\nextern fn each(xs: [int], f: fn(int) -> void) -> void;\nfn id(p: Point) -> Point {\n    return p;\n}\ntypealias Id = int;\nconst MAX: int = 10;\nimport ui.widgets;\n",
        );
    }

    #[test]
    fn multi_line_struct_decl_and_variant() {
        assert_fmt(
            "struct Color {\n  r: int, g: int,\n  b: int\n}\nvariant Dir {\n  Up, Down,\n  Left(x: int, y: int)\n}\n",
            "struct Color {\n    r: int,\n    g: int,\n    b: int\n}\nvariant Dir {\n    Up,\n    Down,\n    Left(x: int, y: int)\n}\n",
        );
    }

    #[test]
    fn trailing_commas_in_multi_line_literals_only() {
        assert_fmt(
            "fn f() {\n let p = Point {\n x: 1, y: 2\n };\n let q = Point { x: 1, y: 2 };\n let xs = [\n 1, 2,\n 3\n ];\n let ys = [1, 2, 3];\n foo(\n a,\n b\n );\n}\n",
            "fn f() {\n    let p = Point {\n        x: 1,\n        y: 2,\n    };\n    let q = Point { x: 1, y: 2 };\n    let xs = [\n        1,\n        2,\n        3,\n    ];\n    let ys = [1, 2, 3];\n    foo(\n        a,\n        b\n    );\n}\n",
        );
    }

    #[test]
    fn trailing_comma_goes_before_a_trailing_comment() {
        assert_fmt(
            "fn f() {\n let xs = [\n 1, // one\n 2 // two\n ];\n}\n",
            "fn f() {\n    let xs = [\n        1, // one\n        2, // two\n    ];\n}\n",
        );
    }

    #[test]
    fn comments_are_preserved_in_place() {
        assert_fmt(
            "// file header\n\n/* block\n   comment */\nfn main() { // trailing\n    // own line\n    let x = 1; /* mid */ let y = 2;\n}\n// tail\n",
            "// file header\n\n/* block\n   comment */\nfn main() { // trailing\n    // own line\n    let x = 1; /* mid */\n    let y = 2;\n}\n// tail\n",
        );
    }

    #[test]
    fn doc_comments_stay_attached() {
        assert_fmt(
            "/// Adds.\n///\n/// More.\nfn add(a: int, b: int) -> int { return a + b; }\n",
            "/// Adds.\n///\n/// More.\nfn add(a: int, b: int) -> int {\n    return a + b;\n}\n",
        );
    }

    #[test]
    fn blank_lines_collapse_to_one_and_vanish_at_block_edges() {
        assert_fmt(
            "\n\n\nfn a() {\n\n\n    x();\n\n\n\n    y();\n\n}\n\n\n\nfn b() {}\n",
            "fn a() {\n    x();\n\n    y();\n}\n\nfn b() {}\n",
        );
    }

    #[test]
    fn match_arms_one_per_line() {
        assert_fmt(
            "fn f(x: int) -> int { return match x { 1 => 10, 2 => { let y = 1; y }, _ => -1 }; }\n",
            "fn f(x: int) -> int {\n    return match x {\n        1 => 10,\n        2 => {\n            let y = 1;\n            y\n        },\n        _ => -1,\n    };\n}\n",
        );
    }

    #[test]
    fn match_arms_without_commas_keep_their_lines() {
        assert_fmt(
            "fn f(x: int) {\n    match x {\n        1 => a()\n        _ => b()\n    }\n}\n",
            "fn f(x: int) {\n    match x {\n        1 => a()\n        _ => b()\n    }\n}\n",
        );
    }

    #[test]
    fn continuation_lines_indent_one_level() {
        assert_fmt(
            "fn f() {\n    let total = a +\n      b +\n            c;\n    view\n        .padding(1)\n        .background(c);\n}\n",
            "fn f() {\n    let total = a +\n        b +\n        c;\n    view\n        .padding(1)\n        .background(c);\n}\n",
        );
    }

    #[test]
    fn ui_declarations() {
        assert_fmt(
            "layer Button {\n signal on_click;\n let label: string = \"Click\";\n on_tap { let x: int = 1; }\n on_swipe(direction: int) { let d: int = direction; }\n fn body() { button(\"x\") { count += 1; } }\n}\npanel Main { fn root() { let t = 1; } }\n",
            "layer Button {\n    signal on_click;\n    let label: string = \"Click\";\n    on_tap {\n        let x: int = 1;\n    }\n    on_swipe(direction: int) {\n        let d: int = direction;\n    }\n    fn body() {\n        button(\"x\") {\n            count += 1;\n        }\n    }\n}\npanel Main {\n    fn root() {\n        let t = 1;\n    }\n}\n",
        );
    }

    #[test]
    fn closures_inside_calls_keep_the_call_on_one_line() {
        assert_fmt(
            "fn f() {\n    on_click(button, action() {\n        count += 1;\n    });\n    let add = action(a: int, b: int) -> int { return a + b; };\n}\n",
            "fn f() {\n    on_click(button, action() {\n        count += 1;\n    });\n    let add = action(a: int, b: int) -> int {\n        return a + b;\n    };\n}\n",
        );
    }

    #[test]
    fn generics_and_decorators() {
        assert_fmt(
            "fn f() { let m: Map < string , int > = make(); let xs: Array<Array<int>> = [[1]]; let c = a < b && b > c; @Bind let s = 1; }\n",
            "fn f() {\n    let m: Map<string, int> = make();\n    let xs: Array<Array<int>> = [[1]];\n    let c = a < b && b > c;\n    @Bind let s = 1;\n}\n",
        );
    }

    #[test]
    fn string_literals_are_untouched() {
        assert_fmt(
            "fn f() { println(\"a  +  b \\(x+1) \\n\"); }\n",
            "fn f() {\n    println(\"a  +  b \\(x+1) \\n\");\n}\n",
        );
    }

    #[test]
    fn try_catch_guard_defer_and_protocol_extension() {
        assert_fmt(
            "protocol P { fn a() -> int; fn b(x: int); }\nextension Color: P, Q { fn a() -> int { return 1; } }\nfn f() { try { risky(); } catch e { log(e); } guard x > 0 else { return; } defer { close(); } }\n",
            "protocol P {\n    fn a() -> int;\n    fn b(x: int);\n}\nextension Color: P, Q {\n    fn a() -> int {\n        return 1;\n    }\n}\nfn f() {\n    try {\n        risky();\n    } catch e {\n        log(e);\n    }\n    guard x > 0 else {\n        return;\n    }\n    defer {\n        close();\n    }\n}\n",
        );
    }

    #[test]
    fn for_and_while_loops() {
        assert_fmt(
            "fn f() { for i in 0..10 { s += i; } for x in xs { print(x); } while i < 10 { i++; } }\n",
            "fn f() {\n    for i in 0..10 {\n        s += i;\n    }\n    for x in xs {\n        print(x);\n    }\n    while i < 10 {\n        i++;\n    }\n}\n",
        );
    }

    #[test]
    fn tabs_and_crlf_are_normalised() {
        assert_fmt("fn f() {\r\n\tx();\r\n}\r\n", "fn f() {\n    x();\n}\n");
    }

    #[test]
    fn errors_for_unlexable_or_unbalanced_input() {
        assert!(format_source("fn f() { let s = \"x; }").is_err());
        assert!(format_source("fn f() { ").is_err());
        assert!(format_source("fn f() ) ").is_err());
        assert!(format_source("fn f() { ] }").is_err());
    }

    #[test]
    fn parse_fmt_args_handles_flags() {
        let o = parse_fmt_args(&["--check".into(), "a.rx".into(), "dir".into()]).expect("ok");
        assert!(o.check);
        assert_eq!(o.paths, vec!["a.rx", "dir"]);
        let o = parse_fmt_args(&["-".into()]).expect("ok");
        assert!(o.stdin);
        assert!(parse_fmt_args(&[]).is_err());
        assert!(parse_fmt_args(&["--stdin".into(), "a.rx".into()]).is_err());
        assert!(parse_fmt_args(&["--bogus".into()]).is_err());
    }

    #[test]
    fn check_mode_reports_unformatted_files() {
        let dir = std::env::temp_dir().join(format!("reox-fmt-test-{}", std::process::id()));
        fs::create_dir_all(&dir).expect("tmp dir");
        let bad = dir.join("bad.rx");
        let good = dir.join("good.reox");
        let skipped = dir.join("notes.txt");
        fs::write(&bad, "fn f(){x();}\n").expect("write");
        fs::write(&good, "fn g() {\n    y();\n}\n").expect("write");
        fs::write(&skipped, "fn h(){}").expect("write");

        let files = collect_files(&[dir.display().to_string()]).expect("collect");
        assert_eq!(files, vec![bad.clone(), good.clone()]);

        let code = run(&["--check".into(), dir.display().to_string()]).expect("run");
        assert_eq!(code, 1);
        assert_eq!(fs::read_to_string(&bad).expect("read"), "fn f(){x();}\n", "check must not write");

        let code = run(&[dir.display().to_string()]).expect("run");
        assert_eq!(code, 0);
        assert_eq!(fs::read_to_string(&bad).expect("read"), "fn f() {\n    x();\n}\n");

        let code = run(&["--check".into(), dir.display().to_string()]).expect("run");
        assert_eq!(code, 0);

        assert!(run(&[dir.join("missing.rx").display().to_string()]).is_err());
        fs::remove_dir_all(&dir).expect("cleanup");
    }
}
