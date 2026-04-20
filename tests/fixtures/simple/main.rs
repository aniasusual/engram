use crate::handler;

fn main() {
    let result = handler::handle_request("token123");
    println!("Result: {}", result);
}
