pub mod chunking;
pub mod languages;
pub mod markdown;
pub mod treesitter;

pub use treesitter::{CodeParser, ParseResult};

// Re-exported for tests and benchmarks
#[allow(unused_imports)]
pub use treesitter::{Edge, EdgeKind, Symbol, SymbolKind};

use anyhow::Result;
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};

/// File extensions we know how to parse.
const SUPPORTED_EXTENSIONS: &[(&str, &str)] = &[
    ("rs", "rust"),
    ("py", "python"),
    ("ts", "typescript"),
    ("tsx", "typescript"),
    ("js", "javascript"),
    ("jsx", "javascript"),
    ("go", "go"),
    ("java", "java"),
    ("c", "c"),
    ("h", "c"),
    ("cpp", "cpp"),
    ("cc", "cpp"),
    ("cxx", "cpp"),
    ("hpp", "cpp"),
    ("hh", "cpp"),
    ("rb", "ruby"),
    ("md", "markdown"),
    ("markdown", "markdown"),
];

/// Detect language from file extension.
pub fn detect_language(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?;
    SUPPORTED_EXTENSIONS
        .iter()
        .find(|(e, _)| *e == ext)
        .map(|(_, lang)| *lang)
}

/// Discover all parseable source files under `root`, respecting .gitignore.
pub fn discover_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            // Skip common non-source directories
            !(name == "node_modules"
                || name == "target"
                || name == ".git"
                || name == ".engram"
                || name == "__pycache__"
                || name == ".venv"
                || name == "vendor"
                || name == "dist"
                || name == "build")
        })
        .build()
    {
        let entry = entry?;
        if entry.file_type().is_some_and(|ft| ft.is_file())
            && detect_language(entry.path()).is_some()
        {
            files.push(entry.into_path());
        }
    }
    Ok(files)
}
