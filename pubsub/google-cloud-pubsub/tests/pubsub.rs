use mastra_pubsub_google_cloud_pubsub::{
    GoogleCloudPubSub, GoogleCloudPubSubConfig, PubSubMessage,
};

#[tokio::test]
async fn publish_and_pull_delivers_messages_in_order() {
    let pubsub = GoogleCloudPubSub::new(GoogleCloudPubSubConfig::new("demo-project"));
    pubsub
        .create_topic("jobs")
        .expect("topic should be created");
    pubsub
        .create_subscription("jobs-sub", "jobs")
        .expect("subscription should be created");

    pubsub
        .publish("jobs", PubSubMessage::text("first"))
        .expect("publish should succeed");
    pubsub
        .publish("jobs", PubSubMessage::text("second"))
        .expect("publish should succeed");

    let batch = pubsub.pull("jobs-sub", 10).expect("pull should succeed");
    assert_eq!(batch.len(), 2);
    assert_eq!(batch[0].message.as_text(), Some("first"));
    assert_eq!(batch[1].message.as_text(), Some("second"));
}

#[tokio::test]
async fn acked_messages_are_not_redelivered() {
    let pubsub = GoogleCloudPubSub::new(GoogleCloudPubSubConfig::new("demo-project"));
    pubsub
        .create_topic("jobs")
        .expect("topic should be created");
    pubsub
        .create_subscription("jobs-sub", "jobs")
        .expect("subscription should be created");
    pubsub
        .publish("jobs", PubSubMessage::text("only-once"))
        .expect("publish should succeed");

    let first_batch = pubsub.pull("jobs-sub", 10).expect("pull should succeed");
    assert_eq!(first_batch.len(), 1);
    pubsub
        .ack("jobs-sub", [first_batch[0].ack_id.clone()])
        .expect("ack should succeed");

    let second_batch = pubsub
        .pull("jobs-sub", 10)
        .expect("second pull should succeed");
    assert!(second_batch.is_empty());
}

#[test]
fn config_defaults_are_cloud_friendly() {
    let config = GoogleCloudPubSubConfig::new("demo-project");
    assert_eq!(config.project_id, "demo-project");
    assert!(config.emulator_host.is_none());
}
