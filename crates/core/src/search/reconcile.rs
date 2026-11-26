//! The reconciliation of `search.documents` (SPEC-0110): the score of one
//! document for one input from the similarities of its chunks, and the weighted
//! mean over the inputs. Pure and deterministic, no I/O.
//!
//! Per input q: `S_q = m + b (1 - m) (1 - product(1 - s_i))`, `m` the best
//! similarity, `s_i` every other chunk at or above the floor `f` (a floor is a
//! minimum, so a chunk exactly at it counts, AUTO-017). With no other chunk
//! `S_q = m`; with `b = 0` it is the best passage score (MaxP).

use crate::errors::{Result, ToolError};

/// The score of one document for one input.
#[derive(Debug, Clone, PartialEq)]
pub struct InputScore {
    pub score: f64,
    /// Chunks that entered the formula: the best one plus the others at or above the floor.
    pub counted: usize,
    /// Index in the slice given of the best chunk (the first one on a tie), `None` for no chunk.
    pub best: Option<usize>,
}

/// `S_q` for the similarities `sims` of one document's chunks for one input
/// (already without the chunks that have no similarity, FR-NEW-008).
pub fn input_score(sims: &[f64], floor: f64, boost: f64) -> InputScore {
    let mut best: Option<usize> = None;
    for (i, s) in sims.iter().enumerate() {
        if best.is_none_or(|b| *s > sims[b]) {
            best = Some(i);
        }
    }
    let Some(b) = best else {
        return InputScore { score: 0.0, counted: 0, best: None };
    };
    let m = sims[b];
    let mut counted = 1;
    let mut miss = 1.0;
    for (i, s) in sims.iter().enumerate() {
        if i != b && *s >= floor {
            counted += 1;
            miss *= 1.0 - s;
        }
    }
    InputScore { score: m + boost * (1.0 - m) * (1.0 - miss), counted, best }
}

/// The weighted mean `sum(w * S) / sum(w)`; a missed input is a 0 in `scores`.
pub fn document_score(scores: &[f64], weights: &[f64]) -> f64 {
    let total: f64 = weights.iter().sum();
    scores.iter().zip(weights).map(|(s, w)| s * w).sum::<f64>() / total
}

/// FR-NEW-006: `floor` and `boost` in 0 to 1, and weights (one per input, none
/// negative, not all zero, all finite). Answers the weights to use: the given
/// ones, or 1 for every input.
pub fn check_params(
    floor: f64,
    boost: f64,
    weights: Option<&[f64]>,
    inputs: usize,
) -> Result<Vec<f64>> {
    for (name, v) in [("floor", floor), ("boost", boost)] {
        if !(0.0..=1.0).contains(&v) {
            return Err(ToolError::invalid_argument(format!("{name} must be between 0 and 1")));
        }
    }
    let Some(w) = weights else {
        return Ok(vec![1.0; inputs]);
    };
    if w.len() != inputs {
        return Err(ToolError::invalid_argument(format!(
            "weights must hold one number per input: {} given for {inputs} inputs",
            w.len()
        )));
    }
    if w.iter().any(|x| !x.is_finite() || *x < 0.0) {
        return Err(ToolError::invalid_argument("weights must be numbers of 0 or more"));
    }
    let total: f64 = w.iter().sum();
    if total <= 0.0 {
        return Err(ToolError::invalid_argument("weights must not all be 0"));
    }
    if !total.is_finite() {
        return Err(ToolError::invalid_argument("weights are too large"));
    }
    Ok(w.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-12, "{a} is not {b}");
    }

    /// m = 0.9 alone: nothing lifts it.
    #[test]
    fn the_best_chunk_alone_scores_its_similarity() {
        let s = input_score(&[0.9], 0.5, 0.5);
        close(s.score, 0.9);
        assert_eq!((s.counted, s.best), (1, Some(0)));
    }

    /// m = 0.9 and three chunks at 0.6: 0.9 + 0.5 * 0.1 * (1 - 0.4^3) = 0.9468.
    #[test]
    fn three_further_chunks_at_06_lift_09_to_09468() {
        let s = input_score(&[0.6, 0.9, 0.6, 0.6], 0.5, 0.5);
        close(s.score, 0.9468);
        assert_eq!((s.counted, s.best), (4, Some(1)));
    }

    /// Three chunks at 0.6: m = 0.6, two others: 0.6 + 0.5 * 0.4 * (1 - 0.16) = 0.768.
    #[test]
    fn three_chunks_at_06_score_0768() {
        let s = input_score(&[0.6, 0.6, 0.6], 0.5, 0.5);
        close(s.score, 0.768);
        assert_eq!((s.counted, s.best), (3, Some(0)));
    }

    /// The floor is a minimum: a chunk exactly at 0.5 counts, one below does not.
    #[test]
    fn a_chunk_exactly_at_the_floor_counts() {
        let at = input_score(&[0.8, 0.5], 0.5, 0.5);
        close(at.score, 0.8 + 0.5 * 0.2 * 0.5);
        assert_eq!(at.counted, 2);
        let below = input_score(&[0.8, 0.4999], 0.5, 0.5);
        close(below.score, 0.8);
        assert_eq!(below.counted, 1);
    }

    #[test]
    fn floor_zero_counts_everything_and_floor_one_only_perfect_chunks() {
        assert_eq!(input_score(&[0.9, 0.1, 0.0], 0.0, 0.5).counted, 3);
        assert_eq!(input_score(&[0.9, 0.99], 1.0, 0.5).counted, 1);
        assert_eq!(input_score(&[1.0, 1.0], 1.0, 0.5).counted, 2);
    }

    /// Two chunks at the highest similarity: one is m, the other lifts it.
    #[test]
    fn a_tie_for_the_best_gives_the_first_and_counts_the_other() {
        let s = input_score(&[0.2, 0.7, 0.7], 0.5, 0.5);
        assert_eq!(s.best, Some(1));
        close(s.score, 0.7 + 0.5 * 0.3 * 0.7);
        assert_eq!(s.counted, 2);
    }

    #[test]
    fn no_chunk_scores_zero() {
        let s = input_score(&[], 0.5, 0.5);
        assert_eq!((s.score, s.counted, s.best), (0.0, 0, None));
    }

    /// boost 0 is MaxP, boost 1 fills the whole gap by the others.
    #[test]
    fn the_boost_scales_the_lift() {
        close(input_score(&[0.6, 0.6, 0.6], 0.5, 0.0).score, 0.6);
        close(input_score(&[0.6, 0.6, 0.6], 0.5, 1.0).score, 0.6 + 0.4 * 0.84);
    }

    #[test]
    fn the_document_score_is_the_weighted_mean() {
        close(document_score(&[0.8, 0.0], &[1.0, 1.0]), 0.4);
        close(document_score(&[0.8, 0.0], &[1.0, 3.0]), 0.2);
        close(document_score(&[0.8, 0.4], &[5.0, 5.0]), document_score(&[0.8, 0.4], &[1.0, 1.0]));
    }

    #[test]
    fn parameters_are_checked() {
        assert_eq!(check_params(0.5, 0.5, None, 3).unwrap(), vec![1.0, 1.0, 1.0]);
        assert_eq!(check_params(0.0, 1.0, Some(&[2.0, 0.0]), 2).unwrap(), vec![2.0, 0.0]);
        for (f, b) in [(-0.1, 0.5), (1.1, 0.5), (f64::NAN, 0.5), (0.5, -0.1), (0.5, 1.1)] {
            assert!(check_params(f, b, None, 1).is_err(), "{f} {b}");
        }
        assert!(check_params(0.5, f64::NAN, None, 1).is_err());
        let bad: [&[f64]; 5] =
            [&[1.0], &[1.0, 1.0, 1.0], &[1.0, -1.0], &[0.0, 0.0], &[f64::NAN, 1.0]];
        for w in bad {
            assert!(check_params(0.5, 0.5, Some(w), 2).is_err(), "{w:?}");
        }
        assert!(check_params(0.5, 0.5, Some(&[f64::INFINITY, 1.0]), 2).is_err());
        // Finite weights whose sum overflows would give NaN scores (review finding).
        assert!(check_params(0.5, 0.5, Some(&[1e308, 1e308]), 2).is_err());
    }
}
