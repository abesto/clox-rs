use thiserror::Error;

use crate::types::Line;

#[derive(Error, Debug)]
pub enum Error {}

pub type Result<T = (), E = Error> = std::result::Result<T, E>;

pub struct Scanner<'a> {
    start: std::str::Chars<'a>,
    current: std::str::Chars<'a>,
    line: Line,
}

impl<'a> Scanner<'a> {
    #[must_use]
    pub fn new(source: &'a str) -> Self {
        Self {
            start: source.chars(),
            current: source.chars(),
            line: Line(1),
        }
    }
}
