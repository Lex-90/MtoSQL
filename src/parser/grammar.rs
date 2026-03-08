/// M recursive-descent parser → AST.
use super::ast::*;
use super::lexer::{SpannedToken, Token};

/// Parser state.
pub struct Parser {
    tokens: Vec<SpannedToken>,
    pos: usize,
}

/// Parse error.
#[derive(Debug, Clone)]
pub struct ParseError {
    /// Line number.
    pub line: usize,
    /// Column number.
    pub col: usize,
    /// Error message.
    pub message: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Parse error at {}:{}: {}",
            self.line, self.col, self.message
        )
    }
}

impl std::error::Error for ParseError {}

impl Parser {
    /// Create a new parser from a token stream.
    pub fn new(tokens: Vec<SpannedToken>) -> Self {
        Self { tokens, pos: 0 }
    }

    /// Parse the entire input as an M document.
    pub fn parse_document(&mut self) -> Result<MDocument, ParseError> {
        // Check if this is section syntax
        if self.peek_token() == &Token::Section {
            return self.parse_section();
        }
        let expr = self.parse_expr()?;
        Ok(MDocument::Expression(expr))
    }

    /// Parse a section: `section Name; shared Binding = expr; ...`
    fn parse_section(&mut self) -> Result<MDocument, ParseError> {
        self.expect(&Token::Section)?;
        let name = self.expect_ident()?;
        self.expect(&Token::Semicolon)?;

        let mut bindings = Vec::new();
        while self.peek_token() != &Token::Eof {
            let is_shared = if self.peek_token() == &Token::Shared {
                self.advance();
                true
            } else {
                false
            };

            let binding_name = self.expect_ident()?;
            self.expect(&Token::Eq)?;
            let expr = self.parse_expr()?;
            self.expect(&Token::Semicolon)?;
            bindings.push((is_shared, binding_name, expr));
        }

        Ok(MDocument::Section { name, bindings })
    }

    /// Parse an expression.
    pub fn parse_expr(&mut self) -> Result<MExpr, ParseError> {
        self.parse_or_expr()
    }

    /// Parse `or` expressions.
    fn parse_or_expr(&mut self) -> Result<MExpr, ParseError> {
        let mut left = self.parse_and_expr()?;
        while self.peek_token() == &Token::Or {
            self.advance();
            let right = self.parse_and_expr()?;
            left = MExpr::BinaryOp {
                op: BinOp::Or,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    /// Parse `and` expressions.
    fn parse_and_expr(&mut self) -> Result<MExpr, ParseError> {
        let mut left = self.parse_not_expr()?;
        while self.peek_token() == &Token::And {
            self.advance();
            let right = self.parse_not_expr()?;
            left = MExpr::BinaryOp {
                op: BinOp::And,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    /// Parse `not` expressions.
    fn parse_not_expr(&mut self) -> Result<MExpr, ParseError> {
        if self.peek_token() == &Token::Not {
            self.advance();
            let expr = self.parse_not_expr()?;
            return Ok(MExpr::UnaryOp {
                op: UnOp::Not,
                expr: Box::new(expr),
            });
        }
        self.parse_comparison()
    }

    /// Parse comparison expressions.
    fn parse_comparison(&mut self) -> Result<MExpr, ParseError> {
        let left = self.parse_concat()?;
        let op = match self.peek_token() {
            Token::Eq => Some(BinOp::Eq),
            Token::Neq => Some(BinOp::Neq),
            Token::Lt => Some(BinOp::Lt),
            Token::Lte => Some(BinOp::Lte),
            Token::Gt => Some(BinOp::Gt),
            Token::Gte => Some(BinOp::Gte),
            _ => None,
        };
        if let Some(op) = op {
            self.advance();
            let right = self.parse_concat()?;
            Ok(MExpr::BinaryOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            })
        } else {
            Ok(left)
        }
    }

    /// Parse `&` concatenation.
    fn parse_concat(&mut self) -> Result<MExpr, ParseError> {
        let mut left = self.parse_additive()?;
        while self.peek_token() == &Token::Ampersand {
            self.advance();
            let right = self.parse_additive()?;
            left = MExpr::BinaryOp {
                op: BinOp::Concat,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    /// Parse additive expressions (`+`, `-`).
    fn parse_additive(&mut self) -> Result<MExpr, ParseError> {
        let mut left = self.parse_multiplicative()?;
        loop {
            let op = match self.peek_token() {
                Token::Plus => Some(BinOp::Add),
                Token::Minus => Some(BinOp::Sub),
                _ => None,
            };
            if let Some(op) = op {
                self.advance();
                let right = self.parse_multiplicative()?;
                left = MExpr::BinaryOp {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                };
            } else {
                break;
            }
        }
        Ok(left)
    }

    /// Parse multiplicative expressions (`*`, `/`).
    fn parse_multiplicative(&mut self) -> Result<MExpr, ParseError> {
        let mut left = self.parse_as_expr()?;
        loop {
            let op = match self.peek_token() {
                Token::Star => Some(BinOp::Mul),
                Token::Slash => Some(BinOp::Div),
                _ => None,
            };
            if let Some(op) = op {
                self.advance();
                let right = self.parse_as_expr()?;
                left = MExpr::BinaryOp {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                };
            } else {
                break;
            }
        }
        Ok(left)
    }

    /// Parse `as type` annotations.
    fn parse_as_expr(&mut self) -> Result<MExpr, ParseError> {
        let expr = self.parse_unary()?;
        if self.peek_token() == &Token::As {
            self.advance();
            let ty = self.parse_type()?;
            Ok(MExpr::TypeAnnotation {
                expr: Box::new(expr),
                ty,
            })
        } else {
            Ok(expr)
        }
    }

    /// Parse unary prefix expressions (`-`, `not`).
    fn parse_unary(&mut self) -> Result<MExpr, ParseError> {
        if self.peek_token() == &Token::Minus {
            self.advance();
            let expr = self.parse_unary()?;
            return Ok(MExpr::UnaryOp {
                op: UnOp::Neg,
                expr: Box::new(expr),
            });
        }
        self.parse_postfix()
    }

    /// Parse postfix expressions (field access, index access, function calls).
    fn parse_postfix(&mut self) -> Result<MExpr, ParseError> {
        let mut expr = self.parse_primary()?;

        loop {
            match self.peek_token() {
                Token::LBrace => {
                    // Index access: expr{key}
                    self.advance();
                    let key = self.parse_expr()?;
                    self.expect(&Token::RBrace)?;
                    // Check for [Data] field access after index
                    if self.peek_token() == &Token::LBracket {
                        self.advance();
                        let field = self.expect_ident_or_string()?;
                        self.expect(&Token::RBracket)?;
                        expr = MExpr::FieldAccess {
                            expr: Box::new(MExpr::IndexAccess {
                                expr: Box::new(expr),
                                key: Box::new(key),
                            }),
                            field,
                        };
                    } else {
                        expr = MExpr::IndexAccess {
                            expr: Box::new(expr),
                            key: Box::new(key),
                        };
                    }
                }
                Token::LParen => {
                    // Function call: only if expr is an identifier
                    if let MExpr::Identifier(name) = &expr {
                        let name = name.clone();
                        self.advance();
                        let args = self.parse_arg_list()?;
                        self.expect(&Token::RParen)?;
                        expr = MExpr::FunctionCall { name, args };
                    } else {
                        break;
                    }
                }
                _ => break,
            }
        }

        Ok(expr)
    }

    /// Parse a primary expression.
    fn parse_primary(&mut self) -> Result<MExpr, ParseError> {
        match self.peek_token().clone() {
            Token::Let => self.parse_let(),
            Token::Each => {
                self.advance();
                let expr = self.parse_expr()?;
                Ok(MExpr::EachExpr(Box::new(expr)))
            }
            Token::StringLit(s) => {
                let s = s.clone();
                self.advance();
                Ok(MExpr::Literal(MLiteral::Text(s)))
            }
            Token::IntegerLit(n) => {
                self.advance();
                Ok(MExpr::Literal(MLiteral::Integer(n)))
            }
            Token::NumberLit(n) => {
                self.advance();
                Ok(MExpr::Literal(MLiteral::Number(n)))
            }
            Token::True => {
                self.advance();
                Ok(MExpr::Literal(MLiteral::Bool(true)))
            }
            Token::False => {
                self.advance();
                Ok(MExpr::Literal(MLiteral::Bool(false)))
            }
            Token::Null => {
                self.advance();
                Ok(MExpr::Literal(MLiteral::Null))
            }
            Token::Type => {
                // type annotation used standalone
                self.advance();
                let ty = self.parse_type()?;
                // Return as a type reference identifier
                Ok(MExpr::Identifier(format!("type {}", type_to_string(&ty))))
            }
            Token::Ident(name) => {
                let name = name.clone();
                self.advance();
                Ok(MExpr::Identifier(name))
            }
            Token::QuotedIdent(name) => {
                let name = name.clone();
                self.advance();
                Ok(MExpr::Identifier(name))
            }
            Token::LBrace => {
                // List literal: {e1, e2, ...}
                self.advance();
                let mut items = Vec::new();
                if self.peek_token() != &Token::RBrace {
                    items.push(self.parse_list_item()?);
                    while self.peek_token() == &Token::Comma {
                        self.advance();
                        if self.peek_token() == &Token::RBrace {
                            break;
                        }
                        items.push(self.parse_list_item()?);
                    }
                }
                self.expect(&Token::RBrace)?;
                Ok(MExpr::List(items))
            }
            Token::LBracket => {
                // Record literal or row field access: [k1=v1, ...] or [ColumnName]
                self.advance();
                // Check if this is a row field access [ColumnName]
                if let Token::Ident(_) | Token::QuotedIdent(_) = self.peek_token() {
                    let name = self.expect_ident()?;
                    if self.peek_token() == &Token::RBracket {
                        self.advance();
                        return Ok(MExpr::RowField(name));
                    }
                    // It's a record: [Name = value, ...]
                    if self.peek_token() == &Token::Eq {
                        self.advance();
                        let value = self.parse_expr()?;
                        let mut fields = vec![(name, value)];
                        while self.peek_token() == &Token::Comma {
                            self.advance();
                            if self.peek_token() == &Token::RBracket {
                                break;
                            }
                            let key = self.expect_ident()?;
                            self.expect(&Token::Eq)?;
                            let val = self.parse_expr()?;
                            fields.push((key, val));
                        }
                        self.expect(&Token::RBracket)?;
                        return Ok(MExpr::Record(fields));
                    }
                    // Just a row field with different following token
                    self.expect(&Token::RBracket)?;
                    return Ok(MExpr::RowField(name));
                }
                // Empty record
                self.expect(&Token::RBracket)?;
                Ok(MExpr::Record(Vec::new()))
            }
            Token::LParen => {
                self.advance();
                let expr = self.parse_expr()?;
                self.expect(&Token::RParen)?;
                Ok(expr)
            }
            Token::Underscore => {
                self.advance();
                Ok(MExpr::Identifier("_".to_string()))
            }
            _ => {
                let st = &self.tokens[self.pos];
                Err(ParseError {
                    line: st.line,
                    col: st.col,
                    message: format!("Unexpected token: {:?}", st.token),
                })
            }
        }
    }

    /// Parse a list item (which may be a nested list like {{"a","b"}}).
    fn parse_list_item(&mut self) -> Result<MExpr, ParseError> {
        self.parse_expr()
    }

    /// Parse a `let ... in` expression.
    fn parse_let(&mut self) -> Result<MExpr, ParseError> {
        self.expect(&Token::Let)?;
        let mut bindings = Vec::new();

        loop {
            let name = self.expect_ident()?;
            self.expect(&Token::Eq)?;
            let value = self.parse_expr()?;
            bindings.push((name, value));

            if self.peek_token() == &Token::Comma {
                self.advance();
            } else {
                break;
            }
        }

        self.expect(&Token::In)?;
        let body = self.parse_expr()?;

        Ok(MExpr::Let {
            bindings,
            body: Box::new(body),
        })
    }

    /// Parse a function argument list.
    fn parse_arg_list(&mut self) -> Result<Vec<MExpr>, ParseError> {
        let mut args = Vec::new();
        if self.peek_token() != &Token::RParen {
            args.push(self.parse_expr()?);
            while self.peek_token() == &Token::Comma {
                self.advance();
                if self.peek_token() == &Token::RParen {
                    break;
                }
                args.push(self.parse_expr()?);
            }
        }
        Ok(args)
    }

    /// Parse an M type.
    fn parse_type(&mut self) -> Result<MType, ParseError> {
        let name = match self.peek_token().clone() {
            Token::Ident(name) => {
                let n = name.clone();
                self.advance();
                n
            }
            _ => {
                let st = &self.tokens[self.pos];
                return Err(ParseError {
                    line: st.line,
                    col: st.col,
                    message: format!("Expected type name, found {:?}", st.token),
                });
            }
        };
        Ok(string_to_type(&name))
    }

    // Helper methods

    fn peek_token(&self) -> &Token {
        if self.pos < self.tokens.len() {
            &self.tokens[self.pos].token
        } else {
            &Token::Eof
        }
    }

    fn advance(&mut self) {
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
    }

    fn expect(&mut self, expected: &Token) -> Result<(), ParseError> {
        let actual = self.peek_token().clone();
        if std::mem::discriminant(&actual) == std::mem::discriminant(expected) {
            self.advance();
            Ok(())
        } else {
            let st = &self.tokens[self.pos.min(self.tokens.len() - 1)];
            Err(ParseError {
                line: st.line,
                col: st.col,
                message: format!("Expected {:?}, found {:?}", expected, actual),
            })
        }
    }

    fn expect_ident(&mut self) -> Result<String, ParseError> {
        match self.peek_token().clone() {
            Token::Ident(name) => {
                let n = name.clone();
                self.advance();
                Ok(n)
            }
            Token::QuotedIdent(name) => {
                let n = name.clone();
                self.advance();
                Ok(n)
            }
            _ => {
                let st = &self.tokens[self.pos.min(self.tokens.len() - 1)];
                Err(ParseError {
                    line: st.line,
                    col: st.col,
                    message: format!("Expected identifier, found {:?}", st.token),
                })
            }
        }
    }

    fn expect_ident_or_string(&mut self) -> Result<String, ParseError> {
        match self.peek_token().clone() {
            Token::Ident(name) | Token::QuotedIdent(name) => {
                self.advance();
                Ok(name)
            }
            Token::StringLit(s) => {
                self.advance();
                Ok(s)
            }
            _ => {
                let st = &self.tokens[self.pos.min(self.tokens.len() - 1)];
                Err(ParseError {
                    line: st.line,
                    col: st.col,
                    message: format!("Expected identifier or string, found {:?}", st.token),
                })
            }
        }
    }

    /// Get the current line number.
    pub fn current_line(&self) -> usize {
        if self.pos < self.tokens.len() {
            self.tokens[self.pos].line
        } else if !self.tokens.is_empty() {
            self.tokens.last().unwrap().line
        } else {
            1
        }
    }
}

/// Convert an M type name to MType.
pub fn string_to_type(name: &str) -> MType {
    match name {
        "text" => MType::Text,
        "number" => MType::Number,
        "integer" => MType::Integer,
        "logical" => MType::Logical,
        "date" => MType::Date,
        "datetime" => MType::DateTime,
        "datetimezone" => MType::DateTimeZone,
        "duration" => MType::Duration,
        "binary" => MType::Binary,
        "table" => MType::Table,
        "any" => MType::Any,
        "Int64.Type" => MType::Integer,
        other => MType::Unknown(other.to_string()),
    }
}

/// Convert MType to a string representation.
pub fn type_to_string(ty: &MType) -> &str {
    match ty {
        MType::Text => "text",
        MType::Number => "number",
        MType::Integer => "integer",
        MType::Logical => "logical",
        MType::Date => "date",
        MType::DateTime => "datetime",
        MType::DateTimeZone => "datetimezone",
        MType::Duration => "duration",
        MType::Binary => "binary",
        MType::Table => "table",
        MType::Any => "any",
        MType::Unknown(s) => s.as_str(),
    }
}
