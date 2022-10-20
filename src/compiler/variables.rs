use crate::chunk::{ConstantLongIndex, OpCode};

use super::{Compiler, Local, ScopeDepth};
use crate::scanner::{Token, TokenKind as TK};

impl<'a> Compiler<'a> {
    pub(super) fn begin_scope(&mut self) {
        *self.scope_depth += 1;
    }

    pub(super) fn end_scope(&mut self) {
        *self.scope_depth -= 1;
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

    pub(super) fn named_variable<S>(&mut self, name: S, can_assign: bool)
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
                if *local.depth != -1 && !local.mutable {
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

        self.emit_byte(op);

        let arg = local_index.map(usize::from).or(global_index).unwrap();
        if let Ok(short_arg) = u8::try_from(arg) {
            self.emit_byte(short_arg);
        } else if !self.emit_24bit_number(arg) {
            self.error(&format!("Too many globals in {:?}", op));
        }
    }

    fn identifier_constant<S>(&mut self, name: S) -> ConstantLongIndex
    where
        S: ToString,
    {
        let name = name.to_string();
        if let Some(index) = self.globals_by_name.get(&name) {
            *index
        } else {
            let index = self.current_chunk().make_constant(name.to_string().into());
            self.globals_by_name.insert(name, index);
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
                if *local.depth == -1 {
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
        if self.locals.len() > usize::pow(2, 23) {
            self.error("Too many local variables in function.");
            return;
        }
        self.locals.push(Local {
            name,
            depth: ScopeDepth(-1),
            mutable,
        });
    }

    pub(super) fn declare_variable(&mut self, mutable: bool) {
        if *self.scope_depth == 0 {
            return;
        }

        let name = self.previous.clone().unwrap();
        if self.locals.iter().rev().any(|local| {
            if *local.depth != -1 && local.depth < self.scope_depth {
                false
            } else {
                local.name.lexeme == name.lexeme
            }
        }) {
            self.error("Already a variable with this name in this scope.");
        }

        self.add_local(name, mutable);
    }

    pub(super) fn parse_variable(&mut self, msg: &str, mutable: bool) -> Option<ConstantLongIndex> {
        self.consume(TK::Identifier, msg);

        self.declare_variable(mutable);
        if *self.scope_depth > 0 {
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

    pub(super) fn define_variable(&mut self, global: Option<ConstantLongIndex>, mutable: bool) {
        if global.is_none() {
            assert!(*self.scope_depth > 0);
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
}
