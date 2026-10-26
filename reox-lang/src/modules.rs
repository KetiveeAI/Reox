// REOX Compiler - Module loader
//
// `import a.b.c;` resolves `a/b/c.rx` (or `.reox`) relative to the importing
// file's directory, then the entry file's directory, then the standard
// library. Every file is a module named after its last path segment; the
// loader parses each file once, tags every span with the file it came from,
// detects import cycles and merges all declarations into one program that
// the type checker checks with the visibility rules in `ModuleMap`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::diagnostic::Diagnostic;
use crate::lexer::Span;
use crate::parser::{Ast, Decl, ImportDecl};

/// Source file extensions tried, in order, for a module path.
const EXTENSIONS: &[&str] = &["rx", "reox"];

/// Environment variable that points at the standard library directory.
pub const STDLIB_ENV: &str = "REOX_STDLIB";

/// One loaded source file.
#[derive(Debug, Clone)]
pub struct SourceFile {
    pub path: PathBuf,
    pub source: String,
}

/// Every file of a program, indexed by the `file` id spans carry.
#[derive(Debug, Clone, Default)]
pub struct SourceMap {
    pub files: Vec<SourceFile>,
}

impl SourceMap {
    /// A map holding a single file, for programs given as one string.
    pub fn single(path: impl Into<PathBuf>, source: impl Into<String>) -> Self {
        Self { files: vec![SourceFile { path: path.into(), source: source.into() }] }
    }

    pub fn get(&self, file: u32) -> Option<&SourceFile> {
        self.files.get(file as usize)
    }

    /// Path shown in diagnostics for `file`.
    pub fn display_path(&self, file: u32) -> String {
        self.get(file).map(|f| f.path.display().to_string()).unwrap_or_else(|| "<unknown>".to_string())
    }

    /// Render diagnostics, each against the file its span belongs to.
    pub fn render(&self, diagnostics: &[Diagnostic]) -> String {
        diagnostics
            .iter()
            .map(|d| match self.get(d.span.file) {
                Some(f) => d.render(&f.path.display().to_string(), &f.source),
                None => d.header("<unknown>"),
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

/// Which file owns which module, and who imports whom. Consulted by the
/// type checker for `pub` visibility and qualified `module.item` access.
#[derive(Debug, Clone, Default)]
pub struct ModuleMap {
    /// Module name of each file id (`a.b.c` imports module `c`).
    pub names: Vec<String>,
    /// File ids each file imports directly.
    pub imports: Vec<Vec<u32>>,
}

impl ModuleMap {
    pub fn name(&self, file: u32) -> &str {
        self.names.get(file as usize).map(String::as_str).unwrap_or("<unknown>")
    }

    pub fn imports(&self, file: u32, other: u32) -> bool {
        self.imports.get(file as usize).map(|v| v.contains(&other)).unwrap_or(false)
    }

    /// File id of the module `name` as seen from `file`, if `file` imports it.
    pub fn imported_module(&self, file: u32, name: &str) -> Option<u32> {
        self.imports.get(file as usize)?.iter().copied().find(|id| self.name(*id) == name)
    }
}

/// A loaded multi-file program.
#[derive(Debug, Clone)]
pub struct Program {
    pub ast: Ast,
    pub sources: SourceMap,
    pub modules: ModuleMap,
}

/// What the loader reports on failure: the diagnostics plus every file read
/// so far, so they can be rendered.
#[derive(Debug, Clone)]
pub struct LoadError {
    pub diagnostics: Vec<Diagnostic>,
    pub sources: SourceMap,
}

/// Directories searched for standard library modules, in order.
pub fn stdlib_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(dir) = std::env::var(STDLIB_ENV) {
        if !dir.is_empty() {
            dirs.push(PathBuf::from(dir));
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(bin) = exe.parent() {
            dirs.push(bin.join("stdlib"));
            dirs.push(bin.join("../stdlib"));
            dirs.push(bin.join("../../stdlib"));
            dirs.push(bin.join("../share/reox/stdlib"));
        }
    }
    dirs.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("stdlib"));
    dirs
}

/// Load the program rooted at `entry`, reading it and every import from disk.
pub fn load_program(entry: &Path) -> Result<Program, LoadError> {
    let source = match std::fs::read_to_string(entry) {
        Ok(s) => s,
        Err(e) => {
            return Err(LoadError {
                diagnostics: vec![Diagnostic::error(format!("failed to read '{}': {}", entry.display(), e), Span::default())],
                sources: SourceMap::default(),
            })
        }
    };
    load_program_from_source(entry, &source)
}

/// Load the program whose entry file is `entry` with the given contents
/// (imports are still read from disk relative to `entry`).
pub fn load_program_from_source(entry: &Path, source: &str) -> Result<Program, LoadError> {
    let mut loader = Loader::new(entry);
    let root_id = loader.add_file(entry.to_path_buf(), source.to_string());
    let mut stack = Vec::new();
    loader.load(root_id, &mut stack);
    loader.finish()
}

struct Loader {
    entry_dir: PathBuf,
    sources: SourceMap,
    modules: ModuleMap,
    /// Parsed declarations per file id.
    decls: Vec<Option<Vec<Decl>>>,
    /// Canonical path -> file id, so a module is loaded once.
    by_path: HashMap<PathBuf, u32>,
    diagnostics: Vec<Diagnostic>,
}

impl Loader {
    fn new(entry: &Path) -> Self {
        let entry_dir = entry.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));
        Self {
            entry_dir,
            sources: SourceMap::default(),
            modules: ModuleMap::default(),
            decls: Vec::new(),
            by_path: HashMap::new(),
            diagnostics: Vec::new(),
        }
    }

    fn add_file(&mut self, path: PathBuf, source: String) -> u32 {
        let id = self.sources.files.len() as u32;
        let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
        self.by_path.insert(canonical, id);
        let name = module_name(&path);
        self.sources.files.push(SourceFile { path, source });
        self.modules.names.push(name);
        self.modules.imports.push(Vec::new());
        self.decls.push(None);
        id
    }

    /// Parse file `id` and load its imports depth-first. `stack` holds the
    /// files whose imports are being resolved, for cycle detection.
    fn load(&mut self, id: u32, stack: &mut Vec<u32>) {
        let source = match self.sources.get(id) {
            Some(f) => f.source.clone(),
            None => return,
        };
        let mut tokens = match crate::lexer::tokenize(&source) {
            Ok(tokens) => tokens,
            Err(e) => {
                self.push(Diagnostic::from(e), id);
                self.decls[id as usize] = Some(Vec::new());
                return;
            }
        };
        for token in tokens.iter_mut() {
            token.span.file = id;
        }
        let ast = match crate::parser::parse(&tokens) {
            Ok(ast) => ast,
            Err(e) => {
                self.push(Diagnostic::from(e), id);
                self.decls[id as usize] = Some(Vec::new());
                return;
            }
        };

        stack.push(id);
        for decl in &ast.declarations {
            if let Decl::Import(import) = decl {
                self.load_import(id, import, stack);
            }
        }
        stack.pop();
        self.decls[id as usize] = Some(ast.declarations);
    }

    fn load_import(&mut self, from: u32, import: &ImportDecl, stack: &mut Vec<u32>) {
        let module = import.path.join(".");
        let importer_dir = self.sources.get(from)
            .and_then(|f| f.path.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| PathBuf::from("."));
        let mut searched = Vec::new();
        let found = resolve_module(&import.path, &importer_dir, &self.entry_dir, &mut searched);
        let path = match found {
            Some(path) => path,
            None => {
                let mut span = import.span;
                span.file = from;
                self.diagnostics.push(
                    Diagnostic::error(format!("cannot find module '{}'", module), span).with_hint(format!(
                        "looked for {}",
                        searched.iter().map(|p| format!("'{}'", p.display())).collect::<Vec<_>>().join(", ")
                    )),
                );
                return;
            }
        };
        let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
        let target = match self.by_path.get(&canonical) {
            Some(id) => *id,
            None => {
                let source = match std::fs::read_to_string(&path) {
                    Ok(s) => s,
                    Err(e) => {
                        let mut span = import.span;
                        span.file = from;
                        self.diagnostics.push(Diagnostic::error(
                            format!("failed to read module '{}' at '{}': {}", module, path.display(), e),
                            span,
                        ));
                        return;
                    }
                };
                self.add_file(path, source)
            }
        };
        if !self.modules.imports[from as usize].contains(&target) {
            self.modules.imports[from as usize].push(target);
        }
        if stack.contains(&target) {
            let mut cycle: Vec<String> = stack
                .iter()
                .skip_while(|id| **id != target)
                .map(|id| self.modules.name(*id).to_string())
                .collect();
            cycle.push(self.modules.name(target).to_string());
            let mut span = import.span;
            span.file = from;
            self.diagnostics.push(
                Diagnostic::error(format!("import cycle: {}", cycle.join(" -> ")), span)
                    .with_hint("move the shared declarations into a third module that both import"),
            );
            return;
        }
        if self.decls[target as usize].is_none() {
            self.load(target, stack);
        }
    }

    fn push(&mut self, mut diagnostic: Diagnostic, file: u32) {
        diagnostic.span.file = file;
        self.diagnostics.push(diagnostic);
    }

    fn finish(self) -> Result<Program, LoadError> {
        if !self.diagnostics.is_empty() {
            return Err(LoadError { diagnostics: self.diagnostics, sources: self.sources });
        }
        // Imported modules first (they were added after the entry file), the
        // entry file last, so declaration order follows dependency order.
        let mut declarations = Vec::new();
        let mut files: Vec<Vec<Decl>> = self.decls.into_iter().map(|d| d.unwrap_or_default()).collect();
        let entry = files.remove(0);
        for decls in files {
            declarations.extend(decls);
        }
        declarations.extend(entry);
        Ok(Program { ast: Ast { declarations }, sources: self.sources, modules: self.modules })
    }
}

/// Module name of a file: its stem (`a/b/c.rx` is module `c`).
pub fn module_name(path: &Path) -> String {
    path.file_stem().and_then(|s| s.to_str()).unwrap_or("module").to_string()
}

/// Find the file for an import path, recording every location tried.
fn resolve_module(segments: &[String], importer_dir: &Path, entry_dir: &Path, searched: &mut Vec<PathBuf>) -> Option<PathBuf> {
    let relative: PathBuf = segments.iter().collect();
    let mut bases: Vec<PathBuf> = vec![importer_dir.to_path_buf()];
    if entry_dir != importer_dir {
        bases.push(entry_dir.to_path_buf());
    }
    bases.extend(stdlib_dirs());
    for base in bases {
        for ext in EXTENSIONS {
            let candidate = base.join(&relative).with_extension(ext);
            if candidate.is_file() {
                return Some(candidate);
            }
            if !searched.contains(&candidate) {
                searched.push(candidate);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("reox-modules-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    fn write(dir: &Path, rel: &str, content: &str) -> PathBuf {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("parent dir");
        }
        std::fs::write(&path, content).expect("write");
        path
    }

    #[test]
    fn imports_resolve_relative_to_the_entry_file_and_merge() {
        let dir = temp_dir("merge");
        write(&dir, "geo/shapes.rx", "pub struct Circle { r: int }\npub fn area(c: Circle) -> int { return c.r * c.r * 3; }");
        let main = write(&dir, "main.rx", "import geo.shapes;\nfn main() -> int { return area(Circle { r: 2 }); }");
        let program = load_program(&main).unwrap_or_else(|e| panic!("{:?}", e.diagnostics));
        assert_eq!(program.sources.files.len(), 2);
        assert_eq!(program.modules.names, vec!["main".to_string(), "shapes".to_string()]);
        assert!(program.modules.imports(0, 1));
        // Imported declarations come first; spans carry their file id.
        let names: Vec<String> = program.ast.declarations.iter().filter_map(|d| match d {
            Decl::Struct(s) => Some(s.name.clone()),
            Decl::Function(f) => Some(f.name.clone()),
            _ => None,
        }).collect();
        assert_eq!(names, vec!["Circle", "area", "main"]);
        match &program.ast.declarations[0] {
            Decl::Struct(s) => assert_eq!(s.span.file, 1),
            other => panic!("{:?}", other),
        }
        match program.ast.declarations.last() {
            Some(Decl::Function(f)) => assert_eq!(f.span.file, 0),
            other => panic!("{:?}", other),
        }
    }

    #[test]
    fn a_module_is_loaded_once_and_stdlib_is_found() {
        let dir = temp_dir("once");
        write(&dir, "a.rx", "import prelude;\npub fn fa() -> int { return 1; }");
        write(&dir, "b.rx", "import prelude;\nimport a;\npub fn fb() -> int { return fa(); }");
        let main = write(&dir, "main.rx", "import a;\nimport b;\nfn main() -> int { return fa() + fb(); }");
        let program = load_program(&main).unwrap_or_else(|e| panic!("{:?}", e.diagnostics));
        assert_eq!(program.sources.files.len(), 4, "{:?}", program.modules.names);
        assert_eq!(program.modules.name(0), "main");
        assert!(program.modules.names.contains(&"prelude".to_string()));
    }

    #[test]
    fn missing_modules_and_cycles_are_diagnostics() {
        let dir = temp_dir("errors");
        let main = write(&dir, "main.rx", "import nowhere.at.all;\nfn main() { }");
        let err = load_program(&main).unwrap_err();
        assert_eq!(err.diagnostics.len(), 1);
        assert_eq!(err.diagnostics[0].message, "cannot find module 'nowhere.at.all'");
        assert!(err.diagnostics[0].hint.as_deref().unwrap_or("").contains("nowhere/at/all.rx"));
        assert!(err.sources.render(&err.diagnostics).starts_with(&format!("{}:1:1: error: cannot find module", main.display())));

        write(&dir, "x.rx", "import y;\npub fn fx() { }");
        write(&dir, "y.rx", "import x;\npub fn fy() { }");
        let main = write(&dir, "cyc.rx", "import x;\nfn main() { }");
        let err = load_program(&main).unwrap_err();
        assert_eq!(err.diagnostics[0].message, "import cycle: x -> y -> x");
        assert_eq!(err.diagnostics[0].span.file, 2);
        assert!(err.diagnostics[0].hint.is_some());
    }

    #[test]
    fn errors_inside_an_imported_file_name_that_file() {
        let dir = temp_dir("imported-error");
        write(&dir, "bad.rx", "pub fn oops( { }");
        let main = write(&dir, "main.rx", "import bad;\nfn main() { }");
        let err = load_program(&main).unwrap_err();
        assert_eq!(err.diagnostics[0].span.file, 1);
        let rendered = err.sources.render(&err.diagnostics);
        assert!(rendered.contains("bad.rx:1:"), "{}", rendered);
    }
}
