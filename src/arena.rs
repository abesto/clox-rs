use std::ops::{Deref, DerefMut};

use derivative::Derivative;

use crate::value::{Function, Value};

macro_rules! arena_id {
    ($t:ty) => {
        paste::paste! {
            #[derive(Clone, Copy, Debug, PartialOrd, Derivative)]
            #[derivative(Hash, PartialEq, Eq)]
            pub struct [<$t Id>] {
                id: usize,
                #[derivative(Hash = "ignore")]
                arena: *mut Arena, // Yes this is terrible, yes I'm OK with it for this project
            }

            impl Deref for [<$t Id>] {
                type Target = $t;

                fn deref(&self) -> &Self::Target {
                    unsafe { self.arena.as_ref().unwrap().[<get_ $t:snake:lower>](self) }
                }
            }

            impl DerefMut for [<$t Id>] {
                fn deref_mut(&mut self) -> &mut Self::Target {
                    unsafe { self.arena.as_mut().unwrap().[<get_ $t:snake:lower _mut>](self) }
                }
            }
        }
    };
}

arena_id!(String);
arena_id!(Value);
arena_id!(Function);

macro_rules! arena_methods {
    ($t:ty) => {
        paste::paste! {
            pub fn [<add_ $t:snake:lower>](&mut self, s: $t) -> [<$t Id>] {
                self.[<$t:snake:lower s>].push(s);
                [<$t Id>] {
                    id: self.[<$t:snake:lower s>].len() - 1,
                    arena: &mut *self,
                }
            }

            pub fn [<get_ $t:snake:lower>](&self, id: &[<$t Id>]) -> &$t {
                debug_assert_eq!(id.arena.cast_const(), self);
                &self.[<$t:snake:lower s>][id.id]
            }

            pub fn [<get_ $t:snake:lower _mut>](&mut self, id: &[<$t Id>]) -> &mut $t {
                debug_assert_eq!(id.arena.cast_const(), self);
                &mut self.[<$t:snake:lower s>][id.id]
            }
        }
    };
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd)]
pub struct Arena {
    strings: Vec<String>,
    values: Vec<Value>,
    functions: Vec<Function>,
}

impl Arena {
    pub fn new() -> Arena {
        Arena {
            strings: Vec::new(),
            values: Vec::new(),
            functions: Vec::new(),
        }
    }

    arena_methods!(String);
    arena_methods!(Value);
    arena_methods!(Function);
}
