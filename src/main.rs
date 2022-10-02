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
    for i in 0..260 {
        chunk.write_constant(i.into(), Line(200));
    }
    chunk.write(OpCode::Return, Line(123));

    let mut vm = VM::new();
    vm.interpret(&chunk).unwrap();
}
