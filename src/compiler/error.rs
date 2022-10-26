use super::Compiler;
use crate::scanner::{Token, TokenKind as TK};

impl<'compiler, 'arena> Compiler<'compiler, 'arena> {
    pub(super) fn error_at_current(&mut self, msg: &str) {
        // Could probably manually inline `error_at` with a macro to avoid this clone, but... really?
        let token = self.shared.borrow().current.clone();
        self.error_at(token, msg);
    }

    pub(super) fn error(&mut self, msg: &str) {
        let token = self.shared.borrow().previous.clone();
        self.error_at(token, msg);
    }

    fn error_at(&mut self, token: Option<Token>, msg: &str) {
        if self.shared.borrow().panic_mode {
            return;
        }
        self.shared.borrow_mut().panic_mode = true;
        if let Some(token) = token.as_ref() {
            eprint!("[line {}] Error", *token.line);
            if token.kind == TK::Eof {
                eprint!(" at end");
            } else if token.kind != TK::Error {
                eprint!(" at '{}'", token.as_str())
            }
            eprintln!(": {}", msg);
        }
        self.shared.borrow_mut().had_error = true;
    }

    pub(super) fn synchronize(&mut self) {
        self.shared.borrow_mut().panic_mode = false;

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
