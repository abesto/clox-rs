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
* `#define`-controlled features initially translated to Cargo features; after the third one I switched them to command-line arguments to simplify my life and save time on recompiling. The values are stored in global atomic bools in `config.rs`.
  * A notable flag is `--std` that disables all non-standard behavior; in this mode, `clox-rs` passes the original `clox` test suite.
  * Doing anything at runtime instead of compile time obviously has some overhead. For this toy project, I'll take the added convenience of not having to recompile, as long as profiling doesn't say checking one of these flags adds significant overhead. (Yes, anything that reads one of the flags on a hot path caches the value so it doesn't trigger an atomic read each time)
* `VM::binary_op` is a higher-order function instead of a macro; hopefully this will be good enough later on.
* Unlike [`jlox-rs`](https://github.com/abesto/jlox-rs/), error reporting on initial implementation follows closely the error reporting logic of the book so that I have less to mentally juggle. Might end up refactoring afterwards to use `Result`s.
* `Scanner`: the `start` / `current` pointer pair is implemented with indices and slices. Using iterators *may* be more performant, and there may be a way to do that, but timed out on it for now.
* The Pratt parser table creation is a bit of a mess due to 1. constraints on array initialization and 2. lifetimes. We create a new instance of the table for each `Compiler` instance, because the compiler instance has a lifetime, and its associated methods capture that lifetime, and so the function references must also capture that same lifetime.
* There's a lot of `self.previous.as_ref().unwrap()` in `Compiler`. There should be a way to get rid of those `Option`s. I think.
* By chapter 19, the book is making a lot of forward references to a garbage collector. While that sounds exciting, I think... we won't... need it? Because the Rust memory management structures we use (basically, `Box` / `String` + `Drop`) ensure we never leak memory? Except, this will probably get a ton more complicated the moment we start in on variables and classes. Let's see what happens.
* Completely skipped chapter 20 (Hash Tables) because we have them in Rust (also mostly skipped the part where we rebuild `Box`).
* `StackFrame::function` is a pointer to an `ObjFunction` in C. In Rust, we can't "just" stuff in a pointer. At a first approximation, I'll use `Rc<RefCell>` for storing `Function` instances. I considered `Weak` for storage in `CallFrame`s, but it doesn't really give any performance improvements I think (they need to be `updrade`ed when used *anyway*).
* The book handles the stack of compilers with an explicit linked list, and lets globals implicitly take care of shared state. This obviously doesn't translate directly to Rust, so:
  * Initially I used the Rust call stack to handle the compiler stack, and explicitly copied the "shared" state into / out of the sub-compiler. This breaks down at closures, where the nested compiler needs to access some of the state of the enclosing compiler.
  * Then I tried representing the shared state in an `Rc<RefCell<SharedCompilerState>>`, and nesting compiler instances that carry their own "private" state. This still doesn't actually provide a solution to "how do nested compilers access their enclosing compiler".
  * Finally: I completely dropped the idea of a stack of compiler instances. I have just the one compiler, with a stack of nestable *states* managed explicitly.
* The book stores the list of open `Upvalue`s in a poor man's linked list. Rust has a `LinkedList` in its stdlib, but it doesn't expose a way to insert an item in the middle in O(1) time given a pointer at an item. <https://github.com/rust-lang/rust/issues/58533> tracks adding a `Cursor` API that would enable this. The book also makes an argument that this is not *quite* performance critical. Instead of trying to implement a half-assed linked list in Rust (which is known to be hard), I'll just throw a `VecDeque` at this. We'll store `ValueId`s, so we get pointer-like semantics in that we'll point at the same single instance of the value (i.e. variable vs value).
* The book stores closed `Upvalue`s with some neat pointer trickery. We can't follow there; instead, `Upvalue` is now an enum with an `Open(usize)` and a `Closed(ValueId)` variant.
* The book makes GC decisions (at least of the stress-testing kind) whenever memory is allocated. Our direct translation would be the `Arena::add_*` methods, but lifetimes make injecting roots there tricky. Instead in `clox-rs` GC is (potentially) triggered between the execution of each instruction.
* The initial `Arena` implementation used `Vec`s as the backing store. This falls apart at GC: the "smart pointers" (e.g. `ValueId`) carry around an index into the `Vec`, but GC compresses the `Vec`, and so all smart pointers become invalid. There's probably a smart and efficient way around this. Instead of figuring that out, I switched the backing store to a `HashMap`, plus a storage for free ids (i.e. ones that have been removed before and can now be reused). I later replaced the `HashMap` + free id store with `slotmap::HopSlotMap` for optimization (see below)
* Printing of some values like instances and bound methods when NOT running with `--std` is more similar to Python than to Lox (more informative).

# TODO

* Drop the VM stack after we're done interpreting a piece of code. In the REPL, stuff can stay there after runtime errors.
* Possible optimization: copy-on-write for `Value`s stored in the `Arena`

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
* 25/2: alias loop variables for the loop body
* 26/2: skip flipping `marked` bit on each object at GC end
* 27/1: reading an undefined field returns `nil`
* 27/2: `getattr` and `setattr` native functions to access instance fields using a variable as the index
  * Until this point, `Instance::fields` was indexed with a `StringId`. This is fast, and it's OK because only constant strings were usable to access fields, and constant strings are deduplicated in the compiler. Now that field indexes can be constructed at runtime, `Instance::fields` has to index using `String`s. This is slower, but hey, features!
* 27/3: `delattr`. Also added `hasattr` to help testing.
* MAYBE: generational GC
* MAYBE: ternary operator (not super interesting)
* STRETCH: add error handling to user code

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

## Results / Optimization Steps

* End of Chapter 24
  * vs [`jlox-rs`](https://github.com/abesto/jlox-rs): 1.79 ± 0.02 times faster 
  * vs `clox` proper: 8.14 ± 0.11 times slower
* After a basic optimization pass
  * vs [`jlox-rs`](https://github.com/abesto/jlox-rs): 4.39 ± 0.07 times faster
  * vs `clox` proper: 3.32 ± 0.07 times slower
    * Two biggest offenders seems like `Value::clone` (called a lot when reading globals) and `core::ptr::drop_in_place` (executed a lot inside `VM::add` on `*stack_item = (stack_item.as_f64() + *b).into();` for some reason)
* After switching memory management to an `Arena`: 4.59 ± 0.18 times slower than `clox`
  * Most of the (new) time is spent in `Vec::push` in `Arena::add_value`. CoW would probably help with this a lot.

* End of Chapter 28
  * Performance is really starting to suffer now from the differences in `clox` and `clox-rs` memory management. We're down to being 18.28 ± 0.48 slower, with most of the time being spent in looking up heap values and in GC.
  * Switching the heap to use `HopSlotMap` instead of `HashMap` for data storage gives a significant speed-up, now "only" 11.31 ± 0.44 times slower than `clox`.
  * Further benchmarking and optimizations got it down to 8.38 ± 0.47 times slower than `clox`.
    * The main change was caching the current function / closure in `VM` instead of looking it up from the last call frame on each `read_byte()` call.
  * Interestingly, using specialized key types with `slotmap::new_key_type` further increased performance, now to 6.78 ± 0.42 times slower than `clox`.
  * Replacing `slotmap::HopSlotMap` with any of the other `SlotMap` flavors, or `slab::Slab`, or `generational_arena::Arena` significantly decreased performance in this benchmark.
  * Using built-in constants for `true`, `false`, `nil`, and integers 0-1024 gives us a further speedup to 4.74 ± 0.14 times slower than `clox`, since we save a ton of time not doing GC on these values. This is on par with the performance before GC. It's also cheating as this is an optimization technique not used in `clox`, but hey, cheating is technique.
  * Switching from `hashbrown` to `rustc_hash` provides a small speedup, to now 4.15 ± 0.13 times slower than `clox`.