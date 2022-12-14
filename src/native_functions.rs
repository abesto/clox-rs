use std::time::{SystemTime, UNIX_EPOCH};

use rustc_hash::FxHashMap as HashMap;

use crate::{
    compiler::Compiler,
    heap::{Heap, StringId, ValueId},
    value::Value,
    vm::VM,
};

fn clock_native(heap: &mut Heap, _args: &[&ValueId]) -> Result<ValueId, String> {
    Ok(heap.add_value(Value::Number(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs_f64(),
    )))
}

fn sqrt_native(heap: &mut Heap, args: &[&ValueId]) -> Result<ValueId, String> {
    match &heap.values[args[0]] {
        Value::Number(n) => Ok(heap.add_value(n.sqrt().into())),
        x => Err(format!("'sqrt' expected numeric argument, got: {}", *x)),
    }
}

fn getattr_native(heap: &mut Heap, args: &[&ValueId]) -> Result<ValueId, String> {
    match (&heap.values[args[0]], &heap.values[args[1]]) {
        (Value::Instance(instance), Value::String(string_id)) => Ok(instance
            .fields
            .get(&heap.strings[string_id])
            .cloned()
            .unwrap_or(heap.builtin_constants().nil)),
        (instance @ Value::Instance(_), x) => Err(format!(
            "`getattr` can only index with string indexes, got: `{}` (instance: `{}`)",
            x, instance
        )),
        (not_instance, _) => Err(format!(
            "`getattr` only works on instances, got `{}`",
            not_instance
        )),
    }
}

fn hasattr_native(heap: &mut Heap, args: &[&ValueId]) -> Result<ValueId, String> {
    match (&heap.values[args[0]], &heap.values[args[1]]) {
        (Value::Instance(instance), Value::String(string_id)) => Ok(heap
            .builtin_constants()
            .bool(instance.fields.contains_key(&heap.strings[string_id]))),
        (instance @ Value::Instance(_), x) => Err(format!(
            "`hasattr` can only index with string indexes, got: `{}` (instance: `{}`)",
            x, instance
        )),
        (not_instance, _) => Err(format!(
            "`hasattr` only works on instances, got `{}`",
            not_instance
        )),
    }
}

fn delattr_native(heap: &mut Heap, args: &[&ValueId]) -> Result<ValueId, String> {
    if let Value::String(string_id) = &heap.values[args[1]] {
        let field = heap.strings[string_id].clone();
        if let Value::Instance(instance) = &mut heap.values[args[0]] {
            instance.fields.remove(&field);
            Ok(heap.builtin_constants().nil)
        } else {
            Err(format!(
                "`delattr` only works on instances, got `{}`",
                heap.values[args[0]]
            ))
        }
    } else {
        Err(format!(
            "`delattr` can only index with string indexes, got: `{}` (instance: `{}`)",
            **args[1], **args[0]
        ))
    }
}

fn setattr_native(heap: &mut Heap, args: &[&ValueId]) -> Result<ValueId, String> {
    if let Value::String(string_id) = &heap.values[args[1]] {
        let field = heap.strings[string_id].clone();
        if let Value::Instance(instance) = &mut heap.values[args[0]] {
            instance.fields.insert(field, *args[2]);
            Ok(heap.builtin_constants().nil)
        } else {
            Err(format!(
                "`setattr` only works on instances, got `{}`",
                **args[0]
            ))
        }
    } else {
        Err(format!(
            "`setattr` can only index with string indexes, got: `{}` (instance: `{}`)",
            **args[1], **args[0]
        ))
    }
}

pub struct NativeFunctions {
    string_ids: HashMap<String, StringId>,
}

impl NativeFunctions {
    #[must_use]
    pub fn new() -> Self {
        Self {
            string_ids: HashMap::default(),
        }
    }

    pub fn create_names(&mut self, heap: &mut Heap) {
        for name in ["clock", "sqrt", "getattr", "setattr", "hasattr", "delattr"] {
            let string_id = heap.add_string(name.to_string());
            self.string_ids.insert(name.to_string(), string_id);
        }
    }

    pub fn register_names(&mut self, compiler: &mut Compiler) {
        compiler.inject_strings(&self.string_ids);
    }

    pub fn define_functions(&self, vm: &mut VM) {
        vm.define_native(self.string_ids["clock"], 0, clock_native);
        vm.define_native(self.string_ids["sqrt"], 1, sqrt_native);
        vm.define_native(self.string_ids["getattr"], 2, getattr_native);
        vm.define_native(self.string_ids["hasattr"], 2, hasattr_native);
        vm.define_native(self.string_ids["delattr"], 2, delattr_native);
        vm.define_native(self.string_ids["setattr"], 3, setattr_native);
    }
}
