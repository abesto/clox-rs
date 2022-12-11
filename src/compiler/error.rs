use super::Compiler;
use crate::{
    scanner::{Token, TokenKind as TK},
    vm::Output,
};

impl<'scanner, 'heap, STDOUT: Output, STDERR: Output> Compiler<'scanner, 'heap, STDOUT, STDERR> {
    pub(super) fn error_at_current(&mut self, msg: &str) {
        // Could probably manually inline `error_at` with a macro to avoid this clone, but... really?
        self.error_at(self.current.clone(), msg);
    }

    pub(super) fn error(&mut self, msg: &str) {
        self.error_at(self.previous.clone(), msg);
    }

    fn error_at(&mut self, token: Option<Token>, msg: &str) {
        if self.panic_mode {
            return;
        }
        self.panic_mode = true;
        if let Some(token) = token.as_ref() {
            write!(self.stderr, "[line {}] Error", *token.line).unwrap();
            if token.kind == TK::Eof {
                write!(self.stderr, " at end").unwrap();
            } else if token.kind != TK::Error {
                write!(self.stderr, " at '{}'", token.as_str()).unwrap();
            }
            writeln!(self.stderr, ": {}", msg).unwrap();
        }
        self.had_error = true;
    }

    pub(super) fn synchronize(&mut self) {
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
}
