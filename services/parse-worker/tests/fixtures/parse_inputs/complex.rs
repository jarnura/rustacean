use std::collections::HashMap;

pub trait Serialize {
    fn serialize(&self) -> Vec<u8>;
}

pub trait Deserialize: Sized {
    fn deserialize(data: &[u8]) -> Option<Self>;
}

pub struct Registry<T> {
    entries: HashMap<String, T>,
}

impl<T: Serialize> Registry<T> {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn insert(&mut self, key: String, value: T) {
        self.entries.insert(key, value);
    }
}

impl<T: Serialize> Default for Registry<T> {
    fn default() -> Self {
        Self::new()
    }
}

pub enum Command {
    Insert { key: String, value: i64 },
    Delete(String),
    Clear,
}

pub mod storage {
    pub const MAX_ENTRIES: usize = 1024;

    pub fn is_full(count: usize) -> bool {
        count >= MAX_ENTRIES
    }
}

pub async fn process_command(cmd: Command) -> Result<(), String> {
    match cmd {
        Command::Insert { key, value: _ } => {
            println!("insert: {key}");
            Ok(())
        }
        Command::Delete(key) => {
            println!("delete: {key}");
            Ok(())
        }
        Command::Clear => Ok(()),
    }
}
