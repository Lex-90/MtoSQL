//! M language tokeniser.

/// Token types for the M lexer.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Keywords
    /// `let`
    Let,
    /// `in` keyword
    In,
    /// `each`
    Each,
    /// `and`
    And,
    /// `or`
    Or,
    /// `not`
    Not,
    /// `true`
    True,
    /// `false`
    False,
    /// `null`
    Null,
    /// `type`
    Type,
    /// `as`
    As,
    /// `section`
    Section,
    /// `shared`
    Shared,

    // Literals
    /// A text string literal.
    StringLit(String),
    /// An integer literal.
    IntegerLit(i64),
    /// A floating-point number literal.
    NumberLit(f64),

    // Identifiers
    /// A plain identifier.
    Ident(String),
    /// A quoted identifier `#"..."`.
    QuotedIdent(String),

    // Punctuation
    /// `(`
    LParen,
    /// `)`
    RParen,
    /// `{`
    LBrace,
    /// `}`
    RBrace,
    /// `[`
    LBracket,
    /// `]`
    RBracket,
    /// `,`
    Comma,
    /// `=`
    Eq,
    /// `<>`
    Neq,
    /// `<`
    Lt,
    /// `<=`
    Lte,
    /// `>`
    Gt,
    /// `>=`
    Gte,
    /// `+`
    Plus,
    /// `-`
    Minus,
    /// `*`
    Star,
    /// `/`
    Slash,
    /// `&`
    Ampersand,
    /// `.`
    Dot,
    /// `;`
    Semicolon,
    /// `_` (underscore, used in `each`)
    Underscore,
    /// `=>`
    FatArrow,
    /// `..`
    DotDot,
    /// `...`
    Ellipsis,
    /// `@`
    At,
    /// `#`
    Hash,

    /// End of input.
    Eof,
}

/// A token with its position information.
#[derive(Debug, Clone)]
pub struct SpannedToken {
    /// The token.
    pub token: Token,
    /// 1-indexed line number.
    pub line: usize,
    /// 1-indexed column number.
    pub col: usize,
}

/// Tokenise M source code into a sequence of tokens.
pub fn tokenize(input: &str) -> Result<Vec<SpannedToken>, LexError> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut pos = 0;
    let mut line = 1;
    let mut col = 1;

    while pos < chars.len() {
        // Skip whitespace
        if chars[pos].is_whitespace() {
            if chars[pos] == '\n' {
                line += 1;
                col = 1;
            } else {
                col += 1;
            }
            pos += 1;
            continue;
        }

        // Skip line comments
        if pos + 1 < chars.len() && chars[pos] == '/' && chars[pos + 1] == '/' {
            while pos < chars.len() && chars[pos] != '\n' {
                pos += 1;
            }
            continue;
        }

        // Skip block comments
        if pos + 1 < chars.len() && chars[pos] == '/' && chars[pos + 1] == '*' {
            pos += 2;
            col += 2;
            let mut depth = 1;
            while pos < chars.len() && depth > 0 {
                if pos + 1 < chars.len() && chars[pos] == '/' && chars[pos + 1] == '*' {
                    depth += 1;
                    pos += 2;
                    col += 2;
                } else if pos + 1 < chars.len() && chars[pos] == '*' && chars[pos + 1] == '/' {
                    depth -= 1;
                    pos += 2;
                    col += 2;
                } else {
                    if chars[pos] == '\n' {
                        line += 1;
                        col = 1;
                    } else {
                        col += 1;
                    }
                    pos += 1;
                }
            }
            continue;
        }

        let start_line = line;
        let start_col = col;

        // String literal
        if chars[pos] == '"' {
            pos += 1;
            col += 1;
            let mut s = String::new();
            while pos < chars.len() && chars[pos] != '"' {
                if chars[pos] == '\\' && pos + 1 < chars.len() {
                    pos += 1;
                    col += 1;
                    match chars[pos] {
                        'n' => s.push('\n'),
                        't' => s.push('\t'),
                        'r' => s.push('\r'),
                        '"' => s.push('"'),
                        '\\' => s.push('\\'),
                        c => {
                            s.push('\\');
                            s.push(c);
                        }
                    }
                } else {
                    if chars[pos] == '\n' {
                        line += 1;
                        col = 0;
                    }
                    s.push(chars[pos]);
                }
                pos += 1;
                col += 1;
            }
            if pos < chars.len() {
                pos += 1; // skip closing quote
                col += 1;
            } else {
                return Err(LexError {
                    line: start_line,
                    col: start_col,
                    message: "Unterminated string literal".to_string(),
                });
            }
            // Check for M's double-quote escape: "" inside strings
            // M uses "" for escaping quotes, not backslash
            tokens.push(SpannedToken {
                token: Token::StringLit(s),
                line: start_line,
                col: start_col,
            });
            continue;
        }

        // Quoted identifier #"..."
        if chars[pos] == '#' && pos + 1 < chars.len() && chars[pos + 1] == '"' {
            pos += 2;
            col += 2;
            let mut s = String::new();
            while pos < chars.len() && chars[pos] != '"' {
                s.push(chars[pos]);
                if chars[pos] == '\n' {
                    line += 1;
                    col = 0;
                }
                pos += 1;
                col += 1;
            }
            if pos < chars.len() {
                pos += 1;
                col += 1;
            }
            tokens.push(SpannedToken {
                token: Token::QuotedIdent(s),
                line: start_line,
                col: start_col,
            });
            continue;
        }

        // Numbers
        if chars[pos].is_ascii_digit() {
            let mut num_str = String::new();
            let mut is_float = false;
            while pos < chars.len() && (chars[pos].is_ascii_digit() || chars[pos] == '.') {
                if chars[pos] == '.' {
                    // Check if next char is also a digit (decimal point vs member access)
                    if pos + 1 < chars.len() && chars[pos + 1].is_ascii_digit() {
                        is_float = true;
                    } else {
                        break;
                    }
                }
                num_str.push(chars[pos]);
                pos += 1;
                col += 1;
            }
            if is_float {
                let val: f64 = num_str.parse().map_err(|_| LexError {
                    line: start_line,
                    col: start_col,
                    message: format!("Invalid number: {}", num_str),
                })?;
                tokens.push(SpannedToken {
                    token: Token::NumberLit(val),
                    line: start_line,
                    col: start_col,
                });
            } else {
                let val: i64 = num_str.parse().map_err(|_| LexError {
                    line: start_line,
                    col: start_col,
                    message: format!("Invalid integer: {}", num_str),
                })?;
                tokens.push(SpannedToken {
                    token: Token::IntegerLit(val),
                    line: start_line,
                    col: start_col,
                });
            }
            continue;
        }

        // Identifiers and keywords
        if chars[pos].is_alphabetic() || chars[pos] == '_' {
            let mut ident = String::new();
            while pos < chars.len()
                && (chars[pos].is_alphanumeric() || chars[pos] == '_' || chars[pos] == '.')
            {
                // Handle dotted identifiers like Table.SelectRows
                if chars[pos] == '.' {
                    // Only treat as part of identifier if next char is alphabetic (function call)
                    if pos + 1 < chars.len() && chars[pos + 1].is_alphabetic() {
                        ident.push(chars[pos]);
                        pos += 1;
                        col += 1;
                        continue;
                    } else {
                        break;
                    }
                }
                ident.push(chars[pos]);
                pos += 1;
                col += 1;
            }

            let token = match ident.as_str() {
                "let" => Token::Let,
                "in" => Token::In,
                "each" => Token::Each,
                "and" => Token::And,
                "or" => Token::Or,
                "not" => Token::Not,
                "true" => Token::True,
                "false" => Token::False,
                "null" => Token::Null,
                "type" => Token::Type,
                "as" => Token::As,
                "section" => Token::Section,
                "shared" => Token::Shared,
                _ => Token::Ident(ident),
            };
            tokens.push(SpannedToken {
                token,
                line: start_line,
                col: start_col,
            });
            continue;
        }

        // Punctuation
        let token = match chars[pos] {
            '(' => {
                pos += 1;
                col += 1;
                Token::LParen
            }
            ')' => {
                pos += 1;
                col += 1;
                Token::RParen
            }
            '{' => {
                pos += 1;
                col += 1;
                Token::LBrace
            }
            '}' => {
                pos += 1;
                col += 1;
                Token::RBrace
            }
            '[' => {
                pos += 1;
                col += 1;
                Token::LBracket
            }
            ']' => {
                pos += 1;
                col += 1;
                Token::RBracket
            }
            ',' => {
                pos += 1;
                col += 1;
                Token::Comma
            }
            ';' => {
                pos += 1;
                col += 1;
                Token::Semicolon
            }
            '@' => {
                pos += 1;
                col += 1;
                Token::At
            }
            '+' => {
                pos += 1;
                col += 1;
                Token::Plus
            }
            '-' => {
                pos += 1;
                col += 1;
                Token::Minus
            }
            '*' => {
                pos += 1;
                col += 1;
                Token::Star
            }
            '/' => {
                pos += 1;
                col += 1;
                Token::Slash
            }
            '&' => {
                pos += 1;
                col += 1;
                Token::Ampersand
            }
            '.' => {
                if pos + 1 < chars.len() && chars[pos + 1] == '.' {
                    if pos + 2 < chars.len() && chars[pos + 2] == '.' {
                        pos += 3;
                        col += 3;
                        Token::Ellipsis
                    } else {
                        pos += 2;
                        col += 2;
                        Token::DotDot
                    }
                } else {
                    pos += 1;
                    col += 1;
                    Token::Dot
                }
            }
            '=' => {
                if pos + 1 < chars.len() && chars[pos + 1] == '>' {
                    pos += 2;
                    col += 2;
                    Token::FatArrow
                } else {
                    pos += 1;
                    col += 1;
                    Token::Eq
                }
            }
            '<' => {
                if pos + 1 < chars.len() && chars[pos + 1] == '>' {
                    pos += 2;
                    col += 2;
                    Token::Neq
                } else if pos + 1 < chars.len() && chars[pos + 1] == '=' {
                    pos += 2;
                    col += 2;
                    Token::Lte
                } else {
                    pos += 1;
                    col += 1;
                    Token::Lt
                }
            }
            '>' => {
                if pos + 1 < chars.len() && chars[pos + 1] == '=' {
                    pos += 2;
                    col += 2;
                    Token::Gte
                } else {
                    pos += 1;
                    col += 1;
                    Token::Gt
                }
            }
            '#' => {
                pos += 1;
                col += 1;
                Token::Hash
            }
            c => {
                return Err(LexError {
                    line: start_line,
                    col: start_col,
                    message: format!("Unexpected character: '{}'", c),
                });
            }
        };
        tokens.push(SpannedToken {
            token,
            line: start_line,
            col: start_col,
        });
    }

    tokens.push(SpannedToken {
        token: Token::Eof,
        line,
        col,
    });

    Ok(tokens)
}

/// Lexer error.
#[derive(Debug, Clone)]
pub struct LexError {
    /// Line number.
    pub line: usize,
    /// Column number.
    pub col: usize,
    /// Error message.
    pub message: String,
}

impl std::fmt::Display for LexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Lex error at {}:{}: {}",
            self.line, self.col, self.message
        )
    }
}

impl std::error::Error for LexError {}
