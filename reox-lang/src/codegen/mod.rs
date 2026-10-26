// REOX Compiler - Code Generator
// Generates C code from typed AST
// Zero external dependencies
//
// Ownership model (ARC, see reox-spec.md "Memory model"):
//
// Every expression of reference type (string, array, map, `ref struct`,
// layer, `Error`) produces either a borrowed (+0) or an owned (+1) value.
// Calls, string concatenation, struct allocation and weak loads produce
// owned values; identifiers, field reads and indexing produce borrowed
// ones. A binding, field store or return takes an owned value (retaining a
// borrowed one first); an argument or operand takes a borrowed value (an
// owned one is parked in a per-statement temporary that is released when
// the statement ends). Every local that holds a reference is released when
// its scope exits, on every path: fall-through, `return`, `break`,
// `continue`, `throw` and a failed `try`.

use crate::parser::{
    Ast, Decl, Stmt, Expr, Literal, BinOp, UnaryOp, CompoundOp,
    FnDecl, StructDecl, ExternDecl, Block, Type, LetStmt,
    ReturnStmt, IfStmt, WhileStmt, ForStmt, GuardStmt, DeferStmt,
    TryCatchStmt, ThrowStmt,
    VariantDecl, ProtocolDecl, ExtensionDecl, LayerDecl, PanelDecl,
    ConstDecl, TypealiasDecl, Pattern,
};
use crate::lexer::Span;
use std::collections::HashMap;
use std::io::Write;

/// Code generation error, located by a source span when one is known
#[derive(Debug, Clone, PartialEq)]
pub struct CodegenError {
    pub message: String,
    pub span: Span,
}

impl CodegenError {
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self { message: message.into(), span }
    }

    pub fn display(&self) -> String {
        format!("codegen error[{}:{}]: {}", self.span.line, self.span.column, self.message)
    }
}

impl std::fmt::Display for CodegenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.display())
    }
}

impl std::error::Error for CodegenError {}

/// Deepest expression nesting the C backend will emit before reporting an
/// error instead of recursing further.
pub const MAX_EXPR_DEPTH: usize = 1024;

/// Type id of a compiler generated reference type: FNV-1a over the name with
/// the top bit set, the same function as `rx_type_id_hash` in the runtime.
pub fn type_id_hash(name: &str) -> u32 {
    let mut h: u32 = 2166136261;
    for b in name.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(16777619);
    }
    h | 0x8000_0000
}

/// Types as the C backend sees them.
#[derive(Debug, Clone, PartialEq)]
pub enum CType {
    Int,
    Float,
    Bool,
    Void,
    /// Fixed width integers; the string is the C spelling (`int32_t`).
    Fixed(&'static str),
    Str,
    Array(Box<CType>),
    /// String keyed map; the parameter is the value type.
    Map(Box<CType>),
    /// Value struct or variant (C struct by value).
    Value(String),
    /// `ref struct` or layer instance (heap object, pointer).
    Ref(String),
    /// The `Error` existential: any thrown object.
    Error,
    Optional(Box<CType>),
    Function(Vec<CType>, Box<CType>),
    Tuple(usize),
    Unknown,
}

impl CType {
    /// Reference counted heap object (pointer)?
    fn is_ref(&self) -> bool {
        match self {
            CType::Str | CType::Array(_) | CType::Map(_) | CType::Ref(_) | CType::Error => true,
            CType::Optional(inner) => inner.is_ref(),
            _ => false,
        }
    }

    fn unwrap_optional(&self) -> &CType {
        match self {
            CType::Optional(inner) => inner,
            other => other,
        }
    }

    /// Spelling used in error messages and in Result type names.
    fn display(&self) -> String {
        match self {
            CType::Int => "int".into(),
            CType::Float => "float".into(),
            CType::Bool => "bool".into(),
            CType::Void => "void".into(),
            CType::Fixed(c) => c.trim_end_matches("_t").to_string(),
            CType::Str => "string".into(),
            CType::Array(inner) => format!("[{}]", inner.display()),
            CType::Map(inner) => format!("[string: {}]", inner.display()),
            CType::Value(n) | CType::Ref(n) => n.clone(),
            CType::Error => "Error".into(),
            CType::Optional(inner) => format!("{}?", inner.display()),
            CType::Function(_, _) => "fn".into(),
            CType::Tuple(n) => format!("tuple{}", n),
            CType::Unknown => "<unknown>".into(),
        }
    }

    /// Identifier-safe spelling, used to name `Result` structs.
    fn mangle(&self) -> String {
        match self {
            CType::Array(inner) => format!("array_{}", inner.mangle()),
            CType::Map(inner) => format!("map_string_{}", inner.mangle()),
            CType::Optional(inner) => format!("opt_{}", inner.mangle()),
            CType::Function(_, _) => "fn".into(),
            CType::Unknown => "unknown".into(),
            other => other.display(),
        }
    }
}

/// A local binding that must be released when its scope exits.
#[derive(Debug, Clone)]
struct Local {
    name: String,
    ty: CType,
    weak: bool,
}

#[derive(Debug, Clone, PartialEq)]
enum ScopeKind {
    Function,
    Block,
    Loop,
    /// `try { }` block; the id names the catch label.
    Try(usize),
}

#[derive(Debug, Clone)]
struct Scope {
    kind: ScopeKind,
    locals: Vec<Local>,
    /// Length of `plain_locals` when the scope opened.
    plain_mark: usize,
}

#[derive(Debug, Clone)]
struct StructC {
    is_ref: bool,
    /// (name, type, is_weak) in declaration order
    fields: Vec<(String, CType, bool)>,
}

#[derive(Debug, Clone)]
struct FnSig {
    params: Vec<CType>,
    ret: CType,
    throws: bool,
    is_extern: bool,
}

/// Code generator state
pub struct CodeGen {
    output: String,
    indent: usize,
    defer_stack: Vec<Block>,
    closure_counter: usize,
    closure_defs: String,
    expr_depth: usize,

    // Declarations
    structs: HashMap<String, StructC>,
    fns: HashMap<String, FnSig>,
    globals: HashMap<String, CType>,
    aliases: HashMap<String, CType>,
    /// Result struct typedefs needed by throwing functions.
    result_types: Vec<(String, CType)>,
    string_literals: Vec<String>,

    // Per function
    scopes: Vec<Scope>,
    /// Every binding in scope with its type (parameters included), used
    /// for type inference; `scopes` only tracks what must be released.
    plain_locals: Vec<(String, CType)>,
    fn_ret: CType,
    fn_throws: bool,
    is_main: bool,
    temp_counter: usize,
    temp_decls: Vec<(String, CType)>,
    stmt_temps: Vec<(String, CType)>,
    hoisted: HashMap<String, CType>,
    label_counter: usize,
}

impl CodeGen {
    pub fn new() -> Self {
        Self {
            output: String::new(),
            indent: 0,
            defer_stack: Vec::new(),
            closure_counter: 0,
            closure_defs: String::new(),
            expr_depth: 0,
            structs: HashMap::new(),
            fns: HashMap::new(),
            globals: HashMap::new(),
            aliases: HashMap::new(),
            result_types: Vec::new(),
            string_literals: Vec::new(),
            scopes: Vec::new(),
            plain_locals: Vec::new(),
            fn_ret: CType::Void,
            fn_throws: false,
            is_main: false,
            temp_counter: 0,
            temp_decls: Vec::new(),
            stmt_temps: Vec::new(),
            hoisted: HashMap::new(),
            label_counter: 0,
        }
    }

    fn emit(&mut self, s: &str) {
        self.output.push_str(s);
    }

    fn emit_line(&mut self, s: &str) {
        self.emit_indent();
        self.output.push_str(s);
        self.output.push('\n');
    }

    fn emit_indent(&mut self) {
        for _ in 0..self.indent {
            self.output.push_str("    ");
        }
    }

    fn indent(&mut self) {
        self.indent += 1;
    }

    fn dedent(&mut self) {
        if self.indent > 0 {
            self.indent -= 1;
        }
    }

    /// Run `f` with a fresh output buffer and return what it emitted.
    fn capture<T>(&mut self, f: impl FnOnce(&mut Self) -> Result<T, CodegenError>) -> Result<(String, T), CodegenError> {
        let saved = std::mem::take(&mut self.output);
        let result = f(self);
        let text = std::mem::replace(&mut self.output, saved);
        result.map(|v| (text, v))
    }

    // === Type model ===

    fn ctype(&self, ty: &Type) -> CType {
        match ty {
            Type::Int => CType::Int,
            Type::Float => CType::Float,
            Type::String => CType::Str,
            Type::Bool => CType::Bool,
            Type::Void => CType::Void,
            Type::I8 => CType::Fixed("int8_t"),
            Type::U8 => CType::Fixed("uint8_t"),
            Type::I16 => CType::Fixed("int16_t"),
            Type::U16 => CType::Fixed("uint16_t"),
            Type::I32 => CType::Fixed("int32_t"),
            Type::U32 => CType::Fixed("uint32_t"),
            Type::I64 => CType::Fixed("int64_t"),
            Type::U64 => CType::Fixed("uint64_t"),
            Type::USize => CType::Fixed("size_t"),
            Type::Named(name) => self.named_ctype(name),
            Type::Array(inner) => CType::Array(Box::new(self.ctype(inner))),
            Type::Map(_, value) => CType::Map(Box::new(self.ctype(value))),
            Type::Optional(inner) => CType::Optional(Box::new(self.ctype(inner))),
            Type::Function(params, ret) => CType::Function(
                params.iter().map(|p| self.ctype(p)).collect(),
                Box::new(self.ctype(ret)),
            ),
            Type::Tuple(elems) => CType::Tuple(elems.len()),
        }
    }

    fn named_ctype(&self, name: &str) -> CType {
        if name == "Error" {
            return CType::Error;
        }
        if let Some(alias) = self.aliases.get(name) {
            return alias.clone();
        }
        match self.structs.get(name) {
            Some(s) if s.is_ref => CType::Ref(name.to_string()),
            _ => CType::Value(name.to_string()),
        }
    }

    fn type_to_c(&self, ty: &Type) -> String {
        self.c_name(&self.ctype(ty))
    }

    fn c_name(&self, ty: &CType) -> String {
        match ty {
            CType::Int => "int64_t".into(),
            CType::Float => "double".into(),
            CType::Bool => "bool".into(),
            CType::Void => "void".into(),
            CType::Fixed(c) => (*c).into(),
            CType::Str => "rx_str*".into(),
            CType::Array(_) => "rx_array*".into(),
            CType::Map(_) => "rx_map*".into(),
            CType::Value(n) => n.clone(),
            CType::Ref(n) => format!("{}*", n),
            CType::Error => "rx_object*".into(),
            CType::Optional(inner) => {
                if inner.is_ref() {
                    self.c_name(inner)
                } else {
                    format!("{}*", self.c_name(inner))
                }
            }
            CType::Function(params, ret) => {
                let ret_c = self.c_name(ret);
                let params_c: Vec<String> = params.iter().map(|p| self.c_name(p)).collect();
                format!("{}(*)({})", ret_c, if params_c.is_empty() { "void".to_string() } else { params_c.join(", ") })
            }
            CType::Tuple(n) => format!("rx_tuple{}", n),
            CType::Unknown => "__auto_type".into(),
        }
    }

    /// Value struct whose fields (transitively) hold references.
    fn value_has_refs(&self, name: &str) -> bool {
        self.value_has_refs_bounded(name, 0)
    }

    fn value_has_refs_bounded(&self, name: &str, depth: usize) -> bool {
        if depth > 32 {
            return false;
        }
        match self.structs.get(name) {
            Some(s) if !s.is_ref => s.fields.iter().any(|(_, ty, _)| self.needs_release_bounded(ty, depth + 1)),
            _ => false,
        }
    }

    /// Whether a value of this type owns references that must be released.
    fn needs_release(&self, ty: &CType) -> bool {
        self.needs_release_bounded(ty, 0)
    }

    fn needs_release_bounded(&self, ty: &CType, depth: usize) -> bool {
        match ty {
            _ if ty.is_ref() => true,
            CType::Value(n) => self.value_has_refs_bounded(n, depth),
            _ => false,
        }
    }

    /// C statement releasing the value held in lvalue `x`, if any.
    fn release_stmt(&self, ty: &CType, x: &str) -> Option<String> {
        if ty.is_ref() {
            Some(format!("rx_release({});", x))
        } else if let CType::Value(n) = ty {
            if self.value_has_refs(n) {
                Some(format!("{}__release_fields(&{});", n, x))
            } else {
                None
            }
        } else {
            None
        }
    }

    /// C expression that turns a borrowed `text` of type `ty` into an owned value.
    fn retain_expr(&self, ty: &CType, text: &str) -> String {
        if ty.is_ref() {
            format!("(({})rx_retain({}))", self.c_name(ty), text)
        } else if let CType::Value(n) = ty {
            if self.value_has_refs(n) {
                format!("{}__copy({})", n, text)
            } else {
                text.to_string()
            }
        } else {
            text.to_string()
        }
    }

    /// Element ops for arrays and maps holding values of `elem`.
    fn elem_ops(&self, elem: &CType) -> String {
        if elem.is_ref() {
            "&rx_ref_elem_ops".into()
        } else if let CType::Value(n) = elem {
            if self.value_has_refs(n) {
                format!("&{}__elem_ops", n)
            } else {
                "NULL".into()
            }
        } else {
            "NULL".into()
        }
    }

    fn struct_field(&self, struct_name: &str, field: &str) -> Option<(CType, bool)> {
        self.structs.get(struct_name).and_then(|s| {
            s.fields.iter().find(|(n, _, _)| n == field).map(|(_, ty, weak)| (ty.clone(), *weak))
        })
    }

    fn result_type_name(&mut self, ty: &CType) -> String {
        let name = format!("Result__{}__Error", ty.mangle());
        if !self.result_types.iter().any(|(n, _)| *n == name) {
            self.result_types.push((name.clone(), ty.clone()));
        }
        name
    }

    // === Scopes and temporaries ===

    fn push_scope(&mut self, kind: ScopeKind) {
        let plain_mark = self.plain_locals.len();
        self.scopes.push(Scope { kind, locals: Vec::new(), plain_mark });
    }

    fn pop_scope(&mut self) {
        if let Some(scope) = self.scopes.pop() {
            self.plain_locals.truncate(scope.plain_mark);
        }
    }

    /// Record a binding: its type for inference and, when it owns
    /// references, its release at scope exit.
    fn declare_local(&mut self, name: &str, ty: &CType, weak: bool) {
        self.plain_locals.push((name.to_string(), ty.clone()));
        if weak || self.needs_release(ty) {
            if let Some(scope) = self.scopes.last_mut() {
                scope.locals.push(Local { name: name.to_string(), ty: ty.clone(), weak });
            }
        }
    }

    fn lookup_local(&self, name: &str) -> Option<(CType, bool)> {
        for scope in self.scopes.iter().rev() {
            if let Some(local) = scope.locals.iter().rev().find(|l| l.name == name) {
                return Some((local.ty.clone(), local.weak));
            }
        }
        None
    }

    fn lookup_plain(&self, name: &str) -> Option<CType> {
        self.plain_locals.iter().rev().find(|(n, _)| n == name).map(|(_, t)| t.clone())
    }

    fn emit_release_local(&mut self, local: &Local) {
        if local.weak {
            self.emit_line(&format!("rx_weak_drop({});", local.name));
        } else if let Some(stmt) = self.release_stmt(&local.ty, &local.name) {
            self.emit_line(&stmt);
        }
    }

    /// Release the locals of the innermost scope (natural block exit).
    fn emit_scope_releases(&mut self) {
        let locals: Vec<Local> = self.scopes.last().map(|s| s.locals.iter().rev().cloned().collect()).unwrap_or_default();
        for local in &locals {
            self.emit_release_local(local);
        }
    }

    /// Release locals from the innermost scope outwards, stopping after the
    /// first scope for which `stop` holds (or after all scopes).
    fn emit_unwind(&mut self, stop: impl Fn(&ScopeKind) -> bool) {
        let mut locals: Vec<Local> = Vec::new();
        for scope in self.scopes.iter().rev() {
            locals.extend(scope.locals.iter().rev().cloned());
            if stop(&scope.kind) {
                break;
            }
        }
        for local in &locals {
            self.emit_release_local(local);
        }
    }

    fn innermost_try(&self) -> Option<usize> {
        self.scopes.iter().rev().find_map(|s| match s.kind {
            ScopeKind::Try(id) => Some(id),
            _ => None,
        })
    }

    fn new_temp(&mut self, ty: &CType, release: bool) -> String {
        let name = format!("__rx_t{}", self.temp_counter);
        self.temp_counter += 1;
        self.temp_decls.push((name.clone(), ty.clone()));
        if release {
            self.stmt_temps.push((name.clone(), ty.clone()));
        }
        name
    }

    /// Release the temporaries of the current statement.
    fn emit_stmt_temp_releases(&mut self) {
        let temps = std::mem::take(&mut self.stmt_temps);
        for (name, ty) in temps.iter().rev() {
            if let Some(stmt) = self.release_stmt(ty, name) {
                self.emit_line(&stmt);
                self.emit_line(&format!("{} = {};", name, self.zero_value(ty)));
            }
        }
    }

    fn zero_value(&self, ty: &CType) -> String {
        if ty.is_ref() {
            "NULL".into()
        } else if let CType::Value(n) = ty {
            format!("({}){{0}}", n)
        } else if matches!(ty, CType::Optional(_)) {
            "NULL".into()
        } else {
            "0".into()
        }
    }

    /// Propagate an error object held in C expression `err`: jump to the
    /// innermost catch or return it from a throwing function.
    fn emit_error_exit(&mut self, err: &str) -> Result<(), CodegenError> {
        if let Some(id) = self.innermost_try() {
            self.emit_unwind(|k| *k == ScopeKind::Try(id));
            self.emit_line(&format!("__rx_err{} = {};", id, err));
            self.emit_line(&format!("goto __rx_catch{};", id));
        } else if self.fn_throws {
            let ret_name = self.result_type_name(&self.fn_ret.clone());
            self.emit_unwind(|_| false);
            if !self.defer_stack.is_empty() {
                self.emit_deferred_cleanup()?;
            }
            self.emit_line(&format!("return ({}){{ .err = {} }};", ret_name, err));
        } else {
            self.emit_line(&format!("rx_release({});", err));
            self.emit_line("rx_panic(\"unhandled error\");");
        }
        Ok(())
    }

    /// Generate C code from AST
    pub fn generate(&mut self, ast: &Ast) -> Result<String, CodegenError> {
        self.collect_declarations(ast);

        // Emit header
        self.emit_line("// Generated by REOX Compiler");
        self.emit_line("// Do not edit manually");
        self.emit_line("");
        self.emit_line("#include <stdint.h>");
        self.emit_line("#include <stdbool.h>");
        self.emit_line("#include <stddef.h>");
        self.emit_line("#include \"reox_runtime.h\"");
        self.emit_line("#include \"reox_nxrender_bridge.h\"");  // NXRender integration
        self.emit_line("");

        // Forward declarations for structs and variants
        for decl in &ast.declarations {
            match decl {
                Decl::Struct(s) => self.emit_line(&format!("typedef struct {} {};", s.name, s.name)),
                Decl::Variant(v) => self.emit_line(&format!("typedef struct {0} {0};", v.name)),
                Decl::Layer(l) => self.emit_line(&format!("typedef struct {0} {0};", l.name)),
                _ => {}
            }
        }
        self.emit_line("");

        // Generate typealias
        for decl in &ast.declarations {
            if let Decl::Typealias(t) = decl {
                self.gen_typealias(t);
            }
        }

        // Generate variant (tagged union)
        for decl in &ast.declarations {
            if let Decl::Variant(v) = decl {
                self.gen_variant(v);
                self.emit_line("");
            }
        }

        // Generate struct definitions
        for decl in &ast.declarations {
            if let Decl::Struct(s) = decl {
                self.gen_struct(s);
                self.emit_line("");
            }
        }

        // Generate protocol (vtable)
        for decl in &ast.declarations {
            if let Decl::Protocol(p) = decl {
                self.gen_protocol(p);
                self.emit_line("");
            }
        }

        // Generate layer structs
        for decl in &ast.declarations {
            if let Decl::Layer(l) = decl {
                self.gen_layer_struct(l);
                self.emit_line("");
            }
        }

        // Generate extern declarations
        for decl in &ast.declarations {
            if let Decl::Extern(e) = decl {
                self.gen_extern(e);
            }
        }
        if ast.declarations.iter().any(|d| matches!(d, Decl::Extern(_))) {
            self.emit_line("");
        }

        // Result structs for throwing functions (value types must be complete here)
        let mut sigs: Vec<CType> = self.fns.values().filter(|f| f.throws).map(|f| f.ret.clone()).collect();
        sigs.sort_by_key(|t| t.mangle());
        for ret in sigs {
            self.result_type_name(&ret);
        }
        self.gen_result_types();

        // Ownership helpers for structs (dealloc, make, copy, release_fields)
        for decl in &ast.declarations {
            match decl {
                Decl::Struct(s) => self.gen_struct_helpers(&s.name, s.is_ref)?,
                Decl::Layer(l) => self.gen_struct_helpers(&l.name, true)?,
                _ => {}
            }
        }

        // Generate function prototypes (including extension methods)
        for decl in &ast.declarations {
            match decl {
                Decl::Function(f) => self.gen_fn_prototype_named(&f.name, f),
                Decl::Extension(ext) => {
                    for method in &ext.methods {
                        let mangled_name = format!("{}_{}", ext.target, method.name);
                        self.gen_fn_prototype_named(&mangled_name, method);
                    }
                }
                Decl::Layer(l) => {
                    for method in &l.methods {
                        let mangled_name = format!("{}_{}", l.name, method.name);
                        self.gen_fn_prototype_named(&mangled_name, method);
                    }
                }
                Decl::Panel(p) => {
                    for method in &p.methods {
                        let mangled_name = format!("{}_{}", p.name, method.name);
                        self.gen_fn_prototype_named(&mangled_name, method);
                    }
                }
                _ => {}
            }
        }
        self.emit_line("");

        // Generate const and global variables (after the literals they may use)
        let (globals_text, ()) = self.capture(|g| {
            for decl in &ast.declarations {
                match decl {
                    Decl::Const(c) => g.gen_const(c)?,
                    Decl::GlobalVar(v) => g.gen_global_var(v)?,
                    _ => {}
                }
            }
            Ok(())
        })?;

        // Generate function implementations into a buffer so that string
        // literals and closures discovered on the way can be emitted first.
        let (functions_text, ()) = self.capture(|g| {
            for decl in &ast.declarations {
                match decl {
                    Decl::Function(f) => {
                        g.gen_function_named(&f.name, f)?;
                        g.emit_line("");
                    }
                    Decl::Extension(ext) => {
                        g.gen_extension(ext)?;
                    }
                    Decl::Layer(l) => {
                        g.gen_layer_methods(l)?;
                    }
                    Decl::Panel(p) => {
                        g.gen_panel(p)?;
                        g.emit_line("");
                    }
                    _ => {}
                }
            }
            Ok(())
        })?;

        // String literal objects
        let literals = std::mem::take(&mut self.string_literals);
        for (i, lit) in literals.iter().enumerate() {
            self.emit_line(&format!("static rx_str __rx_lit{} = RX_STR_LIT_INIT(\"{}\");", i, lit));
        }
        if !literals.is_empty() {
            self.emit_line("");
        }
        self.string_literals = literals;

        self.emit(&globals_text);
        if !globals_text.is_empty() {
            self.emit_line("");
        }

        // Emit closure static functions (generated during expression codegen)
        if !self.closure_defs.is_empty() {
            self.emit(&self.closure_defs.clone());
            self.emit_line("");
        }

        self.emit(&functions_text);

        Ok(self.output.clone())
    }

    /// First pass: struct layouts, function signatures, aliases and globals.
    fn collect_declarations(&mut self, ast: &Ast) {
        // Names first so that fields can refer to types declared later
        for decl in &ast.declarations {
            match decl {
                Decl::Struct(s) => {
                    self.structs.insert(s.name.clone(), StructC { is_ref: s.is_ref, fields: Vec::new() });
                }
                Decl::Layer(l) => {
                    self.structs.insert(l.name.clone(), StructC { is_ref: true, fields: Vec::new() });
                }
                Decl::Variant(v) => {
                    self.structs.insert(v.name.clone(), StructC { is_ref: false, fields: Vec::new() });
                }
                _ => {}
            }
        }
        for decl in &ast.declarations {
            if let Decl::Typealias(t) = decl {
                let target = self.ctype(&t.target);
                self.aliases.insert(t.name.clone(), target);
            }
        }
        for decl in &ast.declarations {
            match decl {
                Decl::Struct(s) => {
                    let fields = s.fields.iter().map(|f| (f.name.clone(), self.ctype(&f.ty), f.is_weak)).collect();
                    self.structs.insert(s.name.clone(), StructC { is_ref: s.is_ref, fields });
                }
                Decl::Layer(l) => {
                    let fields = l.fields.iter().map(|f| (f.name.clone(), self.ctype(&f.ty), f.is_weak)).collect();
                    self.structs.insert(l.name.clone(), StructC { is_ref: true, fields });
                }
                Decl::Function(f) => {
                    let sig = self.fn_sig(f, false);
                    self.fns.insert(f.name.clone(), sig);
                }
                Decl::Extern(e) => {
                    let sig = FnSig {
                        params: e.params.iter().map(|p| self.ctype(&p.ty)).collect(),
                        ret: e.return_type.as_ref().map(|t| self.ctype(t)).unwrap_or(CType::Void),
                        throws: false,
                        is_extern: true,
                    };
                    self.fns.insert(e.name.clone(), sig);
                }
                Decl::Extension(ext) => {
                    for m in &ext.methods {
                        let sig = self.fn_sig(m, false);
                        self.fns.insert(format!("{}_{}", ext.target, m.name), sig);
                    }
                }
                _ => {}
            }
        }
        for decl in &ast.declarations {
            match decl {
                Decl::Layer(l) => {
                    for m in &l.methods {
                        let sig = self.fn_sig(m, false);
                        self.fns.insert(format!("{}_{}", l.name, m.name), sig);
                    }
                }
                Decl::Panel(p) => {
                    for m in &p.methods {
                        let sig = self.fn_sig(m, false);
                        self.fns.insert(format!("{}_{}", p.name, m.name), sig);
                    }
                }
                Decl::Const(c) => {
                    let ty = c.ty.as_ref().map(|t| self.ctype(t)).unwrap_or_else(|| self.infer_type(&c.value));
                    self.globals.insert(c.name.clone(), ty);
                }
                Decl::GlobalVar(g) => {
                    let ty = g.ty.as_ref().map(|t| self.ctype(t))
                        .or_else(|| g.init.as_ref().map(|e| self.infer_type(e)))
                        .unwrap_or(CType::Int);
                    self.globals.insert(g.name.clone(), ty);
                }
                _ => {}
            }
        }
    }

    fn fn_sig(&self, f: &FnDecl, is_extern: bool) -> FnSig {
        FnSig {
            params: f.params.iter().map(|p| self.ctype(&p.ty)).collect(),
            ret: f.return_type.as_ref().map(|t| self.ctype(t)).unwrap_or(CType::Void),
            throws: f.throws,
            is_extern,
        }
    }

    fn gen_struct(&mut self, s: &StructDecl) {
        self.emit_line(&format!("struct {} {{", s.name));
        self.indent();
        if s.is_ref {
            self.emit_line("rx_object header;");
        }
        for field in &s.fields {
            if field.is_weak {
                self.emit_line(&format!("rx_weak {};", field.name));
            } else {
                let c_type = self.type_to_c(&field.ty);
                self.emit_line(&format!("{} {};", c_type, field.name));
            }
        }
        self.dedent();
        self.emit_line("};");
    }

    fn gen_result_types(&mut self) {
        let types = self.result_types.clone();
        for (name, ty) in &types {
            self.emit_line(&format!("typedef struct {} {{", name));
            self.indent();
            self.emit_line("rx_object* err;");
            if *ty != CType::Void {
                let c = self.c_name(ty);
                self.emit_line(&format!("{} value;", c));
            }
            self.dedent();
            self.emit_line(&format!("}} {};", name));
        }
        if !types.is_empty() {
            self.emit_line("");
        }
    }

    /// Ownership helpers for a struct: ref structs get a type id, a dealloc
    /// hook and an allocating constructor; value structs that hold references
    /// get retain/release helpers for their fields and element ops.
    fn gen_struct_helpers(&mut self, name: &str, is_ref: bool) -> Result<(), CodegenError> {
        let info = match self.structs.get(name) {
            Some(info) => info.clone(),
            None => return Ok(()),
        };
        if is_ref {
            self.emit_line(&format!("#define {}__TYPE_ID 0x{:08x}u", name, type_id_hash(name)));
            self.emit_line(&format!("static void {}__dealloc(void* __p) {{", name));
            self.indent();
            self.emit_line(&format!("{0}* self = ({0}*)__p;", name));
            for (field, ty, weak) in &info.fields {
                if *weak {
                    self.emit_line(&format!("rx_weak_drop(self->{});", field));
                } else if let Some(stmt) = self.release_stmt(ty, &format!("self->{}", field)) {
                    self.emit_line(&stmt);
                }
            }
            self.emit_line("rx_free(self);");
            self.dedent();
            self.emit_line("}");

            // Constructor: strong fields are passed owned (+1), weak fields borrowed.
            let params: Vec<String> = info.fields.iter().map(|(field, ty, weak)| {
                let c = if *weak { self.c_name(ty.unwrap_optional()) } else { self.c_name(ty) };
                format!("{} {}", c, field)
            }).collect();
            let params_str = if params.is_empty() { "void".to_string() } else { params.join(", ") };
            self.emit_line(&format!("static {0}* {0}__make({1}) {{", name, params_str));
            self.indent();
            self.emit_line(&format!("{0}* o = ({0}*)rx_calloc(1, sizeof({0}));", name));
            self.emit_line(&format!("if (!o) rx_panic(\"out of memory allocating {}\");", name));
            self.emit_line(&format!("rx_object_init(&o->header, {0}__TYPE_ID, {0}__dealloc);", name));
            for (field, _, weak) in &info.fields {
                if *weak {
                    self.emit_line(&format!("o->{0} = rx_weak_ref({0});", field));
                } else {
                    self.emit_line(&format!("o->{0} = {0};", field));
                }
            }
            self.emit_line("return o;");
            self.dedent();
            self.emit_line("}");
            self.emit_line("");
        } else if self.value_has_refs(name) {
            self.emit_line(&format!("static void {0}__release_fields({0}* v) {{", name));
            self.indent();
            for (field, ty, _) in &info.fields {
                if let Some(stmt) = self.release_stmt(ty, &format!("v->{}", field)) {
                    self.emit_line(&stmt);
                }
            }
            self.dedent();
            self.emit_line("}");
            self.emit_line(&format!("static void {0}__retain_fields({0}* v) {{", name));
            self.indent();
            for (field, ty, _) in &info.fields {
                if ty.is_ref() {
                    self.emit_line(&format!("rx_retain(v->{});", field));
                } else if let CType::Value(inner) = ty {
                    if self.value_has_refs(inner) {
                        self.emit_line(&format!("{}__retain_fields(&v->{});", inner, field));
                    }
                }
            }
            self.dedent();
            self.emit_line("}");
            self.emit_line(&format!("static {0} {0}__copy({0} v) {{ {0}__retain_fields(&v); return v; }}", name));
            self.emit_line(&format!("static void {0}__retain_slot(void* s) {{ {0}__retain_fields(({0}*)s); }}", name));
            self.emit_line(&format!("static void {0}__release_slot(void* s) {{ {0}__release_fields(({0}*)s); }}", name));
            self.emit_line(&format!("static const rx_elem_ops {0}__elem_ops = {{ {0}__retain_slot, {0}__release_slot }};", name));
            self.emit_line("");
        }
        Ok(())
    }

    fn gen_extern(&mut self, e: &ExternDecl) {
        // Strings cross the extern boundary as C strings.
        let ret_type = match e.return_type.as_ref().map(|t| self.ctype(t)) {
            None | Some(CType::Void) => "void".to_string(),
            Some(CType::Str) => "const char*".to_string(),
            Some(other) => self.c_name(&other),
        };

        let params: Vec<String> = e.params.iter()
            .map(|p| {
                let ty = self.ctype(&p.ty);
                let c = if ty == CType::Str { "const char*".to_string() } else { self.c_name(&ty) };
                format!("{} {}", c, p.name)
            })
            .collect();

        let params_str = if params.is_empty() {
            "void".to_string()
        } else {
            params.join(", ")
        };

        self.emit_line(&format!("extern {} {}({});", ret_type, e.name, params_str));
    }

    fn fn_signature(&mut self, name: &str, f: &FnDecl) -> String {
        let ret = f.return_type.as_ref().map(|t| self.ctype(t)).unwrap_or(CType::Void);
        let ret_type = if name == "main" {
            "int".to_string()
        } else if f.throws {
            self.result_type_name(&ret)
        } else {
            self.c_name(&ret)
        };

        let params: Vec<String> = f.params.iter()
            .map(|p| format!("{} {}", self.type_to_c(&p.ty), p.name))
            .collect();

        let params_str = if params.is_empty() {
            "void".to_string()
        } else {
            params.join(", ")
        };

        format!("{} {}({})", ret_type, name, params_str)
    }

    fn gen_fn_prototype_named(&mut self, name: &str, f: &FnDecl) {
        let sig = self.fn_signature(name, f);
        self.emit_line(&format!("{};", sig));
    }

    fn gen_variant(&mut self, v: &VariantDecl) {
        // Emit C enum for tag
        self.emit_line("typedef enum {");
        self.indent();
        for (i, case) in v.cases.iter().enumerate() {
            let suffix = if i < v.cases.len() - 1 { "," } else { "" };
            self.emit_line(&format!("{}_{}{}", v.name.to_uppercase(), case.name.to_uppercase(), suffix));
        }
        self.dedent();
        self.emit_line(&format!("}} {}_Tag;", v.name));
        self.emit_line("");

        // Emit tagged union struct
        self.emit_line(&format!("struct {} {{", v.name));
        self.indent();
        self.emit_line(&format!("{}_Tag tag;", v.name));

        // If any case has fields, emit a union
        let has_payload = v.cases.iter().any(|c| !c.fields.is_empty());
        if has_payload {
            self.emit_line("union {");
            self.indent();
            for case in &v.cases {
                if !case.fields.is_empty() {
                    self.emit_line("struct {");
                    self.indent();
                    for field in &case.fields {
                        let c_type = self.type_to_c(&field.ty);
                        self.emit_line(&format!("{} {};", c_type, field.name));
                    }
                    self.dedent();
                    self.emit_line(&format!("}} {};", case.name.to_lowercase()));
                }
            }
            self.dedent();
            self.emit_line("} data;");
        }

        self.dedent();
        self.emit_line("};");
    }

    fn gen_protocol(&mut self, p: &ProtocolDecl) {
        // Protocol → vtable struct of function pointers
        self.emit_line(&format!("typedef struct {}_vtable {{", p.name));
        self.indent();
        for method in &p.methods {
            let ret_type = method.return_type.as_ref()
                .map(|t| self.type_to_c(t))
                .unwrap_or_else(|| "void".to_string());

            let mut params: Vec<String> = vec!["void* self".to_string()];
            for param in &method.params {
                params.push(format!("{} {}", self.type_to_c(&param.ty), param.name));
            }

            self.emit_line(&format!("{} (*{})({});", ret_type, method.name, params.join(", ")));
        }
        self.dedent();
        self.emit_line(&format!("}} {}_vtable;", p.name));
    }

    fn gen_extension(&mut self, ext: &ExtensionDecl) -> Result<(), CodegenError> {
        for method in &ext.methods {
            let mangled_name = format!("{}_{}", ext.target, method.name);
            self.gen_function_named(&mangled_name, method)?;
            self.emit_line("");
        }
        Ok(())
    }

    fn gen_layer_struct(&mut self, l: &LayerDecl) {
        self.emit_line(&format!("struct {} {{", l.name));
        self.indent();
        self.emit_line("rx_object header;");
        for field in &l.fields {
            if field.is_weak {
                self.emit_line(&format!("rx_weak {};", field.name));
            } else {
                let c_type = self.type_to_c(&field.ty);
                self.emit_line(&format!("{} {};", c_type, field.name));
            }
        }
        // Signal callback slots
        for signal in &l.signals {
            self.emit_line(&format!("void (*{})(void*);", signal.name));
        }
        self.dedent();
        self.emit_line("};");
    }

    fn gen_layer_methods(&mut self, l: &LayerDecl) -> Result<(), CodegenError> {
        for method in &l.methods {
            let mangled_name = format!("{}_{}", l.name, method.name);
            self.gen_function_named(&mangled_name, method)?;
            self.emit_line("");
        }

        // Generate gesture handler callbacks
        for gesture in &l.gestures {
            let fn_name = format!("{}_{}", l.name, gesture.kind);
            let param_strs: Vec<String> = gesture.params.iter()
                .map(|p| format!("{} {}", self.type_to_c(&p.ty), p.name))
                .collect();
            let params_c = if param_strs.is_empty() {
                "void* ctx".to_string()
            } else {
                format!("void* ctx, {}", param_strs.join(", "))
            };

            self.emit_line(&format!("static void {}({}) {{", fn_name, params_c));
            self.indent();
            self.begin_function(CType::Void, false, false);
            for p in &gesture.params {
                let ty = self.ctype(&p.ty);
                self.plain_locals.push((p.name.clone(), ty));
            }
            let (body, ()) = self.capture(|g| {
                g.gen_block(&gesture.body)?;
                if !Self::block_terminates(&gesture.body) {
                    g.emit_scope_releases();
                }
                Ok(())
            })?;
            self.emit_temp_decls();
            self.emit(&body);
            self.end_function();
            self.dedent();
            self.emit_line("}");
            self.emit_line("");
        }
        Ok(())
    }

    fn gen_panel(&mut self, p: &PanelDecl) -> Result<(), CodegenError> {
        // Generate panel methods
        for method in &p.methods {
            let mangled_name = format!("{}_{}", p.name, method.name);
            self.gen_function_named(&mangled_name, method)?;
            self.emit_line("");
        }

        // Generate panel setup function
        self.emit_line(&format!("void {}_setup(void) {{", p.name));
        self.indent();
        self.emit_line("rx_app_t* app = rx_app_new();");

        // Emit property assignments
        for (key, value) in &p.properties {
            match key.as_str() {
                "title" => {
                    self.emit_indent();
                    self.emit("rx_app_set_title(app, ");
                    self.gen_expr(value)?;
                    self.emit(");\n");
                }
                "size" => {
                    self.emit_indent();
                    self.emit("rx_app_set_size(app, ");
                    self.gen_expr(value)?;
                    self.emit(");\n");
                }
                _ => {
                    self.emit_line(&format!("/* panel property: {} */", key));
                }
            }
        }

        self.emit_line("rx_app_run(app);");
        self.dedent();
        self.emit_line("}");
        Ok(())
    }

    fn gen_const(&mut self, c: &ConstDecl) -> Result<(), CodegenError> {
        let c_type = c.ty.as_ref()
            .map(|t| self.type_to_c(t))
            .unwrap_or_else(|| "const int64_t".to_string());

        self.emit_indent();
        self.emit(&format!("static const {} {} = ", c_type, c.name));
        self.gen_expr(&c.value)?;
        self.emit(";\n");
        Ok(())
    }

    fn gen_global_var(&mut self, g: &LetStmt) -> Result<(), CodegenError> {
        let c_type = g.ty.as_ref()
            .map(|t| self.type_to_c(t))
            .unwrap_or_else(|| "int64_t".to_string());

        // A type that already carries `const` must not get a second one.
        let qualifier = if g.mutable || c_type.starts_with("const ") {
            "static"
        } else {
            "static const"
        };
        self.emit_indent();
        self.emit(&format!("{} {} {}", qualifier, c_type, g.name));

        if let Some(init) = &g.init {
            self.emit(" = ");
            self.gen_expr(init)?;
        }
        self.emit(";\n");
        Ok(())
    }

    fn gen_typealias(&mut self, t: &TypealiasDecl) {
        let target_c = self.type_to_c(&t.target);
        self.emit_line(&format!("typedef {} {};", target_c, t.name));
    }

    // === Functions ===

    fn begin_function(&mut self, ret: CType, throws: bool, is_main: bool) {
        self.defer_stack.clear();
        self.scopes.clear();
        self.plain_locals.clear();
        self.push_scope(ScopeKind::Function);
        self.fn_ret = ret;
        self.fn_throws = throws;
        self.is_main = is_main;
        self.temp_counter = 0;
        self.temp_decls.clear();
        self.stmt_temps.clear();
        self.hoisted.clear();
    }

    fn end_function(&mut self) {
        self.scopes.clear();
        self.plain_locals.clear();
        self.defer_stack.clear();
    }

    /// Declarations of every temporary the function body used.
    fn emit_temp_decls(&mut self) {
        let decls = std::mem::take(&mut self.temp_decls);
        for (name, ty) in &decls {
            let c = self.c_name(ty);
            let zero = self.zero_value(ty);
            self.emit_line(&format!("{} {} = {};", c, name, zero));
        }
    }

    fn gen_function_named(&mut self, name: &str, f: &FnDecl) -> Result<(), CodegenError> {
        let ret = f.return_type.as_ref().map(|t| self.ctype(t)).unwrap_or(CType::Void);
        let sig = self.fn_signature(name, f);
        self.emit_line(&format!("{} {{", sig));
        self.indent();

        self.begin_function(ret.clone(), f.throws, name == "main");
        for p in &f.params {
            // Parameters are borrowed from the caller: typed, never released here.
            let ty = self.ctype(&p.ty);
            self.plain_locals.push((p.name.clone(), ty));
        }

        let (body, ()) = self.capture(|g| {
            g.gen_block(&f.body)?;
            if Self::block_terminates(&f.body) {
                return Ok(());
            }
            // Natural end of the function
            if !g.defer_stack.is_empty() {
                g.emit_deferred_cleanup()?;
            }
            g.emit_scope_releases();
            if g.is_main {
                g.emit_line("return 0;");
            } else if f.throws && ret == CType::Void {
                let ret_name = g.result_type_name(&ret);
                g.emit_line(&format!("return ({}){{ .err = NULL }};", ret_name));
            }
            Ok(())
        })?;
        self.emit_temp_decls();
        self.emit(&body);
        self.end_function();

        self.dedent();
        self.emit_line("}");
        Ok(())
    }

    fn gen_block(&mut self, block: &Block) -> Result<(), CodegenError> {
        for stmt in &block.statements {
            self.gen_statement(stmt)?;
        }
        Ok(())
    }

    /// Whether control never falls off the end of the block.
    fn block_terminates(block: &Block) -> bool {
        matches!(
            block.statements.last(),
            Some(Stmt::Return(_)) | Some(Stmt::Throw(_)) | Some(Stmt::Break(_)) | Some(Stmt::Continue(_))
        )
    }

    /// A nested block with its own scope and releases at the end.
    fn gen_scoped_block(&mut self, block: &Block, kind: ScopeKind) -> Result<(), CodegenError> {
        self.push_scope(kind);
        self.gen_block(block)?;
        if !Self::block_terminates(block) {
            self.emit_scope_releases();
        }
        self.pop_scope();
        Ok(())
    }

    fn gen_statement(&mut self, stmt: &Stmt) -> Result<(), CodegenError> {
        let saved_temps = std::mem::take(&mut self.stmt_temps);
        let saved_hoisted = std::mem::take(&mut self.hoisted);
        let result = self.gen_statement_inner(stmt);
        self.hoisted = saved_hoisted;
        self.stmt_temps = saved_temps;
        result
    }

    fn gen_statement_inner(&mut self, stmt: &Stmt) -> Result<(), CodegenError> {
        match stmt {
            Stmt::Let(l) => self.gen_let(l)?,
            Stmt::Return(r) => self.gen_return(r)?,
            Stmt::If(i) => self.gen_if(i)?,
            Stmt::While(w) => self.gen_while(w)?,
            Stmt::For(f) => self.gen_for(f)?,
            Stmt::Expr(e) => {
                let e = self.hoist(e)?;
                let (text, owned) = self.capture(|g| g.gen_expr(&e))?;
                let ty = self.infer_type(&e);
                self.emit_indent();
                if owned && ty.is_ref() {
                    self.emit(&format!("rx_release({});\n", text));
                } else if owned && self.needs_release(&ty) {
                    let tmp = self.new_temp(&ty, true);
                    self.emit(&format!("{} = {};\n", tmp, text));
                } else {
                    self.emit(&text);
                    self.emit(";\n");
                }
                self.emit_stmt_temp_releases();
            }
            Stmt::Block(b) => {
                self.emit_line("{");
                self.indent();
                self.gen_scoped_block(b, ScopeKind::Block)?;
                self.dedent();
                self.emit_line("}");
            }
            Stmt::Break(_) => {
                self.emit_unwind(|k| *k == ScopeKind::Loop);
                self.emit_line("break;");
            }
            Stmt::Continue(_) => {
                self.emit_unwind(|k| *k == ScopeKind::Loop);
                self.emit_line("continue;");
            }
            // Swift/C++ style statements
            Stmt::Guard(g) => self.gen_guard(g)?,
            Stmt::Defer(d) => self.gen_defer(d),
            Stmt::TryCatch(t) => self.gen_try_catch(t)?,
            Stmt::Throw(t) => self.gen_throw(t)?,
        }
        Ok(())
    }

    fn gen_guard(&mut self, g: &GuardStmt) -> Result<(), CodegenError> {
        // guard condition else { ... }
        // Translates to: if (!(condition)) { ... }
        let cond = self.gen_condition(&g.condition)?;
        self.emit_line(&format!("if (!({})) {{", cond));
        self.indent();
        self.gen_scoped_block(&g.else_block, ScopeKind::Block)?;
        self.dedent();
        self.emit_line("}");
        Ok(())
    }

    fn gen_defer(&mut self, d: &DeferStmt) {
        // Push deferred block onto stack - will be emitted before returns
        self.defer_stack.push(d.body.clone());
    }

    fn emit_deferred_cleanup(&mut self) -> Result<(), CodegenError> {
        // Emit all deferred blocks in reverse order (LIFO)
        let blocks: Vec<Block> = self.defer_stack.iter().rev().cloned().collect();
        for block in blocks {
            self.emit_line("/* defer cleanup */");
            self.emit_line("{");
            self.indent();
            self.gen_scoped_block(&block, ScopeKind::Block)?;
            self.dedent();
            self.emit_line("}");
        }
        Ok(())
    }

    fn gen_try_catch(&mut self, t: &TryCatchStmt) -> Result<(), CodegenError> {
        let id = self.label_counter;
        self.label_counter += 1;
        self.emit_line("{");
        self.indent();
        self.emit_line(&format!("rx_object* __rx_err{} = NULL;", id));
        self.emit_line("{");
        self.indent();
        self.gen_scoped_block(&t.try_block, ScopeKind::Try(id))?;
        self.dedent();
        self.emit_line("}");
        self.emit_line(&format!("goto __rx_end{};", id));
        self.emit_line(&format!("__rx_catch{}:;", id));
        self.emit_line("{");
        self.indent();
        self.push_scope(ScopeKind::Block);
        if let Some(ty) = &t.catch_type {
            let cty = self.ctype(ty);
            let name = match &cty {
                CType::Ref(n) => n.clone(),
                _ => return Err(CodegenError::new("catch pattern type must be a 'ref struct'", t.span)),
            };
            // Not our type: pass the error on to the next handler
            self.emit_line(&format!("if (rx_type_id(__rx_err{}) != {}__TYPE_ID) {{", id, name));
            self.indent();
            let err = format!("__rx_err{}", id);
            self.emit_error_exit(&err)?;
            self.dedent();
            self.emit_line("}");
            if let Some(var) = &t.catch_var {
                self.emit_line(&format!("{0}* {1} = ({0}*)__rx_err{2};", name, var, id));
                self.declare_local(var, &cty, false);
            } else {
                self.emit_line(&format!("rx_release(__rx_err{});", id));
            }
        } else if let Some(var) = &t.catch_var {
            self.emit_line(&format!("rx_object* {} = __rx_err{};", var, id));
            self.declare_local(var, &CType::Error, false);
        } else {
            self.emit_line(&format!("rx_release(__rx_err{});", id));
        }
        self.gen_block(&t.catch_block)?;
        if !Self::block_terminates(&t.catch_block) {
            self.emit_scope_releases();
        }
        self.pop_scope();
        self.dedent();
        self.emit_line("}");
        self.emit_line(&format!("__rx_end{}:;", id));
        self.dedent();
        self.emit_line("}");
        Ok(())
    }

    fn gen_throw(&mut self, t: &ThrowStmt) -> Result<(), CodegenError> {
        let value = self.hoist(&t.value)?;
        let ty = self.infer_type(&value);
        if !ty.is_ref() {
            return Err(CodegenError::new(
                format!("thrown value must be a 'ref struct' conforming to Error, found '{}'", ty.display()),
                t.span,
            ));
        }
        let (text, owned) = self.capture(|g| g.gen_expr(&value))?;
        let owned_text = if owned { text } else { self.retain_expr(&ty, &text) };
        let tmp = self.new_temp(&CType::Error, false);
        self.emit_line(&format!("{} = (rx_object*){};", tmp, owned_text));
        self.emit_stmt_temp_releases();
        self.emit_error_exit(&tmp)
    }

    fn gen_let(&mut self, l: &LetStmt) -> Result<(), CodegenError> {
        let declared = l.ty.as_ref().map(|t| self.ctype(t));
        let init = match &l.init {
            Some(init) => Some(self.hoist(init)?),
            None => None,
        };
        let inferred = init.as_ref().map(|e| self.infer_type(e));
        let ty = declared.clone().or(inferred.clone()).unwrap_or(CType::Unknown);

        if l.is_weak {
            // Weak binding: a handle into the weak table, dropped at scope exit.
            self.emit_indent();
            self.emit(&format!("rx_weak {} = rx_weak_ref(", l.name));
            match &init {
                Some(init) => { self.gen_expr_borrowed(init, Some(&ty))?; }
                None => self.emit("NULL"),
            }
            self.emit(");\n");
            self.emit_stmt_temp_releases();
            let inner = ty.unwrap_optional().clone();
            self.declare_local(&l.name, &inner, true);
            return Ok(());
        }

        let c_type = self.c_name(&ty);
        self.emit_indent();
        self.emit(&format!("{} {}", c_type, l.name));

        if let Some(init) = &init {
            self.emit(" = ");
            self.gen_expr_owned(init, &ty)?;
        } else if self.needs_release(&ty) || ty.is_ref() || matches!(ty, CType::Optional(_)) {
            let zero = self.zero_value(&ty);
            self.emit(&format!(" = {}", zero));
        }

        self.emit(";\n");
        self.emit_stmt_temp_releases();
        self.declare_local(&l.name, &ty, false);
        Ok(())
    }

    fn gen_return(&mut self, r: &ReturnStmt) -> Result<(), CodegenError> {
        let ret_ty = self.fn_ret.clone();
        let value = match &r.value {
            Some(v) => Some(self.hoist(v)?),
            None => None,
        };

        // Evaluate the value first (owned), then unwind, then return it.
        let mut ret_expr: Option<String> = None;
        if let Some(value) = &value {
            let (text, ()) = self.capture(|g| { g.gen_expr_owned(value, &ret_ty)?; Ok(()) })?;
            let has_cleanup = !self.stmt_temps.is_empty()
                || self.scopes.iter().any(|s| !s.locals.is_empty())
                || !self.defer_stack.is_empty();
            if has_cleanup {
                let tmp = self.new_temp(&ret_ty, false);
                self.emit_line(&format!("{} = {};", tmp, text));
                ret_expr = Some(tmp);
            } else {
                ret_expr = Some(text);
            }
        }
        self.emit_stmt_temp_releases();

        if !self.defer_stack.is_empty() {
            self.emit_deferred_cleanup()?;
        }
        self.emit_unwind(|_| false);

        self.emit_indent();
        match ret_expr {
            Some(text) if self.is_main => self.emit(&format!("return (int)({});\n", text)),
            Some(text) if self.fn_throws => {
                let ret_name = self.result_type_name(&ret_ty);
                self.emit(&format!("return ({}){{ .err = NULL, .value = {} }};\n", ret_name, text));
            }
            Some(text) => self.emit(&format!("return {};\n", text)),
            None if self.is_main => self.emit("return 0;\n"),
            None if self.fn_throws => {
                let ret_name = self.result_type_name(&ret_ty);
                self.emit(&format!("return ({}){{ .err = NULL }};\n", ret_name));
            }
            None => self.emit("return;\n"),
        }
        Ok(())
    }

    /// Emit everything a condition needs before it is read (hoisted calls,
    /// temporaries) and return the C text of the condition itself. When the
    /// condition created temporaries it is pre-evaluated into a bool so that
    /// they can be released before the controlled statement runs.
    fn gen_condition(&mut self, cond: &Expr) -> Result<String, CodegenError> {
        let cond = self.hoist(cond)?;
        let (text, _) = self.capture(|g| g.gen_expr(&cond))?;
        if self.stmt_temps.is_empty() {
            return Ok(text);
        }
        let tmp = self.new_temp(&CType::Bool, false);
        self.emit_line(&format!("{} = {};", tmp, text));
        self.emit_stmt_temp_releases();
        Ok(tmp)
    }

    fn gen_if(&mut self, i: &IfStmt) -> Result<(), CodegenError> {
        if let Some(name) = &i.binding {
            // if let x = expr { } else { }
            let cond = self.hoist(&i.condition)?;
            let ty = self.infer_type(&cond);
            let inner = ty.unwrap_optional().clone();
            self.emit_line("{");
            self.indent();
            self.push_scope(ScopeKind::Block);
            let c = self.c_name(&inner);
            self.emit_indent();
            self.emit(&format!("{} {} = ", c, name));
            self.gen_expr_owned(&cond, &inner)?;
            self.emit(";\n");
            self.emit_stmt_temp_releases();
            self.declare_local(name, &inner, false);
            self.emit_line(&format!("if ({} != NULL) {{", name));
            self.indent();
            self.gen_scoped_block(&i.then_block, ScopeKind::Block)?;
            self.dedent();
            if let Some(else_block) = &i.else_block {
                self.emit_line("} else {");
                self.indent();
                self.gen_scoped_block(else_block, ScopeKind::Block)?;
                self.dedent();
            }
            self.emit_line("}");
            self.emit_scope_releases();
            self.pop_scope();
            self.dedent();
            self.emit_line("}");
            return Ok(());
        }

        let cond = self.gen_condition(&i.condition)?;
        self.emit_line(&format!("if ({}) {{", cond));
        self.indent();
        self.gen_scoped_block(&i.then_block, ScopeKind::Block)?;
        self.dedent();

        if let Some(else_block) = &i.else_block {
            // Detect else-if: single If statement in the else block
            if else_block.statements.len() == 1 {
                if let Stmt::If(nested_if) = &else_block.statements[0] {
                    self.emit_line("} else {");
                    self.indent();
                    self.gen_statement(&Stmt::If(nested_if.clone()))?;
                    self.dedent();
                    self.emit_line("}");
                    return Ok(());
                }
            }
            self.emit_line("} else {");
            self.indent();
            self.gen_scoped_block(else_block, ScopeKind::Block)?;
            self.dedent();
        }

        self.emit_line("}");
        Ok(())
    }

    fn gen_while(&mut self, w: &WhileStmt) -> Result<(), CodegenError> {
        // The condition may need hoisted calls and temporaries on every
        // iteration, so it is evaluated inside the loop body.
        let (prelude, cond) = self.capture(|g| g.gen_condition(&w.condition))?;
        if prelude.is_empty() {
            self.emit_line(&format!("while ({}) {{", cond));
        } else {
            self.emit_line("for (;;) {");
            self.indent();
            self.emit(&prelude);
            self.emit_line(&format!("if (!({})) break;", cond));
            self.dedent();
        }
        self.indent();
        self.gen_scoped_block(&w.body, ScopeKind::Loop)?;
        self.dedent();
        self.emit_line("}");
        Ok(())
    }

    fn gen_for(&mut self, f: &ForStmt) -> Result<(), CodegenError> {
        match &f.iterable {
            Expr::Range(start, end, _) => {
                // Optimized C loop: for (int64_t i = start; i <= end; ++i)
                self.emit_indent();
                self.emit(&format!("for (int64_t {} = ", f.var));
                self.gen_expr(start)?;
                self.emit(&format!("; {} <= ", f.var));
                self.gen_expr(end)?;
                self.emit(&format!("; ++{}) {{\n", f.var));
                self.indent();
                self.push_scope(ScopeKind::Loop);
                self.plain_locals.push((f.var.clone(), CType::Int));
                self.gen_block(&f.body)?;
                if !Self::block_terminates(&f.body) {
                    self.emit_scope_releases();
                }
                self.pop_scope();
                self.dedent();
                self.emit_line("}");
            }
            _ => {
                let iterable = self.hoist(&f.iterable)?;
                let iter_ty = self.infer_type(&iterable);
                let elem_ty = match &iter_ty {
                    CType::Array(elem) => (**elem).clone(),
                    other => {
                        return Err(CodegenError::new(
                            format!("cannot iterate over '{}'", other.display()),
                            f.span,
                        ))
                    }
                };
                let iter_name = format!("__rx_it{}", self.label_counter);
                let index_name = format!("__rx_i{}", self.label_counter);
                self.label_counter += 1;

                self.emit_line("{");
                self.indent();
                self.push_scope(ScopeKind::Block);
                self.emit_indent();
                self.emit(&format!("rx_array* {} = ", iter_name));
                self.gen_expr_owned(&iterable, &iter_ty)?;
                self.emit(";\n");
                self.emit_stmt_temp_releases();
                self.declare_local(&iter_name, &iter_ty, false);

                self.emit_line(&format!(
                    "for (size_t {0} = 0; {0} < array_len({1}); ++{0}) {{",
                    index_name, iter_name
                ));
                self.indent();
                self.push_scope(ScopeKind::Loop);
                let elem_c = self.c_name(&elem_ty);
                let borrowed = format!("(*({}*)array_get_unchecked({}, {}))", elem_c, iter_name, index_name);
                let owned = self.retain_expr(&elem_ty, &borrowed);
                self.emit_line(&format!("{} {} = {};", elem_c, f.var, owned));
                self.declare_local(&f.var, &elem_ty, false);
                self.gen_block(&f.body)?;
                if !Self::block_terminates(&f.body) {
                    self.emit_scope_releases();
                }
                self.pop_scope();
                self.dedent();
                self.emit_line("}");

                self.emit_scope_releases();
                self.pop_scope();
                self.dedent();
                self.emit_line("}");
            }
        }
        Ok(())
    }

    // === Hoisting ===

    /// Rewrite `expr` so that `try` calls and array literals are evaluated
    /// in statements emitted before the current one; each is replaced by an
    /// identifier that reads the hoisted temporary (borrowed).
    fn hoist(&mut self, expr: &Expr) -> Result<Expr, CodegenError> {
        if !Self::contains_hoistable(expr) {
            return Ok(expr.clone());
        }
        self.hoist_inner(expr)
    }

    fn contains_hoistable(expr: &Expr) -> bool {
        match expr {
            Expr::Try(_, _) | Expr::ArrayLit(_, _) => true,
            Expr::Literal(_) | Expr::Identifier(_, _) | Expr::Nil(_) => false,
            Expr::Binary(l, _, r, _) | Expr::Assign(l, r, _) | Expr::CompoundAssign(l, _, r, _)
            | Expr::NullCoalesce(l, r, _) | Expr::Index(l, r, _) | Expr::Range(l, r, _) => {
                Self::contains_hoistable(l) || Self::contains_hoistable(r)
            }
            Expr::Unary(_, e, _) | Expr::Member(e, _, _) | Expr::OptionalChain(e, _, _)
            | Expr::PreIncrement(e, _) | Expr::PreDecrement(e, _) | Expr::PostIncrement(e, _)
            | Expr::PostDecrement(e, _) | Expr::Await(e, _) => Self::contains_hoistable(e),
            Expr::Call(callee, args, _) => Self::contains_hoistable(callee) || args.iter().any(Self::contains_hoistable),
            Expr::StructLit(_, fields, _) => fields.iter().any(|(_, e)| Self::contains_hoistable(e)),
            Expr::Match(s, arms, _) => Self::contains_hoistable(s) || arms.iter().any(|a| Self::contains_hoistable(&a.body)),
            Expr::TrailingClosure(c, _, _) => Self::contains_hoistable(c),
            Expr::Action(_, _, _, _) => false,
        }
    }

    fn hoist_inner(&mut self, expr: &Expr) -> Result<Expr, CodegenError> {
        if self.expr_depth >= MAX_EXPR_DEPTH {
            return Err(CodegenError::new(
                format!("expression nested deeper than {} levels", MAX_EXPR_DEPTH),
                expr.span(),
            ));
        }
        self.expr_depth += 1;
        let result = self.hoist_node(expr);
        self.expr_depth -= 1;
        result
    }

    fn hoist_node(&mut self, expr: &Expr) -> Result<Expr, CodegenError> {
        let b = |e: Expr| Box::new(e);
        Ok(match expr {
            Expr::Try(inner, span) => {
                let inner = self.hoist_inner(inner)?;
                self.hoist_try(&inner, *span)?
            }
            Expr::ArrayLit(elements, span) => {
                let mut hoisted = Vec::with_capacity(elements.len());
                for e in elements {
                    hoisted.push(self.hoist_inner(e)?);
                }
                self.hoist_array_literal(&hoisted, *span)?
            }
            Expr::Literal(_) | Expr::Identifier(_, _) | Expr::Nil(_) | Expr::Action(_, _, _, _) => expr.clone(),
            Expr::Binary(l, op, r, s) => Expr::Binary(b(self.hoist_inner(l)?), *op, b(self.hoist_inner(r)?), *s),
            Expr::Assign(l, r, s) => Expr::Assign(b(self.hoist_inner(l)?), b(self.hoist_inner(r)?), *s),
            Expr::CompoundAssign(l, op, r, s) => Expr::CompoundAssign(b(self.hoist_inner(l)?), *op, b(self.hoist_inner(r)?), *s),
            Expr::NullCoalesce(l, r, s) => Expr::NullCoalesce(b(self.hoist_inner(l)?), b(self.hoist_inner(r)?), *s),
            Expr::Index(l, r, s) => Expr::Index(b(self.hoist_inner(l)?), b(self.hoist_inner(r)?), *s),
            Expr::Range(l, r, s) => Expr::Range(b(self.hoist_inner(l)?), b(self.hoist_inner(r)?), *s),
            Expr::Unary(op, e, s) => Expr::Unary(*op, b(self.hoist_inner(e)?), *s),
            Expr::Member(e, f, s) => Expr::Member(b(self.hoist_inner(e)?), f.clone(), *s),
            Expr::OptionalChain(e, f, s) => Expr::OptionalChain(b(self.hoist_inner(e)?), f.clone(), *s),
            Expr::PreIncrement(e, s) => Expr::PreIncrement(b(self.hoist_inner(e)?), *s),
            Expr::PreDecrement(e, s) => Expr::PreDecrement(b(self.hoist_inner(e)?), *s),
            Expr::PostIncrement(e, s) => Expr::PostIncrement(b(self.hoist_inner(e)?), *s),
            Expr::PostDecrement(e, s) => Expr::PostDecrement(b(self.hoist_inner(e)?), *s),
            Expr::Await(e, s) => Expr::Await(b(self.hoist_inner(e)?), *s),
            Expr::Call(callee, args, s) => {
                let callee = self.hoist_inner(callee)?;
                let mut new_args = Vec::with_capacity(args.len());
                for a in args {
                    new_args.push(self.hoist_inner(a)?);
                }
                Expr::Call(b(callee), new_args, *s)
            }
            Expr::StructLit(name, fields, s) => {
                let mut new_fields = Vec::with_capacity(fields.len());
                for (n, e) in fields {
                    new_fields.push((n.clone(), self.hoist_inner(e)?));
                }
                Expr::StructLit(name.clone(), new_fields, *s)
            }
            Expr::Match(scrutinee, arms, s) => {
                let scrutinee = self.hoist_inner(scrutinee)?;
                let mut new_arms = Vec::with_capacity(arms.len());
                for arm in arms {
                    new_arms.push(crate::parser::MatchArm {
                        pattern: arm.pattern.clone(),
                        body: self.hoist_inner(&arm.body)?,
                        span: arm.span,
                    });
                }
                Expr::Match(b(scrutinee), new_arms, *s)
            }
            Expr::TrailingClosure(c, body, s) => Expr::TrailingClosure(b(self.hoist_inner(c)?), body.clone(), *s),
        })
    }

    /// Emit `Result r = f(args); if (r.err) { propagate }` and return the
    /// identifier reading `r.value`.
    fn hoist_try(&mut self, call: &Expr, span: Span) -> Result<Expr, CodegenError> {
        let (callee_name, args) = match call {
            Expr::Call(callee, args, _) => match callee.as_ref() {
                Expr::Identifier(name, _) => (name.clone(), args),
                _ => return Err(CodegenError::new("'try' must be applied to a call of a named function", span)),
            },
            _ => return Err(CodegenError::new("'try' must be applied to a call", span)),
        };
        let sig = match self.fns.get(&callee_name) {
            Some(sig) if sig.throws => sig.clone(),
            _ => return Err(CodegenError::new(format!("'{}' is not a throwing function", callee_name), span)),
        };
        let ret_name = self.result_type_name(&sig.ret);
        let result_var = format!("__rx_r{}", self.label_counter);
        self.label_counter += 1;

        // The call is a statement of its own: its temporaries are released
        // right after it.
        let saved_temps = std::mem::take(&mut self.stmt_temps);
        self.emit_indent();
        self.emit(&format!("{} {} = ", ret_name, result_var));
        self.gen_call(&callee_name, args, span)?;
        self.emit(";\n");
        self.emit_stmt_temp_releases();
        self.stmt_temps = saved_temps;

        self.emit_line(&format!("if ({}.err != NULL) {{", result_var));
        self.indent();
        let err = format!("{}.err", result_var);
        self.emit_error_exit(&err)?;
        self.dedent();
        self.emit_line("}");

        if sig.ret == CType::Void {
            return Ok(Expr::Nil(span));
        }
        // The value is +1 inside the result struct: hand it to a statement
        // temporary so that it is released at the end of the statement, and
        // read it as a borrowed identifier.
        let value_expr = format!("{}.value", result_var);
        if self.needs_release(&sig.ret) {
            let tmp = self.new_temp(&sig.ret, true);
            self.emit_line(&format!("{} = {};", tmp, value_expr));
            self.hoisted.insert(tmp.clone(), sig.ret.clone());
            Ok(Expr::Identifier(tmp, span))
        } else {
            self.hoisted.insert(value_expr.clone(), sig.ret.clone());
            Ok(Expr::Identifier(value_expr, span))
        }
    }

    /// Emit the construction of an array literal into a statement temporary
    /// and return the identifier reading it.
    fn hoist_array_literal(&mut self, elements: &[Expr], span: Span) -> Result<Expr, CodegenError> {
        let elem_ty = match elements.first() {
            Some(first) => self.infer_type(first),
            None => CType::Unknown,
        };
        let arr_ty = CType::Array(Box::new(elem_ty.clone()));
        let tmp = self.new_temp(&arr_ty, true);
        let elem_c = if elem_ty == CType::Unknown { "int64_t".to_string() } else { self.c_name(&elem_ty) };
        let ops = self.elem_ops(&elem_ty);
        self.emit_line(&format!(
            "{} = array_new(sizeof({}), {}, {});",
            tmp, elem_c, elements.len().max(1), ops
        ));
        self.emit_line(&format!("if (!{}) rx_panic(\"out of memory allocating array\");", tmp));
        for e in elements {
            let saved_temps = std::mem::take(&mut self.stmt_temps);
            self.emit_indent();
            self.emit(&format!("array_push({}, ({}[]){{ ", tmp, elem_c));
            self.gen_expr_borrowed(e, Some(&elem_ty))?;
            self.emit(" });\n");
            self.emit_stmt_temp_releases();
            self.stmt_temps = saved_temps;
        }
        self.hoisted.insert(tmp.clone(), arr_ty);
        Ok(Expr::Identifier(tmp, span))
    }

    // === Type inference (pure) ===

    fn builtin_return_type(&self, name: &str, args: &[Expr]) -> Option<CType> {
        let arg_ty = |i: usize| args.get(i).map(|a| self.infer_type(a)).unwrap_or(CType::Unknown);
        Some(match name {
            "println" | "print" | "push" | "map_remove" | "map_clear" | "array_clear" | "exit" | "rx_exit" => CType::Void,
            "len" | "array_length" | "array_len" | "map_size" | "map_len" | "str_len" => CType::Int,
            "str" | "string_concat" | "int_to_str" | "float_to_str" | "bool_to_str" | "to_upper" | "to_lower" | "trim" => CType::Str,
            "map_has" | "contains" | "starts_with" | "ends_with" | "array_push" | "map_set" => CType::Bool,
            "map_new" => CType::Map(Box::new(CType::Unknown)),
            "map_keys" | "split" => CType::Array(Box::new(CType::Str)),
            "map_get" => match arg_ty(0) {
                CType::Map(v) => *v,
                _ => CType::Unknown,
            },
            "pop" | "array_pop" | "array_get" => match arg_ty(0) {
                CType::Array(e) => *e,
                _ => CType::Unknown,
            },
            _ => return None,
        })
    }

    fn is_user_fn(&self, name: &str) -> bool {
        self.fns.contains_key(name)
    }

    /// Static type of an expression. Never emits code.
    fn infer_type(&self, expr: &Expr) -> CType {
        match expr {
            Expr::Literal(Literal::Int(_, _)) => CType::Int,
            Expr::Literal(Literal::Float(_, _)) => CType::Float,
            Expr::Literal(Literal::String(_, _)) => CType::Str,
            Expr::Literal(Literal::Bool(_, _)) => CType::Bool,
            Expr::Identifier(name, _) => {
                if let Some(ty) = self.hoisted.get(name) {
                    return ty.clone();
                }
                if let Some((ty, true)) = self.lookup_local(name) {
                    return CType::Optional(Box::new(ty));
                }
                if let Some(ty) = self.lookup_plain(name) {
                    return ty;
                }
                if let Some(ty) = self.globals.get(name) {
                    return ty.clone();
                }
                if let Some(sig) = self.fns.get(name) {
                    return CType::Function(sig.params.clone(), Box::new(sig.ret.clone()));
                }
                CType::Unknown
            }
            Expr::Binary(l, op, r, _) => {
                let lt = self.infer_type(l);
                let rt = self.infer_type(r);
                match op {
                    BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge | BinOp::And | BinOp::Or => CType::Bool,
                    BinOp::Add if lt == CType::Str || rt == CType::Str => CType::Str,
                    _ => {
                        if lt == CType::Float || rt == CType::Float { CType::Float }
                        else if lt == CType::Unknown { rt } else { lt }
                    }
                }
            }
            Expr::Unary(UnaryOp::Not, _, _) => CType::Bool,
            Expr::Unary(_, e, _) => self.infer_type(e),
            Expr::Call(callee, args, _) => {
                if let Expr::Identifier(name, _) = callee.as_ref() {
                    if let Some(sig) = self.fns.get(name) {
                        return sig.ret.clone();
                    }
                    if let Some(ty) = self.builtin_return_type(name, args) {
                        return ty;
                    }
                    if let Some(s) = self.structs.get(name) {
                        return if s.is_ref { CType::Ref(name.clone()) } else { CType::Value(name.clone()) };
                    }
                }
                match self.infer_type(callee) {
                    CType::Function(_, ret) => *ret,
                    _ => CType::Unknown,
                }
            }
            Expr::Member(obj, field, _) => {
                let obj_ty = self.infer_type(obj);
                match obj_ty {
                    CType::Ref(name) | CType::Value(name) => self.struct_field(&name, field).map(|(t, _)| t).unwrap_or(CType::Unknown),
                    _ => CType::Unknown,
                }
            }
            Expr::OptionalChain(obj, field, _) => {
                let obj_ty = self.infer_type(obj);
                match obj_ty.unwrap_optional() {
                    CType::Ref(name) | CType::Value(name) => match self.struct_field(name, field) {
                        Some((CType::Optional(inner), _)) => CType::Optional(inner),
                        Some((t, _)) => CType::Optional(Box::new(t)),
                        None => CType::Unknown,
                    },
                    _ => CType::Unknown,
                }
            }
            Expr::Index(arr, _, _) => match self.infer_type(arr) {
                CType::Array(elem) => *elem,
                _ => CType::Unknown,
            },
            Expr::Assign(_, _, _) | Expr::CompoundAssign(_, _, _, _) => CType::Void,
            Expr::StructLit(name, _, _) => self.named_ctype(name),
            Expr::ArrayLit(elements, _) => CType::Array(Box::new(
                elements.first().map(|e| self.infer_type(e)).unwrap_or(CType::Unknown),
            )),
            Expr::Match(_, arms, _) => arms.first().map(|a| self.infer_type(&a.body)).unwrap_or(CType::Unknown),
            Expr::PreIncrement(e, _) | Expr::PreDecrement(e, _) | Expr::PostIncrement(e, _) | Expr::PostDecrement(e, _) => self.infer_type(e),
            Expr::NullCoalesce(l, r, _) => {
                let lt = self.infer_type(l);
                match lt {
                    CType::Optional(inner) => *inner,
                    CType::Unknown => self.infer_type(r),
                    other => other,
                }
            }
            Expr::TrailingClosure(c, _, _) => self.infer_type(c),
            Expr::Nil(_) => CType::Optional(Box::new(CType::Unknown)),
            Expr::Await(e, _) => self.infer_type(e),
            Expr::Range(_, _, _) => CType::Array(Box::new(CType::Int)),
            Expr::Action(params, ret, _, _) => CType::Function(
                params.iter().map(|p| self.ctype(&p.ty)).collect(),
                Box::new(ret.as_ref().map(|t| self.ctype(t)).unwrap_or(CType::Void)),
            ),
            Expr::Try(inner, _) => self.infer_type(inner),
        }
    }

    // === Expressions ===

    /// Emit `expr` and report whether the emitted value is owned (+1).
    fn gen_expr(&mut self, expr: &Expr) -> Result<bool, CodegenError> {
        self.gen_expr_ctx(expr, None)
    }

    fn gen_expr_ctx(&mut self, expr: &Expr, expected: Option<&CType>) -> Result<bool, CodegenError> {
        if self.expr_depth >= MAX_EXPR_DEPTH {
            return Err(CodegenError::new(
                format!("expression nested deeper than {} levels", MAX_EXPR_DEPTH),
                expr.span(),
            ));
        }
        self.expr_depth += 1;
        let result = self.gen_expr_inner(expr, expected);
        self.expr_depth -= 1;
        result
    }

    /// Emit `expr` as a borrowed value: an owned result is parked in a
    /// statement temporary.
    fn gen_expr_borrowed(&mut self, expr: &Expr, expected: Option<&CType>) -> Result<(), CodegenError> {
        let ty = self.infer_type(expr);
        let (text, owned) = self.capture(|g| g.gen_expr_ctx(expr, expected))?;
        if owned && self.needs_release(&ty) {
            let tmp = self.new_temp(&ty, true);
            self.emit(&format!("({} = {})", tmp, text));
        } else {
            self.emit(&text);
        }
        Ok(())
    }

    /// Emit `expr` as an owned value of type `target`: a borrowed result is
    /// retained, and the value is converted to `target` where needed.
    fn gen_expr_owned(&mut self, expr: &Expr, target: &CType) -> Result<(), CodegenError> {
        let ty = self.infer_type(expr);
        let (text, owned) = self.capture(|g| g.gen_expr_ctx(expr, Some(target)))?;
        let text = self.coerce(&text, &ty, target);
        if !owned && self.needs_release(&ty) {
            let retained = self.retain_expr(target, &text);
            self.emit(&retained);
        } else {
            self.emit(&text);
        }
        Ok(())
    }

    /// Casts needed when a value of type `from` is stored into `to`.
    fn coerce(&self, text: &str, from: &CType, to: &CType) -> String {
        match (from, to) {
            (CType::Ref(_), CType::Error) | (CType::Optional(_), CType::Error) => format!("((rx_object*){})", text),
            (CType::Error, CType::Ref(n)) => format!("(({}*){})", n, text),
            _ => text.to_string(),
        }
    }

    fn gen_args(&mut self, args: &[Expr], params: &[CType]) -> Result<(), CodegenError> {
        for (i, arg) in args.iter().enumerate() {
            if i > 0 {
                self.emit(", ");
            }
            let expected = params.get(i).cloned();
            self.gen_expr_borrowed(arg, expected.as_ref())?;
        }
        Ok(())
    }

    /// Call of a declared function (user or extern). Returns whether the
    /// result is owned.
    fn gen_call(&mut self, name: &str, args: &[Expr], span: Span) -> Result<bool, CodegenError> {
        let sig = match self.fns.get(name) {
            Some(sig) => sig.clone(),
            None => return Err(CodegenError::new(format!("call to undeclared function '{}'", name), span)),
        };
        if sig.is_extern {
            // Strings cross the boundary as C strings; a returned C string is
            // copied into a fresh object.
            if sig.ret == CType::Str {
                self.emit("str_new(");
            }
            self.emit(&format!("{}(", name));
            for (i, arg) in args.iter().enumerate() {
                if i > 0 {
                    self.emit(", ");
                }
                let param = sig.params.get(i).cloned();
                if param == Some(CType::Str) || self.infer_type(arg) == CType::Str {
                    self.emit("str_cstr(");
                    self.gen_expr_borrowed(arg, param.as_ref())?;
                    self.emit(")");
                } else {
                    self.gen_expr_borrowed(arg, param.as_ref())?;
                }
            }
            self.emit(")");
            if sig.ret == CType::Str {
                self.emit(")");
            }
            return Ok(self.needs_release(&sig.ret));
        }
        self.emit(&format!("{}(", name));
        self.gen_args(args, &sig.params)?;
        self.emit(")");
        Ok(self.needs_release(&sig.ret))
    }

    /// Runtime natives the compiled backend knows. Returns None when `name`
    /// is not a builtin.
    fn gen_builtin_call(&mut self, name: &str, args: &[Expr], expected: Option<&CType>, span: Span) -> Result<Option<bool>, CodegenError> {
        let arg_ty = |g: &Self, i: usize| args.get(i).map(|a| g.infer_type(a)).unwrap_or(CType::Unknown);
        let want = |n: usize| -> Result<(), CodegenError> {
            if args.len() != n {
                Err(CodegenError::new(format!("'{}' expects {} argument(s), found {}", name, n, args.len()), span))
            } else {
                Ok(())
            }
        };
        match name {
            "println" | "print" => {
                want(1)?;
                let ty = arg_ty(self, 0);
                let suffix = match ty.unwrap_optional() {
                    CType::Str => "str",
                    CType::Int | CType::Fixed(_) => "int",
                    CType::Float => "float",
                    CType::Bool => "bool",
                    other => return Err(CodegenError::new(format!("cannot print a value of type '{}'", other.display()), span)),
                };
                self.emit(&format!("{}_{}(", name, suffix));
                self.gen_expr_borrowed(&args[0], None)?;
                self.emit(")");
                Ok(Some(false))
            }
            "len" => {
                want(1)?;
                let f = match arg_ty(self, 0).unwrap_optional() {
                    CType::Str => "str_len",
                    CType::Array(_) => "array_len",
                    CType::Map(_) => "map_len",
                    other => return Err(CodegenError::new(format!("'len' of '{}' is not defined", other.display()), span)),
                };
                self.emit(&format!("(int64_t){}(", f));
                self.gen_expr_borrowed(&args[0], None)?;
                self.emit(")");
                Ok(Some(false))
            }
            "push" => {
                want(2)?;
                let elem = match arg_ty(self, 0) {
                    CType::Array(e) => *e,
                    other => return Err(CodegenError::new(format!("'push' needs an array, found '{}'", other.display()), span)),
                };
                let elem_c = self.c_name(&elem);
                self.emit("array_push(");
                self.gen_expr_borrowed(&args[0], None)?;
                self.emit(&format!(", ({}[]){{ ", elem_c));
                self.gen_expr_borrowed(&args[1], Some(&elem))?;
                self.emit(" })");
                Ok(Some(false))
            }
            "str" => {
                want(1)?;
                let ty = arg_ty(self, 0);
                match ty {
                    CType::Str => {
                        let owned = self.gen_expr_ctx(&args[0], None)?;
                        Ok(Some(owned))
                    }
                    CType::Int | CType::Fixed(_) => { self.emit("int_to_str("); self.gen_expr_borrowed(&args[0], None)?; self.emit(")"); Ok(Some(true)) }
                    CType::Float => { self.emit("float_to_str("); self.gen_expr_borrowed(&args[0], None)?; self.emit(")"); Ok(Some(true)) }
                    CType::Bool => { self.emit("bool_to_str("); self.gen_expr_borrowed(&args[0], None)?; self.emit(")"); Ok(Some(true)) }
                    other => Err(CodegenError::new(format!("cannot convert '{}' to string", other.display()), span)),
                }
            }
            "string_concat" => {
                want(2)?;
                self.emit("str_concat(");
                self.gen_expr_borrowed(&args[0], None)?;
                self.emit(", ");
                self.gen_expr_borrowed(&args[1], None)?;
                self.emit(")");
                Ok(Some(true))
            }
            "map_new" => {
                want(0)?;
                let value_ty = match expected {
                    Some(CType::Map(v)) if **v != CType::Unknown => (**v).clone(),
                    _ => return Err(CodegenError::new(
                        "cannot infer the value type of 'map_new()'; annotate the binding, for example 'let m: [string: int] = map_new();'",
                        span,
                    )),
                };
                let c = self.c_name(&value_ty);
                let ops = self.elem_ops(&value_ty);
                self.emit(&format!("map_new(sizeof({}), {})", c, ops));
                Ok(Some(true))
            }
            "map_set" => {
                want(3)?;
                let value_ty = match arg_ty(self, 0) {
                    CType::Map(v) => *v,
                    other => return Err(CodegenError::new(format!("'map_set' needs a map, found '{}'", other.display()), span)),
                };
                let c = self.c_name(&value_ty);
                self.emit("map_set(");
                self.gen_expr_borrowed(&args[0], None)?;
                self.emit(", ");
                self.gen_expr_borrowed(&args[1], Some(&CType::Str))?;
                self.emit(&format!(", ({}[]){{ ", c));
                self.gen_expr_borrowed(&args[2], Some(&value_ty))?;
                self.emit(" })");
                Ok(Some(false))
            }
            "map_get" => {
                want(2)?;
                let value_ty = match arg_ty(self, 0) {
                    CType::Map(v) => *v,
                    other => return Err(CodegenError::new(format!("'map_get' needs a map, found '{}'", other.display()), span)),
                };
                let c = self.c_name(&value_ty);
                self.emit(&format!("(*({}*)rx_map_at(", c));
                self.gen_expr_borrowed(&args[0], None)?;
                self.emit(", ");
                self.gen_expr_borrowed(&args[1], Some(&CType::Str))?;
                self.emit("))");
                Ok(Some(false))
            }
            "map_has" | "map_remove" => {
                want(2)?;
                self.emit(&format!("{}(", name));
                self.gen_expr_borrowed(&args[0], None)?;
                self.emit(", ");
                self.gen_expr_borrowed(&args[1], Some(&CType::Str))?;
                self.emit(")");
                Ok(Some(false))
            }
            "map_size" | "map_len" => {
                want(1)?;
                self.emit("(int64_t)map_len(");
                self.gen_expr_borrowed(&args[0], None)?;
                self.emit(")");
                Ok(Some(false))
            }
            "map_keys" => {
                want(1)?;
                self.emit("map_keys(");
                self.gen_expr_borrowed(&args[0], None)?;
                self.emit(")");
                Ok(Some(true))
            }
            "exit" | "rx_exit" => {
                want(1)?;
                self.emit("rx_exit((int)");
                self.gen_expr_borrowed(&args[0], None)?;
                self.emit(")");
                Ok(Some(false))
            }
            _ => Ok(None),
        }
    }

    fn gen_expr_inner(&mut self, expr: &Expr, expected: Option<&CType>) -> Result<bool, CodegenError> {
        match expr {
            Expr::Literal(lit) => {
                self.gen_literal(lit);
                Ok(false)
            }
            Expr::Identifier(name, _) => {
                if self.hoisted.contains_key(name) {
                    self.emit(name);
                    return Ok(false);
                }
                if let Some((_, true)) = self.lookup_local(name) {
                    // Weak binding: load a strong reference (+1) or NULL
                    let ty = self.infer_type(expr);
                    let c = self.c_name(&ty);
                    self.emit(&format!("(({})rx_weak_load({}))", c, name));
                    return Ok(true);
                }
                self.emit(name);
                Ok(false)
            }
            Expr::Binary(left, op, right, span) => {
                let lt = self.infer_type(left);
                let rt = self.infer_type(right);
                if lt == CType::Str && rt == CType::Str {
                    match op {
                        BinOp::Add => {
                            self.emit("str_concat(");
                            self.gen_expr_borrowed(left, None)?;
                            self.emit(", ");
                            self.gen_expr_borrowed(right, None)?;
                            self.emit(")");
                            return Ok(true);
                        }
                        BinOp::Eq | BinOp::Ne => {
                            if *op == BinOp::Ne { self.emit("!"); }
                            self.emit("str_eq(");
                            self.gen_expr_borrowed(left, None)?;
                            self.emit(", ");
                            self.gen_expr_borrowed(right, None)?;
                            self.emit(")");
                            return Ok(false);
                        }
                        BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => {
                            self.emit("(str_cmp(");
                            self.gen_expr_borrowed(left, None)?;
                            self.emit(", ");
                            self.gen_expr_borrowed(right, None)?;
                            self.emit(&format!(") {} 0)", self.binop_to_c(op)));
                            return Ok(false);
                        }
                        _ => return Err(CodegenError::new("operator is not defined for strings", *span)),
                    }
                }
                self.emit("(");
                self.gen_expr_borrowed(left, None)?;
                self.emit(&format!(" {} ", self.binop_to_c(op)));
                self.gen_expr_borrowed(right, None)?;
                self.emit(")");
                Ok(false)
            }
            Expr::Unary(op, operand, _) => {
                self.emit(self.unaryop_to_c(op));
                self.gen_expr_borrowed(operand, None)?;
                Ok(false)
            }
            Expr::Call(callee, args, span) => {
                if let Expr::Identifier(name, _) = callee.as_ref() {
                    // UI widget constructors and theme colors
                    if let Some(owned) = self.gen_ui_call(name, args)? {
                        return Ok(owned);
                    }
                    if self.is_user_fn(name) {
                        return self.gen_call(name, args, *span);
                    }
                    if let Some(owned) = self.gen_builtin_call(name, args, expected, *span)? {
                        return Ok(owned);
                    }
                    if let Some(info) = self.structs.get(name).cloned() {
                        // Positional constructor: Name(a, b)
                        let fields: Vec<(String, Expr)> = info.fields.iter().zip(args.iter())
                            .map(|((n, _, _), e)| (n.clone(), e.clone())).collect();
                        return self.gen_struct_lit(name, &fields, *span);
                    }
                }
                // Default: emit as regular (indirect) function call
                self.gen_expr_borrowed(callee, None)?;
                self.emit("(");
                self.gen_args(args, &[])?;
                self.emit(")");
                Ok(false)
            }
            Expr::Member(obj, field, span) => {
                let obj_ty = self.infer_type(obj);
                match &obj_ty {
                    CType::Ref(name) => {
                        let (_, weak) = self.struct_field(name, field)
                            .ok_or_else(|| CodegenError::new(format!("'{}' has no field '{}'", name, field), *span))?;
                        if weak {
                            let ty = self.infer_type(expr);
                            let c = self.c_name(&ty);
                            self.emit(&format!("(({})rx_weak_load(", c));
                            self.gen_expr_borrowed(obj, None)?;
                            self.emit(&format!("->{}))", field));
                            return Ok(true);
                        }
                        self.gen_expr_borrowed(obj, None)?;
                        self.emit(&format!("->{}", field));
                        Ok(false)
                    }
                    CType::Optional(inner) if matches!(**inner, CType::Ref(_)) => Err(CodegenError::new(
                        format!("cannot access field '{}' on optional '{}'; unwrap it with 'if let', '?.' or '??'", field, obj_ty.display()),
                        *span,
                    )),
                    _ => {
                        self.gen_expr_borrowed(obj, None)?;
                        self.emit(&format!(".{}", field));
                        Ok(false)
                    }
                }
            }
            Expr::Index(arr, idx, span) => {
                let arr_ty = self.infer_type(arr);
                match arr_ty {
                    CType::Array(elem) => {
                        let elem_c = self.c_name(&elem);
                        self.emit(&format!("(*({}*)array_at(", elem_c));
                        self.gen_expr_borrowed(arr, None)?;
                        self.emit(", (size_t)(");
                        self.gen_expr_borrowed(idx, None)?;
                        self.emit(")))");
                        Ok(false)
                    }
                    CType::Unknown => {
                        self.gen_expr_borrowed(arr, None)?;
                        self.emit("[");
                        self.gen_expr_borrowed(idx, None)?;
                        self.emit("]");
                        Ok(false)
                    }
                    other => Err(CodegenError::new(format!("cannot index into '{}'", other.display()), *span)),
                }
            }
            Expr::Assign(target, value, span) => {
                self.gen_assign(target, value, *span)?;
                Ok(false)
            }
            Expr::StructLit(name, fields, span) => self.gen_struct_lit(name, fields, *span),
            Expr::ArrayLit(_, span) => Err(CodegenError::new(
                "array literal in a position the compiler cannot hoist",
                *span,
            )),
            Expr::Match(scrutinee, arms, _) => {
                // Generate match as a series of if-else chains
                // For simple integer patterns, could use switch but if-else is more general
                self.emit("({\n");
                self.indent();

                // Store scrutinee in temporary variable
                self.emit_indent();
                self.emit("__auto_type _match_val = ");
                self.gen_expr_borrowed(scrutinee, None)?;
                self.emit(";\n");

                // Default result variable
                self.emit_line("__typeof__(_match_val) _match_result = 0;");

                for (i, arm) in arms.iter().enumerate() {
                    self.emit_indent();
                    if i == 0 {
                        self.emit("if (");
                    } else {
                        self.emit("} else if (");
                    }

                    // Generate pattern condition
                    match &arm.pattern {
                        Pattern::Literal(lit) => {
                            self.emit("_match_val == ");
                            self.gen_literal(lit);
                        }
                        Pattern::Identifier(name) => {
                            // Binding pattern - always matches
                            self.emit("1 /* bind ");
                            self.emit(name);
                            self.emit(" */");
                        }
                        Pattern::Wildcard => {
                            self.emit("1 /* wildcard */");
                        }
                    }

                    self.emit(") {\n");
                    self.indent();
                    self.emit_indent();
                    self.emit("_match_result = ");
                    self.gen_expr_borrowed(&arm.body, None)?;
                    self.emit(";\n");
                    self.dedent();
                }

                if !arms.is_empty() {
                    self.emit_line("}");
                }

                self.emit_line("_match_result;");
                self.dedent();
                self.emit_indent();
                self.emit("})");
                Ok(false)
            }
            // Swift/C++ style expressions
            Expr::CompoundAssign(target, op, value, span) => {
                let ty = self.infer_type(target);
                if ty == CType::Str && *op == CompoundOp::AddEq {
                    let concat = Expr::Binary(target.clone(), BinOp::Add, value.clone(), *span);
                    self.gen_assign(target, &concat, *span)?;
                    return Ok(false);
                }
                self.gen_expr_borrowed(target, None)?;
                self.emit(&format!(" {} ", self.compound_op_to_c(op)));
                self.gen_expr_borrowed(value, None)?;
                Ok(false)
            }
            Expr::PreIncrement(operand, _) => {
                self.emit("++");
                self.gen_expr_borrowed(operand, None)?;
                Ok(false)
            }
            Expr::PreDecrement(operand, _) => {
                self.emit("--");
                self.gen_expr_borrowed(operand, None)?;
                Ok(false)
            }
            Expr::PostIncrement(operand, _) => {
                self.gen_expr_borrowed(operand, None)?;
                self.emit("++");
                Ok(false)
            }
            Expr::PostDecrement(operand, _) => {
                self.gen_expr_borrowed(operand, None)?;
                self.emit("--");
                Ok(false)
            }
            Expr::NullCoalesce(left, right, _) => {
                // Evaluate the left side once into a scratch variable
                let lt = self.infer_type(left);
                let result_ty = self.infer_type(expr);
                let scratch_ty = if lt == CType::Unknown { result_ty.clone() } else { lt.clone() };
                let (ltext, lowned) = self.capture(|g| g.gen_expr_ctx(left, None))?;
                let release = lowned && self.needs_release(&scratch_ty);
                let tmp = self.new_temp(&scratch_ty, release);
                let zero = self.zero_value(&scratch_ty);
                self.emit(&format!("((({} = {}) != {}) ? {} : ", tmp, ltext, zero, tmp));
                self.gen_expr_borrowed(right, Some(&result_ty))?;
                self.emit(")");
                Ok(false)
            }
            Expr::OptionalChain(obj, field, span) => {
                let obj_ty = self.infer_type(obj);
                let inner = obj_ty.unwrap_optional().clone();
                let name = match &inner {
                    CType::Ref(n) => n.clone(),
                    other => return Err(CodegenError::new(
                        format!("'?.' needs an optional reference, found '{}'", other.display()),
                        *span,
                    )),
                };
                let (_, weak) = self.struct_field(&name, field)
                    .ok_or_else(|| CodegenError::new(format!("'{}' has no field '{}'", name, field), *span))?;
                let result_ty = self.infer_type(expr);
                let (otext, oowned) = self.capture(|g| g.gen_expr_ctx(obj, None))?;
                let tmp = self.new_temp(&inner, oowned);
                let zero = self.zero_value(&result_ty);
                if weak {
                    let c = self.c_name(&result_ty);
                    self.emit(&format!("((({} = {}) != NULL) ? (({})rx_weak_load({}->{})) : {})", tmp, otext, c, tmp, field, zero));
                    return Ok(true);
                }
                self.emit(&format!("((({} = {}) != NULL) ? {}->{} : {})", tmp, otext, tmp, field, zero));
                Ok(false)
            }
            Expr::TrailingClosure(callee, body, _) => {
                // Emit callee call, then the closure body as a block
                self.gen_expr_borrowed(callee, None)?;
                self.emit("; /* trailing closure: */\n");
                self.emit_line("{");
                self.indent();
                self.gen_scoped_block(body, ScopeKind::Block)?;
                self.dedent();
                self.emit_line("}");
                Ok(false)
            }
            Expr::Nil(_) => {
                self.emit("NULL");
                Ok(false)
            }
            Expr::Await(operand, _) => {
                // Emit await as rx_await() runtime call
                self.emit("rx_await(");
                self.gen_expr_borrowed(operand, None)?;
                self.emit(")");
                Ok(false)
            }
            Expr::Range(start, end, _) => {
                // Runtime call to create array from range
                self.emit("rx_range(");
                self.gen_expr_borrowed(start, None)?;
                self.emit(", ");
                self.gen_expr_borrowed(end, None)?;
                self.emit(")");
                Ok(true)
            }
            Expr::Action(params, return_type, body, _) => {
                // Generate a static C function for this closure
                let closure_id = self.closure_counter;
                self.closure_counter += 1;
                let fn_name = format!("__rx_closure_{}", closure_id);

                let ret_c = return_type.as_ref()
                    .map(|t| self.type_to_c(t))
                    .unwrap_or_else(|| "void".to_string());

                let param_strs: Vec<String> = params.iter()
                    .map(|p| format!("{} {}", self.type_to_c(&p.ty), p.name))
                    .collect();
                let params_c = if param_strs.is_empty() {
                    "void".to_string()
                } else {
                    param_strs.join(", ")
                };

                // Generate the closure body with its own function state
                let mut body_gen = CodeGen::new();
                body_gen.structs = self.structs.clone();
                body_gen.fns = self.fns.clone();
                body_gen.globals = self.globals.clone();
                body_gen.aliases = self.aliases.clone();
                body_gen.string_literals = std::mem::take(&mut self.string_literals);
                body_gen.closure_counter = self.closure_counter;
                body_gen.indent = 1;
                let ret_ty = return_type.as_ref().map(|t| self.ctype(t)).unwrap_or(CType::Void);
                body_gen.begin_function(ret_ty, false, false);
                for p in params {
                    let ty = self.ctype(&p.ty);
                    body_gen.plain_locals.push((p.name.clone(), ty));
                }
                let (inner, ()) = body_gen.capture(|g| {
                    g.gen_block(body)?;
                    if !Self::block_terminates(body) {
                        g.emit_scope_releases();
                    }
                    Ok(())
                })?;
                body_gen.emit_temp_decls();
                body_gen.emit(&inner);
                self.string_literals = std::mem::take(&mut body_gen.string_literals);
                self.closure_counter = body_gen.closure_counter;
                self.closure_defs.push_str(&body_gen.closure_defs);

                self.closure_defs.push_str(&format!(
                    "static {} {}({}) {{\n{}}}\n\n",
                    ret_c, fn_name, params_c, body_gen.output
                ));

                // Emit the function pointer at the call site
                self.emit(&fn_name);
                Ok(false)
            }
            Expr::Try(_, span) => Err(CodegenError::new(
                "'try' in a position the compiler cannot hoist",
                *span,
            )),
        }
    }

    /// UI widget constructors and theme colors that map straight to runtime
    /// calls. Returns None when `name` is not one of them.
    fn gen_ui_call(&mut self, name: &str, args: &[Expr]) -> Result<Option<bool>, CodegenError> {
        if self.is_user_fn(name) {
            return Ok(None);
        }
        let direct = match name {
            "button" => Some("reox_button_create"),
            "text" | "label" => Some("reox_label_create"),
            "input" | "textfield" => Some("reox_textfield_create"),
            "slider" => Some("reox_slider_create"),
            "checkbox" => Some("reox_checkbox_create"),
            "window" => Some("reox_window_create"),
            _ => None,
        };
        if let Some(c_fn) = direct {
            self.emit(&format!("{}(", c_fn));
            self.gen_args(args, &[])?;
            self.emit(")");
            return Ok(Some(false));
        }
        match name {
            "vstack" | "hstack" => {
                self.emit(&format!("reox_{}(", name));
                if let Some(first) = args.first() {
                    self.gen_expr_borrowed(first, None)?;
                } else {
                    self.emit("0");
                }
                self.emit(")");
                Ok(Some(false))
            }
            // Theme colors -> emit directly
            "color_primary" => { self.emit("(rx_color){0, 122, 255, 255}"); Ok(Some(false)) }
            "color_secondary" => { self.emit("(rx_color){88, 86, 214, 255}"); Ok(Some(false)) }
            "color_success" => { self.emit("(rx_color){52, 199, 89, 255}"); Ok(Some(false)) }
            "color_warning" => { self.emit("(rx_color){255, 149, 0, 255}"); Ok(Some(false)) }
            "color_danger" => { self.emit("(rx_color){255, 59, 48, 255}"); Ok(Some(false)) }
            "color_background" => { self.emit("(rx_color){28, 28, 30, 255}"); Ok(Some(false)) }
            "color_surface" => { self.emit("(rx_color){44, 44, 46, 255}"); Ok(Some(false)) }
            "color_text" => { self.emit("(rx_color){255, 255, 255, 255}"); Ok(Some(false)) }
            _ => Ok(None),
        }
    }

    /// Struct literal: a value struct becomes a compound literal whose
    /// reference fields are owned; a ref struct is allocated through its
    /// generated constructor.
    fn gen_struct_lit(&mut self, name: &str, fields: &[(String, Expr)], span: Span) -> Result<bool, CodegenError> {
        let info = match self.structs.get(name) {
            Some(info) => info.clone(),
            None => {
                // Unknown struct (e.g. a runtime type): plain compound literal
                self.emit(&format!("({}){{", name));
                for (i, (field_name, value)) in fields.iter().enumerate() {
                    if i > 0 {
                        self.emit(", ");
                    }
                    self.emit(&format!(".{} = ", field_name));
                    self.gen_expr_borrowed(value, None)?;
                }
                self.emit("}");
                return Ok(false);
            }
        };
        for (field_name, _) in fields {
            if !info.fields.iter().any(|(n, _, _)| n == field_name) {
                return Err(CodegenError::new(format!("'{}' has no field '{}'", name, field_name), span));
            }
        }
        if info.is_ref {
            self.emit(&format!("{}__make(", name));
            for (i, (field_name, ty, weak)) in info.fields.iter().enumerate() {
                if i > 0 {
                    self.emit(", ");
                }
                match fields.iter().find(|(n, _)| n == field_name) {
                    Some((_, value)) if *weak => {
                        let inner = ty.unwrap_optional().clone();
                        self.gen_expr_borrowed(value, Some(&inner))?;
                    }
                    Some((_, value)) => self.gen_expr_owned(value, ty)?,
                    None => {
                        let zero = if *weak { "NULL".to_string() } else { self.zero_value(ty) };
                        self.emit(&zero);
                    }
                }
            }
            self.emit(")");
            Ok(true)
        } else {
            self.emit(&format!("({}){{", name));
            let mut first = true;
            for (field_name, ty, _) in &info.fields {
                if let Some((_, value)) = fields.iter().find(|(n, _)| n == field_name) {
                    if !first {
                        self.emit(", ");
                    }
                    first = false;
                    self.emit(&format!(".{} = ", field_name));
                    self.gen_expr_owned(value, ty)?;
                }
            }
            self.emit("}");
            Ok(true)
        }
    }

    /// `target = value` with the retain/release pairs the target's type needs.
    fn gen_assign(&mut self, target: &Expr, value: &Expr, _span: Span) -> Result<(), CodegenError> {
        // Weak slots
        if let Expr::Identifier(name, _) = target {
            if let Some((ty, true)) = self.lookup_local(name) {
                self.emit(&format!("rx_weak_assign(&{}, ", name));
                self.gen_expr_borrowed(value, Some(&ty))?;
                self.emit(")");
                return Ok(());
            }
        }
        if let Expr::Member(obj, field, _) = target {
            if let CType::Ref(sname) = self.infer_type(obj) {
                if let Some((ty, true)) = self.struct_field(&sname, field) {
                    self.emit("rx_weak_assign(&");
                    self.gen_expr_borrowed(obj, None)?;
                    self.emit(&format!("->{}, ", field));
                    self.gen_expr_borrowed(value, Some(ty.unwrap_optional()))?;
                    self.emit(")");
                    return Ok(());
                }
            }
        }
        // Array element
        if let Expr::Index(arr, idx, _) = target {
            if let CType::Array(elem) = self.infer_type(arr) {
                let elem_c = self.c_name(&elem);
                self.emit("array_set(");
                self.gen_expr_borrowed(arr, None)?;
                self.emit(", (size_t)(");
                self.gen_expr_borrowed(idx, None)?;
                self.emit(&format!("), ({}[]){{ ", elem_c));
                self.gen_expr_borrowed(value, Some(&elem))?;
                self.emit(" })");
                return Ok(());
            }
        }

        let ty = self.infer_type(target);
        if ty.is_ref() {
            // Store the owned new value, releasing the old one.
            let (ttext, _) = self.capture(|g| g.gen_expr_ctx(target, None))?;
            self.emit(&format!("rx_store((void**)&{}, ", ttext));
            self.gen_expr_owned(value, &ty)?;
            self.emit(")");
            return Ok(());
        }
        if let CType::Value(n) = &ty {
            if self.value_has_refs(n) {
                let (ttext, _) = self.capture(|g| g.gen_expr_ctx(target, None))?;
                let tmp = self.new_temp(&ty, false);
                self.emit(&format!("({} = {}, ", tmp, ttext));
                self.emit(&format!("{} = ", ttext));
                self.gen_expr_owned(value, &ty)?;
                self.emit(&format!(", {}__release_fields(&{}))", n, tmp));
                return Ok(());
            }
        }
        self.gen_expr_borrowed(target, None)?;
        self.emit(" = ");
        self.gen_expr_borrowed(value, Some(&ty))?;
        Ok(())
    }

    fn compound_op_to_c(&self, op: &CompoundOp) -> &'static str {
        match op {
            CompoundOp::AddEq => "+=",
            CompoundOp::SubEq => "-=",
            CompoundOp::MulEq => "*=",
            CompoundOp::DivEq => "/=",
            CompoundOp::ModEq => "%=",
        }
    }

    fn gen_literal(&mut self, lit: &Literal) {
        match lit {
            Literal::Int(n, _) => self.emit(&format!("{}", n)),
            Literal::Float(n, _) => self.emit(&format!("{:.15}", n)),
            Literal::String(s, _) => {
                let escaped = self.escape_string(s);
                let index = match self.string_literals.iter().position(|l| *l == escaped) {
                    Some(i) => i,
                    None => {
                        self.string_literals.push(escaped);
                        self.string_literals.len() - 1
                    }
                };
                self.emit(&format!("(&__rx_lit{})", index));
            }
            Literal::Bool(b, _) => self.emit(if *b { "true" } else { "false" }),
        }
    }

    fn escape_string(&self, s: &str) -> String {
        let mut result = String::new();
        for ch in s.chars() {
            match ch {
                '\n' => result.push_str("\\n"),
                '\t' => result.push_str("\\t"),
                '\r' => result.push_str("\\r"),
                '\\' => result.push_str("\\\\"),
                '"' => result.push_str("\\\""),
                '\0' => result.push_str("\\0"),
                _ => result.push(ch),
            }
        }
        result
    }

    fn binop_to_c(&self, op: &BinOp) -> &'static str {
        match op {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Mod => "%",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::Lt => "<",
            BinOp::Gt => ">",
            BinOp::Le => "<=",
            BinOp::Ge => ">=",
            BinOp::And => "&&",
            BinOp::Or => "||",
            BinOp::BitwiseAnd => "&",
            BinOp::BitwiseOr => "|",
            BinOp::BitwiseXor => "^",
            BinOp::ShiftLeft => "<<",
            BinOp::ShiftRight => ">>",
        }
    }

    fn unaryop_to_c(&self, op: &UnaryOp) -> &'static str {
        match op {
            UnaryOp::Neg => "-",
            UnaryOp::Not => "!",
            UnaryOp::BitwiseNot => "~",
        }
    }
}

impl Default for CodeGen {
    fn default() -> Self {
        Self::new()
    }
}

/// Stack reserved for code generation; see `generate_string`.
const CODEGEN_STACK_BYTES: usize = 64 * 1024 * 1024;

/// Generate C code from AST as a string. Code generation recurses over the
/// expression tree, so it runs on a thread with room for `MAX_EXPR_DEPTH` levels.
pub fn generate_string(ast: &Ast) -> Result<String, CodegenError> {
    std::thread::scope(|scope| {
        let worker = std::thread::Builder::new()
            .name("reox-codegen".to_string())
            .stack_size(CODEGEN_STACK_BYTES)
            .spawn_scoped(scope, || CodeGen::new().generate(ast));
        match worker {
            Ok(handle) => handle.join().unwrap_or_else(|_| {
                Err(CodegenError::new("code generator thread terminated unexpectedly", Span::default()))
            }),
            Err(_) => CodeGen::new().generate(ast),
        }
    })
}

/// Generate C code from AST and write to file. I/O failures are reported
/// as codegen errors naming the output path.
pub fn generate(ast: &Ast, output_path: &str) -> Result<(), CodegenError> {
    let c_code = generate_string(ast)?;

    let io_err = |what: &str, e: std::io::Error| {
        CodegenError::new(format!("failed to {} '{}': {}", what, output_path, e), Span::default())
    };
    let mut file = std::fs::File::create(output_path).map_err(|e| io_err("create", e))?;
    file.write_all(c_code.as_bytes()).map_err(|e| io_err("write", e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;
    use crate::parser::parse;

    fn gen(source: &str) -> String {
        let tokens = tokenize(source).unwrap();
        let ast = parse(&tokens).unwrap();
        CodeGen::new().generate(&ast).unwrap()
    }

    fn gen_err(source: &str) -> CodegenError {
        let tokens = tokenize(source).unwrap();
        let ast = parse(&tokens).unwrap();
        CodeGen::new().generate(&ast).unwrap_err()
    }

    #[test]
    fn test_simple_function() {
        let output = gen(r#"
            fn main() {
                let x: int = 42;
                return 0;
            }
        "#);

        assert!(output.contains("int main(void)"));
        assert!(output.contains("int64_t x = 42"));
        assert!(output.contains("return (int)(0)"));
    }

    #[test]
    fn test_function_with_params() {
        let output = gen(r#"
            fn add(a: int, b: int) -> int {
                return a + b;
            }
        "#);

        assert!(output.contains("int64_t add(int64_t a, int64_t b)"));
        assert!(output.contains("return (a + b)"));
    }

    #[test]
    fn test_struct() {
        let output = gen(r#"
            struct Point {
                x: int,
                y: int
            }
        "#);

        assert!(output.contains("struct Point {"));
        assert!(output.contains("int64_t x;"));
        assert!(output.contains("int64_t y;"));
    }

    #[test]
    fn test_if_else() {
        let output = gen(r#"
            fn check(x: int) -> int {
                if x > 0 {
                    return 1;
                } else {
                    return 0;
                }
            }
        "#);

        assert!(output.contains("if ((x > 0))"));
        assert!(output.contains("} else {"));
    }

    #[test]
    fn test_while_loop() {
        let output = gen(r#"
            fn count() {
                let mut i: int = 0;
                while i < 10 {
                    i = i + 1;
                }
            }
        "#);

        assert!(output.contains("while ((i < 10))"));
    }

    // === ARC ===

    #[test]
    fn string_locals_are_retained_and_released() {
        let output = gen(r#"
            fn main() {
                let a: string = "hi";
                let b: string = a;
                println(b);
            }
        "#);
        assert!(output.contains("static rx_str __rx_lit0 = RX_STR_LIT_INIT(\"hi\");"), "{}", output);
        assert!(output.contains("rx_str* a = ((rx_str*)rx_retain((&__rx_lit0)));"), "{}", output);
        assert!(output.contains("rx_str* b = ((rx_str*)rx_retain(a));"), "{}", output);
        assert!(output.contains("println_str(b);"), "{}", output);
        // released in reverse order at scope exit
        let rb = output.find("rx_release(b);").expect("b released");
        let ra = output.find("rx_release(a);").expect("a released");
        assert!(rb < ra, "{}", output);
    }

    #[test]
    fn owned_temporaries_are_released_at_statement_end() {
        let output = gen(r#"
            fn make() -> string { return "x"; }
            fn main() {
                println(make());
            }
        "#);
        assert!(output.contains("println_str((__rx_t0 = make()));"), "{}", output);
        assert!(output.contains("rx_release(__rx_t0);"), "{}", output);
    }

    #[test]
    fn returns_transfer_ownership() {
        let output = gen(r#"
            fn pick(a: string) -> string {
                let b: string = a;
                return b;
            }
        "#);
        // The local is retained for the return value, then released as a local.
        assert!(output.contains("__rx_t0 = ((rx_str*)rx_retain(b));"), "{}", output);
        assert!(output.contains("rx_release(b);"), "{}", output);
        assert!(output.contains("return __rx_t0;"), "{}", output);
    }

    #[test]
    fn ref_struct_gets_header_dealloc_and_constructor() {
        let output = gen(r#"
            ref struct Node {
                value: int,
                next: Node?,
                weak parent: Node
            }
            fn main() {
                let n = Node { value: 1 };
            }
        "#);
        assert!(output.contains("rx_object header;"), "{}", output);
        assert!(output.contains("rx_weak parent;"), "{}", output);
        assert!(output.contains(&format!("#define Node__TYPE_ID 0x{:08x}u", type_id_hash("Node"))), "{}", output);
        assert!(output.contains("rx_release(self->next);"), "{}", output);
        assert!(output.contains("rx_weak_drop(self->parent);"), "{}", output);
        assert!(output.contains("o->parent = rx_weak_ref(parent);"), "{}", output);
        assert!(output.contains("Node* n = Node__make(1, NULL, NULL);"), "{}", output);
        assert!(output.contains("rx_release(n);"), "{}", output);
    }

    #[test]
    fn weak_locals_use_the_weak_table() {
        let output = gen(r#"
            ref struct Node { value: int }
            fn main() {
                let n = Node { value: 1 };
                weak let w = n;
                if let got = w {
                    println(got.value);
                }
            }
        "#);
        assert!(output.contains("rx_weak w = rx_weak_ref(n);"), "{}", output);
        assert!(output.contains("Node* got = ((Node*)rx_weak_load(w));"), "{}", output);
        assert!(output.contains("rx_weak_drop(w);"), "{}", output);
        assert!(output.contains("println_int(got->value);"), "{}", output);
    }

    #[test]
    fn assignment_releases_the_old_value() {
        let output = gen(r#"
            fn main() {
                let mut s: string = "a";
                s = "b";
            }
        "#);
        assert!(output.contains("rx_store((void**)&s, ((rx_str*)rx_retain((&__rx_lit1))));"), "{}", output);
    }

    #[test]
    fn break_releases_loop_locals_only() {
        let output = gen(r#"
            fn main() {
                let outer: string = "o";
                while true {
                    let inner: string = "i";
                    break;
                }
            }
        "#);
        let brk = output.find("break;").expect("break");
        let inner = output.find("rx_release(inner);").expect("inner released before break");
        assert!(inner < brk, "{}", output);
        let before_break = &output[..brk];
        assert!(!before_break.contains("rx_release(outer);"), "{}", output);
    }

    #[test]
    fn no_setjmp_longjmp_in_generated_code() {
        let output = gen(r#"
            ref struct Oops { code: int }
            extension Oops: Error { }
            fn risky(n: int) throws -> int {
                if n < 0 { throw Oops { code: n }; }
                return n;
            }
            fn main() {
                try {
                    let v = try risky(1);
                    println(v);
                } catch e as Oops {
                    println(e.code);
                }
            }
        "#);
        assert!(!output.contains("setjmp"), "{}", output);
        assert!(!output.contains("longjmp"), "{}", output);
        assert!(output.contains("typedef struct Result__int__Error {"), "{}", output);
        assert!(output.contains("Result__int__Error risky(int64_t n)"), "{}", output);
        assert!(output.contains("return (Result__int__Error){ .err = __rx_t0 };"), "{}", output);
        assert!(output.contains("return (Result__int__Error){ .err = NULL, .value = n };"), "{}", output);
        assert!(output.contains("Result__int__Error __rx_r1 = risky(1);"), "{}", output);
        assert!(output.contains("goto __rx_catch0;"), "{}", output);
        assert!(output.contains("if (rx_type_id(__rx_err0) != Oops__TYPE_ID)"), "{}", output);
        assert!(output.contains("Oops* e = (Oops*)__rx_err0;"), "{}", output);
        assert!(output.contains("rx_release(e);"), "{}", output);
    }

    #[test]
    fn extern_strings_cross_as_c_strings() {
        let output = gen(r#"
            extern fn host_name() -> string;
            extern fn host_log(msg: string) -> void;
            fn main() {
                let n: string = host_name();
                host_log(n);
            }
        "#);
        assert!(output.contains("extern const char* host_name(void);"), "{}", output);
        assert!(output.contains("extern void host_log(const char* msg);"), "{}", output);
        assert!(output.contains("rx_str* n = str_new(host_name());"), "{}", output);
        assert!(output.contains("host_log(str_cstr(n));"), "{}", output);
    }

    #[test]
    fn map_new_needs_a_value_type() {
        let err = gen_err("fn main() { let m = map_new(); }");
        assert!(err.message.contains("cannot infer the value type"), "{}", err.message);
        let output = gen("fn main() { let m: [string: int] = map_new(); map_set(m, \"k\", 1); }");
        assert!(output.contains("rx_map* m = map_new(sizeof(int64_t), NULL);"), "{}", output);
        assert!(output.contains("map_set(m, (&__rx_lit0), (int64_t[]){ 1 });"), "{}", output);
    }

    // === Hardening: error paths ===

    #[test]
    fn generate_reports_unwritable_output_path() {
        let tokens = tokenize("fn main() { }").unwrap();
        let ast = parse(&tokens).unwrap();
        let err = generate(&ast, "/nonexistent-dir-for-reox/out.c").unwrap_err();
        assert!(err.message.starts_with("failed to create '/nonexistent-dir-for-reox/out.c'"), "{}", err.message);
        assert!(err.display().starts_with("codegen error[1:1]"));
    }

    #[test]
    fn generate_string_matches_codegen_output() {
        let tokens = tokenize("fn main() -> int { return 1; }").unwrap();
        let ast = parse(&tokens).unwrap();
        let direct = CodeGen::new().generate(&ast).unwrap();
        assert_eq!(generate_string(&ast).unwrap(), direct);
    }

    #[test]
    fn deep_expression_tree_is_reported_not_overflowed() {
        let span = crate::lexer::Span::default();
        let mut expr = Expr::Literal(Literal::Int(1, span));
        for _ in 0..(MAX_EXPR_DEPTH + 10) {
            expr = Expr::Unary(UnaryOp::Neg, Box::new(expr), span);
        }
        let ast = Ast {
            declarations: vec![Decl::Function(FnDecl {
                name: "main".into(),
                params: vec![],
                return_type: None,
                body: Block { statements: vec![Stmt::Expr(expr)], span },
                is_async: false,
                throws: false,
                is_pub: false,
                is_static: false,
                span,
            })],
        };
        let err = generate_string(&ast).unwrap_err();
        assert!(err.message.contains("nested deeper than"), "{}", err.message);
    }
}
