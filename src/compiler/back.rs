use crate::{
    chunk::{CodeOffset, OpCode},
    scanner::{Token, TokenKind},
    value::Value,
};

use super::{Compiler, FunctionType};

impl<'scanner, 'heap> Compiler<'scanner, 'heap> {
    pub(super) fn emit_byte<T>(&mut self, byte: T)
    where
        T: Into<u8>,
    {
        let line = self.line();
        self.current_chunk().write(byte, line)
    }

    pub(super) fn emit_24bit_number(&mut self, number: usize) -> bool {
        let line = self.line();
        self.current_chunk().write_24bit_number(number, line)
    }

    pub(super) fn emit_bytes<T1, T2>(&mut self, byte1: T1, byte2: T2)
    where
        T1: Into<u8>,
        T2: Into<u8>,
    {
        self.emit_byte(byte1);
        self.emit_byte(byte2);
    }

    pub(super) fn emit_return(&mut self) {
        if self.function_type() == FunctionType::Initializer {
            self.emit_bytes(OpCode::GetLocal, 0);
        } else {
            self.emit_byte(OpCode::Nil);
        }
        self.emit_byte(OpCode::Return);
    }

    pub(super) fn emit_constant<T>(&mut self, value: T)
    where
        T: Into<Value>,
    {
        let line = self.line();
        let value_id = self.heap.values.add(value.into());
        if !self.current_chunk().write_constant(value_id, line) {
            self.error("Too many constants in one chunk.");
        }
    }

    /// Returns the offset of the last byte of the emitted jump instruction
    pub(super) fn emit_jump(&mut self, instruction: OpCode) -> CodeOffset {
        self.emit_byte(instruction);
        let retval = CodeOffset(self.current_chunk().code().len() - 1);
        self.emit_byte(0xff);
        self.emit_byte(0xff);
        retval
    }

    /// `jump_offset`: the code offset of the last byte of the jump instruction
    pub(super) fn patch_jump(&mut self, jump_offset: CodeOffset) {
        let jump_length = self.current_chunk().code().len() - *jump_offset - 3; // 3: length of the jump instruction + its arg

        if jump_length > usize::from(u16::MAX) {
            self.error("Too much code to jump over.");
        }

        self.current_chunk()
            .patch(CodeOffset(*jump_offset + 1), (jump_length >> 8) as u8);
        self.current_chunk()
            .patch(CodeOffset(*jump_offset + 2), jump_length as u8);
    }

    pub(super) fn emit_loop(&mut self, loop_start: CodeOffset) {
        let offset = self.current_chunk().code().len() - *loop_start + 3; // 3: length of the loop instruction + its arg

        self.emit_byte(OpCode::Loop);
        if offset > usize::from(u16::MAX) {
            self.error("Loop body too large.");
        }

        self.emit_byte((offset >> 8) as u8);
        self.emit_byte(offset as u8);
    }

    pub(super) fn emit_number(&mut self, n: usize, long: bool) -> bool {
        if long {
            self.emit_24bit_number(n)
        } else if let Ok(n) = u8::try_from(n) {
            self.emit_byte(n);
            true
        } else {
            false
        }
    }

    pub(super) fn synthetic_token(&self, kind: TokenKind) -> Token<'scanner> {
        Token {
            kind,
            lexeme: match kind {
                TokenKind::Super => "super",
                TokenKind::This => "this",
                _ => unimplemented!(),
            }
            .as_bytes(),
            line: self.line(),
        }
    }
}
