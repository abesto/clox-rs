use std::ops::{Deref, DerefMut};

use derivative::Derivative;

use crate::value::Value;

#[derive(Clone, Copy, Debug, PartialOrd, Derivative)]
#[derivative(Hash, PartialEq, Eq)]
pub struct StringId {
    id: usize,
    #[derivative(Hash = "ignore")]
    arena: *const Arena, // Yes this is terrible, yes I'm OK with it for this project
}

impl Deref for StringId {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        unsafe { self.arena.as_ref().unwrap().get_string(self) }
    }
}

#[derive(Clone, Copy, Debug, PartialOrd, Derivative)]
#[derivative(Hash, PartialEq, Eq)]
pub struct ValueId {
    id: usize,
    #[derivative(Hash = "ignore")]
    arena: *mut Arena, // Yes this is terrible, yes I'm OK with it for this project
}

impl Deref for ValueId {
    type Target = Value;

    fn deref(&self) -> &Self::Target {
        unsafe { self.arena.as_ref().unwrap().get_value(self) }
    }
}

impl DerefMut for ValueId {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.arena.as_mut().unwrap().get_value_mut(self) }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd)]
pub struct Arena {
    strings: Vec<String>,
    values: Vec<Value>,
}

impl Arena {
    pub fn new() -> Arena {
        Arena {
            strings: Vec::new(),
            values: Vec::new(),
        }
    }

    pub fn add_string(&mut self, s: String) -> StringId {
        self.strings.push(s);
        StringId {
            id: self.strings.len() - 1,
            arena: &*self,
        }
    }

    pub fn get_string(&self, id: &StringId) -> &String {
        debug_assert_eq!(id.arena, self);
        &self.strings[id.id]
    }

    pub fn add_value(&mut self, v: Value) -> ValueId {
        self.values.push(v);
        ValueId {
            id: self.values.len() - 1,
            arena: &mut *self,
        }
    }

    pub fn get_value(&self, id: &ValueId) -> &Value {
        debug_assert_eq!(id.arena.cast_const(), self);
        &self.values[id.id]
    }

    pub fn get_value_mut(&mut self, id: &ValueId) -> &mut Value {
        debug_assert_eq!(id.arena.cast_const(), self);
        &mut self.values[id.id]
    }
}
