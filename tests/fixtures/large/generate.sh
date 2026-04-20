#!/bin/bash
# Generate a deterministic large repo for performance benchmarks.
# Creates 200 Rust files with interconnected functions.

set -e
DIR="$(cd "$(dirname "$0")" && pwd)/generated"
rm -rf "$DIR"
mkdir -p "$DIR"

echo "Generating 200-file test repo in $DIR..."

for i in $(seq 1 200); do
    FILE="$DIR/module_$(printf '%03d' $i).rs"
    cat > "$FILE" << RUST
/// Module $i — auto-generated for benchmarking
pub struct Service${i} {
    name: String,
    value: i32,
}

impl Service${i} {
    pub fn new(name: &str) -> Self {
        Self { name: name.to_string(), value: $i }
    }

    /// Process data in service $i
    pub fn process(&self) -> String {
        let result = self.validate();
        format!("Service${i}: {} valid={}", self.name, result)
    }

    /// Validate state in service $i
    pub fn validate(&self) -> bool {
        self.value > 0 && !self.name.is_empty()
    }

    /// Helper method for service $i
    fn helper(&self) -> i32 {
        self.value * 2
    }
}

/// Standalone function in module $i
pub fn process_module_${i}(input: &str) -> String {
    let svc = Service${i}::new(input);
    svc.process()
}

/// Another function that calls process_module
pub fn run_module_${i}() -> String {
    process_module_${i}("test")
}
RUST
done

echo "Generated 200 files with ~1200 symbols"
