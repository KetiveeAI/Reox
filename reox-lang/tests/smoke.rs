// REOX Compiler Smoke Tests
// Verifies core lexer, parser, and interpreter functionality

use reoxc::lexer::{tokenize, TokenKind};
use reoxc::parser::Parser;
use reoxc::interpreter::{Interpreter, Value};

#[test]
fn lexer_tokenizes_basic_code() {
    let source = "let x = 42;";
    let result = tokenize(source);
    assert!(result.is_ok(), "Lexer should produce tokens");
    let tokens = result.unwrap();
    assert!(!tokens.is_empty(), "Lexer should produce tokens");
    assert!(tokens.len() >= 4, "Expected at least 4 tokens for 'let x = 42;'");
}

#[test]
fn lexer_handles_keywords() {
    let source = "fn if else while for return let mut struct import extern";
    let result = tokenize(source);
    assert!(result.is_ok(), "Should tokenize keywords");
    let tokens = result.unwrap();
    assert!(tokens.len() >= 11, "Should tokenize all keywords");
}

#[test]
fn lexer_handles_operators() {
    let source = "+ - * / == != < > <= >= && || !";
    let result = tokenize(source);
    assert!(result.is_ok(), "Should tokenize operators");
    let tokens = result.unwrap();
    assert!(tokens.len() >= 12, "Should tokenize all operators");
}

#[test]
fn parser_parses_function() {
    let source = "fn add(a: int, b: int) -> int { return a + b; }";
    let tokens = tokenize(source).unwrap();
    let mut parser = Parser::new(&tokens);
    let result = parser.parse_program();
    assert!(result.is_ok(), "Should parse function declaration");
    let ast = result.unwrap();
    assert!(!ast.declarations.is_empty(), "Should have function declaration");
}

#[test]
fn parser_parses_let_statement() {
    let source = "fn main() { let x = 10; }";
    let tokens = tokenize(source).unwrap();
    let mut parser = Parser::new(&tokens);
    let result = parser.parse_program();
    assert!(result.is_ok(), "Should parse function with let");
    let ast = result.unwrap();
    assert!(!ast.declarations.is_empty(), "Should have function declaration");
}

#[test]
fn parser_parses_if_else() {
    let source = "fn test() { if true { return 1; } else { return 0; } }";
    let tokens = tokenize(source).unwrap();
    let mut parser = Parser::new(&tokens);
    let result = parser.parse_program();
    assert!(result.is_ok(), "Should parse if-else");
    let ast = result.unwrap();
    assert!(!ast.declarations.is_empty(), "Should have function declaration");
}

#[test]
fn interpreter_evaluates_arithmetic() {
    let source = "fn main() -> int { return 2 + 3 * 4; }";
    let tokens = tokenize(source).unwrap();
    let mut parser = Parser::new(&tokens);
    let ast = parser.parse_program().unwrap();
    let mut interp = Interpreter::new();
    let result = interp.eval(&ast);
    assert!(result.is_ok(), "Should evaluate arithmetic");
    if let Ok(Value::Int(n)) = result {
        assert_eq!(n, 14, "2 + 3 * 4 should equal 14");
    }
}

#[test]
fn interpreter_handles_variables() {
    let source = "fn main() -> int { let x = 5; let y = 10; return x + y; }";
    let tokens = tokenize(source).unwrap();
    let mut parser = Parser::new(&tokens);
    let ast = parser.parse_program().unwrap();
    let mut interp = Interpreter::new();
    let result = interp.eval(&ast);
    assert!(result.is_ok(), "Should handle variables");
    if let Ok(Value::Int(n)) = result {
        assert_eq!(n, 15, "5 + 10 should equal 15");
    }
}

#[test]
fn interpreter_handles_conditionals() {
    let source = "fn main() -> int { if 5 > 3 { return 1; } else { return 0; } }";
    let tokens = tokenize(source).unwrap();
    let mut parser = Parser::new(&tokens);
    let ast = parser.parse_program().unwrap();
    let mut interp = Interpreter::new();
    let result = interp.eval(&ast);
    assert!(result.is_ok(), "Should handle conditionals");
    if let Ok(Value::Int(n)) = result {
        assert_eq!(n, 1, "5 > 3 should return 1");
    }
}

#[test]
fn interpreter_handles_string_literals() {
    let source = r#"fn main() -> string { return "hello"; }"#;
    let tokens = tokenize(source).unwrap();
    let mut parser = Parser::new(&tokens);
    let ast = parser.parse_program().unwrap();
    let mut interp = Interpreter::new();
    let result = interp.eval(&ast);
    assert!(result.is_ok(), "Should handle strings");
}
