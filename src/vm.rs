use std::collections::HashMap;

#[cfg(feature = "trace_execution")]
use crate::chunk::InstructionDisassembler;
use crate::{
    chunk::{Chunk, CodeOffset, OpCode},
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

macro_rules! runtime_error {
    ($self:ident, $($arg:expr),* $(,)?) => {
        eprintln!($($arg),*);
        let line = $self.chunk.as_ref().unwrap().get_line(&CodeOffset($self.ip - 1));
        eprintln!("[line {}] in script", *line);
    };
}

macro_rules! binary_op {
    ($self:ident, $op:tt) => {
        if !$self.binary_op(|a, b| a $op b) {
            return InterpretResult::RuntimeError;
        }
    }
}

type BinaryOp<T> = fn(f64, f64) -> T;

pub struct VM {
    chunk: Option<Chunk>,
    ip: usize,
    stack: Vec<Value>,
    globals: HashMap<String, Value>,
}

impl VM {
    #[must_use]
    pub fn new() -> Self {
        Self {
            chunk: None,
            ip: 0,
            stack: Vec::with_capacity(256),
            globals: HashMap::new(),
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
        loop {
            #[allow(unused_variables)]
            let instruction = self.read_byte("instruction");
            #[cfg(feature = "trace_execution")]
            {
                let mut disassembler = InstructionDisassembler::new(&self.chunk.as_ref().unwrap());
                *disassembler.offset = self.ip - 1;
                println!(
                    "          [{}]",
                    self.stack
                        .iter()
                        .map(|v| format!("{}", v))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                print!("{:?}", disassembler);
            }
            match OpCode::try_from(instruction).expect("Internal error: unrecognized opcode") {
                OpCode::Print => {
                    println!("{}", self.stack.pop().expect("stack underflow in OP_PRINT"));
                }
                OpCode::Pop => {
                    self.stack.pop().expect("stack underflow in OP_POP");
                }
                OpCode::GetGlobal => match self.read_constant(false).clone() {
                    Value::String(name) => match self.globals.get(&*name) {
                        Some(value) => self.stack.push(value.clone()),
                        None => {
                            runtime_error!(self, "Undefined variable '{}'.", name);
                            return InterpretResult::RuntimeError;
                        }
                    },
                    x => panic!(
                        "Internal error: non-string operand to OP_GET_GLOBAL: {:?}",
                        x
                    ),
                },
                OpCode::DefineGlobal => match self.read_constant(false).clone() {
                    Value::String(name) => {
                        self.globals.insert(
                            *name,
                            self.stack
                                .last()
                                .expect("stack underflow in OP_DEFINE_GLOBAL")
                                .clone(),
                        );
                        self.stack.pop();
                    }
                    x => panic!(
                        "Internal error: non-string operand to OP_DEFINE_GLOBAL: {:?}",
                        x
                    ),
                },
                OpCode::Return => {
                    return InterpretResult::Ok;
                }
                OpCode::Constant => {
                    let value = self.read_constant(false).clone();
                    self.stack.push(value);
                }
                OpCode::ConstantLong => {
                    let value = self.read_constant(true).clone();
                    self.stack.push(value);
                }
                OpCode::Nil => self.stack.push(Value::Nil),
                OpCode::True => self.stack.push(Value::Bool(true)),
                OpCode::False => self.stack.push(Value::Bool(false)),

                OpCode::Negate => {
                    let value = self.stack.last_mut().expect("stack underflow in OP_NEGATE");
                    match value {
                        Value::Number(n) => *n = -*n,
                        _ => {
                            runtime_error!(self, "Operand must be a number.");
                            return InterpretResult::RuntimeError;
                        }
                    }
                }
                OpCode::Not => {
                    let value = self
                        .stack
                        .pop()
                        .expect("stack underflow in OP_NOT")
                        .is_falsey();
                    self.stack.push(value.into());
                }

                OpCode::Equal => {
                    let value = self
                        .stack
                        .pop()
                        .expect("stack underflow in OP_EQUAL (first)")
                        == self
                            .stack
                            .pop()
                            .expect("stack underflow in OP_EQUAL (second)");
                    self.stack.push(value.into());
                }

                OpCode::Add => {
                    let slice_start = self.stack.len() - 2;
                    match &mut self.stack[slice_start..] {
                        [stack_item @ Value::Number(_), Value::Number(b)] => {
                            *stack_item = (stack_item.as_f64() + *b).into();
                            self.stack.pop();
                        }
                        [Value::String(a), Value::String(b)] => {
                            a.push_str(b);
                            self.stack.pop();
                        }
                        _ => {
                            runtime_error!(self, "Operands must be two numbers or two strings.");
                            return InterpretResult::RuntimeError;
                        }
                    }
                }

                OpCode::Subtract => binary_op!(self, -),
                OpCode::Multiply => binary_op!(self, *),
                OpCode::Divide => binary_op!(self, /),

                OpCode::Greater => binary_op!(self, >),
                OpCode::Less => binary_op!(self, <),
            };
        }
    }

    fn read_byte(&mut self, msg: &str) -> u8 {
        self.ip += 1;
        *self.get_byte(self.ip - 1).expect(msg)
    }

    fn get_byte(&self, index: usize) -> Option<&u8> {
        self.chunk.as_ref().unwrap().code().get(index)
    }

    fn read_constant(&mut self, long: bool) -> &Value {
        let index = if long {
            (usize::from(self.read_byte("read_constant/long/0")) << 16)
                + (usize::from(self.read_byte("read_constant/long/1")) << 8)
                + (usize::from(self.read_byte("read_constant/long/2")))
        } else {
            usize::from(self.read_byte("read_constant"))
        };
        self.chunk.as_ref().unwrap().get_constant(index)
    }

    fn binary_op<T: Into<Value>>(&mut self, op: BinaryOp<T>) -> bool {
        let slice_start = self.stack.len() - 2;
        match &mut self.stack[slice_start..] {
            [stack_item @ Value::Number(_), Value::Number(b)] => {
                *stack_item = op(stack_item.as_f64(), *b).into();
                self.stack.pop();
            }
            _ => {
                runtime_error!(self, "Operands must be numbers.");
                return false;
            }
        }
        true
    }
}
