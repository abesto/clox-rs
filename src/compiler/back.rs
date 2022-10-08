use crate::{chunk::OpCode, value::Value};

use super::Compiler;

impl<'a> Compiler<'a> {
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
        self.emit_byte(OpCode::Return);
    }

    pub(super) fn emit_constant<T>(&mut self, value: T)
    where
        T: Into<Value>,
    {
        if !self.chunk.write_constant(value.into(), self.line()) {
            self.error("Too many constants in one chunk.");
        }
    }
}
