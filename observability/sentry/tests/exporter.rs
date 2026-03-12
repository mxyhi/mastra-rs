use mastra_observability__test_utils::sample_trace_batch;
use mastra_observability_sentry::{SentryConfig, SentryExporter};

#[test]
fn builds_sentry_envelope_request() {
    let exporter = SentryExporter::new(SentryConfig {
        dsn: "https://public@example.ingest.sentry.io/42".to_string(),
        environment: Some("test".to_string()),
        release: Some("0.1.0".to_string()),
    });

    let requests = exporter
        .build_requests(&sample_trace_batch())
        .expect("request planning should succeed");

    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].url.as_str(),
        "https://example.ingest.sentry.io/api/42/envelope/"
    );
    assert_eq!(
        requests[0].headers.get("content-type"),
        Some(&"application/x-sentry-envelope".to_string())
    );

    let envelope = String::from_utf8(requests[0].body.clone()).expect("envelope should be utf8");
    let mut lines = envelope.lines();
    let header: serde_json::Value =
        serde_json::from_str(lines.next().expect("missing envelope header")).expect("valid header");
    let item_header: serde_json::Value =
        serde_json::from_str(lines.next().expect("missing item header"))
            .expect("valid item header");
    let payload: serde_json::Value =
        serde_json::from_str(lines.next().expect("missing payload")).expect("valid payload");

    assert_eq!(header["dsn"], "https://public@example.ingest.sentry.io/42");
    assert_eq!(item_header["type"], "span");
    assert_eq!(
        item_header["content_type"],
        "application/vnd.sentry.items.span.v2+json"
    );
    assert_eq!(payload["items"].as_array().map(Vec::len), Some(4));
    assert_eq!(payload["items"][0]["is_segment"], true);
}
