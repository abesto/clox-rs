use std::rc::Rc;

use super::{rules::Precedence, Compiler, FunctionType, LoopState};
use crate::{
    chunk::{CodeOffset, OpCode},
    scanner::TokenKind as TK,
    types::Line,
    value::Value,
};

impl<'compiler, 'arena> Compiler<'compiler, 'arena> {
    pub(super) fn advance(&mut self) {
        {
            let mut shared = self.shared.borrow_mut();
            shared.previous = std::mem::take(&mut shared.current);
        }
        loop {
            {
                let mut shared = self.shared.borrow_mut();
                shared.current = Some(shared.scanner.scan());
            }
            if !self.check(TK::Error) {
                break;
            }
            // Could manually recursively inline `error_at_current` to get rid of this string copy,
            // but... this seems good enough, really.
            let msg = self
                .shared
                .borrow()
                .current
                .as_ref()
                .unwrap()
                .as_str()
                .to_string();
            #[allow(clippy::unnecessary_to_owned)]
            self.error_at_current(&msg);
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
        self.shared.borrow().previous.as_ref().unwrap().line
    }

    pub(super) fn match_(&mut self, kind: TK) -> bool {
        if !self.check(kind) {
            return false;
        }
        self.advance();
        true
    }

    pub(super) fn current_token_kind(&self) -> Option<TK> {
        self.shared.borrow().current.as_ref().map(|t| t.kind)
    }

    pub(super) fn check(&self, kind: TK) -> bool {
        self.current_token_kind()
            .map(|k| k == kind)
            .unwrap_or_else(|| false)
    }

    pub(super) fn check_previous(&self, kind: TK) -> bool {
        self.shared
            .borrow()
            .previous
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

    fn function(&mut self, function_type: FunctionType) {
        let line = self.line();

        let function = {
            let function_name = self
                .shared
                .borrow()
                .previous
                .as_ref()
                .unwrap()
                .as_str()
                .to_string();
            let mut compiler =
                Compiler::new_(Rc::clone(&self.shared), function_name, function_type);

            compiler.begin_scope();
            compiler.consume(TK::LeftParen, "Expect '(' after function name.");

            if !compiler.check(TK::RightParen) {
                loop {
                    compiler.current_function.arity += 1;
                    if compiler.current_function.arity > 255 {
                        compiler.error_at_current("Can't have more than 255 parameters.");
                    }
                    let constant = compiler.parse_variable("Expect parameter name.", false);
                    compiler.define_variable(constant, false);
                    if !compiler.match_(TK::Comma) {
                        break;
                    }
                }
            }

            compiler.consume(TK::RightParen, "Expect ')' after parameters.");
            compiler.consume(TK::LeftBrace, "Expect '{' before function body.");
            compiler.block();

            compiler.end();
            compiler.current_function
        };

        let value_id = self
            .shared
            .borrow_mut()
            .arena
            .add_value(Value::from(function));
        self.current_chunk().write_constant(value_id, line);
    }

    fn fun_declaration(&mut self) {
        let global = self.parse_variable("Expect function name.", false);
        self.mark_initialized();
        self.function(FunctionType::Function);
        self.define_variable(global, false);
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

        let old_loop_state = {
            let start = CodeOffset(self.current_chunk_len());
            std::mem::replace(
                &mut self.loop_state,
                Some(LoopState {
                    depth: self.scope_depth,
                    start,
                }),
            )
        };

        let mut exit_jump = None;
        if !self.match_(TK::Semicolon) {
            self.expression();
            self.consume(TK::Semicolon, "Expect ';' after loop condition.");
            exit_jump = Some(self.emit_jump(OpCode::JumpIfFalse));
            self.emit_byte(OpCode::Pop);
        }

        if !self.match_(TK::RightParen) {
            let body_jump = self.emit_jump(OpCode::Jump);
            let increment_start = CodeOffset(self.current_chunk_len());
            self.expression();
            self.emit_byte(OpCode::Pop);
            self.consume(TK::RightParen, "Expect ')' after for clauses.");

            self.emit_loop(self.loop_state.as_ref().unwrap().start);
            self.loop_state.as_mut().unwrap().start = increment_start;
            self.patch_jump(body_jump);
        }

        self.statement();
        self.emit_loop(self.loop_state.as_ref().unwrap().start);

        if let Some(exit_jump) = exit_jump {
            self.patch_jump(exit_jump);
            self.emit_byte(OpCode::Pop);
        }

        self.loop_state = old_loop_state;
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
        let old_loop_state = {
            let start = CodeOffset(self.current_chunk_len());
            std::mem::replace(
                &mut self.loop_state,
                Some(LoopState {
                    depth: self.scope_depth,
                    start,
                }),
            )
        };
        self.consume(TK::LeftParen, "Expect '(' after 'while'.");
        self.expression();
        self.consume(TK::RightParen, "Expect ')' after condition.");

        let exit_jump = self.emit_jump(OpCode::JumpIfFalse);
        self.emit_byte(OpCode::Pop);
        self.statement();
        self.emit_loop(self.loop_state.as_ref().unwrap().start);

        self.patch_jump(exit_jump);
        self.emit_byte(OpCode::Pop);
        self.loop_state = old_loop_state;
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

            let miss_jump = if self.match_(TK::Case) {
                self.emit_byte(OpCode::Dup); // Get a copy of the switch value for comparison
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

    fn continue_statement(&mut self) {
        match self.loop_state {
            None => self.error("'continue' outside a loop."),
            Some(state) => {
                self.consume(TK::Semicolon, "Expect ';' after 'continue'.");

                let locals_to_drop = self
                    .locals
                    .iter()
                    .rev()
                    .take_while(|local| local.depth > state.depth)
                    .count();
                for _ in 0..locals_to_drop {
                    self.emit_byte(OpCode::Pop);
                }

                self.emit_loop(state.start);
            }
        }
    }

    pub(super) fn declaration(&mut self) {
        if self.match_(TK::Fun) {
            self.fun_declaration();
        } else if self.match_(TK::Var) {
            self.var_declaration(true);
        } else if self.match_(TK::Const) {
            self.var_declaration(false);
        } else {
            self.statement();
        }

        if self.shared.borrow().panic_mode {
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
        } else if self.match_(TK::Return) {
            self.return_statement();
        } else if self.match_(TK::While) {
            self.while_statement();
        } else if self.match_(TK::Switch) {
            self.switch_statement();
        } else if self.match_(TK::Continue) {
            self.continue_statement();
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

    fn return_statement(&mut self) {
        if self.function_type == FunctionType::Script {
            self.error("Can't return from top-level code.");
        }
        if self.match_(TK::Semicolon) {
            self.emit_return();
        } else {
            self.expression();
            self.consume(TK::Semicolon, "Expect ';' after return value.");
            self.emit_byte(OpCode::Return);
        }
    }
}
