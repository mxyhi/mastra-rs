use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct MDocument {
    pub id: String,
    pub text: String,
    pub source: Option<String>,
    pub metadata: Value,
}

impl MDocument {
    pub fn new(id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            source: None,
            metadata: Value::Object(Default::default()),
        }
    }

    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    pub fn with_metadata(mut self, metadata: Value) -> Self {
        self.metadata = metadata;
        self
    }

    pub fn chunk(&self, options: &ChunkOptions) -> Result<Vec<DocumentChunk>, ChunkError> {
        chunk_document(self, options)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ChunkOptions {
    pub max_chars: usize,
    pub overlap_chars: usize,
}

impl ChunkOptions {
    pub const fn new(max_chars: usize, overlap_chars: usize) -> Self {
        Self {
            max_chars,
            overlap_chars,
        }
    }
}

impl Default for ChunkOptions {
    fn default() -> Self {
        Self {
            max_chars: 500,
            overlap_chars: 50,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct DocumentChunk {
    pub id: String,
    pub document_id: String,
    pub index: usize,
    pub text: String,
    pub start_char: usize,
    pub end_char: usize,
    pub metadata: Value,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ChunkError {
    #[error("document text cannot be empty")]
    EmptyDocument,
    #[error("chunk options are invalid: max_chars must be > 0 and overlap_chars must be smaller than max_chars")]
    InvalidOptions,
}

pub fn chunk_document(document: &MDocument, options: &ChunkOptions) -> Result<Vec<DocumentChunk>, ChunkError> {
    if document.text.is_empty() {
        return Err(ChunkError::EmptyDocument);
    }

    if options.max_chars == 0 || options.overlap_chars >= options.max_chars {
        return Err(ChunkError::InvalidOptions);
    }

    let total_chars = document.text.chars().count();
    let mut start_char = 0;
    let mut chunks = Vec::new();
    let step = options.max_chars - options.overlap_chars;

    while start_char < total_chars {
        let end_char = usize::min(start_char + options.max_chars, total_chars);
        let text = slice_chars(&document.text, start_char, end_char);
        let index = chunks.len();

        chunks.push(DocumentChunk {
            id: format!("{}#chunk-{index}", document.id),
            document_id: document.id.clone(),
            index,
            text,
            start_char,
            end_char,
            metadata: document.metadata.clone(),
        });

        if end_char == total_chars {
            break;
        }

        start_char += step;
    }

    Ok(chunks)
}

fn slice_chars(text: &str, start_char: usize, end_char: usize) -> String {
    // The Rust port walks character boundaries once per slice so UTF-8 input
    // is chunked deterministically without risking invalid byte offsets.
    let start_byte = char_index_to_byte_offset(text, start_char);
    let end_byte = char_index_to_byte_offset(text, end_char);
    text[start_byte..end_byte].to_string()
}

fn char_index_to_byte_offset(text: &str, char_index: usize) -> usize {
    if char_index == 0 {
        return 0;
    }

    text.char_indices()
        .nth(char_index)
        .map(|(byte_index, _)| byte_index)
        .unwrap_or(text.len())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{chunk_document, ChunkError, ChunkOptions, MDocument};

    #[test]
    fn chunk_document_splits_text_with_overlap() {
        let document = MDocument::new("doc-1", "abcdefghij").with_metadata(json!({ "lang": "en" }));
        let chunks = chunk_document(&document, &ChunkOptions::new(4, 1)).expect("chunking should work");

        let payload = chunks.into_iter().map(|chunk| chunk.text).collect::<Vec<_>>();
        assert_eq!(payload, vec!["abcd", "defg", "ghij"]);
    }

    #[test]
    fn chunk_document_handles_unicode_char_boundaries() {
        let document = MDocument::new("doc-2", "你好世界欢迎你");
        let chunks = document.chunk(&ChunkOptions::new(3, 1)).expect("chunking should work");

        let payload = chunks.into_iter().map(|chunk| chunk.text).collect::<Vec<_>>();
        assert_eq!(payload, vec!["你好世", "世界欢", "欢迎你"]);
    }

    #[test]
    fn chunk_document_rejects_invalid_options() {
        let document = MDocument::new("doc-3", "abc");
        let error = document
            .chunk(&ChunkOptions::new(2, 2))
            .expect_err("overlap equal to chunk size must be rejected");

        assert_eq!(error, ChunkError::InvalidOptions);
    }
}
