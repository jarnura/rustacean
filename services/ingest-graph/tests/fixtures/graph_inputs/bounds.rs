use std::fmt::{Debug, Display};

pub fn process<T>(val: T) -> String
where
    T: Clone + Debug + Display,
{
    format!("{val:?}")
}

pub struct Container<T: Clone + Send>(T);

impl<T: Clone + Send> Container<T> {
    pub fn new(val: T) -> Self {
        Container(val)
    }
}
