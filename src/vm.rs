use std::pin::Pin;
use std::rc::Rc;

use hashbrown::HashMap;

use crate::arena::ValueId;
use crate::chunk::InstructionDisassembler;
use crate::native_functions::NativeFunctions;
use crate::{
    arena::{Arena, StringId},
    chunk::{CodeOffset, OpCode},
    compiler::Compiler,
    config,
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
            eprintln!("[line {}] in {}", *line, *frame.function.name);
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
    value: ValueId,
    mutable: bool,
}

pub struct CallFrame {
    function: Rc<Function>,
    ip: usize,
    stack_base: usize,
}

struct BuiltinConstants {
    pub nil: ValueId,
    pub true_: ValueId,
    pub false_: ValueId,
}

impl BuiltinConstants {
    #[must_use]
    pub fn new(arena: &mut Arena) -> Self {
        Self {
            nil: arena.add_value(Value::Nil),
            true_: arena.add_value(Value::Bool(true)),
            false_: arena.add_value(Value::Bool(false)),
        }
    }

    pub fn bool(&self, val: bool) -> ValueId {
        if val {
            self.true_
        } else {
            self.false_
        }
    }
}

pub struct VM {
    arena: Pin<Box<Arena>>,
    builtin_constants: BuiltinConstants,
    frames: Vec<CallFrame>,
    stack: Vec<ValueId>,
    globals: HashMap<StringId, Global>,
}

impl VM {
    #[must_use]
    pub fn new() -> Self {
        let mut arena = Pin::new(Box::new(Arena::new()));
        Self {
            builtin_constants: BuiltinConstants::new(&mut arena),
            arena,
            frames: Vec::with_capacity(crate::config::FRAMES_MAX),
            stack: Vec::with_capacity(crate::config::STACK_MAX),
            globals: HashMap::new(),
        }
    }

    pub fn interpret(&mut self, source: &[u8]) -> InterpretResult {
        let scanner = Scanner::new(source);

        let mut native_functions = NativeFunctions::new();
        native_functions.create_names(&mut self.arena);
        let mut compiler = Compiler::new(scanner, &mut self.arena);
        native_functions.register_names(&mut compiler);

        let result = if let Some(function) = compiler.compile() {
            native_functions.define_functions(self);
            let function = Rc::new(function);
            self.stack_push_value(Value::Function(Rc::clone(&function)));
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
        let trace_execution = config::TRACE_EXECUTION.load();
        loop {
            if trace_execution {
                let function = &self.frame().function;
                let mut disassembler = InstructionDisassembler::new(&function.chunk);
                *disassembler.offset = self.frame().ip;
                println!(
                    "          [{}]",
                    self.stack
                        .iter()
                        .map(|v| format!("{}", **v))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                print!("{:?}", disassembler);
            }
            match OpCode::try_from(self.read_byte("instruction"))
                .expect("Internal error: unrecognized opcode")
            {
                OpCode::Print => {
                    println!(
                        "{}",
                        *self.stack.pop().expect("stack underflow in OP_PRINT")
                    );
                }
                OpCode::Pop => {
                    self.stack.pop().expect("stack underflow in OP_POP");
                }
                OpCode::Dup => {
                    self.stack_push_value(
                        (**self.stack.last().expect("stack underflow in OP_DUP")).clone(),
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
                    let value = *self.read_constant(false);
                    self.stack_push(value);
                }
                OpCode::ConstantLong => {
                    let value = *self.read_constant(true);
                    self.stack_push(value);
                }
                OpCode::Closure => {
                    let value = *self.read_constant(true);
                    self.stack_push(value);
                }
                OpCode::Nil => self.stack_push(self.builtin_constants.nil),
                OpCode::True => self.stack_push(self.builtin_constants.true_),
                OpCode::False => self.stack_push(self.builtin_constants.false_),

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

        let ok = match &mut self.stack[slice_start..] {
            [left, right] => match (&mut **left, &**right) {
                (Value::Number(a), Value::Number(b)) => {
                    let value = (*a + *b).into();
                    self.stack.pop();
                    self.stack.pop();
                    self.stack_push_value(value);
                    true
                }
                (Value::String(a), Value::String(b)) => {
                    // This could be optimized by allowing mutations via the arena
                    let new_string_id = self.arena.add_string(format!("{}{}", **a, **b));
                    self.stack.pop();
                    self.stack.pop();
                    self.stack_push_value(new_string_id.into());
                    true
                }
                _ => false,
            },
            _ => false,
        };

        if !ok {
            if config::STD_MODE.load() {
                runtime_error!(self, "Operands must be two numbers or two strings.");
            } else {
                runtime_error!(
                    self,
                    "Operands must be two numbers or two strings. Got: [{}]",
                    self.stack[slice_start..]
                        .iter()
                        .map(|v| format!("{}", **v))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
            return Some(InterpretResult::RuntimeError);
        }
        None
    }

    fn equal(&mut self) {
        let value = *self
            .stack
            .pop()
            .expect("stack underflow in OP_EQUAL (first)")
            == *self
                .stack
                .pop()
                .expect("stack underflow in OP_EQUAL (second)");
        self.stack_push(self.builtin_constants.bool(value));
    }

    fn not_(&mut self) {
        let value = self
            .stack
            .pop()
            .expect("stack underflow in OP_NOT")
            .is_falsey();
        self.stack_push(self.builtin_constants.bool(value));
    }

    fn negate(&mut self) -> Option<InterpretResult> {
        let value = &mut **self.stack.last_mut().expect("stack underflow in OP_NEGATE");
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
        match &**self.read_constant(op == OpCode::DefineGlobalLong) {
            Value::String(name) => {
                let name = *name;
                self.globals.insert(
                    name,
                    Global {
                        value: *self
                            .stack
                            .last()
                            .unwrap_or_else(|| panic!("stack underflow in {:?}", op)),
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
        let callee = self.stack[self.stack.len() - 1 - usize::from(arg_count)];
        if !self.call_value(callee, arg_count) {
            return Some(InterpretResult::RuntimeError);
        }
        None
    }

    fn set_global(&mut self, op: OpCode) -> Option<InterpretResult> {
        let constant_index = self.read_constant_index(op == OpCode::SetGlobalLong);

        let name = match &**self.read_constant_value(constant_index) {
            Value::String(name) => *name,
            x => panic!(
                "Internal error: non-string operand to OP_SET_GLOBAL: {:?}",
                x
            ),
        };

        if let Some(global) = self.globals.get_mut(&name) {
            if !global.mutable {
                runtime_error!(self, "Reassignment to global 'const'.");
                return Some(InterpretResult::RuntimeError);
            }
            global.value = *self
                .stack
                .last()
                .unwrap_or_else(|| panic!("stack underflow in {:?}", op));
        } else {
            runtime_error!(self, "Undefined variable '{}'.", *name);
            return Some(InterpretResult::RuntimeError);
        }

        None
    }

    fn get_global(&mut self, op: OpCode) -> Option<InterpretResult> {
        let constant_index = self.read_constant_index(op == OpCode::GetGlobalLong);
        match &**self.read_constant_value(constant_index) {
            Value::String(name) => match self.globals.get(name) {
                Some(global) => self.stack_push(global.value),
                None => {
                    runtime_error!(self, "Undefined variable '{}'.", **name);
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
        *self.stack_get_mut(slot) = *self.stack.last().expect("stack underflow in OP_SET_LOCAL");
    }

    fn get_local(&mut self, op: OpCode) {
        let slot = if op == OpCode::GetLocalLong {
            self.read_24bit_number("Internal error: missing operand for OP_GET_LOCAL_LONG")
        } else {
            usize::from(self.read_byte("Internal error: missing operand for OP_GET_LOCAL"))
        };
        self.stack_push(*self.stack_get(slot));
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

    fn read_constant_value(&self, index: usize) -> &ValueId {
        self.frame().function.chunk.get_constant(index)
    }

    fn read_constant(&mut self, long: bool) -> &ValueId {
        let index = self.read_constant_index(long);
        self.read_constant_value(index)
    }

    fn binary_op<T: Into<Value>>(&mut self, op: BinaryOp<T>) -> bool {
        let slice_start = self.stack.len() - 2;

        let ok = match &mut self.stack[slice_start..] {
            [left, right] => {
                if let (Value::Number(a), Value::Number(b)) = (&**left, &**right) {
                    let value = op(*a, *b).into();
                    self.stack.pop();
                    self.stack.pop();
                    self.stack_push_value(value);
                    true
                } else {
                    false
                }
            }
            _ => false,
        };

        if !ok {
            runtime_error!(self, "Operands must be numbers.");
        }
        ok
    }

    #[inline]
    fn stack_push(&mut self, value_id: ValueId) {
        self.stack.push(value_id);
        // This check has a pretty big performance overhead; disabled for now
        // TODO find a better way: keep the check and minimize overhead
        /*
        if self.stack.len() > STACK_MAX {
            runtime_error!(self, "Stack overflow");
        }
        */
    }

    #[inline]
    fn stack_push_value(&mut self, value: Value) {
        let value_id = self.arena.add_value(value);
        self.stack.push(value_id);
    }

    fn stack_get(&self, slot: usize) -> &ValueId {
        &self.stack[self.stack_base() + slot]
    }

    fn stack_get_mut(&mut self, slot: usize) -> &mut ValueId {
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

    fn call_value(&mut self, callee: ValueId, arg_count: u8) -> bool {
        match &*callee {
            Value::Function(f) => self.execute_call(Rc::clone(f), arg_count),
            Value::NativeFunction(NativeFunction { fun, arity, name }) => {
                if arg_count != *arity {
                    runtime_error!(
                        self,
                        "Native function '{}' expected {} arguments, got {}.",
                        name,
                        arity,
                        arg_count
                    );
                    false
                } else {
                    let start_index = self.stack.len() - usize::from(arg_count);
                    let args = self.stack[start_index..]
                        .iter()
                        .map(|v| (**v).clone())
                        .collect::<Vec<_>>();
                    match fun(&args) {
                        Ok(value) => {
                            self.stack
                                .truncate(self.stack.len() - usize::from(arg_count) - 1);
                            self.stack_push_value(value);
                            true
                        }
                        Err(e) => {
                            runtime_error!(self, "{}", e);
                            false
                        }
                    }
                }
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

    pub fn define_native(&mut self, name: StringId, arity: u8, fun: NativeFunctionImpl) {
        let value = Value::NativeFunction(NativeFunction {
            name: name.to_string(),
            arity,
            fun,
        });
        let value_id = self.arena.add_value(value);

        self.globals.insert(
            name,
            Global {
                value: value_id,
                mutable: false,
            },
        );
    }
}
