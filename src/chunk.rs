use num_enum::{IntoPrimitive, TryFromPrimitive};
use shrinkwraprs::Shrinkwrap;

use crate::value::Value;

#[derive(Shrinkwrap, PartialEq, Eq, Clone, Copy)]
pub struct Line(pub usize);

#[derive(Shrinkwrap)]
#[shrinkwrap(mutable)]
pub struct CodeOffset(pub usize);

#[derive(Shrinkwrap)]
pub struct ConstantIndex(pub u8);

#[derive(IntoPrimitive, TryFromPrimitive)]
#[repr(u8)]
pub enum OpCode {
    Constant,
    Return,
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

    pub fn add_constant(&mut self, what: Value) -> ConstantIndex {
        self.constants.push(what);
        ConstantIndex(u8::try_from(self.constants.len()).unwrap() - 1)
    }
}

impl std::fmt::Debug for Chunk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "== {} ==", self.name)?;
        let mut offset = CodeOffset(0);
        while offset.as_ref() < &self.code.len() {
            write!(f, "{:04} ", *offset)?;
            if *offset > 0
                && self.get_line(&offset) == self.get_line(&CodeOffset(offset.as_ref() - 1))
            {
                write!(f, "   | ")?;
            } else {
                write!(f, "{:>4} ", *self.get_line(&offset))?;
            }

            *offset += match OpCode::try_from_primitive(self.code[*offset.as_ref()])
                .unwrap_or_else(|_| panic!("Unknown opcode: {}", self.code[*offset.as_ref()]))
            {
                OpCode::Constant => self.debug_constant_opcode(f, "OP_CONSTANT", &offset)?,
                OpCode::Return => self.debug_simple_opcode(f, "OP_RETURN")?,
            }
        }
        Ok(())
    }
}

// Debug helpers
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

    fn debug_constant_opcode(
        &self,
        f: &mut std::fmt::Formatter,
        name: &str,
        offset: &CodeOffset,
    ) -> Result<usize, std::fmt::Error> {
        let constant_index = ConstantIndex(self.code[offset.as_ref() + 1]);
        writeln!(
            f,
            "{:-16} {:>4} '{}'",
            name,
            *constant_index,
            self.constants[usize::from(*constant_index)]
        )?;
        Ok(2)
    }

    fn debug_simple_opcode(
        &self,
        f: &mut std::fmt::Formatter,
        name: &str,
    ) -> Result<usize, std::fmt::Error> {
        writeln!(f, "{}", name)?;
        Ok(1)
    }
}
