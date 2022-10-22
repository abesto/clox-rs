use std::ops::Deref;

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
        &self.strings[id.id]
    }
}
