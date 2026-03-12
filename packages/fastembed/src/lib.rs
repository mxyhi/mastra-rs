use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FastEmbedModel {
    pub model_id: String,
    pub dimensions: usize,
}

impl FastEmbedModel {
    pub fn small() -> Self {
        Self {
            model_id: "bge-small-en-v1.5".into(),
            dimensions: 384,
        }
    }

    pub fn base() -> Self {
        Self {
            model_id: "bge-base-en-v1.5".into(),
            dimensions: 768,
        }
    }

    pub fn embed(&self, text: &str) -> Vec<f32> {
        let mut vector = vec![0.0; self.dimensions];

        for token in tokenize(text) {
            let hash = stable_hash(&token);
            let index = hash as usize % self.dimensions;
            let sign = if hash & 1 == 0 { 1.0 } else { -1.0 };
            vector[index] += sign;
        }

        normalize_vector(vector)
    }

    pub fn embed_batch<'a, I>(&self, values: I) -> Vec<Vec<f32>>
    where
        I: IntoIterator<Item = &'a str>,
    {
        values.into_iter().map(|value| self.embed(value)).collect()
    }
}

pub fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    let dot = left
        .iter()
        .zip(right.iter())
        .map(|(left, right)| left * right)
        .sum::<f32>();
    let left_norm = left.iter().map(|value| value * value).sum::<f32>().sqrt();
    let right_norm = right.iter().map(|value| value * value).sum::<f32>().sqrt();

    if left_norm == 0.0 || right_norm == 0.0 {
        0.0
    } else {
        dot / (left_norm * right_norm)
    }
}

fn tokenize(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|token| token.to_ascii_lowercase())
        .collect()
}

fn stable_hash(value: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn normalize_vector(mut vector: Vec<f32>) -> Vec<f32> {
    let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm == 0.0 {
        return vector;
    }

    for value in &mut vector {
        *value /= norm;
    }
    vector
}

#[cfg(test)]
mod tests {
    use super::{FastEmbedModel, cosine_similarity};

    #[test]
    fn embeddings_are_deterministic() {
        let model = FastEmbedModel::small();

        assert_eq!(
            model.embed("weather forecast"),
            model.embed("weather forecast")
        );
    }

    #[test]
    fn similar_texts_score_higher_than_unrelated_texts() {
        let model = FastEmbedModel::small();
        let forecast = model.embed("weather forecast tomorrow");
        let forecast_variant = model.embed("tomorrow weather forecast");
        let finance = model.embed("quarterly revenue guidance");

        assert!(
            cosine_similarity(&forecast, &forecast_variant)
                > cosine_similarity(&forecast, &finance)
        );
    }

    #[test]
    fn models_have_different_dimensions() {
        assert_ne!(
            FastEmbedModel::small().dimensions,
            FastEmbedModel::base().dimensions
        );
    }
}
