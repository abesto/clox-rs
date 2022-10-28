use derivative::Derivative;

use crate::{
    arena::{FunctionId, StringId, ValueId},
    chunk::Chunk,
    config,
};

#[derive(Debug, PartialEq, PartialOrd, Clone)]
pub enum Value {
    Bool(bool),
    Nil,
    Number(f64),

    String(StringId),

    Function(FunctionId),
    Closure(Closure),
    NativeFunction(NativeFunction),

    Upvalue(usize),
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Clone)]
pub struct Closure {
    pub function: FunctionId,
    pub upvalues: Vec<ValueId>,
    pub upvalue_count: u8,
}

impl Closure {
    pub fn new(function: FunctionId) -> Closure {
        let upvalue_count = function.upvalue_count;
        Closure {
            function,
            upvalues: Vec::with_capacity(usize::from(upvalue_count)),
            upvalue_count,
        }
    }
}

impl Value {
    pub fn closure(function: FunctionId) -> Value {
        let upvalue_count = function.upvalue_count;
        Value::Closure(Closure {
            function,
            upvalues: Vec::with_capacity(usize::from(upvalue_count)),
            upvalue_count,
        })
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}

impl From<f64> for Value {
    fn from(f: f64) -> Self {
        Value::Number(f)
    }
}

impl From<StringId> for Value {
    fn from(s: StringId) -> Self {
        Value::String(s)
    }
}

impl From<FunctionId> for Value {
    fn from(f: FunctionId) -> Self {
        Value::Function(f)
    }
}

impl From<Closure> for Value {
    fn from(c: Closure) -> Self {
        Value::Closure(c)
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Bool(bool) => f.pad(&format!("{}", bool)),
            Value::Number(num) => f.pad(&format!("{}", num)),
            Value::Nil => f.pad("nil"),
            Value::String(s) => f.pad(s),
            Value::Function(function_id) => f.pad(&format!("<fn {}>", *function_id.name)),
            Value::Closure(closure) => {
                if config::STD_MODE.load() {
                    f.pad(&format!("<fn {}>", *closure.function.name))
                } else {
                    f.pad(&format!(
                        "<fn {}:{}:{}>",
                        *closure.function.name,
                        closure.upvalue_count,
                        closure.upvalues.len()
                    ))
                }
            }
            Value::NativeFunction(fun) => {
                if config::STD_MODE.load() {
                    f.pad("<native fn>")
                } else {
                    f.pad(&format!("<native fn {}>", fun.name))
                }
            }
            Value::Upvalue(_) => f.pad("upvalue"),
        }
    }
}

impl Value {
    pub fn is_falsey(&self) -> bool {
        matches!(self, Self::Bool(false) | Self::Nil)
    }

    pub fn as_closure(&self) -> &Closure {
        match self {
            Value::Closure(c) => c,
            _ => unreachable!("Expected Closure, found `{}`", self),
        }
    }

    pub fn as_function(&self) -> &FunctionId {
        match self {
            Value::Function(f) => f,
            _ => unreachable!("Expected Function, found `{}`", self),
        }
    }

    pub fn as_upvalue(&self) -> usize {
        match self {
            Value::Upvalue(v) => *v,
            _ => unreachable!("Expected upvalue, found `{}`", self),
        }
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Clone)]
pub struct Function {
    pub arity: usize,
    pub chunk: Chunk,
    pub name: StringId,
    pub upvalue_count: u8,
}

impl Function {
    #[must_use]
    pub fn new(arity: usize, name: StringId) -> Self {
        Self {
            arity,
            name,
            chunk: Chunk::new(name),
            upvalue_count: 0,
        }
    }
}

#[derive(Derivative)]
#[derivative(Debug, PartialEq, PartialOrd, Clone)]
pub struct NativeFunction {
    pub name: String,
    pub arity: u8,

    #[derivative(
            Debug = "ignore",
            // Treat the implementation as always equal; we discriminate built-in functions by name
            PartialEq(compare_with = "always_equals"),
            PartialOrd = "ignore"
        )]
    pub fun: NativeFunctionImpl,
}

pub type NativeFunctionImpl = fn(&[Value]) -> Result<Value, String>;

fn always_equals<T>(_: &T, _: &T) -> bool {
    true
}
