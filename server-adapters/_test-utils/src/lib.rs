use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteRequest {
    pub method: HttpMethod,
    pub path: String,
    pub query: BTreeMap<String, String>,
    pub headers: BTreeMap<String, String>,
    pub body: Option<String>,
}

impl RouteRequest {
    pub fn new(method: HttpMethod, path: impl Into<String>) -> Self {
        Self {
            method,
            path: path.into(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: None,
        }
    }

    pub fn query(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.query.insert(key.into(), value.into());
        self
    }

    pub fn header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers
            .insert(key.into().to_ascii_lowercase(), value.into());
        self
    }

    pub fn body(mut self, value: impl Into<String>) -> Self {
        self.body = Some(value.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteResponse {
    pub status: u16,
    pub headers: BTreeMap<String, String>,
    pub body: String,
}

impl RouteResponse {
    pub fn new(status: u16, body: impl Into<String>) -> Self {
        Self {
            status,
            headers: BTreeMap::new(),
            body: body.into(),
        }
    }

    pub fn header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers
            .insert(key.into().to_ascii_lowercase(), value.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResponseExpectation {
    status: u16,
    required_headers: BTreeMap<String, String>,
    body_fragments: Vec<String>,
}

impl ResponseExpectation {
    pub fn new(status: u16) -> Self {
        Self {
            status,
            required_headers: BTreeMap::new(),
            body_fragments: Vec::new(),
        }
    }

    pub fn header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.required_headers
            .insert(key.into().to_ascii_lowercase(), value.into());
        self
    }

    pub fn body_contains(mut self, fragment: impl Into<String>) -> Self {
        self.body_fragments.push(fragment.into());
        self
    }

    pub fn verify(&self, response: &RouteResponse) -> Result<(), ResponseMismatch> {
        if response.status != self.status {
            return Err(ResponseMismatch::Status {
                expected: self.status,
                actual: response.status,
            });
        }

        for (key, expected) in &self.required_headers {
            let actual = response.headers.get(key);
            if actual != Some(expected) {
                return Err(ResponseMismatch::Header {
                    key: key.clone(),
                    expected: expected.clone(),
                    actual: actual.cloned(),
                });
            }
        }

        for fragment in &self.body_fragments {
            if !response.body.contains(fragment) {
                return Err(ResponseMismatch::BodyFragment {
                    expected_fragment: fragment.clone(),
                });
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResponseMismatch {
    Status {
        expected: u16,
        actual: u16,
    },
    Header {
        key: String,
        expected: String,
        actual: Option<String>,
    },
    BodyFragment {
        expected_fragment: String,
    },
}

#[cfg(test)]
mod tests {
    use super::{HttpMethod, ResponseExpectation, ResponseMismatch, RouteRequest, RouteResponse};

    #[test]
    fn builds_request_with_normalized_headers() {
        let request = RouteRequest::new(HttpMethod::Post, "/mcp/tools")
            .query("cursor", "abc")
            .header("Content-Type", "application/json")
            .body("{\"name\":\"weather\"}");

        assert_eq!(request.method, HttpMethod::Post);
        assert_eq!(request.path, "/mcp/tools");
        assert_eq!(request.query.get("cursor"), Some(&"abc".to_string()));
        assert_eq!(
            request.headers.get("content-type"),
            Some(&"application/json".to_string())
        );
        assert_eq!(request.body.as_deref(), Some("{\"name\":\"weather\"}"));
    }

    #[test]
    fn verifies_status_headers_and_body_fragments() {
        let response = RouteResponse::new(200, "{\"ok\":true,\"id\":\"tool_1\"}")
            .header("Content-Type", "application/json");

        let expectation = ResponseExpectation::new(200)
            .header("content-type", "application/json")
            .body_contains("\"ok\":true")
            .body_contains("\"id\":\"tool_1\"");

        assert_eq!(expectation.verify(&response), Ok(()));
    }

    #[test]
    fn reports_first_detected_mismatch() {
        let response = RouteResponse::new(202, "{\"ok\":false}");
        let expectation = ResponseExpectation::new(200);

        let mismatch = expectation
            .verify(&response)
            .expect_err("status mismatch should fail");

        assert_eq!(
            mismatch,
            ResponseMismatch::Status {
                expected: 200,
                actual: 202,
            }
        );
    }
}
