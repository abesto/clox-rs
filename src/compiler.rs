use thiserror::Error;

use crate::scanner::Scanner;

#[derive(Error, Debug)]
pub enum Error {}

pub type Result<T = (), E = Error> = std::result::Result<T, E>;

pub fn compile(source: &str) -> Result {
    let scanner = Scanner::new(source);
    Ok(())
}
