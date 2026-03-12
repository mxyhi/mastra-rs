use mastra_core::{RequestContext, Step, Workflow};
use mastra_workflows_inngest::{InngestEvent, InngestRuntime, RegisteredWorkflow};
use serde_json::json;

#[tokio::test]
async fn dispatch_runs_matching_workflow_and_returns_output() {
    let workflow = Workflow::new("sum").then(Step::new("double", |input, _| async move {
        let value = input["value"].as_i64().expect("value should exist");
        Ok(json!({ "value": value * 2 }))
    }));

    let runtime = InngestRuntime::new().register(
        RegisteredWorkflow::new("math/double", workflow),
    );

    let result = runtime
        .dispatch(
            InngestEvent::new("math/double", json!({ "value": 21 })),
            RequestContext::new().with_resource_id("workspace-1"),
        )
        .await
        .expect("workflow should run");

    assert_eq!(result.workflow_id, "sum");
    assert_eq!(result.output, json!({ "value": 42 }));
}

#[tokio::test]
async fn runtime_lists_registered_bindings_in_stable_order() {
    let first = RegisteredWorkflow::new("alpha", Workflow::new("workflow-a"));
    let second = RegisteredWorkflow::new("beta", Workflow::new("workflow-b"));
    let runtime = InngestRuntime::new().register(first).register(second);

    let bindings = runtime.bindings();
    assert_eq!(bindings.len(), 2);
    assert_eq!(bindings[0].event_name, "alpha");
    assert_eq!(bindings[1].workflow_id, "workflow-b");
}

#[tokio::test]
async fn dispatch_returns_not_found_for_unknown_event() {
    let runtime = InngestRuntime::new();
    let error = runtime
        .dispatch(InngestEvent::new("missing", json!(null)), RequestContext::new())
        .await
        .expect_err("unknown event should fail");

    assert!(error.to_string().contains("missing"));
}
