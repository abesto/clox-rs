use itertools::{peek_nth, Itertools};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use shrinkwraprs::Shrinkwrap;

use crate::types::Line;

#[derive(Shrinkwrap, PartialEq, Eq, Clone, Copy)]
pub struct TokenLength(pub usize);

#[derive(IntoPrimitive, TryFromPrimitive, PartialEq, Eq, Clone)]
#[repr(u8)]
pub enum TokenKind {
    // Single-Character Tokens.
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    Comma,
    Dot,
    Minus,
    Plus,
    Semicolon,
    Slash,
    Star,

    // One Or Two Character Tokens.
    Bang,
    BangEqual,
    Equal,
    EqualEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,

    // Literals.
    Identifier,
    String,
    Number,

    // Keywords.
    And,
    Class,
    Else,
    False,
    For,
    Fun,
    If,
    Nil,
    Or,
    Print,
    Return,
    Super,
    This,
    True,
    Var,
    While,

    Error,
    Eof,
}

pub struct Token<'a> {
    pub kind: TokenKind,
    pub lexeme: &'a str,
    pub line: Line,
}

pub struct Scanner<'a> {
    current: itertools::PeekNth<std::str::Chars<'a>>,
    lexeme: &'a str,
    line: Line,
}

impl<'a> Scanner<'a> {
    #[must_use]
    pub fn new(source: &'a str) -> Self {
        Self {
            current: peek_nth(source.chars()),
            lexeme: Default::default(),
            line: Line(1),
        }
    }

    pub fn scan(&mut self) -> Token<'a> {
        self.lexeme = Default::default();

        if self.is_at_end() {
            return self.make_token(TokenKind::Eof);
        }

        self.error_token("Unexpected character.")
    }

    fn is_at_end(&mut self) -> bool {
        self.current.peek().is_some()
    }

    fn make_token(&self, kind: TokenKind) -> Token<'a> {
        Token {
            kind,
            lexeme: self.lexeme,
            line: self.line,
        }
    }

    fn error_token(&self, msg: &'static str) -> Token<'a> {
        Token {
            kind: TokenKind::Error,
            lexeme: msg,
            line: self.line,
        }
    }
}
