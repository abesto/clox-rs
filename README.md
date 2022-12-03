# clox-rs

> Following along the second (C) part of https://craftinginterpreters.com/, but with Rust, because reasons. 

Status: done!

In-browser version coming soon.

Check out [`NOTES.md`](NOTES.md) for a ton of details, especially around differences vs. `clox` as a result of using Rust.

## Performance

On `fib(30)`, about 4x slower than `clox`. Most of the overhead comes from explicit (safe) memory management using an arena, as opposed to the manual memory management of `clox`. See [`NOTES.md`](NOTES.md) for lots of measurments and a breakdown of various optimizations applied.

## Correctness

* Running with `--std`, `clox-rs` passes the complete test-suite of `clox`
* There's also a small custom test-suite to verify non-`--std` behavior (challenge solutions and some more informative output)
* `make test` executes both, with and without GC stress-testing (forced GC after each instruction)
