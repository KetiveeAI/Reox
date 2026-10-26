// REOX Compiler - Parser
// Recursive descent parser with Pratt parsing for expressions
// Zero external dependencies

mod ast;

pub use ast::*;

use crate::lexer::{Token, TokenKind, Span};

/// Parser error, located by a source span, with an optional hint for the user
#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
    pub hint: Option<String>,
}

impl ParseError {
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
            hint: None,
        }
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn display(&self) -> String {
        format!(
            "error[{}:{}]: {}",
            self.span.line, self.span.column, self.message
        )
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.display())
    }
}

impl std::error::Error for ParseError {}

/// Deepest nesting of expressions, blocks and types the parser accepts.
/// Recursive descent uses the native stack, so unbounded nesting would crash
/// the compiler instead of producing a diagnostic.
pub const MAX_NESTING_DEPTH: usize = 128;

/// Token returned when the parser is asked to look past the end of its input.
static END_OF_INPUT: Token = Token {
    kind: TokenKind::Eof,
    span: Span { line: 0, column: 0, start: 0, end: 0 },
};

/// REOX Parser
pub struct Parser<'a> {
    tokens: &'a [Token],
    current: usize,
    depth: usize,
    /// Set when a `>>` token closed one generic argument list and its second
    /// `>` is still owed to the enclosing list (`Box<Box<int>>`).
    pending_gt: bool,
}

impl<'a> Parser<'a> {
    pub fn new(tokens: &'a [Token]) -> Self {
        Self { tokens, current: 0, depth: 0, pending_gt: false }
    }

    /// Consume the `>` that closes a generic parameter or argument list,
    /// splitting a `>>` token when two lists close at once.
    fn consume_gt(&mut self, msg: &str) -> Result<(), ParseError> {
        if self.pending_gt {
            self.pending_gt = false;
            return Ok(());
        }
        if self.check(&TokenKind::Gt) {
            self.advance();
            return Ok(());
        }
        if self.check(&TokenKind::ShiftRight) {
            self.advance();
            self.pending_gt = true;
            return Ok(());
        }
        Err(self.error_here(msg))
    }

    /// Optional `<T, U: Bound + Other>` after a declaration name.
    fn parse_type_params(&mut self) -> Result<Vec<TypeParam>, ParseError> {
        if !self.check(&TokenKind::Lt) {
            return Ok(Vec::new());
        }
        self.advance();
        let mut params = Vec::new();
        loop {
            let span = self.peek().span;
            let name = self.parse_identifier()?;
            let mut bounds = Vec::new();
            if self.match_token(&[TokenKind::Colon]) {
                bounds.push(self.parse_identifier()?);
                while self.match_token(&[TokenKind::Plus]) {
                    bounds.push(self.parse_identifier()?);
                }
            }
            if params.iter().any(|p: &TypeParam| p.name == name) {
                return Err(ParseError::new(format!("type parameter '{}' is declared twice", name), span));
            }
            params.push(TypeParam { name, bounds, span });
            // A `>>` closed a nested list and already owes this one its `>`.
            if self.pending_gt || !self.match_token(&[TokenKind::Comma]) {
                break;
            }
        }
        self.consume_gt("expected '>' after type parameters")?;
        if params.is_empty() {
            return Err(self.error_here("expected at least one type parameter"));
        }
        Ok(params)
    }

    /// `<Type, Type>` after a type name.
    fn parse_type_args(&mut self) -> Result<Vec<Type>, ParseError> {
        self.consume(&TokenKind::Lt, "expected '<'")?;
        let mut args = Vec::new();
        loop {
            args.push(self.parse_type()?);
            if self.pending_gt || !self.match_token(&[TokenKind::Comma]) {
                break;
            }
        }
        self.consume_gt("expected '>' after type arguments")?;
        Ok(args)
    }

    // === Utility Methods ===

    fn peek(&self) -> &Token {
        self.tokens
            .get(self.current)
            .or_else(|| self.tokens.last())
            .unwrap_or(&END_OF_INPUT)
    }

    fn peek_kind(&self) -> &TokenKind {
        &self.peek().kind
    }

    fn is_at_end(&self) -> bool {
        matches!(self.peek_kind(), TokenKind::Eof)
    }

    /// Consume the current token and return it. At end of input the EOF
    /// token is returned and the position does not move.
    fn advance(&mut self) -> &Token {
        if !self.is_at_end() && self.current < self.tokens.len() {
            let consumed = self.current;
            self.current += 1;
            match self.tokens.get(consumed) {
                Some(t) => t,
                None => self.peek(),
            }
        } else {
            self.peek()
        }
    }

    fn check(&self, kind: &TokenKind) -> bool {
        std::mem::discriminant(self.peek_kind()) == std::mem::discriminant(kind)
    }

    fn consume(&mut self, kind: &TokenKind, msg: &str) -> Result<&Token, ParseError> {
        if self.check(kind) {
            Ok(self.advance())
        } else {
            Err(self.error_here(msg))
        }
    }

    /// Build an "<msg>, found <token>" error at the current token.
    fn error_here(&self, msg: &str) -> ParseError {
        let found = describe_token(self.peek_kind());
        let err = ParseError::new(format!("{}, found {}", msg, found), self.peek().span);
        if self.is_at_end() {
            err.with_hint("the input ended before this construct was complete")
        } else {
            err
        }
    }

    fn match_token(&mut self, kinds: &[TokenKind]) -> bool {
        for kind in kinds {
            if self.check(kind) {
                self.advance();
                return true;
            }
        }
        false
    }

    /// Consume one of `table`'s tokens and map it to its value. Returns
    /// `None` without consuming anything when the current token is not listed.
    fn match_map<T: Copy>(&mut self, table: &[(TokenKind, T)]) -> Option<T> {
        for (kind, value) in table {
            if self.check(kind) {
                self.advance();
                return Some(*value);
            }
        }
        None
    }

    /// Run `f` one nesting level deeper, failing cleanly past MAX_NESTING_DEPTH.
    fn nested<T>(
        &mut self,
        what: &str,
        f: impl FnOnce(&mut Self) -> Result<T, ParseError>,
    ) -> Result<T, ParseError> {
        if self.depth >= MAX_NESTING_DEPTH {
            return Err(ParseError::new(
                format!("{} nested deeper than {} levels", what, MAX_NESTING_DEPTH),
                self.peek().span,
            )
            .with_hint("split the construct into smaller pieces"));
        }
        self.depth += 1;
        let result = f(self);
        self.depth -= 1;
        result
    }

    /// Account for one more link in a left-associative operator or postfix
    /// chain. Each link deepens the expression tree by one level, which later
    /// stages walk recursively, so chains share the nesting budget.
    fn chain_link(&mut self) -> Result<(), ParseError> {
        if self.depth >= MAX_NESTING_DEPTH {
            return Err(ParseError::new(
                format!("expression chain longer than {} links", MAX_NESTING_DEPTH),
                self.peek().span,
            )
            .with_hint("split the expression into smaller pieces with intermediate variables"));
        }
        self.depth += 1;
        Ok(())
    }

    // === Parsing Methods ===

    pub fn parse_program(&mut self) -> Result<Program, ParseError> {
        let mut declarations = Vec::new();

        while !self.is_at_end() {
            declarations.push(self.parse_declaration()?);
        }

        Ok(Program { declarations })
    }

    fn parse_declaration(&mut self) -> Result<Decl, ParseError> {
        match self.peek_kind() {
            TokenKind::Fn => self.parse_fn_decl(false).map(Decl::Function),
            TokenKind::Async => {
                self.advance(); // consume 'async'
                if self.check(&TokenKind::Fn) {
                    self.parse_fn_decl(true).map(Decl::Function)
                } else {
                    Err(ParseError::new(
                        "expected 'fn' after 'async'",
                        self.peek().span,
                    ))
                }
            }
            TokenKind::Struct => self.parse_struct_decl(false).map(Decl::Struct),
            TokenKind::Import => self.parse_import_decl().map(Decl::Import),
            TokenKind::Extern => self.parse_extern_decl().map(Decl::Extern),
            TokenKind::Variant => self.parse_variant_decl(false).map(Decl::Variant),
            TokenKind::Protocol => self.parse_protocol_decl(false).map(Decl::Protocol),
            TokenKind::Extension => self.parse_extension_decl().map(Decl::Extension),
            TokenKind::Layer => self.parse_layer_decl(false).map(Decl::Layer),
            TokenKind::Panel => self.parse_panel_decl(false).map(Decl::Panel),
            TokenKind::Const => self.parse_const_decl(false).map(Decl::Const),
            TokenKind::Typealias => self.parse_typealias_decl(false).map(Decl::Typealias),
            TokenKind::Pub => self.parse_pub_decl(),
            TokenKind::Static => self.parse_static_decl(),
            TokenKind::Let => self.parse_global_var_decl(),
            TokenKind::Mut => Err(ParseError::new("'mut fn' is only allowed inside a struct, extension, layer or panel body", self.peek().span)
                .with_hint("a free function has no 'self' to mutate; drop 'mut'")),
            _ => Err(self
                .error_here("expected declaration")
                .with_hint("top-level items are fn, struct, variant, protocol, extension, layer, panel, const, typealias, import, extern or let")),
        }
    }

    fn parse_global_var_decl(&mut self) -> Result<Decl, ParseError> {
        let span = self.peek().span;
        self.consume(&TokenKind::Let, "expected 'let'")?;
        let mutable = self.match_token(&[TokenKind::Mut]);
        let name = self.parse_identifier()?;
        let ty = if self.match_token(&[TokenKind::Colon]) {
            Some(self.parse_type()?)
        } else {
            None
        };
        let init = if self.match_token(&[TokenKind::Eq]) {
            Some(self.parse_expression()?)
        } else {
            None
        };
        self.consume(&TokenKind::Semicolon, "expected ';' after global variable")?;
        Ok(Decl::GlobalVar(LetStmt {
            name,
            mutable,
            ty,
            init,
            is_bind: false,
            span,
        }))
    }

    fn parse_fn_decl(&mut self, is_async: bool) -> Result<FnDecl, ParseError> {
        self.parse_fn_decl_with_modifiers(is_async, false, false)
    }

    fn parse_fn_decl_with_modifiers(&mut self, is_async: bool, is_pub: bool, is_static: bool) -> Result<FnDecl, ParseError> {
        self.parse_fn_decl_full(is_async, is_pub, is_static, false)
    }

    /// Parse a method inside a struct, extension, layer or panel body:
    /// `[pub] [static] [mut] fn name(params) [-> Type] { body }`.
    fn parse_method_decl(&mut self) -> Result<FnDecl, ParseError> {
        let is_pub = self.match_token(&[TokenKind::Pub]);
        let is_static = self.match_token(&[TokenKind::Static]);
        let mut_span = self.peek().span;
        let is_mut = self.match_token(&[TokenKind::Mut]);
        if is_mut && is_static {
            return Err(ParseError::new("a static method cannot be 'mut'", mut_span)
                .with_hint("static methods have no 'self' to mutate; remove 'mut' or 'static'"));
        }
        if !self.check(&TokenKind::Fn) {
            return Err(self.error_here("expected 'fn' to start a method"));
        }
        self.parse_fn_decl_full(false, is_pub, is_static, is_mut)
    }

    fn parse_fn_decl_full(&mut self, is_async: bool, is_pub: bool, is_static: bool, is_mut: bool) -> Result<FnDecl, ParseError> {
        let start_span = self.peek().span;
        self.consume(&TokenKind::Fn, "expected 'fn'")?;

        let name = self.parse_identifier()?;
        let type_params = self.parse_type_params()?;
        self.consume(&TokenKind::LParen, "expected '(' after function name")?;

        let params = self.parse_param_list()?;
        self.consume(&TokenKind::RParen, "expected ')' after parameters")?;

        let return_type = if self.match_token(&[TokenKind::Arrow]) {
            Some(self.parse_type()?)
        } else {
            None
        };

        let body = self.parse_block()?;

        Ok(FnDecl {
            name,
            type_params,
            params,
            return_type,
            body,
            is_async,
            is_pub,
            is_static,
            is_mut,
            span: start_span,
        })
    }

    fn parse_param_list(&mut self) -> Result<Vec<Param>, ParseError> {
        let mut params = Vec::new();

        if !self.check(&TokenKind::RParen) {
            loop {
                params.push(self.parse_param()?);
                if !self.match_token(&[TokenKind::Comma]) {
                    break;
                }
            }
        }

        Ok(params)
    }

    fn parse_param(&mut self) -> Result<Param, ParseError> {
        let span = self.peek().span;
        let name = self.parse_identifier()?;
        self.consume(&TokenKind::Colon, "expected ':' after parameter name")?;
        let ty = self.parse_type()?;

        Ok(Param { name, ty, span })
    }

    /// Parse: struct Name { field: Type, ... [pub] [static] [mut] fn method() { ... } ... }
    fn parse_struct_decl(&mut self, is_pub: bool) -> Result<StructDecl, ParseError> {
        let start_span = self.peek().span;
        self.consume(&TokenKind::Struct, "expected 'struct'")?;

        let name = self.parse_identifier()?;
        let type_params = self.parse_type_params()?;
        self.consume(&TokenKind::LBrace, "expected '{'")?;

        let mut fields = Vec::new();
        let mut methods = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            if self.starts_method() {
                methods.push(self.parse_method_decl()?);
                continue;
            }
            if !methods.is_empty() {
                return Err(self
                    .error_here("expected a method or '}' after struct methods")
                    .with_hint("declare every field before the first method"));
            }
            fields.push(self.parse_field()?);
            if !self.match_token(&[TokenKind::Comma]) {
                // The field list may end without a trailing comma before methods or '}'.
                if !self.check(&TokenKind::RBrace) && !self.starts_method() {
                    return Err(self.error_here("expected ',' or '}' after struct field"));
                }
            }
        }

        self.consume(&TokenKind::RBrace, "expected '}'")?;

        Ok(StructDecl {
            name,
            type_params,
            fields,
            methods,
            is_pub,
            span: start_span,
        })
    }

    /// Whether the current token begins a method declaration inside a type body.
    fn starts_method(&self) -> bool {
        matches!(
            self.peek_kind(),
            TokenKind::Fn | TokenKind::Pub | TokenKind::Static | TokenKind::Mut
        )
    }

    fn parse_field(&mut self) -> Result<Field, ParseError> {
        let span = self.peek().span;
        let name = self.parse_identifier()?;
        self.consume(&TokenKind::Colon, "expected ':' after field name")?;
        let ty = self.parse_type()?;

        Ok(Field { name, ty, span })
    }

    fn parse_import_decl(&mut self) -> Result<ImportDecl, ParseError> {
        let span = self.peek().span;
        self.consume(&TokenKind::Import, "expected 'import'")?;

        let mut path = vec![self.parse_identifier()?];
        while self.match_token(&[TokenKind::Colon]) {
            self.consume(&TokenKind::Colon, "expected '::'")?;
            path.push(self.parse_identifier()?);
        }

        self.consume(&TokenKind::Semicolon, "expected ';' after import")?;

        Ok(ImportDecl { path, span })
    }

    fn parse_extern_decl(&mut self) -> Result<ExternDecl, ParseError> {
        let span = self.peek().span;
        self.consume(&TokenKind::Extern, "expected 'extern'")?;
        
        // Check for async extern fn
        let is_async = self.match_token(&[TokenKind::Async]);
        
        self.consume(&TokenKind::Fn, "expected 'fn' after 'extern'")?;

        let name = self.parse_identifier()?;
        self.consume(&TokenKind::LParen, "expected '('")?;
        let params = self.parse_param_list()?;
        self.consume(&TokenKind::RParen, "expected ')'")?;

        let return_type = if self.match_token(&[TokenKind::Arrow]) {
            Some(self.parse_type()?)
        } else {
            None
        };

        self.consume(&TokenKind::Semicolon, "expected ';'")?;

        Ok(ExternDecl {
            name,
            params,
            return_type,
            is_async,
            span,
        })
    }

    fn parse_type(&mut self) -> Result<Type, ParseError> {
        self.nested("type", |p| p.parse_type_inner())
    }

    fn parse_type_inner(&mut self) -> Result<Type, ParseError> {
        match self.peek_kind().clone() {
            TokenKind::Int => {
                self.advance();
                Ok(Type::Int)
            }
            TokenKind::Float => {
                self.advance();
                Ok(Type::Float)
            }
            TokenKind::String => {
                self.advance();
                Ok(Type::String)
            }
            TokenKind::Bool => {
                self.advance();
                Ok(Type::Bool)
            }
            TokenKind::Void => {
                self.advance();
                Ok(Type::Void)
            }
            TokenKind::I8 => { self.advance(); Ok(Type::I8) }
            TokenKind::U8 => { self.advance(); Ok(Type::U8) }
            TokenKind::I16 => { self.advance(); Ok(Type::I16) }
            TokenKind::U16 => { self.advance(); Ok(Type::U16) }
            TokenKind::I32 => { self.advance(); Ok(Type::I32) }
            TokenKind::U32 => { self.advance(); Ok(Type::U32) }
            TokenKind::I64 => { self.advance(); Ok(Type::I64) }
            TokenKind::U64 => { self.advance(); Ok(Type::U64) }
            TokenKind::USize => { self.advance(); Ok(Type::USize) }
            TokenKind::Ident(name) => {
                self.advance();
                if self.check(&TokenKind::Lt) {
                    let args = self.parse_type_args()?;
                    return Ok(Type::Generic(name, args));
                }
                Ok(Type::Named(name))
            }
            TokenKind::LBracket => {
                self.advance();
                let inner = self.parse_type()?;
                self.consume(&TokenKind::RBracket, "expected ']'")?;
                Ok(Type::Array(Box::new(inner)))
            }
            _ => Err(self.error_here("expected type")),
        }
    }

    fn parse_block(&mut self) -> Result<Block, ParseError> {
        self.nested("block", |p| p.parse_block_inner())
    }

    fn parse_block_inner(&mut self) -> Result<Block, ParseError> {
        let span = self.peek().span;
        self.consume(&TokenKind::LBrace, "expected '{'")?;

        let mut statements = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            statements.push(self.parse_statement()?);
        }

        if self.is_at_end() {
            return Err(ParseError::new("unclosed block: expected '}'", self.peek().span)
                .with_hint(format!("the block opened at {}:{} is never closed", span.line, span.column)));
        }
        self.consume(&TokenKind::RBrace, "expected '}'")?;

        Ok(Block { statements, span })
    }

    fn parse_statement(&mut self) -> Result<Stmt, ParseError> {
        match self.peek_kind() {
            TokenKind::Let => self.parse_let_stmt(),
            TokenKind::Return => self.parse_return_stmt(),
            TokenKind::If => self.parse_if_stmt(),
            TokenKind::While => self.parse_while_stmt(),
            TokenKind::For => self.parse_for_stmt(),
            TokenKind::LBrace => Ok(Stmt::Block(self.parse_block()?)),
            TokenKind::Ident(name) if name == "break" => {
                let span = self.peek().span;
                self.advance();
                self.consume(&TokenKind::Semicolon, "expected ';' after 'break'")?;
                Ok(Stmt::Break(span))
            }
            TokenKind::Ident(name) if name == "continue" => {
                let span = self.peek().span;
                self.advance();
                self.consume(&TokenKind::Semicolon, "expected ';' after 'continue'")?;
                Ok(Stmt::Continue(span))
            }
            // Swift/C++ style statements
            TokenKind::Guard => self.parse_guard_stmt(),
            TokenKind::Defer => self.parse_defer_stmt(),
            TokenKind::Try => self.parse_try_catch_stmt(),
            TokenKind::Throw => self.parse_throw_stmt(),
            TokenKind::Emit => self.parse_emit_stmt(),
            TokenKind::At => self.parse_bind_let_stmt(),
            _ => self.parse_expr_stmt(),
        }
    }

    fn parse_let_stmt(&mut self) -> Result<Stmt, ParseError> {
        let span = self.peek().span;
        self.consume(&TokenKind::Let, "expected 'let'")?;

        let mutable = self.match_token(&[TokenKind::Mut]);
        let name = self.parse_identifier()?;

        let ty = if self.match_token(&[TokenKind::Colon]) {
            Some(self.parse_type()?)
        } else {
            None
        };

        let init = if self.match_token(&[TokenKind::Eq]) {
            Some(self.parse_expression()?)
        } else {
            None
        };

        self.consume(&TokenKind::Semicolon, "expected ';' after variable declaration")?;

        Ok(Stmt::Let(LetStmt {
            name,
            mutable,
            ty,
            init,
            is_bind: false,
            span,
        }))
    }

    fn parse_return_stmt(&mut self) -> Result<Stmt, ParseError> {
        let span = self.peek().span;
        self.consume(&TokenKind::Return, "expected 'return'")?;

        let value = if !self.check(&TokenKind::Semicolon) {
            Some(self.parse_expression()?)
        } else {
            None
        };

        self.consume(&TokenKind::Semicolon, "expected ';' after return")?;

        Ok(Stmt::Return(ReturnStmt { value, span }))
    }

    fn parse_if_stmt(&mut self) -> Result<Stmt, ParseError> {
        self.nested("if statement", |p| p.parse_if_stmt_inner())
    }

    fn parse_if_stmt_inner(&mut self) -> Result<Stmt, ParseError> {
        let span = self.peek().span;
        self.consume(&TokenKind::If, "expected 'if'")?;

        let condition = self.parse_expression()?;
        let then_block = self.parse_block()?;

        let else_block = if self.match_token(&[TokenKind::Else]) {
            if self.check(&TokenKind::If) {
                // else if chain: wrap the nested if into a block
                let nested_if = self.parse_if_stmt()?;
                let nested_span = span;
                Some(Block {
                    statements: vec![nested_if],
                    span: nested_span,
                })
            } else {
                Some(self.parse_block()?)
            }
        } else {
            None
        };

        Ok(Stmt::If(IfStmt {
            condition,
            then_block,
            else_block,
            span,
        }))
    }

    fn parse_while_stmt(&mut self) -> Result<Stmt, ParseError> {
        let span = self.peek().span;
        self.consume(&TokenKind::While, "expected 'while'")?;

        let condition = self.parse_expression()?;
        let body = self.parse_block()?;

        Ok(Stmt::While(WhileStmt {
            condition,
            body,
            span,
        }))
    }

    fn parse_for_stmt(&mut self) -> Result<Stmt, ParseError> {
        let span = self.peek().span;
        self.consume(&TokenKind::For, "expected 'for'")?;

        let var = self.parse_identifier()?;
        self.consume(&TokenKind::In, "expected 'in'")?;
        let iterable = self.parse_expression()?;
        let body = self.parse_block()?;

        Ok(Stmt::For(ForStmt {
            var,
            iterable,
            body,
            span,
        }))
    }

    fn parse_expr_stmt(&mut self) -> Result<Stmt, ParseError> {
        let expr = self.parse_expression()?;
        // A match used as a statement may end with its closing brace, and the
        // last expression of a block may omit its ';' (the block's value).
        if !self.check(&TokenKind::Semicolon) && (matches!(expr, Expr::Match(..)) || self.check(&TokenKind::RBrace)) {
            return Ok(Stmt::Expr(expr));
        }
        self.consume(&TokenKind::Semicolon, "expected ';' after expression")?;
        Ok(Stmt::Expr(expr))
    }

    // === Swift/C++ Style Statement Parsing ===

    fn parse_guard_stmt(&mut self) -> Result<Stmt, ParseError> {
        // guard condition else { ... }
        let span = self.peek().span;
        self.consume(&TokenKind::Guard, "expected 'guard'")?;
        let condition = self.parse_expression()?;
        self.consume(&TokenKind::Else, "expected 'else' after guard condition")?;
        let else_block = self.parse_block()?;
        Ok(Stmt::Guard(GuardStmt { condition, else_block, span }))
    }

    fn parse_defer_stmt(&mut self) -> Result<Stmt, ParseError> {
        // defer { ... }  (function-scope only for v1)
        let span = self.peek().span;
        self.consume(&TokenKind::Defer, "expected 'defer'")?;
        let body = self.parse_block()?;
        Ok(Stmt::Defer(DeferStmt { body, span }))
    }

    fn parse_try_catch_stmt(&mut self) -> Result<Stmt, ParseError> {
        // try { ... } catch e { ... }  (single catch only for v1)
        let span = self.peek().span;
        self.consume(&TokenKind::Try, "expected 'try'")?;
        let try_block = self.parse_block()?;
        self.consume(&TokenKind::Catch, "expected 'catch'")?;
        let catch_var = match self.peek_kind() {
            TokenKind::Ident(_) => Some(self.parse_identifier()?),
            _ => None,
        };
        let catch_block = self.parse_block()?;
        Ok(Stmt::TryCatch(TryCatchStmt { try_block, catch_var, catch_block, span }))
    }

    fn parse_throw_stmt(&mut self) -> Result<Stmt, ParseError> {
        let span = self.peek().span;
        self.consume(&TokenKind::Throw, "expected 'throw'")?;
        let value = self.parse_expression()?;
        self.consume(&TokenKind::Semicolon, "expected ';' after throw")?;
        Ok(Stmt::Throw(ThrowStmt { value, span }))
    }

    // === Expression Parsing (Pratt Parser) ===

    fn parse_expression(&mut self) -> Result<Expr, ParseError> {
        self.nested("expression", |p| p.parse_assignment())
    }

    fn parse_assignment(&mut self) -> Result<Expr, ParseError> {
        let expr = self.parse_nullish_coalesce()?;

        // Check for assignment or compound assignment
        if self.match_token(&[TokenKind::Eq]) {
            let span = self.peek().span;
            let value = self.parse_assignment()?;
            return Ok(Expr::Assign(Box::new(expr), Box::new(value), span));
        }

        // Compound assignment operators
        if let Some(op) = self.match_map(&[
            (TokenKind::PlusEq, CompoundOp::AddEq),
            (TokenKind::MinusEq, CompoundOp::SubEq),
            (TokenKind::StarEq, CompoundOp::MulEq),
            (TokenKind::SlashEq, CompoundOp::DivEq),
            (TokenKind::PercentEq, CompoundOp::ModEq),
        ]) {
            let span = self.peek().span;
            let value = self.parse_assignment()?;
            return Ok(Expr::CompoundAssign(Box::new(expr), op, Box::new(value), span));
        }

        Ok(expr)
    }

    fn parse_nullish_coalesce(&mut self) -> Result<Expr, ParseError> {
        let base_depth = self.depth;
        let mut left = self.parse_or()?;

        while self.match_token(&[TokenKind::QuestionQuestion]) {
            self.chain_link()?;
            let span = self.peek().span;
            let right = self.parse_or()?;
            left = Expr::NullCoalesce(Box::new(left), Box::new(right), span);
        }

        self.depth = base_depth;
        Ok(left)
    }

    fn parse_or(&mut self) -> Result<Expr, ParseError> {
        let base_depth = self.depth;
        let mut left = self.parse_and()?;

        while self.match_token(&[TokenKind::Or]) {
            self.chain_link()?;
            let span = self.peek().span;
            let right = self.parse_and()?;
            left = Expr::Binary(Box::new(left), BinOp::Or, Box::new(right), span);
        }

        self.depth = base_depth;
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr, ParseError> {
        let base_depth = self.depth;
        let mut left = self.parse_bitwise_or()?;

        while self.match_token(&[TokenKind::And]) {
            self.chain_link()?;
            let span = self.peek().span;
            let right = self.parse_bitwise_or()?;
            left = Expr::Binary(Box::new(left), BinOp::And, Box::new(right), span);
        }

        self.depth = base_depth;
        Ok(left)
    }

    fn parse_bitwise_or(&mut self) -> Result<Expr, ParseError> {
        let base_depth = self.depth;
        let mut left = self.parse_bitwise_xor()?;

        while self.match_token(&[TokenKind::BitwiseOr]) {
            self.chain_link()?;
            let span = self.peek().span;
            let right = self.parse_bitwise_xor()?;
            left = Expr::Binary(Box::new(left), BinOp::BitwiseOr, Box::new(right), span);
        }

        self.depth = base_depth;
        Ok(left)
    }

    fn parse_bitwise_xor(&mut self) -> Result<Expr, ParseError> {
        let base_depth = self.depth;
        let mut left = self.parse_bitwise_and()?;

        while self.match_token(&[TokenKind::BitwiseXor]) {
            self.chain_link()?;
            let span = self.peek().span;
            let right = self.parse_bitwise_and()?;
            left = Expr::Binary(Box::new(left), BinOp::BitwiseXor, Box::new(right), span);
        }

        self.depth = base_depth;
        Ok(left)
    }

    fn parse_bitwise_and(&mut self) -> Result<Expr, ParseError> {
        let base_depth = self.depth;
        let mut left = self.parse_equality()?;

        while self.match_token(&[TokenKind::BitwiseAnd]) {
            self.chain_link()?;
            let span = self.peek().span;
            let right = self.parse_equality()?;
            left = Expr::Binary(Box::new(left), BinOp::BitwiseAnd, Box::new(right), span);
        }

        self.depth = base_depth;
        Ok(left)
    }

    fn parse_equality(&mut self) -> Result<Expr, ParseError> {
        let base_depth = self.depth;
        let mut left = self.parse_comparison()?;

        while let Some(op) = self.match_map(&[
            (TokenKind::EqEq, BinOp::Eq),
            (TokenKind::BangEq, BinOp::Ne),
        ]) {
            self.chain_link()?;
            let span = self.peek().span;
            let right = self.parse_comparison()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right), span);
        }

        self.depth = base_depth;
        Ok(left)
    }

    fn parse_comparison(&mut self) -> Result<Expr, ParseError> {
        let base_depth = self.depth;
        let mut left = self.parse_shift()?;

        while let Some(op) = self.match_map(&[
            (TokenKind::Lt, BinOp::Lt),
            (TokenKind::Gt, BinOp::Gt),
            (TokenKind::LtEq, BinOp::Le),
            (TokenKind::GtEq, BinOp::Ge),
        ]) {
            self.chain_link()?;
            let span = self.peek().span;
            let right = self.parse_shift()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right), span);
        }

        self.depth = base_depth;
        Ok(left)
    }

    fn parse_shift(&mut self) -> Result<Expr, ParseError> {
        let base_depth = self.depth;
        let mut left = self.parse_term()?;

        while let Some(op) = self.match_map(&[
            (TokenKind::ShiftLeft, BinOp::ShiftLeft),
            (TokenKind::ShiftRight, BinOp::ShiftRight),
        ]) {
            self.chain_link()?;
            let span = self.peek().span;
            let right = self.parse_term()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right), span);
        }

        self.depth = base_depth;
        Ok(left)
    }

    fn parse_term(&mut self) -> Result<Expr, ParseError> {
        let base_depth = self.depth;
        let mut left = self.parse_factor()?;

        while let Some(op) = self.match_map(&[
            (TokenKind::Plus, BinOp::Add),
            (TokenKind::Minus, BinOp::Sub),
        ]) {
            self.chain_link()?;
            let span = self.peek().span;
            let right = self.parse_factor()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right), span);
        }

        self.depth = base_depth;
        Ok(left)
    }

    fn parse_factor(&mut self) -> Result<Expr, ParseError> {
        let base_depth = self.depth;
        let mut left = self.parse_unary()?;

        while let Some(op) = self.match_map(&[
            (TokenKind::Star, BinOp::Mul),
            (TokenKind::Slash, BinOp::Div),
            (TokenKind::Percent, BinOp::Mod),
        ]) {
            self.chain_link()?;
            let span = self.peek().span;
            let right = self.parse_unary()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right), span);
        }

        self.depth = base_depth;
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        // Await expression: await expr
        if self.match_token(&[TokenKind::Await]) {
            let span = self.peek().span;
            let operand = self.parse_unary_operand()?;
            return Ok(Expr::Await(Box::new(operand), span));
        }
        
        // Pre-increment/decrement
        if self.match_token(&[TokenKind::PlusPlus]) {
            let span = self.peek().span;
            let operand = self.parse_unary_operand()?;
            return Ok(Expr::PreIncrement(Box::new(operand), span));
        }
        if self.match_token(&[TokenKind::MinusMinus]) {
            let span = self.peek().span;
            let operand = self.parse_unary_operand()?;
            return Ok(Expr::PreDecrement(Box::new(operand), span));
        }
        
        // Unary operators: -, !, ~
        if let Some(op) = self.match_map(&[
            (TokenKind::Minus, UnaryOp::Neg),
            (TokenKind::Bang, UnaryOp::Not),
            (TokenKind::BitwiseNot, UnaryOp::BitwiseNot),
        ]) {
            let span = self.peek().span;
            let right = self.parse_unary_operand()?;
            return Ok(Expr::Unary(op, Box::new(right), span));
        }

        self.parse_postfix()
    }

    /// Operand of a prefix operator. Chains such as `- - - x` recurse, so
    /// each operator counts as one nesting level.
    fn parse_unary_operand(&mut self) -> Result<Expr, ParseError> {
        self.nested("unary expression", |p| p.parse_unary())
    }

    fn parse_postfix(&mut self) -> Result<Expr, ParseError> {
        let base_depth = self.depth;
        let mut expr = self.parse_call()?;

        // Post-increment/decrement
        loop {
            if !self.check(&TokenKind::PlusPlus) && !self.check(&TokenKind::MinusMinus) {
                break;
            }
            self.chain_link()?;
            if self.match_token(&[TokenKind::PlusPlus]) {
                let span = self.peek().span;
                expr = Expr::PostIncrement(Box::new(expr), span);
            } else if self.match_token(&[TokenKind::MinusMinus]) {
                let span = self.peek().span;
                expr = Expr::PostDecrement(Box::new(expr), span);
            } else {
                break;
            }
        }

        self.depth = base_depth;
        Ok(expr)
    }

    fn parse_call(&mut self) -> Result<Expr, ParseError> {
        let base_depth = self.depth;
        let mut expr = self.parse_primary()?;

        loop {
            let is_link = self.check(&TokenKind::LParen)
                || self.check(&TokenKind::Dot)
                || self.check(&TokenKind::QuestionDot)
                || self.check(&TokenKind::LBracket);
            if !is_link {
                break;
            }
            self.chain_link()?;
            if self.match_token(&[TokenKind::LParen]) {
                let span = self.peek().span;
                let args = self.parse_arg_list()?;
                self.consume(&TokenKind::RParen, "expected ')' after arguments")?;
                expr = Expr::Call(Box::new(expr), args, span);
            } else if self.match_token(&[TokenKind::Dot]) {
                let span = self.peek().span;
                let name = self.parse_identifier()?;
                expr = Expr::Member(Box::new(expr), name, span);
            } else if self.match_token(&[TokenKind::QuestionDot]) {
                // Optional chaining: a?.b
                let span = self.peek().span;
                let name = self.parse_identifier()?;
                expr = Expr::OptionalChain(Box::new(expr), name, span);
            } else if self.match_token(&[TokenKind::LBracket]) {
                let span = self.peek().span;
                let index = self.parse_expression()?;
                self.consume(&TokenKind::RBracket, "expected ']'")?;
                expr = Expr::Index(Box::new(expr), Box::new(index), span);
            } else {
                break;
            }
        }

        self.depth = base_depth;
        Ok(expr)
    }

    fn parse_arg_list(&mut self) -> Result<Vec<Expr>, ParseError> {
        let mut args = Vec::new();

        if !self.check(&TokenKind::RParen) {
            loop {
                args.push(self.parse_expression()?);
                if !self.match_token(&[TokenKind::Comma]) {
                    break;
                }
            }
        }

        Ok(args)
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        let token = self.peek().clone();

        match &token.kind {
            TokenKind::IntLit(n) => {
                let n = *n;
                self.advance();
                Ok(Expr::Literal(Literal::Int(n, token.span)))
            }
            TokenKind::FloatLit(n) => {
                let n = *n;
                self.advance();
                Ok(Expr::Literal(Literal::Float(n, token.span)))
            }
            TokenKind::StringLit(s) => {
                let s = s.clone();
                self.advance();
                Ok(Expr::Literal(Literal::String(s, token.span)))
            }
            TokenKind::True => {
                self.advance();
                Ok(Expr::Literal(Literal::Bool(true, token.span)))
            }
            TokenKind::False => {
                self.advance();
                Ok(Expr::Literal(Literal::Bool(false, token.span)))
            }
            TokenKind::Ident(name) => {
                let name = name.clone();
                self.advance();

                // Check for struct literal
                if self.check(&TokenKind::LBrace) {
                    // Look ahead to confirm it's actually a struct literal and not a block
                    let is_struct_literal = {
                        let mut lookahead = self.current;
                        lookahead += 1; // point to token after LBrace
                        let next1 = self.tokens.get(lookahead).map(|t| &t.kind);
                        match next1 {
                            Some(TokenKind::Ident(_)) => {
                                let next2 = self.tokens.get(lookahead + 1).map(|t| &t.kind);
                                matches!(next2, Some(TokenKind::Colon))
                            },
                            Some(TokenKind::RBrace) => true,
                            _ => false,
                        }
                    };

                    if is_struct_literal {
                        self.advance(); // consume LBrace
                        let mut fields = Vec::new();
                        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
                            let field_name = self.parse_identifier()?;
                            self.consume(&TokenKind::Colon, "expected ':'")?;
                            let value = self.parse_expression()?;
                            fields.push((field_name, value));
                            if !self.match_token(&[TokenKind::Comma]) {
                                break;
                            }
                        }
                        self.consume(&TokenKind::RBrace, "expected '}'")?;
                        return Ok(Expr::StructLit(name, fields, token.span));
                    }
                }

                Ok(Expr::Identifier(name, token.span))
            }
            TokenKind::LParen => {
                self.advance();
                let expr = self.parse_expression()?;
                self.consume(&TokenKind::RParen, "expected ')'")?;
                Ok(expr)
            }
            TokenKind::LBracket => {
                self.advance();
                let mut elements = Vec::new();
                while !self.check(&TokenKind::RBracket) && !self.is_at_end() {
                    elements.push(self.parse_expression()?);
                    if !self.match_token(&[TokenKind::Comma]) {
                        break;
                    }
                }
                self.consume(&TokenKind::RBracket, "expected ']'")?;
                Ok(Expr::ArrayLit(elements, token.span))
            }
            TokenKind::Nil => {
                self.advance();
                Ok(Expr::Nil(token.span))
            }
            TokenKind::Self_ => {
                self.advance();
                Ok(Expr::SelfRef(token.span))
            }
            TokenKind::Match => {
                self.advance();
                let scrutinee = self.parse_expression()?;
                self.consume(&TokenKind::LBrace, "expected '{' after match expression")?;
                
                let mut arms = Vec::new();
                while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
                    let arm = self.parse_match_arm()?;
                    arms.push(arm);
                    // Optional comma between arms
                    self.match_token(&[TokenKind::Comma]);
                }
                
                self.consume(&TokenKind::RBrace, "expected '}' after match arms")?;
                Ok(Expr::Match(Box::new(scrutinee), arms, token.span))
            }
            TokenKind::StringInterp(parts) => {
                // Desugar: "Hello \(name)!" -> string_concat("Hello ", string_concat(name, "!"))
                // We build the expression bottom-up from the parts list
                let parts = parts.clone();
                let span = token.span;
                self.advance();

                // Convert parts to expressions
                let mut exprs: Vec<Expr> = Vec::new();
                for part in parts {
                    match part {
                        crate::lexer::StringPart::Literal(s) => {
                            exprs.push(Expr::Literal(Literal::String(s, span)));
                        }
                        crate::lexer::StringPart::Expr(code) => {
                            // Parse the embedded expression text
                            exprs.push(Expr::Call(
                                Box::new(Expr::Identifier("str".to_string(), span)),
                                vec![Expr::Identifier(code, span)],
                                span,
                            ));
                        }
                    }
                }

                if exprs.is_empty() {
                    return Ok(Expr::Literal(Literal::String(String::new(), span)));
                }

                // Fold into string_concat chain
                let mut result = exprs.remove(0);
                for next in exprs {
                    result = Expr::Call(
                        Box::new(Expr::Identifier("string_concat".to_string(), span)),
                        vec![result, next],
                        span,
                    );
                }
                Ok(result)
            }
            _ => Err(self.error_here("expected expression")),
        }
    }

    fn parse_identifier(&mut self) -> Result<String, ParseError> {
        match self.peek_kind().clone() {
            TokenKind::Ident(name) => {
                self.advance();
                Ok(name)
            }
            _ => Err(self.error_here("expected identifier")),
        }
    }

    fn parse_match_arm(&mut self) -> Result<MatchArm, ParseError> {
        let span = self.peek().span;
        let pattern = self.parse_pattern()?;
        self.consume(&TokenKind::FatArrow, "expected '=>' after pattern")?;
        
        // Body can be a single expression or a block whose last expression
        // statement is its value
        let body = if self.check(&TokenKind::LBrace) {
            let block_span = self.peek().span;
            let block = self.parse_block()?;
            Expr::Block(block, block_span)
        } else {
            self.parse_expression()?
        };
        
        Ok(MatchArm { pattern, body, span })
    }

    fn parse_pattern(&mut self) -> Result<Pattern, ParseError> {
        let token = self.peek().clone();
        
        match &token.kind {
            TokenKind::IntLit(n) => {
                let n = *n;
                self.advance();
                Ok(Pattern::Literal(Literal::Int(n, token.span)))
            }
            TokenKind::StringLit(s) => {
                let s = s.clone();
                self.advance();
                Ok(Pattern::Literal(Literal::String(s, token.span)))
            }
            TokenKind::True => {
                self.advance();
                Ok(Pattern::Literal(Literal::Bool(true, token.span)))
            }
            TokenKind::False => {
                self.advance();
                Ok(Pattern::Literal(Literal::Bool(false, token.span)))
            }
            TokenKind::Ident(name) => {
                let name = name.clone();
                self.advance();
                if name == "_" {
                    return Ok(Pattern::Wildcard);
                }
                // Type.Case or Type.Case(args)
                if self.match_token(&[TokenKind::Dot]) {
                    let case = self.parse_identifier()?;
                    let args = if self.check(&TokenKind::LParen) { self.parse_pattern_args()? } else { Vec::new() };
                    return Ok(Pattern::Variant(VariantPattern { type_name: Some(name), case, args, span: token.span }));
                }
                // Case(args)
                if self.check(&TokenKind::LParen) {
                    let args = self.parse_pattern_args()?;
                    return Ok(Pattern::Variant(VariantPattern { type_name: None, case: name, args, span: token.span }));
                }
                Ok(Pattern::Identifier(name))
            }
            _ => Err(self.error_here("expected pattern")),
        }
    }

    /// `( pattern, pattern, ... )` after a variant case name.
    fn parse_pattern_args(&mut self) -> Result<Vec<Pattern>, ParseError> {
        self.consume(&TokenKind::LParen, "expected '(' after case name")?;
        let mut args = Vec::new();
        if !self.check(&TokenKind::RParen) {
            loop {
                args.push(self.nested("pattern", |p| p.parse_pattern())?);
                if !self.match_token(&[TokenKind::Comma]) {
                    break;
                }
            }
        }
        self.consume(&TokenKind::RParen, "expected ')' after case patterns")?;
        Ok(args)
    }

    // === UI-Specific Declaration Parsing ===

    /// Parse: variant Name { Case1, Case2(field: Type), ... }
    fn parse_variant_decl(&mut self, is_pub: bool) -> Result<VariantDecl, ParseError> {
        let span = self.peek().span;
        self.consume(&TokenKind::Variant, "expected 'variant'")?;
        let name = self.parse_identifier()?;
        let type_params = self.parse_type_params()?;
        self.consume(&TokenKind::LBrace, "expected '{' after variant name")?;

        let mut cases = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            let case_span = self.peek().span;
            let case_name = self.parse_identifier()?;

            let fields = if self.match_token(&[TokenKind::LParen]) {
                let mut f = Vec::new();
                if !self.check(&TokenKind::RParen) {
                    loop {
                        f.push(self.parse_variant_field(f.len())?);
                        if !self.match_token(&[TokenKind::Comma]) {
                            break;
                        }
                    }
                }
                self.consume(&TokenKind::RParen, "expected ')'")?;
                f
            } else {
                Vec::new()
            };

            cases.push(VariantCase { name: case_name, fields, span: case_span });

            if !self.match_token(&[TokenKind::Comma]) {
                break;
            }
        }

        self.consume(&TokenKind::RBrace, "expected '}'")?;
        Ok(VariantDecl { name, type_params, cases, is_pub, span })
    }

    /// A variant payload field: `name: Type`, or a bare `Type` named `_<index>`.
    fn parse_variant_field(&mut self, index: usize) -> Result<Field, ParseError> {
        let span = self.peek().span;
        let named = matches!(self.peek_kind(), TokenKind::Ident(_))
            && matches!(self.tokens.get(self.current + 1).map(|t| &t.kind), Some(TokenKind::Colon));
        if named {
            return self.parse_field();
        }
        let ty = self.parse_type()?;
        Ok(Field { name: format!("_{}", index), ty, span })
    }

    /// Parse: protocol Name { fn method(params) -> Type; ... }
    fn parse_protocol_decl(&mut self, is_pub: bool) -> Result<ProtocolDecl, ParseError> {
        let span = self.peek().span;
        self.consume(&TokenKind::Protocol, "expected 'protocol'")?;
        let name = self.parse_identifier()?;
        self.consume(&TokenKind::LBrace, "expected '{' after protocol name")?;

        let mut methods = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            let method_span = self.peek().span;
            self.consume(&TokenKind::Fn, "expected 'fn' in protocol body")?;
            let method_name = self.parse_identifier()?;
            self.consume(&TokenKind::LParen, "expected '('")?;
            let params = self.parse_param_list()?;
            self.consume(&TokenKind::RParen, "expected ')'")?;

            let return_type = if self.match_token(&[TokenKind::Arrow]) {
                Some(self.parse_type()?)
            } else {
                None
            };

            self.consume(&TokenKind::Semicolon, "expected ';' after protocol method")?;

            methods.push(ProtocolMethod {
                name: method_name,
                params,
                return_type,
                span: method_span,
            });
        }

        self.consume(&TokenKind::RBrace, "expected '}'")?;
        Ok(ProtocolDecl { name, methods, is_pub, span })
    }

    /// Parse: extension TypeName [: Protocol1, Protocol2] { fn method() { ... } ... }
    fn parse_extension_decl(&mut self) -> Result<ExtensionDecl, ParseError> {
        let span = self.peek().span;
        self.consume(&TokenKind::Extension, "expected 'extension'")?;
        let target = self.parse_identifier()?;
        let type_params = self.parse_type_params()?;

        let protocols = if self.match_token(&[TokenKind::Colon]) {
            let mut p = vec![self.parse_identifier()?];
            while self.match_token(&[TokenKind::Comma]) {
                p.push(self.parse_identifier()?);
            }
            p
        } else {
            Vec::new()
        };

        self.consume(&TokenKind::LBrace, "expected '{'")?;

        let mut methods = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            if !self.starts_method() {
                return Err(self.error_here("expected a method in extension body")
                    .with_hint("extensions may only contain '[pub] [static] [mut] fn' declarations"));
            }
            methods.push(self.parse_method_decl()?);
        }

        self.consume(&TokenKind::RBrace, "expected '}'")?;
        Ok(ExtensionDecl { target, type_params, protocols, methods, span })
    }

    /// Parse: layer Name { fields, signals, fn body() -> View { ... } }
    fn parse_layer_decl(&mut self, is_pub: bool) -> Result<LayerDecl, ParseError> {
        let span = self.peek().span;
        self.consume(&TokenKind::Layer, "expected 'layer'")?;
        let name = self.parse_identifier()?;
        self.consume(&TokenKind::LBrace, "expected '{' after layer name")?;

        let mut fields = Vec::new();
        let mut signals = Vec::new();
        let mut methods = Vec::new();
        let mut gestures = Vec::new();

        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            match self.peek_kind() {
                TokenKind::Signal => {
                    let sig_span = self.peek().span;
                    self.advance(); // consume 'signal'
                    let sig_name = self.parse_identifier()?;
                    let payload_type = if self.match_token(&[TokenKind::Colon]) {
                        Some(self.parse_type()?)
                    } else {
                        None
                    };
                    self.consume(&TokenKind::Semicolon, "expected ';' after signal")?;
                    signals.push(SignalField { name: sig_name, payload_type, span: sig_span });
                }
                TokenKind::OnTap | TokenKind::OnPan | TokenKind::OnSwipe
                | TokenKind::OnPinch | TokenKind::OnRotate => {
                    let gesture_span = self.peek().span;
                    let kind = match self.peek_kind() {
                        TokenKind::OnPan => "on_pan",
                        TokenKind::OnSwipe => "on_swipe",
                        TokenKind::OnPinch => "on_pinch",
                        TokenKind::OnRotate => "on_rotate",
                        // Only gesture keywords reach this arm; OnTap is the remaining one.
                        _ => "on_tap",
                    }.to_string();
                    self.advance(); // consume gesture keyword

                    // Optional parameter list
                    let params = if self.check(&TokenKind::LParen) {
                        self.advance();
                        let mut params = Vec::new();
                        while !self.check(&TokenKind::RParen) && !self.is_at_end() {
                            let pname = self.parse_identifier()?;
                            self.consume(&TokenKind::Colon, "expected ':' after param name")?;
                            let pty = self.parse_type()?;
                            params.push(Param { name: pname, ty: pty, span: gesture_span });
                            self.match_token(&[TokenKind::Comma]);
                        }
                        self.consume(&TokenKind::RParen, "expected ')'")?;
                        params
                    } else {
                        Vec::new()
                    };

                    // Body block
                    let body = self.parse_block()?;

                    gestures.push(GestureHandler { kind, params, body, span: gesture_span });
                }
                TokenKind::Fn | TokenKind::Pub | TokenKind::Static | TokenKind::Mut => {
                    methods.push(self.parse_method_decl()?);
                }
                TokenKind::Let | TokenKind::At => {
                    // Fields declared with let or @Bind let
                    let is_bind = self.match_token(&[TokenKind::At]);
                    if is_bind {
                        // Consume 'Bind' identifier after @
                        if let TokenKind::Ident(id) = self.peek_kind().clone() {
                            if id == "Bind" {
                                self.advance();
                            }
                        }
                    }
                    self.consume(&TokenKind::Let, "expected 'let'")?;
                    let _mutable = self.match_token(&[TokenKind::Mut]);
                    let field_span = self.peek().span;
                    let field_name = self.parse_identifier()?;
                    self.consume(&TokenKind::Colon, "expected ':' after field name")?;
                    let field_ty = self.parse_type()?;
                    // Skip optional initializer
                    if self.match_token(&[TokenKind::Eq]) {
                        self.parse_expression()?;
                    }
                    self.consume(&TokenKind::Semicolon, "expected ';'")?;
                    fields.push(Field { name: field_name, ty: field_ty, span: field_span });
                }
                _ => {
                    // Try as a regular field: name: type
                    let field = self.parse_field()?;
                    fields.push(field);
                    if !self.match_token(&[TokenKind::Comma]) {
                        self.match_token(&[TokenKind::Semicolon]);
                    }
                }
            }
        }

        self.consume(&TokenKind::RBrace, "expected '}'")?;
        Ok(LayerDecl { name, fields, signals, methods, gestures, is_pub, span })
    }

    /// Parse: panel Name { title: "...", size: (w, h), fn root() -> View { ... } }
    fn parse_panel_decl(&mut self, is_pub: bool) -> Result<PanelDecl, ParseError> {
        let span = self.peek().span;
        self.consume(&TokenKind::Panel, "expected 'panel'")?;
        let name = self.parse_identifier()?;
        self.consume(&TokenKind::LBrace, "expected '{' after panel name")?;

        let mut properties = Vec::new();
        let mut methods = Vec::new();

        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            match self.peek_kind() {
                TokenKind::Fn | TokenKind::Pub | TokenKind::Static | TokenKind::Mut => {
                    methods.push(self.parse_method_decl()?);
                }
                TokenKind::Ident(_) => {
                    // Property: key: value
                    let key = self.parse_identifier()?;
                    self.consume(&TokenKind::Colon, "expected ':' after property name")?;
                    let value = self.parse_expression()?;
                    properties.push((key, value));
                    // Optional comma or semicolon separator
                    if !self.match_token(&[TokenKind::Comma]) {
                        self.match_token(&[TokenKind::Semicolon]);
                    }
                }
                _ => {
                    return Err(self.error_here("expected property or 'fn' in panel body"));
                }
            }
        }

        self.consume(&TokenKind::RBrace, "expected '}'")?;
        Ok(PanelDecl { name, properties, methods, is_pub, span })
    }

    /// Parse: const NAME: Type = value;
    fn parse_const_decl(&mut self, is_pub: bool) -> Result<ConstDecl, ParseError> {
        let span = self.peek().span;
        self.consume(&TokenKind::Const, "expected 'const'")?;
        let name = self.parse_identifier()?;

        let ty = if self.match_token(&[TokenKind::Colon]) {
            Some(self.parse_type()?)
        } else {
            None
        };

        self.consume(&TokenKind::Eq, "expected '=' after const name")?;
        let value = self.parse_expression()?;
        self.consume(&TokenKind::Semicolon, "expected ';' after const declaration")?;

        Ok(ConstDecl { name, ty, value, is_pub, span })
    }

    /// Parse: typealias Name = TargetType;
    fn parse_typealias_decl(&mut self, is_pub: bool) -> Result<TypealiasDecl, ParseError> {
        let span = self.peek().span;
        self.consume(&TokenKind::Typealias, "expected 'typealias'")?;
        let name = self.parse_identifier()?;
        self.consume(&TokenKind::Eq, "expected '='")?;
        let target = self.parse_type()?;
        self.consume(&TokenKind::Semicolon, "expected ';'")?;

        Ok(TypealiasDecl { name, target, is_pub, span })
    }

    /// Parse: pub <declaration>
    fn parse_pub_decl(&mut self) -> Result<Decl, ParseError> {
        self.advance(); // consume 'pub'
        match self.peek_kind() {
            TokenKind::Fn => self.parse_fn_decl_with_modifiers(false, true, false).map(Decl::Function),
            TokenKind::Struct => self.parse_struct_decl(true).map(Decl::Struct),
            TokenKind::Variant => self.parse_variant_decl(true).map(Decl::Variant),
            TokenKind::Protocol => self.parse_protocol_decl(true).map(Decl::Protocol),
            TokenKind::Layer => self.parse_layer_decl(true).map(Decl::Layer),
            TokenKind::Panel => self.parse_panel_decl(true).map(Decl::Panel),
            TokenKind::Const => self.parse_const_decl(true).map(Decl::Const),
            TokenKind::Typealias => self.parse_typealias_decl(true).map(Decl::Typealias),
            TokenKind::Async => {
                self.advance();
                self.parse_fn_decl_with_modifiers(true, true, false).map(Decl::Function)
            }
            TokenKind::Static => {
                self.advance();
                if self.check(&TokenKind::Fn) {
                    self.parse_fn_decl_with_modifiers(false, true, true).map(Decl::Function)
                } else {
                    Err(ParseError::new(
                        "expected 'fn' after 'pub static'",
                        self.peek().span,
                    ))
                }
            }
            _ => Err(self.error_here("expected declaration after 'pub'")),
        }
    }

    /// Parse: static fn ...
    fn parse_static_decl(&mut self) -> Result<Decl, ParseError> {
        self.advance(); // consume 'static'
        match self.peek_kind() {
            TokenKind::Fn => self.parse_fn_decl_with_modifiers(false, false, true).map(Decl::Function),
            _ => Err(self.error_here("expected 'fn' after 'static'")),
        }
    }

    /// Parse: emit signal_name(value);
    fn parse_emit_stmt(&mut self) -> Result<Stmt, ParseError> {
        let span = self.peek().span;
        self.consume(&TokenKind::Emit, "expected 'emit'")?;
        let signal = self.parse_identifier()?;

        let value = if self.match_token(&[TokenKind::LParen]) {
            let v = Some(self.parse_expression()?);
            self.consume(&TokenKind::RParen, "expected ')'")?;
            v
        } else {
            None
        };

        self.consume(&TokenKind::Semicolon, "expected ';' after emit")?;
        Ok(Stmt::Expr(Expr::Call(
            Box::new(Expr::Identifier(format!("__emit_{}", signal), span)),
            value.into_iter().collect(),
            span,
        )))
    }

    /// Parse: @Bind let name: Type = value;
    fn parse_bind_let_stmt(&mut self) -> Result<Stmt, ParseError> {
        let span = self.peek().span;
        self.consume(&TokenKind::At, "expected '@'")?;

        // Expect 'Bind' identifier
        if let TokenKind::Ident(id) = self.peek_kind().clone() {
            if id == "Bind" {
                self.advance();
            } else {
                return Err(ParseError::new(
                    format!("expected 'Bind' after '@', found '{}'", id),
                    self.peek().span,
                ));
            }
        } else {
            return Err(ParseError::new(
                "expected 'Bind' after '@'",
                self.peek().span,
            ));
        }

        self.consume(&TokenKind::Let, "expected 'let' after '@Bind'")?;
        let mutable = self.match_token(&[TokenKind::Mut]);
        let name = self.parse_identifier()?;

        let ty = if self.match_token(&[TokenKind::Colon]) {
            Some(self.parse_type()?)
        } else {
            None
        };

        let init = if self.match_token(&[TokenKind::Eq]) {
            Some(self.parse_expression()?)
        } else {
            None
        };

        self.consume(&TokenKind::Semicolon, "expected ';'")?;

        Ok(Stmt::Let(LetStmt {
            name,
            mutable,
            ty,
            init,
            is_bind: true,
            span,
        }))
    }
}

/// Human-readable token name for error messages.
fn describe_token(kind: &TokenKind) -> String {
    match kind {
        TokenKind::Eof => "end of input".to_string(),
        TokenKind::Ident(name) => format!("identifier '{}'", name),
        TokenKind::IntLit(n) => format!("integer literal '{}'", n),
        TokenKind::FloatLit(n) => format!("float literal '{}'", n),
        TokenKind::StringLit(s) => format!("string literal \"{}\"", s),
        TokenKind::StringInterp(_) => "interpolated string literal".to_string(),
        TokenKind::LParen => "'('".to_string(),
        TokenKind::RParen => "')'".to_string(),
        TokenKind::LBrace => "'{'".to_string(),
        TokenKind::RBrace => "'}'".to_string(),
        TokenKind::LBracket => "'['".to_string(),
        TokenKind::RBracket => "']'".to_string(),
        TokenKind::Semicolon => "';'".to_string(),
        TokenKind::Comma => "','".to_string(),
        TokenKind::Colon => "':'".to_string(),
        TokenKind::Dot => "'.'".to_string(),
        TokenKind::Eq => "'='".to_string(),
        TokenKind::Arrow => "'->'".to_string(),
        TokenKind::FatArrow => "'=>'".to_string(),
        other => format!("{:?}", other),
    }
}

/// Convenience type for backward compatibility
pub type Ast = Program;

/// Stack reserved for parsing. Recursive descent through the precedence
/// chain uses many frames per nesting level, which are large in debug builds,
/// so parsing runs on its own thread.
const PARSE_STACK_BYTES: usize = 64 * 1024 * 1024;

/// Parse tokens into an AST. Any syntax error is returned, never printed.
/// Parsing runs on a dedicated large-stack thread; see `MAX_NESTING_DEPTH`.
pub fn parse(tokens: &[Token]) -> Result<Ast, ParseError> {
    std::thread::scope(|scope| {
        let worker = std::thread::Builder::new()
            .name("reox-parse".to_string())
            .stack_size(PARSE_STACK_BYTES)
            .spawn_scoped(scope, || Parser::new(tokens).parse_program());
        match worker {
            Ok(handle) => handle.join().unwrap_or_else(|_| {
                Err(ParseError::new("parser thread terminated unexpectedly", Span::default()))
            }),
            // Could not get a thread: parse here, still protected by the depth limit.
            Err(_) => Parser::new(tokens).parse_program(),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;

    #[test]
    fn test_parse_simple_function() {
        let tokens = tokenize("fn main() { }").unwrap();
        let ast = parse(&tokens).unwrap();
        assert_eq!(ast.declarations.len(), 1);
        match &ast.declarations[0] {
            Decl::Function(f) => {
                assert_eq!(f.name, "main");
                assert!(f.params.is_empty());
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn test_parse_function_with_params() {
        let tokens = tokenize("fn add(a: int, b: int) -> int { return a + b; }").unwrap();
        let ast = parse(&tokens).unwrap();
        assert_eq!(ast.declarations.len(), 1);
        match &ast.declarations[0] {
            Decl::Function(f) => {
                assert_eq!(f.name, "add");
                assert_eq!(f.params.len(), 2);
                assert_eq!(f.return_type, Some(Type::Int));
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn test_parse_struct() {
        let tokens = tokenize("struct Point { x: int, y: int }").unwrap();
        let ast = parse(&tokens).unwrap();
        assert_eq!(ast.declarations.len(), 1);
        match &ast.declarations[0] {
            Decl::Struct(s) => {
                assert_eq!(s.name, "Point");
                assert_eq!(s.fields.len(), 2);
            }
            _ => panic!("expected struct"),
        }
    }

    #[test]
    fn test_parse_let_statement() {
        let tokens = tokenize("fn main() { let x: int = 42; }").unwrap();
        let ast = parse(&tokens).unwrap();
        match &ast.declarations[0] {
            Decl::Function(f) => {
                assert_eq!(f.body.statements.len(), 1);
                match &f.body.statements[0] {
                    Stmt::Let(l) => {
                        assert_eq!(l.name, "x");
                        assert_eq!(l.ty, Some(Type::Int));
                    }
                    _ => panic!("expected let"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn test_parse_if_statement() {
        let tokens = tokenize("fn main() { if x > 0 { } else { } }").unwrap();
        let ast = parse(&tokens).unwrap();
        match &ast.declarations[0] {
            Decl::Function(f) => {
                assert_eq!(f.body.statements.len(), 1);
                match &f.body.statements[0] {
                    Stmt::If(i) => {
                        assert!(i.else_block.is_some());
                    }
                    _ => panic!("expected if"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn test_parse_expression_precedence() {
        let tokens = tokenize("fn main() { let x = 1 + 2 * 3; }").unwrap();
        let ast = parse(&tokens).unwrap();
        // 1 + (2 * 3) due to precedence
        match &ast.declarations[0] {
            Decl::Function(f) => {
                match &f.body.statements[0] {
                    Stmt::Let(l) => {
                        match l.init.as_ref().unwrap() {
                            Expr::Binary(_, BinOp::Add, right, _) => {
                                match right.as_ref() {
                                    Expr::Binary(_, BinOp::Mul, _, _) => {}
                                    _ => panic!("expected mul on right"),
                                }
                            }
                            _ => panic!("expected binary add"),
                        }
                    }
                    _ => panic!("expected let"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn test_parse_function_call() {
        let tokens = tokenize("fn main() { print(42); }").unwrap();
        let ast = parse(&tokens).unwrap();
        match &ast.declarations[0] {
            Decl::Function(f) => {
                match &f.body.statements[0] {
                    Stmt::Expr(Expr::Call(callee, args, _)) => {
                        match callee.as_ref() {
                            Expr::Identifier(name, _) => assert_eq!(name, "print"),
                            _ => panic!("expected identifier"),
                        }
                        assert_eq!(args.len(), 1);
                    }
                    _ => panic!("expected call"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    // === Swift/C++ Style Feature Tests ===

    #[test]
    fn test_parse_guard_statement() {
        let tokens = tokenize("fn main() { guard x > 0 else { return; } }").unwrap();
        let ast = parse(&tokens).unwrap();
        match &ast.declarations[0] {
            Decl::Function(f) => {
                assert_eq!(f.body.statements.len(), 1);
                match &f.body.statements[0] {
                    Stmt::Guard(g) => {
                        assert!(!g.else_block.statements.is_empty());
                    }
                    _ => panic!("expected guard statement"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn test_parse_defer_statement() {
        let tokens = tokenize("fn main() { defer { cleanup(); } }").unwrap();
        let ast = parse(&tokens).unwrap();
        match &ast.declarations[0] {
            Decl::Function(f) => {
                assert_eq!(f.body.statements.len(), 1);
                match &f.body.statements[0] {
                    Stmt::Defer(d) => {
                        assert!(!d.body.statements.is_empty());
                    }
                    _ => panic!("expected defer statement"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn test_parse_try_catch_statement() {
        let tokens = tokenize("fn main() { try { risky(); } catch e { handle(); } }").unwrap();
        let ast = parse(&tokens).unwrap();
        match &ast.declarations[0] {
            Decl::Function(f) => {
                assert_eq!(f.body.statements.len(), 1);
                match &f.body.statements[0] {
                    Stmt::TryCatch(t) => {
                        assert!(!t.try_block.statements.is_empty());
                        assert_eq!(t.catch_var, Some("e".to_string()));
                        assert!(!t.catch_block.statements.is_empty());
                    }
                    _ => panic!("expected try/catch statement"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn test_parse_throw_statement() {
        let tokens = tokenize("fn main() { throw error; }").unwrap();
        let ast = parse(&tokens).unwrap();
        match &ast.declarations[0] {
            Decl::Function(f) => {
                assert_eq!(f.body.statements.len(), 1);
                match &f.body.statements[0] {
                    Stmt::Throw(_) => {}
                    _ => panic!("expected throw statement"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn test_parse_compound_assignment() {
        let tokens = tokenize("fn main() { x += 1; }").unwrap();
        let ast = parse(&tokens).unwrap();
        match &ast.declarations[0] {
            Decl::Function(f) => {
                match &f.body.statements[0] {
                    Stmt::Expr(Expr::CompoundAssign(_, CompoundOp::AddEq, _, _)) => {}
                    _ => panic!("expected compound assignment"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn test_parse_null_coalescing() {
        let tokens = tokenize("fn main() { let x = a ?? b; }").unwrap();
        let ast = parse(&tokens).unwrap();
        match &ast.declarations[0] {
            Decl::Function(f) => {
                match &f.body.statements[0] {
                    Stmt::Let(l) => {
                        match l.init.as_ref().unwrap() {
                            Expr::NullCoalesce(_, _, _) => {}
                            _ => panic!("expected null coalescing"),
                        }
                    }
                    _ => panic!("expected let"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn test_parse_optional_chaining() {
        let tokens = tokenize("fn main() { let x = a?.b; }").unwrap();
        let ast = parse(&tokens).unwrap();
        match &ast.declarations[0] {
            Decl::Function(f) => {
                match &f.body.statements[0] {
                    Stmt::Let(l) => {
                        match l.init.as_ref().unwrap() {
                            Expr::OptionalChain(_, name, _) => {
                                assert_eq!(name, "b");
                            }
                            _ => panic!("expected optional chaining"),
                        }
                    }
                    _ => panic!("expected let"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn test_parse_pre_increment() {
        let tokens = tokenize("fn main() { ++x; }").unwrap();
        let ast = parse(&tokens).unwrap();
        match &ast.declarations[0] {
            Decl::Function(f) => {
                match &f.body.statements[0] {
                    Stmt::Expr(Expr::PreIncrement(_, _)) => {}
                    _ => panic!("expected pre-increment"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn test_parse_post_increment() {
        let tokens = tokenize("fn main() { x++; }").unwrap();
        let ast = parse(&tokens).unwrap();
        match &ast.declarations[0] {
            Decl::Function(f) => {
                match &f.body.statements[0] {
                    Stmt::Expr(Expr::PostIncrement(_, _)) => {}
                    _ => panic!("expected post-increment"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn test_parse_nil_literal() {
        let tokens = tokenize("fn main() { let x = nil; }").unwrap();
        let ast = parse(&tokens).unwrap();
        match &ast.declarations[0] {
            Decl::Function(f) => {
                match &f.body.statements[0] {
                    Stmt::Let(l) => {
                        match l.init.as_ref().unwrap() {
                            Expr::Nil(_) => {}
                            _ => panic!("expected nil"),
                        }
                    }
                    _ => panic!("expected let"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn test_parse_async_function() {
        let source = r#"
            async fn fetch_data(url: string) -> string {
                return url;
            }
        "#;
        let tokens = tokenize(source).unwrap();
        let ast = parse(&tokens).unwrap();
        
        assert_eq!(ast.declarations.len(), 1);
        match &ast.declarations[0] {
            Decl::Function(f) => {
                assert_eq!(f.name, "fetch_data");
                assert!(f.is_async);
            }
            _ => panic!("expected async function"),
        }
    }

    #[test]
    fn test_parse_await_expression() {
        let source = r#"
            async fn main() {
                let result: string = await fetch("url");
            }
        "#;
        let tokens = tokenize(source).unwrap();
        let ast = parse(&tokens).unwrap();
        
        assert_eq!(ast.declarations.len(), 1);
        match &ast.declarations[0] {
            Decl::Function(f) => {
                assert!(f.is_async);
                match &f.body.statements[0] {
                    Stmt::Let(l) => {
                        match l.init.as_ref().unwrap() {
                            Expr::Await(inner, _) => {
                                match inner.as_ref() {
                                    Expr::Call(_, _, _) => {}
                                    _ => panic!("expected call inside await"),
                                }
                            }
                            _ => panic!("expected await expression"),
                        }
                    }
                    _ => panic!("expected let statement"),
                }
            }
            _ => panic!("expected async function"),
        }
    }

    #[test]
    fn test_parse_variant_decl() {
        let tokens = tokenize("variant Direction { Up, Down, Left, Right }").unwrap();
        let ast = parse(&tokens).unwrap();
        assert_eq!(ast.declarations.len(), 1);
        match &ast.declarations[0] {
            Decl::Variant(v) => {
                assert_eq!(v.name, "Direction");
                assert_eq!(v.cases.len(), 4);
                assert_eq!(v.cases[0].name, "Up");
                assert_eq!(v.cases[3].name, "Right");
            }
            _ => panic!("expected variant declaration"),
        }
    }

    #[test]
    fn test_parse_variant_with_fields() {
        let tokens = tokenize("variant Shape { Circle(radius: float), Rect(w: int, h: int) }").unwrap();
        let ast = parse(&tokens).unwrap();
        match &ast.declarations[0] {
            Decl::Variant(v) => {
                assert_eq!(v.name, "Shape");
                assert_eq!(v.cases.len(), 2);
                assert_eq!(v.cases[0].name, "Circle");
                assert_eq!(v.cases[0].fields.len(), 1);
                assert_eq!(v.cases[1].name, "Rect");
                assert_eq!(v.cases[1].fields.len(), 2);
            }
            _ => panic!("expected variant declaration"),
        }
    }

    #[test]
    fn test_parse_protocol_decl() {
        let tokens = tokenize("protocol Drawable { fn draw(canvas: int); fn size() -> int; }").unwrap();
        let ast = parse(&tokens).unwrap();
        match &ast.declarations[0] {
            Decl::Protocol(p) => {
                assert_eq!(p.name, "Drawable");
                assert_eq!(p.methods.len(), 2);
                assert_eq!(p.methods[0].name, "draw");
                assert_eq!(p.methods[0].params.len(), 1);
                assert_eq!(p.methods[1].name, "size");
                assert_eq!(p.methods[1].return_type, Some(Type::Int));
            }
            _ => panic!("expected protocol declaration"),
        }
    }

    #[test]
    fn test_parse_extension_decl() {
        let tokens = tokenize("extension Color { fn lighten(amount: float) -> float { return amount; } }").unwrap();
        let ast = parse(&tokens).unwrap();
        match &ast.declarations[0] {
            Decl::Extension(ext) => {
                assert_eq!(ext.target, "Color");
                assert_eq!(ext.methods.len(), 1);
                assert_eq!(ext.methods[0].name, "lighten");
            }
            _ => panic!("expected extension declaration"),
        }
    }

    #[test]
    fn test_parse_layer_decl() {
        let tokens = tokenize("layer MyButton { signal on_click; fn body() { } }").unwrap();
        let ast = parse(&tokens).unwrap();
        match &ast.declarations[0] {
            Decl::Layer(l) => {
                assert_eq!(l.name, "MyButton");
                assert_eq!(l.signals.len(), 1);
                assert_eq!(l.signals[0].name, "on_click");
                assert_eq!(l.methods.len(), 1);
                assert_eq!(l.methods[0].name, "body");
            }
            _ => panic!("expected layer declaration"),
        }
    }

    #[test]
    fn test_parse_layer_with_gestures() {
        let tokens = tokenize("layer Card { on_tap { } on_swipe(dir: int) { } }").unwrap();
        let ast = parse(&tokens).unwrap();
        match &ast.declarations[0] {
            Decl::Layer(l) => {
                assert_eq!(l.name, "Card");
                assert_eq!(l.gestures.len(), 2);
                assert_eq!(l.gestures[0].kind, "on_tap");
                assert_eq!(l.gestures[0].params.len(), 0);
                assert_eq!(l.gestures[1].kind, "on_swipe");
                assert_eq!(l.gestures[1].params.len(), 1);
                assert_eq!(l.gestures[1].params[0].name, "dir");
            }
            _ => panic!("expected layer with gestures"),
        }
    }

    #[test]
    fn test_parse_panel_decl() {
        let tokens = tokenize("panel MainWindow { fn root() { } }").unwrap();
        let ast = parse(&tokens).unwrap();
        match &ast.declarations[0] {
            Decl::Panel(p) => {
                assert_eq!(p.name, "MainWindow");
                assert_eq!(p.methods.len(), 1);
                assert_eq!(p.methods[0].name, "root");
            }
            _ => panic!("expected panel declaration"),
        }
    }

    #[test]
    fn test_parse_const_decl() {
        let tokens = tokenize("const MAX_SIZE: int = 100;").unwrap();
        let ast = parse(&tokens).unwrap();
        match &ast.declarations[0] {
            Decl::Const(c) => {
                assert_eq!(c.name, "MAX_SIZE");
                assert_eq!(c.ty, Some(Type::Int));
            }
            _ => panic!("expected const declaration"),
        }
    }

    #[test]
    fn test_parse_typealias_decl() {
        let tokens = tokenize("typealias Color = int;").unwrap();
        let ast = parse(&tokens).unwrap();
        match &ast.declarations[0] {
            Decl::Typealias(t) => {
                assert_eq!(t.name, "Color");
                assert_eq!(t.target, Type::Int);
            }
            _ => panic!("expected typealias declaration"),
        }
    }

    #[test]
    fn test_parse_pub_fn() {
        let tokens = tokenize("pub fn render() { }").unwrap();
        let ast = parse(&tokens).unwrap();
        match &ast.declarations[0] {
            Decl::Function(f) => {
                assert_eq!(f.name, "render");
                assert!(f.is_pub);
            }
            _ => panic!("expected pub function"),
        }
    }

    #[test]
    fn test_parse_emit_stmt() {
        let tokens = tokenize("fn main() { emit on_click(42); }").unwrap();
        let ast = parse(&tokens).unwrap();
        match &ast.declarations[0] {
            Decl::Function(f) => {
                match &f.body.statements[0] {
                    Stmt::Expr(e) => {
                        // emit is lowered to a function call: __emit_on_click(42)
                        match e {
                            Expr::Call(callee, args, _) => {
                                if let Expr::Identifier(name, _) = callee.as_ref() {
                                    assert_eq!(name, "__emit_on_click");
                                }
                                assert_eq!(args.len(), 1);
                            }
                            _ => panic!("expected call expression from emit"),
                        }
                    }
                    _ => panic!("expected expression statement"),
                }
            }
            _ => panic!("expected function"),
        }
    }

    // === Hardening: error paths ===

    fn parse_err(source: &str) -> ParseError {
        let tokens = tokenize(source).unwrap();
        parse(&tokens).unwrap_err()
    }

    #[test]
    fn parse_returns_error_instead_of_empty_program() {
        let err = parse_err("fn main( { }");
        assert!(err.message.starts_with("expected identifier"), "{}", err.message);
        assert_eq!((err.span.line, err.span.column), (1, 10));
    }

    #[test]
    fn parser_survives_empty_token_slice() {
        let err = Parser::new(&[]).parse_program();
        assert!(err.is_ok(), "no tokens means an empty program");
        let mut parser = Parser::new(&[]);
        assert!(parser.is_at_end());
        assert_eq!(parser.advance().kind, TokenKind::Eof);
        assert!(parser.consume(&TokenKind::Fn, "expected 'fn'").is_err());
    }

    #[test]
    fn advance_at_end_of_input_does_not_underflow() {
        let tokens = tokenize("").unwrap();
        let mut parser = Parser::new(&tokens);
        for _ in 0..3 {
            assert_eq!(parser.advance().kind, TokenKind::Eof);
        }
        assert_eq!(parser.current, 0);
    }

    #[test]
    fn unclosed_block_reports_end_of_input_with_hint() {
        let err = parse_err("fn main() {\n  let x = 1;\n");
        assert_eq!(err.message, "unclosed block: expected '}'");
        assert_eq!(err.span.line, 3);
        assert!(err.hint.as_deref().unwrap_or("").contains("1:11"));
    }

    #[test]
    fn end_of_input_errors_name_end_of_input() {
        let err = parse_err("fn main() { let x = ");
        assert!(err.message.ends_with("found end of input"), "{}", err.message);
        assert!(err.hint.is_some());
    }

    #[test]
    fn break_and_continue_require_semicolons() {
        let err = parse_err("fn main() { while true { break } }");
        assert!(err.message.starts_with("expected ';' after 'break'"), "{}", err.message);
        let err = parse_err("fn main() { while true { continue } }");
        assert!(err.message.starts_with("expected ';' after 'continue'"), "{}", err.message);
        let tokens = tokenize("fn main() { while true { break; continue; } }").unwrap();
        assert!(parse(&tokens).is_ok());
    }

    #[test]
    fn deeply_nested_parentheses_are_a_nesting_error() {
        let source = format!("fn main() {{ let x = {}1{}; }}", "(".repeat(5000), ")".repeat(5000));
        let err = parse_err(&source);
        assert_eq!(err.message, format!("expression nested deeper than {} levels", MAX_NESTING_DEPTH));
        assert!(err.hint.is_some());
    }

    #[test]
    fn deeply_nested_blocks_types_and_unary_are_nesting_errors() {
        let blocks = format!("fn main() {}{}", "{".repeat(5000), "}".repeat(5000));
        assert!(parse_err(&blocks).message.contains("block nested deeper"));
        let types = format!("fn f(a: {}int{}) {{ }}", "[".repeat(5000), "]".repeat(5000));
        assert!(parse_err(&types).message.contains("type nested deeper"));
        let unary = format!("fn main() {{ let x = {}1; }}", "-".repeat(5000));
        assert!(parse_err(&unary).message.contains("unary expression nested deeper"));
        let ifs = format!("fn main() {{ if true {{ }} {} }}", "else if true { }".repeat(5000));
        assert!(parse_err(&ifs).message.contains("nested deeper"));
    }

    #[test]
    fn long_operator_chains_are_bounded() {
        let source = format!("fn main() {{ let x = {}1; }}", "1 + ".repeat(50_000));
        let err = parse_err(&source);
        assert_eq!(err.message, format!("expression chain longer than {} links", MAX_NESTING_DEPTH));
        let members = format!("fn main() {{ let x = a{}; }}", ".b".repeat(50_000));
        assert!(parse_err(&members).message.contains("chain longer"));
        let calls = format!("fn main() {{ f{}; }}", "()".repeat(50_000));
        assert!(parse_err(&calls).message.contains("chain longer"));
    }

    #[test]
    fn moderate_nesting_and_chains_still_parse() {
        let parens = format!("fn main() {{ let x = {}1{}; }}", "(".repeat(100), ")".repeat(100));
        assert!(parse(&tokenize(&parens).unwrap()).is_ok());
        let chain = format!("fn main() {{ let x = {}1; }}", "1 + ".repeat(100));
        assert!(parse(&tokenize(&chain).unwrap()).is_ok());
        let blocks = format!("fn main() {}{}", "{".repeat(100), "}".repeat(100));
        assert!(parse(&tokenize(&blocks).unwrap()).is_ok());
    }

    #[test]
    fn chain_budget_is_restored_between_expressions() {
        // Two long-but-legal chains in sequence must not accumulate depth.
        let chain = "1 + ".repeat(100);
        let source = format!("fn main() {{ let x = {}1; let y = {}1; let z = {}1; }}", chain, chain, chain);
        assert!(parse(&tokenize(&source).unwrap()).is_ok());
    }

    #[test]
    fn describe_token_is_readable() {
        assert_eq!(describe_token(&TokenKind::Eof), "end of input");
        assert_eq!(describe_token(&TokenKind::Ident("x".into())), "identifier 'x'");
        assert_eq!(describe_token(&TokenKind::IntLit(7)), "integer literal '7'");
        assert_eq!(describe_token(&TokenKind::StringLit("s".into())), "string literal \"s\"");
        assert_eq!(describe_token(&TokenKind::LBrace), "'{'");
        assert_eq!(describe_token(&TokenKind::Semicolon), "';'");
        assert_eq!(describe_token(&TokenKind::Fn), "Fn");
    }

    #[test]
    fn declaration_error_has_hint() {
        let err = parse_err("42");
        assert!(err.message.starts_with("expected declaration, found integer literal '42'"), "{}", err.message);
        assert!(err.hint.is_some());
    }

    #[test]
    fn parse_error_display_and_hint() {
        let err = ParseError::new("boom", Span::new(2, 3, 0, 1)).with_hint("fix it");
        assert_eq!(err.display(), "error[2:3]: boom");
        assert_eq!(err.to_string(), "error[2:3]: boom");
        assert_eq!(err.hint.as_deref(), Some("fix it"));
    }

    #[test]
    fn match_map_leaves_unmatched_tokens_alone() {
        let tokens = tokenize("x").unwrap();
        let mut parser = Parser::new(&tokens);
        assert_eq!(parser.match_map(&[(TokenKind::Plus, 1), (TokenKind::Minus, 2)]), None);
        assert_eq!(parser.current, 0);
    }

    // === Methods and self ===

    #[test]
    fn struct_body_holds_fields_then_methods() {
        let tokens = tokenize("struct P { x: int, y: int fn len() -> int { return self.x; } mut fn reset() { self.x = 0; } static fn zero() -> P { return P { x: 0, y: 0 }; } }").unwrap();
        let ast = parse(&tokens).unwrap();
        match &ast.declarations[0] {
            Decl::Struct(s) => {
                assert_eq!(s.fields.len(), 2);
                assert_eq!(s.methods.len(), 3);
                assert!(!s.methods[0].is_mut && !s.methods[0].is_static);
                assert!(s.methods[1].is_mut);
                assert!(s.methods[2].is_static);
                match &s.methods[0].body.statements[0] {
                    Stmt::Return(r) => assert!(matches!(r.value, Some(Expr::Member(ref obj, ref f, _)) if matches!(**obj, Expr::SelfRef(_)) && f == "x")),
                    other => panic!("expected return, got {:?}", other),
                }
            }
            other => panic!("expected struct, got {:?}", other),
        }
    }

    #[test]
    fn field_after_method_is_rejected_with_hint() {
        let tokens = tokenize("struct P { fn f() { } x: int }").unwrap();
        let err = parse(&tokens).unwrap_err();
        assert!(err.message.contains("expected a method or '}'"), "{}", err.message);
        assert!(err.hint.is_some());
    }

    #[test]
    fn mut_fn_is_only_valid_inside_type_bodies() {
        let tokens = tokenize("mut fn f() { }").unwrap();
        let err = parse(&tokens).unwrap_err();
        assert!(err.message.contains("'mut fn' is only allowed inside"), "{}", err.message);
        let tokens = tokenize("struct P { static mut fn f() { } }").unwrap();
        let err = parse(&tokens).unwrap_err();
        assert!(err.message.contains("static method cannot be 'mut'"), "{}", err.message);
    }

    #[test]
    fn extension_layer_and_panel_methods_accept_mut() {
        let tokens = tokenize("extension P { mut fn f() { } } layer L { mut fn g() { } } panel W { mut fn h() { } }").unwrap();
        let ast = parse(&tokens).unwrap();
        assert!(matches!(&ast.declarations[0], Decl::Extension(e) if e.methods[0].is_mut));
        assert!(matches!(&ast.declarations[1], Decl::Layer(l) if l.methods[0].is_mut));
        assert!(matches!(&ast.declarations[2], Decl::Panel(p) if p.methods[0].is_mut));
        let tokens = tokenize("extension P { let x = 1; }").unwrap();
        assert!(parse(&tokens).unwrap_err().message.contains("expected a method in extension body"));
    }

    // === Variant patterns ===

    #[test]
    fn variant_patterns_parse_bare_qualified_and_nested() {
        let tokens = tokenize("fn f() { match r { Ok(v) => v, Result.Err(msg) => 0, Maybe.Nothing => 1, Some(Ok(x)) => x, _ => 2 } }").unwrap();
        let ast = parse(&tokens).unwrap();
        let f = match &ast.declarations[0] { Decl::Function(f) => f, other => panic!("{:?}", other) };
        let arms = match &f.body.statements[0] { Stmt::Expr(Expr::Match(_, arms, _)) => arms, other => panic!("{:?}", other) };
        assert_eq!(arms.len(), 5);
        match &arms[0].pattern {
            Pattern::Variant(vp) => { assert_eq!(vp.type_name, None); assert_eq!(vp.case, "Ok"); assert_eq!(vp.args, vec![Pattern::Identifier("v".into())]); }
            other => panic!("{:?}", other),
        }
        match &arms[1].pattern {
            Pattern::Variant(vp) => { assert_eq!(vp.type_name.as_deref(), Some("Result")); assert_eq!(vp.case, "Err"); }
            other => panic!("{:?}", other),
        }
        match &arms[2].pattern {
            Pattern::Variant(vp) => { assert_eq!(vp.type_name.as_deref(), Some("Maybe")); assert!(vp.args.is_empty()); }
            other => panic!("{:?}", other),
        }
        match &arms[3].pattern {
            Pattern::Variant(vp) => assert!(matches!(&vp.args[0], Pattern::Variant(inner) if inner.case == "Ok")),
            other => panic!("{:?}", other),
        }
        assert_eq!(arms[4].pattern, Pattern::Wildcard);
    }

    #[test]
    fn match_statement_and_block_arm_bodies() {
        let tokens = tokenize("fn f() { match x { 1 => { println(\"a\"); 10 } _ => 0 } let y = 1; }").unwrap();
        let ast = parse(&tokens).unwrap();
        let f = match &ast.declarations[0] { Decl::Function(f) => f, other => panic!("{:?}", other) };
        assert_eq!(f.body.statements.len(), 2);
        let arms = match &f.body.statements[0] { Stmt::Expr(Expr::Match(_, arms, _)) => arms, other => panic!("{:?}", other) };
        match &arms[0].body {
            Expr::Block(block, _) => {
                assert_eq!(block.statements.len(), 2);
                assert!(matches!(block.statements[1], Stmt::Expr(Expr::Literal(Literal::Int(10, _)))));
            }
            other => panic!("{:?}", other),
        }
    }

    #[test]
    fn variant_payloads_may_be_positional_or_named() {
        let tokens = tokenize("variant Result { Ok(int), Err(message: string), Pair(int, bool) }").unwrap();
        let ast = parse(&tokens).unwrap();
        let v = match &ast.declarations[0] { Decl::Variant(v) => v, other => panic!("{:?}", other) };
        assert_eq!(v.cases[0].fields[0].name, "_0");
        assert_eq!(v.cases[0].fields[0].ty, Type::Int);
        assert_eq!(v.cases[1].fields[0].name, "message");
        assert_eq!(v.cases[2].fields[1].name, "_1");
        assert_eq!(v.cases[2].fields[1].ty, Type::Bool);
    }

    #[test]
    fn unclosed_pattern_args_are_reported() {
        let tokens = tokenize("fn f() { match r { Ok(v => 1 } }").unwrap();
        let err = parse(&tokens).unwrap_err();
        assert!(err.message.contains("expected ')' after case patterns"), "{}", err.message);
    }

    // === Generics ===

    #[test]
    fn type_parameters_and_generic_types_parse() {
        let tokens = tokenize("fn swap<T>(a: T, b: T) -> T { return b; } struct Box<T: Drawable + Eq> { value: T } variant Result<T, E> { Ok(T), Err(E) } extension Box<T> : P { fn f() { } }").unwrap();
        let ast = parse(&tokens).unwrap();
        match &ast.declarations[0] {
            Decl::Function(f) => { assert_eq!(f.type_params.len(), 1); assert_eq!(f.type_params[0].name, "T"); assert_eq!(f.params[0].ty, Type::Named("T".into())); }
            other => panic!("{:?}", other),
        }
        match &ast.declarations[1] {
            Decl::Struct(s) => assert_eq!(s.type_params[0].bounds, vec!["Drawable".to_string(), "Eq".to_string()]),
            other => panic!("{:?}", other),
        }
        match &ast.declarations[2] {
            Decl::Variant(v) => assert_eq!(v.type_params.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), vec!["T", "E"]),
            other => panic!("{:?}", other),
        }
        match &ast.declarations[3] {
            Decl::Extension(e) => { assert_eq!(e.type_params.len(), 1); assert_eq!(e.protocols, vec!["P".to_string()]); }
            other => panic!("{:?}", other),
        }
    }

    #[test]
    fn nested_generic_type_arguments_split_the_shift_token() {
        let tokens = tokenize("fn f(a: Option<Box<int>>, b: Result<Option<Box<string>>, int>) { }").unwrap();
        let ast = parse(&tokens).unwrap();
        let f = match &ast.declarations[0] { Decl::Function(f) => f, other => panic!("{:?}", other) };
        assert_eq!(f.params[0].ty, Type::Generic("Option".into(), vec![Type::Generic("Box".into(), vec![Type::Int])]));
        assert_eq!(
            f.params[1].ty,
            Type::Generic("Result".into(), vec![
                Type::Generic("Option".into(), vec![Type::Generic("Box".into(), vec![Type::String])]),
                Type::Int,
            ])
        );
    }

    #[test]
    fn generic_syntax_errors_are_reported() {
        let tokens = tokenize("fn f<T, T>() { }").unwrap();
        assert!(parse(&tokens).unwrap_err().message.contains("type parameter 'T' is declared twice"));
        let tokens = tokenize("fn f<>() { }").unwrap();
        assert!(parse(&tokens).is_err());
        let tokens = tokenize("fn f(a: Box<int) { }").unwrap();
        assert!(parse(&tokens).unwrap_err().message.contains("expected '>' after type arguments"));
    }
}
