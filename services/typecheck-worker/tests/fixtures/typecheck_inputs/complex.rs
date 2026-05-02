use std::fmt::Display;

// Generic function: fn id<T: Display>(x: T) -> T
pub fn id<T: Display>(x: T) -> T {
    x
}

pub fn process<T, U>(val: T) -> U
where
    T: Clone + Send,
    U: Default,
{
    let _ = val;
    U::default()
}

// Trait with supertraits
pub trait Describable: Display + Clone {
    fn describe(&self) -> String;
}

// Blanket impl for all T: Display + Clone
impl<T: Display + Clone> Describable for T {
    fn describe(&self) -> String {
        format!("{self}")
    }
}

// Struct instantiated at three concrete type args (i32, String, f64)
pub struct Container<T> {
    pub value: T,
}

impl Container<i32> {
    pub fn as_i32(&self) -> i32 {
        self.value
    }
}

impl Container<String> {
    pub fn as_str(&self) -> &str {
        &self.value
    }
}

impl Container<f64> {
    pub fn as_f64(&self) -> f64 {
        self.value
    }
}

// dyn Trait storage site
pub struct DynHolder {
    pub inner: Box<dyn Display>,
}

pub fn make_holder(val: Box<dyn Display>) -> DynHolder {
    DynHolder { inner: val }
}

pub mod utils {
    pub fn helper() -> bool {
        true
    }
}

pub type Result<T> = std::result::Result<T, String>;
