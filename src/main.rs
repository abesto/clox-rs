use std::io::Write;

use crate::vm::{Error, VM};

mod bitwise;
mod chunk;
mod compiler;
mod scanner;
mod types;
mod value;
mod vm;

fn main() {
    match std::env::args().collect::<Vec<_>>().as_slice() {
        [_] => repl(),
        [_, file] => run_file(file),
        _ => {
            eprintln!("Usage: clox-rs [path]");
            std::process::exit(64);
        }
    };
}

fn repl() {
    let mut vm = VM::new();
    loop {
        print!("> ");
        std::io::stdout().flush().unwrap();
        let mut line = String::new();
        if std::io::stdin().read_line(&mut line).unwrap() > 0 {
            vm.interpret(line.as_bytes()).unwrap();
        } else {
            println!();
            break;
        }
    }
}

fn run_file(file: &str) {
    let mut vm = VM::new();
    match std::fs::read(file) {
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(74);
        }
        Ok(contents) => match vm.interpret(&contents) {
            Err(Error::CompileError(_)) => std::process::exit(65),
            Err(Error::RuntimeError) => std::process::exit(70),
            Ok(_) => {}
        },
    }
}
