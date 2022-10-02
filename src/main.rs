use crate::chunk::{Line, OpCode};

mod chunk;
mod value;

fn main() {
    let mut chunk = chunk::Chunk::new("test chunk");

    let constant_index = chunk.add_constant(1.2);
    chunk.write(OpCode::Constant, Line(123));
    chunk.write(*constant_index, Line(123));
    chunk.write(OpCode::Return, Line(123));

    print!("{:?}", chunk);
}
