use std::{io::Write, path::PathBuf};

use clap::Parser;

use vm::InterpretResult;

use crate::vm::VM;

mod arena;
mod bitwise;
mod chunk;
mod compiler;
mod config;
mod scanner;
mod types;
mod value;
mod vm;

#[derive(Parser, Debug)]
#[command(version)]
struct Args {
    file: Option<PathBuf>,

    /// Standards mode: compatibility with standard `clox`. Passes the standard `clox` test suite.
    #[arg(long)]
    std: bool,

    #[arg(long)]
    trace_execution: bool,

    #[arg(long)]
    print_code: bool,
}

fn main() {
    let args = Args::parse();

    config::STD_MODE.store(args.std);
    config::TRACE_EXECUTION.store(args.trace_execution);
    config::PRINT_CODE.store(args.print_code);

    if let Some(path) = args.file {
        run_file(path);
    } else {
        repl();
    }
}

fn repl() {
    let mut vm = VM::new();
    loop {
        print!("> ");
        std::io::stdout().flush().unwrap();
        let mut line = String::new();
        if std::io::stdin().read_line(&mut line).unwrap() > 0 {
            vm.interpret(line.as_bytes());
        } else {
            println!();
            break;
        }
    }
}

fn run_file(file: PathBuf) {
    match std::fs::read(file) {
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(74);
        }
        Ok(contents) => {
            let mut vm = VM::new();
            match vm.interpret(&contents) {
                InterpretResult::CompileError => std::process::exit(65),
                InterpretResult::RuntimeError => std::process::exit(70),
                InterpretResult::Ok => {}
            }
        }
    }
}
