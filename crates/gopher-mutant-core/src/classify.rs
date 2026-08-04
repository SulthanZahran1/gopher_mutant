//! Classification: map a mutant's test run into one of the five outcome
//! buckets, and the overall report model shared by console + JSON.

use serde::Serialize;

/// The five outcome buckets (GOAL-1 contract).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// Tests failed under the mutant.
    Killed,
    /// Tests passed and the mutant was covered.
    Survived,
    /// Tests passed and the mutant was NOT covered.
    NotCovered,
    /// The patched source fails to compile.
    CompileError,
    /// The test run exceeded the per-mutant timeout.
    Timeout,
}

impl Outcome {
    pub fn as_str(&self) -> &'static str {
        match self {
            Outcome::Killed => "killed",
            Outcome::Survived => "survived",
            Outcome::NotCovered => "not_covered",
            Outcome::CompileError => "compile_error",
            Outcome::Timeout => "timeout",
        }
    }
}

/// Classification of one mutant run.
#[derive(Debug, Clone, Serialize)]
pub struct Classification {
    pub id: usize,
    pub file: String,
    pub line: usize,
    pub column: usize,
    pub operator: String,
    pub label: String,
    pub outcome: Outcome,
    /// Wall-clock ms of the test run (0 for compile_error).
    pub duration_ms: u128,
    /// Whether the mutant was covered by the suite's coverage profile.
    pub covered: bool,
}

/// Aggregated run report.
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub total: usize,
    pub killed: usize,
    pub survived: usize,
    pub not_covered: usize,
    pub compile_error: usize,
    pub timeout: usize,
    /// Mutation score (MSI) as a percentage, computed as
    /// killed / (killed + survived + not_covered), per GOAL-1.
    pub mutation_score: f64,
    pub elapsed_ms: u128,
    pub classifications: Vec<Classification>,
    /// Non-zero when MSI < threshold (exit code 1).
    pub below_threshold: bool,
    pub threshold: f64,
}

impl Report {
    pub fn new(classifications: Vec<Classification>, elapsed_ms: u128, threshold: f64) -> Self {
        let mut killed = 0;
        let mut survived = 0;
        let mut not_covered = 0;
        let mut compile_error = 0;
        let mut timeout = 0;
        for c in &classifications {
            match c.outcome {
                Outcome::Killed => killed += 1,
                Outcome::Survived => survived += 1,
                Outcome::NotCovered => not_covered += 1,
                Outcome::CompileError => compile_error += 1,
                Outcome::Timeout => timeout += 1,
            }
        }
        let total = classifications.len();
        let scoreable = killed + survived + not_covered;
        let mutation_score = if scoreable == 0 {
            0.0
        } else {
            killed as f64 / scoreable as f64 * 100.0
        };
        let below_threshold = total > 0 && mutation_score < threshold;
        Report {
            total,
            killed,
            survived,
            not_covered,
            compile_error,
            timeout,
            mutation_score,
            elapsed_ms,
            classifications,
            below_threshold,
            threshold,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cls(id: usize, outcome: Outcome) -> Classification {
        Classification {
            id,
            file: "f.go".into(),
            line: 1,
            column: 1,
            operator: "AOR".into(),
            label: "x".into(),
            outcome,
            duration_ms: 1,
            covered: true,
        }
    }

    #[test]
    fn report_sums_buckets() {
        let cs = vec![
            cls(0, Outcome::Killed),
            cls(1, Outcome::Killed),
            cls(2, Outcome::Survived),
            cls(3, Outcome::NotCovered),
            cls(4, Outcome::CompileError),
            cls(5, Outcome::Timeout),
        ];
        let r = Report::new(cs, 100, 80.0);
        assert_eq!(r.total, 6);
        assert_eq!(r.killed, 2);
        assert_eq!(r.survived, 1);
        assert_eq!(r.not_covered, 1);
        assert_eq!(r.compile_error, 1);
        assert_eq!(r.timeout, 1);
        // MSI = 2 / (2+1+1) = 50%
        assert!((r.mutation_score - 50.0).abs() < 1e-9);
        assert!(r.below_threshold);
    }

    #[test]
    fn report_full_kill_passes_threshold() {
        let cs = vec![cls(0, Outcome::Killed), cls(1, Outcome::Killed)];
        let r = Report::new(cs, 10, 80.0);
        assert_eq!(r.mutation_score, 100.0);
        assert!(!r.below_threshold);
    }

    #[test]
    fn outcome_names() {
        assert_eq!(Outcome::Killed.as_str(), "killed");
        assert_eq!(Outcome::Survived.as_str(), "survived");
        assert_eq!(Outcome::NotCovered.as_str(), "not_covered");
        assert_eq!(Outcome::CompileError.as_str(), "compile_error");
        assert_eq!(Outcome::Timeout.as_str(), "timeout");
    }
}
