use std::fmt;
use yomnova_diagnostics::Span;

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Literals
    Integer(i64),
    Float(f64),
    Text(String),
    Bool(bool),
    Identifier(String),

    // Keywords
    Module,
    Fn,
    Return,
    Let,
    Mut,
    Const,
    If,
    Else,
    While,
    For,
    In,
    Flow,
    Struct,
    Enum,
    Match,
    Emit,
    Import,
    Type,

    // Protocol Keywords / Types
    IPv4,
    IPv6,
    Port,
    Endpoint,
    TCP,
    UDP,
    Packet,

    // Operators
    Plus,         // +
    Minus,        // -
    Star,         // *
    Slash,        // /
    Percent,      // %
    Equal,        // =
    EqualEqual,   // ==
    BangEqual,    // !=
    Less,         // <
    LessEqual,    // <=
    Greater,      // >
    GreaterEqual, // >=
    AndAnd,       // &&
    Ampersand,    // &
    OrOr,         // ||
    Bang,         // !
    PipeGreater,  // |>
    Bar,          // | (alternatives in a match arm)
    Arrow,        // ->
    FatArrow,     // =>
    Dot,          // .

    // Punctuation
    OpenParen,    // (
    CloseParen,   // )
    OpenBrace,    // {
    CloseBrace,   // }
    OpenBracket,  // [
    CloseBracket, // ]
    Comma,        // ,
    Colon,        // :
    Semicolon,    // ;
    BangBracket,  // ![

    // Special
    Eof,
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenKind::Integer(n) => write!(f, "integer '{n}'"),
            TokenKind::Float(n) => write!(f, "float '{n}'"),
            TokenKind::Text(s) => write!(f, "text \"{s}\""),
            TokenKind::Bool(b) => write!(f, "bool '{b}'"),
            TokenKind::Identifier(id) => write!(f, "identifier '{id}'"),
            TokenKind::Module => write!(f, "keyword 'module'"),
            TokenKind::Fn => write!(f, "keyword 'fn'"),
            TokenKind::Return => write!(f, "keyword 'return'"),
            TokenKind::Let => write!(f, "keyword 'let'"),
            TokenKind::Mut => write!(f, "keyword 'mut'"),
            TokenKind::Const => write!(f, "keyword 'const'"),
            TokenKind::If => write!(f, "keyword 'if'"),
            TokenKind::Else => write!(f, "keyword 'else'"),
            TokenKind::While => write!(f, "keyword 'while'"),
            TokenKind::For => write!(f, "keyword 'for'"),
            TokenKind::In => write!(f, "keyword 'in'"),
            TokenKind::Flow => write!(f, "keyword 'flow'"),
            TokenKind::Struct => write!(f, "keyword 'struct'"),
            TokenKind::Enum => write!(f, "keyword 'enum'"),
            TokenKind::Match => write!(f, "keyword 'match'"),
            TokenKind::Emit => write!(f, "keyword 'emit'"),
            TokenKind::Import => write!(f, "keyword 'import'"),
            TokenKind::Type => write!(f, "keyword 'type'"),
            TokenKind::IPv4 => write!(f, "keyword 'IPv4'"),
            TokenKind::IPv6 => write!(f, "keyword 'IPv6'"),
            TokenKind::Port => write!(f, "keyword 'Port'"),
            TokenKind::Endpoint => write!(f, "keyword 'Endpoint'"),
            TokenKind::TCP => write!(f, "keyword 'TCP'"),
            TokenKind::UDP => write!(f, "keyword 'UDP'"),
            TokenKind::Packet => write!(f, "keyword 'Packet'"),
            TokenKind::Plus => write!(f, "'+'"),
            TokenKind::Minus => write!(f, "'-'"),
            TokenKind::Star => write!(f, "'*'"),
            TokenKind::Slash => write!(f, "'/'"),
            TokenKind::Percent => write!(f, "'%'"),
            TokenKind::Equal => write!(f, "'='"),
            TokenKind::EqualEqual => write!(f, "'=='"),
            TokenKind::BangEqual => write!(f, "'!='"),
            TokenKind::Less => write!(f, "'<'"),
            TokenKind::LessEqual => write!(f, "'<='"),
            TokenKind::Greater => write!(f, "'>'"),
            TokenKind::GreaterEqual => write!(f, "'>='"),
            TokenKind::AndAnd => write!(f, "'&&'"),
            TokenKind::Ampersand => write!(f, "'&'"),
            TokenKind::OrOr => write!(f, "'||'"),
            TokenKind::Bang => write!(f, "'!'"),
            TokenKind::PipeGreater => write!(f, "'|>'"),
            TokenKind::Bar => write!(f, "'|'"),
            TokenKind::Arrow => write!(f, "'->'"),
            TokenKind::FatArrow => write!(f, "'=>'"),
            TokenKind::Dot => write!(f, "'.'"),
            TokenKind::OpenParen => write!(f, "'('"),
            TokenKind::CloseParen => write!(f, "')'"),
            TokenKind::OpenBrace => write!(f, "'{{'"),
            TokenKind::CloseBrace => write!(f, "'}}'"),
            TokenKind::OpenBracket => write!(f, "'['"),
            TokenKind::CloseBracket => write!(f, "']'"),
            TokenKind::Comma => write!(f, "','"),
            TokenKind::Colon => write!(f, "':'"),
            TokenKind::Semicolon => write!(f, "';'"),
            TokenKind::BangBracket => write!(f, "'!['"),
            TokenKind::Eof => write!(f, "<EOF>"),
        }
    }
}
