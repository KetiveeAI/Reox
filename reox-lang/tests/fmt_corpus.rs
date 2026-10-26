// Formatter corpus test: `reoxc fmt` must be idempotent on every REOX source
// file in the repository and must never change the token stream the compiler
// sees (whitespace and trailing commas in literals are the only differences).

use std::fs;
use std::path::{Path, PathBuf};

use reoxc::fmt::format_source;
use reoxc::lexer::{tokenize, TokenKind};

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for path in entries {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if path.is_dir() {
            if !name.starts_with('.') && name != "target" && name != "node_modules" {
                collect(&path, out);
            }
        } else if name.ends_with(".rx") || name.ends_with(".reox") {
            out.push(path);
        }
    }
}

fn corpus() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    for sub in ["tests", "examples", "stdlib"] {
        collect(&root.join(sub), &mut files);
    }
    // The rest of the repository, when the crate sits inside it.
    if let Some(repo) = root.parent() {
        for sub in ["reox", "Calculator.app", "reox-lsp/tests/fixtures"] {
            collect(&repo.join(sub), &mut files);
        }
    }
    files
}

/// Token kinds with literal payloads, ignoring positions.
fn kinds(source: &str) -> Vec<TokenKind> {
    tokenize(source)
        .map(|toks| toks.into_iter().map(|t| t.kind).collect())
        .unwrap_or_default()
}

#[test]
fn corpus_is_non_empty() {
    assert!(corpus().len() >= 20, "expected the repository corpus, found {}", corpus().len());
}

#[test]
fn formatting_is_idempotent_on_every_source_file() {
    let mut failures = Vec::new();
    for path in corpus() {
        let source = fs::read_to_string(&path).unwrap_or_default();
        let once = match format_source(&source) {
            Ok(s) => s,
            Err(d) => {
                failures.push(format!("{}: {}", path.display(), d.message));
                continue;
            }
        };
        match format_source(&once) {
            Ok(twice) if twice == once => {}
            Ok(twice) => {
                let line = once
                    .lines()
                    .zip(twice.lines())
                    .position(|(a, b)| a != b)
                    .unwrap_or(0);
                failures.push(format!(
                    "{}: not idempotent at line {}:\n  once:  {:?}\n  twice: {:?}",
                    path.display(),
                    line + 1,
                    once.lines().nth(line).unwrap_or(""),
                    twice.lines().nth(line).unwrap_or("")
                ));
            }
            Err(d) => failures.push(format!("{}: second pass failed: {}", path.display(), d.message)),
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

#[test]
fn formatting_preserves_the_token_stream() {
    let mut failures = Vec::new();
    for path in corpus() {
        let source = fs::read_to_string(&path).unwrap_or_default();
        let Ok(formatted) = format_source(&source) else {
            continue;
        };
        let before = kinds(&source);
        let after = kinds(&formatted);
        // Trailing commas may be added inside literals, nothing else.
        let before_no_commas: Vec<&TokenKind> = before.iter().filter(|k| **k != TokenKind::Comma).collect();
        let after_no_commas: Vec<&TokenKind> = after.iter().filter(|k| **k != TokenKind::Comma).collect();
        if before_no_commas != after_no_commas {
            let at = before_no_commas
                .iter()
                .zip(after_no_commas.iter())
                .position(|(a, b)| a != b)
                .unwrap_or(before_no_commas.len().min(after_no_commas.len()));
            failures.push(format!(
                "{}: token stream changed at token {} ({:?} -> {:?})",
                path.display(),
                at,
                before_no_commas.get(at),
                after_no_commas.get(at)
            ));
        }
        if after.len() < before.len() {
            failures.push(format!("{}: formatter dropped commas", path.display()));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

#[test]
fn formatting_preserves_every_comment() {
    let mut failures = Vec::new();
    for path in corpus() {
        let source = fs::read_to_string(&path).unwrap_or_default();
        let Ok(formatted) = format_source(&source) else {
            continue;
        };
        let before = source.matches("//").count() + source.matches("/*").count();
        let after = formatted.matches("//").count() + formatted.matches("/*").count();
        if before != after {
            failures.push(format!("{}: comment markers {} -> {}", path.display(), before, after));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

#[test]
fn formatting_keeps_files_parseable() {
    // Files that parse before formatting must still parse (and type check the
    // same way) afterwards.
    let mut failures = Vec::new();
    for path in corpus() {
        let source = fs::read_to_string(&path).unwrap_or_default();
        let Ok(formatted) = format_source(&source) else {
            continue;
        };
        let before = reoxc::cli::check_source(&source).map(|_| ()).map_err(|d| d.len());
        let after = reoxc::cli::check_source(&formatted).map(|_| ()).map_err(|d| d.len());
        if before != after {
            failures.push(format!("{}: diagnostics changed {:?} -> {:?}", path.display(), before, after));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}
