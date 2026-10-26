// REOX Compiler Library
// Zero external dependencies

pub mod diagnostic;
pub mod lexer;
pub mod parser;
pub mod typechecker;
pub mod codegen;
pub mod profiler;
pub mod interpreter;
pub mod stdlib;
pub mod cli;
pub mod pkg;
pub mod templates;
pub mod fmt;

// Re-export main types for convenience
pub use diagnostic::{Diagnostic, Severity};
pub use lexer::{Token, TokenKind, Span, tokenize, LexError};
pub use parser::{Ast, parse, ParseError};
pub use typechecker::{check, TypeError};
pub use codegen::{generate, CodegenError};
pub use interpreter::{Interpreter, Value, eval, RuntimeError};
pub use cli::{CliCommand, Args, parse_cli, parse_args, check_source, compile_source, check_file, compile_file, report_diagnostics, report_program_diagnostics};
pub use modules::{SourceMap, ModuleMap, Program, load_program};
pub use templates::{Template, ProjectConfig, create_project};
