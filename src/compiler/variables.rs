use hashbrown::hash_map::Entry;

use crate::{
    arena::StringId,
    chunk::{ConstantLongIndex, OpCode},
    config,
};

use super::{Compiler, Local, ScopeDepth, Upvalue};
use crate::scanner::{Token, TokenKind as TK};

impl<'scanner, 'arena> Compiler<'scanner, 'arena> {
    pub(super) fn begin_scope(&mut self) {
        **self.scope_depth_mut() += 1;
    }

    pub(super) fn end_scope(&mut self) {
        **self.scope_depth_mut() -= 1;
        let scope_depth = self.scope_depth();

        let mut instructions = vec![];

        {
            let locals = self.locals_mut();
            while locals
                .last()
                .map(|local| local.depth > scope_depth)
                .unwrap_or(false)
            {
                instructions.push(if locals.last().unwrap().is_captured {
                    OpCode::CloseUpvalue
                } else {
                    OpCode::Pop
                });
                locals.pop();
            }
        }

        for instruction in instructions {
            self.emit_byte(instruction);
        }
    }

    pub(super) fn named_variable<S>(&mut self, name: S, can_assign: bool)
    where
        S: ToString,
    {
        let mut get_op = OpCode::GetLocal;
        let mut set_op = OpCode::SetLocal;
        let mut arg = self.resolve_local(name.to_string());

        // Upvalue?
        if arg.is_none() {
            if let Some(upvalue_arg) = self.resolve_upvalue(name.to_string()) {
                get_op = OpCode::GetUpvalue;
                set_op = OpCode::SetUpvalue;
                arg = Some(usize::from(upvalue_arg));
            }
        }

        // If neither local nor upvalue, then it must be a global
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

        if let Some(index) = self.globals_by_name().get(&string_id) {
            *index
        } else {
            let value_id = self.arena.add_value(string_id.into());
            let index = self.current_chunk().make_constant(value_id);
            self.globals_by_name_mut().insert(string_id, index);
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
            .locals()
            .iter()
            .enumerate()
            .rev()
            .find(|(_, local)| local.name.lexeme == name)
            .map(|(index, local)| {
                if *local.depth == -1 {
                    self.locals().len()
                } else {
                    index
                }
            });
        if retval == Some(self.locals().len()) {
            self.error("Can't read local variable in its own initializer.");
        }
        retval
    }

    fn resolve_upvalue<S>(&mut self, name: S) -> Option<u8>
    where
        S: ToString,
    {
        if !self.has_enclosing() {
            return None;
        }

        if let Some(local) = self.in_enclosing(|compiler| compiler.resolve_local(name.to_string()))
        {
            self.in_enclosing(|compiler| compiler.locals_mut()[local].is_captured = true);
            return Some(self.add_upvalue(local, true));
        }

        if let Some(upvalue) =
            self.in_enclosing(|compiler| compiler.resolve_upvalue(name.to_string()))
        {
            return Some(self.add_upvalue(usize::from(upvalue), false));
        }

        None
    }

    fn add_upvalue(&mut self, local_index: usize, is_local: bool) -> u8 {
        if let Ok(local_index) = u8::try_from(local_index) {
            // Return index if we already have it
            if let Some((upvalue_index, _)) =
                self.upvalues().iter().enumerate().find(|(_, upvalue)| {
                    upvalue.index == local_index && upvalue.is_local == is_local
                })
            {
                return u8::try_from(upvalue_index).unwrap();
            }

            if self.upvalues().len() >= usize::from(u8::MAX) + 1 {
                self.error("Too many closure variables in function.");
                return 0;
            }

            // Record new upvalue
            self.upvalues_mut().push(Upvalue {
                index: local_index,
                is_local,
            });
            let upvalue_count = self.upvalues().len();
            self.current_function_mut().upvalue_count = upvalue_count;
            u8::try_from(upvalue_count - 1).unwrap()
        } else {
            // This is where `(Get|Set)UpvalueLong` would go
            self.error("Too variables in function surrounding closure.");
            0
        }
    }

    pub(super) fn add_local(&mut self, name: Token<'scanner>, mutable: bool) {
        let limit_exp = if config::STD_MODE.load() { 8 } else { 24 };
        if self.locals().len() > usize::pow(2, limit_exp) - 1 {
            self.error("Too many local variables in function.");
            return;
        }
        self.locals_mut().push(Local {
            name,
            depth: ScopeDepth(-1),
            mutable,
            is_captured: false,
        });
    }

    pub(super) fn declare_variable(&mut self, mutable: bool) {
        if *self.scope_depth() == 0 {
            return;
        }

        let name = self.previous.clone().unwrap();
        let scope_depth = self.scope_depth();
        if self.locals_mut().iter().rev().any(|local| {
            if *local.depth != -1 && local.depth < scope_depth {
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
        if *self.scope_depth() > 0 {
            None
        } else {
            Some(self.identifier_constant(self.previous.as_ref().unwrap().as_str().to_string()))
        }
    }

    pub(super) fn mark_initialized(&mut self) {
        let scope_depth = self.scope_depth();
        if *scope_depth == 0 {
            return;
        }
        if let Some(local) = self.locals_mut().last_mut() {
            local.depth = scope_depth;
        }
    }

    pub(super) fn define_variable(&mut self, global: Option<ConstantLongIndex>, mutable: bool) {
        if global.is_none() {
            assert!(*self.scope_depth() > 0);
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
        let local = &self.locals()[local_index];
        if *local.depth != -1 && !local.mutable {
            self.error("Reassignment to local 'const'.");
        }
    }
}
