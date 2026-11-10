pub mod token;

pub use token::{Token, TokenKind};
use yomnova_diagnostics::{Diagnostic, Span};

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum LexerError {
    #[error("Unexpected character '{ch}' at {span}")]
    UnexpectedChar { ch: char, span: Span },
    #[error("Unterminated string literal starting at {span}")]
    UnterminatedString { span: Span },
    #[error("Unterminated block comment starting at {span}")]
    UnterminatedComment { span: Span },
    #[error("Invalid integer literal '{raw}' at {span}")]
    InvalidInteger { raw: String, span: Span },
    #[error("Invalid float literal '{raw}' at {span}")]
    InvalidFloat { raw: String, span: Span },
}

impl LexerError {
    pub fn to_diagnostic(&self) -> Diagnostic {
        match self {
            LexerError::UnexpectedChar { span, .. } => {
                Diagnostic::error(self.to_string(), Some(*span))
            }
            LexerError::UnterminatedString { span } => {
                Diagnostic::error(self.to_string(), Some(*span))
                    .with_hint("Close the string literal with a closing quote '\"'")
            }
            LexerError::UnterminatedComment { span } => {
                Diagnostic::error(self.to_string(), Some(*span))
                    .with_hint("Close the block comment with '*/'")
            }
            LexerError::InvalidInteger { span, .. } => {
                Diagnostic::error(self.to_string(), Some(*span))
            }
            LexerError::InvalidFloat { span, .. } => {
                Diagnostic::error(self.to_string(), Some(*span))
            }
        }
    }
}

pub struct Lexer<'a> {
    chars: Vec<(usize, char)>,
    cursor: usize,
    line: usize,
    col: usize,
    _source: &'a str,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        let chars: Vec<(usize, char)> = source.char_indices().collect();
        Self {
            chars,
            cursor: 0,
            line: 1,
            col: 1,
            _source: source,
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<Token>, LexerError> {
        let mut tokens = Vec::new();

        loop {
            self.skip_whitespace_and_comments()?;
            if self.is_at_end() {
                let span = self.current_span(0);
                tokens.push(Token::new(TokenKind::Eof, span));
                break;
            }

            let token = self.next_token()?;
            tokens.push(token);
        }

        Ok(tokens)
    }

    fn skip_whitespace_and_comments(&mut self) -> Result<(), LexerError> {
        while !self.is_at_end() {
            let (_, ch) = self.peek();
            match ch {
                ' ' | '\t' | '\r' => {
                    self.advance();
                }
                '\n' => {
                    self.advance();
                    self.line += 1;
                    self.col = 1;
                }
                '/' => {
                    if let Some((_, '/')) = self.peek_second() {
                        // Single-line comment
                        self.advance();
                        self.advance();
                        while !self.is_at_end() && self.peek().1 != '\n' {
                            self.advance();
                        }
                    } else if let Some((_, '*')) = self.peek_second() {
                        // Multi-line block comment
                        let start_span = self.current_span(2);
                        self.advance();
                        self.advance();
                        let mut closed = false;
                        while !self.is_at_end() {
                            if self.peek().1 == '\n' {
                                self.advance();
                                self.line += 1;
                                self.col = 1;
                            } else if self.peek().1 == '*' {
                                if let Some((_, '/')) = self.peek_second() {
                                    self.advance();
                                    self.advance();
                                    closed = true;
                                    break;
                                } else {
                                    self.advance();
                                }
                            } else {
                                self.advance();
                            }
                        }
                        if !closed {
                            return Err(LexerError::UnterminatedComment { span: start_span });
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

    fn next_token(&mut self) -> Result<Token, LexerError> {
        let (start_idx, ch) = self.advance();
        let start_line = self.line;
        let start_col = self.col - 1;

        let make_span = |end_idx: usize, col_offset: usize| {
            Span::new(start_idx, end_idx, start_line, start_col + col_offset)
        };

        // Identifiers & keywords
        if ch.is_ascii_alphabetic() || ch == '_' {
            let mut id = String::new();
            id.push(ch);
            while !self.is_at_end()
                && (self.peek().1.is_ascii_alphanumeric() || self.peek().1 == '_')
            {
                id.push(self.advance().1);
            }
            let end_idx = if self.is_at_end() {
                self.chars
                    .last()
                    .map(|(i, c)| i + c.len_utf8())
                    .unwrap_or(start_idx + id.len())
            } else {
                self.peek().0
            };
            let span = Span::new(start_idx, end_idx, start_line, start_col);
            let kind = match id.as_str() {
                "module" => TokenKind::Module,
                "fn" => TokenKind::Fn,
                "return" => TokenKind::Return,
                "let" => TokenKind::Let,
                "mut" => TokenKind::Mut,
                "const" => TokenKind::Const,
                "if" => TokenKind::If,
                "else" => TokenKind::Else,
                "while" => TokenKind::While,
                "for" => TokenKind::For,
                "in" => TokenKind::In,
                "flow" => TokenKind::Flow,
                "struct" => TokenKind::Struct,
                "enum" => TokenKind::Enum,
                "match" => TokenKind::Match,
                "emit" => TokenKind::Emit,
                "import" => TokenKind::Import,
                "type" => TokenKind::Type,
                "true" => TokenKind::Bool(true),
                "false" => TokenKind::Bool(false),
                "IPv4" => TokenKind::IPv4,
                "IPv6" => TokenKind::IPv6,
                "Port" => TokenKind::Port,
                "Endpoint" => TokenKind::Endpoint,
                "TCP" => TokenKind::TCP,
                "UDP" => TokenKind::UDP,
                "Packet" => TokenKind::Packet,
                _ => TokenKind::Identifier(id),
            };
            return Ok(Token::new(kind, span));
        }

        // Numbers (Integer and Float)
        if ch.is_ascii_digit() {
            let mut num_str = String::new();
            num_str.push(ch);
            let mut is_float = false;

            while !self.is_at_end() && self.peek().1.is_ascii_digit() {
                num_str.push(self.advance().1);
            }

            if !self.is_at_end() && self.peek().1 == '.' {
                if let Some((_, next_ch)) = self.peek_second() {
                    if next_ch.is_ascii_digit() {
                        is_float = true;
                        num_str.push(self.advance().1); // '.'
                        while !self.is_at_end() && self.peek().1.is_ascii_digit() {
                            num_str.push(self.advance().1);
                        }
                    }
                }
            }

            let end_idx = if self.is_at_end() {
                start_idx + num_str.len()
            } else {
                self.peek().0
            };
            let span = Span::new(start_idx, end_idx, start_line, start_col);

            return if is_float {
                match num_str.parse::<f64>() {
                    Ok(f) => Ok(Token::new(TokenKind::Float(f), span)),
                    Err(_) => Err(LexerError::InvalidFloat { raw: num_str, span }),
                }
            } else {
                match num_str.parse::<i64>() {
                    Ok(n) => Ok(Token::new(TokenKind::Integer(n), span)),
                    Err(_) => Err(LexerError::InvalidInteger { raw: num_str, span }),
                }
            };
        }

        // Strings
        if ch == '"' {
            let mut s = String::new();
            let mut closed = false;
            let start_span = Span::new(start_idx, start_idx + 1, start_line, start_col);

            while !self.is_at_end() {
                let (_, next_ch) = self.advance();
                if next_ch == '"' {
                    closed = true;
                    break;
                } else if next_ch == '\\' {
                    if self.is_at_end() {
                        break;
                    }
                    let (_, esc) = self.advance();
                    match esc {
                        'n' => s.push('\n'),
                        't' => s.push('\t'),
                        'r' => s.push('\r'),
                        '\\' => s.push('\\'),
                        '"' => s.push('"'),
                        '0' => s.push('\0'),
                        other => s.push(other),
                    }
                } else if next_ch == '\n' {
                    self.line += 1;
                    self.col = 1;
                    s.push(next_ch);
                } else {
                    s.push(next_ch);
                }
            }

            if !closed {
                return Err(LexerError::UnterminatedString { span: start_span });
            }

            let end_idx = if self.is_at_end() {
                start_idx + s.len() + 2
            } else {
                self.peek().0
            };
            let span = Span::new(start_idx, end_idx, start_line, start_col);
            return Ok(Token::new(TokenKind::Text(s), span));
        }

        // Symbols and Operators
        let span1 = make_span(start_idx + 1, 0);

        match ch {
            '+' => Ok(Token::new(TokenKind::Plus, span1)),
            '-' => {
                if !self.is_at_end() && self.peek().1 == '>' {
                    let (end_idx, _) = self.advance();
                    Ok(Token::new(TokenKind::Arrow, make_span(end_idx + 1, 0)))
                } else {
                    Ok(Token::new(TokenKind::Minus, span1))
                }
            }
            '*' => Ok(Token::new(TokenKind::Star, span1)),
            '/' => Ok(Token::new(TokenKind::Slash, span1)),
            '%' => Ok(Token::new(TokenKind::Percent, span1)),
            '(' => Ok(Token::new(TokenKind::OpenParen, span1)),
            ')' => Ok(Token::new(TokenKind::CloseParen, span1)),
            '{' => Ok(Token::new(TokenKind::OpenBrace, span1)),
            '}' => Ok(Token::new(TokenKind::CloseBrace, span1)),
            '[' => Ok(Token::new(TokenKind::OpenBracket, span1)),
            ']' => Ok(Token::new(TokenKind::CloseBracket, span1)),
            ',' => Ok(Token::new(TokenKind::Comma, span1)),
            ':' => Ok(Token::new(TokenKind::Colon, span1)),
            ';' => Ok(Token::new(TokenKind::Semicolon, span1)),
            '.' => Ok(Token::new(TokenKind::Dot, span1)),
            '=' => {
                if !self.is_at_end() && self.peek().1 == '=' {
                    let (end_idx, _) = self.advance();
                    Ok(Token::new(TokenKind::EqualEqual, make_span(end_idx + 1, 0)))
                } else if !self.is_at_end() && self.peek().1 == '>' {
                    let (end_idx, _) = self.advance();
                    Ok(Token::new(TokenKind::FatArrow, make_span(end_idx + 1, 0)))
                } else {
                    Ok(Token::new(TokenKind::Equal, span1))
                }
            }
            '!' => {
                if !self.is_at_end() && self.peek().1 == '=' {
                    let (end_idx, _) = self.advance();
                    Ok(Token::new(TokenKind::BangEqual, make_span(end_idx + 1, 0)))
                } else if !self.is_at_end() && self.peek().1 == '[' {
                    let (end_idx, _) = self.advance();
                    Ok(Token::new(
                        TokenKind::BangBracket,
                        make_span(end_idx + 1, 0),
                    ))
                } else {
                    Ok(Token::new(TokenKind::Bang, span1))
                }
            }
            '<' => {
                if !self.is_at_end() && self.peek().1 == '=' {
                    let (end_idx, _) = self.advance();
                    Ok(Token::new(TokenKind::LessEqual, make_span(end_idx + 1, 0)))
                } else {
                    Ok(Token::new(TokenKind::Less, span1))
                }
            }
            '>' => {
                if !self.is_at_end() && self.peek().1 == '=' {
                    let (end_idx, _) = self.advance();
                    Ok(Token::new(
                        TokenKind::GreaterEqual,
                        make_span(end_idx + 1, 0),
                    ))
                } else {
                    Ok(Token::new(TokenKind::Greater, span1))
                }
            }
            '&' => {
                if !self.is_at_end() && self.peek().1 == '&' {
                    let (end_idx, _) = self.advance();
                    Ok(Token::new(TokenKind::AndAnd, make_span(end_idx + 1, 0)))
                } else {
                    Ok(Token::new(TokenKind::Ampersand, span1))
                }
            }
            '|' => {
                if !self.is_at_end() && self.peek().1 == '|' {
                    let (end_idx, _) = self.advance();
                    Ok(Token::new(TokenKind::OrOr, make_span(end_idx + 1, 0)))
                } else if !self.is_at_end() && self.peek().1 == '>' {
                    let (end_idx, _) = self.advance();
                    Ok(Token::new(
                        TokenKind::PipeGreater,
                        make_span(end_idx + 1, 0),
                    ))
                } else {
                    Ok(Token::new(TokenKind::Bar, span1))
                }
            }
            _ => Err(LexerError::UnexpectedChar { ch, span: span1 }),
        }
    }

    fn is_at_end(&self) -> bool {
        self.cursor >= self.chars.len()
    }

    fn peek(&self) -> (usize, char) {
        if self.is_at_end() {
            (
                self.chars
                    .last()
                    .map(|(i, c)| i + c.len_utf8())
                    .unwrap_or(0),
                '\0',
            )
        } else {
            self.chars[self.cursor]
        }
    }

    fn peek_second(&self) -> Option<(usize, char)> {
        if self.cursor + 1 < self.chars.len() {
            Some(self.chars[self.cursor + 1])
        } else {
            None
        }
    }

    fn advance(&mut self) -> (usize, char) {
        let item = self.chars[self.cursor];
        self.cursor += 1;
        self.col += 1;
        item
    }

    fn current_span(&self, len: usize) -> Span {
        let (idx, _) = self.peek();
        Span::new(idx, idx + len, self.line, self.col)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lex_hello_world() {
        let src = r#"module genesis
fn main() -> Int {
    emit("Hello from YOMNOVA")
    return 0
}"#;
        let mut lexer = Lexer::new(src);
        let tokens = lexer.tokenize().expect("Tokenization failed");

        assert_eq!(tokens[0].kind, TokenKind::Module);
        assert_eq!(tokens[1].kind, TokenKind::Identifier("genesis".to_string()));
        assert_eq!(tokens[2].kind, TokenKind::Fn);
        assert_eq!(tokens[3].kind, TokenKind::Identifier("main".to_string()));
        assert_eq!(tokens[4].kind, TokenKind::OpenParen);
        assert_eq!(tokens[5].kind, TokenKind::CloseParen);
        assert_eq!(tokens[6].kind, TokenKind::Arrow);
        assert_eq!(tokens[7].kind, TokenKind::Identifier("Int".to_string()));
        assert_eq!(tokens[8].kind, TokenKind::OpenBrace);
        assert_eq!(tokens[9].kind, TokenKind::Emit);
        assert_eq!(tokens[10].kind, TokenKind::OpenParen);
        assert_eq!(
            tokens[11].kind,
            TokenKind::Text("Hello from YOMNOVA".to_string())
        );
        assert_eq!(tokens[12].kind, TokenKind::CloseParen);
        assert_eq!(tokens[13].kind, TokenKind::Return);
        assert_eq!(tokens[14].kind, TokenKind::Integer(0));
        assert_eq!(tokens[15].kind, TokenKind::CloseBrace);
        assert_eq!(tokens[16].kind, TokenKind::Eof);
    }

    #[test]
    fn test_lex_pipeline_and_network_keywords() {
        let src = r#"flow result = [1, 2] |> filter(x => x > 1)
let ip: IPv4 = IPv4("127.0.0.1")
let p: Port = Port(8080)"#;
        let mut lexer = Lexer::new(src);
        let tokens = lexer.tokenize().expect("Tokenization failed");
        assert_eq!(tokens[0].kind, TokenKind::Flow);
        assert!(tokens.iter().any(|t| t.kind == TokenKind::PipeGreater));
        assert!(tokens.iter().any(|t| t.kind == TokenKind::IPv4));
        assert!(tokens.iter().any(|t| t.kind == TokenKind::Port));
    }

    #[test]
    fn test_lex_comments() {
        let src = r#"// Single line comment
/* Multi-line
   comment */
let a = 42"#;
        let mut lexer = Lexer::new(src);
        let tokens = lexer.tokenize().expect("Tokenization failed");
        assert_eq!(tokens[0].kind, TokenKind::Let);
        assert_eq!(tokens[1].kind, TokenKind::Identifier("a".to_string()));
        assert_eq!(tokens[2].kind, TokenKind::Equal);
        assert_eq!(tokens[3].kind, TokenKind::Integer(42));
    }

    #[test]
    fn test_lex_unterminated_string() {
        let src = r#"let s = "Unclosed string"#;
        let mut lexer = Lexer::new(src);
        let err = lexer.tokenize().unwrap_err();
        match err {
            LexerError::UnterminatedString { .. } => {}
            other => panic!("Expected UnterminatedString, got {other:?}"),
        }
    }
}
