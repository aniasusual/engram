use crate::Handler;

pub struct JsonHandler;

impl Handler for JsonHandler {
    fn handle(&self, input: &str) -> String {
        format!("{{\"result\": \"{}\"}}", input)
    }

    fn name(&self) -> &str {
        "json"
    }
}
