mod chunk;
mod debug;

fn main() {
    let mut chunk = chunk::Chunk::default();
    chunk.write(chunk::OpCode::OpReturn);
    chunk.disassemble("test chunk");
}
