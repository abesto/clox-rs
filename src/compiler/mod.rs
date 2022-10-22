mod back;
mod error;
mod front;
mod rules;
mod variables;

use hashbrown::HashMap;
use shrinkwraprs::Shrinkwrap;

use crate::{
    arena::{Arena, StringId},
    chunk::{Chunk, CodeOffset, ConstantLongIndex},
    compiler::rules::{make_rules, Rules},
    scanner::{Scanner, Token, TokenKind},
    types::Line,
    value::Function,
};

#[derive(Shrinkwrap, Clone, Copy, PartialOrd, Ord, PartialEq, Eq)]
#[shrinkwrap(mutable)]
struct ScopeDepth(i32);

struct Local<'scanner> {
    name: Token<'scanner>,
    depth: ScopeDepth,
    mutable: bool,
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum FunctionType {
    Function,
    Script,
}

#[derive(Copy, Clone)]
struct LoopState {
    depth: ScopeDepth,
    start: CodeOffset,
}

pub struct Compiler<'scanner, 'arena> {
    arena: &'arena mut Arena,
    scanner: Scanner<'scanner>,
    previous: Option<Token<'scanner>>,
    current: Option<Token<'scanner>>,

    had_error: bool,
    panic_mode: bool,

    current_function: Function,
    function_type: FunctionType,

    strings_by_name: HashMap<String, StringId>,
    globals_by_name: HashMap<StringId, ConstantLongIndex>,
    rules: Rules<'scanner, 'arena>,
    locals: Vec<Local<'scanner>>,
    scope_depth: ScopeDepth,
    loop_state: Option<LoopState>,
}

impl<'scanner, 'arena> Compiler<'scanner, 'arena> {
    #[must_use]
    fn new<S>(
        scanner: Scanner<'scanner>,
        arena: &'arena mut Arena,
        function_name: S,
        function_type: FunctionType,
    ) -> Self
    where
        S: ToString,
    {
        let function_name = arena.add_string(function_name.to_string());
        let mut compiler = Compiler {
            arena,
            current_function: Function::new(0, function_name),
            function_type,
            globals_by_name: HashMap::new(),
            strings_by_name: HashMap::new(),
            scanner,
            previous: None,
            current: None,
            had_error: false,
            panic_mode: false,
            rules: make_rules(),
            locals: Vec::new(),
            scope_depth: ScopeDepth(0),
            loop_state: None,
        };

        compiler.locals.push(Local {
            name: Token {
                kind: TokenKind::Identifier,
                lexeme: &[],
                line: Line(0),
            },
            depth: ScopeDepth(0),
            mutable: false,
        });

        compiler
    }

    fn compile_(mut self) -> Option<Function> {
        self.advance();

        while !self.match_(TokenKind::Eof) {
            self.declaration();
        }

        self.end();
        if self.had_error {
            None
        } else {
            Some(self.current_function)
        }
    }

    pub fn compile(scanner: Scanner<'scanner>, arena: &'arena mut Arena) -> Option<Function> {
        let compiler = Compiler::new(scanner, arena, "<script>", FunctionType::Script);
        compiler.compile_()
    }

    fn end(&mut self) {
        self.emit_return();

        #[cfg(feature = "print_code")]
        if !self.had_error {
            println!("{:?}", self.current_chunk());
        }
    }

    pub(super) fn current_chunk(&mut self) -> &mut Chunk {
        &mut self.current_function.chunk
    }

    pub(super) fn current_chunk_len(&mut self) -> usize {
        self.current_chunk().code().len()
    }
}
