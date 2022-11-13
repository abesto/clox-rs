use std::ops::{Deref, DerefMut};

use derivative::Derivative;
use hashbrown::HashMap;

use crate::value::{Function, Upvalue, Value};

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

            impl [<$t Id>] {
                #[allow(dead_code)]
                pub fn marked(&self) -> bool {
                    unsafe {
                        self.arena.as_ref().unwrap().[<$t:snake:lower s>].is_marked(self.id)
                    }
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
                let id = self.[<$t:snake:lower s>].add(s);
                if self.log_gc {
                    eprintln!("{}/{} allocate {} for {}",
                              stringify!($t),
                              id,
                              humansize::format_size(std::mem::size_of::<$t>(), humansize::BINARY),
                              self.[<$t:snake:lower s>][id]
                             );
                }

                self.bytes_allocated += std::mem::size_of::<$t>();

                [<$t Id>] {
                    id,
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

            fn [<sweep_ $t:snake:lower s>](&mut self) {
                self.[<$t:snake:lower s>].retain_mut(|id, value, marked| {
                    let retain = *marked;
                    if !retain && self.log_gc {
                        eprintln!("{}/{} free {}",
                                  stringify!($t),
                                  id,
                                  value
                                 );
                    }
                    if !retain {
                        self.bytes_allocated -= std::mem::size_of::<$t>();
                    }
                    *marked = false;
                    retain
                });
            }

            #[allow(dead_code)]
            pub fn [<mark_ $t:snake:lower>](&mut self, id: &[<$t Id>]) -> bool {
                debug_assert_eq!(id.arena.cast_const(), self);
                self.[<mark_ $t:snake:lower _raw>](id.id)
            }

            #[allow(dead_code)]
            pub fn [<mark_ $t:snake:lower _raw>](&mut self, id: usize) -> bool {
                if self.[<$t:snake:lower s>].is_marked(id) {
                    return false;
                }
                if self.log_gc {
                    eprintln!("{}/{} mark {}", stringify!($t), id, self.[<$t:snake:lower s>][id]);
                }
                self.[<$t:snake:lower s>].set_marked(id, true);
                self.[<gray_ $t:snake:lower s>].push(id);
                true
            }

            pub fn [<trace_ $t:snake:lower s>](&mut self) {
                while let Some(index) = self.[<gray_ $t:snake:lower s>].pop() {
                    self.[<blacken_ $t:snake:lower>](index);
                }
            }
        }
    };
}

#[derive(Clone, Debug, PartialEq)]
struct Item<T> {
    marked: bool,
    item: T,
}

impl<T> From<T> for Item<T> {
    fn from(item: T) -> Self {
        Self {
            item,
            marked: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Storage<V> {
    data: HashMap<usize, Item<V>>,
    free_keys: Vec<usize>,
}

impl<V: std::fmt::Debug + std::cmp::PartialEq> Storage<V> {
    #[must_use]
    fn new() -> Self {
        Self {
            data: HashMap::new(),
            free_keys: Vec::new(),
        }
    }

    fn add(&mut self, value: V) -> usize {
        let id = self.free_keys.pop().unwrap_or_else(|| self.data.len());
        let old = self.data.insert(id, value.into());
        debug_assert_eq!(None, old);
        id
    }

    fn retain_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(usize, &mut V, &mut bool) -> bool,
    {
        let mut to_remove = vec![];
        for (key, value) in self.data.iter_mut() {
            if !f(*key, &mut value.item, &mut value.marked) {
                to_remove.push(*key);
            }
        }

        for key in to_remove {
            self.data.remove(&key);
            self.free_keys.push(key);
        }
    }

    fn is_marked(&self, index: usize) -> bool {
        self.data[&index].marked
    }

    fn set_marked(&mut self, index: usize, marked: bool) {
        self.data.get_mut(&index).unwrap().marked = marked;
    }
}

impl<V> std::ops::Index<usize> for Storage<V> {
    type Output = V;

    fn index(&self, index: usize) -> &Self::Output {
        &self
            .data
            .get(&index)
            .expect(&format!("no entry found for key {}", index))
            .item
    }
}

impl<V> std::ops::IndexMut<usize> for Storage<V> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.data.get_mut(&index).unwrap().item
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Arena {
    strings: Storage<String>,
    values: Storage<Value>,
    functions: Storage<Function>,

    log_gc: bool,
    gray_strings: Vec<usize>,
    gray_values: Vec<usize>,
    gray_functions: Vec<usize>,

    bytes_allocated: usize,
    next_gc: usize,
}

impl Arena {
    pub fn new() -> Arena {
        Arena {
            strings: Storage::new(),
            values: Storage::new(),
            functions: Storage::new(),

            log_gc: crate::config::LOG_GC.load(),
            gray_strings: Vec::new(),
            gray_values: Vec::new(),
            gray_functions: Vec::new(),

            bytes_allocated: 0,
            next_gc: 1024 * 1024,
        }
    }

    arena_methods!(String);
    arena_methods!(Value);
    arena_methods!(Function);

    pub fn needs_gc(&self) -> bool {
        self.bytes_allocated > self.next_gc
    }

    pub fn gc_start(&self) {
        if self.log_gc {
            eprintln!("-- gc begin");
        }
    }

    pub fn trace(&mut self) {
        if self.log_gc {
            eprintln!("-- trace start");
        }
        while !self.gray_functions.is_empty()
            || !self.gray_strings.is_empty()
            || !self.gray_values.is_empty()
        {
            self.trace_values();
            self.trace_strings();
            self.trace_functions();
        }
    }

    fn blacken_value(&mut self, index: usize) {
        if self.log_gc {
            eprintln!("Value/{} blacken {}", index, self.values[index]);
        }

        self.mark_value_raw(index);
        match &self.values[index] {
            Value::Bool(_)
            | Value::Nil
            | Value::Number(_)
            | Value::NativeFunction(_)
            | Value::Upvalue(Upvalue::Open(_)) => {}
            Value::String(string_id) => self.gray_strings.push(string_id.id),
            Value::Function(function_id) => self.gray_functions.push(function_id.id),
            Value::Closure(closure) => {
                self.gray_functions.push(closure.function.id);
                for upvalue in &closure.upvalues {
                    self.gray_values.push(upvalue.id);
                }
            }
            Value::Upvalue(Upvalue::Closed(value_id)) => self.gray_values.push(value_id.id),
            Value::Class(c) => self.gray_strings.push(c.name.id),
            Value::Instance(instance) => {
                self.gray_values.push(instance.class.id);
                for value in instance.fields.values() {
                    self.gray_values.push(value.id);
                }
            }
        }
    }

    fn blacken_string(&mut self, index: usize) {
        if self.log_gc {
            eprintln!("String/{} blacken {}", index, self.strings[index]);
        }
        self.mark_string_raw(index);
    }

    fn blacken_function(&mut self, index: usize) {
        if self.log_gc {
            eprintln!("Function/{} blacken {}", index, self.functions[index]);
        }
        let function = &self.functions[index];
        self.gray_strings.push(function.name.id);
        for constant in function.chunk.constants() {
            self.gray_values.push(constant.id);
        }
        self.mark_function_raw(index);
    }

    pub fn sweep(&mut self) {
        if self.log_gc {
            eprintln!("-- sweep start");
        }

        let before = self.bytes_allocated;
        self.sweep_values();
        self.sweep_functions();
        self.sweep_strings();

        self.next_gc = self.bytes_allocated * crate::config::GC_HEAP_GROW_FACTOR;
        if self.log_gc {
            eprintln!("-- gc end");
            eprintln!(
                "   collected {} (from {} to {}) next at {}",
                humansize::format_size(before - self.bytes_allocated, humansize::BINARY),
                humansize::format_size(before, humansize::BINARY),
                humansize::format_size(self.bytes_allocated, humansize::BINARY),
                humansize::format_size(self.next_gc, humansize::BINARY),
            );
        }
    }
}
