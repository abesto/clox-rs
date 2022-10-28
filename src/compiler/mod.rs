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
    config,
    scanner::{Scanner, Token, TokenKind},
    types::Line,
    value::Function,
};

#[derive(Shrinkwrap, Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Default)]
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

#[derive(Clone, Debug)]
struct Upvalue {
    index: u8,
    is_local: bool,
}

struct NestableState<'scanner> {
    current_function: Function,
    function_type: FunctionType,

    locals: Vec<Local<'scanner>>,
    globals_by_name: HashMap<StringId, ConstantLongIndex>,
    upvalues: Vec<Upvalue>,

    scope_depth: ScopeDepth,
    loop_state: Option<LoopState>,
}

impl<'scanner> NestableState<'scanner> {
    fn new(function_name: StringId, function_type: FunctionType) -> Self {
        NestableState {
            current_function: Function::new(0, function_name),
            function_type,
            locals: vec![Local {
                name: Token {
                    kind: TokenKind::Identifier,
                    lexeme: &[],
                    line: Line(0),
                },
                depth: ScopeDepth(0),
                mutable: false,
            }],
            upvalues: Default::default(),
            globals_by_name: Default::default(),
            scope_depth: Default::default(),
            loop_state: Default::default(),
        }
    }
}

pub struct Compiler<'scanner, 'arena> {
    arena: &'arena mut Arena,
    strings_by_name: HashMap<String, StringId>,

    rules: Rules<'scanner, 'arena>,

    scanner: Scanner<'scanner>,
    previous: Option<Token<'scanner>>,
    current: Option<Token<'scanner>>,

    had_error: bool,
    panic_mode: bool,

    nestable_state: Vec<NestableState<'scanner>>,
}

impl<'scanner, 'arena> Compiler<'scanner, 'arena> {
    #[must_use]
    pub fn new(scanner: Scanner<'scanner>, arena: &'arena mut Arena) -> Self {
        let function_name = arena.add_string(String::from("<script>"));
        Compiler {
            arena,
            strings_by_name: HashMap::new(),
            scanner,
            previous: None,
            current: None,
            had_error: false,
            panic_mode: false,
            rules: make_rules(),
            nestable_state: vec![NestableState::new(function_name, FunctionType::Script)],
        }
    }

    fn start_nesting<S>(&mut self, function_name: S, function_type: FunctionType)
    where
        S: ToString,
    {
        let function_name = self.string_id(function_name);
        self.nestable_state
            .push(NestableState::new(function_name, function_type));
    }

    fn end_nesting(&mut self) -> NestableState {
        self.nestable_state.pop().unwrap()
    }

    fn nested<F, S>(&mut self, function_name: S, function_type: FunctionType, f: F) -> NestableState
    where
        S: ToString,
        F: Fn(&mut Self),
    {
        self.start_nesting(function_name, function_type);
        f(self);
        self.end_nesting()
    }

    fn has_enclosing(&self) -> bool {
        self.nestable_state.len() > 1
    }

    fn in_enclosing<F, R>(&mut self, f: F) -> R
    where
        F: Fn(&mut Self) -> R,
    {
        assert!(self.has_enclosing());
        let state = self.nestable_state.pop().unwrap();
        let result = f(self);
        self.nestable_state.push(state);
        result
    }

    pub fn compile(mut self) -> Option<Function> {
        self.advance();

        while !self.match_(TokenKind::Eof) {
            self.declaration();
        }

        self.end();
        if self.had_error {
            None
        } else {
            Some(self.nestable_state.pop().unwrap().current_function)
        }
    }

    fn end(&mut self) {
        self.emit_return();

        if config::PRINT_CODE.load() && !self.had_error {
            println!("{:?}", self.current_chunk());
        }
    }

    fn current_function(&self) -> &Function {
        &self.nestable_state.last().unwrap().current_function
    }

    fn current_function_mut(&mut self) -> &mut Function {
        &mut self.nestable_state.last_mut().unwrap().current_function
    }

    fn loop_state(&mut self) -> &Option<LoopState> {
        &self.nestable_state.last().unwrap().loop_state
    }

    fn loop_state_mut(&mut self) -> &mut Option<LoopState> {
        &mut self.nestable_state.last_mut().unwrap().loop_state
    }

    fn locals(&self) -> &Vec<Local> {
        &self.nestable_state.last().unwrap().locals
    }

    fn locals_mut(&mut self) -> &mut Vec<Local<'scanner>> {
        &mut self.nestable_state.last_mut().unwrap().locals
    }

    fn function_type(&self) -> FunctionType {
        self.nestable_state.last().unwrap().function_type
    }

    fn scope_depth(&self) -> ScopeDepth {
        self.nestable_state.last().unwrap().scope_depth
    }

    fn scope_depth_mut(&mut self) -> &mut ScopeDepth {
        &mut self.nestable_state.last_mut().unwrap().scope_depth
    }

    fn globals_by_name(&self) -> &HashMap<StringId, ConstantLongIndex> {
        &self.nestable_state.last().unwrap().globals_by_name
    }

    fn globals_by_name_mut(&mut self) -> &mut HashMap<StringId, ConstantLongIndex> {
        &mut self.nestable_state.last_mut().unwrap().globals_by_name
    }

    fn upvalues(&self) -> &Vec<Upvalue> {
        &self.nestable_state.last().unwrap().upvalues
    }

    fn upvalues_mut(&mut self) -> &mut Vec<Upvalue> {
        &mut self.nestable_state.last_mut().unwrap().upvalues
    }

    pub(super) fn current_chunk(&mut self) -> &mut Chunk {
        &mut self.current_function_mut().chunk
    }

    pub(super) fn current_chunk_len(&mut self) -> usize {
        self.current_chunk().code().len()
    }

    pub fn inject_strings(&mut self, names: &HashMap<String, StringId>) {
        for (key, value) in names {
            self.strings_by_name.insert(key.clone(), *value);
        }
    }
}
