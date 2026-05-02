/// Simple feature-flagged fixture crate for expand-worker integration tests.

#[cfg(feature = "logging")]
pub fn log_message(msg: &str) {
    println!("[LOG] {msg}");
}

#[cfg(not(feature = "logging"))]
pub fn log_message(_msg: &str) {}

pub fn greet(name: &str) -> String {
    format!("Hello, {name}!")
}
