use mastra_observability__test_utils::sample_trace_batch;
use mastra_observability_posthog::{PostHogConfig, PostHogExporter};

#[test]
fn builds_posthog_batch_capture_request() {
    let exporter = PostHogExporter::new(PostHogConfig {
        api_key: "phc_test".to_string(),
        host: "https://us.i.posthog.com".to_string(),
        default_distinct_id: "anonymous".to_string(),
    });

    let requests = exporter
        .build_requests(&sample_trace_batch())
        .expect("request planning should succeed");

    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].url.as_str(), "https://us.i.posthog.com/batch/");

    let body: serde_json::Value =
        serde_json::from_slice(&requests[0].body).expect("body should be valid json");
    assert_eq!(body["api_key"], "phc_test");
    assert_eq!(body["batch"].as_array().map(Vec::len), Some(4));
    assert_eq!(body["batch"][0]["event"], "$ai_trace");
    assert_eq!(body["batch"][1]["event"], "$ai_generation");
    assert_eq!(body["batch"][1]["properties"]["$ai_model"], "gpt-4.1-mini");
}
