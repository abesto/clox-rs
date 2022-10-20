use std::{cell::RefCell, rc::Rc};

use crate::chunk::Chunk;

#[derive(Debug, PartialEq, PartialOrd, Clone)]
#[allow(clippy::box_collection)]
pub enum Value {
    Bool(bool),
    Nil,
    Number(f64),

    String(Box<String>),
    Function(Rc<RefCell<Function>>),
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

impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::String(Box::new(s))
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Bool(bool) => f.pad(&format!("{}", bool)),
            Value::Number(num) => f.pad(&format!("{}", num)),
            Value::Nil => f.pad("nil"),
            Value::String(s) => f.pad(&format!("{}", *s)),
            Value::Function(fun) => f.pad(&format!("<fn {}>", fun.borrow().name)),
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
    pub name: String,
}

impl Function {
    #[must_use]
    pub fn new<S>(arity: usize, name: S) -> Self
    where
        S: ToString,
    {
        Self {
            arity,
            name: name.to_string(),
            chunk: Chunk::new(name),
        }
    }
}

#[cfg(test)]
#[test]
fn value_size() {
    assert_eq!(16, std::mem::size_of::<Value>());
}
