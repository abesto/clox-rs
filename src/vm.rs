use std::{
    cell::{Ref, RefCell},
    collections::HashMap,
    rc::Rc,
};

#[cfg(feature = "trace_execution")]
use crate::chunk::InstructionDisassembler;
use crate::{
    chunk::{CodeOffset, OpCode},
    compiler::Compiler,
    scanner::Scanner,
    value::{Function, Value},
};

const FRAMES_MAX: usize = 64;
const STACK_MAX: usize = FRAMES_MAX * 255;

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
        let line = $self.function().borrow().chunk.get_line(&CodeOffset($self.frame().ip - 1));
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

struct Global {
    value: Value,
    mutable: bool,
}

pub struct CallFrame {
    function: Rc<RefCell<Function>>,
    ip: usize,
    stack_base: usize,
}

pub struct VM {
    frames: Vec<CallFrame>,
    stack: Vec<Value>,
    globals: HashMap<String, Global>,
}

impl VM {
    #[must_use]
    pub fn new() -> Self {
        Self {
            frames: Vec::with_capacity(FRAMES_MAX),
            stack: Vec::with_capacity(STACK_MAX),
            globals: HashMap::new(),
        }
    }

    pub fn interpret(&mut self, source: &[u8]) -> InterpretResult {
        let scanner = Scanner::new(source);
        let result = if let Some(function) = Compiler::compile(scanner) {
            let function = Rc::new(RefCell::new(function));
            self.frames.push(CallFrame {
                function: Rc::clone(&function),
                ip: 0,
                stack_base: 0,
            });
            self.stack_push(Value::Function(function));
            self.run()
        } else {
            InterpretResult::CompileError
        };

        if result == InterpretResult::Ok {
            assert_eq!(self.stack.len(), 1);
        }
        result
    }

    fn run(&mut self) -> InterpretResult {
        loop {
            #[allow(unused_variables)]
            let instruction = self.read_byte("instruction");
            #[cfg(feature = "trace_execution")]
            {
                let mut disassembler = InstructionDisassembler::new(&self.chunk());
                *disassembler.offset = self.frame().ip - 1;
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
                OpCode::Dup => {
                    self.stack_push(
                        self.stack
                            .last()
                            .expect("stack underflow in OP_DUP")
                            .clone(),
                    );
                }
                op @ (OpCode::GetLocal | OpCode::GetLocalLong) => {
                    let slot = if op == OpCode::GetLocalLong {
                        self.read_24bit_number(
                            "Internal error: missing operand for OP_GET_LOCAL_LONG",
                        )
                    } else {
                        usize::from(
                            self.read_byte("Internal error: missing operand for OP_GET_LOCAL"),
                        )
                    };
                    let value = self.stack_get(slot).clone();
                    self.stack_push(value);
                }
                op @ (OpCode::SetLocal | OpCode::SetLocalLong) => {
                    let slot = if op == OpCode::GetLocalLong {
                        self.read_24bit_number(
                            "Internal error: missing operand for OP_SET_LOCAL_LONG",
                        )
                    } else {
                        usize::from(
                            self.read_byte("Internal error: missing operand for OP_SET_LOCAL"),
                        )
                    };
                    *self.stack_get_mut(slot) = self
                        .stack
                        .last()
                        .expect("stack underflow in OP_SET_LOCAL")
                        .clone();
                }
                op @ (OpCode::GetGlobal | OpCode::GetGlobalLong) => {
                    match self.read_constant(op == OpCode::GetGlobalLong).clone() {
                        Value::String(name) => match self.globals.get(&*name) {
                            Some(global) => self.stack_push(global.value.clone()),
                            None => {
                                runtime_error!(self, "Undefined variable '{}'.", name);
                                return InterpretResult::RuntimeError;
                            }
                        },
                        x => panic!("Internal error: non-string operand to {:?}: {:?}", op, x),
                    }
                }
                op @ (OpCode::SetGlobal | OpCode::SetGlobalLong) => {
                    match self.read_constant(op == OpCode::SetGlobalLong).clone() {
                        Value::String(name) => {
                            if let Some(global) = self.globals.get_mut(&*name) {
                                if !global.mutable {
                                    runtime_error!(self, "Reassignment to global 'const'.");
                                    return InterpretResult::RuntimeError;
                                }
                                global.value = self
                                    .stack
                                    .last()
                                    .unwrap_or_else(|| panic!("stack underflow in {:?}", op))
                                    .clone();
                            } else {
                                runtime_error!(self, "Undefined variable '{}'.", name);
                                return InterpretResult::RuntimeError;
                            }
                        }
                        x => panic!(
                            "Internal error: non-string operand to OP_SET_GLOBAL: {:?}",
                            x
                        ),
                    }
                }
                op @ (OpCode::DefineGlobal
                | OpCode::DefineGlobalLong
                | OpCode::DefineGlobalConst
                | OpCode::DefineGlobalConstLong) => {
                    match self.read_constant(op == OpCode::DefineGlobalLong).clone() {
                        Value::String(name) => {
                            self.globals.insert(
                                *name,
                                Global {
                                    value: self
                                        .stack
                                        .last()
                                        .unwrap_or_else(|| panic!("stack underflow in {:?}", op))
                                        .clone(),
                                    mutable: op != OpCode::DefineGlobalConst
                                        && op != OpCode::DefineGlobalConstLong,
                                },
                            );
                            self.stack.pop();
                        }
                        x => panic!(
                            "Internal error: non-string operand to OP_DEFINE_GLOBAL: {:?}",
                            x
                        ),
                    }
                }
                OpCode::JumpIfFalse => {
                    let offset = self
                        .read_16bit_number("Internal error: missing operand for OP_JUMP_IF_FALSE");
                    if self
                        .stack
                        .last()
                        .expect("stack underflow in OP_JUMP_IF_FALSE")
                        .is_falsey()
                    {
                        self.frame_mut().ip += offset;
                    }
                }
                OpCode::Jump => {
                    let offset =
                        self.read_16bit_number("Internal error: missing operand for OP_JUMP");
                    self.frame_mut().ip += offset;
                }
                OpCode::Loop => {
                    let offset =
                        self.read_16bit_number("Internal error: missing operand for OP_+loop");
                    self.frame_mut().ip -= offset;
                }
                OpCode::Return => {
                    return InterpretResult::Ok;
                }
                OpCode::Constant => {
                    let value = self.read_constant(false).clone();
                    self.stack_push(value);
                }
                OpCode::ConstantLong => {
                    let value = self.read_constant(true).clone();
                    self.stack_push(value);
                }
                OpCode::Nil => self.stack_push(Value::Nil),
                OpCode::True => self.stack_push(Value::Bool(true)),
                OpCode::False => self.stack_push(Value::Bool(false)),

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
                    self.stack_push(value.into());
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
                    self.stack_push(value.into());
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
        let frame = self.frame_mut();
        frame.ip += 1;
        let index = frame.ip - 1;
        self.get_byte(index).expect(msg)
    }

    fn get_byte(&self, index: usize) -> Option<u8> {
        self.function().borrow().chunk.code().get(index).cloned()
    }

    fn read_24bit_number(&mut self, msg: &str) -> usize {
        (usize::from(self.read_byte(msg)) << 16)
            + (usize::from(self.read_byte(msg)) << 8)
            + (usize::from(self.read_byte(msg)))
    }

    fn read_16bit_number(&mut self, msg: &str) -> usize {
        (usize::from(self.read_byte(msg)) << 8) + (usize::from(self.read_byte(msg)))
    }

    fn read_constant(&mut self, long: bool) -> Value {
        let index = if long {
            self.read_24bit_number("read_constant/long")
        } else {
            usize::from(self.read_byte("read_constant"))
        };
        self.function().borrow().chunk.get_constant(index).clone()
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

    fn stack_push(&mut self, value: Value) {
        if self.stack.len() == STACK_MAX {
            runtime_error!(self, "Stack overflow");
        } else {
            self.stack.push(value);
        }
    }

    fn stack_get(&self, slot: usize) -> &Value {
        &self.stack[self.stack_base() + slot]
    }

    fn stack_get_mut(&mut self, slot: usize) -> &mut Value {
        let offset = self.stack_base();
        &mut self.stack[offset + slot]
    }

    fn frame(&self) -> &CallFrame {
        self.frames.last().expect("Out of call frames, somehow?")
    }

    fn frame_mut(&mut self) -> &mut CallFrame {
        self.frames
            .last_mut()
            .expect("Out of call frames, somehow?")
    }

    fn stack_base(&self) -> usize {
        self.frame().stack_base
    }

    fn function(&self) -> Rc<RefCell<Function>> {
        Rc::clone(&self.frame().function)
    }
}
