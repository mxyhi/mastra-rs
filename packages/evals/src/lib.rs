#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluationRun {
    pub input: String,
    pub output: String,
    pub reference: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScoreResult {
    pub scorer_id: String,
    pub score: f32,
    pub reason: String,
}

pub trait Scorer {
    fn score(&self, run: &EvaluationRun) -> ScoreResult;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactMatchScorer {
    pub ignore_case: bool,
}

impl Scorer for ExactMatchScorer {
    fn score(&self, run: &EvaluationRun) -> ScoreResult {
        let Some(reference) = &run.reference else {
            return ScoreResult {
                scorer_id: "exact-match".into(),
                score: 0.0,
                reason: "reference missing".into(),
            };
        };

        let left = normalize(&run.output, self.ignore_case);
        let right = normalize(reference, self.ignore_case);
        let passed = left == right;

        ScoreResult {
            scorer_id: "exact-match".into(),
            score: if passed { 1.0 } else { 0.0 },
            reason: if passed {
                "output matched reference".into()
            } else {
                "output differed from reference".into()
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeywordCoverageScorer {
    pub keywords: Vec<String>,
}

impl Scorer for KeywordCoverageScorer {
    fn score(&self, run: &EvaluationRun) -> ScoreResult {
        if self.keywords.is_empty() {
            return ScoreResult {
                scorer_id: "keyword-coverage".into(),
                score: 1.0,
                reason: "no keywords configured".into(),
            };
        }

        let output = run.output.to_ascii_lowercase();
        let matched = self
            .keywords
            .iter()
            .filter(|keyword| output.contains(&keyword.to_ascii_lowercase()))
            .count();
        let score = matched as f32 / self.keywords.len() as f32;

        ScoreResult {
            scorer_id: "keyword-coverage".into(),
            score,
            reason: format!("matched {matched}/{} keywords", self.keywords.len()),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EvaluationSummary {
    pub total: usize,
    pub passed: usize,
    pub average_score: f32,
}

pub fn summarize_scores(results: &[ScoreResult], pass_threshold: f32) -> EvaluationSummary {
    let total = results.len();
    let passed = results
        .iter()
        .filter(|result| result.score >= pass_threshold)
        .count();
    let average_score = if total == 0 {
        0.0
    } else {
        results.iter().map(|result| result.score).sum::<f32>() / total as f32
    };

    EvaluationSummary {
        total,
        passed,
        average_score,
    }
}

fn normalize(value: &str, ignore_case: bool) -> String {
    let value = value.trim();
    if ignore_case {
        value.to_ascii_lowercase()
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{EvaluationRun, ExactMatchScorer, KeywordCoverageScorer, Scorer, summarize_scores};

    #[test]
    fn exact_match_can_ignore_case() {
        let scorer = ExactMatchScorer { ignore_case: true };
        let result = scorer.score(&EvaluationRun {
            input: "question".into(),
            output: "PARIS".into(),
            reference: Some("paris".into()),
        });

        assert_eq!(result.score, 1.0);
    }

    #[test]
    fn keyword_coverage_scores_partial_matches() {
        let scorer = KeywordCoverageScorer {
            keywords: vec!["paris".into(), "france".into(), "river".into()],
        };
        let result = scorer.score(&EvaluationRun {
            input: String::new(),
            output: "Paris is the capital of France.".into(),
            reference: None,
        });

        assert!((result.score - (2.0 / 3.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn summary_counts_passed_results() {
        let summary = summarize_scores(
            &[
                ExactMatchScorer { ignore_case: false }.score(&EvaluationRun {
                    input: String::new(),
                    output: "a".into(),
                    reference: Some("a".into()),
                }),
                KeywordCoverageScorer {
                    keywords: vec!["a".into(), "b".into()],
                }
                .score(&EvaluationRun {
                    input: String::new(),
                    output: "a".into(),
                    reference: None,
                }),
            ],
            0.5,
        );

        assert_eq!(summary.total, 2);
        assert_eq!(summary.passed, 2);
        assert!((summary.average_score - 0.75).abs() < f32::EPSILON);
    }
}
