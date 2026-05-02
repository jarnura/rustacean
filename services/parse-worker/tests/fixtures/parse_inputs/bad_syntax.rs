// This file contains deliberate syntax errors to test error recovery.

pub fn valid_before_error() -> i32 {
    42
}

// Broken function: missing closing paren
pub fn broken_fn(x: i32, y {
    x + y
}

pub struct ValidAfterError {
    pub value: String,
}
