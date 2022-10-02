Things that were hard, and particularly things where I deviate from `clox` proper.

* Not implementing a custom dynamic array type; let's use `Vec`. I somewhat expect to run into limitations with that sooner or later as the internals of the array type are exposed in the book, but let's see if that actually happens.
* `Chunk`: can't tell if `uint8_t*` is supposed to be *only* `OpCode`s or other stuff too and this will trip me up, but for now a `Vec<OpCode>` will do.