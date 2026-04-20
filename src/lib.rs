// Some items are used by tests/benches but not by the binary — allow dead_code at crate level
// to keep the build clean. All public API items are tested.
#![allow(dead_code)]

pub mod cli;
pub mod embeddings;
pub mod git;
pub mod graph;
pub mod intelligence;
pub mod memory;
pub mod mcp;
pub mod parser;
pub mod temporal;
pub mod watcher;
