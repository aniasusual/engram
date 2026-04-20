/// A sample Rust module for parser testing
pub struct Config {
    pub name: String,
    pub value: i32,
}

pub trait Validator {
    fn validate(&self) -> bool;
}

impl Validator for Config {
    fn validate(&self) -> bool {
        !self.name.is_empty() && self.value > 0
    }
}

pub enum Status {
    Active,
    Inactive,
    Pending,
}

/// Processes a config and returns its status
pub fn process_config(config: &Config) -> Status {
    if config.validate() {
        Status::Active
    } else {
        Status::Inactive
    }
}

const MAX_RETRIES: u32 = 3;
