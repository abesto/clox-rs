use std::{
    ops::{Deref, DerefMut},
    pin::Pin,
    ptr::NonNull,
};

use derivative::Derivative;
use slotmap::{new_key_type, HopSlotMap as SlotMap, Key};
use std::fmt::{Debug, Display};

use crate::value::{Function, Upvalue, Value};

pub trait ArenaValue: Debug + Display + PartialEq {}
impl<T> ArenaValue for T where T: Debug + Display + PartialEq {}

new_key_type! {
    pub struct ValueKey;
    pub struct FunctionKey;
    pub struct StringKey;
}

#[derive(Clone, Debug, PartialOrd, Derivative)]
#[derivative(Hash, PartialEq, Eq)]
pub struct ArenaId<K: Key, T: ArenaValue> {
    id: K,
    #[derivative(Hash = "ignore")]
    arena: NonNull<Arena<K, T>>, // Yes this is terrible, yes I'm OK with it for this project
}

impl<K: Key, T: ArenaValue + Clone> Copy for ArenaId<K, T> {}

impl<K: Key, T: ArenaValue> Deref for ArenaId<K, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &self.arena.as_ref()[self] }
    }
}

impl<K: Key, T: ArenaValue> DerefMut for ArenaId<K, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut self.arena.as_mut()[self as &Self] }
    }
}

impl<K: Key, T: ArenaValue> ArenaId<K, T> {
    pub fn marked(&self, black_value: bool) -> bool {
        unsafe { self.arena.as_ref().is_marked(self.id, black_value) }
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

pub type ValueId = ArenaId<ValueKey, Value>;
pub type StringId = ArenaId<StringKey, String>;
pub type FunctionId = ArenaId<FunctionKey, Function>;

#[derive(Clone, Debug)]
pub struct Arena<K: Key, V: ArenaValue> {
    name: &'static str,
    log_gc: bool,

    data: SlotMap<K, Item<V>>,
    bytes_allocated: usize,

    gray: Vec<K>,
}

impl<K: Key, V: ArenaValue> Arena<K, V> {
    #[must_use]
    fn new(name: &'static str, log_gc: bool) -> Self {
        Self {
            name,
            log_gc,
            data: SlotMap::with_key(),
            bytes_allocated: 0,
            gray: Vec::new(),
        }
    }

    pub fn add(&mut self, value: V) -> ArenaId<K, V> {
        let id = self.data.insert(value.into());
        self.bytes_allocated += std::mem::size_of::<V>();

        if self.log_gc {
            eprintln!(
                "{}/{:?} allocate {} for {}",
                self.name,
                id,
                humansize::format_size(std::mem::size_of::<V>(), humansize::BINARY),
                self.data[id].item
            );
        }

        ArenaId {
            id,
            arena: (&mut *self).into(),
        }
    }

    fn is_marked(&self, index: K, black_value: bool) -> bool {
        self.data[index].marked == black_value
    }

    fn set_marked(&mut self, index: K, marked: bool) {
        self.data[index].marked = marked;
    }

    fn flush_gray(&mut self) -> Vec<K> {
        let capacity = self.gray.capacity();
        std::mem::replace(&mut self.gray, Vec::with_capacity(capacity))
    }

    pub fn mark(&mut self, index: &ArenaId<K, V>, black_value: bool) -> bool {
        debug_assert_eq!(index.arena.as_ptr().cast_const(), self);
        self.mark_raw(index.id, black_value)
    }

    fn mark_raw(&mut self, index: K, black_value: bool) -> bool {
        if self.is_marked(index, black_value) {
            return false;
        }
        if self.log_gc {
            eprintln!("{}/{:?} mark {}", self.name, index, self[index]);
        }
        self.set_marked(index, black_value);
        self.gray.push(index);
        true
    }

    fn sweep(&mut self, black_value: bool) {
        self.data.retain(|key, value| {
            let retain = value.marked == black_value;
            if !retain && self.log_gc {
                eprintln!("{}/{:?} free {}", self.name, key, value.item);
            }
            retain
        });
        self.bytes_allocated = std::mem::size_of::<V>() * self.data.len();
    }

    fn bytes_allocated(&self) -> usize {
        self.bytes_allocated
    }
}

impl<K: Key, V: ArenaValue> std::ops::Index<&ArenaId<K, V>> for Arena<K, V> {
    type Output = V;

    fn index(&self, index: &ArenaId<K, V>) -> &Self::Output {
        debug_assert_eq!(index.arena.as_ptr().cast_const(), self);
        &self[index.id]
    }
}

impl<K: Key, V: ArenaValue> std::ops::Index<K> for Arena<K, V> {
    type Output = V;

    fn index(&self, index: K) -> &Self::Output {
        &self.data[index].item
    }
}

impl<K: Key, V: ArenaValue> std::ops::IndexMut<&ArenaId<K, V>> for Arena<K, V> {
    fn index_mut(&mut self, index: &ArenaId<K, V>) -> &mut Self::Output {
        debug_assert_eq!(index.arena.as_ptr().cast_const(), self);
        &mut self[index.id]
    }
}

impl<K: Key, V: ArenaValue> std::ops::IndexMut<K> for Arena<K, V> {
    fn index_mut(&mut self, index: K) -> &mut Self::Output {
        &mut self.data[index].item
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BuiltinConstants {
    pub nil: ValueId,
    pub true_: ValueId,
    pub false_: ValueId,
    pub init_string: StringId,
    pub numbers: Vec<ValueId>,
}

impl BuiltinConstants {
    #[must_use]
    pub fn new(heap: &mut Heap) -> Self {
        Self {
            nil: heap.values.add(Value::Nil),
            true_: heap.values.add(Value::Bool(true)),
            false_: heap.values.add(Value::Bool(false)),
            init_string: heap.strings.add("init".to_string()),
            numbers: (0..1024)
                .map(|n| heap.values.add(Value::Number(n.into())))
                .collect(),
        }
    }

    pub fn bool(&self, val: bool) -> ValueId {
        if val {
            self.true_
        } else {
            self.false_
        }
    }

    pub fn number(&self, n: f64) -> Option<ValueId> {
        if n.fract() != 0.0 || n.is_nan() || n.is_infinite() {
            None
        } else {
            self.numbers.get(n as usize).copied()
        }
    }
}

#[derive(Clone, Debug)]
pub struct Heap {
    builtin_constants: Option<BuiltinConstants>,

    pub strings: Arena<StringKey, String>,
    pub values: Arena<ValueKey, Value>,
    pub functions: Arena<FunctionKey, Function>,

    log_gc: bool,
    next_gc: usize,
    pub black_value: bool,
}

impl Heap {
    pub fn new() -> Pin<Box<Self>> {
        let log_gc = crate::config::LOG_GC.load();

        let mut heap = Box::pin(Self {
            builtin_constants: None,

            strings: Arena::new("String", log_gc),
            values: Arena::new("Value", log_gc),
            functions: Arena::new("Function", log_gc),

            log_gc,
            next_gc: 1024 * 1024,
            black_value: true,
        });

        // Very important: first pin, *then* initialize the constants, as the `ArenaId`s generated
        // here will carry a raw pointer that needs to remain valid
        heap.builtin_constants = Some(BuiltinConstants::new(&mut heap));

        heap
    }

    pub fn builtin_constants(&self) -> &BuiltinConstants {
        self.builtin_constants.as_ref().unwrap()
    }

    fn bytes_allocated(&self) -> usize {
        self.values.bytes_allocated()
            + self.strings.bytes_allocated()
            + self.functions.bytes_allocated()
    }

    pub fn needs_gc(&self) -> bool {
        self.bytes_allocated() > self.next_gc
    }

    pub fn gc_start(&mut self) {
        if self.log_gc {
            eprintln!("-- gc begin");
        }

        self.values
            .mark(&self.builtin_constants().nil.clone(), self.black_value);
        self.values
            .mark(&self.builtin_constants().true_.clone(), self.black_value);
        self.values
            .mark(&self.builtin_constants().false_.clone(), self.black_value);
        self.strings.mark(
            &self.builtin_constants().init_string.clone(),
            self.black_value,
        );
        for number in self.builtin_constants().numbers.clone() {
            self.values.mark(&number, self.black_value);
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

    fn blacken_value(&mut self, index: ValueKey) {
        if self.log_gc {
            eprintln!("Value/{:?} blacken {}", index, self.values[index]);
        }

        self.values.mark_raw(index, self.black_value);
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
            Value::Class(c) => {
                self.strings.gray.push(c.name.id);
                let method_ids = c
                    .methods
                    .iter()
                    .map(|(n, c)| (n.id, c.id))
                    .collect::<Vec<_>>();
                for (method_name, closure) in method_ids {
                    self.strings.gray.push(method_name);
                    self.values.gray.push(closure);
                }
            }
            Value::Instance(instance) => {
                let mut fields = instance.fields.values().map(|value| value.id).collect();
                let class_id = instance.class.id;
                self.values.gray.append(&mut fields);
                self.values.gray.push(class_id);
            }
            Value::BoundMethod(bound_method) => {
                let receiver_id = bound_method.receiver.id;
                let method_id = bound_method.method.id;
                self.values.gray.push(receiver_id);
                self.values.gray.push(method_id);
            }
        }
    }

    fn blacken_string(&mut self, index: StringKey) {
        if self.log_gc {
            eprintln!("String/{:?} blacken {}", index, self.strings[index]);
        }
        self.strings.mark_raw(index, self.black_value);
    }

    fn blacken_function(&mut self, index: FunctionKey) {
        if self.log_gc {
            eprintln!("Function/{:?} blacken {}", index, self.functions[index]);
        }
        let function = &self.functions[index];
        self.strings.gray.push(function.name.id);
        for constant in function.chunk.constants() {
            self.values.gray.push(constant.id);
        }
        self.functions.mark_raw(index, self.black_value);
    }

    pub fn sweep(&mut self) {
        if self.log_gc {
            eprintln!("-- sweep start");
        }

        let before = self.bytes_allocated();
        self.values.sweep(self.black_value);
        self.functions.sweep(self.black_value);
        self.strings.sweep(self.black_value);
        self.black_value = !self.black_value;

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
