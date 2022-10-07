use std::collections::HashMap;

use num_enum::{IntoPrimitive, TryFromPrimitive};

use crate::{
    chunk::{Chunk, ConstantLongIndex, OpCode},
    scanner::{Scanner, Token, TokenKind as TK},
    types::Line,
    value::Value,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
enum Precedence {
    None,
    Assignment, // =
    Or,         // or
    And,        // and
    Equality,   // == !=
    Comparison, // < > <= >=
    Term,       // + -
    Factor,     // * /
    Unary,      // ! -
    Call,       // . ()
    Primary,
}

type ParseFn<'a> = fn(&mut Compiler<'a>, bool) -> ();

struct Rule<'a> {
    prefix: Option<ParseFn<'a>>,
    infix: Option<ParseFn<'a>>,
    precedence: Precedence,
}

impl<'a> Default for Rule<'a> {
    fn default() -> Self {
        Self {
            prefix: Default::default(),
            infix: Default::default(),
            precedence: Precedence::None,
        }
    }
}

macro_rules! make_rules {
    (@parse_fn None) => { None };
    (@parse_fn $prefix:ident) => { Some(Compiler::$prefix) };

    ($($token:ident = [$prefix:ident, $infix:ident, $precedence:ident]),* $(,)?) => {{
        // Horrible hack to pre-fill the array with *something* before assigning the right values based on the macro input
        // Needed because `Rule` cannot be `Copy` (due to `fn`s)
        let mut rules = [$(Rule { prefix: make_rules!(@parse_fn $prefix), infix: make_rules!(@parse_fn $infix), precedence: Precedence::$precedence }),*];
        $(
            rules[TK::$token as usize] = Rule {
                prefix: make_rules!(@parse_fn $prefix),
                infix: make_rules!(@parse_fn $infix),
                precedence: Precedence::$precedence
            };
        )*
        rules
    }};
}

type Rules<'a> = [Rule<'a>; 43];

// Can't be a static value because the associated function types include lifetimes
#[rustfmt::skip]
fn make_rules<'a>() -> Rules<'a> {
    make_rules!(
        LeftParen    = [grouping, None,   None],
        RightParen   = [None,     None,   None],
        LeftParen    = [grouping, None,   None],
        RightParen   = [None,     None,   None],
        LeftBrace    = [None,     None,   None],
        RightBrace   = [None,     None,   None],
        Comma        = [None,     None,   None],
        Const        = [None,     None,   None],
        Dot          = [None,     None,   None],
        Minus        = [unary,    binary, Term],
        Plus         = [None,     binary, Term],
        Semicolon    = [None,     None,   None],
        Slash        = [None,     binary, Factor],
        Star         = [None,     binary, Factor],
        Bang         = [unary,    None,   None],
        BangEqual    = [None,     binary, Equality],
        Equal        = [None,     None,   None],
        EqualEqual   = [None,     binary, Equality],
        Greater      = [None,     binary, Comparison],
        GreaterEqual = [None,     binary, Comparison],
        Less         = [None,     binary, Comparison],
        LessEqual    = [None,     binary, Comparison],
        Identifier   = [variable, None,   None],
        String       = [string,   None,   None],
        Number       = [number,   None,   None],
        And          = [None,     None,   None],
        Class        = [None,     None,   None],
        Else         = [None,     None,   None],
        False        = [literal,  None,   None],
        For          = [None,     None,   None],
        Fun          = [None,     None,   None],
        If           = [None,     None,   None],
        Nil          = [literal,  None,   None],
        Or           = [None,     None,   None],
        Print        = [None,     None,   None],
        Return       = [None,     None,   None],
        Super        = [None,     None,   None],
        This         = [None,     None,   None],
        True         = [literal,  None,   None],
        Var          = [None,     None,   None],
        While        = [None,     None,   None],
        Error        = [None,     None,   None],
        Eof          = [None,     None,   None],
    )
}

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

        while !self.match_(TK::Eof) {
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

    fn advance(&mut self) {
        self.previous = std::mem::take(&mut self.current);
        loop {
            let token = self.scanner.scan();
            self.current = Some(token);
            if !self.check(TK::Error) {
                break;
            }
            // Could manually recursively inline `error_at_current` to get rid of this string copy,
            // but... this seems good enough, really.
            #[allow(clippy::unnecessary_to_owned)]
            self.error_at_current(&self.current.as_ref().unwrap().as_str().to_string());
        }
    }

    fn consume(&mut self, kind: TK, msg: &str) {
        if self.check(kind) {
            self.advance();
            return;
        }

        self.error_at_current(msg);
    }

    fn line(&self) -> Line {
        self.previous.as_ref().unwrap().line
    }

    fn emit_byte<T>(&mut self, byte: T)
    where
        T: Into<u8>,
    {
        let line = self.line();
        self.current_chunk().write(byte, line)
    }

    fn emit_24bit_number(&mut self, number: usize) -> bool {
        let line = self.line();
        self.current_chunk().write_24bit_number(number, line)
    }

    fn emit_bytes<T1, T2>(&mut self, byte1: T1, byte2: T2)
    where
        T1: Into<u8>,
        T2: Into<u8>,
    {
        self.emit_byte(byte1);
        self.emit_byte(byte2);
    }

    fn emit_return(&mut self) {
        self.emit_byte(OpCode::Return);
    }

    fn emit_constant<T>(&mut self, value: T)
    where
        T: Into<Value>,
    {
        if !self.chunk.write_constant(value.into(), self.line()) {
            self.error("Too many constants in one chunk.");
        }
    }

    fn end(&mut self) {
        self.emit_return();

        #[cfg(feature = "print_code")]
        if !self.had_error {
            println!("{:?}", self.chunk);
        }
    }

    fn begin_scope(&mut self) {
        self.scope_depth += 1;
    }

    fn end_scope(&mut self) {
        self.scope_depth -= 1;
        while self
            .locals
            .last()
            .map(|local| local.depth > self.scope_depth)
            .unwrap_or(false)
        {
            self.emit_byte(OpCode::Pop);
            self.locals.pop();
        }
    }

    fn binary(&mut self, _can_assign: bool) {
        let operator = self.previous.as_ref().unwrap().kind;
        let rule = self.get_rule(operator);

        self.parse_precedence(
            Precedence::try_from_primitive(u8::from(rule.precedence) - 1).unwrap(),
        );

        // Emit the operator
        match operator {
            TK::Plus => self.emit_byte(OpCode::Add),
            TK::Minus => self.emit_byte(OpCode::Subtract),
            TK::Star => self.emit_byte(OpCode::Multiply),
            TK::Slash => self.emit_byte(OpCode::Divide),
            TK::BangEqual => self.emit_bytes(OpCode::Equal, OpCode::Not),
            TK::EqualEqual => self.emit_byte(OpCode::Equal),
            TK::Greater => self.emit_byte(OpCode::Greater),
            TK::GreaterEqual => self.emit_bytes(OpCode::Less, OpCode::Not),
            TK::Less => self.emit_byte(OpCode::Less),
            TK::LessEqual => self.emit_bytes(OpCode::Greater, OpCode::Not),

            _ => unreachable!("unknown binary operator: {}", operator),
        }
    }

    fn literal(&mut self, _can_assign: bool) {
        match self.previous.as_ref().unwrap().kind {
            TK::False => self.emit_byte(OpCode::False),
            TK::True => self.emit_byte(OpCode::True),
            TK::Nil => self.emit_byte(OpCode::Nil),
            _ => unreachable!("literal"),
        }
    }

    fn grouping(&mut self, _can_assign: bool) {
        self.expression();
        self.consume(TK::RightParen, "Expect ')' after expression.");
    }

    fn number(&mut self, _can_assign: bool) {
        let value: f64 = self.previous.as_ref().unwrap().as_str().parse().unwrap();
        self.emit_constant(value);
    }

    fn string(&mut self, _can_assign: bool) {
        let lexeme = self.previous.as_ref().unwrap().as_str();
        let value = lexeme[1..lexeme.len() - 1].to_string();
        self.emit_constant(value);
    }

    fn variable(&mut self, can_assign: bool) {
        self.named_variable(
            self.previous.as_ref().unwrap().as_str().to_string(),
            can_assign,
        );
    }

    fn named_variable<S>(&mut self, name: S, can_assign: bool)
    where
        S: ToString,
    {
        let local_index = self.resolve_local(name.to_string());
        let global_index = if local_index.is_some() {
            None
        } else {
            Some(*self.identifier_constant(name))
        };

        let op = if can_assign && self.match_(TK::Equal) {
            self.expression();
            if let Some(global_index) = global_index {
                if global_index > u8::MAX.into() {
                    OpCode::SetGlobalLong
                } else {
                    OpCode::SetGlobal
                }
            } else {
                let local_index = local_index.unwrap();
                let local = &self.locals[local_index];
                if local.depth != -1 && !local.mutable {
                    self.error("Reassignment to local 'const'.");
                }
                if local_index > u8::MAX.into() {
                    OpCode::SetLocalLong
                } else {
                    OpCode::SetLocal
                }
            }
        } else if let Some(global_index) = global_index {
            if global_index > u8::MAX.into() {
                OpCode::GetGlobalLong
            } else {
                OpCode::GetGlobal
            }
        } else if local_index.unwrap() > u8::MAX.into() {
            OpCode::GetLocalLong
        } else {
            OpCode::GetLocal
        };

        self.emit_byte(op.clone());

        let arg = local_index.map(usize::from).or(global_index).unwrap();
        if let Ok(short_arg) = u8::try_from(arg) {
            self.emit_byte(short_arg);
        } else if !self.emit_24bit_number(arg) {
            self.error(&format!("Too many globals in {:?}", op));
        }
    }

    fn unary(&mut self, _can_assign: bool) {
        let operator = self.previous.as_ref().unwrap().kind;

        // Compile the operand
        self.parse_precedence(Precedence::Unary);

        // Emit the operator
        match operator {
            TK::Minus => self.emit_byte(OpCode::Negate),
            TK::Bang => self.emit_byte(OpCode::Not),
            _ => unreachable!("unary but not negation: {}", operator),
        }
    }

    fn parse_precedence(&mut self, precedence: Precedence) {
        self.advance();
        if let Some(prefix_rule) = self.get_rule(self.previous.as_ref().unwrap().kind).prefix {
            let can_assign = precedence <= Precedence::Assignment;
            prefix_rule(self, can_assign);

            while precedence
                < self
                    .get_rule(self.current.as_ref().unwrap().kind)
                    .precedence
            {
                self.advance();
                let infix_rule = self
                    .get_rule(self.previous.as_ref().unwrap().kind)
                    .infix
                    .unwrap();
                infix_rule(self, can_assign);
            }

            if can_assign && self.match_(TK::Equal) {
                self.error("Invalid assignment target.");
            }
        } else {
            self.error("Expect expression.");
        }
    }

    fn identifier_constant<S>(&mut self, name: S) -> ConstantLongIndex
    where
        S: ToString,
    {
        let name = name.to_string();
        if let Some(index) = self.globals_by_name.get(&name) {
            index.clone()
        } else {
            let index = self.current_chunk().make_constant(name.to_string().into());
            self.globals_by_name.insert(name, index.clone());
            index
        }
    }

    fn resolve_local<S>(&mut self, name: S) -> Option<usize>
    where
        S: ToString,
    {
        let name_string = name.to_string();
        let name = name_string.as_bytes();
        let retval = self
            .locals
            .iter()
            .enumerate()
            .rev()
            .find(|(_, local)| local.name.lexeme == name)
            .map(|(index, local)| {
                if local.depth == -1 {
                    self.locals.len()
                } else {
                    index
                }
            });
        if retval == Some(self.locals.len()) {
            self.error("Can't read local variable in its own initializer.");
        }
        retval
    }

    fn add_local(&mut self, name: Token<'a>, mutable: bool) {
        if self.locals.len() > usize::from(u8::MAX) + 1 {
            self.error("Too many local variables in function.");
            return;
        }
        self.locals.push(Local {
            name,
            depth: -1,
            mutable,
        });
    }

    fn declare_variable(&mut self, mutable: bool) {
        if self.scope_depth == 0 {
            return;
        }

        let name = self.previous.clone().unwrap();
        if self.locals.iter().rev().any(|local| {
            if local.depth != -1 && local.depth < self.scope_depth {
                false
            } else {
                local.name.lexeme == name.lexeme
            }
        }) {
            self.error("Already a variable with this name in this scope.");
        }

        self.add_local(name, mutable);
    }

    fn parse_variable(&mut self, msg: &str, mutable: bool) -> Option<ConstantLongIndex> {
        self.consume(TK::Identifier, msg);

        self.declare_variable(mutable);
        if self.scope_depth > 0 {
            None
        } else {
            Some(self.identifier_constant(self.previous.as_ref().unwrap().as_str().to_string()))
        }
    }

    fn mark_initialized(&mut self) {
        if let Some(local) = self.locals.last_mut() {
            local.depth = self.scope_depth;
        }
    }

    fn define_variable(&mut self, global: Option<ConstantLongIndex>, mutable: bool) {
        if global.is_none() {
            assert!(self.scope_depth > 0);
            self.mark_initialized();
            return;
        }
        let global = global.unwrap();

        if let Ok(short) = u8::try_from(*global) {
            if mutable {
                self.emit_byte(OpCode::DefineGlobal);
            } else {
                self.emit_byte(OpCode::DefineGlobalConst);
            }
            self.emit_byte(short);
        } else {
            if mutable {
                self.emit_byte(OpCode::DefineGlobalLong);
            } else {
                self.emit_byte(OpCode::DefineGlobalConstLong);
            }
            if !self.emit_24bit_number(*global) {
                self.error("Too many globals in define_global!");
            }
        }
    }

    fn current_chunk(&mut self) -> &mut Chunk {
        &mut self.chunk
    }

    fn error_at_current(&mut self, msg: &str) {
        // Could probably manually inline `error_at` with a macro to avoid this clone, but... really?
        self.error_at(self.current.clone(), msg);
    }

    fn error(&mut self, msg: &str) {
        self.error_at(self.previous.clone(), msg);
    }

    fn error_at(&mut self, token: Option<Token>, msg: &str) {
        if self.panic_mode {
            return;
        }
        self.panic_mode = true;
        if let Some(token) = token.as_ref() {
            eprint!("[line {}] Error", *token.line);
            if token.kind == TK::Eof {
                eprint!(" at end");
            } else if token.kind != TK::Error {
                eprint!(" at '{}'", token.as_str())
            }
            eprintln!(": {}", msg);
        }
        self.had_error = true;
    }

    fn expression(&mut self) {
        self.parse_precedence(Precedence::Assignment);
    }

    fn block(&mut self) {
        while !self.check(TK::RightBrace) && !self.check(TK::Eof) {
            self.declaration();
        }
        self.consume(TK::RightBrace, "Expect '}' after block.");
    }

    fn var_declaration(&mut self, mutable: bool) {
        let global = self.parse_variable("Expect variable name.", mutable);

        if self.match_(TK::Equal) {
            self.expression();
        } else {
            self.emit_byte(OpCode::Nil);
        }
        self.consume(TK::Semicolon, "Expect ';' after variable declaration.");

        self.define_variable(global, mutable);
    }

    fn expression_statement(&mut self) {
        self.expression();
        self.consume(TK::Semicolon, "Expect ';' after expression.");
        self.emit_byte(OpCode::Pop);
    }

    fn declaration(&mut self) {
        if self.match_(TK::Var) {
            self.var_declaration(true);
        } else if self.match_(TK::Const) {
            self.var_declaration(false);
        } else {
            self.statement();
        }

        if self.panic_mode {
            self.synchronize();
        }
    }

    fn statement(&mut self) {
        if self.match_(TK::Print) {
            self.print_statement();
        } else if self.match_(TK::LeftBrace) {
            self.begin_scope();
            self.block();
            self.end_scope();
        } else {
            self.expression_statement();
        }
    }

    fn print_statement(&mut self) {
        self.expression();
        self.consume(TK::Semicolon, "Expect ';' after value.");
        self.emit_byte(OpCode::Print);
    }

    fn synchronize(&mut self) {
        self.panic_mode = false;

        while !self.check(TK::Eof) {
            if self.check_previous(TK::Semicolon) {
                return;
            }
            if let Some(
                TK::Class
                | TK::Fun
                | TK::Const
                | TK::Var
                | TK::For
                | TK::If
                | TK::While
                | TK::Print
                | TK::Return,
            ) = self.current_token_kind()
            {
                return;
            }
            self.advance();
        }
    }

    fn match_(&mut self, kind: TK) -> bool {
        if !self.check(kind) {
            return false;
        }
        self.advance();
        true
    }

    fn current_token_kind(&self) -> Option<TK> {
        self.current.as_ref().map(|t| t.kind)
    }

    fn check(&self, kind: TK) -> bool {
        self.current_token_kind()
            .map(|k| k == kind)
            .unwrap_or_else(|| false)
    }

    fn check_previous(&self, kind: TK) -> bool {
        self.previous
            .as_ref()
            .map(|t| t.kind == kind)
            .unwrap_or(false)
    }

    fn get_rule(&self, operator: TK) -> &Rule<'a> {
        &self.rules[operator as usize]
    }
}
