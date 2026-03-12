use mastra_observability__test_utils::sample_trace_batch;
use mastra_observability_arize::{ArizeConfig, ArizeExporter};

#[test]
fn builds_arize_otlp_wrapper_request() {
    let exporter = ArizeExporter::new(ArizeConfig {
        endpoint: "https://collector.arize.test/v1/traces".to_string(),
        api_key: Some("arize-key".to_string()),
        project_name: Some("phoenix".to_string()),
        headers: Default::default(),
    });

    let requests = exporter
        .build_requests(&sample_trace_batch())
        .expect("request planning should succeed");

    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].headers.get("api_key"),
        Some(&"arize-key".to_string())
    );
}
