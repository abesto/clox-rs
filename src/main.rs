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
    chunk.write_constant(3.4, Line(1));

    chunk.write(OpCode::Add, Line(2));

    chunk.write_constant(5.6, Line(3));
    chunk.write(OpCode::Divide, Line(3));

    chunk.write(OpCode::Negate, Line(4));
    chunk.write(OpCode::Return, Line(4));
    let mut vm = VM::new();
    vm.interpret(&chunk).unwrap();
}
