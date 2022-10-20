use num_enum::{IntoPrimitive, TryFromPrimitive};

use crate::chunk::OpCode;
use crate::scanner::TokenKind as TK;

use super::Compiler;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub(super) enum Precedence {
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

#[derive(Clone)]
pub(super) struct Rule<'a> {
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

pub(super) type Rules<'a> = [Rule<'a>; 48];

// Can't be a static value because the associated function types include lifetimes
#[rustfmt::skip]
pub(super) fn make_rules<'a>() -> Rules<'a> {
    make_rules!(
        LeftParen    = [grouping, None,   None],
        RightParen   = [None,     None,   None],
        LeftParen    = [grouping, None,   None],
        RightParen   = [None,     None,   None],
        LeftBrace    = [None,     None,   None],
        RightBrace   = [None,     None,   None],
        Colon        = [None,     None,   None],
        Comma        = [None,     None,   None],
        Default      = [None,     None,   None],
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
        And          = [None,     and,    And],
        Case         = [None,     None,   None],
        Class        = [None,     None,   None],
        Const        = [None,     None,   None],
        Continue     = [None,     None,   None],
        Else         = [None,     None,   None],
        False        = [literal,  None,   None],
        For          = [None,     None,   None],
        Fun          = [None,     None,   None],
        If           = [None,     None,   None],
        Nil          = [literal,  None,   None],
        Or           = [None,     or,     Or],
        Print        = [None,     None,   None],
        Return       = [None,     None,   None],
        Switch       = [None,     None,   None],
        Super        = [None,     None,   None],
        This         = [None,     None,   None],
        True         = [literal,  None,   None],
        Var          = [None,     None,   None],
        While        = [None,     None,   None],
        Error        = [None,     None,   None],
        Eof          = [None,     None,   None],
    )
}

impl<'a> Compiler<'a> {
    fn get_rule(&self, operator: TK) -> &Rule<'a> {
        &self.rules[operator as usize]
    }

    pub(super) fn parse_precedence(&mut self, precedence: Precedence) {
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

    fn and(&mut self, _can_assign: bool) {
        let end_jump = self.emit_jump(OpCode::JumpIfFalse);
        self.emit_byte(OpCode::Pop);
        self.parse_precedence(Precedence::And);
        self.patch_jump(end_jump);
    }

    fn or(&mut self, _can_assign: bool) {
        let else_jump = self.emit_jump(OpCode::JumpIfFalse);
        let end_jump = self.emit_jump(OpCode::Jump);

        self.patch_jump(else_jump);
        self.emit_byte(OpCode::Pop);

        self.parse_precedence(Precedence::Or);
        self.patch_jump(end_jump);
    }
}
