/// Handler trait for processing requests
pub trait Handler {
    fn handle(&self, input: &str) -> String;
    fn name(&self) -> &str;
}
