#![allow(dead_code)]

mod cli;
mod embeddings;
mod git;
mod graph;
mod intelligence;
mod memory;
mod mcp;
mod parser;
mod temporal;
mod watcher;

use clap::Parser;
use cli::Cli;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "engram=info".parse().unwrap()),
        )
        .init();

    let cli = Cli::parse();
    cli.run().await
}
