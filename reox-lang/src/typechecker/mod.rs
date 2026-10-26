// REOX Compiler - Type Checker
// Validates types and builds symbol table
// Zero external dependencies

#![allow(dead_code, unused_imports, unused_variables)]

mod types;

pub use types::*;

use std::collections::{HashMap, HashSet};

use crate::parser::{
    Ast, Decl, Stmt, Expr, Literal, BinOp, UnaryOp,
    FnDecl, StructDecl, ExternDecl, Block, Type, LetStmt,
    ReturnStmt, IfStmt, WhileStmt, ForStmt, GuardStmt, DeferStmt,
    TryCatchStmt, ThrowStmt, CompoundOp,
};
use crate::lexer::Span;

/// Type check error, located by a source span
#[derive(Debug, Clone, PartialEq)]
pub struct TypeError {
    pub message: String,
    pub span: Span,
}

impl TypeError {
    pub fn new(message: impl Into<String>, span: &Span) -> Self {
        Self {
            message: message.into(),
            span: *span,
        }
    }

    pub fn line(&self) -> u32 {
        self.span.line
    }

    pub fn column(&self) -> u32 {
        self.span.column
    }

    pub fn display(&self) -> String {
        format!("type error[{}:{}]: {}", self.span.line, self.span.column, self.message)
    }
}

impl std::fmt::Display for TypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.display())
    }
}

impl std::error::Error for TypeError {}

/// Deepest expression tree the checker walks before reporting an error
/// instead of recursing further.
pub const MAX_EXPR_DEPTH: usize = 1024;

/// Longest chain of type aliases followed before reporting a cycle.
const MAX_ALIAS_CHAIN: usize = 64;

/// Stack reserved for type checking; see `check`.
const CHECK_STACK_BYTES: usize = 64 * 1024 * 1024;

/// Type checker state
pub struct TypeChecker {
    symbols: SymbolTable,
    errors: Vec<TypeError>,
    current_return_type: Option<ResolvedType>,
    type_aliases: HashMap<String, ResolvedType>,
    expr_depth: usize,
    /// Runtime natives (print, len, push, ...). They accept any arguments and
    /// return an unknown type; a user declaration of the same name shadows them.
    builtins: HashSet<String>,
    /// Names of reference types: `ref struct`s and layers. Strings, arrays
    /// and maps are reference types as well (see `is_reference_type`).
    ref_types: HashSet<String>,
    /// Protocols each named type conforms to (`extension T: P`).
    conformances: HashMap<String, HashSet<String>>,
    /// Functions declared `throws`.
    throwing_fns: HashSet<String>,
    /// Whether the function being checked is declared `throws`.
    current_fn_throws: bool,
    /// Name of the function being checked.
    current_fn_name: String,
    /// Nesting depth of `try { }` blocks around the current statement.
    try_depth: usize,
    /// Set while checking the operand of a `try` expression so the call
    /// inside it is allowed to throw.
    in_try_expr: bool,
}

impl TypeChecker {
    pub fn new() -> Self {
        let mut builtins: HashSet<String> = crate::interpreter::native_names()
            .map(str::to_string)
            .collect();
        builtins.insert("print".to_string());
        Self {
            symbols: SymbolTable::new(),
            errors: Vec::new(),
            current_return_type: None,
            type_aliases: HashMap::new(),
            expr_depth: 0,
            builtins,
            ref_types: HashSet::new(),
            conformances: HashMap::new(),
            throwing_fns: HashSet::new(),
            current_fn_throws: false,
            current_fn_name: String::new(),
            try_depth: 0,
            in_try_expr: false,
        }
    }

    /// Whether values of `ty` are reference counted heap objects.
    fn is_reference_type(&self, ty: &ResolvedType) -> bool {
        match self.resolve_type(ty) {
            ResolvedType::String | ResolvedType::Array(_) | ResolvedType::Map(_, _) => true,
            ResolvedType::Struct(name) => name == "Error" || self.ref_types.contains(&name),
            ResolvedType::Optional(inner) => self.is_reference_type(&inner),
            _ => false,
        }
    }

    /// Whether `ty` may be thrown: a `ref struct` that conforms to `Error`,
    /// or the `Error` existential itself (rethrow).
    fn conforms_to_error(&self, ty: &ResolvedType) -> bool {
        match self.resolve_type(ty) {
            ResolvedType::Struct(name) => {
                name == "Error"
                    || (self.ref_types.contains(&name)
                        && self.conformances.get(&name).is_some_and(|p| p.contains("Error")))
            }
            ResolvedType::Error | ResolvedType::Unknown => true,
            _ => false,
        }
    }

    /// Root identifier of an assignment target such as `a.b[0].c`.
    fn assignment_root(expr: &Expr) -> Option<(&str, &Span)> {
        match expr {
            Expr::Identifier(name, span) => Some((name.as_str(), span)),
            Expr::Member(obj, _, _) | Expr::Index(obj, _, _) | Expr::OptionalChain(obj, _, _) => {
                Self::assignment_root(obj)
            }
            _ => None,
        }
    }

    /// Whether `name` is a runtime built-in that no user declaration shadows.
    fn is_builtin(&self, name: &str) -> bool {
        self.builtins.contains(name) && self.symbols.lookup(name).is_none()
    }

    /// Type check the entire AST
    pub fn check_program(&mut self, ast: &Ast) -> Result<(), Vec<TypeError>> {
        // Pass zero: which names are reference types and which protocols
        // they conform to, so that field and `weak` checks in the first pass
        // can refer to types declared later in the file.
        for decl in &ast.declarations {
            match decl {
                Decl::Struct(s) if s.is_ref => { self.ref_types.insert(s.name.clone()); }
                Decl::Layer(l) => { self.ref_types.insert(l.name.clone()); }
                Decl::Extension(ext) => {
                    self.conformances.entry(ext.target.clone()).or_default()
                        .extend(ext.protocols.iter().cloned());
                }
                Decl::Function(f) if f.throws => { self.throwing_fns.insert(f.name.clone()); }
                _ => {}
            }
        }
        // The `Error` protocol is intrinsic: every program can throw and
        // catch it without importing the prelude.
        let _ = self.symbols.define(Symbol {
            name: "Error".to_string(),
            ty: ResolvedType::Struct("Error".to_string()),
            mutable: false,
            kind: SymbolKind::Struct,
        });

        // First pass: collect all struct and function declarations
        for decl in &ast.declarations {
            match decl {
                Decl::Struct(s) => self.register_struct(s),
                Decl::Function(f) => self.register_function(f),
                Decl::Extern(e) => self.register_extern(e),
                Decl::Import(_) => {}
                Decl::Variant(v) => {
                    // Register variant type itself
                    let _ = self.symbols.define(Symbol {
                        name: v.name.clone(),
                        ty: ResolvedType::Struct(v.name.clone()),
                        mutable: false,
                        kind: SymbolKind::Struct,
                    });
                    // Register each case as a constructor function
                    for case in &v.cases {
                        let case_symbol_name = format!("{}_{}", v.name, case.name);
                        if case.fields.is_empty() {
                            let _ = self.symbols.define(Symbol {
                                name: case_symbol_name,
                                ty: ResolvedType::Struct(v.name.clone()),
                                mutable: false,
                                kind: SymbolKind::Variable,
                            });
                        } else {
                            let param_types: Vec<ResolvedType> = case.fields.iter()
                                .map(|f| ResolvedType::from_parser_type(&f.ty))
                                .collect();
                            let _ = self.symbols.define(Symbol {
                                name: case_symbol_name,
                                ty: ResolvedType::Function {
                                    params: param_types,
                                    ret: Box::new(ResolvedType::Struct(v.name.clone())),
                                },
                                mutable: false,
                                kind: SymbolKind::Function,
                            });
                        }
                    }
                }
                Decl::Protocol(p) => {
                    // Register protocol as a named type
                    let _ = self.symbols.define(Symbol {
                        name: p.name.clone(),
                        ty: ResolvedType::Struct(p.name.clone()),
                        mutable: false,
                        kind: SymbolKind::Struct,
                    });
                    // Register protocol method signatures
                    for method in &p.methods {
                        let mangled = format!("{}_{}", p.name, method.name);
                        let _ = self.symbols.define(Symbol {
                            name: mangled,
                            ty: ResolvedType::Function {
                                params: method.params.iter()
                                    .map(|param| ResolvedType::from_parser_type(&param.ty))
                                    .collect(),
                                ret: Box::new(method.return_type.as_ref()
                                    .map(ResolvedType::from_parser_type)
                                    .unwrap_or(ResolvedType::Void)),
                            },
                            mutable: false,
                            kind: SymbolKind::Function,
                        });
                    }
                }
                Decl::Extension(ext) => {
                    for method in &ext.methods {
                        self.register_function_named(&format!("{}_{}", ext.target, method.name), method);
                    }
                }
                Decl::Layer(l) => {
                    let _ = self.symbols.define(Symbol {
                        name: l.name.clone(),
                        ty: ResolvedType::Struct(l.name.clone()),
                        mutable: false,
                        kind: SymbolKind::Struct,
                    });
                    for method in &l.methods {
                        self.register_function_named(&format!("{}_{}", l.name, method.name), method);
                    }
                }
                Decl::Panel(p) => {
                    for method in &p.methods {
                        self.register_function_named(&format!("{}_{}", p.name, method.name), method);
                    }
                }
                Decl::Const(c) => {
                    let ty = c.ty.as_ref()
                        .map(ResolvedType::from_parser_type)
                        .unwrap_or(ResolvedType::Unknown);
                    let _ = self.symbols.define(Symbol {
                        name: c.name.clone(),
                        ty,
                        mutable: false,
                        kind: SymbolKind::Variable,
                    });
                }
                Decl::Typealias(t) => {
                    let resolved = ResolvedType::from_parser_type(&t.target);
                    self.type_aliases.insert(t.name.clone(), resolved.clone());
                    let _ = self.symbols.define(Symbol {
                        name: t.name.clone(),
                        ty: resolved,
                        mutable: false,
                        kind: SymbolKind::Struct,
                    });
                }
                Decl::GlobalVar(g) => {
                    let declared_type = g.ty.as_ref()
                        .map(ResolvedType::from_parser_type);
                    let inferred_type = g.init.as_ref().map(|e| self.infer_expr_type(e));
                    let final_type = declared_type
                        .or(inferred_type)
                        .unwrap_or(ResolvedType::Unknown);
                    let _ = self.symbols.define(Symbol {
                        name: g.name.clone(),
                        ty: final_type,
                        mutable: g.mutable,
                        kind: SymbolKind::Variable,
                    });
                }
            }
        }

        // Second pass: type check all function/method bodies
        for decl in &ast.declarations {
            match decl {
                Decl::Function(f) => self.check_function(f),
                Decl::Extension(ext) => {
                    for method in &ext.methods {
                        self.check_function(method);
                    }
                }
                Decl::Layer(l) => {
                    for method in &l.methods {
                        self.check_function(method);
                    }
                }
                Decl::Panel(p) => {
                    for method in &p.methods {
                        self.check_function(method);
                    }
                }
                _ => {}
            }
        }

        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(self.errors.clone())
        }
    }

    fn register_struct(&mut self, s: &StructDecl) {
        let mut fields = std::collections::HashMap::new();
        for field in &s.fields {
            let mut ty = ResolvedType::from_parser_type(&field.ty);
            if field.is_weak {
                if !self.is_reference_type(&ty) {
                    self.errors.push(TypeError::new(
                        format!(
                            "weak field '{}' requires a reference type; '{}' is a value type",
                            field.name,
                            ty.display_name()
                        ),
                        &field.span,
                    ));
                }
                if !s.is_ref {
                    self.errors.push(TypeError::new(
                        format!("weak field '{}' is only allowed in a 'ref struct'", field.name),
                        &field.span,
                    ));
                }
                // A weak slot reads as an optional: the target may be gone.
                if !matches!(ty, ResolvedType::Optional(_)) {
                    ty = ResolvedType::Optional(Box::new(ty));
                }
            }
            fields.insert(field.name.clone(), ty);
        }

        if let Err(e) = self.symbols.define_struct(StructInfo {
            name: s.name.clone(),
            fields,
            is_ref: s.is_ref,
        }) {
            self.errors.push(TypeError {
                message: e,
                span: s.span,
            });
        }
    }

    fn register_function(&mut self, f: &FnDecl) {
        let params: Vec<ResolvedType> = f.params
            .iter()
            .map(|p| ResolvedType::from_parser_type(&p.ty))
            .collect();

        let ret = f.return_type
            .as_ref()
            .map(ResolvedType::from_parser_type)
            .unwrap_or(ResolvedType::Void);

        let fn_type = ResolvedType::Function {
            params,
            ret: Box::new(ret),
        };

        if let Err(e) = self.symbols.define_function(f.name.clone(), fn_type.clone()) {
            self.errors.push(TypeError {
                message: e,
                span: f.span,
            });
        }

        // Also add to symbol table for lookup
        let _ = self.symbols.define(Symbol {
            name: f.name.clone(),
            ty: fn_type,
            mutable: false,
            kind: SymbolKind::Function,
        });
    }

    fn register_function_named(&mut self, name: &str, f: &FnDecl) {
        let params: Vec<ResolvedType> = f.params
            .iter()
            .map(|p| ResolvedType::from_parser_type(&p.ty))
            .collect();

        let ret = f.return_type
            .as_ref()
            .map(ResolvedType::from_parser_type)
            .unwrap_or(ResolvedType::Void);

        let fn_type = ResolvedType::Function {
            params,
            ret: Box::new(ret),
        };

        let _ = self.symbols.define_function(name.to_string(), fn_type.clone());
        let _ = self.symbols.define(Symbol {
            name: name.to_string(),
            ty: fn_type,
            mutable: false,
            kind: SymbolKind::Function,
        });
    }

    fn register_extern(&mut self, e: &ExternDecl) {
        let params: Vec<ResolvedType> = e.params
            .iter()
            .map(|p| ResolvedType::from_parser_type(&p.ty))
            .collect();

        let ret = e.return_type
            .as_ref()
            .map(ResolvedType::from_parser_type)
            .unwrap_or(ResolvedType::Void);

        let fn_type = ResolvedType::Function {
            params,
            ret: Box::new(ret),
        };

        if let Err(e_msg) = self.symbols.define_function(e.name.clone(), fn_type.clone()) {
            self.errors.push(TypeError {
                message: e_msg,
                span: e.span,
            });
        }

        let _ = self.symbols.define(Symbol {
            name: e.name.clone(),
            ty: fn_type,
            mutable: false,
            kind: SymbolKind::Function,
        });
    }

    fn check_function(&mut self, f: &FnDecl) {
        self.symbols.push_scope();

        // Add parameters to scope
        for param in &f.params {
            let ty = ResolvedType::from_parser_type(&param.ty);
            let _ = self.symbols.define(Symbol {
                name: param.name.clone(),
                ty,
                mutable: false,
                kind: SymbolKind::Parameter,
            });
        }

        // Set expected return type
        self.current_return_type = f.return_type
            .as_ref()
            .map(ResolvedType::from_parser_type);
        self.current_fn_throws = f.throws;
        self.current_fn_name = f.name.clone();
        if f.throws && f.name == "main" {
            self.errors.push(TypeError::new(
                "'main' cannot be declared 'throws'; catch errors with try/catch inside it",
                &f.span,
            ));
        }

        // Check function body
        self.check_block(&f.body);

        self.current_return_type = None;
        self.current_fn_throws = false;
        self.current_fn_name.clear();
        self.symbols.pop_scope();
    }

    fn check_block(&mut self, block: &Block) {
        for stmt in &block.statements {
            self.check_statement(stmt);
        }
    }

    fn check_statement(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let(l) => self.check_let(l),
            Stmt::Return(r) => self.check_return(r),
            Stmt::If(i) => self.check_if(i),
            Stmt::While(w) => self.check_while(w),
            Stmt::For(f) => self.check_for(f),
            Stmt::Expr(e) => { self.infer_expr_type(e); }
            Stmt::Block(b) => {
                self.symbols.push_scope();
                self.check_block(b);
                self.symbols.pop_scope();
            }
            Stmt::Break(_) | Stmt::Continue(_) => {}
            // Swift/C++ style statements
            Stmt::Guard(g) => {
                let cond_type = self.infer_expr_type(&g.condition);
                if cond_type != ResolvedType::Bool {
                    self.errors.push(TypeError {
                        message: format!("guard condition must be bool, found '{}'", cond_type.display_name()),
                        span: g.span,
                    });
                }
                self.symbols.push_scope();
                self.check_block(&g.else_block);
                self.symbols.pop_scope();
            }
            Stmt::Defer(d) => {
                self.symbols.push_scope();
                self.check_block(&d.body);
                self.symbols.pop_scope();
            }
            Stmt::TryCatch(t) => {
                self.symbols.push_scope();
                self.try_depth += 1;
                self.check_block(&t.try_block);
                self.try_depth -= 1;
                self.symbols.pop_scope();

                // The caught value is the `Error` existential unless the
                // catch names a concrete error type.
                let caught_ty = match &t.catch_type {
                    Some(ty) => {
                        let resolved = self.resolve_type(&ResolvedType::from_parser_type(ty));
                        if !self.conforms_to_error(&resolved) {
                            self.errors.push(TypeError::new(
                                format!(
                                    "catch pattern type '{}' is not a 'ref struct' conforming to protocol Error",
                                    resolved.display_name()
                                ),
                                &t.span,
                            ));
                        }
                        resolved
                    }
                    None => ResolvedType::Struct("Error".to_string()),
                };
                self.symbols.push_scope();
                if let Some(var) = &t.catch_var {
                    let _ = self.symbols.define(Symbol {
                        name: var.clone(),
                        ty: caught_ty,
                        mutable: false,
                        kind: SymbolKind::Variable,
                    });
                }
                self.check_block(&t.catch_block);
                self.symbols.pop_scope();
            }
            Stmt::Throw(t) => {
                let value_ty = self.infer_expr_type(&t.value);
                if !self.conforms_to_error(&value_ty) {
                    self.errors.push(TypeError::new(
                        format!(
                            "thrown value of type '{}' does not conform to protocol Error; throw a 'ref struct' declared with 'extension {}: Error {{ }}'",
                            value_ty.display_name(),
                            value_ty.display_name()
                        ),
                        &t.span,
                    ));
                }
                if !self.current_fn_throws && self.try_depth == 0 {
                    self.errors.push(TypeError::new(
                        format!(
                            "'throw' in function '{}' which is not declared 'throws' and is outside any try block",
                            self.current_fn_name
                        ),
                        &t.span,
                    ));
                }
            }
        }
    }

    fn resolve_type(&self, ty: &ResolvedType) -> ResolvedType {
        self.resolve_type_bounded(ty, 0)
    }

    /// Follow type aliases. A cycle such as `typealias A = B; typealias B = A;`
    /// resolves to `Error` instead of recursing forever.
    fn resolve_type_bounded(&self, ty: &ResolvedType, followed: usize) -> ResolvedType {
        if followed > MAX_ALIAS_CHAIN {
            return ResolvedType::Error;
        }
        match ty {
            ResolvedType::Struct(name) => {
                if let Some(resolved) = self.type_aliases.get(name) {
                    self.resolve_type_bounded(resolved, followed + 1)
                } else {
                    ty.clone()
                }
            }
            ResolvedType::Optional(inner) => {
                ResolvedType::Optional(Box::new(self.resolve_type_bounded(inner, followed)))
            }
            ResolvedType::Array(inner) => {
                ResolvedType::Array(Box::new(self.resolve_type_bounded(inner, followed)))
            }
            _ => ty.clone(),
        }
    }

    fn check_let(&mut self, l: &LetStmt) {
        let declared_type = l.ty.as_ref()
            .map(|t| self.resolve_type(&ResolvedType::from_parser_type(t)));
        
        let inferred_type = l.init.as_ref().map(|e| self.infer_expr_type(e));

        let mut final_type = match (&declared_type, &inferred_type) {
            (Some(decl), Some(infer)) => {
                if !decl.is_assignable_from(infer) {
                    self.errors.push(TypeError {
                        message: format!(
                            "type mismatch: expected '{}', found '{}'",
                            decl.display_name(),
                            infer.display_name()
                        ),
                        span: l.span,
                    });
                }
                decl.clone()
            }
            (Some(decl), None) => decl.clone(),
            (None, Some(infer)) => infer.clone(),
            (None, None) => {
                self.errors.push(TypeError {
                    message: "cannot infer type without initializer".to_string(),
                    span: l.span,
                });
                ResolvedType::Error
            }
        };

        if l.is_weak {
            if !self.is_reference_type(&final_type) {
                self.errors.push(TypeError::new(
                    format!(
                        "weak binding '{}' requires a reference type; '{}' is a value type",
                        l.name,
                        final_type.display_name()
                    ),
                    &l.span,
                ));
            }
            // A weak slot reads as an optional: the target may be gone.
            if !matches!(final_type, ResolvedType::Optional(_) | ResolvedType::Error) {
                final_type = ResolvedType::Optional(Box::new(final_type));
            }
        }

        if let Err(e) = self.symbols.define(Symbol {
            name: l.name.clone(),
            ty: final_type,
            mutable: l.mutable,
            kind: SymbolKind::Variable,
        }) {
            self.errors.push(TypeError {
                message: e,
                span: l.span,
            });
        }
    }

    fn check_return(&mut self, r: &ReturnStmt) {
        let return_type = r.value.as_ref()
            .map(|e| self.infer_expr_type(e))
            .unwrap_or(ResolvedType::Void);

        if let Some(expected) = &self.current_return_type {
            if !expected.is_assignable_from(&return_type) {
                self.errors.push(TypeError {
                    message: format!(
                        "return type mismatch: expected '{}', found '{}'",
                        expected.display_name(),
                        return_type.display_name()
                    ),
                    span: r.span,
                });
            }
        }
    }

    fn check_if(&mut self, i: &IfStmt) {
        let cond_type = self.infer_expr_type(&i.condition);
        let mut bound: Option<(String, ResolvedType)> = None;
        if let Some(name) = &i.binding {
            // `if let x = expr`: expr must be an optional (or a nullable
            // reference); x gets the unwrapped type inside the then block.
            let unwrapped = match self.resolve_type(&cond_type) {
                ResolvedType::Optional(inner) => *inner,
                other if self.is_reference_type(&other) => other,
                ResolvedType::Error | ResolvedType::Unknown => ResolvedType::Unknown,
                other => {
                    self.errors.push(TypeError {
                        message: format!(
                            "'if let' requires an optional or reference value, found '{}'",
                            other.display_name()
                        ),
                        span: i.span,
                    });
                    ResolvedType::Error
                }
            };
            bound = Some((name.clone(), unwrapped));
        } else if cond_type != ResolvedType::Bool {
            self.errors.push(TypeError {
                message: format!(
                    "if condition must be bool, found '{}'",
                    cond_type.display_name()
                ),
                span: i.span,
            });
        }

        self.symbols.push_scope();
        if let Some((name, ty)) = bound {
            let _ = self.symbols.define(Symbol { name, ty, mutable: false, kind: SymbolKind::Variable });
        }
        self.check_block(&i.then_block);
        self.symbols.pop_scope();

        if let Some(else_block) = &i.else_block {
            self.symbols.push_scope();
            self.check_block(else_block);
            self.symbols.pop_scope();
        }
    }

    fn check_while(&mut self, w: &WhileStmt) {
        let cond_type = self.infer_expr_type(&w.condition);
        if cond_type != ResolvedType::Bool {
            self.errors.push(TypeError {
                message: format!(
                    "while condition must be bool, found '{}'",
                    cond_type.display_name()
                ),
                span: w.span,
            });
        }

        self.symbols.push_scope();
        self.check_block(&w.body);
        self.symbols.pop_scope();
    }

    fn check_for(&mut self, f: &ForStmt) {
        let iter_type = self.infer_expr_type(&f.iterable);
        
        let elem_type = match iter_type {
            ResolvedType::Array(inner) => *inner,
            ResolvedType::Int => ResolvedType::Int, // For range-like iteration
            _ => {
                self.errors.push(TypeError {
                    message: format!(
                        "cannot iterate over '{}'",
                        iter_type.display_name()
                    ),
                    span: f.span,
                });
                ResolvedType::Error
            }
        };

        self.symbols.push_scope();
        let _ = self.symbols.define(Symbol {
            name: f.var.clone(),
            ty: elem_type,
            mutable: false,
            kind: SymbolKind::Variable,
        });
        self.check_block(&f.body);
        self.symbols.pop_scope();
    }

    /// Infer the type of an expression
    fn infer_expr_type(&mut self, expr: &Expr) -> ResolvedType {
        if self.expr_depth >= MAX_EXPR_DEPTH {
            self.errors.push(TypeError::new(
                format!("expression nested deeper than {} levels", MAX_EXPR_DEPTH),
                &expr.span(),
            ));
            return ResolvedType::Error;
        }
        self.expr_depth += 1;
        let ty = self.infer_expr_type_inner(expr);
        self.expr_depth -= 1;
        ty
    }

    fn infer_expr_type_inner(&mut self, expr: &Expr) -> ResolvedType {
        match expr {
            Expr::Literal(lit) => self.infer_literal_type(lit),
            Expr::Identifier(name, span) => {
                if let Some(sym) = self.symbols.lookup(name) {
                    sym.ty.clone()
                } else {
                    self.errors.push(TypeError::new(
                        format!("undefined variable '{}'", name),
                        span,
                    ));
                    ResolvedType::Error
                }
            }
            Expr::Binary(left, op, right, span) => {
                let left_ty = self.infer_expr_type(left);
                let right_ty = self.infer_expr_type(right);
                self.check_binary_op(&left_ty, op, &right_ty, span)
            }
            Expr::Unary(op, operand, span) => {
                let operand_ty = self.infer_expr_type(operand);
                self.check_unary_op(op, &operand_ty, span)
            }
            Expr::Call(callee, args, span) => {
                let under_try = std::mem::replace(&mut self.in_try_expr, false);
                if let Expr::Identifier(name, _) = callee.as_ref() {
                    if self.is_builtin(name) {
                        // Natives are variadic and dynamically typed; still check the arguments.
                        for arg in args {
                            self.infer_expr_type(arg);
                        }
                        return ResolvedType::Unknown;
                    }
                    let throws = self.throwing_fns.contains(name) && self.symbols.lookup(name).is_some_and(|s| s.kind == SymbolKind::Function);
                    if throws && !under_try {
                        self.errors.push(TypeError::new(
                            format!("call to throwing function '{}' must be marked with 'try'", name),
                            span,
                        ));
                    }
                    if !throws && under_try {
                        self.errors.push(TypeError::new(
                            format!("'try' applied to a call of '{}', which does not throw", name),
                            span,
                        ));
                    }
                } else if under_try {
                    self.errors.push(TypeError::new("'try' must be applied directly to a call of a throwing function", span));
                }
                let callee_ty = self.infer_expr_type(callee);
                self.check_call(&callee_ty, args, span)
            }
            Expr::Try(inner, span) => {
                if !self.current_fn_throws && self.try_depth == 0 {
                    self.errors.push(TypeError::new(
                        format!(
                            "'try' in function '{}' which is not declared 'throws' and is outside any try block",
                            self.current_fn_name
                        ),
                        span,
                    ));
                }
                if !matches!(inner.as_ref(), Expr::Call(_, _, _)) {
                    self.errors.push(TypeError::new("'try' must be applied directly to a call of a throwing function", span));
                    return self.infer_expr_type(inner);
                }
                self.in_try_expr = true;
                let ty = self.infer_expr_type(inner);
                self.in_try_expr = false;
                ty
            }
            Expr::Member(obj, field, span) => {
                let obj_ty = self.infer_expr_type(obj);
                self.check_member_access(&obj_ty, field, span)
            }
            Expr::Index(arr, idx, span) => {
                let arr_ty = self.infer_expr_type(arr);
                let idx_ty = self.infer_expr_type(idx);
                self.check_index(&arr_ty, &idx_ty, span)
            }
            Expr::Assign(target, value, span) => {
                let target_ty = self.infer_expr_type(target);
                let value_ty = self.infer_expr_type(value);
                // Mutability check: assignment to immutable variable is an error
                if let Expr::Identifier(name, ident_span) = target.as_ref() {
                    if let Some(sym) = self.symbols.lookup(name) {
                        if !sym.mutable {
                            self.errors.push(TypeError::new(
                                format!("cannot assign to immutable variable '{}'; declare it with 'let mut'", name),
                                ident_span,
                            ));
                        }
                    }
                } else if let Some((root, root_span)) = Self::assignment_root(target) {
                    // Mutation through a field or element of an immutable
                    // binding is an error as well, reference types included.
                    if let Some(sym) = self.symbols.lookup(root) {
                        if !sym.mutable && matches!(sym.kind, SymbolKind::Variable | SymbolKind::Parameter) {
                            self.errors.push(TypeError::new(
                                format!("cannot mutate through immutable binding '{}'; declare it with 'let mut'", root),
                                root_span,
                            ));
                        }
                    }
                }
                if !target_ty.is_assignable_from(&value_ty) {
                    self.errors.push(TypeError::new(
                        format!(
                            "cannot assign '{}' to '{}'",
                            value_ty.display_name(),
                            target_ty.display_name()
                        ),
                        span,
                    ));
                }
                target_ty
            }
            Expr::StructLit(name, fields, span) => {
                if let Some(struct_info) = self.symbols.lookup_struct(name) {
                    // Clone fields to avoid borrow conflict
                    let expected_fields = struct_info.fields.clone();
                    
                    // Check all fields are provided with correct types
                    for (field_name, value) in fields {
                        let value_ty = self.infer_expr_type(value);
                        if let Some(expected_ty) = expected_fields.get(field_name) {
                            if !expected_ty.is_assignable_from(&value_ty) {
                                self.errors.push(TypeError::new(
                                    format!(
                                        "field '{}' expects '{}', found '{}'",
                                        field_name,
                                        expected_ty.display_name(),
                                        value_ty.display_name()
                                    ),
                                    span,
                                ));
                            }
                        } else {
                            self.errors.push(TypeError::new(
                                format!("struct '{}' has no field '{}'", name, field_name),
                                span,
                            ));
                        }
                    }
                    ResolvedType::Struct(name.clone())
                } else {
                    self.errors.push(TypeError::new(
                        format!("undefined struct '{}'", name),
                        span,
                    ));
                    ResolvedType::Error
                }
            }
            Expr::ArrayLit(elements, _) => {
                if elements.is_empty() {
                    ResolvedType::Array(Box::new(ResolvedType::Unknown))
                } else {
                    let elem_ty = self.infer_expr_type(&elements[0]);
                    ResolvedType::Array(Box::new(elem_ty))
                }
            }
            Expr::Match(scrutinee, arms, span) => {
                // Infer the type being matched on
                let _scrutinee_ty = self.infer_expr_type(scrutinee);
                
                if arms.is_empty() {
                    return ResolvedType::Void;
                }
                
                // Infer types from all arm bodies and find common type
                let mut common_type: Option<ResolvedType> = None;
                
                for arm in arms {
                    let arm_ty = self.infer_expr_type(&arm.body);
                    
                    match &common_type {
                        None => {
                            common_type = Some(arm_ty);
                        }
                        Some(existing) => {
                            // Check if types are compatible
                            if !existing.is_assignable_from(&arm_ty) && !arm_ty.is_assignable_from(existing) {
                                self.errors.push(TypeError::new(
                                    format!(
                                        "match arms have incompatible types: '{}' and '{}'",
                                        existing.display_name(),
                                        arm_ty.display_name()
                                    ),
                                    span,
                                ));
                            }
                            // Use the more general type if one is assignable from the other
                            if arm_ty.is_assignable_from(existing) {
                                common_type = Some(arm_ty);
                            }
                        }
                    }
                }
                
                common_type.unwrap_or(ResolvedType::Void)
            }
            // Swift/C++ style expressions
            Expr::CompoundAssign(target, _op, value, span) => {
                let target_ty = self.infer_expr_type(target);
                let value_ty = self.infer_expr_type(value);
                if !target_ty.is_assignable_from(&value_ty) {
                    self.errors.push(TypeError::new(
                        format!("cannot compound assign '{}' to '{}'", value_ty.display_name(), target_ty.display_name()),
                        span,
                    ));
                }
                target_ty
            }
            Expr::PreIncrement(operand, span) | Expr::PreDecrement(operand, span) |  
            Expr::PostIncrement(operand, span) | Expr::PostDecrement(operand, span) => {
                let ty = self.infer_expr_type(operand);
                if !matches!(ty, ResolvedType::Int | ResolvedType::Float) {
                    self.errors.push(TypeError::new(
                        format!("cannot increment/decrement '{}'", ty.display_name()),
                        span,
                    ));
                }
                ty
            }
            Expr::NullCoalesce(left, right, span) => {
                let left_ty = self.infer_expr_type(left);
                let right_ty = self.infer_expr_type(right);
                // `a ?? b` has the unwrapped type of a; b must fit it.
                let unwrapped = match self.resolve_type(&left_ty) {
                    ResolvedType::Optional(inner) => *inner,
                    other => other,
                };
                if !unwrapped.is_assignable_from(&right_ty) {
                    self.errors.push(TypeError::new(
                        format!(
                            "'??' fallback has type '{}', expected '{}'",
                            right_ty.display_name(),
                            unwrapped.display_name()
                        ),
                        span,
                    ));
                }
                unwrapped
            }
            Expr::OptionalChain(obj, field, span) => {
                let obj_ty = self.infer_expr_type(obj);
                let inner = match self.resolve_type(&obj_ty) {
                    ResolvedType::Optional(inner) => *inner,
                    other => other,
                };
                let field_ty = self.check_member_access(&inner, field, span);
                match field_ty {
                    ResolvedType::Optional(_) | ResolvedType::Error | ResolvedType::Unknown => field_ty,
                    other => ResolvedType::Optional(Box::new(other)),
                }
            }
            Expr::TrailingClosure(callee, body, span) => {
                let callee_ty = self.infer_expr_type(callee);
                self.check_block(body);
                // Return the callee's return type if it's a function
                match callee_ty {
                    ResolvedType::Function { ret, .. } => *ret,
                    _ => ResolvedType::Void
                }
            }
            Expr::Nil(_) => ResolvedType::Unknown,
            Expr::Await(operand, _) => {
                // Await unwraps the async return type - for now return the inner type
                let operand_ty = self.infer_expr_type(operand);
                // If it's a function call, return the function's return type
                // Otherwise return the operand type itself
                operand_ty
            }
            Expr::Range(start, end, span) => {
                let start_ty = self.infer_expr_type(start);
                let end_ty = self.infer_expr_type(end);
                
                if start_ty != ResolvedType::Int || end_ty != ResolvedType::Int {
                    self.errors.push(TypeError::new(
                        format!(
                            "range bounds must be int, found '{}' and '{}'",
                            start_ty.display_name(),
                            end_ty.display_name()
                        ),
                        span,
                    ));
                }
                // Range produces an array of integers
                ResolvedType::Array(Box::new(ResolvedType::Int))
            }
            Expr::Action(params, return_type, body, _span) => {
                let param_types: Vec<ResolvedType> = params.iter()
                    .map(|p| ResolvedType::from_parser_type(&p.ty))
                    .collect();
                let ret = return_type.as_ref()
                    .map(|t| ResolvedType::from_parser_type(t))
                    .unwrap_or(ResolvedType::Void);
                self.symbols.push_scope();
                self.check_block(body);
                self.symbols.pop_scope();
                ResolvedType::Function {
                    params: param_types,
                    ret: Box::new(ret),
                }
            }
        }
    }

    fn infer_literal_type(&self, lit: &Literal) -> ResolvedType {
        match lit {
            Literal::Int(_, _) => ResolvedType::Int,
            Literal::Float(_, _) => ResolvedType::Float,
            Literal::String(_, _) => ResolvedType::String,
            Literal::Bool(_, _) => ResolvedType::Bool,
        }
    }

    fn check_binary_op(&mut self, left: &ResolvedType, op: &BinOp, right: &ResolvedType, span: &Span) -> ResolvedType {
        // A runtime native's result is dynamically typed: let it combine with
        // anything and take the other operand's type.
        if matches!(left, ResolvedType::Unknown) || matches!(right, ResolvedType::Unknown) {
            return match op {
                BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge | BinOp::And | BinOp::Or => ResolvedType::Bool,
                _ if matches!(left, ResolvedType::Unknown) => right.clone(),
                _ => left.clone(),
            };
        }
        match op {
            // Arithmetic operators
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => {
                match (left, right) {
                    (ResolvedType::Int, ResolvedType::Int) => ResolvedType::Int,
                    (ResolvedType::Float, ResolvedType::Float) => ResolvedType::Float,
                    (ResolvedType::Float, ResolvedType::Int) => ResolvedType::Float,
                    (ResolvedType::Int, ResolvedType::Float) => ResolvedType::Float,
                    (ResolvedType::String, ResolvedType::String) if *op == BinOp::Add => {
                        ResolvedType::String // String concatenation
                    }
                    _ => {
                        self.errors.push(TypeError::new(
                            format!(
                                "cannot apply operator to '{}' and '{}'",
                                left.display_name(),
                                right.display_name()
                            ),
                            span,
                        ));
                        ResolvedType::Error
                    }
                }
            }
            // Comparison operators
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => {
                let nil_check = matches!(op, BinOp::Eq | BinOp::Ne)
                    && (matches!(left, ResolvedType::Unknown) || matches!(right, ResolvedType::Unknown));
                if nil_check || left == right || 
                   (matches!(left, ResolvedType::Int | ResolvedType::Float) && 
                    matches!(right, ResolvedType::Int | ResolvedType::Float)) {
                    ResolvedType::Bool
                } else {
                    self.errors.push(TypeError::new(
                        format!(
                            "cannot compare '{}' and '{}'",
                            left.display_name(),
                            right.display_name()
                        ),
                        span,
                    ));
                    ResolvedType::Error
                }
            }
            // Logical operators
            BinOp::And | BinOp::Or => {
                if *left == ResolvedType::Bool && *right == ResolvedType::Bool {
                    ResolvedType::Bool
                } else {
                    self.errors.push(TypeError::new(
                        format!(
                            "logical operators require bool operands, found '{}' and '{}'",
                            left.display_name(),
                            right.display_name()
                        ),
                        span,
                    ));
                    ResolvedType::Error
                }
            }
            // Bitwise operators
            BinOp::BitwiseAnd | BinOp::BitwiseOr | BinOp::BitwiseXor | BinOp::ShiftLeft | BinOp::ShiftRight => {
                match (left, right) {
                    (ResolvedType::Int, ResolvedType::Int) => ResolvedType::Int,
                    _ => {
                        self.errors.push(TypeError::new(
                            format!(
                                "bitwise operators require int operands, found '{}' and '{}'",
                                left.display_name(),
                                right.display_name()
                            ),
                            span,
                        ));
                        ResolvedType::Error
                    }
                }
            }
        }
    }

    fn check_unary_op(&mut self, op: &UnaryOp, operand: &ResolvedType, span: &Span) -> ResolvedType {
        match op {
            UnaryOp::Neg => {
                if matches!(operand, ResolvedType::Int | ResolvedType::Float) {
                    operand.clone()
                } else {
                    self.errors.push(TypeError::new(
                        format!("cannot negate '{}'", operand.display_name()),
                        span,
                    ));
                    ResolvedType::Error
                }
            }
            UnaryOp::Not => {
                if *operand == ResolvedType::Bool {
                    ResolvedType::Bool
                } else {
                    self.errors.push(TypeError::new(
                        format!("cannot apply '!' to '{}'", operand.display_name()),
                        span,
                    ));
                    ResolvedType::Error
                }
            }
            UnaryOp::BitwiseNot => {
                if *operand == ResolvedType::Int {
                    ResolvedType::Int
                } else {
                    self.errors.push(TypeError::new(
                        format!("cannot apply '~' to '{}'", operand.display_name()),
                        span,
                    ));
                    ResolvedType::Error
                }
            }
        }
    }

    fn check_call(&mut self, callee: &ResolvedType, args: &[Expr], span: &Span) -> ResolvedType {
        match callee {
            ResolvedType::Function { params, ret } => {
                if args.len() != params.len() {
                    self.errors.push(TypeError::new(
                        format!(
                            "expected {} arguments, found {}",
                            params.len(),
                            args.len()
                        ),
                        span,
                    ));
                    return *ret.clone();
                }

                for (i, (arg, param)) in args.iter().zip(params.iter()).enumerate() {
                    let arg_ty = self.infer_expr_type(arg);
                    if !param.is_assignable_from(&arg_ty) {
                        self.errors.push(TypeError::new(
                            format!(
                                "argument {} type mismatch: expected '{}', found '{}'",
                                i + 1,
                                param.display_name(),
                                arg_ty.display_name()
                            ),
                            span,
                        ));
                    }
                }

                *ret.clone()
            }
            ResolvedType::Error => ResolvedType::Error,
            _ => {
                self.errors.push(TypeError::new(
                    format!("'{}' is not callable", callee.display_name()),
                    span,
                ));
                ResolvedType::Error
            }
        }
    }

    fn check_member_access(&mut self, obj: &ResolvedType, field: &str, span: &Span) -> ResolvedType {
        match obj {
            ResolvedType::Struct(name) => {
                if let Some(struct_info) = self.symbols.lookup_struct(name) {
                    if let Some(field_ty) = struct_info.fields.get(field) {
                        field_ty.clone()
                    } else {
                        self.errors.push(TypeError::new(
                            format!("struct '{}' has no field '{}'", name, field),
                            span,
                        ));
                        ResolvedType::Error
                    }
                } else {
                    self.errors.push(TypeError::new(
                        format!("undefined struct '{}'", name),
                        span,
                    ));
                    ResolvedType::Error
                }
            }
            ResolvedType::Optional(inner) => {
                self.errors.push(TypeError::new(
                    format!(
                        "cannot access field '{}' on optional '{}'; unwrap it with 'if let', '?.' or '??'",
                        field,
                        obj.display_name()
                    ),
                    span,
                ));
                let _ = inner;
                ResolvedType::Error
            }
            ResolvedType::Error => ResolvedType::Error,
            _ => {
                self.errors.push(TypeError::new(
                    format!("cannot access field '{}' on '{}'", field, obj.display_name()),
                    span,
                ));
                ResolvedType::Error
            }
        }
    }

    fn check_index(&mut self, arr: &ResolvedType, idx: &ResolvedType, span: &Span) -> ResolvedType {
        if *idx != ResolvedType::Int {
            self.errors.push(TypeError::new(
                format!("array index must be int, found '{}'", idx.display_name()),
                span,
            ));
        }

        match arr {
            ResolvedType::Array(inner) => *inner.clone(),
            ResolvedType::String => ResolvedType::String, // String indexing returns char/string
            ResolvedType::Error => ResolvedType::Error,
            _ => {
                self.errors.push(TypeError::new(
                    format!("cannot index into '{}'", arr.display_name()),
                    span,
                ));
                ResolvedType::Error
            }
        }
    }
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}

/// Type check the AST. On success the (unchanged) AST is returned; every
/// type error found is returned together, none is printed.
pub fn check(ast: &Ast) -> Result<Ast, Vec<TypeError>> {
    // The checker recurses over the expression tree, so run it on a thread
    // with a stack large enough for `MAX_EXPR_DEPTH` levels in debug builds.
    let outcome = std::thread::scope(|scope| {
        let worker = std::thread::Builder::new()
            .name("reox-typecheck".to_string())
            .stack_size(CHECK_STACK_BYTES)
            .spawn_scoped(scope, || TypeChecker::new().check_program(ast));
        match worker {
            Ok(handle) => handle.join().unwrap_or_else(|_| {
                Err(vec![TypeError::new("type checker thread terminated unexpectedly", &Span::default())])
            }),
            Err(_) => TypeChecker::new().check_program(ast),
        }
    });
    outcome?;
    Ok(ast.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;
    use crate::parser::parse;

    fn check_source(source: &str) -> Result<(), Vec<TypeError>> {
        let tokens = tokenize(source).unwrap();
        let ast = parse(&tokens).unwrap();
        let mut checker = TypeChecker::new();
        checker.check_program(&ast)
    }

    #[test]
    fn test_valid_function() {
        let result = check_source(r#"
            fn add(a: int, b: int) -> int {
                return a + b;
            }
        "#);
        assert!(result.is_ok());
    }

    #[test]
    fn test_undefined_variable() {
        let result = check_source(r#"
            fn main() {
                let x: int = y;
            }
        "#);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.message.contains("undefined")));
    }

    #[test]
    fn test_type_mismatch() {
        let result = check_source(r#"
            fn main() {
                let x: int = "hello";
            }
        "#);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.message.contains("type mismatch")));
    }

    #[test]
    fn test_return_type_mismatch() {
        let result = check_source(r#"
            fn foo() -> int {
                return "hello";
            }
        "#);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.message.contains("return type")));
    }

    #[test]
    fn test_struct_field_access() {
        let result = check_source(r#"
            struct Point { x: int, y: int }
            fn main() {
                let p: Point = Point { x: 1, y: 2 };
                let a: int = p.x;
            }
        "#);
        assert!(result.is_ok());
    }

    #[test]
    fn test_invalid_field_access() {
        let result = check_source(r#"
            struct Point { x: int, y: int }
            fn main() {
                let p: Point = Point { x: 1, y: 2 };
                let a: int = p.z;
            }
        "#);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.message.contains("no field")));
    }

    #[test]
    fn test_function_call_type_check() {
        let result = check_source(r#"
            fn add(a: int, b: int) -> int {
                return a + b;
            }
            fn main() {
                let x: int = add(1, 2);
            }
        "#);
        assert!(result.is_ok());
    }

    #[test]
    fn test_function_wrong_arg_count() {
        let result = check_source(r#"
            fn add(a: int, b: int) -> int {
                return a + b;
            }
            fn main() {
                let x: int = add(1);
            }
        "#);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.message.contains("arguments")));
    }

    // === Hardening: error paths ===

    #[test]
    fn check_returns_every_error_instead_of_printing() {
        let tokens = tokenize("fn main() { let a: int = \"x\"; let b: bool = 1; }").unwrap();
        let ast = parse(&tokens).unwrap();
        let errors = check(&ast).unwrap_err();
        assert_eq!(errors.len(), 2);
        assert_eq!((errors[0].line(), errors[0].column()), (1, 13));
        assert!(errors[0].display().starts_with("type error[1:13]: type mismatch"));
    }

    #[test]
    fn check_returns_ast_on_success() {
        let tokens = tokenize("fn main() -> int { return 1; }").unwrap();
        let ast = parse(&tokens).unwrap();
        assert_eq!(check(&ast).unwrap().declarations.len(), 1);
    }

    #[test]
    fn type_alias_cycle_does_not_recurse_forever() {
        let result = check_source("typealias A = B; typealias B = A; fn main() { let x: A = 1; }");
        // Whatever the verdict, the checker must terminate and keep its spans.
        if let Err(errors) = result {
            for e in errors { assert!(e.span.line >= 1); }
        }
    }

    #[test]
    fn deep_expression_tree_is_reported_not_overflowed() {
        // Build a left-deep tree by hand, deeper than the checker's limit.
        let span = Span::default();
        let mut expr = Expr::Literal(Literal::Int(1, span));
        for _ in 0..(MAX_EXPR_DEPTH + 10) {
            expr = Expr::Binary(Box::new(expr), BinOp::Add, Box::new(Expr::Literal(Literal::Int(1, span))), span);
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
        let errors = check(&ast).unwrap_err();
        assert!(errors.iter().any(|e| e.message.contains("nested deeper than")));
    }

    #[test]
    fn runtime_natives_are_known_to_the_checker() {
        let ok = check_source(r#"
            fn main() {
                println("a", 1);
                let n: int = len([1, 2]);
                let m = map_set(map_new(), "k", 1);
                print();
            }
        "#);
        assert!(ok.is_ok(), "{:?}", ok.err());
    }

    #[test]
    fn user_declarations_shadow_natives() {
        // An extern or fn named like a native replaces it, with its own arity.
        let ok = check_source("extern fn print(s: string); fn main() { print(\"x\"); }");
        assert!(ok.is_ok(), "{:?}", ok.err());
        let errors = check_source("fn len(a: int) -> int { return a; } fn main() { len(1, 2); }").unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.starts_with("expected 1 arguments, found 2"));
    }

    #[test]
    fn native_arguments_are_still_checked() {
        let errors = check_source("fn main() { println(undefined_thing); }").unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("undefined variable 'undefined_thing'"));
    }
    // === ARC and typed errors ===

    fn first_error(source: &str) -> String {
        check_source(source).unwrap_err()[0].message.clone()
    }

    #[test]
    fn weak_requires_a_reference_type() {
        let msg = first_error("fn main() { weak let n = 1; }");
        assert!(msg.contains("weak binding 'n' requires a reference type"), "{}", msg);
        let msg = first_error("ref struct N { weak count: int }");
        assert!(msg.contains("weak field 'count' requires a reference type"), "{}", msg);
        let msg = first_error("struct V { weak s: string }");
        assert!(msg.contains("only allowed in a 'ref struct'"), "{}", msg);
        assert!(check_source("ref struct N { weak next: N } fn main() { let n = N { }; weak let w = n; }").is_ok());
    }

    #[test]
    fn weak_slots_read_as_optional() {
        let msg = first_error(r#"
            ref struct N { value: int, weak parent: N }
            fn main() { let n = N { value: 1 }; let v: int = n.parent.value; }
        "#);
        assert!(msg.contains("cannot access field 'value' on optional 'N?'"), "{}", msg);
        assert!(check_source(r#"
            ref struct N { value: int, weak parent: N }
            fn main() {
                let n = N { value: 1 };
                if let p = n.parent { let v: int = p.value; }
                let w: int = n.parent?.value ?? 0;
                let mut q = N { value: 2 };
                q.parent = n;
            }
        "#).is_ok());
    }

    #[test]
    fn mutation_through_immutable_binding_is_an_error() {
        let msg = first_error("ref struct N { value: int } fn main() { let n = N { value: 1 }; n.value = 2; }");
        assert!(msg.contains("cannot mutate through immutable binding 'n'"), "{}", msg);
        let msg = first_error("fn main() { let a: [int] = [1]; a[0] = 2; }");
        assert!(msg.contains("cannot mutate through immutable binding 'a'"), "{}", msg);
        assert!(check_source("ref struct N { value: int } fn main() { let mut n = N { value: 1 }; n.value = 2; }").is_ok());
    }

    #[test]
    fn throw_requires_an_error_type_and_a_throwing_context() {
        let msg = first_error("fn f() throws { throw 1; }");
        assert!(msg.contains("does not conform to protocol Error"), "{}", msg);
        let msg = first_error("ref struct E { } fn f() throws { throw E { }; }");
        assert!(msg.contains("does not conform to protocol Error"), "{}", msg);
        let msg = first_error("ref struct E { } extension E: Error { } fn f() { throw E { }; }");
        assert!(msg.contains("'throw' in function 'f' which is not declared 'throws'"), "{}", msg);
        assert!(check_source("ref struct E { } extension E: Error { } fn f() throws { throw E { }; }").is_ok());
        assert!(check_source("ref struct E { } extension E: Error { } fn f() { try { throw E { }; } catch e { } }").is_ok());
    }

    #[test]
    fn throwing_calls_need_try_and_vice_versa() {
        let base = "ref struct E { } extension E: Error { } fn risky() throws -> int { return 1; } fn safe() -> int { return 2; }";
        let msg = first_error(&format!("{} fn main() {{ let v = risky(); }}", base));
        assert!(msg.contains("call to throwing function 'risky' must be marked with 'try'"), "{}", msg);
        let msg = first_error(&format!("{} fn main() {{ try {{ let v = try safe(); }} catch e {{ }} }}", base));
        assert!(msg.contains("'try' applied to a call of 'safe', which does not throw"), "{}", msg);
        let msg = first_error(&format!("{} fn main() {{ let v = try risky(); }}", base));
        assert!(msg.contains("'try' in function 'main' which is not declared 'throws'"), "{}", msg);
        assert!(check_source(&format!("{} fn g() throws -> int {{ return try risky(); }}", base)).is_ok());
        assert!(check_source(&format!("{} fn main() {{ try {{ let v = try risky(); }} catch e {{ }} }}", base)).is_ok());
    }

    #[test]
    fn catch_patterns_bind_typed_errors() {
        let base = "ref struct E { code: int } extension E: Error { } fn risky() throws { throw E { code: 1 }; }";
        assert!(check_source(&format!("{} fn main() {{ try {{ try risky(); }} catch e as E {{ let c: int = e.code; }} }}", base)).is_ok());
        let msg = first_error(&format!("{} fn main() {{ try {{ try risky(); }} catch e {{ let c: int = e.code; }} }}", base));
        assert!(msg.contains("no field 'code'") || msg.contains("undefined struct 'Error'"), "{}", msg);
        let msg = first_error(&format!("{} fn main() {{ try {{ try risky(); }} catch e as int {{ }} }}", base));
        assert!(msg.contains("catch pattern type 'int' is not a 'ref struct'"), "{}", msg);
    }

    #[test]
    fn main_cannot_throw() {
        let msg = first_error("fn main() throws { }");
        assert!(msg.contains("'main' cannot be declared 'throws'"), "{}", msg);
    }

    #[test]
    fn map_types_and_optional_suffix_parse_and_check() {
        assert!(check_source("fn f(m: [string: int], s: string?) -> int? { return nil; }").is_ok());
        let msg = first_error("fn main() { let m: [string: int] = 1; }");
        assert!(msg.contains("expected '[string: int]'") || msg.contains("Map<string, int>"), "{}", msg);
    }
}
