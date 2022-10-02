use thiserror::Error;

#[cfg(feature = "trace_execution")]
use crate::chunk::InstructionDisassembler;
use crate::{
    chunk::{Chunk, OpCode},
    value::Value,
};

#[derive(Error, Debug)]
pub enum Error {}

type Result<T = (), E = Error> = std::result::Result<T, E>;

pub struct VM<'a> {
    chunk: Option<&'a Chunk>,
    ip: std::iter::Enumerate<std::slice::Iter<'a, u8>>,
    stack: Vec<Value>,
}

impl<'a> VM<'a> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            chunk: None,
            ip: [].iter().enumerate(),
            stack: Vec::with_capacity(256),
        }
    }

    pub fn interpret(&mut self, chunk: &'a Chunk) -> Result {
        self.chunk = Some(chunk);
        self.ip = chunk.code().iter().enumerate();
        self.run()
    }

    fn run(&mut self) -> Result {
        #[cfg(feature = "trace_execution")]
        let mut disassembler = InstructionDisassembler::new(self.chunk.unwrap());
        loop {
            #[allow(unused_variables)]
            let (offset, instruction) = self
                .ip
                .next()
                .expect("Internal error: ran out of instructions");
            #[cfg(feature = "trace_execution")]
            {
                *disassembler.offset = offset;
                println!("          {:?}", self.stack);
                print!("{:?}", disassembler);
            }
            match OpCode::try_from(*instruction).expect("Internal error: unrecognized opcode") {
                OpCode::Return => {
                    println!("{}", self.stack.pop().expect("stack underflow"));
                    return Ok(());
                }
                OpCode::Constant => {
                    let value = self.read_constant(false);
                    self.stack.push(value);
                }
                OpCode::ConstantLong => {
                    let value = self.read_constant(true);
                    self.stack.push(value);
                }
                OpCode::Negate => {
                    let value = self.stack.last_mut().expect("stack underflow in OP_NEGATE");
                    *value = -*value;
                }
                OpCode::Add => self.binary_op(|a, b| a + b),
                OpCode::Subtract => self.binary_op(|a, b| a - b),
                OpCode::Multiply => self.binary_op(|a, b| a * b),
                OpCode::Divide => self.binary_op(|a, b| a / b),
            };
        }
    }

    fn read_byte(&mut self, msg: &str) -> u8 {
        *self.ip.next().expect(msg).1
    }

    fn read_constant(&mut self, long: bool) -> Value {
        let index = if long {
            (usize::from(self.read_byte("read_constant/long/0")) << 16)
                + (usize::from(self.read_byte("read_constant/long/1")) << 8)
                + (usize::from(self.read_byte("read_constant/long/2")))
        } else {
            usize::from(self.read_byte("read_constant"))
        };
        self.chunk.unwrap().get_constant(index)
    }

    fn binary_op(&mut self, op: fn(Value, Value) -> Value) {
        let b = self.stack.pop().expect("stack underflow in binary_op");
        let a = self.stack.last_mut().expect("stack underflow in binary_op");
        *a = op(*a, b);
    }
}
