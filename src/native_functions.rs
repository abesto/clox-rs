use std::time::{SystemTime, UNIX_EPOCH};

use hashbrown::HashMap;

use crate::{
    arena::{Arena, StringId, ValueId},
    compiler::Compiler,
    value::Value,
    vm::VM,
};

fn clock_native(arena: &mut Arena, _args: &[&ValueId]) -> Result<Option<ValueId>, String> {
    Ok(Some(
        arena.add_value(Value::Number(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs_f64(),
        )),
    ))
}

fn sqrt_native(arena: &mut Arena, args: &[&ValueId]) -> Result<Option<ValueId>, String> {
    match &**args[0] {
        Value::Number(n) => Ok(Some(arena.add_value(n.sqrt().into()))),
        x => Err(format!("'sqrt' expected numeric argument, got: {}", *x)),
    }
}

fn getattr_native(arena: &mut Arena, args: &[&ValueId]) -> Result<Option<ValueId>, String> {
    match (arena.get_value(args[0]), arena.get_value(args[1])) {
        (Value::Instance(instance), Value::String(string_id)) => {
            Ok(instance.fields.get(arena.get_string(string_id)).copied())
        }
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

fn setattr_native(arena: &mut Arena, args: &[&ValueId]) -> Result<Option<ValueId>, String> {
    if let Value::String(string_id) = arena.get_value(args[1]) {
        let field = arena.get_string(string_id).clone();
        if let Value::Instance(instance) = arena.get_value_mut(args[0]) {
            instance.fields.insert(field, *args[2]);
            Ok(None)
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
            string_ids: HashMap::new(),
        }
    }

    pub fn create_names(&mut self, arena: &mut Arena) {
        for name in ["clock", "sqrt", "getattr", "setattr"] {
            let string_id = arena.add_string(name.to_string());
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
        vm.define_native(self.string_ids["setattr"], 3, setattr_native);
    }
}
