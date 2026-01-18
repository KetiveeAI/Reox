// REOX Compiler Library
// Zero external dependencies

pub mod lexer;
pub mod parser;
pub mod typechecker;
pub mod codegen;
pub mod profiler;
pub mod interpreter;
pub mod stdlib;
pub mod cli;
pub mod templates;

// Re-export main types for convenience
pub use lexer::{Token, TokenKind, Span, tokenize, LexError};
pub use parser::{Ast, parse};
pub use typechecker::check;
pub use codegen::generate;
pub use interpreter::{Interpreter, Value, eval};
pub use cli::{CliCommand, Args, parse_cli, parse_args};
pub use templates::{Template, ProjectConfig, create_project};
