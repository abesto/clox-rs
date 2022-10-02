#[repr(u8)]
#[derive(PartialEq, Eq)]
pub enum OpCode {
    Return,
}

pub struct Chunk {
    code: Vec<OpCode>,
}
