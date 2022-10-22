use std::rc::Rc;

use derivative::Derivative;

use crate::{arena::StringId, chunk::Chunk};

#[derive(Debug, PartialEq, PartialOrd, Clone)]
pub enum Value {
    Bool(bool),
    Nil,
    Number(f64),

    String(StringId),
    Function(Rc<Function>),
    NativeFunction(NativeFunction),
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

impl From<Function> for Value {
    fn from(f: Function) -> Self {
        Value::Function(Rc::new(f))
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Bool(bool) => f.pad(&format!("{}", bool)),
            Value::Number(num) => f.pad(&format!("{}", num)),
            Value::Nil => f.pad("nil"),
            Value::String(s) => f.pad(s),
            Value::Function(fun) => f.pad(&format!("<fn {}>", *fun.name)),
            Value::NativeFunction(fun) => {
                if crate::config::is_std_mode() {
                    f.pad("<native fn>")
                } else {
                    f.pad(&format!("<native fn {}>", fun.name))
                }
            }
        }
    }
}

impl Value {
    pub fn is_falsey(&self) -> bool {
        matches!(self, Self::Bool(false) | Self::Nil)
    }

    pub fn as_f64(&self) -> f64 {
        match self {
            Self::Number(num) => *num,
            _ => panic!("as_64() called on non-Number Value"),
        }
    }
}

#[derive(Debug, PartialEq, PartialOrd, Clone)]
pub struct Function {
    pub arity: usize,
    pub chunk: Chunk,
    pub name: StringId,
}

impl Function {
    #[must_use]
    pub fn new(arity: usize, name: StringId) -> Self {
        Self {
            arity,
            name,
            chunk: Chunk::new(name),
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

pub type NativeFunctionImpl = fn(&mut [Value]) -> Result<Value, String>;

fn always_equals<T>(_: &T, _: &T) -> bool {
    true
}
