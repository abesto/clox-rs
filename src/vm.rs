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
}

impl<'a> VM<'a> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            chunk: None,
            ip: [].iter().enumerate(),
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
                print!("{:?}", disassembler);
            }
            match OpCode::try_from(*instruction) {
                Err(_) => panic!("Internal error: unrecognized opcode {}", instruction),
                Ok(OpCode::Return) => return Ok(()),
                Ok(OpCode::Constant) => {
                    println!("{}", self.read_constant(false));
                }
                Ok(OpCode::ConstantLong) => {
                    println!("{}", self.read_constant(true));
                }
                #[allow(unreachable_patterns)]
                _ => {}
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
}
