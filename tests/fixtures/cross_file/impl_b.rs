use crate::Handler;

pub struct XmlHandler;

impl Handler for XmlHandler {
    fn handle(&self, input: &str) -> String {
        format!("<result>{}</result>", input)
    }

    fn name(&self) -> &str {
        "xml"
    }
}
