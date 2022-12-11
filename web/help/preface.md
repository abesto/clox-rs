So this dude called Robert Nystrom wrote a book called [Crafting Interpreters](https://craftinginterpreters.com/). You should totally buy it. It's amazing.

It walks you through every small piece of creating an interpreter for a small programming language called Lox. The first ~third of the book introduces a tree-walk interpreter in Java. You can check out my Rust version of it at [gh:abesto/jlox-rs](https://abesto.github.io/jlox-rs/).

What you're now actually looking at is my Rust version of the *second* part of the book, which writes in C a compiler and VM. In particular, you're staring at:

* A bytecode compiler
* A disassembler for said bytecode (tick "Show Bytecode" and / or "Trace Execution" on the top)
* Manual, safe memory management for the heap using arenas
* Garbage collection for said heap
* A virtual machine that executes the bytecode

## The Webby Bits

This webpage is almost fully built in Rust, using the [yew](https://yew.rs/) framework - it's basically React, but in Rust. The editor is [Monaco](https://microsoft.github.io/monaco-editor/), via the [rust-monaco](https://github.com/siku2/rust-monaco) bindings. Output is collected via the [`log`](https://docs.rs/log/latest/log/) crate in the compiler / VM, and displayed on the right using a custom log formatter. This text is compiled at build time from markdown using `cmark` to HTML and included as raw HTML.

## Notes
