use std::collections::VecDeque;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmCall {
    pub prompt: String,
    pub response: String,
    pub provider: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MockError {
    ResponseQueueExhausted { prompt: String },
    PendingResponses { remaining: usize },
}

#[derive(Debug, Clone, Default)]
pub struct LlmMock {
    responses: VecDeque<String>,
    calls: Vec<LlmCall>,
    provider: Option<String>,
    model: Option<String>,
}

impl LlmMock {
    pub fn new<I, S>(responses: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            responses: responses.into_iter().map(Into::into).collect(),
            calls: Vec::new(),
            provider: None,
            model: None,
        }
    }

    pub fn with_identity(mut self, provider: impl Into<String>, model: impl Into<String>) -> Self {
        self.provider = Some(provider.into());
        self.model = Some(model.into());
        self
    }

    pub fn complete(&mut self, prompt: impl Into<String>) -> Result<String, MockError> {
        let prompt = prompt.into();
        let Some(response) = self.responses.pop_front() else {
            return Err(MockError::ResponseQueueExhausted { prompt });
        };

        self.calls.push(LlmCall {
            prompt,
            response: response.clone(),
            provider: self.provider.clone(),
            model: self.model.clone(),
        });

        Ok(response)
    }

    pub fn calls(&self) -> &[LlmCall] {
        &self.calls
    }

    pub fn pending_responses(&self) -> usize {
        self.responses.len()
    }

    pub fn assert_exhausted(&self) -> Result<(), MockError> {
        if self.responses.is_empty() {
            Ok(())
        } else {
            Err(MockError::PendingResponses {
                remaining: self.responses.len(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{LlmCall, LlmMock, MockError};

    #[test]
    fn records_calls_in_order_with_identity() {
        let mut mock = LlmMock::new(["alpha", "beta"]).with_identity("openai", "gpt-4o-mini");

        let first = mock.complete("hello").expect("first response should exist");
        let second = mock
            .complete("world")
            .expect("second response should exist");

        assert_eq!(first, "alpha");
        assert_eq!(second, "beta");
        assert_eq!(
            mock.calls(),
            &[
                LlmCall {
                    prompt: "hello".into(),
                    response: "alpha".into(),
                    provider: Some("openai".into()),
                    model: Some("gpt-4o-mini".into()),
                },
                LlmCall {
                    prompt: "world".into(),
                    response: "beta".into(),
                    provider: Some("openai".into()),
                    model: Some("gpt-4o-mini".into()),
                },
            ]
        );
    }

    #[test]
    fn errors_when_queue_is_exhausted() {
        let mut mock = LlmMock::new(["only-once"]);
        let _ = mock.complete("first").expect("seed response should exist");

        let err = mock
            .complete("second")
            .expect_err("queue exhaustion should fail");

        assert_eq!(
            err,
            MockError::ResponseQueueExhausted {
                prompt: "second".into(),
            }
        );
    }

    #[test]
    fn assert_exhausted_reports_remaining_count() {
        let mock = LlmMock::new(["a", "b", "c"]);

        let err = mock
            .assert_exhausted()
            .expect_err("unconsumed responses should be reported");

        assert_eq!(err, MockError::PendingResponses { remaining: 3 });
    }
}
