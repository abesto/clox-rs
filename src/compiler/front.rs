use super::{rules::Precedence, ClassState, Compiler, FunctionType, LoopState};
use crate::{
    chunk::{CodeOffset, ConstantIndex, OpCode},
    scanner::TokenKind as TK,
    types::Line,
};

impl<'scanner, 'heap> Compiler<'scanner, 'heap> {
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

    fn function(&mut self, function_type: FunctionType) {
        let function_name = self.previous.as_ref().unwrap().as_str().to_string();

        let nested_state = self.nested(function_name, function_type, |compiler| {
            compiler.begin_scope();

            compiler.consume(TK::LeftParen, "Expect '(' after function name.");

            if !compiler.check(TK::RightParen) {
                loop {
                    compiler.current_function_mut().arity += 1;
                    if compiler.current_function().arity > 255 {
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
        });
        let nested_function = nested_state.current_function;
        let nested_upvalues = nested_state.upvalues;

        self.emit_byte(OpCode::Closure);
        let function_id = self.heap.functions.add(nested_function);
        let value_id = self.heap.values.add(function_id.into());
        let value_id_byte = u8::try_from(self.current_chunk().make_constant(value_id).0).unwrap();
        self.emit_byte(value_id_byte);

        for upvalue in nested_upvalues {
            self.emit_bytes(upvalue.is_local, upvalue.index);
        }
    }

    fn method(&mut self) {
        self.consume(TK::Identifier, "Expect method name.");
        let name_constant =
            self.identifier_constant(self.previous.as_ref().unwrap().as_str().to_string());
        let function_type = if self.previous.as_ref().unwrap().lexeme == "init".as_bytes() {
            FunctionType::Initializer
        } else {
            FunctionType::Method
        };
        self.function(function_type);
        self.emit_bytes(
            OpCode::Method,
            ConstantIndex::try_from(name_constant)
                .expect("Too many constants when declaring method."),
        );
    }

    fn class_declaration(&mut self) {
        self.consume(TK::Identifier, "Expect class name.");
        let class_name = self.previous.as_ref().unwrap().as_str().to_string();
        let name_constant = self.identifier_constant(class_name.to_string());
        self.declare_variable(true);

        self.emit_bytes(
            OpCode::Class,
            ConstantIndex::try_from(name_constant)
                .expect("Too many constants when declaring class."),
        );
        self.define_variable(Some(name_constant), true);
        self.class_state.push(ClassState::new());

        if self.match_(TK::Less) {
            self.consume(TK::Identifier, "Expect superclass name.");
            self.variable(false);

            if class_name == self.previous.as_ref().unwrap().as_str() {
                self.error("A class can't inherit from itself.");
            }

            self.begin_scope();
            self.add_local(self.synthetic_token(TK::Super), false);
            self.define_variable(None, false);

            self.named_variable(&class_name, false);
            self.emit_byte(OpCode::Inherit);
            self.current_class_mut().unwrap().has_superclass = true;
        }

        self.named_variable(class_name, false);
        self.consume(TK::LeftBrace, "Expect '{' before class body.");
        while !self.check(TK::RightBrace) && !self.check(TK::Eof) {
            self.method();
        }
        self.consume(TK::RightBrace, "Expect '}' after class body.");
        self.emit_byte(OpCode::Pop);

        if self.current_class().unwrap().has_superclass {
            self.end_scope();
        }

        self.class_state.pop();
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

        // Compile initializer, store loop variable
        let loop_var_and_name = if self.match_(TK::Semicolon) {
            // No initializer
            None
        } else if self.match_(TK::Var) || self.match_(TK::Const) {
            let name = self.current.clone().unwrap();
            self.var_declaration(self.check_previous(TK::Var));
            // Challenge 25/2: alias loop variables
            if crate::config::STD_MODE.load() {
                None
            } else if let Ok(loop_var) = u8::try_from(self.locals().len() - 1) {
                Some((loop_var, name))
            } else {
                self.error("Creating loop variable led to too many locals.");
                None
            }
        } else {
            self.expression_statement();
            None
        };

        // Store old loop state to restore at then end
        let old_loop_state = {
            let start = CodeOffset(self.current_chunk_len());
            let depth = self.scope_depth();
            std::mem::replace(self.loop_state_mut(), Some(LoopState { depth, start }))
        };

        // Compile loop condition
        let mut exit_jump = None;
        if !self.match_(TK::Semicolon) {
            self.expression();
            self.consume(TK::Semicolon, "Expect ';' after loop condition.");
            exit_jump = Some(self.emit_jump(OpCode::JumpIfFalse));
            self.emit_byte(OpCode::Pop);
        }

        // Compile "i = i + 1" clause (what's this actually called?)
        if !self.match_(TK::RightParen) {
            let body_jump = self.emit_jump(OpCode::Jump);
            let increment_start = CodeOffset(self.current_chunk_len());
            self.expression();
            self.emit_byte(OpCode::Pop);
            self.consume(TK::RightParen, "Expect ')' after for clauses.");

            let loop_start = self.loop_state().as_ref().unwrap().start;
            self.emit_loop(loop_start);
            self.loop_state_mut().as_mut().unwrap().start = increment_start;
            self.patch_jump(body_jump);
        }

        // Alias loop variable for this iteration of the loop
        let loop_and_inner_var = if let Some((loop_var, loop_var_name)) = loop_var_and_name {
            self.begin_scope();
            self.emit_bytes(OpCode::GetLocal, loop_var);
            self.add_local(loop_var_name, true);
            self.mark_initialized();
            if let Ok(inner_var) = u8::try_from(self.locals().len() - 1) {
                Some((loop_var, inner_var))
            } else {
                self.error("Aliasing loop variable led to too many locals.");
                None
            }
        } else {
            None
        };

        self.statement();

        // Clean up alias for loop variable
        if let Some((loop_var, inner_var)) = loop_and_inner_var {
            self.emit_bytes(OpCode::GetLocal, inner_var);
            self.emit_bytes(OpCode::SetLocal, loop_var);
            self.emit_byte(OpCode::Pop);
            self.end_scope();
        }

        let loop_start = self.loop_state().as_ref().unwrap().start;
        self.emit_loop(loop_start);

        if let Some(exit_jump) = exit_jump {
            self.patch_jump(exit_jump);
            self.emit_byte(OpCode::Pop);
        }

        *self.loop_state_mut() = old_loop_state;
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
            let depth = self.scope_depth();
            std::mem::replace(self.loop_state_mut(), Some(LoopState { depth, start }))
        };
        self.consume(TK::LeftParen, "Expect '(' after 'while'.");
        self.expression();
        self.consume(TK::RightParen, "Expect ')' after condition.");

        let exit_jump = self.emit_jump(OpCode::JumpIfFalse);
        self.emit_byte(OpCode::Pop);
        self.statement();
        let loop_start = self.loop_state().as_ref().unwrap().start;
        self.emit_loop(loop_start);

        self.patch_jump(exit_jump);
        self.emit_byte(OpCode::Pop);
        *self.loop_state_mut() = old_loop_state;
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
        match *self.loop_state() {
            None => self.error("'continue' outside a loop."),
            Some(state) => {
                self.consume(TK::Semicolon, "Expect ';' after 'continue'.");

                let locals_to_drop = self
                    .locals()
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
        if self.match_(TK::Class) {
            self.class_declaration();
        } else if self.match_(TK::Fun) {
            self.fun_declaration();
        } else if self.match_(TK::Var) {
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
        if self.function_type() == FunctionType::Script {
            self.error("Can't return from top-level code.");
        }
        if self.match_(TK::Semicolon) {
            self.emit_return();
        } else {
            if self.function_type() == FunctionType::Initializer {
                self.error("Can't return a value from an initializer.");
            }
            self.expression();
            self.consume(TK::Semicolon, "Expect ';' after return value.");
            self.emit_byte(OpCode::Return);
        }
    }
}
