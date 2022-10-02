use crate::{
    chunk::{Line, OpCode},
    vm::VM,
};

mod bitwise;
mod chunk;
mod value;
mod vm;

fn main() {
    let mut chunk = chunk::Chunk::new("test chunk");
    chunk.write_constant(1.2, Line(1));
    for _ in 0..10000000 {
        chunk.write(OpCode::Negate, Line(4));
    }
    chunk.write(OpCode::Return, Line(4));
    let mut vm = VM::new();
    vm.interpret(&chunk).unwrap();
}
