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

/// Function declaration
#[derive(Debug, Clone)]
pub struct FnDecl {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<Type>,
    pub body: Block,
    pub is_async: bool,
    pub is_pub: bool,
    pub is_static: bool,
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
    pub fields: Vec<Field>,
    pub span: Span,
}

/// Struct field
#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub ty: Type,
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
    Array(Box<Type>),
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
}

/// Literal values
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
pub enum Pattern {
    Literal(Literal),
    Identifier(String),
    Wildcard,
}

// === UI-specific AST Nodes ===

/// Variant declaration (REOX enum type)
#[derive(Debug, Clone)]
pub struct VariantDecl {
    pub name: String,
    pub cases: Vec<VariantCase>,
    pub is_pub: bool,
    pub span: Span,
}

/// A single variant case
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

/// Protocol method signature (no body)
#[derive(Debug, Clone)]
pub struct ProtocolMethod {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<Type>,
    pub span: Span,
}

/// Extension declaration (add methods to existing type)
#[derive(Debug, Clone)]
pub struct ExtensionDecl {
    pub target: String,
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
