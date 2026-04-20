/// Module 8 — auto-generated for benchmarking
pub struct Service8 {
    name: String,
    value: i32,
}

impl Service8 {
    pub fn new(name: &str) -> Self {
        Self { name: name.to_string(), value: 8 }
    }

    /// Process data in service 8
    pub fn process(&self) -> String {
        let result = self.validate();
        format!("Service8: {} valid={}", self.name, result)
    }

    /// Validate state in service 8
    pub fn validate(&self) -> bool {
        self.value > 0 && !self.name.is_empty()
    }

    /// Helper method for service 8
    fn helper(&self) -> i32 {
        self.value * 2
    }
}

/// Standalone function in module 8
pub fn process_module_8(input: &str) -> String {
    let svc = Service8::new(input);
    svc.process()
}

/// Another function that calls process_module
pub fn run_module_8() -> String {
    process_module_8("test")
}
