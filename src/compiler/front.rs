use super::{rules::Precedence, Compiler};
use crate::{
    chunk::{Chunk, OpCode},
    scanner::TokenKind as TK,
    types::Line,
};

impl<'a> Compiler<'a> {
    pub(super) fn advance(&mut self) {
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

    pub(super) fn consume(&mut self, kind: TK, msg: &str) {
        if self.check(kind) {
            self.advance();
            return;
        }

        self.error_at_current(msg);
    }

    pub(super) fn line(&self) -> Line {
        self.previous.as_ref().unwrap().line
    }

    pub(super) fn current_chunk(&mut self) -> &mut Chunk {
        &mut self.chunk
    }

    pub(super) fn match_(&mut self, kind: TK) -> bool {
        if !self.check(kind) {
            return false;
        }
        self.advance();
        true
    }

    pub(super) fn current_token_kind(&self) -> Option<TK> {
        self.current.as_ref().map(|t| t.kind)
    }

    pub(super) fn check(&self, kind: TK) -> bool {
        self.current_token_kind()
            .map(|k| k == kind)
            .unwrap_or_else(|| false)
    }

    pub(super) fn check_previous(&self, kind: TK) -> bool {
        self.previous
            .as_ref()
            .map(|t| t.kind == kind)
            .unwrap_or(false)
    }

    pub(super) fn expression(&mut self) {
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

    pub(super) fn declaration(&mut self) {
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
}
