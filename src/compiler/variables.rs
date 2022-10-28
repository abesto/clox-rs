use hashbrown::hash_map::Entry;

use crate::{
    arena::StringId,
    chunk::{ConstantLongIndex, OpCode},
    config,
};

use super::{Compiler, Local, ScopeDepth};
use crate::scanner::{Token, TokenKind as TK};

impl<'scanner, 'arena> Compiler<'scanner, 'arena> {
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
        let mut get_op = OpCode::GetLocal;
        let mut set_op = OpCode::SetLocal;
        let mut arg = self.resolve_local(name.to_string());

        // Local or global?
        if arg.is_none() {
            arg = Some(*self.identifier_constant(name));
            get_op = OpCode::GetGlobal;
            set_op = OpCode::SetGlobal;
        }
        let arg = arg.unwrap();

        // Support for more than u8::MAX variables in a scope
        let long = if !crate::config::STD_MODE.load() && arg > u8::MAX.into() {
            get_op = get_op.to_long();
            set_op = set_op.to_long();
            true
        } else {
            false
        };

        // Get or set?
        let op = if can_assign && self.match_(TK::Equal) {
            self.expression();
            if set_op == OpCode::SetLocal || set_op == OpCode::SetLocalLong {
                self.check_local_const(arg);
            }
            set_op
        } else {
            get_op
        };

        // Generate the code.
        self.emit_byte(op);
        if !self.emit_number(arg, long) {
            self.error(&format!("Too many globals in {:?}", op));
        }
    }

    pub(super) fn string_id<S>(&mut self, s: S) -> StringId
    where
        S: ToString,
    {
        match self.strings_by_name.entry(s.to_string()) {
            Entry::Vacant(entry) => *entry.insert(self.arena.add_string(s.to_string())),
            Entry::Occupied(entry) => *entry.get(),
        }
    }

    fn identifier_constant<S>(&mut self, name: S) -> ConstantLongIndex
    where
        S: ToString,
    {
        let string_id = self.string_id(name);

        if let Some(index) = self.globals_by_name.get(&string_id) {
            *index
        } else {
            let value_id = self.arena.add_value(string_id.into());
            let index = self.current_chunk().make_constant(value_id);
            self.globals_by_name.insert(string_id, index);
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

    fn add_local(&mut self, name: Token<'scanner>, mutable: bool) {
        let limit_exp = if config::STD_MODE.load() { 8 } else { 24 };
        if self.locals.len() > usize::pow(2, limit_exp) - 1 {
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

    pub(super) fn mark_initialized(&mut self) {
        if *self.scope_depth == 0 {
            return;
        }
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

    pub(super) fn argument_list(&mut self) -> u8 {
        let mut arg_count = 0;
        if !self.check(TK::RightParen) {
            loop {
                self.expression();
                if arg_count == 255 {
                    self.error("Can't have more than 255 arguments.");
                    break;
                } else {
                    arg_count += 1;
                }
                if !self.match_(TK::Comma) {
                    break;
                }
            }
        }
        self.consume(TK::RightParen, "Expect ')' after arguments.");
        arg_count
    }

    fn check_local_const(&mut self, local_index: usize) {
        let local = &self.locals[local_index];
        if *local.depth != -1 && !local.mutable {
            self.error("Reassignment to local 'const'.");
        }
    }
}
