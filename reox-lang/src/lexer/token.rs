// REOX Compiler - Token Definitions
// Zero external dependencies

#![allow(dead_code)]

/// Source location for error reporting
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub line: u32,
    pub column: u32,
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(line: u32, column: u32, start: usize, end: usize) -> Self {
        Self { line, column, start, end }
    }
}

impl Default for Span {
    fn default() -> Self {
        Self { line: 1, column: 1, start: 0, end: 0 }
    }
}

/// Token kinds for REOX language
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Keywords
    Fn,
    Let,
    Mut,
    If,
    Else,
    While,
    For,
    In,
    Return,
    Struct,
    Match,
    Import,
    Extern,
    True,
    False,
    
    // REOX-unique keywords (NeolyxOS System UI)
    Kind,       // enum with variants (REOX term for enum)
    Layer,      // UI container (like SwiftUI View, but REOX term)
    Panel,      // Top-level UI window
    Action,     // Closure/lambda
    Maybe,      // Optional type (?-equivalent)
    Effect,     // Side effect marker
    Bind,       // State binding (@State equivalent)
    Emit,       // Event emission
    Signal,     // Reactive signal
    When,       // Pattern in match
    Self_,      // Self reference
    Pub,        // Public visibility
    Async,      // Async function
    Await,      // Await expression
    
    // Swift/C++ Style Keywords
    Guard,      // guard statement (Swift)
    Defer,      // defer statement (Swift/Go)
    Throw,      // throw exception
    Try,        // try block
    Catch,      // catch block
    Where,      // where clause
    Typealias,  // type alias
    Protocol,   // protocol/interface
    Extension,  // type extension
    Static,     // static member
    Const,      // constant
    Nil,        // nil/null value
    
    // Gesture Keywords (REOX-specific)
    Gesture,    // gesture declaration
    OnTap,      // tap gesture
    OnPan,      // pan gesture
    OnSwipe,    // swipe gesture
    OnPinch,    // pinch gesture
    OnRotate,   // rotate gesture

    // Types
    Int,
    Float,
    String,
    Bool,
    Void,

    // Identifiers and Literals
    Ident(String),
    IntLit(i64),
    FloatLit(f64),
    StringLit(String),

    // Operators
    Plus,       // +
    Minus,      // -
    Star,       // *
    Slash,      // /
    Percent,    // %
    Eq,         // =
    EqEq,       // ==
    BangEq,     // !=
    Lt,         // <
    Gt,         // >
    LtEq,       // <=
    GtEq,       // >=
    And,        // &&
    Or,         // ||
    Bang,       // !
    Arrow,      // ->
    FatArrow,   // => (for match arms)
    Question,   // ? (optional chaining)
    Pipe,       // | (union types)
    Ampersand,  // & (reference)
    
    // Compound Assignment (Swift/C/C++ style)
    PlusEq,     // +=
    MinusEq,    // -=
    StarEq,     // *=
    SlashEq,    // /=
    PercentEq,  // %=
    
    // Increment/Decrement (C/C++ style)
    PlusPlus,   // ++
    MinusMinus, // --
    
    // Bitwise Operators (C/C++ style)
    BitwiseAnd, // & (single)
    BitwiseOr,  // | (single)
    BitwiseXor, // ^
    BitwiseNot, // ~
    ShiftLeft,  // <<
    ShiftRight, // >>
    
    // Null Coalescing (Swift style)
    QuestionQuestion, // ??
    QuestionDot,      // ?. (optional chaining)

    // Delimiters
    LParen,     // (
    RParen,     // )
    LBrace,     // {
    RBrace,     // }
    LBracket,   // [
    RBracket,   // ]
    Comma,      // ,
    Semicolon,  // ;
    Colon,      // :
    Dot,        // .
    At,         // @ (for decorators like @Bind)
    Hash,       // # (for system directives)
    DotDot,     // .. (range)

    // Special
    Eof,
}

impl TokenKind {
    /// Check if this token is a keyword
    pub fn is_keyword(&self) -> bool {
        matches!(
            self,
            TokenKind::Fn
                | TokenKind::Let
                | TokenKind::Mut
                | TokenKind::If
                | TokenKind::Else
                | TokenKind::While
                | TokenKind::For
                | TokenKind::In
                | TokenKind::Return
                | TokenKind::Struct
                | TokenKind::Match
                | TokenKind::Import
                | TokenKind::Extern
                | TokenKind::True
                | TokenKind::False
                // REOX-unique keywords
                | TokenKind::Kind
                | TokenKind::Layer
                | TokenKind::Panel
                | TokenKind::Action
                | TokenKind::Maybe
                | TokenKind::Effect
                | TokenKind::Bind
                | TokenKind::Emit
                | TokenKind::Signal
                | TokenKind::When
                | TokenKind::Self_
                | TokenKind::Pub
                | TokenKind::Async
                | TokenKind::Await
                // Swift/C++ style keywords
                | TokenKind::Guard
                | TokenKind::Defer
                | TokenKind::Throw
                | TokenKind::Try
                | TokenKind::Catch
                | TokenKind::Where
                | TokenKind::Typealias
                | TokenKind::Protocol
                | TokenKind::Extension
                | TokenKind::Static
                | TokenKind::Const
                | TokenKind::Nil
                // Gesture keywords
                | TokenKind::Gesture
                | TokenKind::OnTap
                | TokenKind::OnPan
                | TokenKind::OnSwipe
                | TokenKind::OnPinch
                | TokenKind::OnRotate
        )
    }

    /// Keywords lookup table
    pub fn keyword_from_str(s: &str) -> Option<TokenKind> {
        match s {
            "fn" => Some(TokenKind::Fn),
            "let" => Some(TokenKind::Let),
            "mut" => Some(TokenKind::Mut),
            "if" => Some(TokenKind::If),
            "else" => Some(TokenKind::Else),
            "while" => Some(TokenKind::While),
            "for" => Some(TokenKind::For),
            "in" => Some(TokenKind::In),
            "return" => Some(TokenKind::Return),
            "struct" => Some(TokenKind::Struct),
            "match" => Some(TokenKind::Match),
            "import" => Some(TokenKind::Import),
            "extern" => Some(TokenKind::Extern),
            "true" => Some(TokenKind::True),
            "false" => Some(TokenKind::False),
            "int" => Some(TokenKind::Int),
            "float" => Some(TokenKind::Float),
            "string" => Some(TokenKind::String),
            "bool" => Some(TokenKind::Bool),
            "void" => Some(TokenKind::Void),
            // REOX-unique keywords (NeolyxOS)
            "kind" => Some(TokenKind::Kind),
            "layer" => Some(TokenKind::Layer),
            "panel" => Some(TokenKind::Panel),
            "action" => Some(TokenKind::Action),
            "maybe" => Some(TokenKind::Maybe),
            "effect" => Some(TokenKind::Effect),
            "bind" => Some(TokenKind::Bind),
            "emit" => Some(TokenKind::Emit),
            "signal" => Some(TokenKind::Signal),
            "when" => Some(TokenKind::When),
            "self" => Some(TokenKind::Self_),
            "pub" => Some(TokenKind::Pub),
            "async" => Some(TokenKind::Async),
            "await" => Some(TokenKind::Await),
            // Swift/C++ style keywords
            "guard" => Some(TokenKind::Guard),
            "defer" => Some(TokenKind::Defer),
            "throw" => Some(TokenKind::Throw),
            "try" => Some(TokenKind::Try),
            "catch" => Some(TokenKind::Catch),
            "where" => Some(TokenKind::Where),
            "typealias" => Some(TokenKind::Typealias),
            "protocol" => Some(TokenKind::Protocol),
            "extension" => Some(TokenKind::Extension),
            "static" => Some(TokenKind::Static),
            "const" => Some(TokenKind::Const),
            "nil" => Some(TokenKind::Nil),
            // Gesture keywords
            "gesture" => Some(TokenKind::Gesture),
            "on_tap" => Some(TokenKind::OnTap),
            "on_pan" => Some(TokenKind::OnPan),
            "on_swipe" => Some(TokenKind::OnSwipe),
            "on_pinch" => Some(TokenKind::OnPinch),
            "on_rotate" => Some(TokenKind::OnRotate),
            _ => None,
        }
    }
}

/// A token with its kind and source location
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }

    pub fn eof(pos: usize) -> Self {
        Self {
            kind: TokenKind::Eof,
            span: Span::new(0, 0, pos, pos),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyword_lookup() {
        assert_eq!(TokenKind::keyword_from_str("fn"), Some(TokenKind::Fn));
        assert_eq!(TokenKind::keyword_from_str("let"), Some(TokenKind::Let));
        assert_eq!(TokenKind::keyword_from_str("if"), Some(TokenKind::If));
        assert_eq!(TokenKind::keyword_from_str("unknown"), None);
    }

    #[test]
    fn test_is_keyword() {
        assert!(TokenKind::Fn.is_keyword());
        assert!(TokenKind::Let.is_keyword());
        assert!(!TokenKind::Plus.is_keyword());
        assert!(!TokenKind::Eof.is_keyword());
    }
}
