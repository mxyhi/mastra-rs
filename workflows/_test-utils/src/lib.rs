use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowStep {
    pub id: String,
    pub output: String,
}

impl WorkflowStep {
    pub fn new(id: impl Into<String>, output: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            output: output.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowRun {
    pub workflow_id: String,
    pub status: WorkflowStatus,
    pub steps: Vec<WorkflowStep>,
    pub output: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkflowStatus {
    Completed,
    Suspended,
    Failed,
}

#[derive(Debug, Clone)]
pub struct WorkflowRunBuilder {
    workflow_id: String,
    status: WorkflowStatus,
    steps: Vec<WorkflowStep>,
    output: Option<String>,
}

impl WorkflowRunBuilder {
    pub fn new(workflow_id: impl Into<String>) -> Self {
        Self {
            workflow_id: workflow_id.into(),
            status: WorkflowStatus::Completed,
            steps: Vec::new(),
            output: None,
        }
    }

    pub fn status(mut self, status: WorkflowStatus) -> Self {
        self.status = status;
        self
    }

    pub fn step(mut self, step: WorkflowStep) -> Self {
        self.steps.push(step);
        self
    }

    pub fn output(mut self, output: impl Into<String>) -> Self {
        self.output = Some(output.into());
        self
    }

    pub fn build(self) -> WorkflowRun {
        WorkflowRun {
            workflow_id: self.workflow_id,
            status: self.status,
            steps: self.steps,
            output: self.output,
        }
    }
}

pub struct MockRegistry<T> {
    factories: BTreeMap<String, Arc<dyn Fn() -> T + Send + Sync>>,
    values: BTreeMap<String, T>,
}

impl<T> Default for MockRegistry<T> {
    fn default() -> Self {
        Self {
            factories: BTreeMap::new(),
            values: BTreeMap::new(),
        }
    }
}

impl<T> MockRegistry<T> {
    pub fn register<F>(&mut self, key: impl Into<String>, factory: F)
    where
        F: Fn() -> T + Send + Sync + 'static,
    {
        let key = key.into();
        let factory = Arc::new(factory);
        let value = factory();
        self.factories.insert(key.clone(), factory);
        self.values.insert(key, value);
    }

    pub fn get(&self, key: &str) -> Option<&T> {
        self.values.get(key)
    }

    pub fn has(&self, key: &str) -> bool {
        self.factories.contains_key(key) || self.values.contains_key(key)
    }

    pub fn keys(&self) -> Vec<&str> {
        self.factories.keys().map(String::as_str).collect()
    }

    pub fn reset(&mut self) {
        self.values = self
            .factories
            .iter()
            .map(|(key, factory)| (key.clone(), factory()))
            .collect();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use super::{MockRegistry, WorkflowRunBuilder, WorkflowStatus, WorkflowStep};

    #[test]
    fn builds_workflow_runs_in_step_order() {
        let run = WorkflowRunBuilder::new("daily-sync")
            .step(WorkflowStep::new("fetch", "ok"))
            .step(WorkflowStep::new("index", "42 docs"))
            .output("done")
            .build();

        assert_eq!(run.workflow_id, "daily-sync");
        assert_eq!(run.status, WorkflowStatus::Completed);
        assert_eq!(run.steps[0].id, "fetch");
        assert_eq!(run.steps[1].output, "42 docs");
        assert_eq!(run.output.as_deref(), Some("done"));
    }

    #[test]
    fn mock_registry_recreates_values_on_reset() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_for_factory = Arc::clone(&counter);
        let mut registry = MockRegistry::default();

        registry.register("workflow:step", move || {
            counter_for_factory.fetch_add(1, Ordering::SeqCst) + 1
        });

        assert_eq!(registry.get("workflow:step"), Some(&1));

        registry.reset();

        assert_eq!(registry.get("workflow:step"), Some(&2));
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn registry_reports_known_keys() {
        let mut registry = MockRegistry::default();
        registry.register("a", || "alpha");
        registry.register("b", || "beta");

        assert!(registry.has("a"));
        assert_eq!(registry.keys(), vec!["a", "b"]);
    }
}
