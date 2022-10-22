use std::{
    rc::Rc,
    time::{SystemTime, UNIX_EPOCH},
};

use hashbrown::HashMap;

#[cfg(feature = "trace_execution")]
use crate::chunk::InstructionDisassembler;
use crate::{
    chunk::{CodeOffset, OpCode},
    compiler::Compiler,
    scanner::Scanner,
    value::{Function, NativeFunction, NativeFunctionImpl, Value},
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
        for frame in $self.frames.iter().rev() {
            let line = frame.function.chunk.get_line(&CodeOffset(frame.ip - 1));
            eprintln!("[line {}] in {}", *line, frame.function.name);
        }
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
    function: Rc<Function>,
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
        let mut vm = Self {
            frames: Vec::with_capacity(crate::config::FRAMES_MAX),
            stack: Vec::with_capacity(crate::config::STACK_MAX),
            globals: HashMap::new(),
        };

        vm.define_native("clock", clock_native);

        vm
    }

    pub fn interpret(&mut self, source: &[u8]) -> InterpretResult {
        let scanner = Scanner::new(source);
        let result = if let Some(function) = Compiler::compile(scanner) {
            let function = Rc::new(function);
            self.stack_push(Value::Function(Rc::clone(&function)));
            self.execute_call(function, 0);
            self.run()
        } else {
            InterpretResult::CompileError
        };

        if result == InterpretResult::Ok {
            assert_eq!(self.stack.len(), 0);
        }
        result
    }

    fn run(&mut self) -> InterpretResult {
        loop {
            #[cfg(feature = "trace_execution")]
            {
                let function_ref = self.function();
                let function = function_ref.borrow();
                let mut disassembler = InstructionDisassembler::new(&function.chunk);
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
            match OpCode::try_from(self.read_byte("instruction"))
                .expect("Internal error: unrecognized opcode")
            {
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
                op @ (OpCode::GetLocal | OpCode::GetLocalLong) => self.get_local(op),
                op @ (OpCode::SetLocal | OpCode::SetLocalLong) => self.set_local(op),
                op @ (OpCode::GetGlobal | OpCode::GetGlobalLong) => {
                    if let Some(value) = self.get_global(op) {
                        return value;
                    }
                }
                op @ (OpCode::SetGlobal | OpCode::SetGlobalLong) => {
                    if let Some(value) = self.set_global(op) {
                        return value;
                    }
                }
                op @ (OpCode::DefineGlobal
                | OpCode::DefineGlobalLong
                | OpCode::DefineGlobalConst
                | OpCode::DefineGlobalConstLong) => self.define_global(op),
                OpCode::JumpIfFalse => {
                    self.jump_if_false();
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
                OpCode::Call => {
                    if let Some(value) = self.call() {
                        return value;
                    }
                }
                OpCode::Return => {
                    if let Some(value) = self.return_() {
                        return value;
                    }
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
                    if let Some(value) = self.negate() {
                        return value;
                    }
                }
                OpCode::Not => {
                    self.not_();
                }

                OpCode::Equal => {
                    self.equal();
                }

                OpCode::Add => {
                    if let Some(value) = self.add() {
                        return value;
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

    fn add(&mut self) -> Option<InterpretResult> {
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
            args => {
                if crate::config::is_std_mode() {
                    runtime_error!(self, "Operands must be two numbers or two strings.");
                } else {
                    runtime_error!(
                        self,
                        "Operands must be two numbers or two strings. Got: [{}]",
                        args.iter()
                            .map(|v| format!("{}", v))
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
                return Some(InterpretResult::RuntimeError);
            }
        }
        None
    }

    fn equal(&mut self) {
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

    fn not_(&mut self) {
        let value = self
            .stack
            .pop()
            .expect("stack underflow in OP_NOT")
            .is_falsey();
        self.stack_push(value.into());
    }

    fn negate(&mut self) -> Option<InterpretResult> {
        let value = self.stack.last_mut().expect("stack underflow in OP_NEGATE");
        match value {
            Value::Number(n) => *n = -*n,
            _ => {
                runtime_error!(self, "Operand must be a number.");
                return Some(InterpretResult::RuntimeError);
            }
        }
        None
    }

    fn jump_if_false(&mut self) {
        let offset = self.read_16bit_number("Internal error: missing operand for OP_JUMP_IF_FALSE");
        if self
            .stack
            .last()
            .expect("stack underflow in OP_JUMP_IF_FALSE")
            .is_falsey()
        {
            self.frame_mut().ip += offset;
        }
    }

    fn define_global(&mut self, op: OpCode) {
        match self.read_constant(op == OpCode::DefineGlobalLong) {
            Value::String(name) => {
                let name = name.clone();
                self.globals.insert(
                    name,
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

    fn return_(&mut self) -> Option<InterpretResult> {
        let result = self.stack.pop();
        let frame = self
            .frames
            .pop()
            .expect("Call stack underflow in OP_RETURN");
        if self.frames.is_empty() {
            self.stack.pop();
            return Some(InterpretResult::Ok);
        }
        self.stack.truncate(frame.stack_base);
        self.stack_push(result.expect("Stack underflow in OP_RETURN"));
        None
    }

    fn call(&mut self) -> Option<InterpretResult> {
        let arg_count = self.read_byte("Internal error: missing operand for OP_CALL");
        if !self.call_value(
            self.stack[self.stack.len() - 1 - usize::from(arg_count)].clone(),
            arg_count,
        ) {
            return Some(InterpretResult::RuntimeError);
        }
        None
    }

    fn set_global(&mut self, op: OpCode) -> Option<InterpretResult> {
        let constant_index = self.read_constant_index(op == OpCode::SetGlobalLong);
        match self.read_constant_value(constant_index).clone() {
            Value::String(name) => {
                let name = name.as_str();
                if let Some(global) = self.globals.get_mut(name) {
                    if !global.mutable {
                        runtime_error!(self, "Reassignment to global 'const'.");
                        return Some(InterpretResult::RuntimeError);
                    }
                    global.value = self
                        .stack
                        .last()
                        .unwrap_or_else(|| panic!("stack underflow in {:?}", op))
                        .clone();
                } else {
                    runtime_error!(self, "Undefined variable '{}'.", name);
                    return Some(InterpretResult::RuntimeError);
                }
            }
            x => panic!(
                "Internal error: non-string operand to OP_SET_GLOBAL: {:?}",
                x
            ),
        }
        None
    }

    fn get_global(&mut self, op: OpCode) -> Option<InterpretResult> {
        let constant_index = self.read_constant_index(op == OpCode::GetGlobalLong);
        match self.read_constant_value(constant_index) {
            Value::String(name) => match self.globals.get(&**name) {
                Some(global) => self.stack_push(global.value.clone()),
                None => {
                    runtime_error!(self, "Undefined variable '{}'.", name);
                    return Some(InterpretResult::RuntimeError);
                }
            },
            x => panic!("Internal error: non-string operand to {:?}: {:?}", op, x),
        }
        None
    }

    fn set_local(&mut self, op: OpCode) {
        let slot = if op == OpCode::GetLocalLong {
            self.read_24bit_number("Internal error: missing operand for OP_SET_LOCAL_LONG")
        } else {
            usize::from(self.read_byte("Internal error: missing operand for OP_SET_LOCAL"))
        };
        *self.stack_get_mut(slot) = self
            .stack
            .last()
            .expect("stack underflow in OP_SET_LOCAL")
            .clone();
    }

    fn get_local(&mut self, op: OpCode) {
        let slot = if op == OpCode::GetLocalLong {
            self.read_24bit_number("Internal error: missing operand for OP_GET_LOCAL_LONG")
        } else {
            usize::from(self.read_byte("Internal error: missing operand for OP_GET_LOCAL"))
        };
        self.stack_push(self.stack_get(slot).clone());
    }

    fn read_byte(&mut self, msg: &str) -> u8 {
        let frame = self.frame_mut();
        frame.ip += 1;
        let index = frame.ip - 1;
        *frame.function.chunk.code().get(index).expect(msg)
    }

    fn read_24bit_number(&mut self, msg: &str) -> usize {
        (usize::from(self.read_byte(msg)) << 16)
            + (usize::from(self.read_byte(msg)) << 8)
            + (usize::from(self.read_byte(msg)))
    }

    fn read_16bit_number(&mut self, msg: &str) -> usize {
        (usize::from(self.read_byte(msg)) << 8) + (usize::from(self.read_byte(msg)))
    }

    fn read_constant_index(&mut self, long: bool) -> usize {
        if long {
            self.read_24bit_number("read_constant/long")
        } else {
            usize::from(self.read_byte("read_constant"))
        }
    }

    fn read_constant_value(&self, index: usize) -> &Value {
        self.frame().function.chunk.get_constant(index)
    }

    fn read_constant(&mut self, long: bool) -> &Value {
        let index = self.read_constant_index(long);
        self.read_constant_value(index)
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

    #[inline]
    fn stack_push(&mut self, value: Value) {
        self.stack.push(value);
        // This check has a pretty big performance overhead; disabled for now
        // TODO find a better way: keep the check and minimize overhead
        /*
        if self.stack.len() > STACK_MAX {
            runtime_error!(self, "Stack overflow");
        }
        */
    }

    fn stack_get(&self, slot: usize) -> &Value {
        &self.stack[self.stack_base() + slot]
    }

    fn stack_get_mut(&mut self, slot: usize) -> &mut Value {
        let offset = self.stack_base();
        &mut self.stack[offset + slot]
    }

    fn frame(&self) -> &CallFrame {
        self.frames
            .last()
            .expect("Out of execute_call frames, somehow?")
    }

    fn frame_mut(&mut self) -> &mut CallFrame {
        let i = self.frames.len() - 1;
        &mut self.frames[i]
    }

    fn stack_base(&self) -> usize {
        self.frame().stack_base
    }

    fn call_value(&mut self, callee: Value, arg_count: u8) -> bool {
        match callee {
            Value::Function(f) => self.execute_call(f, arg_count),
            Value::NativeFunction(NativeFunction { fun, .. }) => {
                let start_index = self.stack.len() - usize::from(arg_count);
                let result = fun(&mut self.stack[start_index..]);
                self.stack
                    .truncate(self.stack.len() - usize::from(arg_count) - 1);
                self.stack_push(result);
                true
            }
            _ => {
                runtime_error!(self, "Can only call functions and classes.");
                false
            }
        }
    }

    fn execute_call(&mut self, f: Rc<Function>, arg_count: u8) -> bool {
        let arity = f.arity;
        let arg_count = usize::from(arg_count);
        if arg_count != arity {
            runtime_error!(self, "Expected {} arguments but got {}.", arity, arg_count);
            return false;
        }

        if self.frames.len() == crate::config::FRAMES_MAX {
            runtime_error!(self, "Stack overflow.");
            return false;
        }

        self.frames.push(CallFrame {
            function: f,
            ip: 0,
            stack_base: self.stack.len() - arg_count - 1,
        });
        true
    }

    pub fn define_native<S>(&mut self, name: S, fun: NativeFunctionImpl)
    where
        S: ToString,
    {
        self.globals.insert(
            name.to_string(),
            Global {
                value: Value::NativeFunction(NativeFunction {
                    name: name.to_string(),
                    fun,
                }),
                mutable: false,
            },
        );
    }
}

fn clock_native(_args: &mut [Value]) -> Value {
    Value::Number(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs_f64(),
    )
}
