use thiserror::Error;

use crate::{
    scanner::{Scanner, TokenKind},
    types::Line,
};

#[derive(Error, Debug)]
pub enum Error {}

pub type Result<T = (), E = Error> = std::result::Result<T, E>;

pub fn compile(source: &str) -> Result {
    let mut scanner = Scanner::new(source);

    let mut line = Line(0);
    loop {
        let token = scanner.scan();
        if token.line != line {
            print!("{:>4} ", *token.line);
            line = token.line;
        } else {
            print!("   | ");
        }
        println!("{:2} '{}'", u8::from(token.kind.clone()), token.lexeme);

        if token.kind == TokenKind::Eof {
            break;
        };
    }

    Ok(())
}
