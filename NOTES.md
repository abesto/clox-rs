Things that were hard, and particularly things where I deviate from `clox` proper.

* Not implementing a custom dynamic array type; let's use `Vec`. I somewhat expect to run into limitations with that sooner or later as the internals of the array type are exposed in the book, but let's see if that actually happens.
* `Chunk`: can't tell if `uint8_t*` is supposed to be *only* `OpCode`s or other stuff too and this will trip me up, but for now a `Vec<OpCode>` will do.
  * I suspect that it'd be neater to encode arguments inside `OpCode` variants, and I suspect we'll encode them as items in `Chunk` after an `OpCode`. I don't know enough about the design space or the (performance) trade-offs or the direction of the book yet to decide, so I'll just do what the book says.
* `debug.rs`: the `disassemble*` functions could be rephrased as `impl Debug for _`; let's see where this goes before doing that though