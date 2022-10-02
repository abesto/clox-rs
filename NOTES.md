Things that were hard, and particularly things where I deviate from `clox` proper.

* Not implementing a custom dynamic array type; let's use `Vec`. I somewhat expect to run into limitations with that sooner or later as the internals of the array type are exposed in the book, but let's see if that actually happens.
* `Chunk` / `OpCode` memory layout: we could play along and make `Chunk` store a `Vec<u8>` and cast instructions (`OpCodes`, etc) from/to `u8`. I'll instead take the extra safety and convenience from a `#[repr(C, u8)]` enum.
* `debug.rs`: implemented the `disassemble*` functions as `impl Debug for Chunk`. An implication of this is that all `Chunk`s store a `name`, which is probably a good idea anyway.
  * Breaking parts out into `impl Debug for Instruction` would need a way to push down (at least) `chunk.constants` into the `Instruction` implementation, which is problematic; I'll pay the price of "no `Debug` for `Instruction`" for some added simplicity.