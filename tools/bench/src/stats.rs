//! Median, nearest-rank p95 and geometric-mean ratio (spec items 5 and 8).

/// Median of `xs` (sorted in place); the mean of the two middle values for even counts.
pub fn median(xs: &mut [u64]) -> Option<f64> {
    xs.sort_unstable();
    let n = xs.len();
    let mid = n / 2;
    if n == 0 {
        None
    } else if n % 2 == 1 {
        xs.get(mid).map(|&x| x as f64)
    } else {
        let a = *xs.get(mid.checked_sub(1)?)? as f64;
        let b = *xs.get(mid)? as f64;
        Some((a + b) / 2.0)
    }
}

/// Median of floating-point values (NaN-free input; NaNs sort last).
pub fn median_f64(xs: &mut [f64]) -> Option<f64> {
    xs.sort_unstable_by(f64::total_cmp);
    let n = xs.len();
    let mid = n / 2;
    if n == 0 {
        None
    } else if n % 2 == 1 {
        xs.get(mid).copied()
    } else {
        Some((xs.get(mid.checked_sub(1)?)? + xs.get(mid)?) / 2.0)
    }
}

/// Nearest-rank 95th percentile: the value at rank ⌈0.95·n⌉ (1-based) of the sorted values.
pub fn p95(xs: &mut [f64]) -> Option<f64> {
    xs.sort_unstable_by(f64::total_cmp);
    let n = xs.len();
    // ⌈95n/100⌉ without floating point; n ≤ the sample cap, so no overflow, but saturate anyway.
    let rank = n.saturating_mul(95).div_ceil(100);
    xs.get(rank.checked_sub(1)?).copied()
}

/// Geometric mean of `max(ours, 1) / max(reference, 1)` over the pairs (µs), or `None`
/// when there are none. Same shape as the judge's `bench_ratio`.
pub fn ratio(pairs: &[(f64, f64)]) -> Option<f64> {
    if pairs.is_empty() {
        return None;
    }
    let sum: f64 = pairs
        .iter()
        .map(|&(ours, reference)| (ours.max(1.0) / reference.max(1.0)).ln())
        .sum();
    Some((sum / pairs.len() as f64).exp())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn medians() {
        assert_eq!(median(&mut []), None);
        assert_eq!(median(&mut [7]), Some(7.0));
        assert_eq!(median(&mut [3, 1, 2]), Some(2.0));
        assert_eq!(median(&mut [4, 1, 3, 2]), Some(2.5));
        assert_eq!(median(&mut [u64::MAX, u64::MAX]), Some(u64::MAX as f64));
        assert_eq!(median_f64(&mut []), None);
        assert_eq!(median_f64(&mut [5.0, 1.0, 3.0]), Some(3.0));
        assert_eq!(median_f64(&mut [4.0, 1.0, 3.0, 2.0]), Some(2.5));
    }

    #[test]
    fn nearest_rank_p95() {
        let seq = |n: usize| (1..=n).rev().map(|i| i as f64).collect::<Vec<_>>();
        assert_eq!(p95(&mut []), None);
        assert_eq!(p95(&mut seq(1)), Some(1.0));
        // ⌈0.95·19⌉ = 19, ⌈0.95·20⌉ = 19, ⌈0.95·21⌉ = 20.
        assert_eq!(p95(&mut seq(19)), Some(19.0));
        assert_eq!(p95(&mut seq(20)), Some(19.0));
        assert_eq!(p95(&mut seq(21)), Some(20.0));
        assert_eq!(p95(&mut seq(100)), Some(95.0));
    }

    #[test]
    fn geometric_mean_ratio() {
        assert_eq!(ratio(&[]), None);
        let r = ratio(&[(200.0, 100.0), (50.0, 100.0)]).unwrap();
        assert!((r - 1.0).abs() < 1e-12, "{r}");
        let r = ratio(&[(400.0, 100.0), (100.0, 100.0)]).unwrap();
        assert!((r - 2.0).abs() < 1e-12, "{r}");
        // Sub-microsecond times clamp to 1 µs on both sides.
        assert_eq!(ratio(&[(0.0, 0.5)]), Some(1.0));
        let r = ratio(&[(0.2, 8.0)]).unwrap();
        assert!((r - 0.125).abs() < 1e-12, "{r}");
    }
}
