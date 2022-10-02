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
    chunk.write_constant(20.0, Line(1));
    chunk.write_constant(30.0, Line(1));
    chunk.write_constant(42.0, Line(2));
    chunk.write(OpCode::Return, Line(2));
    let mut vm = VM::new();
    vm.interpret(&chunk).unwrap();
}
