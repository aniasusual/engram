//! Topology-based module clustering via Louvain-style modularity optimization.
//!
//! Clusters symbols into communities based on call graph connectivity,
//! replacing file-path heuristics with actual structural relationships.

use anyhow::Result;
use std::collections::{HashMap, HashSet};

use crate::graph::Store;

/// A detected community of symbols.
#[derive(Debug, Clone)]
pub struct Community {
    pub id: usize,
    pub label: String,
    pub symbol_ids: Vec<String>,
    pub cohesion: f64,
}

/// Run community detection on the call graph.
///
/// Uses a greedy modularity optimization (Louvain-style):
/// 1. Start with each symbol in its own community
/// 2. For each symbol, try moving it to a neighbor's community
/// 3. Accept the move if it increases modularity
/// 4. Repeat until no improvement
pub fn detect_communities(store: &Store) -> Result<Vec<Community>> {
    // Build adjacency list from edges (undirected for community detection)
    let all_symbols = store.get_all_symbols()?;
    if all_symbols.is_empty() {
        return Ok(vec![]);
    }

    let symbol_ids: Vec<String> = all_symbols.iter().map(|s| s.id.clone()).collect();
    let id_to_idx: HashMap<String, usize> = symbol_ids
        .iter()
        .enumerate()
        .map(|(i, id)| (id.clone(), i))
        .collect();

    let n = symbol_ids.len();

    // Build undirected adjacency
    let mut adj: Vec<HashSet<usize>> = vec![HashSet::new(); n];
    let mut total_edges = 0usize;

    for sym in &all_symbols {
        let from_idx = id_to_idx[&sym.id];

        // Get callers and callees
        if let Ok(callers) = store.get_direct_callers(&sym.id) {
            for caller_id in &callers {
                if let Some(&to_idx) = id_to_idx.get(caller_id)
                    && from_idx != to_idx
                {
                    adj[from_idx].insert(to_idx);
                    adj[to_idx].insert(from_idx);
                    total_edges += 1;
                }
            }
        }
    }

    if total_edges == 0 {
        // No edges — fall back to file-based clustering
        return file_based_communities(&all_symbols);
    }

    // Louvain Phase 1: local modularity optimization
    let m = total_edges as f64 / 2.0; // each edge counted twice
    let degree: Vec<f64> = adj.iter().map(|neighbors| neighbors.len() as f64).collect();

    // Each node starts in its own community
    let mut community: Vec<usize> = (0..n).collect();
    let mut improved = true;
    let mut iterations = 0;

    while improved && iterations < 20 {
        improved = false;
        iterations += 1;

        for i in 0..n {
            let current_comm = community[i];

            // Count edges to each neighboring community
            let mut comm_edges: HashMap<usize, f64> = HashMap::new();
            for &j in &adj[i] {
                *comm_edges.entry(community[j]).or_default() += 1.0;
            }

            // Sum of degrees in current community (excluding node i)
            let sigma_current: f64 = (0..n)
                .filter(|&j| j != i && community[j] == current_comm)
                .map(|j| degree[j])
                .sum();

            let ki = degree[i];
            let ki_in_current = *comm_edges.get(&current_comm).unwrap_or(&0.0);

            let mut best_comm = current_comm;
            let mut best_delta = 0.0;

            for (&target_comm, &ki_in_target) in &comm_edges {
                if target_comm == current_comm {
                    continue;
                }

                let sigma_target: f64 = (0..n)
                    .filter(|&j| community[j] == target_comm)
                    .map(|j| degree[j])
                    .sum();

                // Modularity delta for moving i from current to target
                let delta = (ki_in_target - ki_in_current) / m
                    - ki * (sigma_target - sigma_current) / (2.0 * m * m);

                if delta > best_delta {
                    best_delta = delta;
                    best_comm = target_comm;
                }
            }

            if best_comm != current_comm {
                community[i] = best_comm;
                improved = true;
            }
        }
    }

    // Build community structs
    let mut comm_members: HashMap<usize, Vec<String>> = HashMap::new();
    for (i, &comm_id) in community.iter().enumerate() {
        comm_members
            .entry(comm_id)
            .or_default()
            .push(symbol_ids[i].clone());
    }

    let mut communities: Vec<Community> = Vec::new();
    for (idx, (_, members)) in comm_members.iter().enumerate() {
        // Compute cohesion: ratio of internal edges to possible edges
        let member_set: HashSet<usize> = members
            .iter()
            .filter_map(|id| id_to_idx.get(id).copied())
            .collect();

        let internal_edges: usize = member_set
            .iter()
            .map(|&i| adj[i].intersection(&member_set).count())
            .sum::<usize>()
            / 2;

        let possible_edges = members.len() * (members.len().saturating_sub(1)) / 2;
        let cohesion = if possible_edges > 0 {
            internal_edges as f64 / possible_edges as f64
        } else {
            1.0
        };

        // Generate label from common file paths or most connected symbol
        let label = generate_label(store, members);

        communities.push(Community {
            id: idx,
            label,
            symbol_ids: members.clone(),
            cohesion,
        });
    }

    // Sort by size descending
    communities.sort_by_key(|c| std::cmp::Reverse(c.symbol_ids.len()));

    // Re-number IDs
    for (i, comm) in communities.iter_mut().enumerate() {
        comm.id = i;
    }

    Ok(communities)
}

/// Fallback: cluster symbols by file path.
fn file_based_communities(symbols: &[crate::graph::SymbolRow]) -> Result<Vec<Community>> {
    let mut by_file: HashMap<String, Vec<String>> = HashMap::new();
    for sym in symbols {
        by_file
            .entry(sym.file.clone())
            .or_default()
            .push(sym.id.clone());
    }

    let mut communities: Vec<Community> = by_file
        .into_iter()
        .enumerate()
        .map(|(i, (file, members))| Community {
            id: i,
            label: file,
            symbol_ids: members,
            cohesion: 1.0, // Within-file symbols are maximally cohesive by definition
        })
        .collect();

    communities.sort_by_key(|c| std::cmp::Reverse(c.symbol_ids.len()));
    Ok(communities)
}

/// Generate a human-readable label for a community.
fn generate_label(store: &Store, member_ids: &[String]) -> String {
    // Use the most common file path prefix
    let files: Vec<String> = member_ids
        .iter()
        .filter_map(|id| store.get_symbol(id).ok().flatten().map(|s| s.file))
        .collect();

    if files.is_empty() {
        return "unknown".to_string();
    }

    // Find common prefix
    if let Some(first) = files.first() {
        let common_prefix: String = first
            .chars()
            .enumerate()
            .take_while(|(i, c)| files.iter().all(|f| f.chars().nth(*i) == Some(*c)))
            .map(|(_, c)| c)
            .collect();

        if common_prefix.len() > 2 {
            return common_prefix.trim_end_matches('/').to_string();
        }
    }

    // Fallback: most common file
    let mut file_counts: HashMap<&str, usize> = HashMap::new();
    for f in &files {
        *file_counts.entry(f.as_str()).or_default() += 1;
    }
    file_counts
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .map(|(f, _)| f.to_string())
        .unwrap_or_else(|| "mixed".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::CodeParser;
    use std::path::Path;

    #[test]
    fn test_community_detection_with_edges() {
        let store = Store::open_in_memory().unwrap();
        store.initialize().unwrap();
        let parser = CodeParser::new();

        // Two clusters: auth functions call each other, db functions call each other
        let source = r#"
fn validate_token() -> bool { check_signature() }
fn check_signature() -> bool { true }
fn auth_middleware() { validate_token(); }

fn query_db() -> String { format_result() }
fn format_result() -> String { "ok".to_string() }
fn save_record() { query_db(); }
"#;
        let result = parser.parse_source(source, "rust", "test.rs").unwrap();
        store.sync_file(Path::new("test.rs"), &result).unwrap();

        let communities = detect_communities(&store).unwrap();
        assert!(
            !communities.is_empty(),
            "should detect at least one community"
        );

        // Total symbols across all communities should match
        let total: usize = communities.iter().map(|c| c.symbol_ids.len()).sum();
        assert_eq!(total, result.symbols.len());
    }

    #[test]
    fn test_community_detection_no_edges() {
        let store = Store::open_in_memory().unwrap();
        store.initialize().unwrap();
        let parser = CodeParser::new();

        let source = "fn standalone_a() {}\nfn standalone_b() {}";
        let result = parser.parse_source(source, "rust", "test.rs").unwrap();
        store.sync_file(Path::new("test.rs"), &result).unwrap();

        let communities = detect_communities(&store).unwrap();
        // Falls back to file-based clustering
        assert!(!communities.is_empty());
    }

    #[test]
    fn test_cohesion_score() {
        let store = Store::open_in_memory().unwrap();
        store.initialize().unwrap();
        let parser = CodeParser::new();

        let source = r#"
fn a() { b(); }
fn b() { a(); }
"#;
        let result = parser.parse_source(source, "rust", "test.rs").unwrap();
        store.sync_file(Path::new("test.rs"), &result).unwrap();

        let communities = detect_communities(&store).unwrap();
        for comm in &communities {
            assert!(
                comm.cohesion >= 0.0 && comm.cohesion <= 1.0,
                "cohesion should be between 0 and 1, got {}",
                comm.cohesion
            );
        }
    }
}
