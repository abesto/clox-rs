use shrinkwraprs::Shrinkwrap;

#[derive(Shrinkwrap, PartialEq, Eq, Clone, Copy, Debug)]
pub struct Line(pub usize);
