// REOX Compiler - AST lowering
//
// The type checker records what it resolved at each source site (which
// method a call dispatches to, the type of an untyped `let`, ...) and this
// pass rewrites the AST accordingly so that later stages (codegen, the
// interpreter) never have to redo name resolution.
//
// Resolutions are keyed by source span. The table is filled and consumed one
// function body at a time, so the same span never carries two meanings.

use std::collections::HashMap;

use crate::lexer::Span;
use crate::parser::*;

/// Which construct a resolution applies to. Distinct kinds may share a span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum SiteKind {
    Let,
    Call,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct SiteKey {
    kind: SiteKind,
    start: usize,
    end: usize,
    line: u32,
    column: u32,
}

impl SiteKey {
    pub(crate) fn new(kind: SiteKind, span: Span) -> Self {
        Self { kind, start: span.start, end: span.end, line: span.line, column: span.column }
    }
}

/// What the checker decided at a site.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Resolution {
    /// Fill in the type of an untyped `let`.
    LetType(Type),
    /// `receiver.method(args)` dispatches statically to a method of `type_name`.
    MethodCall { type_name: String },
    /// `Type.method(args)` is a static method call of the function `mangled`.
    StaticCall { mangled: String },
}

pub(crate) type Resolutions = HashMap<SiteKey, Resolution>;

/// Rebuilds AST nodes, applying the recorded resolutions.
pub(crate) struct Lowerer<'a> {
    res: &'a Resolutions,
}

impl<'a> Lowerer<'a> {
    pub(crate) fn new(res: &'a Resolutions) -> Self {
        Self { res }
    }

    fn lookup(&self, kind: SiteKind, span: Span) -> Option<&Resolution> {
        self.res.get(&SiteKey::new(kind, span))
    }

    pub(crate) fn fn_decl(&self, f: &FnDecl) -> FnDecl {
        FnDecl { body: self.block(&f.body), ..f.clone() }
    }

    pub(crate) fn block(&self, b: &Block) -> Block {
        Block {
            statements: b.statements.iter().map(|s| self.stmt(s)).collect(),
            span: b.span,
        }
    }

    pub(crate) fn let_stmt(&self, l: &LetStmt) -> LetStmt {
        let ty = match (&l.ty, self.lookup(SiteKind::Let, l.span)) {
            (None, Some(Resolution::LetType(t))) => Some(t.clone()),
            (ty, _) => ty.clone(),
        };
        LetStmt {
            ty,
            init: l.init.as_ref().map(|e| self.expr(e)),
            ..l.clone()
        }
    }

    fn stmt(&self, s: &Stmt) -> Stmt {
        match s {
            Stmt::Let(l) => Stmt::Let(self.let_stmt(l)),
            Stmt::Expr(e) => Stmt::Expr(self.expr(e)),
            Stmt::Return(r) => Stmt::Return(ReturnStmt {
                value: r.value.as_ref().map(|e| self.expr(e)),
                span: r.span,
            }),
            Stmt::If(i) => Stmt::If(IfStmt {
                condition: self.expr(&i.condition),
                then_block: self.block(&i.then_block),
                else_block: i.else_block.as_ref().map(|b| self.block(b)),
                span: i.span,
            }),
            Stmt::While(w) => Stmt::While(WhileStmt {
                condition: self.expr(&w.condition),
                body: self.block(&w.body),
                span: w.span,
            }),
            Stmt::For(f) => Stmt::For(ForStmt {
                var: f.var.clone(),
                iterable: self.expr(&f.iterable),
                body: self.block(&f.body),
                span: f.span,
            }),
            Stmt::Block(b) => Stmt::Block(self.block(b)),
            Stmt::Break(s) => Stmt::Break(*s),
            Stmt::Continue(s) => Stmt::Continue(*s),
            Stmt::Guard(g) => Stmt::Guard(GuardStmt {
                condition: self.expr(&g.condition),
                else_block: self.block(&g.else_block),
                span: g.span,
            }),
            Stmt::Defer(d) => Stmt::Defer(DeferStmt { body: self.block(&d.body), span: d.span }),
            Stmt::TryCatch(t) => Stmt::TryCatch(TryCatchStmt {
                try_block: self.block(&t.try_block),
                catch_var: t.catch_var.clone(),
                catch_block: self.block(&t.catch_block),
                span: t.span,
            }),
            Stmt::Throw(t) => Stmt::Throw(ThrowStmt { value: self.expr(&t.value), span: t.span }),
        }
    }

    fn exprs(&self, es: &[Expr]) -> Vec<Expr> {
        es.iter().map(|e| self.expr(e)).collect()
    }

    pub(crate) fn expr(&self, e: &Expr) -> Expr {
        match e {
            Expr::Literal(_) | Expr::Identifier(_, _) | Expr::Nil(_) | Expr::SelfRef(_) => e.clone(),
            Expr::Binary(l, op, r, s) => Expr::Binary(Box::new(self.expr(l)), *op, Box::new(self.expr(r)), *s),
            Expr::Unary(op, x, s) => Expr::Unary(*op, Box::new(self.expr(x)), *s),
            Expr::Call(callee, args, s) => self.call(callee, args, *s),
            Expr::MethodCall(recv, ty, m, args, s) => {
                Expr::MethodCall(Box::new(self.expr(recv)), ty.clone(), m.clone(), self.exprs(args), *s)
            }
            Expr::Member(obj, name, s) => Expr::Member(Box::new(self.expr(obj)), name.clone(), *s),
            Expr::Index(a, i, s) => Expr::Index(Box::new(self.expr(a)), Box::new(self.expr(i)), *s),
            Expr::Assign(t, v, s) => Expr::Assign(Box::new(self.expr(t)), Box::new(self.expr(v)), *s),
            Expr::StructLit(name, fields, s) => Expr::StructLit(
                name.clone(),
                fields.iter().map(|(n, v)| (n.clone(), self.expr(v))).collect(),
                *s,
            ),
            Expr::ArrayLit(elems, s) => Expr::ArrayLit(self.exprs(elems), *s),
            Expr::Match(scrutinee, arms, s) => Expr::Match(
                Box::new(self.expr(scrutinee)),
                arms.iter()
                    .map(|arm| MatchArm { pattern: arm.pattern.clone(), body: self.expr(&arm.body), span: arm.span })
                    .collect(),
                *s,
            ),
            Expr::CompoundAssign(t, op, v, s) => {
                Expr::CompoundAssign(Box::new(self.expr(t)), *op, Box::new(self.expr(v)), *s)
            }
            Expr::PreIncrement(x, s) => Expr::PreIncrement(Box::new(self.expr(x)), *s),
            Expr::PreDecrement(x, s) => Expr::PreDecrement(Box::new(self.expr(x)), *s),
            Expr::PostIncrement(x, s) => Expr::PostIncrement(Box::new(self.expr(x)), *s),
            Expr::PostDecrement(x, s) => Expr::PostDecrement(Box::new(self.expr(x)), *s),
            Expr::NullCoalesce(l, r, s) => Expr::NullCoalesce(Box::new(self.expr(l)), Box::new(self.expr(r)), *s),
            Expr::OptionalChain(obj, name, s) => Expr::OptionalChain(Box::new(self.expr(obj)), name.clone(), *s),
            Expr::TrailingClosure(callee, body, s) => {
                Expr::TrailingClosure(Box::new(self.expr(callee)), Box::new(self.block(body)), *s)
            }
            Expr::Await(x, s) => Expr::Await(Box::new(self.expr(x)), *s),
            Expr::Range(a, b, s) => Expr::Range(Box::new(self.expr(a)), Box::new(self.expr(b)), *s),
            Expr::Action(params, ret, body, s) => {
                Expr::Action(params.clone(), ret.clone(), Box::new(self.block(body)), *s)
            }
        }
    }

    fn call(&self, callee: &Expr, args: &[Expr], span: Span) -> Expr {
        let args = self.exprs(args);
        match (callee, self.lookup(SiteKind::Call, span)) {
            (Expr::Member(recv, method, _), Some(Resolution::MethodCall { type_name })) => {
                Expr::MethodCall(Box::new(self.expr(recv)), type_name.clone(), method.clone(), args, span)
            }
            (Expr::Member(_, _, member_span), Some(Resolution::StaticCall { mangled })) => {
                Expr::Call(Box::new(Expr::Identifier(mangled.clone(), *member_span)), args, span)
            }
            _ => Expr::Call(Box::new(self.expr(callee)), args, span),
        }
    }
}
