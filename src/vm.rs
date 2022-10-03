#[cfg(feature = "trace_execution")]
use crate::chunk::InstructionDisassembler;
use crate::{
    chunk::{Chunk, OpCode},
    compiler::Compiler,
    value::Value,
};

#[derive(Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum InterpretResult {
    Ok,
    CompileError,
    RuntimeError,
}

pub struct VM {
    chunk: Option<Chunk>,
    ip: usize,
    stack: Vec<Value>,
}

impl VM {
    #[must_use]
    pub fn new() -> Self {
        Self {
            chunk: None,
            ip: 0,
            stack: Vec::with_capacity(256),
        }
    }

    pub fn interpret(&mut self, source: &[u8]) -> InterpretResult {
        if let Some(chunk) = Compiler::compile(source) {
            self.chunk = Some(chunk);
            self.ip = 0;
            self.run()
        } else {
            InterpretResult::CompileError
        }
    }

    fn run(&mut self) -> InterpretResult {
        #[cfg(feature = "trace_execution")]
        let mut disassembler = InstructionDisassembler::new(self.chunk.unwrap());
        loop {
            #[allow(unused_variables)]
            let instruction = self.chunk.as_ref().unwrap().code()[self.ip];
            #[cfg(feature = "trace_execution")]
            {
                *disassembler.offset = offset;
                println!("          {:?}", self.stack);
                print!("{:?}", disassembler);
            }
            match OpCode::try_from(instruction).expect("Internal error: unrecognized opcode") {
                OpCode::Return => {
                    println!("{}", self.stack.pop().expect("stack underflow"));
                    return InterpretResult::Ok;
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
        self.ip += 1;
        *self.get_byte(self.ip).expect(msg)
    }

    fn get_byte(&self, index: usize) -> Option<&u8> {
        self.chunk.as_ref().unwrap().code().get(index)
    }

    fn read_constant(&mut self, long: bool) -> Value {
        let index = if long {
            (usize::from(self.read_byte("read_constant/long/0")) << 16)
                + (usize::from(self.read_byte("read_constant/long/1")) << 8)
                + (usize::from(self.read_byte("read_constant/long/2")))
        } else {
            usize::from(self.read_byte("read_constant"))
        };
        self.chunk.as_ref().unwrap().get_constant(index)
    }

    fn binary_op(&mut self, op: fn(Value, Value) -> Value) {
        let b = self.stack.pop().expect("stack underflow in binary_op");
        let a = self.stack.last_mut().expect("stack underflow in binary_op");
        *a = op(*a, b);
    }
}
