Things that were hard, and particularly things where I deviate from `clox` proper.

* Not implementing a custom dynamic array type; let's use `Vec`. I somewhat expect to run into limitations with that sooner or later as the internals of the array type are exposed in the book, but let's see if that actually happens.
* `Chunk` / `OpCode` memory layout: initially I wanted to use a `#[repr(C, u8)]` enum, with operators that have operands encoded as fields of the enum variant. Even in the simplest case, `std::mem::size_of` said that takes up 16 bytes. That is WAY too much, so we'll do the same kind of tight, manual packing that the C implementation uses.
* `debug.rs`: implemented the `disassemble*` functions as `impl Debug for Chunk`. An implication of this is that all `Chunk`s store a `name`, which is probably a good idea anyway.
  * `Debug` for specific instructions is implemented with a helper struct `InstructionDisassembler` that wraps a `Chunk` reference and an offset. This also allows fully consistent formatting from `Chunk::debug` and execution tracing.
  * `(int)(vm.ip - vm.chunk->code)` has no translation into Rust; instead the `VM::ip` is an iterator over `(code_offset, instruction)`.
* `#define`-controlled features translate to Cargo features

# Challenges

* `Chunk::lines` uses run-length encoding
* `OpCode::ConstantLong` / `OP_CONSTANT_LONG`: support for more than 256 constants

# Dependencies

* `num_enum`: More safely and conveniently convert between the `u8` of byte-code and `OpCode`s
* `shrinkwraprs`: We use `u8` / `usize` for a ton of different meanings. Would be good to not mix them up. This helps with that.
  * If used incorrectly it'll likely have a pretty bad performance impact, but: first make it correct, then make it fast.
  * It also leads to a fair bit of `.as_ref()` noise, but... maybe it's still worth it? Let's see.
* `thiserror`: High-quality errors