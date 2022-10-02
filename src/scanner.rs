use num_enum::{IntoPrimitive, TryFromPrimitive};
use shrinkwraprs::Shrinkwrap;

use crate::types::Line;

#[derive(Shrinkwrap, PartialEq, Eq, Clone, Copy)]
pub struct TokenLength(pub usize);

#[derive(IntoPrimitive, TryFromPrimitive, PartialEq, Eq, Clone, Debug)]
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

impl std::fmt::Display for TokenKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.pad(&format!("{:?}", self))
    }
}

pub struct Token<'a> {
    pub kind: TokenKind,
    pub lexeme: &'a [u8],
    pub line: Line,
}

pub struct Scanner<'a> {
    source: &'a [u8],
    start: usize,
    current: usize,
    line: Line,
}

impl<'a> Scanner<'a> {
    #[must_use]
    pub fn new(source: &'a [u8]) -> Self {
        Self {
            source,
            start: 0,
            current: 0,
            line: Line(1),
        }
    }

    pub fn scan(&mut self) -> Token<'a> {
        use TokenKind as TK;
        self.skip_whitespace();
        self.start = self.current;

        let token_kind = match self.advance() {
            None => TK::Eof,
            Some(c) => match c {
                b'(' => TK::LeftParen,
                b')' => TK::RightParen,
                b'{' => TK::LeftBrace,
                b'}' => TK::RightBrace,
                b';' => TK::Semicolon,
                b',' => TK::Comma,
                b'.' => TK::Dot,
                b'-' => TK::Minus,
                b'+' => TK::Plus,
                b'/' => TK::Slash,
                b'*' => TK::Star,
                b'!' => {
                    if self.match_(b'=') {
                        TK::BangEqual
                    } else {
                        TK::Bang
                    }
                }
                b'=' => {
                    if self.match_(b'=') {
                        TK::EqualEqual
                    } else {
                        TK::Equal
                    }
                }
                b'<' => {
                    if self.match_(b'=') {
                        TK::LessEqual
                    } else {
                        TK::Less
                    }
                }
                b'>' => {
                    if self.match_(b'=') {
                        TK::GreaterEqual
                    } else {
                        TK::Greater
                    }
                }
                b'"' => return self.string(),
                c if c.is_ascii_digit() => return self.number(),
                _ => return self.error_token("Unexpected character."),
            },
        };
        self.make_token(token_kind)
    }

    fn advance(&mut self) -> Option<&u8> {
        self.current += 1;
        self.source.get(self.current - 1)
    }

    fn peek(&self) -> Option<&u8> {
        self.source.get(self.current)
    }

    fn peek_next(&self) -> Option<&u8> {
        self.source.get(self.current + 1)
    }

    fn match_(&mut self, expected: u8) -> bool {
        match self.source.get(self.current) {
            Some(actual) if actual == &expected => {
                self.current += 1;
                true
            }
            _ => false,
        }
    }

    fn skip_whitespace(&mut self) {
        loop {
            match self.peek() {
                Some(b' ' | b'\t' | b'\r') => {
                    self.advance();
                }
                Some(b'\n') => {
                    self.advance();
                    *self.line += 1;
                }
                // Line comment
                Some(b'/') => {
                    if self.peek_next() == Some(&b'/') {
                        while let Some(b'\n') | None = self.peek() {
                            self.advance();
                        }
                    }
                }
                _ => break,
            }
        }
    }

    fn string(&mut self) -> Token<'a> {
        while self.peek().map(|c| c != &b'"').unwrap_or(false) {
            if self.peek() == Some(&b'\n') {
                *self.line += 1;
            }
            self.advance();
        }

        // The closing quote.
        if !self.match_(b'"') {
            return self.error_token("Unterminated string.");
        }

        self.make_token(TokenKind::String)
    }

    fn number(&mut self) -> Token<'a> {
        while self.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            self.advance();
        }

        // Fractions
        if self.peek() == Some(&b'.')
            && self
                .peek_next()
                .map(|c| c.is_ascii_digit())
                .unwrap_or(false)
        {
            self.advance();
            while self.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                self.advance();
            }
        }

        self.make_token(TokenKind::Number)
    }

    fn make_token(&self, kind: TokenKind) -> Token<'a> {
        Token {
            kind,
            lexeme: &self.source[self.start..self.current.min(self.source.len())],
            line: self.line,
        }
    }

    fn error_token(&self, msg: &'static str) -> Token<'a> {
        Token {
            kind: TokenKind::Error,
            lexeme: msg.as_bytes(),
            line: self.line,
        }
    }
}
