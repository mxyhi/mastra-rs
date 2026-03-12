use mastra_observability__test_utils::sample_trace_batch;
use mastra_observability_langsmith::{LangSmithConfig, LangSmithExporter};

#[test]
fn builds_langsmith_batch_request() {
    let exporter = LangSmithExporter::new(LangSmithConfig {
        api_key: "lsv2_pt_test".to_string(),
        api_url: "https://api.smith.langchain.com".to_string(),
        project_name: "mastra-rs".to_string(),
    });

    let requests = exporter
        .build_requests(&sample_trace_batch())
        .expect("request planning should succeed");

    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].url.as_str(),
        "https://api.smith.langchain.com/runs/batch"
    );
    assert_eq!(
        requests[0].headers.get("x-api-key"),
        Some(&"lsv2_pt_test".to_string())
    );

    let body: serde_json::Value =
        serde_json::from_slice(&requests[0].body).expect("body should be valid json");
    assert_eq!(body["post"].as_array().map(Vec::len), Some(4));
    assert_eq!(body["patch"].as_array().map(Vec::len), Some(4));
    assert_eq!(body["post"][0]["session_name"], "mastra-rs");
    assert_eq!(body["post"][1]["run_type"], "llm");
}
