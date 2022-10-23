use std::time::{SystemTime, UNIX_EPOCH};

use hashbrown::HashMap;

use crate::{
    arena::{Arena, StringId},
    compiler::Compiler,
    value::Value,
    vm::VM,
};

fn clock_native(_args: &mut [Value]) -> Result<Value, String> {
    Ok(Value::Number(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs_f64(),
    ))
}

fn sqrt_native(args: &mut [Value]) -> Result<Value, String> {
    match args {
        [Value::Number(n)] => Ok(n.sqrt().into()),
        [x] => Err(format!("'sqrt' expected numeric argument, got: {}", x)),
        _ => unreachable!(),
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
        for name in ["clock", "sqrt"] {
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
    }
}
