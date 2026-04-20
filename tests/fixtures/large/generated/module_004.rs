/// Module 4 — auto-generated for benchmarking
pub struct Service4 {
    name: String,
    value: i32,
}

impl Service4 {
    pub fn new(name: &str) -> Self {
        Self { name: name.to_string(), value: 4 }
    }

    /// Process data in service 4
    pub fn process(&self) -> String {
        let result = self.validate();
        format!("Service4: {} valid={}", self.name, result)
    }

    /// Validate state in service 4
    pub fn validate(&self) -> bool {
        self.value > 0 && !self.name.is_empty()
    }

    /// Helper method for service 4
    fn helper(&self) -> i32 {
        self.value * 2
    }
}

/// Standalone function in module 4
pub fn process_module_4(input: &str) -> String {
    let svc = Service4::new(input);
    svc.process()
}

/// Another function that calls process_module
pub fn run_module_4() -> String {
    process_module_4("test")
}
