use num_enum::{IntoPrimitive, TryFromPrimitive};
use shrinkwraprs::Shrinkwrap;

use crate::{types::Line, value::Value};

#[derive(Shrinkwrap)]
#[shrinkwrap(mutable)]
pub struct CodeOffset(pub usize);

#[derive(Shrinkwrap)]
pub struct ConstantIndex(pub u8);

#[derive(Shrinkwrap)]
pub struct ConstantLongIndex(pub usize);

#[derive(IntoPrimitive, TryFromPrimitive)]
#[repr(u8)]
pub enum OpCode {
    Constant,
    ConstantLong,

    Negate,

    Add,
    Subtract,
    Multiply,
    Divide,

    Return,
}

impl OpCode {
    /// Length of the instruction in bytes, including the operator and operands
    pub fn instruction_len(&self) -> usize {
        use OpCode::*;
        match self {
            Constant => 2,
            ConstantLong => 4,
            Negate | Add | Subtract | Multiply | Divide | Return => 1,
        }
    }
}

pub struct Chunk {
    name: String,
    code: Vec<u8>,
    lines: Vec<(usize, Line)>,
    constants: Vec<Value>,
}

impl Chunk {
    pub fn new<S>(name: S) -> Self
    where
        S: ToString,
    {
        Chunk {
            name: name.to_string(),
            code: Default::default(),
            lines: Default::default(),
            constants: Default::default(),
        }
    }

    pub fn code(&self) -> &[u8] {
        &self.code
    }

    pub fn get_constant<T>(&self, index: T) -> Value
    where
        T: Into<usize>,
    {
        self.constants[index.into()]
    }

    pub fn write<T>(&mut self, what: T, line: Line)
    where
        T: Into<u8>,
    {
        self.code.push(what.into());
        match self.lines.last_mut() {
            Some((count, last_line)) if last_line.as_ref() == line.as_ref() => {
                *count += 1;
            }
            _ => self.lines.push((1, line)),
        }
    }

    pub fn write_constant(&mut self, what: Value, line: Line) -> bool {
        self.constants.push(what);
        let long_index = self.constants.len() - 1;
        if let Ok(short_index) = u8::try_from(long_index) {
            self.write(OpCode::Constant, line);
            self.write(short_index, line);
        } else {
            self.write(OpCode::ConstantLong, line);
            let (a, b, c, d) = crate::bitwise::get_4_bytes(long_index);
            if a > 0 {
                return false;
            }
            self.write(b, line);
            self.write(c, line);
            self.write(d, line);
        }
        return true;
    }
}

impl std::fmt::Debug for Chunk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "== {} ==", self.name)?;
        let mut disassembler = InstructionDisassembler::new(self);
        while disassembler.offset.as_ref() < &self.code.len() {
            write!(f, "{:?}", disassembler)?;
            *disassembler.offset +=
                OpCode::try_from_primitive(self.code[*disassembler.offset.as_ref()])
                    .unwrap()
                    .instruction_len();
        }
        Ok(())
    }
}

// Debug helpers
pub struct InstructionDisassembler<'a> {
    chunk: &'a Chunk,
    pub offset: CodeOffset,
}

impl<'a> InstructionDisassembler<'a> {
    #[must_use]
    pub fn new(chunk: &'a Chunk) -> Self {
        Self {
            chunk,
            offset: CodeOffset(0),
        }
    }

    fn debug_constant_opcode(
        &self,
        f: &mut std::fmt::Formatter,
        name: &str,
        offset: &CodeOffset,
    ) -> std::fmt::Result {
        let constant_index = ConstantIndex(self.chunk.code()[offset.as_ref() + 1]);
        writeln!(
            f,
            "{:-16} {:>4} '{}'",
            name,
            *constant_index,
            self.chunk.get_constant(*constant_index.as_ref())
        )
    }

    fn debug_constant_long_opcode(
        &self,
        f: &mut std::fmt::Formatter,
        name: &str,
        offset: &CodeOffset,
    ) -> std::fmt::Result {
        let code = self.chunk.code();
        let constant_index = ConstantLongIndex(
            (usize::from(code[offset.as_ref() + 1]) << 16)
                + (usize::from(code[offset.as_ref() + 2]) << 8)
                + (usize::from(code[offset.as_ref() + 3])),
        );
        writeln!(
            f,
            "{:-16} {:>4} '{}'",
            name,
            *constant_index,
            self.chunk.get_constant(*constant_index.as_ref())
        )
    }

    fn debug_simple_opcode(&self, f: &mut std::fmt::Formatter, name: &str) -> std::fmt::Result {
        writeln!(f, "{}", name)
    }
}

impl<'a> std::fmt::Debug for InstructionDisassembler<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let code = self.chunk.code();
        let offset = &self.offset;

        write!(f, "{:04} ", *offset.as_ref())?;
        if *offset.as_ref() > 0
            && self.chunk.get_line(offset) == self.chunk.get_line(&CodeOffset(offset.as_ref() - 1))
        {
            write!(f, "   | ")?;
        } else {
            write!(f, "{:>4} ", *self.chunk.get_line(offset))?;
        }

        match OpCode::try_from_primitive(code[*offset.as_ref()])
            .unwrap_or_else(|_| panic!("Unknown opcode: {}", code[*offset.as_ref()]))
        {
            OpCode::Constant => self.debug_constant_opcode(f, "OP_CONSTANT", offset),
            OpCode::ConstantLong => self.debug_constant_long_opcode(f, "OP_CONSTANT_LONG", offset),
            OpCode::Return => self.debug_simple_opcode(f, "OP_RETURN"),
            OpCode::Negate => self.debug_simple_opcode(f, "OP_NEGATE"),
            OpCode::Add => self.debug_simple_opcode(f, "OP_ADD"),
            OpCode::Subtract => self.debug_simple_opcode(f, "OP_SUBTRACT"),
            OpCode::Multiply => self.debug_simple_opcode(f, "OP_MULTIPLY"),
            OpCode::Divide => self.debug_simple_opcode(f, "OP_DIVIDE"),
        }?;
        Ok(())
    }
}

impl Chunk {
    fn get_line(&self, offset: &CodeOffset) -> Line {
        let mut iter = self.lines.iter();
        let (mut consumed, mut line) = iter.next().unwrap();
        while consumed < *offset.as_ref() {
            let entry = iter.next().unwrap();
            consumed += entry.0;
            line = entry.1;
        }
        line
    }
}
