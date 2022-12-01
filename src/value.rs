use derivative::Derivative;
use hashbrown::HashMap;

use crate::{
    chunk::Chunk,
    config,
    heap::{FunctionId, Heap, StringId, ValueId},
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

    Upvalue(Upvalue),

    Class(Class),
    Instance(Instance),
    BoundMethod(BoundMethod),
}

#[derive(Debug, PartialEq, PartialOrd, Clone)]
pub enum Upvalue {
    Open(usize),
    Closed(ValueId),
}

impl Upvalue {
    pub fn as_open(&self) -> usize {
        match self {
            Upvalue::Open(n) => *n,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, PartialEq, PartialOrd, Clone)]
pub struct Closure {
    pub function: FunctionId,
    pub upvalues: Vec<ValueId>,
    pub upvalue_count: usize,
}

impl Closure {
    pub fn new(function: FunctionId) -> Closure {
        let upvalue_count = function.upvalue_count;
        Closure {
            function,
            upvalues: Vec::with_capacity(upvalue_count),
            upvalue_count,
        }
    }
}

impl Value {
    pub fn closure(function: FunctionId) -> Value {
        let upvalue_count = function.upvalue_count;
        Value::Closure(Closure {
            function,
            upvalues: Vec::with_capacity(upvalue_count),
            upvalue_count,
        })
    }

    pub fn bound_method(receiver: ValueId, method: ValueId) -> Value {
        Value::BoundMethod(BoundMethod { receiver, method })
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

impl From<Class> for Value {
    fn from(c: Class) -> Self {
        Value::Class(c)
    }
}

impl From<Instance> for Value {
    fn from(i: Instance) -> Self {
        Value::Instance(i)
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
            Value::Closure(closure) => f.pad(&format!("<fn {}>", *closure.function.name)),
            Value::NativeFunction(fun) => {
                if config::STD_MODE.load() {
                    f.pad("<native fn>")
                } else {
                    f.pad(&format!("<native fn {}>", fun.name))
                }
            }
            Value::Upvalue(_) => f.pad("upvalue"),
            Value::Class(c) => {
                if config::STD_MODE.load() {
                    f.pad(&c.name)
                } else {
                    f.pad(&format!("<class {}>", *c.name))
                }
            }
            Value::Instance(instance) => f.pad(&format!(
                "<{} instance>",
                *(*instance.class).as_class().name
            )),
            Value::BoundMethod(method) => {
                if config::STD_MODE.load() {
                    f.pad(&format!("{}", *method.method))
                } else {
                    f.pad(&format!(
                        "<bound method {}.{} of {}>",
                        *method.receiver.as_instance().class.as_class().name,
                        *method.method.as_closure().function.name,
                        *method.receiver
                    ))
                }
            }
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

    pub fn as_class(&self) -> &Class {
        match self {
            Value::Class(c) => c,
            _ => unreachable!("Expected Class, found `{}`", self),
        }
    }

    pub fn as_class_mut(&mut self) -> &mut Class {
        match self {
            Value::Class(c) => c,
            _ => unreachable!("Expected Class, found `{}`", self),
        }
    }

    pub fn as_instance(&self) -> &Instance {
        match self {
            Value::Instance(i) => i,
            _ => unreachable!("Expected Instance, found `{}`", self),
        }
    }

    pub fn as_instance_mut(&mut self) -> &mut Instance {
        match self {
            Value::Instance(i) => i,
            _ => unreachable!("Expected Instance, found `{}`", self),
        }
    }

    pub fn upvalue_location(&self) -> &Upvalue {
        match self {
            Value::Upvalue(v) => v,
            _ => unreachable!("Expected upvalue, found `{}`", self),
        }
    }

    pub fn upvalue_location_mut(&mut self) -> &mut Upvalue {
        match self {
            Value::Upvalue(v) => v,
            _ => unreachable!("Expected upvalue, found `{}`", self),
        }
    }
}

#[derive(Debug, PartialEq, PartialOrd, Clone)]
pub struct Function {
    pub arity: usize,
    pub chunk: Chunk,
    pub name: StringId,
    pub upvalue_count: usize,
}

impl std::fmt::Display for Function {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.pad(&format!("<fn {}>", *self.name))
    }
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

pub type NativeFunctionImpl = fn(&mut Heap, &[&ValueId]) -> Result<ValueId, String>;

fn always_equals<T>(_: &T, _: &T) -> bool {
    true
}

#[derive(Debug, PartialEq, Clone, Derivative)]
#[derivative(PartialOrd)]
pub struct Class {
    pub name: StringId,
    #[derivative(PartialOrd = "ignore")]
    pub methods: HashMap<StringId, ValueId>,
}

impl Class {
    #[must_use]
    pub fn new(name: StringId) -> Self {
        Class {
            name,
            methods: HashMap::new(),
        }
    }
}

#[derive(Derivative)]
#[derivative(Debug, PartialEq, PartialOrd, Clone)]
pub struct Instance {
    pub class: ValueId,
    #[derivative(PartialOrd = "ignore")]
    pub fields: HashMap<String, ValueId>,
}

impl Instance {
    #[must_use]
    pub fn new(class: ValueId) -> Self {
        Instance {
            class,
            fields: HashMap::new(),
        }
    }
}

#[derive(Debug, PartialEq, PartialOrd, Clone)]
pub struct BoundMethod {
    pub receiver: ValueId,
    pub method: ValueId,
}
