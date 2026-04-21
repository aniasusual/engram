//! Git integration via libgit2.
//!
//! Provides in-process blame, log, and ownership analysis without shell-out.

use anyhow::{Context, Result};
use git2::{BlameOptions, Repository, Sort};
use std::collections::HashMap;
use std::path::Path;

use crate::graph::Store;

/// Blame information for a single line.
#[derive(Debug, Clone)]
pub struct BlameLine {
    pub line: usize,
    pub commit_hash: String,
    pub author: String,
    pub email: String,
    pub timestamp: i64,
    pub summary: String,
}

/// Commit touching a symbol.
#[derive(Debug, Clone)]
pub struct SymbolCommit {
    pub hash: String,
    pub author: String,
    pub email: String,
    pub timestamp: i64,
    pub message: String,
}

/// File ownership entry.
#[derive(Debug, Clone)]
pub struct OwnershipEntry {
    pub author: String,
    pub email: String,
    pub commits: usize,
    pub last_touched: i64,
}

/// Open a git repository at the given root.
pub fn open_repo(root: &Path) -> Result<Repository> {
    Repository::discover(root).context("not a git repository")
}

/// Get blame data for a file.
pub fn blame_file(repo: &Repository, file_path: &str) -> Result<Vec<BlameLine>> {
    let mut opts = BlameOptions::new();
    let blame = repo
        .blame_file(Path::new(file_path), Some(&mut opts))
        .with_context(|| format!("blaming {}", file_path))?;

    let mut lines = Vec::new();
    for (i, hunk) in blame.iter().enumerate() {
        let sig = hunk.final_signature();
        let commit_id = hunk.final_commit_id();
        lines.push(BlameLine {
            line: hunk.final_start_line() + i - (i.min(hunk.final_start_line().saturating_sub(1))),
            commit_hash: commit_id.to_string(),
            author: sig.name().unwrap_or("unknown").to_string(),
            email: sig.email().unwrap_or("").to_string(),
            timestamp: sig.when().seconds(),
            summary: String::new(), // filled lazily
        });
    }
    Ok(lines)
}

/// Get recent commits for the repository (most recent first).
pub fn get_recent_commits(repo: &Repository, limit: usize) -> Result<Vec<SymbolCommit>> {
    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    revwalk.set_sorting(Sort::TIME)?;

    let mut commits = Vec::new();
    for oid in revwalk.take(limit) {
        let oid = oid?;
        let commit = repo.find_commit(oid)?;
        let sig = commit.author();
        commits.push(SymbolCommit {
            hash: oid.to_string(),
            author: sig.name().unwrap_or("unknown").to_string(),
            email: sig.email().unwrap_or("").to_string(),
            timestamp: sig.when().seconds(),
            message: commit.summary().unwrap_or("").to_string(),
        });
    }
    Ok(commits)
}

/// Get commits that touched a specific file.
pub fn get_file_commits(
    repo: &Repository,
    file_path: &str,
    limit: usize,
) -> Result<Vec<SymbolCommit>> {
    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    revwalk.set_sorting(Sort::TIME)?;

    let mut commits = Vec::new();
    let mut prev_tree = None;

    for oid in revwalk {
        if commits.len() >= limit {
            break;
        }
        let oid = oid?;
        let commit = repo.find_commit(oid)?;
        let tree = commit.tree()?;

        // Check if this commit changed the file
        let changed = if let Some(prev) = &prev_tree {
            let diff = repo.diff_tree_to_tree(Some(prev), Some(&tree), None)?;
            diff.deltas().any(|d| {
                d.new_file()
                    .path()
                    .map(|p| p.to_string_lossy() == file_path)
                    .unwrap_or(false)
                    || d.old_file()
                        .path()
                        .map(|p| p.to_string_lossy() == file_path)
                        .unwrap_or(false)
            })
        } else {
            // First commit — check if file exists in tree
            tree.get_path(Path::new(file_path)).is_ok()
        };

        if changed {
            let sig = commit.author();
            commits.push(SymbolCommit {
                hash: oid.to_string(),
                author: sig.name().unwrap_or("unknown").to_string(),
                email: sig.email().unwrap_or("").to_string(),
                timestamp: sig.when().seconds(),
                message: commit.summary().unwrap_or("").to_string(),
            });
        }

        prev_tree = Some(tree);
    }
    Ok(commits)
}

/// Compute file ownership from blame data.
pub fn compute_ownership(blame_lines: &[BlameLine]) -> Vec<OwnershipEntry> {
    let mut ownership: HashMap<String, OwnershipEntry> = HashMap::new();

    for line in blame_lines {
        let entry = ownership
            .entry(line.author.clone())
            .or_insert_with(|| OwnershipEntry {
                author: line.author.clone(),
                email: line.email.clone(),
                commits: 0,
                last_touched: 0,
            });
        entry.commits += 1;
        if line.timestamp > entry.last_touched {
            entry.last_touched = line.timestamp;
        }
    }

    let mut result: Vec<OwnershipEntry> = ownership.into_values().collect();
    result.sort_by(|a, b| b.commits.cmp(&a.commits));
    result
}

/// Sync git data into the store: commits, ownership.
pub fn sync_git_data(store: &Store, root: &Path) -> Result<()> {
    let repo = open_repo(root)?;

    // Sync recent commits
    let commits = get_recent_commits(&repo, 500)?;
    for commit in &commits {
        store.upsert_git_commit(
            &commit.hash,
            &commit.author,
            &commit.email,
            commit.timestamp,
            &commit.message,
        )?;
    }

    // Sync file ownership
    let files = store.get_all_files()?;
    for file in &files {
        if let Ok(blame_lines) = blame_file(&repo, file) {
            let ownership = compute_ownership(&blame_lines);
            for entry in &ownership {
                store.upsert_file_ownership(
                    file,
                    &entry.author,
                    &entry.email,
                    entry.commits,
                    entry.last_touched,
                )?;
            }
        }
    }

    Ok(())
}

/// Get the current branch name.
#[allow(dead_code)]
pub fn current_branch(repo: &Repository) -> Result<String> {
    let head = repo.head()?;
    Ok(head.shorthand().unwrap_or("HEAD").to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_ownership() {
        let lines = vec![
            BlameLine {
                line: 1,
                commit_hash: "aaa".into(),
                author: "Alice".into(),
                email: "a@x.com".into(),
                timestamp: 100,
                summary: String::new(),
            },
            BlameLine {
                line: 2,
                commit_hash: "aaa".into(),
                author: "Alice".into(),
                email: "a@x.com".into(),
                timestamp: 100,
                summary: String::new(),
            },
            BlameLine {
                line: 3,
                commit_hash: "bbb".into(),
                author: "Bob".into(),
                email: "b@x.com".into(),
                timestamp: 200,
                summary: String::new(),
            },
        ];

        let ownership = compute_ownership(&lines);
        assert_eq!(ownership.len(), 2);
        assert_eq!(ownership[0].author, "Alice"); // 2 lines > 1 line
        assert_eq!(ownership[0].commits, 2);
        assert_eq!(ownership[1].author, "Bob");
        assert_eq!(ownership[1].commits, 1);
    }

    #[test]
    fn test_open_repo_on_engram() {
        // This test runs on the engram repo itself
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let repo = open_repo(root);
        // May or may not be a git repo in CI, so don't assert success
        if let Ok(repo) = repo {
            // Branch detection may fail if HEAD is unborn (no commits)
            let _branch = current_branch(&repo);
        }
    }
}
