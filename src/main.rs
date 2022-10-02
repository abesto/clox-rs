mod chunk;
mod value;

fn main() {
    let mut chunk = chunk::Chunk::new("test chunk");

    let constant_index = chunk.add_constant(1.2);
    chunk.write(chunk::Instruction::Constant(constant_index), 123);
    chunk.write(chunk::Instruction::Return, 123);

    println!("{:?}", chunk);
}
