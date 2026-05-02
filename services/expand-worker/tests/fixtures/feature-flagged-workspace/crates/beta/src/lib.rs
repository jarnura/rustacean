/// Beta crate — no default features, tests feature resolution with default=false.

pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[cfg(feature = "extra")]
pub fn multiply(a: i32, b: i32) -> i32 {
    a * b
}
