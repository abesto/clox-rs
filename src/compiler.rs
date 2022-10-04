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

type ParseFn<'a> = fn(&mut Compiler<'a>) -> ();

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

type Rules<'a> = [Rule<'a>; 42];

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
        Identifier   = [None,     None,   None],
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

pub struct Compiler<'a> {
    scanner: Scanner<'a>,
    previous: Option<Token<'a>>,
    current: Option<Token<'a>>,
    had_error: bool,
    panic_mode: bool,
    chunk: Chunk,
    rules: Rules<'a>,
}

impl<'a> Compiler<'a> {
    #[must_use]
    fn new(source: &'a [u8]) -> Self {
        Self {
            chunk: Chunk::new("<main>"),
            scanner: Scanner::new(source),
            previous: None,
            current: None,
            had_error: false,
            panic_mode: false,
            rules: make_rules(),
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

    fn emit_byte<T>(&mut self, byte: T, line: Line)
    where
        T: Into<u8>,
    {
        self.current_chunk().write(byte, line)
    }

    fn emit_bytes<T1, T2>(&mut self, byte1: T1, byte2: T2, line: Line)
    where
        T1: Into<u8>,
        T2: Into<u8>,
    {
        self.emit_byte(byte1, line);
        self.emit_byte(byte2, line);
    }

    fn emit_return(&mut self) {
        self.emit_byte(OpCode::Return, self.line());
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

    fn binary(&mut self) {
        let operator = self.previous.as_ref().unwrap().kind;
        let line = self.line();
        let rule = self.get_rule(operator);

        self.parse_precedence(
            Precedence::try_from_primitive(u8::from(rule.precedence) + 1).unwrap(),
        );

        // Emit the operator
        match operator {
            TK::Plus => self.emit_byte(OpCode::Add, line),
            TK::Minus => self.emit_byte(OpCode::Subtract, line),
            TK::Star => self.emit_byte(OpCode::Multiply, line),
            TK::Slash => self.emit_byte(OpCode::Divide, line),
            TK::BangEqual => self.emit_bytes(OpCode::Equal, OpCode::Not, line),
            TK::EqualEqual => self.emit_byte(OpCode::Equal, line),
            TK::Greater => self.emit_byte(OpCode::Greater, line),
            TK::GreaterEqual => self.emit_bytes(OpCode::Less, OpCode::Not, line),
            TK::Less => self.emit_byte(OpCode::Less, line),
            TK::LessEqual => self.emit_bytes(OpCode::Greater, OpCode::Not, line),

            _ => unreachable!("unknown binary operator: {}", operator),
        }
    }

    fn literal(&mut self) {
        match self.previous.as_ref().unwrap().kind {
            TK::False => self.emit_byte(OpCode::False, self.line()),
            TK::True => self.emit_byte(OpCode::True, self.line()),
            TK::Nil => self.emit_byte(OpCode::Nil, self.line()),
            _ => unreachable!("literal"),
        }
    }

    fn grouping(&mut self) {
        self.expression();
        self.consume(TK::RightParen, "Expect ')' after expression.");
    }

    fn number(&mut self) {
        let value: f64 = self.previous.as_ref().unwrap().as_str().parse().unwrap();
        self.emit_constant(value);
    }

    fn string(&mut self) {
        let lexeme = self.previous.as_ref().unwrap().as_str();
        let value = lexeme[1..lexeme.len() - 1].to_string();
        self.emit_constant(value);
    }

    fn unary(&mut self) {
        let operator = self.previous.as_ref().unwrap().kind;
        let line = self.line();

        // Compile the operand
        self.parse_precedence(Precedence::Unary);

        // Emit the operator
        match operator {
            TK::Minus => self.emit_byte(OpCode::Negate, line),
            TK::Bang => self.emit_byte(OpCode::Not, line),
            _ => unreachable!("unary but not negation: {}", operator),
        }
    }

    fn parse_precedence(&mut self, precedence: Precedence) {
        self.advance();
        if let Some(prefix_rule) = self.get_rule(self.previous.as_ref().unwrap().kind).prefix {
            prefix_rule(self);

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
                infix_rule(self);
            }
        } else {
            self.error("Expected expression.");
        }
    }

    fn identifier_constant<S>(&mut self, name: S) -> ConstantLongIndex
    where
        S: ToString,
    {
        self.current_chunk().make_constant(name.to_string().into())
    }

    fn parse_variable(&mut self, msg: &str) -> ConstantLongIndex {
        self.consume(TK::Identifier, msg);
        self.identifier_constant(self.previous.as_ref().unwrap().as_str().to_string())
    }

    fn define_variable(&mut self, global: ConstantLongIndex) {
        if let Ok(short) = u8::try_from(*global) {
            self.emit_bytes(OpCode::DefineGlobal, short, self.line());
        } else {
            self.error("Too many globals!")
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

    fn var_declaration(&mut self) {
        let global = self.parse_variable("Expect variable name.");

        if self.match_(TK::Equal) {
            self.expression();
        } else {
            self.emit_byte(OpCode::Nil, self.line());
        }
        self.consume(TK::Semicolon, "Expect ';' after variable declaration.");

        self.define_variable(global);
    }

    fn expression_statement(&mut self) {
        let line = self.line();
        self.expression();
        self.consume(TK::Semicolon, "Expect ';' after expression.");
        self.emit_byte(OpCode::Pop, line);
    }

    fn declaration(&mut self) {
        if self.match_(TK::Var) {
            self.var_declaration();
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
        } else {
            self.expression_statement();
        }
    }

    fn print_statement(&mut self) {
        let line = self.line();
        self.expression();
        self.consume(TK::Semicolon, "Expect ';' after value.");
        self.emit_byte(OpCode::Print, line);
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
            .unwrap_or(false)
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
