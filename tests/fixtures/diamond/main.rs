/// Top of diamond: calls both B and C
fn top() {
    process_b();
    process_c();
}

fn process_b() {
    shared_leaf();
}

fn process_c() {
    shared_leaf();
}

/// Shared leaf node at bottom of diamond
fn shared_leaf() -> bool {
    true
}
