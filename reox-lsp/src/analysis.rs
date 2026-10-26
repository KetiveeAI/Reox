// REOX Language Server - per-document analysis.
//
// Runs the compiler front end (lexer, parser, type checker) from the `reoxc`
// library and builds a flat symbol index with byte ranges, so hover, go to
// definition, document symbols and completion are all lookups over one table.

use reoxc::diagnostic::{Diagnostic, Severity};
use reoxc::lexer::{Token, TokenKind};
use reoxc::parser::{
    Block, Decl, Expr, FnDecl, Literal, Param, Pattern, Stmt, Type,
};
use reoxc::parser::Ast;
use reoxc::Span;

/// What kind of thing a symbol is. The numeric mappings to LSP kinds live in
/// `lsp_symbol_kind` and `lsp_completion_kind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymKind {
    Function,
    Extern,
    Method,
    Struct,
    Field,
    Variant,
    VariantCase,
    Protocol,
    Extension,
    Layer,
    Panel,
    Signal,
    Gesture,
    Const,
    Typealias,
    Global,
    Local,
    Param,
    Import,
}

impl SymKind {
    pub fn label(self) -> &'static str {
        match self {
            SymKind::Function => "function",
            SymKind::Extern => "extern function",
            SymKind::Method => "method",
            SymKind::Struct => "struct",
            SymKind::Field => "field",
            SymKind::Variant => "variant",
            SymKind::VariantCase => "variant case",
            SymKind::Protocol => "protocol",
            SymKind::Extension => "extension",
            SymKind::Layer => "layer",
            SymKind::Panel => "panel",
            SymKind::Signal => "signal",
            SymKind::Gesture => "gesture",
            SymKind::Const => "constant",
            SymKind::Typealias => "type alias",
            SymKind::Global => "global",
            SymKind::Local => "local",
            SymKind::Param => "parameter",
            SymKind::Import => "import",
        }
    }

    /// LSP `SymbolKind` number.
    pub fn lsp_symbol_kind(self) -> u32 {
        match self {
            SymKind::Function | SymKind::Extern => 12,
            SymKind::Method | SymKind::Gesture => 6,
            SymKind::Struct => 23,
            SymKind::Field => 8,
            SymKind::Variant => 10,
            SymKind::VariantCase => 22,
            SymKind::Protocol => 11,
            SymKind::Extension | SymKind::Layer | SymKind::Panel => 5,
            SymKind::Signal => 24,
            SymKind::Const => 14,
            SymKind::Typealias => 26,
            SymKind::Global | SymKind::Local | SymKind::Param => 13,
            SymKind::Import => 2,
        }
    }

    /// LSP `CompletionItemKind` number.
    pub fn lsp_completion_kind(self) -> u32 {
        match self {
            SymKind::Function | SymKind::Extern => 3,
            SymKind::Method | SymKind::Gesture => 2,
            SymKind::Struct => 22,
            SymKind::Field => 5,
            SymKind::Variant => 13,
            SymKind::VariantCase => 20,
            SymKind::Protocol => 8,
            SymKind::Extension | SymKind::Layer | SymKind::Panel => 7,
            SymKind::Signal => 23,
            SymKind::Const => 21,
            SymKind::Typealias => 25,
            SymKind::Global | SymKind::Local | SymKind::Param => 6,
            SymKind::Import => 9,
        }
    }
}

/// One declared name.
#[derive(Debug, Clone, PartialEq)]
pub struct SymbolInfo {
    pub name: String,
    pub kind: SymKind,
    /// One-line signature shown in hover and completion detail.
    pub detail: String,
    /// Byte range of the name token itself.
    pub name_range: (usize, usize),
    /// Byte range of the whole declaration.
    pub range: (usize, usize),
    /// Where the name is visible. `None` means the whole file.
    pub scope: Option<(usize, usize)>,
    /// Index of the enclosing symbol (for the document symbol tree).
    pub parent: Option<usize>,
    /// `///` documentation found directly above the declaration.
    pub doc: Option<String>,
}

impl SymbolInfo {
    /// Fields, cases, methods, signals and gestures: reachable through their
    /// container, not as bare names.
    pub fn is_member(&self) -> bool {
        self.parent.is_some() && !matches!(self.kind, SymKind::Local | SymKind::Param)
    }

    /// Declared at file level and visible everywhere by its bare name.
    pub fn is_global(&self) -> bool {
        self.scope.is_none() && !self.is_member()
    }

    pub fn visible_at(&self, offset: usize) -> bool {
        match self.scope {
            None => true,
            Some((start, end)) => offset >= start && offset <= end,
        }
    }
}

/// Everything the server keeps per document after analysis.
#[derive(Debug, Default)]
pub struct Analysis {
    pub diagnostics: Vec<Diagnostic>,
    pub tokens: Vec<Token>,
    pub ast: Option<Ast>,
    pub symbols: Vec<SymbolInfo>,
}

impl Analysis {
    /// Globals (file-wide) and members, excluding locals and parameters.
    pub fn globals(&self) -> impl Iterator<Item = &SymbolInfo> {
        self.symbols.iter().filter(|s| s.is_global())
    }

    /// Resolve `name` as seen from `offset`: innermost visible local first,
    /// then file-wide declarations, then members (fields, cases, methods).
    pub fn resolve(&self, name: &str, offset: usize) -> Option<&SymbolInfo> {
        // The cursor is on the declaration itself.
        if let Some(decl) = self
            .symbols
            .iter()
            .find(|s| s.name == name && s.name_range.0 <= offset && offset <= s.name_range.1)
        {
            return Some(decl);
        }
        let mut best: Option<&SymbolInfo> = None;
        for sym in self.symbols.iter().filter(|s| s.name == name) {
            match sym.scope {
                Some((start, end)) if offset >= start && offset <= end => {
                    let better = match best.and_then(|b| b.scope) {
                        Some((bs, be)) => (end - start) < (be - bs),
                        None => true,
                    };
                    if better {
                        best = Some(sym);
                    }
                }
                _ => {}
            }
        }
        if best.is_some() {
            return best;
        }
        self.symbols
            .iter()
            .filter(|s| s.name == name && s.scope.is_none())
            .min_by_key(|s| if s.parent.is_none() { 0 } else { 1 })
    }

    /// Symbols visible at `offset` for completion (deduplicated by name,
    /// innermost wins).
    pub fn visible_at(&self, offset: usize) -> Vec<&SymbolInfo> {
        let mut out: Vec<&SymbolInfo> = Vec::new();
        for sym in self.symbols.iter().filter(|s| s.visible_at(offset) && !s.is_member()) {
            if let Some(existing) = out.iter_mut().find(|e| e.name == sym.name) {
                let narrower = match (existing.scope, sym.scope) {
                    (None, Some(_)) => true,
                    (Some((es, ee)), Some((ss, se))) => (se - ss) < (ee - es),
                    _ => false,
                };
                if narrower {
                    *existing = sym;
                }
            } else {
                out.push(sym);
            }
        }
        out
    }
}

/// Run lexer, parser and type checker over `source` and index its symbols.
/// Never panics: every failure becomes a diagnostic.
pub fn analyze(source: &str) -> Analysis {
    let mut analysis = Analysis::default();

    let tokens = match reoxc::lexer::tokenize(source) {
        Ok(tokens) => tokens,
        Err(e) => {
            analysis.diagnostics.push(Diagnostic::from(e));
            return analysis;
        }
    };
    analysis.tokens = tokens;

    let ast = match reoxc::parser::parse(&analysis.tokens) {
        Ok(ast) => ast,
        Err(e) => {
            analysis.diagnostics.push(Diagnostic::from(e));
            return analysis;
        }
    };

    if let Err(errors) = reoxc::typechecker::check(&ast) {
        analysis.diagnostics.extend(errors.into_iter().map(Diagnostic::from));
    }

    analysis.symbols = index_symbols(source, &analysis.tokens, &ast);
    analysis.ast = Some(ast);
    analysis
}

/// LSP `DiagnosticSeverity` number.
pub fn lsp_severity(severity: Severity) -> u32 {
    match severity {
        Severity::Error => 1,
        Severity::Warning => 2,
        Severity::Note => 3,
    }
}

// === Type and signature rendering ===

pub fn type_to_string(ty: &Type) -> String {
    match ty {
        Type::Int => "int".to_string(),
        Type::Float => "float".to_string(),
        Type::String => "string".to_string(),
        Type::Bool => "bool".to_string(),
        Type::Void => "void".to_string(),
        Type::I8 => "i8".to_string(),
        Type::U8 => "u8".to_string(),
        Type::I16 => "i16".to_string(),
        Type::U16 => "u16".to_string(),
        Type::I32 => "i32".to_string(),
        Type::U32 => "u32".to_string(),
        Type::I64 => "i64".to_string(),
        Type::U64 => "u64".to_string(),
        Type::USize => "usize".to_string(),
        Type::Named(name) => name.clone(),
        Type::Array(inner) => format!("[{}]", type_to_string(inner)),
        Type::Optional(inner) => format!("{}?", type_to_string(inner)),
        Type::Function(params, ret) => {
            let params: Vec<String> = params.iter().map(type_to_string).collect();
            format!("fn({}) -> {}", params.join(", "), type_to_string(ret))
        }
        Type::Tuple(items) => {
            let items: Vec<String> = items.iter().map(type_to_string).collect();
            format!("({})", items.join(", "))
        }
    }
}

fn params_to_string(params: &[Param]) -> String {
    let parts: Vec<String> = params
        .iter()
        .map(|p| format!("{}: {}", p.name, type_to_string(&p.ty)))
        .collect();
    parts.join(", ")
}

fn signature(prefix: &str, name: &str, params: &[Param], ret: Option<&Type>) -> String {
    let ret = ret.map(|t| format!(" -> {}", type_to_string(t))).unwrap_or_default();
    format!("{}{}({}){}", prefix, name, params_to_string(params), ret)
}

// === Symbol indexing ===

struct Indexer<'a> {
    source: &'a str,
    tokens: &'a [Token],
    symbols: Vec<SymbolInfo>,
    /// Return types of functions declared anywhere in the file, for inference.
    fn_returns: Vec<(String, String)>,
}

/// Build the symbol table for a parsed file.
pub fn index_symbols(source: &str, tokens: &[Token], ast: &Ast) -> Vec<SymbolInfo> {
    let mut indexer = Indexer { source, tokens, symbols: Vec::new(), fn_returns: Vec::new() };
    for decl in &ast.declarations {
        match decl {
            Decl::Function(f) => indexer.remember_return(&f.name, f.return_type.as_ref()),
            Decl::Extern(e) => indexer.remember_return(&e.name, e.return_type.as_ref()),
            _ => {}
        }
    }
    for decl in &ast.declarations {
        indexer.decl(decl);
    }
    indexer.symbols
}

impl<'a> Indexer<'a> {
    fn remember_return(&mut self, name: &str, ret: Option<&Type>) {
        let ret = ret.map(type_to_string).unwrap_or_else(|| "void".to_string());
        self.fn_returns.push((name.to_string(), ret));
    }

    /// Index of the first token at or after `offset`.
    fn token_at(&self, offset: usize) -> Option<usize> {
        self.tokens.iter().position(|t| t.span.start >= offset)
    }

    /// Byte range of the first identifier token named `name` at or after `offset`.
    fn name_range(&self, name: &str, offset: usize) -> (usize, usize) {
        let found = self
            .tokens
            .iter()
            .filter(|t| t.span.start >= offset)
            .find(|t| match &t.kind {
                TokenKind::Ident(id) => id == name,
                // Gesture handlers are keywords, not identifiers.
                other => keyword_text(other) == Some(name),
            });
        match found {
            Some(t) => (t.span.start, t.span.end),
            None => (offset, offset + name.len().min(self.source.len().saturating_sub(offset))),
        }
    }

    /// End offset of the `}` matching the `{` at or after `open_offset`.
    /// Falls back to the end of the file for unbalanced input.
    fn brace_end(&self, open_offset: usize) -> usize {
        let Some(start) = self.token_at(open_offset) else {
            return self.source.len();
        };
        let mut depth = 0usize;
        for t in &self.tokens[start..] {
            match t.kind {
                TokenKind::LBrace => depth += 1,
                TokenKind::RBrace => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return t.span.end;
                    }
                }
                _ => {}
            }
        }
        self.source.len()
    }

    /// End of a declaration that has no block: the next `;` or the next
    /// top-level token, whichever comes first.
    fn stmt_end(&self, start_offset: usize) -> usize {
        let Some(start) = self.token_at(start_offset) else {
            return self.source.len();
        };
        for t in &self.tokens[start..] {
            if t.kind == TokenKind::Semicolon {
                return t.span.end;
            }
        }
        self.source.len()
    }

    /// `///` lines directly above the declaration starting at `offset`.
    fn doc_above(&self, offset: usize) -> Option<String> {
        let before = self.source.get(..offset)?;
        // Skip the partial line the declaration starts on.
        let line_start = before.rfind('\n').map(|i| i + 1).unwrap_or(0);
        let head = &before[..line_start];
        let mut lines: Vec<&str> = Vec::new();
        for line in head.lines().rev() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("///") {
                lines.push(rest.strip_prefix(' ').unwrap_or(rest));
            } else if trimmed.is_empty() && lines.is_empty() {
                // Allow blank lines between the doc block and the decl? No:
                // a blank line ends the search, matching rustdoc.
                break;
            } else {
                break;
            }
        }
        if lines.is_empty() {
            return None;
        }
        lines.reverse();
        Some(lines.join("\n"))
    }

    #[allow(clippy::too_many_arguments)]
    fn push(
        &mut self,
        name: &str,
        kind: SymKind,
        detail: String,
        span: Span,
        range_end: usize,
        scope: Option<(usize, usize)>,
        parent: Option<usize>,
    ) -> usize {
        let name_range = self.name_range(name, span.start);
        let doc = self.doc_above(span.start);
        self.symbols.push(SymbolInfo {
            name: name.to_string(),
            kind,
            detail,
            name_range,
            range: (span.start, range_end.max(name_range.1)),
            scope,
            parent,
            doc,
        });
        self.symbols.len() - 1
    }

    fn decl(&mut self, decl: &Decl) {
        match decl {
            Decl::Function(f) => {
                self.function(f, SymKind::Function, None, "fn ");
            }
            Decl::Extern(e) => {
                let prefix = if e.is_async { "extern async fn " } else { "extern fn " };
                let detail = signature(prefix, &e.name, &e.params, e.return_type.as_ref());
                let end = self.stmt_end(e.span.start);
                self.push(&e.name, SymKind::Extern, detail, e.span, end, None, None);
            }
            Decl::Struct(s) => {
                let end = self.brace_end(s.span.start);
                let detail = format!("struct {}", s.name);
                let idx = self.push(&s.name, SymKind::Struct, detail, s.span, end, None, None);
                for field in &s.fields {
                    let detail = format!("{}.{}: {}", s.name, field.name, type_to_string(&field.ty));
                    let fend = field.span.start + field.name.len();
                    self.push(&field.name, SymKind::Field, detail, field.span, fend, None, Some(idx));
                }
            }
            Decl::Import(i) => {
                let path = i.path.join(".");
                let end = self.stmt_end(i.span.start);
                let last = i.path.last().cloned().unwrap_or_default();
                let detail = format!("import {}", path);
                self.push(&last, SymKind::Import, detail, i.span, end, None, None);
            }
            Decl::Variant(v) => {
                let end = self.brace_end(v.span.start);
                let detail = format!("variant {}", v.name);
                let idx = self.push(&v.name, SymKind::Variant, detail, v.span, end, None, None);
                for case in &v.cases {
                    let fields = if case.fields.is_empty() {
                        String::new()
                    } else {
                        let parts: Vec<String> = case
                            .fields
                            .iter()
                            .map(|f| format!("{}: {}", f.name, type_to_string(&f.ty)))
                            .collect();
                        format!("({})", parts.join(", "))
                    };
                    let detail = format!("{}.{}{}", v.name, case.name, fields);
                    let cend = case.span.start + case.name.len();
                    self.push(&case.name, SymKind::VariantCase, detail, case.span, cend, None, Some(idx));
                }
            }
            Decl::Protocol(p) => {
                let end = self.brace_end(p.span.start);
                let detail = format!("protocol {}", p.name);
                let idx = self.push(&p.name, SymKind::Protocol, detail, p.span, end, None, None);
                for m in &p.methods {
                    let detail = signature("fn ", &m.name, &m.params, m.return_type.as_ref());
                    let mend = self.stmt_end(m.span.start);
                    self.push(&m.name, SymKind::Method, detail, m.span, mend, None, Some(idx));
                }
            }
            Decl::Extension(e) => {
                let end = self.brace_end(e.span.start);
                let protocols = if e.protocols.is_empty() {
                    String::new()
                } else {
                    format!(": {}", e.protocols.join(", "))
                };
                let detail = format!("extension {}{}", e.target, protocols);
                let idx = self.push(&e.target, SymKind::Extension, detail, e.span, end, None, None);
                for m in &e.methods {
                    self.function(m, SymKind::Method, Some(idx), "fn ");
                }
            }
            Decl::Layer(l) => {
                let end = self.brace_end(l.span.start);
                let detail = format!("layer {}", l.name);
                let idx = self.push(&l.name, SymKind::Layer, detail, l.span, end, None, None);
                for field in &l.fields {
                    let detail = format!("{}.{}: {}", l.name, field.name, type_to_string(&field.ty));
                    let fend = self.stmt_end(field.span.start);
                    self.push(&field.name, SymKind::Field, detail, field.span, fend, None, Some(idx));
                }
                for sig in &l.signals {
                    let payload = sig
                        .payload_type
                        .as_ref()
                        .map(|t| format!("({})", type_to_string(t)))
                        .unwrap_or_default();
                    let detail = format!("signal {}{}", sig.name, payload);
                    let send = self.stmt_end(sig.span.start);
                    self.push(&sig.name, SymKind::Signal, detail, sig.span, send, None, Some(idx));
                }
                for m in &l.methods {
                    self.function(m, SymKind::Method, Some(idx), "fn ");
                }
                for g in &l.gestures {
                    let gend = self.brace_end(g.span.start);
                    let detail = signature("", &g.kind, &g.params, None);
                    let gidx = self.push(&g.kind, SymKind::Gesture, detail, g.span, gend, None, Some(idx));
                    let body_start = g.body.span.start;
                    for p in &g.params {
                        self.param(p, (body_start, gend), Some(gidx));
                    }
                    self.block(&g.body, Some(gidx));
                }
            }
            Decl::Panel(p) => {
                let end = self.brace_end(p.span.start);
                let detail = format!("panel {}", p.name);
                let idx = self.push(&p.name, SymKind::Panel, detail, p.span, end, None, None);
                for m in &p.methods {
                    self.function(m, SymKind::Method, Some(idx), "fn ");
                }
            }
            Decl::Const(c) => {
                let ty = c
                    .ty
                    .as_ref()
                    .map(type_to_string)
                    .unwrap_or_else(|| self.infer(&c.value, c.span.start));
                let detail = format!("const {}: {}", c.name, ty);
                let end = self.stmt_end(c.span.start);
                self.push(&c.name, SymKind::Const, detail, c.span, end, None, None);
            }
            Decl::Typealias(t) => {
                let detail = format!("typealias {} = {}", t.name, type_to_string(&t.target));
                let end = self.stmt_end(t.span.start);
                self.push(&t.name, SymKind::Typealias, detail, t.span, end, None, None);
            }
            Decl::GlobalVar(l) => {
                let detail = self.let_detail(l);
                let end = self.stmt_end(l.span.start);
                self.push(&l.name, SymKind::Global, detail, l.span, end, None, None);
            }
        }
    }

    fn function(&mut self, f: &FnDecl, kind: SymKind, parent: Option<usize>, prefix: &str) {
        let mut prefix = prefix.to_string();
        if f.is_static {
            prefix.insert_str(0, "static ");
        }
        if f.is_async {
            prefix.insert_str(0, "async ");
        }
        if f.is_pub {
            prefix.insert_str(0, "pub ");
        }
        let detail = signature(&prefix, &f.name, &f.params, f.return_type.as_ref());
        let end = self.brace_end(f.body.span.start);
        let idx = self.push(&f.name, kind, detail, f.span, end, None, parent);
        let body_start = f.body.span.start;
        for p in &f.params {
            self.param(p, (body_start, end), Some(idx));
        }
        self.block(&f.body, Some(idx));
    }

    fn param(&mut self, p: &Param, scope: (usize, usize), parent: Option<usize>) {
        let detail = format!("{}: {}", p.name, type_to_string(&p.ty));
        let end = p.span.start + p.name.len();
        let name_range = self.name_range(&p.name, p.span.start);
        self.symbols.push(SymbolInfo {
            name: p.name.clone(),
            kind: SymKind::Param,
            detail,
            name_range,
            range: (p.span.start, end.max(name_range.1)),
            scope: Some(scope),
            parent,
            doc: None,
        });
    }

    fn let_detail(&self, l: &reoxc::parser::LetStmt) -> String {
        let ty = match (&l.ty, &l.init) {
            (Some(ty), _) => type_to_string(ty),
            (None, Some(init)) => self.infer(init, l.span.start),
            (None, None) => "<unknown>".to_string(),
        };
        let keyword = if l.is_bind { "@Bind let" } else { "let" };
        let mutability = if l.mutable { " mut" } else { "" };
        format!("{}{} {}: {}", keyword, mutability, l.name, ty)
    }

    fn block(&mut self, block: &Block, parent: Option<usize>) {
        let end = self.brace_end(block.span.start);
        for stmt in &block.statements {
            self.stmt(stmt, end, parent);
        }
    }

    fn stmt(&mut self, stmt: &Stmt, block_end: usize, parent: Option<usize>) {
        match stmt {
            Stmt::Let(l) => {
                let detail = self.let_detail(l);
                if let Some(init) = &l.init {
                    self.expr(init, parent);
                }
                let name_range = self.name_range(&l.name, l.span.start);
                let stmt_end = self.stmt_end(l.span.start);
                self.symbols.push(SymbolInfo {
                    name: l.name.clone(),
                    kind: SymKind::Local,
                    detail,
                    name_range,
                    range: (l.span.start, stmt_end.max(name_range.1)),
                    scope: Some((stmt_end.max(name_range.1), block_end)),
                    parent,
                    doc: None,
                });
            }
            Stmt::Expr(e) => self.expr(e, parent),
            Stmt::Return(r) => {
                if let Some(v) = &r.value {
                    self.expr(v, parent);
                }
            }
            Stmt::If(i) => {
                self.expr(&i.condition, parent);
                self.block(&i.then_block, parent);
                if let Some(e) = &i.else_block {
                    self.block(e, parent);
                }
            }
            Stmt::While(w) => {
                self.expr(&w.condition, parent);
                self.block(&w.body, parent);
            }
            Stmt::For(f) => {
                self.expr(&f.iterable, parent);
                let end = self.brace_end(f.body.span.start);
                let elem = self.infer_element(&f.iterable, f.span.start);
                let name_range = self.name_range(&f.var, f.span.start);
                self.symbols.push(SymbolInfo {
                    name: f.var.clone(),
                    kind: SymKind::Local,
                    detail: format!("let {}: {}", f.var, elem),
                    name_range,
                    range: name_range,
                    scope: Some((f.span.start, end)),
                    parent,
                    doc: None,
                });
                self.block(&f.body, parent);
            }
            Stmt::Block(b) => self.block(b, parent),
            Stmt::Break(_) | Stmt::Continue(_) => {}
            Stmt::Guard(g) => {
                self.expr(&g.condition, parent);
                self.block(&g.else_block, parent);
            }
            Stmt::Defer(d) => self.block(&d.body, parent),
            Stmt::TryCatch(t) => {
                self.block(&t.try_block, parent);
                if let Some(var) = &t.catch_var {
                    let end = self.brace_end(t.catch_block.span.start);
                    let name_range = self.name_range(var, t.try_block.span.start);
                    self.symbols.push(SymbolInfo {
                        name: var.clone(),
                        kind: SymKind::Local,
                        detail: format!("let {}: string", var),
                        name_range,
                        range: name_range,
                        scope: Some((name_range.0, end)),
                        parent,
                        doc: None,
                    });
                }
                self.block(&t.catch_block, parent);
            }
            Stmt::Throw(t) => self.expr(&t.value, parent),
        }
    }

    fn expr(&mut self, expr: &Expr, parent: Option<usize>) {
        match expr {
            Expr::Literal(_) | Expr::Identifier(..) | Expr::Nil(_) => {}
            Expr::Binary(l, _, r, _)
            | Expr::Index(l, r, _)
            | Expr::Assign(l, r, _)
            | Expr::CompoundAssign(l, _, r, _)
            | Expr::NullCoalesce(l, r, _)
            | Expr::Range(l, r, _) => {
                self.expr(l, parent);
                self.expr(r, parent);
            }
            Expr::Unary(_, e, _)
            | Expr::Member(e, _, _)
            | Expr::PreIncrement(e, _)
            | Expr::PreDecrement(e, _)
            | Expr::PostIncrement(e, _)
            | Expr::PostDecrement(e, _)
            | Expr::OptionalChain(e, _, _)
            | Expr::Await(e, _) => self.expr(e, parent),
            Expr::Call(callee, args, _) => {
                self.expr(callee, parent);
                for a in args {
                    self.expr(a, parent);
                }
            }
            Expr::StructLit(_, fields, _) => {
                for (_, e) in fields {
                    self.expr(e, parent);
                }
            }
            Expr::ArrayLit(items, _) => {
                for e in items {
                    self.expr(e, parent);
                }
            }
            Expr::Match(scrutinee, arms, _) => {
                self.expr(scrutinee, parent);
                for arm in arms {
                    if let Pattern::Identifier(name) = &arm.pattern {
                        let end = arm.body.span().end.max(self.stmt_end(arm.span.start));
                        let name_range = self.name_range(name, arm.span.start);
                        self.symbols.push(SymbolInfo {
                            name: name.clone(),
                            kind: SymKind::Local,
                            detail: format!("let {}: <pattern>", name),
                            name_range,
                            range: name_range,
                            scope: Some((arm.span.start, end)),
                            parent,
                            doc: None,
                        });
                    }
                    self.expr(&arm.body, parent);
                }
            }
            Expr::TrailingClosure(callee, body, _) => {
                self.expr(callee, parent);
                self.block(body, parent);
            }
            Expr::Action(params, _, body, _) => {
                let end = self.brace_end(body.span.start);
                for p in params {
                    self.param(p, (body.span.start, end), parent);
                }
                self.block(body, parent);
            }
        }
    }

    // === Light type inference for untyped `let` ===

    fn infer(&self, expr: &Expr, at: usize) -> String {
        match expr {
            Expr::Literal(Literal::Int(..)) => "int".to_string(),
            Expr::Literal(Literal::Float(..)) => "float".to_string(),
            Expr::Literal(Literal::String(..)) => "string".to_string(),
            Expr::Literal(Literal::Bool(..)) => "bool".to_string(),
            Expr::Nil(_) => "nil".to_string(),
            Expr::StructLit(name, ..) => name.clone(),
            Expr::ArrayLit(items, _) => match items.first() {
                Some(first) => format!("[{}]", self.infer(first, at)),
                None => "[<unknown>]".to_string(),
            },
            Expr::Binary(l, op, r, _) => {
                use reoxc::parser::BinOp;
                match op {
                    BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge | BinOp::And | BinOp::Or => {
                        "bool".to_string()
                    }
                    _ => {
                        let left = self.infer(l, at);
                        if left == "<unknown>" {
                            self.infer(r, at)
                        } else {
                            left
                        }
                    }
                }
            }
            Expr::Unary(op, e, _) => {
                use reoxc::parser::UnaryOp;
                match op {
                    UnaryOp::Not => "bool".to_string(),
                    _ => self.infer(e, at),
                }
            }
            Expr::Call(callee, _, _) => match callee.as_ref() {
                Expr::Identifier(name, _) => self
                    .fn_returns
                    .iter()
                    .find(|(n, _)| n == name)
                    .map(|(_, r)| r.clone())
                    .unwrap_or_else(|| "<unknown>".to_string()),
                _ => "<unknown>".to_string(),
            },
            Expr::Identifier(name, span) => {
                // Only names declared earlier in an enclosing scope are known.
                let lookup = span.start.max(at);
                self.symbols
                    .iter()
                    .rfind(|s| s.name == *name && s.visible_at(lookup) && s.name_range.0 < lookup)
                    .and_then(|s| s.detail.rsplit_once(": ").map(|(_, t)| t.to_string()))
                    .unwrap_or_else(|| "<unknown>".to_string())
            }
            Expr::NullCoalesce(l, _, _) => self.infer(l, at),
            Expr::Match(..) | Expr::Member(..) | Expr::Index(..) | Expr::OptionalChain(..) => {
                "<unknown>".to_string()
            }
            Expr::Action(params, ret, _, _) => {
                let params: Vec<String> = params.iter().map(|p| type_to_string(&p.ty)).collect();
                let ret = ret.as_ref().map(|t| type_to_string(t)).unwrap_or_else(|| "void".to_string());
                format!("fn({}) -> {}", params.join(", "), ret)
            }
            _ => "<unknown>".to_string(),
        }
    }

    fn infer_element(&self, iterable: &Expr, at: usize) -> String {
        let ty = self.infer(iterable, at);
        if let Some(inner) = ty.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            return inner.to_string();
        }
        if ty == "int" {
            return "int".to_string();
        }
        "<unknown>".to_string()
    }
}

/// Source text of a keyword token kind, for the few keywords that can name a
/// declaration (gesture handlers).
fn keyword_text(kind: &TokenKind) -> Option<&'static str> {
    match kind {
        TokenKind::OnTap => Some("on_tap"),
        TokenKind::OnPan => Some("on_pan"),
        TokenKind::OnSwipe => Some("on_swipe"),
        TokenKind::OnPinch => Some("on_pinch"),
        TokenKind::OnRotate => Some("on_rotate"),
        TokenKind::Gesture => Some("gesture"),
        _ => None,
    }
}

/// Every keyword the lexer recognises, for completion.
pub const KEYWORDS: &[&str] = &[
    "fn", "let", "mut", "if", "else", "while", "for", "in", "return", "struct", "match",
    "import", "extern", "true", "false", "int", "float", "string", "bool", "void", "i8",
    "u8", "i16", "u16", "i32", "u32", "i64", "u64", "usize", "variant", "layer", "panel",
    "action", "maybe", "effect", "bind", "emit", "signal", "when", "self", "pub", "async",
    "await", "guard", "defer", "throw", "try", "catch", "where", "typealias", "protocol",
    "extension", "static", "const", "nil", "gesture", "on_tap", "on_pan", "on_swipe",
    "on_pinch", "on_rotate", "break", "continue",
];

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"/// Adds two numbers.
/// Second line.
fn add(a: int, b: int) -> int {
    let total = a + b;
    return total;
}

struct Point {
    x: int,
    y: float
}

variant Shape {
    Circle(radius: float),
    Square
}

protocol Drawable {
    fn draw() -> void;
}

layer Button {
    signal clicked;
    let label: string = "ok";
    on_tap {
        let hit = 1;
    }
    fn body() -> int {
        return 1;
    }
}

panel Main {
    fn root() {
        let p = Point { x: 1, y: 2.0 };
        let n = add(1, 2);
        for i in [1, 2] {
            let q = i;
        }
    }
}

const LIMIT: int = 10;
typealias Id = int;
extern fn sys_write(s: string) -> void;
"#;

    #[test]
    fn analysis_of_valid_file_has_no_diagnostics() {
        let a = analyze(SAMPLE);
        assert!(a.diagnostics.is_empty(), "{:?}", a.diagnostics);
        assert!(a.ast.is_some());
    }

    #[test]
    fn indexes_every_declaration_kind() {
        let a = analyze(SAMPLE);
        let find = |name: &str, kind: SymKind| {
            a.symbols
                .iter()
                .find(|s| s.name == name && s.kind == kind)
                .unwrap_or_else(|| panic!("missing {} {:?}", name, kind))
                .clone()
        };
        assert_eq!(find("add", SymKind::Function).detail, "fn add(a: int, b: int) -> int");
        assert_eq!(find("add", SymKind::Function).doc.as_deref(), Some("Adds two numbers.\nSecond line."));
        assert_eq!(find("Point", SymKind::Struct).detail, "struct Point");
        assert_eq!(find("y", SymKind::Field).detail, "Point.y: float");
        assert_eq!(find("Circle", SymKind::VariantCase).detail, "Shape.Circle(radius: float)");
        assert_eq!(find("Square", SymKind::VariantCase).detail, "Shape.Square");
        assert_eq!(find("draw", SymKind::Method).detail, "fn draw() -> void");
        assert_eq!(find("clicked", SymKind::Signal).detail, "signal clicked");
        assert_eq!(find("label", SymKind::Field).detail, "Button.label: string");
        assert_eq!(find("on_tap", SymKind::Gesture).detail, "on_tap()");
        assert_eq!(find("body", SymKind::Method).detail, "fn body() -> int");
        assert_eq!(find("Main", SymKind::Panel).detail, "panel Main");
        assert_eq!(find("LIMIT", SymKind::Const).detail, "const LIMIT: int");
        assert_eq!(find("Id", SymKind::Typealias).detail, "typealias Id = int");
        assert_eq!(find("sys_write", SymKind::Extern).detail, "extern fn sys_write(s: string) -> void");
    }

    #[test]
    fn name_ranges_point_at_the_name_token() {
        let a = analyze(SAMPLE);
        for sym in &a.symbols {
            let (s, e) = sym.name_range;
            assert_eq!(&SAMPLE[s..e], sym.name, "{:?}", sym);
            assert!(sym.range.0 <= s && sym.range.1 >= e, "{:?}", sym);
        }
    }

    #[test]
    fn locals_are_scoped_and_typed() {
        let a = analyze(SAMPLE);
        let total = a.symbols.iter().find(|s| s.name == "total").expect("total");
        assert_eq!(total.kind, SymKind::Local);
        assert_eq!(total.detail, "let total: int");
        let (ss, se) = total.scope.expect("scoped");
        assert!(SAMPLE[..ss].ends_with("let total = a + b;"), "scope starts after the statement");
        assert!(SAMPLE[ss..se].ends_with('}'));

        let p = a.symbols.iter().find(|s| s.name == "p").expect("p");
        assert_eq!(p.detail, "let p: Point");
        let n = a.symbols.iter().find(|s| s.name == "n").expect("n");
        assert_eq!(n.detail, "let n: int");
        let q = a.symbols.iter().find(|s| s.name == "q").expect("q");
        assert_eq!(q.detail, "let q: int");
        let i = a.symbols.iter().find(|s| s.name == "i").expect("i");
        assert_eq!(i.detail, "let i: int");
        let hit = a.symbols.iter().find(|s| s.name == "hit").expect("hit");
        assert_eq!(hit.detail, "let hit: int");
    }

    #[test]
    fn resolve_prefers_innermost_visible_local() {
        let src = "fn f(x: int) -> int {\n    let x = 2;\n    return x;\n}\nfn x() {}\n";
        let a = analyze(src);
        let use_offset = src.find("return x").map(|i| i + 7).expect("use");
        let sym = a.resolve("x", use_offset).expect("resolved");
        assert_eq!(sym.kind, SymKind::Local);
        // Before the shadowing let, the parameter wins.
        let param_offset = src.find("let x").expect("let");
        let sym = a.resolve("x", param_offset).expect("resolved");
        assert_eq!(sym.kind, SymKind::Param);
        // Outside the function only the global is visible.
        let sym = a.resolve("x", src.len() - 1).expect("resolved");
        assert_eq!(sym.kind, SymKind::Function);
        assert!(a.resolve("nope", 0).is_none());
    }

    #[test]
    fn visible_at_lists_globals_and_locals_in_scope() {
        let a = analyze(SAMPLE);
        let inside = SAMPLE.find("return total").expect("inside add");
        let names: Vec<&str> = a.visible_at(inside).iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"total"));
        assert!(names.contains(&"a"));
        assert!(names.contains(&"Point"));
        assert!(!names.contains(&"p"));
        assert!(!names.contains(&"x"), "fields are members, not visible names");
    }

    #[test]
    fn lex_parse_and_type_errors_become_diagnostics() {
        let a = analyze("fn main() { let s = \"open; }");
        assert_eq!(a.diagnostics.len(), 1);
        assert!(a.tokens.is_empty());

        let a = analyze("fn main() {");
        assert_eq!(a.diagnostics.len(), 1);
        assert!(a.ast.is_none());
        assert!(!a.tokens.is_empty());

        let a = analyze("fn main() { let a: int = \"x\"; }");
        assert_eq!(a.diagnostics.len(), 1);
        assert!(a.ast.is_some(), "type errors keep the AST for navigation");
        assert!(a.symbols.iter().any(|s| s.name == "main"));
    }

    #[test]
    fn malformed_input_never_panics() {
        for src in ["", "{", "}}}}", "fn", "\u{0}", "let x = ;", "\"\\(", "/* /* */", "0b_"] {
            let _ = analyze(src);
        }
    }

    #[test]
    fn type_rendering() {
        let t = Type::Function(vec![Type::Int, Type::Named("P".into())], Box::new(Type::Optional(Box::new(Type::Array(Box::new(Type::String))))));
        assert_eq!(type_to_string(&t), "fn(int, P) -> [string]?");
        assert_eq!(type_to_string(&Type::Tuple(vec![Type::I8, Type::USize])), "(i8, usize)");
    }
}
