use crate::value::Value;

pub type ConstantIndex = usize;
pub type Line = usize;

#[repr(C, u8)]
pub enum Instruction {
    Constant(ConstantIndex),
    Return,
}

pub struct Chunk {
    name: String,
    code: Vec<Instruction>,
    lines: Vec<Line>,
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

    pub fn write(&mut self, what: Instruction, line: Line) {
        self.code.push(what);
        self.lines.push(line);
    }

    pub fn add_constant(&mut self, what: Value) -> usize {
        self.constants.push(what);
        self.constants.len() - 1
    }
}

impl std::fmt::Debug for Chunk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "== {} ==", self.name)?;
        for (offset, instruction) in self.code.iter().enumerate() {
            write!(f, "{:04} ", offset)?;
            if offset > 0 && self.lines[offset] == self.lines[offset - 1] {
                write!(f, "   | ")?;
            } else {
                write!(f, "{:>4} ", self.lines[offset])?;
            }

            match instruction {
                Instruction::Constant(constant_index) => {
                    self.debug_constant_instruction(f, "OP_CONSTANT", *constant_index)?
                }
                Instruction::Return => write!(f, "OP_RETURN")?,
            }
        }
        Ok(())
    }
}

// Debug helpers
impl Chunk {
    fn debug_constant_instruction(
        &self,
        f: &mut std::fmt::Formatter,
        name: &str,
        constant_index: usize,
    ) -> std::fmt::Result {
        writeln!(
            f,
            "{:-16} {:>4} '{}'",
            name, constant_index, self.constants[constant_index]
        )
    }
}
