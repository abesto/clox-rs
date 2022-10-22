Things that were hard, and particularly things where I deviate from `clox` proper.

* Not implementing a custom dynamic array type; let's use `Vec`. I somewhat expect to run into limitations with that sooner or later as the internals of the array type are exposed in the book, but let's see if that actually happens.
* `Chunk` / `OpCode` memory layout: initially I wanted to use a `#[repr(C, u8)]` enum, with operators that have operands encoded as fields of the enum variant. Even in the simplest case, `std::mem::size_of` said that takes up 16 bytes. That is WAY too much, so we'll do the same kind of tight, manual packing that the C implementation uses.
* `debug.rs`: implemented the `disassemble*` functions as `impl Debug for Chunk`. An implication of this is that all `Chunk`s store a `name`, which is probably a good idea anyway.
  * `Debug` for specific instructions is implemented with a helper struct `InstructionDisassembler` that wraps a `Chunk` reference and an offset. This also allows fully consistent formatting from `Chunk::debug` and execution tracing.
  * Serialization of the opcodes is exactly as seen in the C version, even though the specific strings don't actually map exactly to our code (i.e. `OP_NIL` instead of `OpCode::Nil`). This is to stay compatible with the books output; plus, I like the aesthetic!
* Translating stored pointers and pointer operations to Rust is always interesting:
  * `(int)(vm.ip - vm.chunk->code)` has no translation into Rust. I tried making `VM::ip` an iterator over `(code_offset, instruction)`, but that leads to lifetime nightmares. For now we go with a simple index here.
  * `Token`s store the "pointer" to the lexeme as a slice.
  * `CallFrame::slots` is a `Value*` in C; a direct translation would be a slice into `VM::stack`, but I can't easily sort out the lifetimes there. So: simple index again! I keep wondering about the performance.
* `#define`-controlled features translate to Cargo features
* `VM::binary_op` is a higher-order function instead of a macro; hopefully this will be good enough later on.
* Unlike [`jlox-rs`](https://github.com/abesto/jlox-rs/), error reporting on initial implementation follows closely the error reporting logic of the book so that I have less to mentally juggle. Might end up refactoring afterwards to use `Result`s.
* `Scanner`: the `start` / `current` pointer pair is implemented with indices and slices. Using iterators *may* be more performant, and there may be a way to do that, but timed out on it for now.
* The Pratt parser table creation is a bit of a mess due to 1. constraints on array initialization and 2. lifetimes. We create a new instance of the table for each `Compiler` instance, because the compiler instance has a lifetime, and its associated methods capture that lifetime, and so the function references must also capture that same lifetime.
* There's a lot of `self.previous.as_ref().unwrap()` in `Compiler`. There should be a way to get rid of those `Option`s. I think.
* By chapter 19, the book is making a lot of forward references to a garbage collector. While that sounds exciting, I think... we won't... need it? Because the Rust memory management structures we use (basically, `Box` / `String` + `Drop`) ensure we never leak memory? Except, this will probably get a ton more complicated the moment we start in on variables and classes. Let's see what happens.
* Completely skipped chapter 20 (Hash Tables) because we have them in Rust (also mostly skipped the part where we rebuild `Box`).
* `StackFrame::function` is a pointer to an `ObjFunction` in C. In Rust, we can't "just" stuff in a pointer. At a first approximation, I'll use `Rc<RefCell>` for storing `Function` instances. I considered `Weak` for storage in `CallFrame`s, but it doesn't really give any performance improvements I think (they need to be `updrade`ed when used *anyway*).
* The book handles the stack of compilers with an explicit linked list, and lets globals implicitly take care of shared state. Instead, we use the Rust call stack to handle the compiler stack, and explicitly fork / join shared state.
* Most `Obj*` things in the book map to a `Value` variant. `Closure` is an exception: `ObjFunction` maps to `Function`, and `ObjClosure` maps to `Value::Function`.

# TODO

* Drop the VM stack after we're done interpreting a piece of code. In the REPL, stuff can stay there after runtime errors.
* Clean up unused strings in `Arena` (GC?!)

# Challenges

* `Chunk::lines` uses run-length encoding
* `OpCode::ConstantLong` / `OP_CONSTANT_LONG`: support for more than 256 constants
  * Also added `OpCode::DefineGlobalLong`, `OpCode::GetGlobalLong`, `OpCode::SetGlobalLong`.
* Optimized negation to mutate the stack value in place, for about a 1.22x speedup. Also did the same for binary operations; strangely, addition (the only one I tested) only sped up by about 1.02x, if that (significant of noise on the measurement).
* 21/1: Don't add global name to constant table each time a global is accessed (name -> constant index hashtable in compiler)
* 22/3: `const` keyword marks variables immutable, can only be assigned in the declaration statement.
* 22/4: Allow more than 256 local variables in scope at a time.
* 23/1: `switch` statements.
* 23/3: `continue` statements. Made a naive implementation, got confused about scopes and missed some edge cases; ported the solution from the book repo.
* 24/1: arity check on native functions
* 24/3: native functions can report runtime errors
* TODO ternary operator
* STRETCH: add error handling to user code
* TODO add a `Value` variant that holds a reference to a string value kept alive somewhere else (Chapter 19)
  * Doing this with lifetimes seems (almost?) impossible: `VM` has both a `Chunk` and a stack, and its stack may have values from multiple chunks, so there's no good `a` for a `Value<'a>`. `Rc` is probably a sane solution to this, but I want to see what future memory management shenanigans we get up to before implementing this. This will probably end up as an arena once I get to GC.

# Dependencies

* `num_enum`: More safely and conveniently convert between the `u8` of byte-code and `OpCode`s
* `shrinkwraprs`: We use `u8` / `usize` for a ton of different meanings. Would be good to not mix them up. This helps with that. Currently only really used by `chunk.rs`.
  * If used incorrectly it'll likely have a pretty bad performance impact, but: first make it correct, then make it fast.
  * It also leads to a fair bit of `.as_ref()` noise, but... maybe it's still worth it? Let's see.

# Performance

## Methodology

`fib.lox`:

```
fun fib(n) {
    if (n < 2) return n;
    return fib(n - 1) + fib(n - 2);
}

print fib(30);
```

Example run:

```
abesto@localhost:~/clox-rs$ hyperfine --warmup 1 '../jlox-rs/target/release/jlox_rs ./fib.lox' './target/release/clox-rs ./fib.lox'
Benchmark 1: ../jlox-rs/target/release/jlox_rs ./fib.lox
  Time (mean ± σ):      2.001 s ±  0.016 s    [User: 1.996 s, System: 0.001 s]
  Range (min … max):    1.980 s …  2.022 s    10 runs
 
Benchmark 2: ./target/release/clox-rs ./fib.lox
  Time (mean ± σ):      1.119 s ±  0.011 s    [User: 1.115 s, System: 0.002 s]
  Range (min … max):    1.104 s …  1.141 s    10 runs
 
Summary
  './target/release/clox-rs ./fib.lox' ran
    1.79 ± 0.02 times faster than '../jlox-rs/target/release/jlox_rs ./fib.lox'
```

* End of Chapter 24
  * vs [`jlox-rs`](https://github.com/abesto/jlox-rs): 1.79 ± 0.02 times faster 
  * vs `clox` proper: 8.14 ± 0.11 times slower
* After a basic optimization pass
  * vs [`jlox-rs`](https://github.com/abesto/jlox-rs): 4.39 ± 0.07 times faster
  * vs `clox` proper: 3.32 ± 0.07 times slower
    * Two biggest offenders seems like `Value::clone` (called a lot when reading globals) and `core::ptr::drop_in_place` (executed a lot inside `VM::add` on `*stack_item = (stack_item.as_f64() + *b).into();` for some reason)