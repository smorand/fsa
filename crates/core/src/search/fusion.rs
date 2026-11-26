//! Reciprocal Rank Fusion (RRF) for merging BM25 and vector result lists.
//!
//! RRF with k=60 is the standard choice for hybrid retrieval: it is parameter
//! insensitive over a wide range of corpus sizes and consistently outperforms
//! simple score combination without requiring calibration.
//!
//! The merge is deterministic: when two entries have the same fused score they
//! are ordered by path (lexicographic), so the output is stable across calls.

use crate::search::SearchResult;
use std::collections::HashMap;

/// Standard RRF constant. Higher values dampen the influence of top-ranked items.
const K: f32 = 60.0;

/// Merge two ranked result lists using Reciprocal Rank Fusion.
///
/// A document appearing in both lists scores higher than one in only one list.
/// The returned list is re-ranked and rank indices are assigned from 1.
pub fn rrf_merge(bm25: &[SearchResult], vector: &[SearchResult]) -> Vec<SearchResult> {
    // Map path -> cumulative RRF score, best chunk from either list.
    let mut scores: HashMap<String, f32> = HashMap::new();
    let mut best: HashMap<String, (String, f32, u32)> = HashMap::new(); // path -> (chunk, score, chunk_idx)

    for (i, r) in bm25.iter().enumerate() {
        let rrf = 1.0 / (K + (i as f32) + 1.0);
        *scores.entry(r.path.clone()).or_default() += rrf;
        best.entry(r.path.clone())
            .and_modify(|(c, s, n)| {
                if r.score > *s {
                    *c = r.chunk.clone();
                    *s = r.score;
                    *n = r.chunk_idx;
                }
            })
            .or_insert_with(|| (r.chunk.clone(), r.score, r.chunk_idx));
    }

    for (i, r) in vector.iter().enumerate() {
        let rrf = 1.0 / (K + (i as f32) + 1.0);
        *scores.entry(r.path.clone()).or_default() += rrf;
        best.entry(r.path.clone())
            .and_modify(|(c, s, n)| {
                if r.score > *s {
                    *c = r.chunk.clone();
                    *s = r.score;
                    *n = r.chunk_idx;
                }
            })
            .or_insert_with(|| (r.chunk.clone(), r.score, r.chunk_idx));
    }

    // Sort by fused score descending, then by path for determinism.
    let mut entries: Vec<(String, f32)> = scores.into_iter().collect();
    entries.sort_by(|a, b| {
        b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal).then_with(|| a.0.cmp(&b.0))
    });

    entries
        .into_iter()
        .enumerate()
        .map(|(i, (path, fused_score))| {
            let (chunk, _orig_score, chunk_idx) = best.remove(&path).unwrap_or_default();
            SearchResult { path, score: fused_score, chunk, rank: i + 1, chunk_idx }
        })
        .collect()
}

/// One unit of the fused list of `search.chunks` (SPEC-0109): a chunk, its fused
/// score and the best rank it had in either list.
#[derive(Debug, Clone, PartialEq)]
pub struct Fused {
    pub path: String,
    pub number: u32,
    pub score: f64,
    pub best_rank: usize,
}

/// Fuses a dense and a keyword list of `(path, chunk number)` units, each best
/// first: the score of a unit is the sum of `1/(60 + rank)` over the lists that
/// hold it (rank from 1). One entry per unit, a unit repeated in a list counting
/// once at its best rank. Order: score, then best rank, then the stable id (path,
/// then number). Pure and deterministic.
pub fn fuse_units(dense: &[(String, u32)], keyword: &[(String, u32)]) -> Vec<Fused> {
    let mut by_unit: HashMap<(&str, u32), Fused> = HashMap::new();
    for list in [dense, keyword] {
        let mut seen: std::collections::HashSet<(&str, u32)> = std::collections::HashSet::new();
        for (i, (path, number)) in list.iter().enumerate() {
            if !seen.insert((path.as_str(), *number)) {
                continue;
            }
            let rank = i + 1;
            let part = 1.0 / (f64::from(K) + rank as f64);
            by_unit
                .entry((path.as_str(), *number))
                .and_modify(|f| {
                    f.score += part;
                    f.best_rank = f.best_rank.min(rank);
                })
                .or_insert_with(|| Fused {
                    path: path.clone(),
                    number: *number,
                    score: part,
                    best_rank: rank,
                });
        }
    }
    let mut out: Vec<Fused> = by_unit.into_values().collect();
    out.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.best_rank.cmp(&b.best_rank))
            .then_with(|| a.path.cmp(&b.path))
            .then_with(|| a.number.cmp(&b.number))
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(path: &str, score: f32, chunk: &str, rank: usize) -> SearchResult {
        SearchResult { path: path.into(), score, chunk: chunk.into(), rank, chunk_idx: 1 }
    }

    #[test]
    fn rrf_merge_is_deterministic() {
        let bm25 = vec![r("/a.md", 0.9, "chunk a", 1), r("/b.md", 0.5, "chunk b", 2)];
        let vec_ = vec![r("/b.md", 0.8, "chunk b", 1), r("/a.md", 0.4, "chunk a", 2)];
        let out1 = rrf_merge(&bm25, &vec_);
        let out2 = rrf_merge(&bm25, &vec_);
        assert_eq!(
            out1.iter().map(|r| &r.path).collect::<Vec<_>>(),
            out2.iter().map(|r| &r.path).collect::<Vec<_>>(),
            "same inputs must produce the same order"
        );
    }

    #[test]
    fn rrf_merge_prefers_appearing_in_both_lists() {
        // /shared.md appears in both lists at rank 2; /only_bm25.md only in bm25 at rank 1.
        let bm25 = vec![r("/only_bm25.md", 1.0, "x", 1), r("/shared.md", 0.5, "y", 2)];
        let vec_ = vec![r("/only_vec.md", 1.0, "z", 1), r("/shared.md", 0.5, "y", 2)];
        let out = rrf_merge(&bm25, &vec_);
        let paths: Vec<&str> = out.iter().map(|r| r.path.as_str()).collect();
        let shared_pos = paths.iter().position(|p| *p == "/shared.md").unwrap();
        let only_bm25_pos = paths.iter().position(|p| *p == "/only_bm25.md").unwrap();
        assert!(
            shared_pos < only_bm25_pos,
            "a doc in both lists should rank above one in only one list: {paths:?}"
        );
    }

    fn u(p: &str, n: u32) -> (String, u32) {
        (p.to_string(), n)
    }

    /// SPEC-0109 FR-NEW-002: the sum of `1/(60 + rank)` over the lists.
    #[test]
    fn fuse_units_sums_reciprocal_ranks() {
        let dense = [u("/a.md", 1), u("/b.md", 1)];
        let keyword = [u("/b.md", 1), u("/c.md", 1)];
        let out = fuse_units(&dense, &keyword);
        let ids: Vec<_> = out.iter().map(|f| (f.path.as_str(), f.number)).collect();
        assert_eq!(ids, [("/b.md", 1), ("/a.md", 1), ("/c.md", 1)]);
        assert!((out[0].score - (1.0 / 62.0 + 1.0 / 61.0)).abs() < 1e-12);
        assert!((out[1].score - 1.0 / 61.0).abs() < 1e-12);
        assert!((out[2].score - 1.0 / 62.0).abs() < 1e-12);
        assert_eq!((out[0].best_rank, out[1].best_rank, out[2].best_rank), (1, 1, 2));
    }

    /// Ties go to the best rank, then to the path, then to the chunk number.
    #[test]
    fn fuse_units_breaks_ties_by_best_rank_then_stable_id() {
        // z at dense 1 and y at keyword 1: equal score and equal best rank, path decides.
        let out = fuse_units(&[u("/z.md", 1)], &[u("/y.md", 1)]);
        assert_eq!(out[0].path, "/y.md");
        // Same path: the number decides, 2 before 10 (numbers, not text).
        let out = fuse_units(&[u("/a.md", 10)], &[u("/a.md", 2)]);
        assert_eq!(out[0].number, 2);
        // Equal score, different best rank: a at dense 2 + keyword 3 against b at dense 1 + keyword 5
        // is not a tie; build one: x dense 1 only (1/61), y keyword 1 only (1/61), w dense 2 + keyword 2.
        let out = fuse_units(&[u("/x.md", 1), u("/w.md", 1)], &[u("/y.md", 1), u("/w.md", 1)]);
        assert_eq!(out[0].path, "/w.md", "two lists beat one");
        assert_eq!(out[1].path, "/x.md");
    }

    /// One line per unit: a unit twice in one list counts once, at its best rank.
    #[test]
    fn fuse_units_keeps_one_unit_once() {
        let out = fuse_units(&[u("/a.md", 1), u("/b.md", 1), u("/a.md", 1)], &[]);
        assert_eq!(out.len(), 2);
        assert!((out[0].score - 1.0 / 61.0).abs() < 1e-12);
        assert_eq!(out[1].best_rank, 2);
    }

    #[test]
    fn fuse_units_is_stable_over_repeats() {
        let dense: Vec<_> = (1..=20).map(|n| u("/a.md", n)).collect();
        let keyword: Vec<_> = (1..=20).rev().map(|n| u("/a.md", n)).collect();
        let first = fuse_units(&dense, &keyword);
        for _ in 0..20 {
            assert_eq!(fuse_units(&dense, &keyword), first);
        }
    }

    #[test]
    fn empty_inputs_produce_empty_output() {
        assert!(rrf_merge(&[], &[]).is_empty());
    }

    #[test]
    fn ranks_are_assigned_starting_at_one() {
        let bm25 = vec![r("/a.md", 1.0, "x", 1)];
        let out = rrf_merge(&bm25, &[]);
        assert_eq!(out[0].rank, 1);
    }
}
