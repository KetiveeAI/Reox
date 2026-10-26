// REOX Compiler - Type Checker
// Validates types and builds symbol table
// Zero external dependencies

#![allow(dead_code, unused_imports, unused_variables)]

mod lower;
mod types;

pub use types::*;

use std::collections::{HashMap, HashSet};

use crate::parser::{
    Ast, Decl, Stmt, Expr, Literal, BinOp, UnaryOp,
    FnDecl, StructDecl, ExternDecl, Block, Type, LetStmt,
    ReturnStmt, IfStmt, WhileStmt, ForStmt, GuardStmt, DeferStmt,
    TryCatchStmt, ThrowStmt, CompoundOp,
    VariantDecl, ProtocolDecl, ExtensionDecl, LayerDecl, PanelDecl, ConstDecl,
};
use crate::lexer::Span;
use lower::{Lowerer, Resolution, Resolutions, SiteKey, SiteKind};

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

/// The method body being checked, when it has an implicit `self`.
#[derive(Debug, Clone)]
struct SelfContext {
    type_name: String,
    is_mut: bool,
    method: String,
}

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
    /// Every nominal type by name and the declaration form that introduced it.
    type_kinds: HashMap<String, TypeKind>,
    /// Methods per nominal type, from struct bodies, extensions, layers and panels.
    methods: HashMap<String, HashMap<String, MethodInfo>>,
    /// `self` of the method body being checked.
    current_self: Option<SelfContext>,
    /// Resolutions recorded while checking the current body; consumed by lowering.
    resolutions: Resolutions,
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
            type_kinds: HashMap::new(),
            methods: HashMap::new(),
            current_self: None,
            resolutions: Resolutions::new(),
        }
    }

    fn record(&mut self, kind: SiteKind, span: Span, resolution: Resolution) {
        self.resolutions.insert(SiteKey::new(kind, span), resolution);
    }

    /// Whether `name` denotes a nominal type at this point (and no variable
    /// of that name shadows it).
    fn is_type_name(&self, name: &str) -> bool {
        self.type_kinds.contains_key(name)
            && !matches!(self.symbols.lookup(name), Some(sym) if sym.kind != SymbolKind::Struct)
    }

    /// Check that `target` may be assigned to: its root must be a mutable
    /// variable, or `self` inside a `mut` method.
    fn check_assignable(&mut self, target: &Expr) {
        match target {
            Expr::Identifier(name, span) => {
                if let Some(sym) = self.symbols.lookup(name) {
                    if !sym.mutable {
                        let what = if sym.kind == SymbolKind::Parameter { "parameter" } else { "immutable variable" };
                        self.errors.push(TypeError::new(
                            format!("cannot assign to {} '{}'; declare it with 'let mut'", what, name),
                            span,
                        ));
                    }
                }
            }
            Expr::SelfRef(span) => {
                let ctx = self.current_self.clone();
                match ctx {
                    Some(ctx) if !ctx.is_mut => self.errors.push(TypeError::new(
                        format!(
                            "cannot assign through 'self' in method '{}' of '{}'; declare it with 'mut fn'",
                            ctx.method, ctx.type_name
                        ),
                        span,
                    )),
                    _ => {}
                }
            }
            Expr::Member(obj, _, _) | Expr::OptionalChain(obj, _, _) | Expr::Index(obj, _, _) => {
                self.check_assignable(obj)
            }
            other => self.errors.push(TypeError::new("invalid assignment target", &other.span())),
        }
    }

    /// Whether `expr` denotes a place in memory (a variable, field, element or `self`).
    fn is_place(expr: &Expr) -> bool {
        match expr {
            Expr::Identifier(_, _) | Expr::SelfRef(_) => true,
            Expr::Member(obj, _, _) | Expr::Index(obj, _, _) => Self::is_place(obj),
            _ => false,
        }
    }

    /// Check a call `receiver.method(args)` or `Type.method(args)` and
    /// record how it dispatches. Returns `None` when `receiver.method` is not
    /// a method, so the caller falls back to treating it as a field access.
    fn check_method_call(&mut self, receiver: &Expr, method: &str, args: &[Expr], call_span: &Span, member_span: &Span) -> Option<ResolvedType> {
        // Static call on a type name: Type.method(args)
        if let Expr::Identifier(type_name, _) = receiver {
            if self.is_type_name(type_name) {
                let info = self.lookup_method(type_name, method).cloned();
                return Some(match info {
                    Some(info) if info.is_static => {
                        self.check_args(&info.params, args, call_span);
                        self.record(SiteKind::Call, *call_span, Resolution::StaticCall {
                            mangled: Self::mangle_method(type_name, method),
                        });
                        info.ret
                    }
                    Some(_) => {
                        self.errors.push(TypeError::new(
                            format!("method '{}' of '{}' needs a receiver; call it on a value of type '{}'", method, type_name, type_name),
                            member_span,
                        ));
                        ResolvedType::Error
                    }
                    None => {
                        self.errors.push(TypeError::new(
                            format!("type '{}' has no static method '{}'", type_name, method),
                            member_span,
                        ));
                        ResolvedType::Error
                    }
                });
            }
        }

        let receiver_ty = self.infer_expr_type(receiver);
        let type_name = match &receiver_ty {
            ResolvedType::Struct(name) => name.clone(),
            _ => return None,
        };
        let info = self.lookup_method(&type_name, method)?.clone();
        if info.is_static {
            self.errors.push(TypeError::new(
                format!("'{}' is a static method of '{}'; call it as '{}.{}(...)'", method, type_name, type_name, method),
                member_span,
            ));
            return Some(ResolvedType::Error);
        }
        if info.is_mut {
            if !Self::is_place(receiver) {
                self.errors.push(TypeError::new(
                    format!("cannot call mut method '{}' on a temporary value of type '{}'", method, type_name),
                    member_span,
                ));
            } else {
                let before = self.errors.len();
                self.check_assignable(receiver);
                // Reword the assignment diagnostics for the call site.
                for err in self.errors.iter_mut().skip(before) {
                    if let Some(rest) = err.message.strip_prefix("cannot assign to ") {
                        err.message = format!("cannot call mut method '{}' on {}", method, rest);
                    } else if err.message.starts_with("cannot assign through 'self'") {
                        err.message = err.message.replacen("cannot assign through 'self'", &format!("cannot call mut method '{}' on 'self'", method), 1);
                    }
                }
            }
        }
        self.check_args(&info.params, args, call_span);
        self.record(SiteKind::Call, *call_span, Resolution::MethodCall { type_name });
        Some(info.ret)
    }

    /// Check call arguments against parameter types.
    fn check_args(&mut self, params: &[ResolvedType], args: &[Expr], span: &Span) {
        if args.len() != params.len() {
            self.errors.push(TypeError::new(
                format!("expected {} arguments, found {}", params.len(), args.len()),
                span,
            ));
            for arg in args {
                self.infer_expr_type(arg);
            }
            return;
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
    }

    /// Whether `name` is a runtime built-in that no user declaration shadows.
    fn is_builtin(&self, name: &str) -> bool {
        self.builtins.contains(name) && self.symbols.lookup(name).is_none()
    }

    /// Type check the entire AST. See `check_and_lower` for the lowered result.
    pub fn check_program(&mut self, ast: &Ast) -> Result<(), Vec<TypeError>> {
        self.check_and_lower(ast).map(|_| ())
    }

    /// Type check the whole program and return the lowered AST: method calls
    /// resolved to `Expr::MethodCall`, static calls to their mangled function,
    /// and every `let` carrying its type.
    pub fn check_and_lower(&mut self, ast: &Ast) -> Result<Ast, Vec<TypeError>> {
        // Pass 1a: nominal types, so that methods, extensions and signatures
        // declared earlier in the file can refer to types declared later.
        for decl in &ast.declarations {
            match decl {
                Decl::Struct(s) => self.register_struct(s),
                Decl::Variant(v) => self.register_variant(v),
                Decl::Protocol(p) => self.register_protocol(p),
                Decl::Layer(l) => self.register_layer(l),
                Decl::Panel(p) => self.register_panel(p),
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
                _ => {}
            }
        }

        // Pass 1b: functions, methods, constants and globals.
        self.resolutions.clear();
        for decl in &ast.declarations {
            match decl {
                Decl::Function(f) => self.register_function(f),
                Decl::Extern(e) => self.register_extern(e),
                Decl::Struct(s) => {
                    for method in &s.methods {
                        self.register_method(&s.name, method);
                    }
                }
                Decl::Extension(ext) => {
                    if !self.type_kinds.contains_key(&ext.target) {
                        self.errors.push(TypeError::new(
                            format!("extension of undefined type '{}'", ext.target),
                            &ext.span,
                        ));
                    }
                    for method in &ext.methods {
                        self.register_method(&ext.target, method);
                    }
                }
                Decl::Layer(l) => {
                    for method in &l.methods {
                        self.register_method(&l.name, method);
                    }
                }
                Decl::Panel(p) => {
                    for method in &p.methods {
                        self.register_method(&p.name, method);
                    }
                }
                Decl::Const(c) => {
                    let declared = c.ty.as_ref().map(ResolvedType::from_parser_type);
                    let inferred = self.infer_expr_type(&c.value);
                    let ty = match declared {
                        Some(decl_ty) => {
                            if !decl_ty.is_assignable_from(&inferred) {
                                self.errors.push(TypeError::new(
                                    format!(
                                        "const '{}' expects '{}', found '{}'",
                                        c.name,
                                        decl_ty.display_name(),
                                        inferred.display_name()
                                    ),
                                    &c.span,
                                ));
                            }
                            decl_ty
                        }
                        None => inferred,
                    };
                    let _ = self.symbols.define(Symbol {
                        name: c.name.clone(),
                        ty,
                        mutable: false,
                        kind: SymbolKind::Variable,
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
                Decl::Import(_) | Decl::Variant(_) | Decl::Protocol(_) | Decl::Typealias(_) => {}
            }
        }
        let global_resolutions = std::mem::take(&mut self.resolutions);

        // Pass 2: check and lower every body.
        let mut lowered: Vec<Decl> = Vec::with_capacity(ast.declarations.len());
        for decl in &ast.declarations {
            let out = match decl {
                Decl::Function(f) => Decl::Function(self.check_function(f)),
                Decl::Struct(s) => Decl::Struct(StructDecl {
                    methods: s.methods.iter().map(|m| self.check_method(&s.name, m)).collect(),
                    ..s.clone()
                }),
                Decl::Extension(ext) => Decl::Extension(ExtensionDecl {
                    methods: ext.methods.iter().map(|m| self.check_method(&ext.target, m)).collect(),
                    ..ext.clone()
                }),
                Decl::Layer(l) => Decl::Layer(LayerDecl {
                    methods: l.methods.iter().map(|m| self.check_method(&l.name, m)).collect(),
                    ..l.clone()
                }),
                Decl::Panel(p) => Decl::Panel(PanelDecl {
                    methods: p.methods.iter().map(|m| self.check_method(&p.name, m)).collect(),
                    ..p.clone()
                }),
                Decl::Const(c) => Decl::Const(ConstDecl {
                    value: Lowerer::new(&global_resolutions).expr(&c.value),
                    ..c.clone()
                }),
                Decl::GlobalVar(g) => Decl::GlobalVar(Lowerer::new(&global_resolutions).let_stmt(g)),
                other => other.clone(),
            };
            lowered.push(out);
        }

        if self.errors.is_empty() {
            Ok(Ast { declarations: lowered })
        } else {
            Err(self.errors.clone())
        }
    }

    fn register_nominal(&mut self, name: &str, kind: TypeKind, fields: HashMap<String, ResolvedType>, span: &Span) {
        if let Some(existing) = self.type_kinds.get(name) {
            self.errors.push(TypeError::new(
                format!("{} '{}' conflicts with the {} of the same name", kind.keyword(), name, existing.keyword()),
                span,
            ));
            return;
        }
        self.type_kinds.insert(name.to_string(), kind);
        if let Err(e) = self.symbols.define_struct(StructInfo { name: name.to_string(), fields }) {
            self.errors.push(TypeError::new(e, span));
        }
    }

    fn register_variant(&mut self, v: &VariantDecl) {
        self.register_nominal(&v.name, TypeKind::Variant, HashMap::new(), &v.span);
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

    fn register_protocol(&mut self, p: &ProtocolDecl) {
        self.register_nominal(&p.name, TypeKind::Protocol, HashMap::new(), &p.span);
        let _ = self.symbols.define(Symbol {
            name: p.name.clone(),
            ty: ResolvedType::Struct(p.name.clone()),
            mutable: false,
            kind: SymbolKind::Struct,
        });
    }

    fn register_layer(&mut self, l: &LayerDecl) {
        let fields = l.fields.iter()
            .map(|f| (f.name.clone(), ResolvedType::from_parser_type(&f.ty)))
            .collect();
        self.register_nominal(&l.name, TypeKind::Layer, fields, &l.span);
        let _ = self.symbols.define(Symbol {
            name: l.name.clone(),
            ty: ResolvedType::Struct(l.name.clone()),
            mutable: false,
            kind: SymbolKind::Struct,
        });
    }

    fn register_panel(&mut self, p: &PanelDecl) {
        self.register_nominal(&p.name, TypeKind::Panel, HashMap::new(), &p.span);
    }

    /// Record a method of `type_name`. The C symbol is `TypeName_method`.
    fn register_method(&mut self, type_name: &str, f: &FnDecl) {
        let info = MethodInfo {
            name: f.name.clone(),
            params: f.params.iter().map(|p| ResolvedType::from_parser_type(&p.ty)).collect(),
            ret: f.return_type.as_ref().map(ResolvedType::from_parser_type).unwrap_or(ResolvedType::Void),
            is_mut: f.is_mut,
            is_static: f.is_static,
            span: f.span,
        };
        let table = self.methods.entry(type_name.to_string()).or_default();
        if table.contains_key(&f.name) {
            self.errors.push(TypeError::new(
                format!("method '{}' is already defined for type '{}'", f.name, type_name),
                &f.span,
            ));
            return;
        }
        table.insert(f.name.clone(), info);
    }

    /// Look up a method of a nominal type.
    fn lookup_method(&self, type_name: &str, method: &str) -> Option<&MethodInfo> {
        self.methods.get(type_name).and_then(|t| t.get(method))
    }

    /// C symbol of a method.
    pub fn mangle_method(type_name: &str, method: &str) -> String {
        format!("{}_{}", type_name, method)
    }

    fn register_struct(&mut self, s: &StructDecl) {
        let mut fields = std::collections::HashMap::new();
        for field in &s.fields {
            let ty = ResolvedType::from_parser_type(&field.ty);
            fields.insert(field.name.clone(), ty);
        }

        self.register_nominal(&s.name, TypeKind::Struct, fields, &s.span);
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

    /// Check a free function body and return it lowered.
    fn check_function(&mut self, f: &FnDecl) -> FnDecl {
        self.check_body(f, None)
    }

    /// Check a method body with `self` bound to `type_name` and return it lowered.
    fn check_method(&mut self, type_name: &str, f: &FnDecl) -> FnDecl {
        if f.is_static {
            self.check_body(f, None)
        } else {
            let ctx = SelfContext { type_name: type_name.to_string(), is_mut: f.is_mut, method: f.name.clone() };
            self.check_body(f, Some(ctx))
        }
    }

    fn check_body(&mut self, f: &FnDecl, self_ctx: Option<SelfContext>) -> FnDecl {
        self.resolutions.clear();
        let saved_self = std::mem::replace(&mut self.current_self, self_ctx);
        self.symbols.push_scope();

        if let Some(ctx) = &self.current_self {
            let _ = self.symbols.define(Symbol {
                name: "self".to_string(),
                ty: ResolvedType::Struct(ctx.type_name.clone()),
                mutable: ctx.is_mut,
                kind: SymbolKind::Parameter,
            });
        }

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

        // Check function body
        self.check_block(&f.body);

        self.current_return_type = None;
        self.symbols.pop_scope();
        self.current_self = saved_self;

        let table = std::mem::take(&mut self.resolutions);
        Lowerer::new(&table).fn_decl(f)
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
                self.check_block(&t.try_block);
                self.symbols.pop_scope();
                
                self.symbols.push_scope();
                if let Some(var) = &t.catch_var {
                    let _ = self.symbols.define(Symbol {
                        name: var.clone(),
                        ty: ResolvedType::String, // Error type
                        mutable: false,
                        kind: SymbolKind::Variable,
                    });
                }
                self.check_block(&t.catch_block);
                self.symbols.pop_scope();
            }
            Stmt::Throw(t) => {
                self.infer_expr_type(&t.value);
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

        let final_type = match (&declared_type, &inferred_type) {
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

        if l.ty.is_none() {
            if let Some(ty) = final_type.to_parser_type() {
                self.record(SiteKind::Let, l.span, Resolution::LetType(ty));
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
        if cond_type != ResolvedType::Bool {
            self.errors.push(TypeError {
                message: format!(
                    "if condition must be bool, found '{}'",
                    cond_type.display_name()
                ),
                span: i.span,
            });
        }

        self.symbols.push_scope();
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
                if let Expr::Identifier(name, _) = callee.as_ref() {
                    if self.is_builtin(name) {
                        // Natives are variadic and dynamically typed; still check the arguments.
                        for arg in args {
                            self.infer_expr_type(arg);
                        }
                        return ResolvedType::Unknown;
                    }
                }
                if let Expr::Member(receiver, method, member_span) = callee.as_ref() {
                    if let Some(ty) = self.check_method_call(receiver, method, args, span, member_span) {
                        return ty;
                    }
                }
                let callee_ty = self.infer_expr_type(callee);
                self.check_call(&callee_ty, args, span)
            }
            Expr::MethodCall(receiver, type_name, method, args, span) => {
                // Already lowered (the checker ran twice); re-check against the method.
                self.infer_expr_type(receiver);
                match self.lookup_method(type_name, method).cloned() {
                    Some(info) => {
                        self.check_args(&info.params, args, span);
                        info.ret
                    }
                    None => {
                        self.errors.push(TypeError::new(
                            format!("type '{}' has no method '{}'", type_name, method),
                            span,
                        ));
                        ResolvedType::Error
                    }
                }
            }
            Expr::SelfRef(span) => match &self.current_self {
                Some(ctx) => ResolvedType::Struct(ctx.type_name.clone()),
                None => {
                    self.errors.push(TypeError::new(
                        "'self' is only available inside a method body",
                        span,
                    ));
                    ResolvedType::Error
                }
            },
            Expr::Member(obj, field, span) => {
                if let Expr::Identifier(type_name, _) = obj.as_ref() {
                    if self.is_type_name(type_name) {
                        let msg = match self.lookup_method(type_name, field) {
                            Some(info) if info.is_static => format!("'{}.{}' is a static method; call it with parentheses", type_name, field),
                            Some(_) => format!("'{}' is an instance method of '{}'; call it on a value", field, type_name),
                            None => format!("type '{}' has no static member '{}'", type_name, field),
                        };
                        self.errors.push(TypeError::new(msg, span));
                        return ResolvedType::Error;
                    }
                }
                let obj_ty = self.infer_expr_type(obj);
                if let ResolvedType::Struct(name) = &obj_ty {
                    let has_field = self.symbols.lookup_struct(name).map(|s| s.fields.contains_key(field)).unwrap_or(false);
                    if !has_field && self.lookup_method(name, field).is_some() {
                        self.errors.push(TypeError::new(
                            format!("'{}' is a method of '{}'; call it with parentheses", field, name),
                            span,
                        ));
                        return ResolvedType::Error;
                    }
                }
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
                self.check_assignable(target);
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
                self.check_assignable(target);
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
                self.check_assignable(operand);
                if !matches!(ty, ResolvedType::Int | ResolvedType::Float) {
                    self.errors.push(TypeError::new(
                        format!("cannot increment/decrement '{}'", ty.display_name()),
                        span,
                    ));
                }
                ty
            }
            Expr::NullCoalesce(left, right, _span) => {
                let _left_ty = self.infer_expr_type(left);
                let right_ty = self.infer_expr_type(right);
                // Return the right type as fallback, ideally unwrapped optional
                right_ty
            }
            Expr::OptionalChain(obj, field, span) => {
                let obj_ty = self.infer_expr_type(obj);
                self.check_member_access(&obj_ty, field, span)
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
                if left == right || 
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

/// Type check the AST. On success the lowered AST is returned (method calls
/// resolved, untyped `let`s annotated); every type error found is returned
/// together, none is printed.
pub fn check(ast: &Ast) -> Result<Ast, Vec<TypeError>> {
    // The checker recurses over the expression tree, so run it on a thread
    // with a stack large enough for `MAX_EXPR_DEPTH` levels in debug builds.
    std::thread::scope(|scope| {
        let worker = std::thread::Builder::new()
            .name("reox-typecheck".to_string())
            .stack_size(CHECK_STACK_BYTES)
            .spawn_scoped(scope, || TypeChecker::new().check_and_lower(ast));
        match worker {
            Ok(handle) => handle.join().unwrap_or_else(|_| {
                Err(vec![TypeError::new("type checker thread terminated unexpectedly", &Span::default())])
            }),
            Err(_) => TypeChecker::new().check_and_lower(ast),
        }
    })
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
                is_pub: false,
                is_static: false,
                is_mut: false,
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

    // === Methods and self ===

    const COUNTER: &str = r#"
        struct Counter {
            count: int
            fn value() -> int { return self.count; }
            mut fn bump(by: int) { self.count += by; }
            static fn make() -> Counter { return Counter { count: 0 }; }
        }
    "#;

    fn with_counter(body: &str) -> Result<(), Vec<TypeError>> {
        check_source(&format!("{}\n{}", COUNTER, body))
    }

    fn messages(result: Result<(), Vec<TypeError>>) -> Vec<String> {
        result.unwrap_err().into_iter().map(|e| e.message).collect()
    }

    #[test]
    fn methods_dispatch_on_receiver_type() {
        let ok = with_counter("fn main() { let mut c = Counter.make(); c.bump(2); let v: int = c.value(); }");
        assert!(ok.is_ok(), "{:?}", ok.err());
    }

    #[test]
    fn method_argument_types_are_checked() {
        let msgs = messages(with_counter("fn main() { let mut c = Counter.make(); c.bump(\"x\"); c.value(1); }"));
        assert!(msgs.iter().any(|m| m.contains("argument 1 type mismatch: expected 'int', found 'string'")), "{:?}", msgs);
        assert!(msgs.iter().any(|m| m.contains("expected 0 arguments, found 1")), "{:?}", msgs);
    }

    #[test]
    fn assigning_through_self_requires_a_mut_method() {
        let msgs = messages(check_source("struct P { x: int fn reset() { self.x = 0; } }"));
        assert_eq!(msgs.len(), 1);
        assert!(msgs[0].contains("cannot assign through 'self' in method 'reset' of 'P'; declare it with 'mut fn'"), "{}", msgs[0]);
        let msgs = messages(check_source("struct P { x: int fn step() { self.x++; } }"));
        assert!(msgs[0].contains("declare it with 'mut fn'"), "{}", msgs[0]);
    }

    #[test]
    fn mut_methods_need_a_mutable_receiver() {
        let msgs = messages(with_counter("fn main() { let c = Counter.make(); c.bump(1); }"));
        assert_eq!(msgs.len(), 1);
        assert!(msgs[0].contains("cannot call mut method 'bump' on immutable variable 'c'; declare it with 'let mut'"), "{}", msgs[0]);
        let msgs = messages(with_counter("fn main() { Counter.make().bump(1); }"));
        assert!(msgs[0].contains("cannot call mut method 'bump' on a temporary value"), "{}", msgs[0]);
        let msgs = messages(with_counter("fn f(c: Counter) { c.bump(1); }"));
        assert!(msgs[0].contains("cannot call mut method 'bump' on parameter 'c'"), "{}", msgs[0]);
        // A mut method may call another mut method on self; a non-mut one may not.
        assert!(with_counter("extension Counter { mut fn twice() { self.bump(1); self.bump(1); } }").is_ok());
        let msgs = messages(with_counter("extension Counter { fn twice() { self.bump(1); } }"));
        assert!(msgs[0].contains("cannot call mut method 'bump' on 'self' in method 'twice'"), "{}", msgs[0]);
    }

    #[test]
    fn self_is_only_available_in_methods() {
        let msgs = messages(check_source("fn main() { let x = self; }"));
        assert!(msgs[0].contains("'self' is only available inside a method body"), "{}", msgs[0]);
        let msgs = messages(check_source("struct P { x: int static fn make() -> int { return self.x; } }"));
        assert!(msgs[0].contains("'self' is only available inside a method body"), "{}", msgs[0]);
    }

    #[test]
    fn static_and_instance_methods_are_not_interchangeable() {
        let msgs = messages(with_counter("fn main() { let v = Counter.value(); }"));
        assert!(msgs[0].contains("method 'value' of 'Counter' needs a receiver"), "{}", msgs[0]);
        let msgs = messages(with_counter("fn main() { let c = Counter.make(); let d = c.make(); }"));
        assert!(msgs[0].contains("'make' is a static method of 'Counter'; call it as 'Counter.make(...)'"), "{}", msgs[0]);
        let msgs = messages(with_counter("fn main() { let c = Counter.make(); let v = c.value; }"));
        assert!(msgs[0].contains("'value' is a method of 'Counter'; call it with parentheses"), "{}", msgs[0]);
        let msgs = messages(with_counter("fn main() { let v = Counter.nothing(); }"));
        assert!(msgs[0].contains("type 'Counter' has no static method 'nothing'"), "{}", msgs[0]);
    }

    #[test]
    fn duplicate_methods_and_unknown_extension_targets_are_errors() {
        let msgs = messages(check_source("struct P { x: int fn f() { } } extension P { fn f() { } }"));
        assert!(msgs[0].contains("method 'f' is already defined for type 'P'"), "{}", msgs[0]);
        let msgs = messages(check_source("extension Nope { fn f() { } }"));
        assert!(msgs[0].contains("extension of undefined type 'Nope'"), "{}", msgs[0]);
    }

    #[test]
    fn layers_and_panels_have_self_too() {
        let ok = check_source(r#"
            layer Button { let label: string; signal on_click; fn title() -> string { return self.label; } }
            panel Main { title: "x", fn root() -> int { return 1; } static fn version() -> int { return 2; } }
            fn main() { let b = Button { label: "ok" }; let t: string = b.title(); let r: int = Main.version(); }
        "#);
        assert!(ok.is_ok(), "{:?}", ok.err());
    }

    #[test]
    fn field_assignment_through_immutable_variable_is_rejected() {
        let msgs = messages(check_source("struct P { x: int } fn main() { let p = P { x: 1 }; p.x = 2; }"));
        assert!(msgs[0].contains("cannot assign to immutable variable 'p'"), "{}", msgs[0]);
        assert!(check_source("struct P { x: int } fn main() { let mut p = P { x: 1 }; p.x = 2; p.x += 1; }").is_ok());
    }

    #[test]
    fn lowering_resolves_method_calls_and_let_types() {
        let tokens = tokenize(&format!("{}\nfn main() {{ let mut c = Counter.make(); c.bump(1); let v = c.value(); }}", COUNTER)).unwrap();
        let ast = parse(&tokens).unwrap();
        let lowered = check(&ast).unwrap();
        let main = match &lowered.declarations[1] { Decl::Function(f) => f, other => panic!("{:?}", other) };
        match &main.body.statements[0] {
            Stmt::Let(l) => {
                assert_eq!(l.ty, Some(Type::Named("Counter".into())));
                assert!(matches!(&l.init, Some(Expr::Call(callee, _, _)) if matches!(callee.as_ref(), Expr::Identifier(n, _) if n == "Counter_make")));
            }
            other => panic!("{:?}", other),
        }
        assert!(matches!(&main.body.statements[1], Stmt::Expr(Expr::MethodCall(_, t, m, args, _)) if t == "Counter" && m == "bump" && args.len() == 1));
        match &main.body.statements[2] {
            Stmt::Let(l) => assert_eq!(l.ty, Some(Type::Int)),
            other => panic!("{:?}", other),
        }
    }
}
