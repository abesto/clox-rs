use std::ops::{Deref, DerefMut};

use derivative::Derivative;
use hashbrown::HashMap;
use std::fmt::{Debug, Display};

use crate::value::{Function, Upvalue, Value};

pub trait ArenaValue: Debug + Display + PartialEq {}
impl<T> ArenaValue for T where T: Debug + Display + PartialEq {}

#[derive(Clone, Debug, PartialOrd, Derivative)]
#[derivative(Hash, PartialEq, Eq)]
pub struct ArenaId<T: ArenaValue> {
    id: usize,
    #[derivative(Hash = "ignore")]
    arena: *mut Arena<T>, // Yes this is terrible, yes I'm OK with it for this project
}

impl<T: ArenaValue + Clone> Copy for ArenaId<T> {}

impl<T: ArenaValue> Deref for ArenaId<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &self.arena.as_ref().unwrap()[self] }
    }
}

impl<T: ArenaValue> DerefMut for ArenaId<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut self.arena.as_mut().unwrap()[self as &Self] }
    }
}

impl<T: ArenaValue> ArenaId<T> {
    pub fn marked(&self) -> bool {
        unsafe { self.arena.as_ref().unwrap().is_marked(self.id) }
    }
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

pub type ValueId = ArenaId<Value>;
pub type StringId = ArenaId<String>;
pub type FunctionId = ArenaId<Function>;

#[derive(Clone, Debug, PartialEq)]
pub struct Arena<V: ArenaValue> {
    name: &'static str,
    log_gc: bool,

    bytes_allocated: usize,
    data: HashMap<usize, Item<V>>,
    free_keys: Vec<usize>,

    gray: Vec<usize>,
}

impl<V: ArenaValue> Arena<V> {
    #[must_use]
    fn new(name: &'static str, log_gc: bool) -> Self {
        Self {
            name,
            log_gc,
            bytes_allocated: 0,
            data: HashMap::new(),
            free_keys: Vec::new(),
            gray: Vec::new(),
        }
    }

    pub fn add(&mut self, value: V) -> ArenaId<V> {
        let id = self.free_keys.pop().unwrap_or_else(|| self.data.len());
        let old = self.data.insert(id, value.into());
        debug_assert_eq!(None, old);

        if self.log_gc {
            eprintln!(
                "{}/{} allocate {} for {}",
                self.name,
                id,
                humansize::format_size(std::mem::size_of::<V>(), humansize::BINARY),
                self.data[&id].item
            );
        }

        self.bytes_allocated += std::mem::size_of::<V>();

        ArenaId {
            id,
            arena: &mut *self,
        }
    }

    fn is_marked(&self, index: usize) -> bool {
        self.data[&index].marked
    }

    fn set_marked(&mut self, index: usize, marked: bool) {
        self.data.get_mut(&index).unwrap().marked = marked;
    }

    fn flush_gray(&mut self) -> Vec<usize> {
        std::mem::take(&mut self.gray)
    }

    pub fn mark(&mut self, index: &ArenaId<V>) -> bool {
        debug_assert_eq!(index.arena.cast_const(), self);
        self.mark_raw(index.id)
    }

    fn mark_raw(&mut self, index: usize) -> bool {
        if self.is_marked(index) {
            return false;
        }
        if self.log_gc {
            eprintln!("{}/{} mark {}", self.name, index, self[index]);
        }
        self.set_marked(index, true);
        self.gray.push(index);
        true
    }

    fn sweep(&mut self) {
        let mut to_remove = vec![];
        for (key, value) in self.data.iter_mut() {
            let retain = value.marked;
            if !retain && self.log_gc {
                eprintln!("{}/{} free {}", self.name, key, value.item);
            }
            if !retain {
                self.bytes_allocated -= std::mem::size_of::<V>();
            }
            value.marked = false;
            if !retain {
                to_remove.push(*key);
            }
        }

        for key in to_remove {
            self.data.remove(&key);
            self.free_keys.push(key);
        }
    }
}

impl<V: ArenaValue> std::ops::Index<&ArenaId<V>> for Arena<V> {
    type Output = V;

    fn index(&self, index: &ArenaId<V>) -> &Self::Output {
        debug_assert_eq!(index.arena.cast_const(), self);
        &self[index.id]
    }
}

impl<V: ArenaValue> std::ops::Index<usize> for Arena<V> {
    type Output = V;

    fn index(&self, index: usize) -> &Self::Output {
        &self.data[&index].item
    }
}

impl<V: ArenaValue> std::ops::IndexMut<&ArenaId<V>> for Arena<V> {
    fn index_mut(&mut self, index: &ArenaId<V>) -> &mut Self::Output {
        debug_assert_eq!(index.arena.cast_const(), self);
        &mut self[index.id]
    }
}

impl<V: ArenaValue> std::ops::IndexMut<usize> for Arena<V> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.data.get_mut(&index).unwrap().item
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Heap {
    pub strings: Arena<String>,
    pub values: Arena<Value>,
    pub functions: Arena<Function>,

    log_gc: bool,
    next_gc: usize,
}

impl Heap {
    pub fn new() -> Self {
        let log_gc = crate::config::LOG_GC.load();
        Self {
            strings: Arena::new("String", log_gc),
            values: Arena::new("Value", log_gc),
            functions: Arena::new("Function", log_gc),

            log_gc,
            next_gc: 1024 * 1024,
        }
    }

    fn bytes_allocated(&self) -> usize {
        self.values.bytes_allocated + self.strings.bytes_allocated + self.functions.bytes_allocated
    }

    pub fn needs_gc(&self) -> bool {
        self.bytes_allocated() > self.next_gc
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
        while !self.functions.gray.is_empty()
            || !self.strings.gray.is_empty()
            || !self.values.gray.is_empty()
        {
            for index in self.values.flush_gray() {
                self.blacken_value(index);
            }
            for index in self.strings.flush_gray() {
                self.blacken_string(index);
            }
            for index in self.functions.flush_gray() {
                self.blacken_function(index);
            }
        }
    }

    fn blacken_value(&mut self, index: usize) {
        if self.log_gc {
            eprintln!("Value/{} blacken {}", index, self.values[index]);
        }

        self.values.mark_raw(index);
        match &self.values[index] {
            Value::Bool(_)
            | Value::Nil
            | Value::Number(_)
            | Value::NativeFunction(_)
            | Value::Upvalue(Upvalue::Open(_)) => {}
            Value::String(string_id) => self.strings.gray.push(string_id.id),
            Value::Function(function_id) => self.functions.gray.push(function_id.id),
            Value::Closure(closure) => {
                self.functions.gray.push(closure.function.id);
                self.values
                    .gray
                    .append(&mut closure.upvalues.iter().map(|uv| uv.id).collect());
            }
            Value::Upvalue(Upvalue::Closed(value_id)) => self.values.gray.push(value_id.id),
            Value::Class(c) => self.strings.gray.push(c.name.id),
            Value::Instance(instance) => {
                let mut fields = instance.fields.values().map(|value| value.id).collect();
                let class_id = instance.class.id;
                self.values.gray.append(&mut fields);
                self.values.gray.push(class_id);
            }
        }
    }

    fn blacken_string(&mut self, index: usize) {
        if self.log_gc {
            eprintln!("String/{} blacken {}", index, self.strings[index]);
        }
        self.strings.mark_raw(index);
    }

    fn blacken_function(&mut self, index: usize) {
        if self.log_gc {
            eprintln!("Function/{} blacken {}", index, self.functions[index]);
        }
        let function = &self.functions[index];
        self.strings.gray.push(function.name.id);
        for constant in function.chunk.constants() {
            self.values.gray.push(constant.id);
        }
        self.functions.mark_raw(index);
    }

    pub fn sweep(&mut self) {
        if self.log_gc {
            eprintln!("-- sweep start");
        }

        let before = self.bytes_allocated();
        self.values.sweep();
        self.functions.sweep();
        self.strings.sweep();

        self.next_gc = self.bytes_allocated() * crate::config::GC_HEAP_GROW_FACTOR;
        if self.log_gc {
            eprintln!("-- gc end");
            eprintln!(
                "   collected {} (from {} to {}) next at {}",
                humansize::format_size(before - self.bytes_allocated(), humansize::BINARY),
                humansize::format_size(before, humansize::BINARY),
                humansize::format_size(self.bytes_allocated(), humansize::BINARY),
                humansize::format_size(self.next_gc, humansize::BINARY),
            );
        }
    }
}