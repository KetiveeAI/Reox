// REOX Compiler - Lexer
// Transforms source text into tokens
// Zero external dependencies

#![allow(unused_assignments)]

mod token;

pub use token::{Token, TokenKind, Span, StringPart};

/// Lexer error, located by a source span
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexError {
    pub message: String,
    pub span: Span,
}

impl LexError {
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }

    /// Build an error that points at a single source position
    pub fn at(message: impl Into<String>, line: u32, column: u32, pos: usize) -> Self {
        Self::new(message, Span::new(line, column, pos, pos + 1))
    }

    pub fn line(&self) -> u32 {
        self.span.line
    }

    pub fn column(&self) -> u32 {
        self.span.column
    }

    /// Format error for display
    pub fn display(&self) -> String {
        format!("error[{}:{}]: {}", self.span.line, self.span.column, self.message)
    }
}

impl std::fmt::Display for LexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.display())
    }
}

impl std::error::Error for LexError {}

/// Longest source accepted by the lexer, in bytes. Spans store byte offsets as
/// `usize`, so this is a sanity bound rather than a technical one.
pub const MAX_SOURCE_BYTES: usize = 64 * 1024 * 1024;

/// REOX Lexer
pub struct Lexer<'a> {
    source: &'a str,
    chars: std::iter::Peekable<std::str::CharIndices<'a>>,
    line: u32,
    column: u32,
    current_pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            chars: source.char_indices().peekable(),
            line: 1,
            column: 1,
            current_pos: 0,
        }
    }

    /// Advance to next character
    fn advance(&mut self) -> Option<(usize, char)> {
        if let Some((pos, ch)) = self.chars.next() {
            self.current_pos = pos;
            if ch == '\n' {
                self.line += 1;
                self.column = 1;
            } else {
                self.column += 1;
            }
            Some((pos, ch))
        } else {
            None
        }
    }

    /// Peek at next character without consuming
    fn peek(&mut self) -> Option<char> {
        self.chars.peek().map(|(_, ch)| *ch)
    }

    /// Peek at character after next
    fn peek_next(&self) -> Option<char> {
        let mut iter = self.chars.clone();
        iter.next();
        iter.peek().map(|(_, ch)| *ch)
    }

    /// Skip whitespace and comments. An unterminated block comment is an error.
    fn skip_whitespace_and_comments(&mut self) -> Result<(), LexError> {
        loop {
            match self.peek() {
                Some(' ') | Some('\t') | Some('\r') | Some('\n') => {
                    self.advance();
                }
                Some('/') => {
                    if self.peek_next() == Some('/') {
                        // Line comment
                        self.advance(); // /
                        self.advance(); // /
                        while let Some(ch) = self.peek() {
                            if ch == '\n' {
                                break;
                            }
                            self.advance();
                        }
                    } else if self.peek_next() == Some('*') {
                        // Block comment (nestable)
                        let start_line = self.line;
                        let start_col = self.column;
                        let start_pos = self.chars.peek().map(|(p, _)| *p).unwrap_or(self.current_pos);
                        self.advance(); // /
                        self.advance(); // *
                        let mut depth = 1;
                        while depth > 0 {
                            match self.advance() {
                                Some((_, '*')) if self.peek() == Some('/') => {
                                    self.advance();
                                    depth -= 1;
                                }
                                Some((_, '/')) if self.peek() == Some('*') => {
                                    self.advance();
                                    depth += 1;
                                }
                                None => {
                                    return Err(LexError::new(
                                        "unterminated block comment",
                                        Span::new(start_line, start_col, start_pos, start_pos + 2),
                                    ));
                                }
                                _ => {}
                            }
                        }
                    } else {
                        break;
                    }
                }
                _ => break,
            }
        }
        Ok(())
    }

    /// Scan an identifier or keyword
    fn scan_identifier(&mut self, start_pos: usize, start_line: u32, start_col: u32) -> Token {
        // `end` is the byte offset one past the last character of the identifier.
        // Identifiers may contain multi-byte letters, so track the full width.
        let mut end = self.source[start_pos..]
            .chars()
            .next()
            .map(|c| start_pos + c.len_utf8())
            .unwrap_or(start_pos + 1);

        while let Some(ch) = self.peek() {
            if ch.is_alphanumeric() || ch == '_' {
                if let Some((pos, c)) = self.advance() {
                    end = pos + c.len_utf8();
                }
            } else {
                break;
            }
        }

        let text = self.source.get(start_pos..end).unwrap_or("");
        let span = Span::new(start_line, start_col, start_pos, end);

        let kind = TokenKind::keyword_from_str(text)
            .unwrap_or_else(|| TokenKind::Ident(text.to_string()));

        Token::new(kind, span)
    }

    /// Scan a number literal (supports decimal, hex 0x, and binary 0b)
    fn scan_number(&mut self, start_pos: usize, start_line: u32, start_col: u32) -> Result<Token, LexError> {
        let mut end_pos = start_pos;
        let mut is_float = false;
        let mut is_hex = false;
        let mut is_bin = false;

        // Check for hex prefix (0x or 0X)
        if self.source[start_pos..].starts_with("0x") || self.source[start_pos..].starts_with("0X") {
            is_hex = true;
            self.advance(); // consume 'x' or 'X'
            end_pos = self.current_pos;
            
            while let Some(ch) = self.peek() {
                if ch.is_ascii_hexdigit() {
                    if let Some((pos, _)) = self.advance() { end_pos = pos; }
                } else { break; }
            }
        } else if self.source[start_pos..].starts_with("0b") || self.source[start_pos..].starts_with("0B") {
            is_bin = true;
            self.advance(); // consume 'b' or 'B'
            end_pos = self.current_pos;
            
            while let Some(ch) = self.peek() {
                if ch == '0' || ch == '1' || ch == '_' {
                    if ch != '_' { if let Some((pos, _)) = self.advance() { end_pos = pos; } else { break; } }
                    else { self.advance(); }
                } else { break; }
            }
        } else {
            // Decimal number
            while let Some(ch) = self.peek() {
                if ch.is_ascii_digit() {
                    if let Some((pos, _)) = self.advance() {
                        end_pos = pos;
                    }
                } else if ch == '.' && !is_float {
                    if let Some(next) = self.peek_next() {
                        if next.is_ascii_digit() {
                            is_float = true;
                            if let Some((pos, _)) = self.advance() {
                                end_pos = pos;
                            }
                        } else { break; }
                    } else { break; }
                } else { break; }
            }
        }

        let text = &self.source[start_pos..=end_pos];
        let span = Span::new(start_line, start_col, start_pos, end_pos + 1);

        if is_hex {
            let hex_digits = &text[2..];
            match i64::from_str_radix(hex_digits, 16) {
                Ok(val) => Ok(Token::new(TokenKind::IntLit(val), span)),
                Err(_) => Err(LexError::new(format!("invalid hex literal: {}", text), span)),
            }
        } else if is_bin {
            let bin_digits: String = text[2..].chars().filter(|c| *c != '_').collect();
            match i64::from_str_radix(&bin_digits, 2) {
                Ok(val) => Ok(Token::new(TokenKind::IntLit(val), span)),
                Err(_) => Err(LexError::new(format!("invalid binary literal: {}", text), span)),
            }
        } else if is_float {
            match text.parse::<f64>() {
                Ok(val) => Ok(Token::new(TokenKind::FloatLit(val), span)),
                Err(_) => Err(LexError::new(format!("invalid float literal: {}", text), span)),
            }
        } else {
            match text.parse::<i64>() {
                Ok(val) => Ok(Token::new(TokenKind::IntLit(val), span)),
                Err(_) => Err(LexError::new(format!("invalid integer literal: {}", text), span)),
            }
        }
    }


    /// Scan a string literal, detecting \(expr) interpolations
    fn scan_string(&mut self, start_pos: usize, start_line: u32, start_col: u32) -> Result<Token, LexError> {
        let mut current_lit = String::new();
        let mut parts: Vec<token::StringPart> = Vec::new();
        let mut end_pos = start_pos;
        let mut has_interp = false;

        loop {
            match self.advance() {
                Some((pos, '"')) => {
                    end_pos = pos;
                    break;
                }
                Some((_, '\\')) => {
                    match self.peek() {
                        Some('(') => {
                            // String interpolation \(expr)
                            has_interp = true;
                            self.advance(); // consume '('
                            // Flush current literal part
                            if !current_lit.is_empty() {
                                parts.push(crate::lexer::StringPart::Literal(current_lit.clone()));
                                current_lit.clear();
                            }
                            // Read identifier/expression until closing ')'
                            let mut expr = String::new();
                            let mut depth = 1;
                            loop {
                                match self.advance() {
                                    Some((_, ')')) => {
                                        depth -= 1;
                                        if depth == 0 { break; }
                                        expr.push(')');
                                    }
                                    Some((_, '(')) => {
                                        depth += 1;
                                        expr.push('(');
                                    }
                                    Some((_, ch)) => expr.push(ch),
                                    None => return Err(LexError::at(
                                        "unterminated string interpolation",
                                        start_line, start_col, start_pos,
                                    )),
                                }
                            }
                            parts.push(token::StringPart::Expr(expr.trim().to_string()));
                        }
                        _ => {
                            // Regular escape sequence
                            let (esc_line, esc_col, esc_pos) = (self.line, self.column.saturating_sub(1), self.current_pos);
                            match self.advance() {
                                Some((_, 'n')) => current_lit.push('\n'),
                                Some((_, 't')) => current_lit.push('\t'),
                                Some((_, 'r')) => current_lit.push('\r'),
                                Some((_, '\\')) => current_lit.push('\\'),
                                Some((_, '"')) => current_lit.push('"'),
                                Some((_, '0')) => current_lit.push('\0'),
                                Some((_, ch)) => {
                                    return Err(LexError::at(
                                        format!("invalid escape sequence: \\{}", ch),
                                        esc_line, esc_col, esc_pos,
                                    ));
                                }
                                None => {
                                    return Err(LexError::at(
                                        "unexpected end of file in escape sequence",
                                        esc_line, esc_col, esc_pos,
                                    ));
                                }
                            }
                        }
                    }
                }
                Some((_, '\n')) => {
                    return Err(LexError::at(
                        "unterminated string literal",
                        start_line, start_col, start_pos,
                    ));
                }
                Some((_, ch)) => {
                    current_lit.push(ch);
                }
                None => {
                    return Err(LexError::at(
                        "unterminated string literal",
                        start_line, start_col, start_pos,
                    ));
                }
            }
        }

        let span = Span::new(start_line, start_col, start_pos, end_pos + 1);

        if has_interp {
            // Flush trailing literal
            if !current_lit.is_empty() {
                parts.push(crate::lexer::token::StringPart::Literal(current_lit));
            }
            Ok(Token::new(TokenKind::StringInterp(parts), span))
        } else {
            Ok(Token::new(TokenKind::StringLit(current_lit), span))
        }
    }


    /// Get next token
    fn next_token(&mut self) -> Result<Token, LexError> {
        self.skip_whitespace_and_comments()?;

        let start_line = self.line;
        let start_col = self.column;

        match self.advance() {
            None => Ok(Token::eof_at(self.line, self.column, self.source.len())),

            Some((pos, ch)) => {
                let span = Span::new(start_line, start_col, pos, pos + 1);

                match ch {
                    // Single character tokens
                    '(' => Ok(Token::new(TokenKind::LParen, span)),
                    ')' => Ok(Token::new(TokenKind::RParen, span)),
                    '{' => Ok(Token::new(TokenKind::LBrace, span)),
                    '}' => Ok(Token::new(TokenKind::RBrace, span)),
                    '[' => Ok(Token::new(TokenKind::LBracket, span)),
                    ']' => Ok(Token::new(TokenKind::RBracket, span)),
                    ',' => Ok(Token::new(TokenKind::Comma, span)),
                    ';' => Ok(Token::new(TokenKind::Semicolon, span)),
                    ':' => Ok(Token::new(TokenKind::Colon, span)),
                    '@' => Ok(Token::new(TokenKind::At, span)),
                    '~' => Ok(Token::new(TokenKind::BitwiseNot, span)),
                    '^' => Ok(Token::new(TokenKind::BitwiseXor, span)),
                    
                    // Dot - check for range (..)
                    '.' => {
                        if self.peek() == Some('.') {
                            self.advance();
                            let span = Span::new(start_line, start_col, pos, pos + 2);
                            Ok(Token::new(TokenKind::DotDot, span))
                        } else {
                            Ok(Token::new(TokenKind::Dot, span))
                        }
                    }

                    // Plus - check for += or ++
                    '+' => {
                        match self.peek() {
                            Some('=') => {
                                self.advance();
                                let span = Span::new(start_line, start_col, pos, pos + 2);
                                Ok(Token::new(TokenKind::PlusEq, span))
                            }
                            Some('+') => {
                                self.advance();
                                let span = Span::new(start_line, start_col, pos, pos + 2);
                                Ok(Token::new(TokenKind::PlusPlus, span))
                            }
                            _ => Ok(Token::new(TokenKind::Plus, span)),
                        }
                    }
                    
                    // Star - check for *=
                    '*' => {
                        if self.peek() == Some('=') {
                            self.advance();
                            let span = Span::new(start_line, start_col, pos, pos + 2);
                            Ok(Token::new(TokenKind::StarEq, span))
                        } else {
                            Ok(Token::new(TokenKind::Star, span))
                        }
                    }
                    
                    // Percent - check for %=
                    '%' => {
                        if self.peek() == Some('=') {
                            self.advance();
                            let span = Span::new(start_line, start_col, pos, pos + 2);
                            Ok(Token::new(TokenKind::PercentEq, span))
                        } else {
                            Ok(Token::new(TokenKind::Percent, span))
                        }
                    }

                    // Minus - check for ->, --, -=
                    '-' => {
                        match self.peek() {
                            Some('>') => {
                                self.advance();
                                let span = Span::new(start_line, start_col, pos, pos + 2);
                                Ok(Token::new(TokenKind::Arrow, span))
                            }
                            Some('-') => {
                                self.advance();
                                let span = Span::new(start_line, start_col, pos, pos + 2);
                                Ok(Token::new(TokenKind::MinusMinus, span))
                            }
                            Some('=') => {
                                self.advance();
                                let span = Span::new(start_line, start_col, pos, pos + 2);
                                Ok(Token::new(TokenKind::MinusEq, span))
                            }
                            _ => Ok(Token::new(TokenKind::Minus, span)),
                        }
                    }
                    
                    // Slash - check for /=
                    '/' => {
                        if self.peek() == Some('=') {
                            self.advance();
                            let span = Span::new(start_line, start_col, pos, pos + 2);
                            Ok(Token::new(TokenKind::SlashEq, span))
                        } else {
                            Ok(Token::new(TokenKind::Slash, span))
                        }
                    }
                    
                    // Equals - check for == or =>
                    '=' => {
                        match self.peek() {
                            Some('=') => {
                                self.advance();
                                let span = Span::new(start_line, start_col, pos, pos + 2);
                                Ok(Token::new(TokenKind::EqEq, span))
                            }
                            Some('>') => {
                                self.advance();
                                let span = Span::new(start_line, start_col, pos, pos + 2);
                                Ok(Token::new(TokenKind::FatArrow, span))
                            }
                            _ => Ok(Token::new(TokenKind::Eq, span)),
                        }
                    }
                    
                    // Bang - check for !=
                    '!' => {
                        if self.peek() == Some('=') {
                            self.advance();
                            let span = Span::new(start_line, start_col, pos, pos + 2);
                            Ok(Token::new(TokenKind::BangEq, span))
                        } else {
                            Ok(Token::new(TokenKind::Bang, span))
                        }
                    }
                    
                    // Less than - check for <= or <<
                    '<' => {
                        match self.peek() {
                            Some('=') => {
                                self.advance();
                                let span = Span::new(start_line, start_col, pos, pos + 2);
                                Ok(Token::new(TokenKind::LtEq, span))
                            }
                            Some('<') => {
                                self.advance();
                                let span = Span::new(start_line, start_col, pos, pos + 2);
                                Ok(Token::new(TokenKind::ShiftLeft, span))
                            }
                            _ => Ok(Token::new(TokenKind::Lt, span)),
                        }
                    }
                    
                    // Greater than - check for >= or >>
                    '>' => {
                        match self.peek() {
                            Some('=') => {
                                self.advance();
                                let span = Span::new(start_line, start_col, pos, pos + 2);
                                Ok(Token::new(TokenKind::GtEq, span))
                            }
                            Some('>') => {
                                self.advance();
                                let span = Span::new(start_line, start_col, pos, pos + 2);
                                Ok(Token::new(TokenKind::ShiftRight, span))
                            }
                            _ => Ok(Token::new(TokenKind::Gt, span)),
                        }
                    }
                    
                    // Ampersand - check for && (logical) or & (bitwise)
                    '&' => {
                        if self.peek() == Some('&') {
                            self.advance();
                            let span = Span::new(start_line, start_col, pos, pos + 2);
                            Ok(Token::new(TokenKind::And, span))
                        } else {
                            Ok(Token::new(TokenKind::BitwiseAnd, span))
                        }
                    }
                    
                    // Pipe - check for || (logical) or | (bitwise)
                    '|' => {
                        if self.peek() == Some('|') {
                            self.advance();
                            let span = Span::new(start_line, start_col, pos, pos + 2);
                            Ok(Token::new(TokenKind::Or, span))
                        } else {
                            Ok(Token::new(TokenKind::BitwiseOr, span))
                        }
                    }
                    
                    // Question mark - check for ?? or ?. or single ?
                    '?' => {
                        match self.peek() {
                            Some('?') => {
                                self.advance();
                                let span = Span::new(start_line, start_col, pos, pos + 2);
                                Ok(Token::new(TokenKind::QuestionQuestion, span))
                            }
                            Some('.') => {
                                self.advance();
                                let span = Span::new(start_line, start_col, pos, pos + 2);
                                Ok(Token::new(TokenKind::QuestionDot, span))
                            }
                            _ => Ok(Token::new(TokenKind::Question, span)),
                        }
                    }

                    // String literals
                    '"' => self.scan_string(pos, start_line, start_col),

                    // Numbers
                    '0'..='9' => self.scan_number(pos, start_line, start_col),

                    // Identifiers and keywords
                    'a'..='z' | 'A'..='Z' | '_' => {
                        Ok(self.scan_identifier(pos, start_line, start_col))
                    }

                    _ => Err(LexError::new(
                        format!("unexpected character: '{}'", ch),
                        Span::new(start_line, start_col, pos, pos + ch.len_utf8()),
                    )),
                }
            }
        }
    }
}

/// Tokenize source code into a vector of tokens
pub fn tokenize(source: &str) -> Result<Vec<Token>, LexError> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err(LexError::new(
            format!("source is {} bytes, larger than the {} byte limit", source.len(), MAX_SOURCE_BYTES),
            Span::default(),
        ));
    }
    let mut lexer = Lexer::new(source);
    let mut tokens = Vec::new();

    loop {
        let token = lexer.next_token()?;
        let is_eof = token.kind == TokenKind::Eof;
        tokens.push(token);
        if is_eof {
            break;
        }
    }

    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_source() {
        let tokens = tokenize("").unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, TokenKind::Eof);
    }

    #[test]
    fn test_keywords() {
        let tokens = tokenize("fn let mut if else while for return struct").unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Fn);
        assert_eq!(tokens[1].kind, TokenKind::Let);
        assert_eq!(tokens[2].kind, TokenKind::Mut);
        assert_eq!(tokens[3].kind, TokenKind::If);
        assert_eq!(tokens[4].kind, TokenKind::Else);
        assert_eq!(tokens[5].kind, TokenKind::While);
        assert_eq!(tokens[6].kind, TokenKind::For);
        assert_eq!(tokens[7].kind, TokenKind::Return);
        assert_eq!(tokens[8].kind, TokenKind::Struct);
    }

    #[test]
    fn test_operators() {
        let tokens = tokenize("+ - * / = == != < > <= >= && ||").unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Plus);
        assert_eq!(tokens[1].kind, TokenKind::Minus);
        assert_eq!(tokens[2].kind, TokenKind::Star);
        assert_eq!(tokens[3].kind, TokenKind::Slash);
        assert_eq!(tokens[4].kind, TokenKind::Eq);
        assert_eq!(tokens[5].kind, TokenKind::EqEq);
        assert_eq!(tokens[6].kind, TokenKind::BangEq);
        assert_eq!(tokens[7].kind, TokenKind::Lt);
        assert_eq!(tokens[8].kind, TokenKind::Gt);
        assert_eq!(tokens[9].kind, TokenKind::LtEq);
        assert_eq!(tokens[10].kind, TokenKind::GtEq);
        assert_eq!(tokens[11].kind, TokenKind::And);
        assert_eq!(tokens[12].kind, TokenKind::Or);
    }

    #[test]
    fn test_delimiters() {
        let tokens = tokenize("( ) { } [ ] , ; : . ->").unwrap();
        assert_eq!(tokens[0].kind, TokenKind::LParen);
        assert_eq!(tokens[1].kind, TokenKind::RParen);
        assert_eq!(tokens[2].kind, TokenKind::LBrace);
        assert_eq!(tokens[3].kind, TokenKind::RBrace);
        assert_eq!(tokens[4].kind, TokenKind::LBracket);
        assert_eq!(tokens[5].kind, TokenKind::RBracket);
        assert_eq!(tokens[6].kind, TokenKind::Comma);
        assert_eq!(tokens[7].kind, TokenKind::Semicolon);
        assert_eq!(tokens[8].kind, TokenKind::Colon);
        assert_eq!(tokens[9].kind, TokenKind::Dot);
        assert_eq!(tokens[10].kind, TokenKind::Arrow);
    }

    #[test]
    fn test_integers() {
        let tokens = tokenize("0 42 12345").unwrap();
        assert_eq!(tokens[0].kind, TokenKind::IntLit(0));
        assert_eq!(tokens[1].kind, TokenKind::IntLit(42));
        assert_eq!(tokens[2].kind, TokenKind::IntLit(12345));
    }

    #[test]
    fn test_floats() {
        let tokens = tokenize("3.75 0.5 123.456").unwrap();
        assert_eq!(tokens[0].kind, TokenKind::FloatLit(3.75));
        assert_eq!(tokens[1].kind, TokenKind::FloatLit(0.5));
        assert_eq!(tokens[2].kind, TokenKind::FloatLit(123.456));
    }

    #[test]
    fn test_strings() {
        let tokens = tokenize(r#""hello" "world""#).unwrap();
        assert_eq!(tokens[0].kind, TokenKind::StringLit("hello".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::StringLit("world".to_string()));
    }

    #[test]
    fn test_string_escapes() {
        let tokens = tokenize(r#""hello\nworld\ttab""#).unwrap();
        assert_eq!(tokens[0].kind, TokenKind::StringLit("hello\nworld\ttab".to_string()));
    }

    #[test]
    fn test_identifiers() {
        let tokens = tokenize("foo bar_baz _private camelCase").unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Ident("foo".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::Ident("bar_baz".to_string()));
        assert_eq!(tokens[2].kind, TokenKind::Ident("_private".to_string()));
        assert_eq!(tokens[3].kind, TokenKind::Ident("camelCase".to_string()));
    }

    #[test]
    fn test_line_comments() {
        let tokens = tokenize("foo // comment\nbar").unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Ident("foo".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::Ident("bar".to_string()));
    }

    #[test]
    fn test_block_comments() {
        let tokens = tokenize("foo /* comment */ bar").unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Ident("foo".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::Ident("bar".to_string()));
    }

    #[test]
    fn test_full_function() {
        let source = r#"
            fn add(a: int, b: int) -> int {
                return a + b;
            }
        "#;
        let tokens = tokenize(source).unwrap();

        assert_eq!(tokens[0].kind, TokenKind::Fn);
        assert_eq!(tokens[1].kind, TokenKind::Ident("add".to_string()));
        assert_eq!(tokens[2].kind, TokenKind::LParen);
        assert_eq!(tokens[3].kind, TokenKind::Ident("a".to_string()));
        assert_eq!(tokens[4].kind, TokenKind::Colon);
        assert_eq!(tokens[5].kind, TokenKind::Int);
    }

    #[test]
    fn test_struct_definition() {
        let source = r#"
            struct Point {
                x: int,
                y: int
            }
        "#;
        let tokens = tokenize(source).unwrap();

        assert_eq!(tokens[0].kind, TokenKind::Struct);
        assert_eq!(tokens[1].kind, TokenKind::Ident("Point".to_string()));
        assert_eq!(tokens[2].kind, TokenKind::LBrace);
    }

    #[test]
    fn test_error_unterminated_string() {
        let result = tokenize("\"unterminated");
        assert!(result.is_err());
    }

    // === Ambiguous Token Tests (Critical for longest-match rule) ===
    
    #[test]
    fn test_question_dot_vs_question_and_dot() {
        // "?." should be QuestionDot, not Question + Dot
        let tokens = tokenize("a?.b").unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Ident("a".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::QuestionDot);
        assert_eq!(tokens[2].kind, TokenKind::Ident("b".to_string()));
    }
    
    #[test]
    fn test_question_dot_spaced() {
        // "? ." with space should be Question + Dot
        let tokens = tokenize("a ? .b").unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Ident("a".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::Question);
        assert_eq!(tokens[2].kind, TokenKind::Dot);
        assert_eq!(tokens[3].kind, TokenKind::Ident("b".to_string()));
    }

    #[test]
    fn test_question_question_vs_two_questions() {
        // "??" should be QuestionQuestion, not Question + Question
        let tokens = tokenize("a ?? b").unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Ident("a".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::QuestionQuestion);
        assert_eq!(tokens[2].kind, TokenKind::Ident("b".to_string()));
    }
    
    #[test]
    fn test_question_question_adjacent() {
        // "a??b" should also work
        let tokens = tokenize("a??b").unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Ident("a".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::QuestionQuestion);
        assert_eq!(tokens[2].kind, TokenKind::Ident("b".to_string()));
    }

    #[test]
    fn test_shift_right_vs_two_greater() {
        // ">>" should be ShiftRight, not Gt + Gt
        let tokens = tokenize("a >> 2").unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Ident("a".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::ShiftRight);
        assert_eq!(tokens[2].kind, TokenKind::IntLit(2));
    }
    
    #[test]
    fn test_shift_left_vs_two_less() {
        // "<<" should be ShiftLeft, not Lt + Lt
        let tokens = tokenize("a << 2").unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Ident("a".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::ShiftLeft);
        assert_eq!(tokens[2].kind, TokenKind::IntLit(2));
    }
    
    #[test]
    fn test_compound_assignment_operators() {
        let tokens = tokenize("a += b -= c *= d /= e %= f").unwrap();
        assert_eq!(tokens[1].kind, TokenKind::PlusEq);
        assert_eq!(tokens[3].kind, TokenKind::MinusEq);
        assert_eq!(tokens[5].kind, TokenKind::StarEq);
        assert_eq!(tokens[7].kind, TokenKind::SlashEq);
        assert_eq!(tokens[9].kind, TokenKind::PercentEq);
    }
    
    #[test]
    fn test_increment_decrement_operators() {
        let tokens = tokenize("a++ b-- ++c --d").unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Ident("a".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::PlusPlus);
        assert_eq!(tokens[2].kind, TokenKind::Ident("b".to_string()));
        assert_eq!(tokens[3].kind, TokenKind::MinusMinus);
        assert_eq!(tokens[4].kind, TokenKind::PlusPlus);
        assert_eq!(tokens[5].kind, TokenKind::Ident("c".to_string()));
        assert_eq!(tokens[6].kind, TokenKind::MinusMinus);
        assert_eq!(tokens[7].kind, TokenKind::Ident("d".to_string()));
    }
    
    #[test]
    fn test_bitwise_operators() {
        let tokens = tokenize("a & b | c ^ d ~ e").unwrap();
        assert_eq!(tokens[1].kind, TokenKind::BitwiseAnd);
        assert_eq!(tokens[3].kind, TokenKind::BitwiseOr);
        assert_eq!(tokens[5].kind, TokenKind::BitwiseXor);
        assert_eq!(tokens[7].kind, TokenKind::BitwiseNot);
    }
    
    #[test]
    fn test_fat_arrow_vs_eq_and_gt() {
        // "=>" should be FatArrow, not Eq + Gt
        let tokens = tokenize("a => b").unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Ident("a".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::FatArrow);
        assert_eq!(tokens[2].kind, TokenKind::Ident("b".to_string()));
    }
    
    #[test]
    fn test_swift_keywords() {
        let tokens = tokenize("guard defer throw try catch where typealias nil").unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Guard);
        assert_eq!(tokens[1].kind, TokenKind::Defer);
        assert_eq!(tokens[2].kind, TokenKind::Throw);
        assert_eq!(tokens[3].kind, TokenKind::Try);
        assert_eq!(tokens[4].kind, TokenKind::Catch);
        assert_eq!(tokens[5].kind, TokenKind::Where);
        assert_eq!(tokens[6].kind, TokenKind::Typealias);
        assert_eq!(tokens[7].kind, TokenKind::Nil);
    }

    // === Hardening: error paths ===

    #[test]
    fn unterminated_block_comment_is_an_error() {
        let err = tokenize("fn main() { } /* never closed").unwrap_err();
        assert_eq!(err.message, "unterminated block comment");
        assert_eq!((err.line(), err.column()), (1, 15));
    }

    #[test]
    fn nested_unterminated_block_comment_is_an_error() {
        let err = tokenize("/* outer /* inner */ still open").unwrap_err();
        assert_eq!(err.message, "unterminated block comment");
    }

    #[test]
    fn non_ascii_identifier_is_sliced_on_char_boundaries() {
        let tokens = tokenize("café naïve_x").unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Ident("café".to_string()));
        assert_eq!(tokens[0].span.end, "café".len());
        assert_eq!(tokens[1].kind, TokenKind::Ident("naïve_x".to_string()));
        assert_eq!(tokens[2].kind, TokenKind::Eof);
    }

    #[test]
    fn non_ascii_identifier_at_end_of_input() {
        let tokens = tokenize("aé").unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Ident("aé".to_string()));
        assert_eq!(tokens[0].span.end, 3);
        // Identifiers must start with an ASCII letter or underscore.
        let err = tokenize("é").unwrap_err();
        assert_eq!(err.message, "unexpected character: 'é'");
    }

    #[test]
    fn unexpected_character_error_carries_span() {
        let err = tokenize("let x = 1 $ 2;").unwrap_err();
        assert_eq!(err.message, "unexpected character: '$'");
        assert_eq!((err.span.line, err.span.column, err.span.start, err.span.end), (1, 11, 10, 11));
    }

    #[test]
    fn unexpected_multibyte_character_error_carries_span() {
        let err = tokenize("x → y").unwrap_err();
        assert_eq!(err.message, "unexpected character: '→'");
        assert_eq!((err.span.start, err.span.end), (2, 5));
    }

    #[test]
    fn unterminated_string_points_at_opening_quote() {
        let err = tokenize("fn main() {\n  let s = \"abc;\n}").unwrap_err();
        assert_eq!(err.message, "unterminated string literal");
        assert_eq!((err.line(), err.column()), (2, 11));
    }

    #[test]
    fn unterminated_interpolation_is_an_error() {
        let err = tokenize("\"a \\(b\"").unwrap_err();
        assert_eq!(err.message, "unterminated string interpolation");
    }

    #[test]
    fn invalid_escape_points_at_backslash() {
        let err = tokenize("\"ab\\q\"").unwrap_err();
        assert_eq!(err.message, "invalid escape sequence: \\q");
        assert_eq!((err.line(), err.column()), (1, 4));
    }

    #[test]
    fn eof_token_has_a_real_position() {
        let tokens = tokenize("fn\nmain").unwrap();
        let eof = tokens.last().unwrap();
        assert_eq!(eof.kind, TokenKind::Eof);
        assert_eq!((eof.span.line, eof.span.column), (2, 5));
        assert_eq!(eof.span.start, 7);
    }

    #[test]
    fn integer_overflow_literal_is_an_error() {
        let err = tokenize("99999999999999999999999").unwrap_err();
        assert!(err.message.starts_with("invalid integer literal"));
        assert_eq!(err.span.start, 0);
    }

    #[test]
    fn hex_and_binary_without_digits_are_errors() {
        assert!(tokenize("0x").unwrap_err().message.starts_with("invalid hex literal"));
        assert!(tokenize("0b").unwrap_err().message.starts_with("invalid binary literal"));
    }

    #[test]
    fn oversized_source_is_rejected() {
        let huge = "a".repeat(MAX_SOURCE_BYTES + 1);
        let err = tokenize(&huge).unwrap_err();
        assert!(err.message.contains("byte limit"));
    }

    #[test]
    fn lex_error_display_includes_position() {
        let err = LexError::at("boom", 3, 4, 10);
        assert_eq!(err.display(), "error[3:4]: boom");
        assert_eq!(err.to_string(), "error[3:4]: boom");
    }
}
