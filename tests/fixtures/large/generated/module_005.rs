/// Module 5 — auto-generated for benchmarking
pub struct Service5 {
    name: String,
    value: i32,
}

impl Service5 {
    pub fn new(name: &str) -> Self {
        Self { name: name.to_string(), value: 5 }
    }

    /// Process data in service 5
    pub fn process(&self) -> String {
        let result = self.validate();
        format!("Service5: {} valid={}", self.name, result)
    }

    /// Validate state in service 5
    pub fn validate(&self) -> bool {
        self.value > 0 && !self.name.is_empty()
    }

    /// Helper method for service 5
    fn helper(&self) -> i32 {
        self.value * 2
    }
}

/// Standalone function in module 5
pub fn process_module_5(input: &str) -> String {
    let svc = Service5::new(input);
    svc.process()
}

/// Another function that calls process_module
pub fn run_module_5() -> String {
    process_module_5("test")
}
