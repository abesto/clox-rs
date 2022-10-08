mod back;
mod error;
mod front;
mod rules;
mod variables;

use std::collections::HashMap;

use crate::{
    chunk::{Chunk, ConstantLongIndex},
    compiler::rules::{make_rules, Rules},
    scanner::{Scanner, Token, TokenKind},
};

struct Local<'a> {
    name: Token<'a>,
    depth: i32,
    mutable: bool,
}

pub struct Compiler<'a> {
    scanner: Scanner<'a>,
    previous: Option<Token<'a>>,
    current: Option<Token<'a>>,
    had_error: bool,
    panic_mode: bool,
    chunk: Chunk,
    globals_by_name: HashMap<String, ConstantLongIndex>,
    rules: Rules<'a>,
    locals: Vec<Local<'a>>,
    scope_depth: i32,
}

impl<'a> Compiler<'a> {
    #[must_use]
    fn new(source: &'a [u8]) -> Self {
        Self {
            chunk: Chunk::new("<main>"),
            globals_by_name: HashMap::new(),
            scanner: Scanner::new(source),
            previous: None,
            current: None,
            had_error: false,
            panic_mode: false,
            rules: make_rules(),
            locals: Vec::new(),
            scope_depth: 0,
        }
    }

    fn compile_(mut self) -> Option<Chunk> {
        self.advance();

        while !self.match_(TokenKind::Eof) {
            self.declaration();
        }

        self.end();
        if self.had_error {
            None
        } else {
            Some(self.chunk)
        }
    }

    pub fn compile(source: &'a [u8]) -> Option<Chunk> {
        Self::new(source).compile_()
    }

    fn end(&mut self) {
        self.emit_return();

        #[cfg(feature = "print_code")]
        if !self.had_error {
            println!("{:?}", self.chunk);
        }
    }
}
