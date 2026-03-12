use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum QuestionType {
    SingleSessionUser,
    SingleSessionAssistant,
    MultiSession,
    TemporalReasoning,
    KnowledgeUpdate,
    SingleSessionPreference,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryConfig {
    pub id: &'static str,
    pub description: &'static str,
    pub uses_semantic_recall: bool,
    pub uses_observational_memory: bool,
    pub requires_sequential: bool,
}

const MEMORY_CONFIGS: &[MemoryConfig] = &[
    MemoryConfig {
        id: "baseline",
        description: "Recent-message baseline without recall",
        uses_semantic_recall: false,
        uses_observational_memory: false,
        requires_sequential: false,
    },
    MemoryConfig {
        id: "observational-memory",
        description: "Observer/reflector memory flow",
        uses_semantic_recall: false,
        uses_observational_memory: true,
        requires_sequential: true,
    },
    MemoryConfig {
        id: "semantic-recall",
        description: "Semantic retrieval over historical memory",
        uses_semantic_recall: true,
        uses_observational_memory: false,
        requires_sequential: false,
    },
];

pub fn available_configs() -> Vec<&'static str> {
    MEMORY_CONFIGS.iter().map(|config| config.id).collect()
}

pub fn get_memory_config(id: &str) -> Option<&'static MemoryConfig> {
    MEMORY_CONFIGS.iter().find(|config| config.id == id)
}

#[derive(Debug, Clone, PartialEq)]
pub struct EvalResult {
    pub question_type: QuestionType,
    pub score: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EvalSummary {
    pub total: usize,
    pub overall_accuracy: f32,
    pub by_type: BTreeMap<QuestionType, f32>,
}

pub fn summarize_results(results: &[EvalResult]) -> EvalSummary {
    let total = results.len();
    let overall_accuracy = if total == 0 {
        0.0
    } else {
        results.iter().map(|result| result.score).sum::<f32>() / total as f32
    };

    let mut grouped: BTreeMap<QuestionType, (f32, usize)> = BTreeMap::new();
    for result in results {
        let entry = grouped.entry(result.question_type).or_insert((0.0, 0));
        entry.0 += result.score;
        entry.1 += 1;
    }

    let by_type = grouped
        .into_iter()
        .map(|(question_type, (score_sum, count))| (question_type, score_sum / count as f32))
        .collect();

    EvalSummary {
        total,
        overall_accuracy,
        by_type,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        EvalResult, QuestionType, available_configs, get_memory_config, summarize_results,
    };

    #[test]
    fn exposes_known_memory_configs() {
        assert!(available_configs().contains(&"observational-memory"));
        assert!(
            get_memory_config("semantic-recall")
                .expect("config should exist")
                .uses_semantic_recall
        );
    }

    #[test]
    fn summarizes_scores_overall_and_by_type() {
        let summary = summarize_results(&[
            EvalResult {
                question_type: QuestionType::MultiSession,
                score: 1.0,
            },
            EvalResult {
                question_type: QuestionType::MultiSession,
                score: 0.0,
            },
            EvalResult {
                question_type: QuestionType::KnowledgeUpdate,
                score: 1.0,
            },
        ]);

        assert_eq!(summary.total, 3);
        assert!((summary.overall_accuracy - (2.0 / 3.0)).abs() < f32::EPSILON);
        assert!((summary.by_type[&QuestionType::MultiSession] - 0.5).abs() < f32::EPSILON);
    }
}
