use mastra_observability__test_utils::sample_trace_batch;
use mastra_observability_laminar::{LaminarConfig, LaminarExporter};

#[test]
fn builds_laminar_otlp_wrapper_request() {
    let exporter = LaminarExporter::new(LaminarConfig {
        api_key: "lmnr_proj_test".to_string(),
        endpoint: "https://api.lmnr.ai/v1/traces".to_string(),
        headers: Default::default(),
    });

    let requests = exporter
        .build_requests(&sample_trace_batch())
        .expect("request planning should succeed");

    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].headers.get("authorization"),
        Some(&"Bearer lmnr_proj_test".to_string())
    );
}
