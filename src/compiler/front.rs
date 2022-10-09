use super::{rules::Precedence, Compiler};
use crate::{
    chunk::{CodeOffset, OpCode},
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

    fn for_statement(&mut self) {
        self.begin_scope();
        self.consume(TK::LeftParen, "Expect '(' after 'for'.");

        if self.match_(TK::Semicolon) {
            // No initializer
        } else if self.match_(TK::Var) {
            self.var_declaration(true);
        } else if self.match_(TK::Const) {
            // This doesn't seem useful but I won't stop you
            self.var_declaration(false);
        } else {
            self.expression_statement();
        }

        let mut loop_start = CodeOffset(self.current_chunk().code().len());
        let mut exit_jump = None;
        if !self.match_(TK::Semicolon) {
            self.expression();
            self.consume(TK::Semicolon, "Expect ';' after loop condition.");
            exit_jump = Some(self.emit_jump(OpCode::JumpIfFalse));
            self.emit_byte(OpCode::Pop);
        }

        if !self.match_(TK::RightParen) {
            let body_jump = self.emit_jump(OpCode::Jump);
            let increment_start = CodeOffset(self.current_chunk().code().len());
            self.expression();
            self.emit_byte(OpCode::Pop);
            self.consume(TK::RightParen, "Expect ')' after for clauses.");

            self.emit_loop(loop_start);
            loop_start = increment_start;
            self.patch_jump(body_jump);
        }

        self.statement();
        self.emit_loop(loop_start);

        if let Some(exit_jump) = exit_jump {
            self.patch_jump(exit_jump);
            self.emit_byte(OpCode::Pop);
        }

        self.end_scope();
    }

    fn if_statement(&mut self) {
        self.consume(TK::LeftParen, "Expect '(' after 'if'.");
        self.expression();
        self.consume(TK::RightParen, "Expect ')' after condition.");

        let then_jump = self.emit_jump(OpCode::JumpIfFalse);
        self.emit_byte(OpCode::Pop);
        self.statement();

        let else_jump = self.emit_jump(OpCode::Jump);
        self.patch_jump(then_jump);
        self.emit_byte(OpCode::Pop);
        if self.match_(TK::Else) {
            self.statement();
        }

        self.patch_jump(else_jump);
    }

    fn while_statement(&mut self) {
        let loop_start = CodeOffset(self.current_chunk().code().len());
        self.consume(TK::LeftParen, "Expect '(' after 'while'.");
        self.expression();
        self.consume(TK::RightParen, "Expect ')' after condition.");

        let exit_jump = self.emit_jump(OpCode::JumpIfFalse);
        self.emit_byte(OpCode::Pop);
        self.statement();
        self.emit_loop(loop_start);

        self.patch_jump(exit_jump);
        self.emit_byte(OpCode::Pop);
    }

    fn switch_statement(&mut self) {
        self.consume(TK::LeftParen, "Expect '(' after 'switch'.");
        self.expression();
        self.consume(TK::RightParen, "Expect ')' after 'switch' value.");
        self.consume(TK::LeftBrace, "Expect '{' before 'switch' body.");

        let mut end_jumps = vec![];
        let mut had_default = false;

        while !self.check(TK::RightBrace) {
            if had_default {
                self.error_at_current("No 'case' or 'default' allowed after 'default' branch.");
            }

            self.emit_byte(OpCode::Dup); // Get a copy of the switch value for comparison

            let miss_jump = if self.match_(TK::Case) {
                self.expression();
                self.consume(TK::Colon, "Expect ':' after 'case' value.");
                self.emit_byte(OpCode::Equal);
                let jump = self.emit_jump(OpCode::JumpIfFalse);
                self.emit_byte(OpCode::Pop); // Get rid of the 'true' of the comparison
                Some(jump)
            } else {
                self.consume(TK::Default, "Expect 'case' or 'default'.");
                self.consume(TK::Colon, "Expect ':' after 'default'.");
                had_default = true;
                None
            };

            while !self.check(TK::RightBrace) && !self.check(TK::Case) && !self.check(TK::Default) {
                self.statement();
            }

            end_jumps.push(self.emit_jump(OpCode::Jump));

            if let Some(miss_jump) = miss_jump {
                self.patch_jump(miss_jump);
                self.emit_byte(OpCode::Pop); // Get rid of the 'false' of the comparison
            }
        }

        for end_jump in end_jumps {
            self.patch_jump(end_jump);
        }
        self.emit_byte(OpCode::Pop); // Get rid of the switch value

        self.consume(TK::RightBrace, "Expect '}' after 'switch' body.");
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
        } else if self.match_(TK::For) {
            self.for_statement();
        } else if self.match_(TK::If) {
            self.if_statement();
        } else if self.match_(TK::While) {
            self.while_statement();
        } else if self.match_(TK::Switch) {
            self.switch_statement();
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
