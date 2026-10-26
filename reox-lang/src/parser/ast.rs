// REOX Compiler - AST Node Definitions
// Abstract Syntax Tree for REOX language

#![allow(dead_code)]

use crate::lexer::Span;

/// A complete program (list of declarations)
#[derive(Debug, Clone)]
pub struct Program {
    pub declarations: Vec<Decl>,
}

/// Top-level declarations
#[derive(Debug, Clone)]
pub enum Decl {
    Function(FnDecl),
    Struct(StructDecl),
    Import(ImportDecl),
    Extern(ExternDecl),
    Variant(VariantDecl),
    Protocol(ProtocolDecl),
    Extension(ExtensionDecl),
    Layer(LayerDecl),
    Panel(PanelDecl),
    Const(ConstDecl),
    Typealias(TypealiasDecl),
    GlobalVar(LetStmt),
}

/// A generic type parameter: `T` or `T: Protocol + Other`.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeParam {
    pub name: String,
    pub bounds: Vec<String>,
    pub span: Span,
}

/// Function declaration
#[derive(Debug, Clone)]
pub struct FnDecl {
    pub name: String,
    /// Generic parameters (`fn swap<T>(...)`); empty for ordinary functions.
    pub type_params: Vec<TypeParam>,
    pub params: Vec<Param>,
    pub return_type: Option<Type>,
    pub body: Block,
    pub is_async: bool,
    /// `fn f() throws -> T`: the function returns `Result<T, Error>` and
    /// callers must use `try`.
    pub throws: bool,
    pub is_pub: bool,
    pub is_static: bool,
    /// `mut fn`: the method may assign through `self`. Only meaningful for
    /// methods declared inside a struct, extension, layer or panel body.
    pub is_mut: bool,
    pub span: Span,
}

/// Function parameter
#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub ty: Type,
    pub span: Span,
}

/// Struct declaration
#[derive(Debug, Clone)]
pub struct StructDecl {
    pub name: String,
    /// Generic parameters (`struct Box<T>`); empty for ordinary structs.
    pub type_params: Vec<TypeParam>,
    pub fields: Vec<Field>,
    /// `ref struct`: instances live on the heap and are reference counted.
    pub is_ref: bool,
    /// Methods declared inside the struct body. Each takes an implicit `self`
    /// unless it is `static`.
    pub methods: Vec<FnDecl>,
    pub is_pub: bool,
    pub span: Span,
}

/// Struct field
#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub ty: Type,
    /// `weak name: T`: a non-owning reference that reads as `T?`.
    pub is_weak: bool,
    pub span: Span,
}

/// Import declaration
#[derive(Debug, Clone)]
pub struct ImportDecl {
    pub path: Vec<String>,
    pub span: Span,
}

/// Extern function declaration
#[derive(Debug, Clone)]
pub struct ExternDecl {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<Type>,
    pub is_async: bool,
    pub span: Span,
}

/// Type annotation
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Int,
    Float,
    String,
    Bool,
    Void,
    // Explicit integer widths
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    I64,
    U64,
    USize,
    Named(String),
    /// A generic type applied to arguments: `Box<int>`, `Result<T, E>`.
    Generic(String, Vec<Type>),
    Array(Box<Type>),
    /// `[K: V]` string keyed map
    Map(Box<Type>, Box<Type>),
    Optional(Box<Type>),
    Function(Vec<Type>, Box<Type>),
    Tuple(Vec<Type>),
}

/// A block of statements
#[derive(Debug, Clone)]
pub struct Block {
    pub statements: Vec<Stmt>,
    pub span: Span,
}

/// Statements
#[derive(Debug, Clone)]
pub enum Stmt {
    Let(LetStmt),
    Expr(Expr),
    Return(ReturnStmt),
    If(IfStmt),
    While(WhileStmt),
    For(ForStmt),
    Block(Block),
    Break(Span),
    Continue(Span),
    // Swift/C++ style statements
    Guard(GuardStmt),
    Defer(DeferStmt),
    TryCatch(TryCatchStmt),
    Throw(ThrowStmt),
}

/// Let statement (variable declaration)
#[derive(Debug, Clone)]
pub struct LetStmt {
    pub name: String,
    pub mutable: bool,
    pub ty: Option<Type>,
    pub init: Option<Expr>,
    pub is_bind: bool,
    /// `weak let`: a non-owning reference that reads as `T?`.
    pub is_weak: bool,
    pub span: Span,
}

/// Return statement
#[derive(Debug, Clone)]
pub struct ReturnStmt {
    pub value: Option<Expr>,
    pub span: Span,
}

/// If statement
#[derive(Debug, Clone)]
pub struct IfStmt {
    pub condition: Expr,
    /// `if let name = condition { ... }`: the then block runs with `name`
    /// bound to the unwrapped value when `condition` is not nil.
    pub binding: Option<String>,
    pub then_block: Block,
    pub else_block: Option<Block>,
    pub span: Span,
}

/// While loop
#[derive(Debug, Clone)]
pub struct WhileStmt {
    pub condition: Expr,
    pub body: Block,
    pub span: Span,
}

/// For loop
#[derive(Debug, Clone)]
pub struct ForStmt {
    pub var: String,
    pub iterable: Expr,
    pub body: Block,
    pub span: Span,
}

/// Guard statement (Swift-style early exit)
#[derive(Debug, Clone)]
pub struct GuardStmt {
    pub condition: Expr,
    pub else_block: Block,
    pub span: Span,
}

/// Defer statement (Swift/Go-style cleanup)
#[derive(Debug, Clone)]
pub struct DeferStmt {
    pub body: Block,
    pub span: Span,
}

/// Try-catch statement
#[derive(Debug, Clone)]
pub struct TryCatchStmt {
    pub try_block: Block,
    pub catch_var: Option<String>,
    /// `catch e as MyError`: only errors of this type are caught here;
    /// any other error propagates.
    pub catch_type: Option<Type>,
    pub catch_block: Block,
    pub span: Span,
}

/// Throw statement
#[derive(Debug, Clone)]
pub struct ThrowStmt {
    pub value: Expr,
    pub span: Span,
}

/// Expressions
#[derive(Debug, Clone)]
pub enum Expr {
    Literal(Literal),
    Identifier(String, Span),
    Binary(Box<Expr>, BinOp, Box<Expr>, Span),
    Unary(UnaryOp, Box<Expr>, Span),
    Call(Box<Expr>, Vec<Expr>, Span),
    Member(Box<Expr>, String, Span),
    Index(Box<Expr>, Box<Expr>, Span),
    Assign(Box<Expr>, Box<Expr>, Span),
    StructLit(String, Vec<(String, Expr)>, Span),
    ArrayLit(Vec<Expr>, Span),
    Match(Box<Expr>, Vec<MatchArm>, Span),
    // Swift/C++ style expressions
    CompoundAssign(Box<Expr>, CompoundOp, Box<Expr>, Span),
    PreIncrement(Box<Expr>, Span),
    PreDecrement(Box<Expr>, Span),
    PostIncrement(Box<Expr>, Span),
    PostDecrement(Box<Expr>, Span),
    NullCoalesce(Box<Expr>, Box<Expr>, Span),     // left ?? right
    OptionalChain(Box<Expr>, String, Span),       // obj?.member
    TrailingClosure(Box<Expr>, Box<Block>, Span), // button("Click") { ... }
    Nil(Span),
    Await(Box<Expr>, Span),                        // await expr
    // Range expressions
    Range(Box<Expr>, Box<Expr>, Span),             // start..end (inclusive)
    // Closure/lambda expression
    Action(Vec<Param>, Option<Box<Type>>, Box<Block>, Span), // action (params) -> Type { body }
    /// `try f()`: call a throwing function and propagate its error
    Try(Box<Expr>, Span),
    /// `self` inside a method body
    SelfRef(Span),
    /// `{ statements }` used as an expression (match arm bodies). Its value is
    /// the last expression statement, or nothing.
    Block(Block, Span),
    /// Type ascription inserted by the type checker where codegen needs the
    /// type of a sub-expression (match scrutinee and result). Never parsed.
    Typed(Box<Expr>, Type, Span),
    /// Statically dispatched method call, produced by the type checker from
    /// `receiver.method(args)`: (receiver, type name, method name, args, span).
    /// Codegen emits `TypeName_method(&receiver, args)`.
    MethodCall(Box<Expr>, String, String, Vec<Expr>, Span),
}

impl Expr {
    /// Source location of the expression, for diagnostics.
    pub fn span(&self) -> Span {
        match self {
            Expr::Literal(lit) => lit.span(),
            Expr::Identifier(_, s)
            | Expr::Binary(_, _, _, s)
            | Expr::Unary(_, _, s)
            | Expr::Call(_, _, s)
            | Expr::Member(_, _, s)
            | Expr::Index(_, _, s)
            | Expr::Assign(_, _, s)
            | Expr::StructLit(_, _, s)
            | Expr::ArrayLit(_, s)
            | Expr::Match(_, _, s)
            | Expr::CompoundAssign(_, _, _, s)
            | Expr::PreIncrement(_, s)
            | Expr::PreDecrement(_, s)
            | Expr::PostIncrement(_, s)
            | Expr::PostDecrement(_, s)
            | Expr::NullCoalesce(_, _, s)
            | Expr::OptionalChain(_, _, s)
            | Expr::TrailingClosure(_, _, s)
            | Expr::Nil(s)
            | Expr::Await(_, s)
            | Expr::Range(_, _, s)
            | Expr::Action(_, _, _, s)
            | Expr::Try(_, s) => *s,
            | Expr::SelfRef(s)
            | Expr::Block(_, s)
            | Expr::Typed(_, _, s)
            | Expr::MethodCall(_, _, _, _, s) => *s,
        }
    }
}

impl Stmt {
    /// Source location of the statement, for diagnostics.
    pub fn span(&self) -> Span {
        match self {
            Stmt::Let(l) => l.span,
            Stmt::Expr(e) => e.span(),
            Stmt::Return(r) => r.span,
            Stmt::If(i) => i.span,
            Stmt::While(w) => w.span,
            Stmt::For(f) => f.span,
            Stmt::Block(b) => b.span,
            Stmt::Break(s) | Stmt::Continue(s) => *s,
            Stmt::Guard(g) => g.span,
            Stmt::Defer(d) => d.span,
            Stmt::TryCatch(t) => t.span,
            Stmt::Throw(t) => t.span,
        }
    }
}

impl Literal {
    /// Source location of the literal, for diagnostics.
    pub fn span(&self) -> Span {
        match self {
            Literal::Int(_, s) | Literal::Float(_, s) | Literal::String(_, s) | Literal::Bool(_, s) => *s,
        }
    }
}

/// Literal values
#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Int(i64, Span),
    Float(f64, Span),
    String(String, Span),
    Bool(bool, Span),
}

/// Compound assignment operators
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompoundOp {
    AddEq,    // +=
    SubEq,    // -=
    MulEq,    // *=
    DivEq,    // /=
    ModEq,    // %=
}

/// Binary operators
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    // Comparison
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    // Logical
    And,
    Or,
    // Bitwise
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    ShiftLeft,
    ShiftRight,
}

impl BinOp {
    /// Get operator precedence (higher = binds tighter)
    pub fn precedence(&self) -> u8 {
        match self {
            BinOp::Or => 1,
            BinOp::And => 2,
            BinOp::BitwiseOr => 3,
            BinOp::BitwiseXor => 4,
            BinOp::BitwiseAnd => 5,
            BinOp::Eq | BinOp::Ne => 6,
            BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => 7,
            BinOp::ShiftLeft | BinOp::ShiftRight => 8,
            BinOp::Add | BinOp::Sub => 9,
            BinOp::Mul | BinOp::Div | BinOp::Mod => 10,
        }
    }
}

/// Unary operators
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
    BitwiseNot,
}

/// Match arm
#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Expr,
    pub span: Span,
}

/// Match patterns
#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    Literal(Literal),
    /// A binding, or a payload-less variant case once the checker has resolved it.
    Identifier(String),
    Wildcard,
    /// `Case(p1, p2)` or `Type.Case(p1, p2)`; `Type.Case` without parentheses
    /// has no args.
    Variant(VariantPattern),
}

impl Pattern {
    /// Whether the pattern matches every value (a wildcard or a plain binding).
    pub fn is_irrefutable(&self) -> bool {
        matches!(self, Pattern::Wildcard | Pattern::Identifier(_))
    }
}

/// Variant case pattern. `type_name` is `None` for a bare `Case(...)` until
/// the type checker resolves it from the scrutinee type.
#[derive(Debug, Clone, PartialEq)]
pub struct VariantPattern {
    pub type_name: Option<String>,
    pub case: String,
    pub args: Vec<Pattern>,
    pub span: Span,
}

// === UI-specific AST Nodes ===

/// Variant declaration (REOX enum type)
#[derive(Debug, Clone)]
pub struct VariantDecl {
    pub name: String,
    /// Generic parameters (`variant Option<T>`); empty for ordinary variants.
    pub type_params: Vec<TypeParam>,
    pub cases: Vec<VariantCase>,
    pub is_pub: bool,
    pub span: Span,
}

/// A single variant case. Positional payloads (`Some(T)`) get the field
/// names `_0`, `_1`, ...; named payloads (`Err(message: string)`) keep theirs.
#[derive(Debug, Clone)]
pub struct VariantCase {
    pub name: String,
    pub fields: Vec<Field>,
    pub span: Span,
}

/// Protocol declaration (trait/interface)
#[derive(Debug, Clone)]
pub struct ProtocolDecl {
    pub name: String,
    pub methods: Vec<ProtocolMethod>,
    pub is_pub: bool,
    pub span: Span,
}

/// Protocol method signature (no body): `[static] [mut] fn name(params) [-> Type];`
#[derive(Debug, Clone)]
pub struct ProtocolMethod {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<Type>,
    pub is_mut: bool,
    pub is_static: bool,
    pub span: Span,
}

/// Extension declaration (add methods to existing type)
#[derive(Debug, Clone)]
pub struct ExtensionDecl {
    pub target: String,
    /// Type parameters when extending a generic type: `extension Box<T> { ... }`.
    pub type_params: Vec<TypeParam>,
    pub protocols: Vec<String>,
    pub methods: Vec<FnDecl>,
    pub span: Span,
}

/// Layer declaration (UI view component)
#[derive(Debug, Clone)]
pub struct LayerDecl {
    pub name: String,
    pub fields: Vec<Field>,
    pub signals: Vec<SignalField>,
    pub methods: Vec<FnDecl>,
    pub gestures: Vec<GestureHandler>,
    pub is_pub: bool,
    pub span: Span,
}

/// Signal declaration inside a layer
#[derive(Debug, Clone)]
pub struct SignalField {
    pub name: String,
    pub payload_type: Option<Type>,
    pub span: Span,
}

/// Gesture handler inside a layer
#[derive(Debug, Clone)]
pub struct GestureHandler {
    pub kind: String,
    pub params: Vec<Param>,
    pub body: Block,
    pub span: Span,
}

/// Panel declaration (top-level window)
#[derive(Debug, Clone)]
pub struct PanelDecl {
    pub name: String,
    pub properties: Vec<(String, Expr)>,
    pub methods: Vec<FnDecl>,
    pub is_pub: bool,
    pub span: Span,
}

/// Const declaration
#[derive(Debug, Clone)]
pub struct ConstDecl {
    pub name: String,
    pub ty: Option<Type>,
    pub value: Expr,
    pub is_pub: bool,
    pub span: Span,
}

/// Typealias declaration
#[derive(Debug, Clone)]
pub struct TypealiasDecl {
    pub name: String,
    pub target: Type,
    pub is_pub: bool,
    pub span: Span,
}

/// Emit statement (fire a signal)
#[derive(Debug, Clone)]
pub struct EmitStmt {
    pub signal: String,
    pub value: Option<Expr>,
    pub span: Span,
}
