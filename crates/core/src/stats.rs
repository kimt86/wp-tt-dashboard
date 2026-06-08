//! Paired statistics for KPI baselines: paired t-test and Cohen's d.
//!
//! Used by the transform stage (L1 -> L2) to compare a current-period sample
//! against a baseline-period sample at a matched (vessel/shift) grain.

use statrs::distribution::{ContinuousCDF, StudentsT};

#[derive(Debug, Clone, PartialEq)]
pub struct PairedTest {
    pub n: usize,
    pub mean_diff: f64,
    pub sd_diff: f64,
    pub t: f64,
    pub df: f64,
    pub p_value: f64, // two-sided
    pub cohens_d: f64,
}

/// Paired t-test of `current` vs `baseline` (must be the same length, matched pairs).
/// Returns `None` if fewer than 2 usable pairs or zero variance in the differences.
pub fn paired_t_test(current: &[f64], baseline: &[f64]) -> Option<PairedTest> {
    if current.len() != baseline.len() {
        return None;
    }
    let diffs: Vec<f64> = current
        .iter()
        .zip(baseline.iter())
        .map(|(c, b)| c - b)
        .collect();
    let n = diffs.len();
    if n < 2 {
        return None;
    }
    let mean = diffs.iter().sum::<f64>() / n as f64;
    let var = diffs.iter().map(|d| (d - mean).powi(2)).sum::<f64>() / (n as f64 - 1.0);
    let sd = var.sqrt();
    if sd == 0.0 || !sd.is_finite() {
        return None;
    }
    let se = sd / (n as f64).sqrt();
    let t = mean / se;
    let df = n as f64 - 1.0;
    let dist = StudentsT::new(0.0, 1.0, df).ok()?;
    // two-sided p-value
    let p = 2.0 * (1.0 - dist.cdf(t.abs()));
    let cohens_d = mean / sd; // paired Cohen's d = mean(diff) / sd(diff)
    Some(PairedTest {
        n,
        mean_diff: mean,
        sd_diff: sd,
        t,
        df,
        p_value: p.clamp(0.0, 1.0),
        cohens_d,
    })
}

/// Simple mean helper.
pub fn mean(xs: &[f64]) -> Option<f64> {
    if xs.is_empty() {
        None
    } else {
        Some(xs.iter().sum::<f64>() / xs.len() as f64)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TwoSampleTest {
    pub n_a: usize,
    pub n_b: usize,
    pub mean_a: f64,
    pub mean_b: f64,
    pub t: f64,
    pub df: f64,
    pub p_value: f64, // two-sided
    pub cohens_d: f64, // pooled
}

fn mean_var(xs: &[f64]) -> (f64, f64) {
    let n = xs.len() as f64;
    let m = xs.iter().sum::<f64>() / n;
    let v = xs.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (n - 1.0);
    (m, v)
}

/// Welch's two-sample t-test (unequal variances), comparing sample `a` (e.g. the
/// recent window) against `b` (the baseline window). Returns `None` if either side
/// has < 2 points or both variances are zero.
pub fn welch_t_test(a: &[f64], b: &[f64]) -> Option<TwoSampleTest> {
    if a.len() < 2 || b.len() < 2 {
        return None;
    }
    let (ma, va) = mean_var(a);
    let (mb, vb) = mean_var(b);
    let (na, nb) = (a.len() as f64, b.len() as f64);
    let se2 = va / na + vb / nb;
    if se2 <= 0.0 || !se2.is_finite() {
        return None;
    }
    let t = (ma - mb) / se2.sqrt();
    // Welch–Satterthwaite degrees of freedom
    let df = se2.powi(2)
        / ((va / na).powi(2) / (na - 1.0) + (vb / nb).powi(2) / (nb - 1.0));
    let dist = StudentsT::new(0.0, 1.0, df).ok()?;
    let p = (2.0 * (1.0 - dist.cdf(t.abs()))).clamp(0.0, 1.0);
    // pooled SD for Cohen's d
    let pooled = (((na - 1.0) * va + (nb - 1.0) * vb) / (na + nb - 2.0)).sqrt();
    let cohens_d = if pooled > 0.0 { (ma - mb) / pooled } else { 0.0 };
    Some(TwoSampleTest {
        n_a: a.len(),
        n_b: b.len(),
        mean_a: ma,
        mean_b: mb,
        t,
        df,
        p_value: p,
        cohens_d,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_paired_sample() {
        // current consistently ~0.12 below baseline, with small per-pair variation
        // (an improvement for a LOWER_BETTER KPI).
        let baseline = [0.96, 0.94, 0.98, 0.95, 0.97, 0.93, 0.99, 0.96];
        let current = [0.84, 0.83, 0.85, 0.84, 0.84, 0.82, 0.86, 0.85];
        let r = paired_t_test(&current, &baseline).unwrap();
        assert_eq!(r.n, 8);
        // mean diff ~ -0.119
        assert!(r.mean_diff < -0.10 && r.mean_diff > -0.13, "mean_diff={}", r.mean_diff);
        // strongly consistent improvement -> tiny p-value
        assert!(r.p_value < 0.001, "p={}", r.p_value);
        // large effect size
        assert!(r.cohens_d.abs() > 2.0, "d={}", r.cohens_d);
    }

    #[test]
    fn zero_variance_returns_none() {
        let a = [1.0, 1.0, 1.0];
        let b = [1.0, 1.0, 1.0];
        assert!(paired_t_test(&a, &b).is_none());
    }

    #[test]
    fn mismatched_lengths_none() {
        assert!(paired_t_test(&[1.0, 2.0], &[1.0]).is_none());
    }

    #[test]
    fn too_few_pairs_none() {
        assert!(paired_t_test(&[1.0], &[2.0]).is_none());
    }

    #[test]
    fn welch_detects_clear_difference() {
        // recent week clearly lower than the baseline weeks
        let recent = [0.84, 0.83, 0.85, 0.84, 0.82, 0.86, 0.85];
        let baseline = [0.95, 0.96, 0.94, 0.97, 0.93, 0.96, 0.95, 0.94, 0.98, 0.96];
        let r = welch_t_test(&recent, &baseline).unwrap();
        assert_eq!((r.n_a, r.n_b), (7, 10));
        assert!(r.mean_a < r.mean_b);
        assert!(r.p_value < 0.001, "p={}", r.p_value);
        assert!(r.cohens_d.abs() > 2.0, "d={}", r.cohens_d);
    }

    #[test]
    fn welch_too_few_none() {
        assert!(welch_t_test(&[1.0], &[1.0, 2.0, 3.0]).is_none());
    }
}
