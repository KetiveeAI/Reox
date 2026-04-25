// REOX Compiler - Parser
// Recursive descent parser with Pratt parsing for expressions
// Zero external dependencies

mod ast;

pub use ast::*;

use crate::lexer::{Token, TokenKind, Span};

/// Parser error
#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
}

impl ParseError {
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }

    pub fn display(&self) -> String {
        format!(
            "error[{}:{}]: {}",
            self.span.line, self.span.column, self.message
        )
    }
}

/// REOX Parser
pub struct Parser<'a> {
    tokens: &'a [Token],
    current: usize,
}

impl<'a> Parser<'a> {
    pub fn new(tokens: &'a [Token]) -> Self {
        Self { tokens, current: 0 }
    }

    // === Utility Methods ===

    fn peek(&self) -> &Token {
        self.tokens.get(self.current).unwrap_or_else(|| {
            self.tokens.last().expect("token stream should have EOF")
        })
    }

    fn peek_kind(&self) -> &TokenKind {
        &self.peek().kind
    }

    fn is_at_end(&self) -> bool {
        matches!(self.peek_kind(), TokenKind::Eof)
    }

    fn advance(&mut self) -> &Token {
        if !self.is_at_end() {
            self.current += 1;
        }
        self.tokens.get(self.current - 1).unwrap()
    }

    fn check(&self, kind: &TokenKind) -> bool {
        std::mem::discriminant(self.peek_kind()) == std::mem::discriminant(kind)
    }

    fn consume(&mut self, kind: &TokenKind, msg: &str) -> Result<&Token, ParseError> {
        if self.check(kind) {
            Ok(self.advance())
        } else {
            Err(ParseError::new(
                format!("{}, found {:?}", msg, self.peek_kind()),
                self.peek().span,
            ))
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
            TokenKind::Struct => self.parse_struct_decl().map(Decl::Struct),
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
            _ => Err(ParseError::new(
                format!("expected declaration, found {:?}", self.peek_kind()),
                self.peek().span,
            )),
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
        let start_span = self.peek().span;
        self.consume(&TokenKind::Fn, "expected 'fn'")?;

        let name = self.parse_identifier()?;
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
            params,
            return_type,
            body,
            is_async,
            is_pub,
            is_static,
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

    fn parse_struct_decl(&mut self) -> Result<StructDecl, ParseError> {
        let start_span = self.peek().span;
        self.consume(&TokenKind::Struct, "expected 'struct'")?;

        let name = self.parse_identifier()?;
        self.consume(&TokenKind::LBrace, "expected '{'")?;

        let mut fields = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            fields.push(self.parse_field()?);
            if !self.match_token(&[TokenKind::Comma]) {
                break;
            }
        }

        self.consume(&TokenKind::RBrace, "expected '}'")?;

        Ok(StructDecl {
            name,
            fields,
            span: start_span,
        })
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
                Ok(Type::Named(name))
            }
            TokenKind::LBracket => {
                self.advance();
                let inner = self.parse_type()?;
                self.consume(&TokenKind::RBracket, "expected ']'")?;
                Ok(Type::Array(Box::new(inner)))
            }
            _ => Err(ParseError::new(
                format!("expected type, found {:?}", self.peek_kind()),
                self.peek().span,
            )),
        }
    }

    fn parse_block(&mut self) -> Result<Block, ParseError> {
        let span = self.peek().span;
        self.consume(&TokenKind::LBrace, "expected '{'")?;

        let mut statements = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            statements.push(self.parse_statement()?);
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
                self.consume(&TokenKind::Semicolon, "expected ';' after 'break'").ok();
                Ok(Stmt::Break(span))
            }
            TokenKind::Ident(name) if name == "continue" => {
                let span = self.peek().span;
                self.advance();
                self.consume(&TokenKind::Semicolon, "expected ';' after 'continue'").ok();
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
        self.parse_assignment()
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
        if self.match_token(&[
            TokenKind::PlusEq, TokenKind::MinusEq, 
            TokenKind::StarEq, TokenKind::SlashEq, TokenKind::PercentEq
        ]) {
            let op = match self.tokens.get(self.current - 1).map(|t| &t.kind) {
                Some(TokenKind::PlusEq) => CompoundOp::AddEq,
                Some(TokenKind::MinusEq) => CompoundOp::SubEq,
                Some(TokenKind::StarEq) => CompoundOp::MulEq,
                Some(TokenKind::SlashEq) => CompoundOp::DivEq,
                Some(TokenKind::PercentEq) => CompoundOp::ModEq,
                _ => unreachable!(),
            };
            let span = self.peek().span;
            let value = self.parse_assignment()?;
            return Ok(Expr::CompoundAssign(Box::new(expr), op, Box::new(value), span));
        }

        Ok(expr)
    }

    fn parse_nullish_coalesce(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_or()?;

        while self.match_token(&[TokenKind::QuestionQuestion]) {
            let span = self.peek().span;
            let right = self.parse_or()?;
            left = Expr::NullCoalesce(Box::new(left), Box::new(right), span);
        }

        Ok(left)
    }

    fn parse_or(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_and()?;

        while self.match_token(&[TokenKind::Or]) {
            let span = self.peek().span;
            let right = self.parse_and()?;
            left = Expr::Binary(Box::new(left), BinOp::Or, Box::new(right), span);
        }

        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_bitwise_or()?;

        while self.match_token(&[TokenKind::And]) {
            let span = self.peek().span;
            let right = self.parse_bitwise_or()?;
            left = Expr::Binary(Box::new(left), BinOp::And, Box::new(right), span);
        }

        Ok(left)
    }

    fn parse_bitwise_or(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_bitwise_xor()?;

        while self.match_token(&[TokenKind::BitwiseOr]) {
            let span = self.peek().span;
            let right = self.parse_bitwise_xor()?;
            left = Expr::Binary(Box::new(left), BinOp::BitwiseOr, Box::new(right), span);
        }

        Ok(left)
    }

    fn parse_bitwise_xor(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_bitwise_and()?;

        while self.match_token(&[TokenKind::BitwiseXor]) {
            let span = self.peek().span;
            let right = self.parse_bitwise_and()?;
            left = Expr::Binary(Box::new(left), BinOp::BitwiseXor, Box::new(right), span);
        }

        Ok(left)
    }

    fn parse_bitwise_and(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_equality()?;

        while self.match_token(&[TokenKind::BitwiseAnd]) {
            let span = self.peek().span;
            let right = self.parse_equality()?;
            left = Expr::Binary(Box::new(left), BinOp::BitwiseAnd, Box::new(right), span);
        }

        Ok(left)
    }

    fn parse_equality(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_comparison()?;

        while self.match_token(&[TokenKind::EqEq, TokenKind::BangEq]) {
            let op = match self.tokens.get(self.current - 1).map(|t| &t.kind) {
                Some(TokenKind::EqEq) => BinOp::Eq,
                Some(TokenKind::BangEq) => BinOp::Ne,
                _ => unreachable!(),
            };
            let span = self.peek().span;
            let right = self.parse_comparison()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right), span);
        }

        Ok(left)
    }

    fn parse_comparison(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_shift()?;

        while self.match_token(&[TokenKind::Lt, TokenKind::Gt, TokenKind::LtEq, TokenKind::GtEq]) {
            let op = match self.tokens.get(self.current - 1).map(|t| &t.kind) {
                Some(TokenKind::Lt) => BinOp::Lt,
                Some(TokenKind::Gt) => BinOp::Gt,
                Some(TokenKind::LtEq) => BinOp::Le,
                Some(TokenKind::GtEq) => BinOp::Ge,
                _ => unreachable!(),
            };
            let span = self.peek().span;
            let right = self.parse_shift()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right), span);
        }

        Ok(left)
    }

    fn parse_shift(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_term()?;

        while self.match_token(&[TokenKind::ShiftLeft, TokenKind::ShiftRight]) {
            let op = match self.tokens.get(self.current - 1).map(|t| &t.kind) {
                Some(TokenKind::ShiftLeft) => BinOp::ShiftLeft,
                Some(TokenKind::ShiftRight) => BinOp::ShiftRight,
                _ => unreachable!(),
            };
            let span = self.peek().span;
            let right = self.parse_term()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right), span);
        }

        Ok(left)
    }

    fn parse_term(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_factor()?;

        while self.match_token(&[TokenKind::Plus, TokenKind::Minus]) {
            let op = match self.tokens.get(self.current - 1).map(|t| &t.kind) {
                Some(TokenKind::Plus) => BinOp::Add,
                Some(TokenKind::Minus) => BinOp::Sub,
                _ => unreachable!(),
            };
            let span = self.peek().span;
            let right = self.parse_factor()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right), span);
        }

        Ok(left)
    }

    fn parse_factor(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_unary()?;

        while self.match_token(&[TokenKind::Star, TokenKind::Slash, TokenKind::Percent]) {
            let op = match self.tokens.get(self.current - 1).map(|t| &t.kind) {
                Some(TokenKind::Star) => BinOp::Mul,
                Some(TokenKind::Slash) => BinOp::Div,
                Some(TokenKind::Percent) => BinOp::Mod,
                _ => unreachable!(),
            };
            let span = self.peek().span;
            let right = self.parse_unary()?;
            left = Expr::Binary(Box::new(left), op, Box::new(right), span);
        }

        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        // Await expression: await expr
        if self.match_token(&[TokenKind::Await]) {
            let span = self.peek().span;
            let operand = self.parse_unary()?;
            return Ok(Expr::Await(Box::new(operand), span));
        }
        
        // Pre-increment/decrement
        if self.match_token(&[TokenKind::PlusPlus]) {
            let span = self.peek().span;
            let operand = self.parse_unary()?;
            return Ok(Expr::PreIncrement(Box::new(operand), span));
        }
        if self.match_token(&[TokenKind::MinusMinus]) {
            let span = self.peek().span;
            let operand = self.parse_unary()?;
            return Ok(Expr::PreDecrement(Box::new(operand), span));
        }
        
        // Unary operators: -, !, ~
        if self.match_token(&[TokenKind::Minus, TokenKind::Bang, TokenKind::BitwiseNot]) {
            let op = match self.tokens.get(self.current - 1).map(|t| &t.kind) {
                Some(TokenKind::Minus) => UnaryOp::Neg,
                Some(TokenKind::Bang) => UnaryOp::Not,
                Some(TokenKind::BitwiseNot) => UnaryOp::BitwiseNot,
                _ => unreachable!(),
            };
            let span = self.peek().span;
            let right = self.parse_unary()?;
            return Ok(Expr::Unary(op, Box::new(right), span));
        }

        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_call()?;

        // Post-increment/decrement
        loop {
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

        Ok(expr)
    }

    fn parse_call(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_primary()?;

        loop {
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
                        crate::lexer::token::StringPart::Literal(s) => {
                            exprs.push(Expr::Literal(Literal::String(s, span)));
                        }
                        crate::lexer::token::StringPart::Expr(code) => {
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
            _ => Err(ParseError::new(
                format!("expected expression, found {:?}", token.kind),
                token.span,
            )),
        }
    }

    fn parse_identifier(&mut self) -> Result<String, ParseError> {
        match self.peek_kind().clone() {
            TokenKind::Ident(name) => {
                self.advance();
                Ok(name)
            }
            _ => Err(ParseError::new(
                format!("expected identifier, found {:?}", self.peek_kind()),
                self.peek().span,
            )),
        }
    }

    fn parse_match_arm(&mut self) -> Result<MatchArm, ParseError> {
        let span = self.peek().span;
        let pattern = self.parse_pattern()?;
        self.consume(&TokenKind::FatArrow, "expected '=>' after pattern")?;
        
        // Body can be a single expression or a block
        let body = if self.check(&TokenKind::LBrace) {
            // Block body - parse statements and use last as value
            let block = self.parse_block()?;
            if let Some(Stmt::Expr(expr)) = block.statements.last() {
                expr.clone()
            } else {
                Expr::Nil(span)
            }
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
                    Ok(Pattern::Wildcard)
                } else {
                    Ok(Pattern::Identifier(name))
                }
            }
            _ => Err(ParseError::new(
                format!("expected pattern, found {:?}", token.kind),
                token.span,
            )),
        }
    }

    // === UI-Specific Declaration Parsing ===

    /// Parse: variant Name { Case1, Case2(field: Type), ... }
    fn parse_variant_decl(&mut self, is_pub: bool) -> Result<VariantDecl, ParseError> {
        let span = self.peek().span;
        self.consume(&TokenKind::Variant, "expected 'variant'")?;
        let name = self.parse_identifier()?;
        self.consume(&TokenKind::LBrace, "expected '{' after variant name")?;

        let mut cases = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            let case_span = self.peek().span;
            let case_name = self.parse_identifier()?;

            let fields = if self.match_token(&[TokenKind::LParen]) {
                let mut f = Vec::new();
                if !self.check(&TokenKind::RParen) {
                    loop {
                        f.push(self.parse_field()?);
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
        Ok(VariantDecl { name, cases, is_pub, span })
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
            let is_pub = self.match_token(&[TokenKind::Pub]);
            let is_static = self.match_token(&[TokenKind::Static]);
            methods.push(self.parse_fn_decl_with_modifiers(false, is_pub, is_static)?);
        }

        self.consume(&TokenKind::RBrace, "expected '}'")?;
        Ok(ExtensionDecl { target, protocols, methods, span })
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
                        TokenKind::OnTap => "on_tap",
                        TokenKind::OnPan => "on_pan",
                        TokenKind::OnSwipe => "on_swipe",
                        TokenKind::OnPinch => "on_pinch",
                        TokenKind::OnRotate => "on_rotate",
                        _ => unreachable!(),
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
                TokenKind::Fn | TokenKind::Pub | TokenKind::Static => {
                    let is_pub = self.match_token(&[TokenKind::Pub]);
                    let is_static = self.match_token(&[TokenKind::Static]);
                    methods.push(self.parse_fn_decl_with_modifiers(false, is_pub, is_static)?);
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
                TokenKind::Fn => {
                    methods.push(self.parse_fn_decl(false)?);
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
                    return Err(ParseError::new(
                        format!("unexpected token in panel body: {:?}", self.peek_kind()),
                        self.peek().span,
                    ));
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
            TokenKind::Struct => self.parse_struct_decl().map(Decl::Struct),
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
            _ => Err(ParseError::new(
                format!("expected declaration after 'pub', found {:?}", self.peek_kind()),
                self.peek().span,
            )),
        }
    }

    /// Parse: static fn ...
    fn parse_static_decl(&mut self) -> Result<Decl, ParseError> {
        self.advance(); // consume 'static'
        match self.peek_kind() {
            TokenKind::Fn => self.parse_fn_decl_with_modifiers(false, false, true).map(Decl::Function),
            _ => Err(ParseError::new(
                format!("expected 'fn' after 'static', found {:?}", self.peek_kind()),
                self.peek().span,
            )),
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

/// Convenience type for backward compatibility
pub type Ast = Program;

/// Parse tokens into AST
pub fn parse(tokens: &[Token]) -> Ast {
    let mut parser = Parser::new(tokens);
    match parser.parse_program() {
        Ok(program) => program,
        Err(e) => {
            eprintln!("{}", e.display());
            Program { declarations: vec![] }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;

    #[test]
    fn test_parse_simple_function() {
        let tokens = tokenize("fn main() { }").unwrap();
        let ast = parse(&tokens);
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
        let ast = parse(&tokens);
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
        let ast = parse(&tokens);
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
        let ast = parse(&tokens);
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
        let ast = parse(&tokens);
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
        let ast = parse(&tokens);
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
        let ast = parse(&tokens);
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
        let ast = parse(&tokens);
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
        let ast = parse(&tokens);
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
        let ast = parse(&tokens);
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
        let ast = parse(&tokens);
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
        let ast = parse(&tokens);
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
        let ast = parse(&tokens);
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
        let ast = parse(&tokens);
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
        let ast = parse(&tokens);
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
        let ast = parse(&tokens);
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
        let ast = parse(&tokens);
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
        let ast = parse(&tokens);
        
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
        let ast = parse(&tokens);
        
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
        let ast = parse(&tokens);
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
        let ast = parse(&tokens);
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
        let ast = parse(&tokens);
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
        let ast = parse(&tokens);
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
        let ast = parse(&tokens);
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
        let ast = parse(&tokens);
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
        let ast = parse(&tokens);
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
        let ast = parse(&tokens);
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
        let ast = parse(&tokens);
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
        let ast = parse(&tokens);
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
        let ast = parse(&tokens);
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
}
