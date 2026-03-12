use mastra_observability__test_utils::sample_trace_batch;
use mastra_observability_langfuse::{LangfuseConfig, LangfuseExporter};

#[test]
fn builds_langfuse_ingestion_request() {
    let exporter = LangfuseExporter::new(LangfuseConfig {
        public_key: "pk-lf-test".to_string(),
        secret_key: "sk-lf-test".to_string(),
        base_url: "https://cloud.langfuse.com".to_string(),
    });

    let requests = exporter
        .build_requests(&sample_trace_batch())
        .expect("request planning should succeed");

    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].url.as_str(),
        "https://cloud.langfuse.com/api/public/ingestion"
    );
    assert!(
        requests[0]
            .headers
            .get("authorization")
            .expect("authorization header")
            .starts_with("Basic ")
    );

    let body: serde_json::Value =
        serde_json::from_slice(&requests[0].body).expect("body should be valid json");
    let batch = body["batch"].as_array().expect("batch should be an array");

    assert!(batch.iter().any(|item| item["type"] == "trace-create"));
    assert!(batch.iter().any(|item| item["type"] == "generation-create"));
    assert!(batch.iter().any(|item| item["type"] == "span-update"));
    assert!(batch.iter().any(|item| item["type"] == "event-create"));
}
