//! CodeSearchNet evaluation.
//!
//! Run with: cargo test --test codesearchnet_eval -- --ignored --nocapture
//!
//! Requires:
//!   data/codesearchnet/annotationStore.csv
//!   data/codesearchnet/python/final/jsonl/test/*.jsonl

use engram::embeddings::{EmbeddingEngine, VectorIndex, search_hybrid};
use engram::graph::Store;
use engram::parser::CodeParser;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug)]
struct Annotation {
    query: String,
    url: String,
    relevance: f64,
}

fn load_python_annotations(path: &str) -> Vec<Annotation> {
    let content = std::fs::read_to_string(path).expect("annotationStore.csv not found");
    content
        .lines()
        .skip(1)
        .filter_map(|line| {
            // Format: Language,Query,GitHubUrl,Relevance,Notes
            let mut parts = Vec::new();
            let mut current = String::new();
            let mut in_quotes = false;
            for ch in line.chars() {
                if ch == '"' {
                    in_quotes = !in_quotes;
                } else if ch == ',' && !in_quotes {
                    parts.push(current.clone());
                    current.clear();
                } else {
                    current.push(ch);
                }
            }
            parts.push(current);

            if parts.len() < 4 {
                return None;
            }
            if parts[0] != "Python" {
                return None;
            }
            let relevance: f64 = parts[3].trim().parse().unwrap_or(0.0);
            Some(Annotation {
                query: parts[1].clone(),
                url: parts[2].clone(),
                relevance,
            })
        })
        .collect()
}

/// Extract the function identifier from a GitHub URL.
/// e.g. https://github.com/user/repo/blob/hash/path/file.py#L10-L20
/// We extract the filename and line range for matching.
fn extract_func_key(url: &str) -> String {
    // Get path after /blob/hash/
    if let Some(blob_idx) = url.find("/blob/") {
        let after_blob = &url[blob_idx + 6..];
        // Skip the commit hash
        if let Some(slash_idx) = after_blob.find('/') {
            let path_and_lines = &after_blob[slash_idx + 1..];
            return path_and_lines.to_string();
        }
    }
    url.to_string()
}

fn ndcg_at_k(ranked_relevances: &[f64], k: usize) -> f64 {
    let k = k.min(ranked_relevances.len());
    if k == 0 {
        return 0.0;
    }

    let dcg: f64 = ranked_relevances[..k]
        .iter()
        .enumerate()
        .map(|(i, &rel)| (2f64.powf(rel) - 1.0) / (i as f64 + 2.0).log2())
        .sum();

    let mut ideal = ranked_relevances.to_vec();
    ideal.sort_by(|a, b| b.partial_cmp(a).unwrap());
    let idcg: f64 = ideal[..k]
        .iter()
        .enumerate()
        .map(|(i, &rel)| (2f64.powf(rel) - 1.0) / (i as f64 + 2.0).log2())
        .sum();

    if idcg == 0.0 { 0.0 } else { dcg / idcg }
}

#[test]
#[ignore]
fn codesearchnet_evaluation() {
    // 1. Load annotations
    let ann_path = "data/codesearchnet/annotationStore.csv";
    if !Path::new(ann_path).exists() {
        println!("Annotations not found. Download with:");
        println!("  curl -sL -o data/codesearchnet/annotationStore.csv \\");
        println!(
            "    https://raw.githubusercontent.com/github/CodeSearchNet/master/resources/annotationStore.csv"
        );
        return;
    }

    let annotations = load_python_annotations(ann_path);
    println!("Loaded {} Python annotations", annotations.len());

    // Group by query
    let mut queries_map: HashMap<String, Vec<(String, f64)>> = HashMap::new();
    for ann in &annotations {
        queries_map
            .entry(ann.query.clone())
            .or_default()
            .push((ann.url.clone(), ann.relevance));
    }
    println!("Unique queries: {}", queries_map.len());

    // 2. Index the Python test corpus
    let test_dir = Path::new("data/codesearchnet/python/final/jsonl/test");
    if !test_dir.exists() {
        println!("Corpus not found at {}", test_dir.display());
        println!("Download and extract with:");
        println!(
            "  cd data/codesearchnet && curl -sLO https://s3.amazonaws.com/code-search-net/CodeSearchNet/v2/python.zip && unzip python.zip"
        );
        println!("  cd python/final/jsonl/test && gunzip *.gz");
        return;
    }

    let store = Store::open_in_memory().unwrap();
    store.initialize().unwrap();
    let parser = CodeParser::new();

    println!("Indexing corpus...");
    let mut indexed_symbols = 0usize;
    let mut indexed_files = 0usize;
    // Map: url_key -> symbol_id for matching annotations to search results
    let mut url_to_symbol: HashMap<String, String> = HashMap::new();

    for entry in std::fs::read_dir(test_dir).unwrap() {
        let path = entry.unwrap().path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        if ext == "gz" {
            println!(
                "  Found .gz file — decompress first: gunzip {}",
                path.display()
            );
            continue;
        }
        if ext != "jsonl" {
            continue;
        }

        let content = std::fs::read_to_string(&path).unwrap();
        for line in content.lines() {
            if let Ok(obj) = serde_json::from_str::<serde_json::Value>(line) {
                let code = match obj["code"].as_str() {
                    Some(c) => c,
                    None => continue,
                };
                let func_name = obj["func_name"].as_str().unwrap_or("unknown");
                let url = obj["url"].as_str().unwrap_or("");
                let repo = obj["repo"].as_str().unwrap_or("unknown");
                let filepath = obj["path"].as_str().unwrap_or("unknown.py");

                let file_key = format!("{}_{}/{}", repo, indexed_files, filepath);

                if let Ok(result) = parser.parse_source(code, "python", &file_key)
                    && store.sync_file(Path::new(&file_key), &result).is_ok()
                {
                    indexed_files += 1;
                    for sym in &result.symbols {
                        indexed_symbols += 1;
                        // Map the URL to this symbol for ground-truth matching
                        if !url.is_empty() {
                            url_to_symbol.insert(url.to_string(), sym.id.clone());
                        }
                        // Also map by func_name for fuzzy matching
                        url_to_symbol
                            .entry(func_name.to_string())
                            .or_insert_with(|| sym.id.clone());
                    }
                }
            }
        }
    }

    println!(
        "Indexed {} files, {} symbols",
        indexed_files, indexed_symbols
    );

    if indexed_symbols == 0 {
        println!("No symbols indexed. Check that .jsonl files are decompressed.");
        return;
    }

    // 3. Compute embeddings (batch for speed)
    println!("Computing embeddings...");
    let engine = match EmbeddingEngine::new() {
        Ok(e) => e,
        Err(e) => {
            println!("Failed to load embedding model: {}", e);
            return;
        }
    };

    let all_symbols = store.get_all_symbols().unwrap();
    let batch_size = 256;
    let total_batches = all_symbols.len().div_ceil(batch_size);

    for (batch_idx, chunk) in all_symbols.chunks(batch_size).enumerate() {
        let texts: Vec<String> = chunk
            .iter()
            .map(|sym| EmbeddingEngine::build_embedding_text(sym, &[], &[]))
            .collect();

        match engine.embed_batch(&texts) {
            Ok(embeddings) => {
                for (i, emb) in embeddings.into_iter().enumerate() {
                    let _ = store.save_embedding(&chunk[i].id, &emb, &chunk[i].body_hash);
                }
            }
            Err(e) => {
                println!("Embedding batch {} failed: {}", batch_idx, e);
            }
        }

        if (batch_idx + 1) % 10 == 0 || batch_idx + 1 == total_batches {
            println!(
                "  Embedded batch {}/{} ({} symbols)",
                batch_idx + 1,
                total_batches,
                (batch_idx + 1) * batch_size
            );
        }
    }

    let mut index = VectorIndex::new();
    index.load_from_store(&store).unwrap();
    println!("Vector index: {} embeddings loaded", index.len());

    // 4. Run evaluation
    println!("\nRunning {} queries...", queries_map.len());
    let mut ndcg_scores: Vec<f64> = Vec::new();
    let mut mrr_scores: Vec<f64> = Vec::new();

    for (query, ground_truth) in &queries_map {
        // Run hybrid search
        let results = search_hybrid(query, &store, &engine, &index, 10).unwrap_or_default();

        // Map results to relevance scores using ground truth
        let mut ranked_relevances = Vec::new();
        let mut found_relevant = false;

        for (rank, result) in results.iter().enumerate() {
            let mut rel = 0.0;

            // Check if this result matches any ground truth annotation
            if let Ok(Some(sym)) = store.get_symbol(&result.symbol_id) {
                for (gt_url, gt_rel) in ground_truth {
                    // Match by URL in our mapping
                    if let Some(gt_sym_id) = url_to_symbol.get(gt_url.as_str())
                        && *gt_sym_id == result.symbol_id
                    {
                        rel = *gt_rel;
                        break;
                    }
                    // Fuzzy match by function name
                    let url_key = extract_func_key(gt_url);
                    if url_key.contains(&sym.name)
                        || sym.name.len() > 3 && gt_url.contains(&sym.name)
                    {
                        rel = *gt_rel;
                        break;
                    }
                }
            }

            ranked_relevances.push(rel);

            if rel > 0.0 && !found_relevant {
                mrr_scores.push(1.0 / (rank as f64 + 1.0));
                found_relevant = true;
            }
        }

        if !found_relevant {
            mrr_scores.push(0.0);
        }

        let ndcg = ndcg_at_k(&ranked_relevances, 10);
        ndcg_scores.push(ndcg);
    }

    let avg_ndcg = ndcg_scores.iter().sum::<f64>() / ndcg_scores.len().max(1) as f64;
    let avg_mrr = mrr_scores.iter().sum::<f64>() / mrr_scores.len().max(1) as f64;
    let nonzero_ndcg = ndcg_scores.iter().filter(|&&s| s > 0.0).count();

    println!("\n=== CodeSearchNet Results (Python) ===");
    println!("  Queries evaluated:  {}", ndcg_scores.len());
    println!("  Average NDCG@10:    {:.4}", avg_ndcg);
    println!("  Average MRR:        {:.4}", avg_mrr);
    println!(
        "  Queries with hits:  {}/{} ({:.1}%)",
        nonzero_ndcg,
        ndcg_scores.len(),
        nonzero_ndcg as f64 / ndcg_scores.len().max(1) as f64 * 100.0
    );
    println!();
    println!("  Published baselines:");
    println!("    Neural BoW:  0.17");
    println!("    RNN:         0.18");
    println!("    1D-CNN:      0.20");
    println!("    Self-Attn:   0.21");
    println!("    BERT:        0.27");
    println!("    CodeBERT:    0.45");
    println!();

    if avg_ndcg > 0.27 {
        println!("  BEATS BERT baseline!");
    } else if avg_ndcg > 0.17 {
        println!("  Beats Neural BoW baseline");
    } else {
        println!("  Below baselines with general-purpose model (AllMiniLM-L6-V2)");
        println!("  Note: baselines use models trained ON CodeSearchNet code-text pairs.");
        println!("  Engram uses a general English model. Swapping to CodeBERT/UniXcoder");
        println!("  via the Embedder trait would significantly improve this score.");
    }

    println!();
    println!(
        "  Embedding coverage: {}/{} symbols ({:.1}%)",
        index.len(),
        all_symbols.len(),
        index.len() as f64 / all_symbols.len().max(1) as f64 * 100.0
    );
}
