use mastra_observability__test_utils::sample_trace_batch;
use mastra_observability_otel_exporter::{OtelConfig, OtelExporter};
use serde_json::json;

#[test]
fn builds_otlp_json_request() {
    let exporter = OtelExporter::new(OtelConfig {
        endpoint: "https://collector.example.com/v1/traces".to_string(),
        headers: Default::default(),
        resource_attributes: std::collections::BTreeMap::from([(
            "deployment.region".to_string(),
            json!("test"),
        )]),
    });

    let requests = exporter
        .build_requests(&sample_trace_batch())
        .expect("request planning should succeed");

    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].url.as_str(),
        "https://collector.example.com/v1/traces"
    );
    let body: serde_json::Value =
        serde_json::from_slice(&requests[0].body).expect("body should be valid json");
    assert_eq!(
        body["resourceSpans"][0]["resource"]["attributes"][0]["key"],
        "service.name"
    );
    assert_eq!(
        body["resourceSpans"][0]["scopeSpans"][0]["spans"]
            .as_array()
            .map(Vec::len),
        Some(3)
    );
}
