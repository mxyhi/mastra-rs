use mastra_observability__test_utils::sample_trace_batch;
use mastra_observability_datadog::{DatadogConfig, DatadogExporter};

#[test]
fn builds_datadog_span_requests() {
    let exporter = DatadogExporter::new(DatadogConfig {
        api_key: "dd-api-key".to_string(),
        site: "datadoghq.com".to_string(),
        ml_app: "mastra-rs".to_string(),
        env: Some("test".to_string()),
    });

    let requests = exporter
        .build_requests(&sample_trace_batch())
        .expect("request planning should succeed");

    assert_eq!(requests.len(), 4);
    assert_eq!(
        requests[0].url.as_str(),
        "https://api.datadoghq.com/api/intake/llm-obs/v1/trace/spans"
    );
    assert_eq!(
        requests[0].headers.get("dd-api-key"),
        Some(&"dd-api-key".to_string())
    );

    let body: serde_json::Value =
        serde_json::from_slice(&requests[1].body).expect("body should be valid json");
    assert_eq!(body["data"]["type"], "span");
    assert_eq!(body["data"]["attributes"]["ml_app"], "mastra-rs");
    assert_eq!(body["data"]["attributes"]["kind"], "llm");
}
