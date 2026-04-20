/// Module 3 — auto-generated for benchmarking
pub struct Service3 {
    name: String,
    value: i32,
}

impl Service3 {
    pub fn new(name: &str) -> Self {
        Self { name: name.to_string(), value: 3 }
    }

    /// Process data in service 3
    pub fn process(&self) -> String {
        let result = self.validate();
        format!("Service3: {} valid={}", self.name, result)
    }

    /// Validate state in service 3
    pub fn validate(&self) -> bool {
        self.value > 0 && !self.name.is_empty()
    }

    /// Helper method for service 3
    fn helper(&self) -> i32 {
        self.value * 2
    }
}

/// Standalone function in module 3
pub fn process_module_3(input: &str) -> String {
    let svc = Service3::new(input);
    svc.process()
}

/// Another function that calls process_module
pub fn run_module_3() -> String {
    process_module_3("test")
}
