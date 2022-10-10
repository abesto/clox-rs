mod back;
mod error;
mod front;
mod rules;
mod variables;

use std::collections::HashMap;

use shrinkwraprs::Shrinkwrap;

use crate::{
    chunk::{Chunk, CodeOffset, ConstantLongIndex},
    compiler::rules::{make_rules, Rules},
    scanner::{Scanner, Token, TokenKind},
};

#[derive(Shrinkwrap, Clone, Copy, PartialOrd, Ord, PartialEq, Eq)]
#[shrinkwrap(mutable)]
struct ScopeDepth(i32);

struct Local<'a> {
    name: Token<'a>,
    depth: ScopeDepth,
    mutable: bool,
}

#[derive(Copy, Clone)]
struct LoopState {
    depth: ScopeDepth,
    start: CodeOffset,
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
    scope_depth: ScopeDepth,
    loop_state: Option<LoopState>,
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
            scope_depth: ScopeDepth(0),
            loop_state: None,
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

    pub(super) fn current_chunk(&mut self) -> &mut Chunk {
        &mut self.chunk
    }

    pub(super) fn current_chunk_len(&mut self) -> usize {
        self.current_chunk().code().len()
    }
}
