use shrinkwraprs::Shrinkwrap;

#[derive(Shrinkwrap, PartialEq, Eq, Clone, Copy, Debug)]
#[shrinkwrap(mutable)]
pub struct Line(pub usize);
