#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Config {
    pub host: String,
    pub port: u16,
}

#[derive(Clone, Copy, Debug)]
pub enum Direction {
    North,
    South,
    East,
    West,
}

pub trait Describable: Clone + std::fmt::Display {
    fn describe(&self) -> String;
}
