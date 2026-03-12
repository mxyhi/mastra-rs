use mastra_observability__test_utils::sample_trace_batch;
use mastra_observability_braintrust::{BraintrustConfig, BraintrustExporter};

#[test]
fn builds_braintrust_insert_request() {
    let exporter = BraintrustExporter::new(BraintrustConfig {
        api_key: "sk-test".to_string(),
        endpoint: "https://api.braintrust.dev".to_string(),
        project_id: "proj_123".to_string(),
    });

    let requests = exporter
        .build_requests(&sample_trace_batch())
        .expect("request planning should succeed");

    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].url.as_str(),
        "https://api.braintrust.dev/v1/project_logs/proj_123/insert"
    );
    assert_eq!(
        requests[0].headers.get("authorization"),
        Some(&"Bearer sk-test".to_string())
    );
    let body: serde_json::Value =
        serde_json::from_slice(&requests[0].body).expect("body should be valid json");
    assert_eq!(body["events"].as_array().map(Vec::len), Some(4));
}
