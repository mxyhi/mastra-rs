use std::collections::BTreeMap;

use mastra_observability__test_utils::{TestHttpServer, sample_trace_batch};
use mastra_observability_mastra::{
    ExportError, HttpExporter, HttpMethod, HttpRequest, HttpRequestBuilder, TraceBatch,
};
use url::Url;

struct StaticRequestBuilder {
    url: Url,
}

impl HttpRequestBuilder for StaticRequestBuilder {
    fn build_requests(&self, batch: &TraceBatch) -> Result<Vec<HttpRequest>, ExportError> {
        Ok(vec![HttpRequest {
            method: HttpMethod::Post,
            url: self.url.clone(),
            headers: BTreeMap::from([("content-type".to_string(), "application/json".to_string())]),
            body: serde_json::to_vec(batch)?,
        }])
    }
}

#[test]
fn trace_batch_serializes_usage_and_events() {
    let batch = sample_trace_batch();
    let value = serde_json::to_value(&batch).expect("batch should serialize");

    assert_eq!(value["service_name"], "mastra-rs-tests");
    assert_eq!(value["spans"].as_array().map(Vec::len), Some(3));
    assert_eq!(value["spans"][1]["kind"], "model_generation");
    assert_eq!(value["spans"][1]["usage"]["input_tokens"], 21);
    assert_eq!(value["spans"][1]["events"][0]["name"], "first_token");
}

#[tokio::test]
async fn http_exporter_posts_built_requests() {
    let server = TestHttpServer::start(200, "ok").await;
    let exporter = HttpExporter::new(StaticRequestBuilder {
        url: server.url("/ingest"),
    });

    exporter
        .export(&sample_trace_batch())
        .await
        .expect("export should succeed");

    let request = server.recv().await;
    assert_eq!(request.method, "POST");
    assert_eq!(request.path, "/ingest");
    assert_eq!(request.header("content-type"), Some("application/json"));

    let value: serde_json::Value =
        serde_json::from_slice(&request.body).expect("request body should be json");
    assert_eq!(value["spans"][0]["name"], "agent.run");
}

#[tokio::test]
async fn http_exporter_returns_status_errors() {
    let server = TestHttpServer::start(502, "bad gateway").await;
    let exporter = HttpExporter::new(StaticRequestBuilder {
        url: server.url("/ingest"),
    });

    let error = exporter
        .export(&sample_trace_batch())
        .await
        .expect_err("non-success status should fail");

    match error {
        ExportError::UnexpectedStatus {
            status_code,
            response_body,
            ..
        } => {
            assert_eq!(status_code, 502);
            assert!(response_body.contains("bad gateway"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
