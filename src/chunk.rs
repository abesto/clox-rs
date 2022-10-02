use std::ops::Index;

#[repr(u8)]
#[derive(PartialEq, Eq, Clone)]
pub enum OpCode {
    OpReturn = 0,
}

#[derive(Default)]
pub struct Chunk {
    code: Vec<OpCode>,
}

impl Chunk {
    pub fn write(&mut self, what: OpCode) {
        self.code.push(what);
    }

    pub fn len(&self) -> usize {
        self.code.len()
    }
}

impl Index<usize> for Chunk {
    type Output = OpCode;

    fn index(&self, index: usize) -> &Self::Output {
        &self.code[index]
    }
}
