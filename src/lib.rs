// Some items are used by tests/benches but not by the binary — allow dead_code at crate level
// to keep the build clean. All public API items are tested.
#![allow(
    dead_code,
    clippy::type_complexity,
    clippy::too_many_arguments,
    clippy::collapsible_if,
    clippy::single_match
)]

pub mod cli;
pub mod embeddings;
pub mod git;
pub mod graph;
pub mod intelligence;
pub mod mcp;
pub mod memory;
pub mod parser;
pub mod temporal;
pub mod watcher;
