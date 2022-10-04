#[derive(Debug, PartialEq, PartialOrd, Clone)]
pub enum Value {
    Bool(bool),
    Nil,
    Number(f64),
    #[allow(clippy::box_collection)]
    // `String` needs 2-3x`usize` to store, while `Box` needs only `usize`.
    // This means dropping the `Box` takes `Value` from 16 bytes to 32 bytes.
    // TODO: benchmark the performance cost of the extra indirection.
    String(Box<String>),
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
            Self::Bool(bool) => f.pad(&format!("{}", bool)),
            Self::Number(num) => f.pad(&format!("{}", num)),
            Self::Nil => f.pad("nil"),
            Self::String(s) => f.pad(&format!("{}", *s)),
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

#[cfg(test)]
#[test]
fn value_size() {
    assert_eq!(16, std::mem::size_of::<Value>());
}
