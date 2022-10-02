use crate::chunk::{Line, OpCode};

mod bitwise;
mod chunk;
mod value;

fn main() {
    let mut chunk = chunk::Chunk::new("test chunk");

    for i in 0..260 {
        chunk.write_constant(i.into(), Line(200));
    }

    chunk.write(OpCode::Return, Line(123));

    print!("{:?}", chunk);
}
