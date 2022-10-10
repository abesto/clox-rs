use num_enum::{IntoPrimitive, TryFromPrimitive};
use shrinkwraprs::Shrinkwrap;

use crate::types::Line;

#[derive(Shrinkwrap, PartialEq, Eq, Clone, Copy)]
pub struct TokenLength(pub usize);

#[derive(IntoPrimitive, TryFromPrimitive, PartialEq, Eq, Clone, Copy, Debug)]
#[repr(u8)]
pub enum TokenKind {
    // Single-Character Tokens.
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    Colon,
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
    Case,
    Class,
    Continue,
    Default,
    Else,
    False,
    For,
    Fun,
    If,
    Nil,
    Or,
    Print,
    Return,
    Switch,
    Super,
    This,
    True,
    Const,
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

#[derive(Clone, Debug)]
pub struct Token<'a> {
    pub kind: TokenKind,
    pub lexeme: &'a [u8],
    pub line: Line,
}

impl<'a> Token<'a> {
    pub fn as_str(&'a self) -> &'a str {
        std::str::from_utf8(self.lexeme).unwrap()
    }
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
                b':' => TK::Colon,
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
                c if c.is_ascii_alphanumeric() || c == &b'_' => return self.identifier(),
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
                        while !matches!(self.peek(), Some(b'\n') | None) {
                            self.advance();
                        }
                    } else {
                        break;
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

    fn identifier(&mut self) -> Token<'a> {
        while self
            .peek()
            .map(|c| c.is_ascii_alphanumeric() || c == &b'_')
            .unwrap_or(false)
        {
            self.advance();
        }
        let token_kind = self.identifier_type();
        self.make_token(token_kind)
    }

    fn identifier_type(&mut self) -> TokenKind {
        match self.source[self.start] {
            b'a' => self.check_keyword(1, "nd", TokenKind::And),
            b'c' => match self.source.get(self.start + 1) {
                Some(b'a') => self.check_keyword(2, "se", TokenKind::Case),
                Some(b'l') => self.check_keyword(2, "ass", TokenKind::Class),
                Some(b'o') => match self.source.get(self.start + 2) {
                    Some(b'n') => match self.source.get(self.start + 3) {
                        Some(b's') => self.check_keyword(4, "t", TokenKind::Const),
                        Some(b't') => self.check_keyword(4, "inue", TokenKind::Continue),
                        _ => TokenKind::Identifier,
                    },
                    _ => TokenKind::Identifier,
                },
                _ => TokenKind::Identifier,
            },
            b'd' => self.check_keyword(1, "efault", TokenKind::Default),
            b'e' => self.check_keyword(1, "lse", TokenKind::Else),
            b'f' => match self.source.get(self.start + 1) {
                Some(b'a') => self.check_keyword(2, "lse", TokenKind::False),
                Some(b'o') => self.check_keyword(2, "r", TokenKind::For),
                Some(b'u') => self.check_keyword(2, "n", TokenKind::Fun),
                _ => TokenKind::Identifier,
            },
            b'i' => self.check_keyword(1, "f", TokenKind::If),
            b'n' => self.check_keyword(1, "il", TokenKind::Nil),
            b'o' => self.check_keyword(1, "r", TokenKind::Or),
            b'p' => self.check_keyword(1, "rint", TokenKind::Print),
            b'r' => self.check_keyword(1, "eturn", TokenKind::Return),
            b's' => match self.source.get(self.start + 1) {
                Some(b'u') => self.check_keyword(2, "per", TokenKind::Super),
                Some(b'w') => self.check_keyword(2, "itch", TokenKind::Switch),
                _ => TokenKind::Identifier,
            },
            b't' => match self.source.get(self.start + 1) {
                Some(b'h') => self.check_keyword(2, "is", TokenKind::This),
                Some(b'r') => self.check_keyword(2, "ue", TokenKind::True),
                _ => TokenKind::Identifier,
            },
            b'v' => self.check_keyword(1, "ar", TokenKind::Var),
            b'w' => self.check_keyword(1, "hile", TokenKind::While),
            _ => TokenKind::Identifier,
        }
    }

    fn check_keyword(&self, start: usize, rest: &str, kind: TokenKind) -> TokenKind {
        let from = self.source.len().min(self.start + start);
        let to = self.source.len().min(from + rest.len());
        if &self.source[from..to] == rest.as_bytes() {
            kind
        } else {
            TokenKind::Identifier
        }
    }

    fn make_token(&self, kind: TokenKind) -> Token<'a> {
        let to = self.current.min(self.source.len());
        let from = to.min(self.start);
        Token {
            kind,
            lexeme: &self.source[from..to],
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
