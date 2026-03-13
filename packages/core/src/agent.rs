use std::sync::Arc;

use async_stream::try_stream;
use futures::{StreamExt, stream};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    error::{MastraError, Result},
    memory::{
        CreateThreadRequest, MemoryConfig, MemoryEngine, MemoryMessage, MemoryRecallRequest,
        MemoryRole, ObservationQuery,
    },
    model::{
        FinishReason, LanguageModel, ModelEvent, ModelRequest, ModelResponse, ModelToolCall,
        ModelToolResult, UsageStats,
    },
    request_context::RequestContext,
    tool::{Tool, ToolExecutionContext},
};

const DEFAULT_AGENT_MAX_STEPS: u32 = 8;

#[derive(Clone)]
pub struct AgentConfig {
    pub id: String,
    pub name: String,
    pub instructions: String,
    pub description: Option<String>,
    pub model: Arc<dyn LanguageModel>,
    pub tools: Vec<Tool>,
    pub memory: Option<Arc<dyn MemoryEngine>>,
    pub memory_config: MemoryConfig,
}

#[derive(Clone)]
pub struct Agent {
    id: String,
    name: String,
    instructions: String,
    description: Option<String>,
    model: Arc<dyn LanguageModel>,
    tools: Vec<Tool>,
    memory: Option<Arc<dyn MemoryEngine>>,
    memory_config: MemoryConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct AgentGenerateRequest {
    pub prompt: String,
    pub thread_id: Option<String>,
    pub resource_id: Option<String>,
    pub run_id: Option<String>,
    pub max_steps: Option<u32>,
    pub instructions_override: Option<String>,
    pub system: Option<String>,
    #[serde(default)]
    pub context: Vec<AgentContextMessage>,
    #[serde(default)]
    pub disable_memory: bool,
    #[serde(default)]
    pub memory_read_only: bool,
    pub active_tools: Option<Vec<String>>,
    pub tool_choice: Option<AgentToolChoice>,
    pub output_schema: Option<Value>,
    pub request_context: RequestContext,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentResponse {
    pub id: String,
    pub text: String,
    pub data: Value,
    pub run_id: String,
    pub finish_reason: FinishReason,
    pub usage: Option<UsageStats>,
    pub thread_id: Option<String>,
    pub tool_names: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct AgentStreamRequest {
    pub prompt: String,
    pub thread_id: Option<String>,
    pub resource_id: Option<String>,
    pub run_id: Option<String>,
    pub max_steps: Option<u32>,
    pub instructions_override: Option<String>,
    pub system: Option<String>,
    #[serde(default)]
    pub context: Vec<AgentContextMessage>,
    #[serde(default)]
    pub disable_memory: bool,
    #[serde(default)]
    pub memory_read_only: bool,
    pub active_tools: Option<Vec<String>>,
    pub tool_choice: Option<AgentToolChoice>,
    pub output_schema: Option<Value>,
    pub request_context: RequestContext,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct AgentContextMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(untagged)]
pub enum AgentToolChoice {
    Mode(AgentToolChoiceMode),
    Tool(AgentNamedToolChoice),
}

impl AgentToolChoice {
    pub fn tool(tool_name: impl Into<String>) -> Self {
        Self::Tool(AgentNamedToolChoice {
            kind: AgentNamedToolChoiceKind::Tool,
            tool_name: tool_name.into(),
        })
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentToolChoiceMode {
    Auto,
    None,
    Required,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct AgentNamedToolChoice {
    #[serde(rename = "type")]
    pub kind: AgentNamedToolChoiceKind,
    #[serde(rename = "toolName")]
    pub tool_name: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub enum AgentNamedToolChoiceKind {
    #[serde(rename = "tool")]
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentStreamResponse {
    pub id: String,
    pub event: ModelEvent,
    pub thread_id: Option<String>,
}

impl Agent {
    pub fn new(config: AgentConfig) -> Self {
        Self {
            id: config.id,
            name: config.name,
            instructions: config.instructions,
            description: config.description,
            model: config.model,
            tools: config.tools,
            memory: config.memory,
            memory_config: config.memory_config,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn instructions(&self) -> &str {
        &self.instructions
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    pub fn tool_names(&self) -> Vec<String> {
        self.tools
            .iter()
            .map(|tool| tool.id().to_string())
            .collect()
    }

    pub fn tools(&self) -> &[Tool] {
        &self.tools
    }

    pub async fn generate(&self, request: AgentGenerateRequest) -> Result<AgentResponse> {
        let AgentGenerateRequest {
            prompt,
            thread_id,
            resource_id,
            run_id,
            max_steps,
            instructions_override,
            system,
            context,
            disable_memory,
            memory_read_only,
            active_tools,
            tool_choice,
            output_schema,
            request_context,
        } = request;
        let model_prompt = compose_prompt(&prompt, &context);
        let instructions = compose_instructions(
            &self.instructions,
            instructions_override.as_deref(),
            system.as_deref(),
            output_schema.as_ref(),
        );
        let tool_names = resolve_tool_names(
            self.tool_names(),
            active_tools.as_deref(),
            tool_choice.as_ref(),
        );
        let (thread_id, memory_context) = self
            .prepare_memory(
                &prompt,
                thread_id,
                resource_id,
                disable_memory,
                memory_read_only,
            )
            .await?;
        let run_id = run_id.unwrap_or_else(|| Uuid::now_v7().to_string());
        let max_steps = max_steps.unwrap_or(DEFAULT_AGENT_MAX_STEPS).max(1);
        let mut tool_results = Vec::new();
        let mut aggregated_usage = None;

        for step in 0..max_steps {
            let response = self
                .model
                .generate(ModelRequest {
                    prompt: model_prompt.clone(),
                    instructions: instructions.clone(),
                    memory: memory_context.clone(),
                    tool_names: tool_names.clone(),
                    tool_results: tool_results.clone(),
                    run_id: Some(run_id.clone()),
                    thread_id: thread_id.clone(),
                    max_steps: Some(max_steps),
                    request_context: request_context.clone(),
                })
                .await?;
            accumulate_usage(&mut aggregated_usage, response.normalized_usage());
            let finish_reason = response.normalized_finish_reason();
            let tool_calls = response.normalized_tool_calls();

            if tool_calls.is_empty() {
                if finish_reason == FinishReason::ToolCall {
                    return Err(MastraError::tool(format!(
                        "agent '{}' received tool_call finish reason without tool payload",
                        self.id
                    )));
                }
                let mut response = response;
                if let Some(usage) = aggregated_usage.clone() {
                    response.usage = Some(usage);
                }
                self.persist_response(
                    &thread_id,
                    &prompt,
                    &tool_results,
                    &response,
                    disable_memory,
                    memory_read_only,
                )
                .await?;
                return Ok(self.to_agent_response(response, run_id, thread_id, tool_names));
            }

            if step + 1 >= max_steps {
                return Err(MastraError::tool(format!(
                    "agent '{}' exhausted max_steps ({max_steps}) before finishing tool loop",
                    self.id
                )));
            }

            let mut round_results = self
                .execute_tool_calls(
                    &tool_calls,
                    &tool_names,
                    &request_context,
                    &run_id,
                    &thread_id,
                )
                .await?;
            tool_results.append(&mut round_results);
        }

        Err(MastraError::tool(format!(
            "agent '{}' failed to complete within {max_steps} steps",
            self.id
        )))
    }

    pub fn stream(
        &self,
        request: AgentStreamRequest,
    ) -> futures::stream::BoxStream<'static, Result<AgentStreamResponse>> {
        let resolved_tool_names = resolve_tool_names(
            self.tool_names(),
            request.active_tools.as_deref(),
            request.tool_choice.as_ref(),
        );

        if !resolved_tool_names.is_empty() {
            let agent = self.clone();
            let resolved_tool_names = resolved_tool_names.clone();
            return try_stream! {
                let AgentStreamRequest {
                    prompt,
                    thread_id,
                    resource_id,
                    run_id,
                    max_steps,
                    instructions_override,
                    system,
                    context,
                    disable_memory,
                    memory_read_only,
                    active_tools: _,
                    tool_choice: _,
                    output_schema,
                    request_context,
                } = request;
                let model_prompt = compose_prompt(&prompt, &context);
                let instructions = compose_instructions(
                    &agent.instructions,
                    instructions_override.as_deref(),
                    system.as_deref(),
                    output_schema.as_ref(),
                );
                let (thread_id, memory_context) =
                    agent
                        .prepare_memory(
                            &prompt,
                            thread_id,
                            resource_id,
                            disable_memory,
                            memory_read_only,
                        )
                        .await?;
                let run_id = run_id.unwrap_or_else(|| Uuid::now_v7().to_string());
                let max_steps = max_steps.unwrap_or(DEFAULT_AGENT_MAX_STEPS).max(1);
                let mut tool_results = Vec::new();
                let mut aggregated_usage = None;

                for step in 0..max_steps {
                    let response = agent
                        .model
                        .generate(ModelRequest {
                            prompt: model_prompt.clone(),
                            instructions: instructions.clone(),
                            memory: memory_context.clone(),
                            tool_names: resolved_tool_names.clone(),
                            tool_results: tool_results.clone(),
                            run_id: Some(run_id.clone()),
                            thread_id: thread_id.clone(),
                            max_steps: Some(max_steps),
                            request_context: request_context.clone(),
                        })
                        .await?;
                    accumulate_usage(&mut aggregated_usage, response.normalized_usage());
                    let finish_reason = response.normalized_finish_reason();
                    let tool_calls = response.normalized_tool_calls();

                    if tool_calls.is_empty() {
                        if finish_reason == FinishReason::ToolCall {
                            Err(MastraError::tool(format!(
                                "agent '{}' received tool_call finish reason without tool payload",
                                agent.id
                            )))?;
                        }
                        let mut response = response;
                        if let Some(usage) = aggregated_usage.clone() {
                            response.usage = Some(usage);
                        }
                        agent
                            .persist_response(
                                &thread_id,
                                &prompt,
                                &tool_results,
                                &response,
                                disable_memory,
                                memory_read_only,
                            )
                            .await?;
                        yield AgentStreamResponse {
                            id: agent.id.clone(),
                            event: ModelEvent::Done(response),
                            thread_id,
                        };
                        return;
                    }

                    if step + 1 >= max_steps {
                        Err(MastraError::tool(format!(
                            "agent '{}' exhausted max_steps ({max_steps}) before finishing tool loop",
                            agent.id
                        )))?;
                    }

                    for call in &tool_calls {
                        yield AgentStreamResponse {
                            id: agent.id.clone(),
                            event: ModelEvent::ToolCall(call.clone()),
                            thread_id: thread_id.clone(),
                        };
                    }

                    let mut round_results = agent
                        .execute_tool_calls(
                            &tool_calls,
                            &resolved_tool_names,
                            &request_context,
                            &run_id,
                            &thread_id,
                        )
                        .await?;

                    for result in &round_results {
                        yield AgentStreamResponse {
                            id: agent.id.clone(),
                            event: ModelEvent::ToolResult(result.clone()),
                            thread_id: thread_id.clone(),
                        };
                    }

                    tool_results.append(&mut round_results);
                }

                Err(MastraError::tool(format!(
                    "agent '{}' failed to complete within {max_steps} steps",
                    agent.id
                )))?;
            }
            .boxed();
        }

        let agent = self.clone();
        stream::once(async move {
            let disable_memory = request.disable_memory;
            let run_id = request.run_id.unwrap_or_else(|| Uuid::now_v7().to_string());
            let max_steps = request.max_steps.unwrap_or(DEFAULT_AGENT_MAX_STEPS).max(1);
            let model_prompt = compose_prompt(&request.prompt, &request.context);
            let instructions = compose_instructions(
                &agent.instructions,
                request.instructions_override.as_deref(),
                request.system.as_deref(),
                request.output_schema.as_ref(),
            );
            let (thread_id, memory_context) = agent
                .prepare_memory(
                    &request.prompt,
                    request.thread_id,
                    request.resource_id,
                    disable_memory,
                    request.memory_read_only,
                )
                .await?;
            let prompt = request.prompt;
            let stream = agent.model.stream(ModelRequest {
                prompt: model_prompt,
                instructions,
                memory: memory_context,
                tool_names: resolved_tool_names,
                tool_results: Vec::new(),
                run_id: Some(run_id),
                thread_id: thread_id.clone(),
                max_steps: Some(max_steps),
                request_context: request.request_context,
            });

            Ok::<_, MastraError>((
                agent,
                prompt,
                thread_id,
                stream,
                disable_memory,
                request.memory_read_only,
            ))
        })
        .flat_map(|result| match result {
            Ok((agent, prompt, thread_id, stream, disable_memory, memory_read_only)) => {
                let agent_id = agent.id.clone();
                stream
                    .then(move |event| {
                        let agent = agent.clone();
                        let prompt = prompt.clone();
                        let thread_id = thread_id.clone();
                        let agent_id = agent_id.clone();
                        async move {
                            let event = event?;
                            if let ModelEvent::Done(response) = &event {
                                agent
                                    .persist_response(
                                        &thread_id,
                                        &prompt,
                                        &[],
                                        response,
                                        disable_memory,
                                        memory_read_only,
                                    )
                                    .await?;
                            }

                            Ok(AgentStreamResponse {
                                id: agent_id,
                                event,
                                thread_id,
                            })
                        }
                    })
                    .boxed()
            }
            Err(error) => stream::once(async { Err(error) }).boxed(),
        })
        .boxed()
    }

    async fn prepare_memory(
        &self,
        prompt: &str,
        thread_id: Option<String>,
        resource_id: Option<String>,
        disable_memory: bool,
        memory_read_only: bool,
    ) -> Result<(Option<String>, Vec<String>)> {
        if disable_memory {
            return Ok((thread_id, Vec::new()));
        }

        let Some(memory) = &self.memory else {
            return Ok((thread_id, Vec::new()));
        };
        let resource_id_for_lookup = resource_id.clone();

        let thread_id = match thread_id {
            Some(thread_id) => thread_id,
            None if memory_read_only => {
                return Ok((None, Vec::new()));
            }
            None => {
                let thread = memory
                    .create_thread(CreateThreadRequest {
                        id: Some(Uuid::now_v7().to_string()),
                        resource_id,
                        title: Some(prompt.chars().take(32).collect()),
                        metadata: Value::Null,
                    })
                    .await?;
                thread.id
            }
        };

        let history = memory
            .list_messages(MemoryRecallRequest {
                thread_id: thread_id.clone(),
                limit: self.memory_config.last_messages,
                resource_id: None,
                page: None,
                per_page: None,
                message_ids: None,
                start_date: None,
                end_date: None,
                order_by: None,
            })
            .await?
            .into_iter()
            .map(|message| format!("{:?}: {}", message.role, message.content))
            .collect::<Vec<_>>();

        let mut memory_context = Vec::new();
        if self.memory_config.working_memory.enabled {
            if let Some(working_memory) = memory
                .get_working_memory(&thread_id, resource_id_for_lookup.as_deref())
                .await?
            {
                memory_context.push(working_memory.system_message());
            }
        }

        if self.memory_config.observational_memory.enabled {
            let observations = memory
                .list_observations(ObservationQuery {
                    thread_id: thread_id.clone(),
                    resource_id: resource_id_for_lookup,
                    scope: Some(self.memory_config.observational_memory.scope),
                    page: None,
                    per_page: None,
                })
                .await?;
            memory_context.extend(
                observations
                    .observations
                    .into_iter()
                    .map(|observation| observation.render_context_line()),
            );
        }

        memory_context.extend(history);

        Ok((Some(thread_id), memory_context))
    }

    async fn execute_tool_calls(
        &self,
        tool_calls: &[ModelToolCall],
        allowed_tool_names: &[String],
        request_context: &RequestContext,
        run_id: &str,
        thread_id: &Option<String>,
    ) -> Result<Vec<ModelToolResult>> {
        let mut results = Vec::with_capacity(tool_calls.len());

        for call in tool_calls {
            if !allowed_tool_names
                .iter()
                .any(|tool_name| tool_name == &call.name)
            {
                return Err(MastraError::tool(format!(
                    "agent '{}' received disallowed tool call '{}'",
                    self.id, call.name
                )));
            }
            let tool = self
                .tools
                .iter()
                .find(|tool| tool.id() == call.name)
                .ok_or_else(|| {
                    MastraError::tool(format!(
                        "agent '{}' received unknown tool call '{}'",
                        self.id, call.name
                    ))
                })?;
            let output = tool
                .execute(
                    call.input.clone(),
                    ToolExecutionContext {
                        request_context: request_context.clone(),
                        run_id: Some(run_id.to_owned()),
                        thread_id: thread_id.clone(),
                        approved: false,
                    },
                )
                .await?;
            results.push(ModelToolResult {
                id: call.id.clone(),
                name: call.name.clone(),
                output,
            });
        }

        Ok(results)
    }

    fn to_agent_response(
        &self,
        response: ModelResponse,
        run_id: String,
        thread_id: Option<String>,
        tool_names: Vec<String>,
    ) -> AgentResponse {
        let finish_reason = response.normalized_finish_reason();
        let usage = response.normalized_usage();
        AgentResponse {
            id: self.id.clone(),
            text: response.text,
            data: response.data,
            run_id,
            finish_reason,
            usage,
            thread_id,
            tool_names,
        }
    }

    async fn persist_response(
        &self,
        thread_id: &Option<String>,
        prompt: &str,
        tool_results: &[ModelToolResult],
        response: &ModelResponse,
        disable_memory: bool,
        memory_read_only: bool,
    ) -> Result<()> {
        if disable_memory || memory_read_only {
            return Ok(());
        }

        let Some(memory) = &self.memory else {
            return Ok(());
        };

        if self.memory_config.read_only {
            return Ok(());
        }

        let Some(thread_id) = thread_id else {
            return Ok(());
        };

        // Keep tool outputs between the user turn and the final assistant reply so replayed
        // memory matches the tool loop that the model observed.
        let mut messages = Vec::with_capacity(tool_results.len() + 2);
        messages.push(MemoryMessage {
            id: Uuid::now_v7().to_string(),
            thread_id: thread_id.clone(),
            role: MemoryRole::User,
            content: prompt.to_string(),
            created_at: chrono::Utc::now(),
            metadata: Value::Null,
        });

        for result in tool_results {
            messages.push(MemoryMessage {
                id: Uuid::now_v7().to_string(),
                thread_id: thread_id.clone(),
                role: MemoryRole::Tool,
                content: match &result.output {
                    Value::String(text) => text.clone(),
                    value => value.to_string(),
                },
                created_at: chrono::Utc::now(),
                metadata: json!({
                    "tool_name": result.name,
                    "tool_call_id": result.id,
                }),
            });
        }

        messages.push(MemoryMessage {
            id: Uuid::now_v7().to_string(),
            thread_id: thread_id.clone(),
            role: MemoryRole::Assistant,
            content: response.text.clone(),
            created_at: chrono::Utc::now(),
            metadata: response.data.clone(),
        });

        memory.append_messages(thread_id, messages).await
    }

    pub fn snapshot(&self) -> Value {
        json!({
          "id": self.id,
          "name": self.name,
          "description": self.description,
          "instructions": self.instructions,
          "tools": self.tool_names(),
        })
    }
}

fn compose_prompt(prompt: &str, context: &[AgentContextMessage]) -> String {
    if context.is_empty() {
        return prompt.to_owned();
    }

    let mut segments = context
        .iter()
        .map(|message| {
            let role = message.role.trim();
            let role = if role.is_empty() { "message" } else { role };
            format!("{role}: {}", message.content)
        })
        .collect::<Vec<_>>();
    segments.push(prompt.to_owned());
    segments.join("\n")
}

fn compose_instructions(
    base_instructions: &str,
    instructions_override: Option<&str>,
    system: Option<&str>,
    output_schema: Option<&Value>,
) -> String {
    let mut sections = Vec::new();

    if !base_instructions.trim().is_empty() {
        sections.push(base_instructions.trim().to_owned());
    }
    if let Some(instructions_override) =
        instructions_override.filter(|value| !value.trim().is_empty())
    {
        sections.push(format!(
            "Request instructions:\n{}",
            instructions_override.trim()
        ));
    }
    if let Some(system) = system.filter(|value| !value.trim().is_empty()) {
        sections.push(format!("System message:\n{}", system.trim()));
    }
    if let Some(output_schema) = output_schema {
        let serialized = serde_json::to_string_pretty(output_schema)
            .unwrap_or_else(|_| output_schema.to_string());
        sections.push(format!("Structured output schema:\n{serialized}"));
    }

    sections.join("\n\n")
}

fn resolve_tool_names(
    available_tool_names: Vec<String>,
    active_tools: Option<&[String]>,
    tool_choice: Option<&AgentToolChoice>,
) -> Vec<String> {
    let mut tool_names = available_tool_names;

    if let Some(active_tools) = active_tools {
        tool_names.retain(|tool_name| active_tools.iter().any(|active| active == tool_name));
    }

    match tool_choice {
        Some(AgentToolChoice::Mode(AgentToolChoiceMode::None)) => {
            tool_names.clear();
        }
        Some(AgentToolChoice::Tool(choice)) => {
            tool_names.retain(|tool_name| tool_name == &choice.tool_name);
        }
        Some(AgentToolChoice::Mode(AgentToolChoiceMode::Auto | AgentToolChoiceMode::Required))
        | None => {}
    }

    tool_names
}

fn accumulate_usage(total: &mut Option<UsageStats>, usage: Option<UsageStats>) {
    let Some(usage) = usage else {
        return;
    };

    match total {
        Some(total) => {
            total.prompt_tokens += usage.prompt_tokens;
            total.completion_tokens += usage.completion_tokens;
        }
        None => *total = Some(usage),
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc};

    use async_trait::async_trait;
    use chrono::Utc;
    use futures::StreamExt;
    use parking_lot::RwLock;
    use serde_json::{Value, json};

    use crate::{
        memory::{
            CreateThreadRequest, MemoryConfig, MemoryEngine, MemoryMessage, MemoryRecallRequest,
            MemoryRole, MemoryScope, ObservationPage, ObservationQuery, ObservationRecord,
            ObservationalMemoryConfig, Thread, WorkingMemoryConfig, WorkingMemoryFormat,
            WorkingMemoryState,
        },
        model::{
            FinishReason, ModelEvent, ModelRequest, ModelResponse, ModelToolCall, ModelToolResult,
            StaticModel, UsageStats,
        },
        request_context::RequestContext,
        tool::Tool,
    };

    use super::{Agent, AgentConfig, AgentGenerateRequest, AgentStreamRequest};

    #[derive(Default)]
    struct RecordingMemory {
        threads: RwLock<HashMap<String, Thread>>,
        messages: RwLock<HashMap<String, Vec<MemoryMessage>>>,
        working_memory: RwLock<HashMap<String, WorkingMemoryState>>,
        observations: RwLock<HashMap<String, Vec<ObservationRecord>>>,
    }

    #[async_trait]
    impl MemoryEngine for RecordingMemory {
        async fn create_thread(&self, request: CreateThreadRequest) -> crate::Result<Thread> {
            let now = Utc::now();
            let thread = Thread {
                id: request
                    .id
                    .unwrap_or_else(|| uuid::Uuid::now_v7().to_string()),
                resource_id: request.resource_id,
                title: request.title,
                created_at: now,
                updated_at: now,
                metadata: request.metadata,
            };
            self.threads
                .write()
                .insert(thread.id.clone(), thread.clone());
            self.messages.write().entry(thread.id.clone()).or_default();
            Ok(thread)
        }

        async fn get_thread(&self, thread_id: &str) -> crate::Result<Option<Thread>> {
            Ok(self.threads.read().get(thread_id).cloned())
        }

        async fn list_threads(&self, _resource_id: Option<&str>) -> crate::Result<Vec<Thread>> {
            Ok(self.threads.read().values().cloned().collect())
        }

        async fn append_messages(
            &self,
            thread_id: &str,
            messages: Vec<MemoryMessage>,
        ) -> crate::Result<()> {
            self.messages
                .write()
                .entry(thread_id.to_string())
                .or_default()
                .extend(messages);
            Ok(())
        }

        async fn list_messages(
            &self,
            request: MemoryRecallRequest,
        ) -> crate::Result<Vec<MemoryMessage>> {
            Ok(self
                .messages
                .read()
                .get(&request.thread_id)
                .cloned()
                .unwrap_or_default())
        }

        async fn get_working_memory(
            &self,
            thread_id: &str,
            _resource_id: Option<&str>,
        ) -> crate::Result<Option<WorkingMemoryState>> {
            Ok(self.working_memory.read().get(thread_id).cloned())
        }

        async fn list_observations(
            &self,
            request: ObservationQuery,
        ) -> crate::Result<ObservationPage> {
            let observations = self
                .observations
                .read()
                .get(&request.thread_id)
                .cloned()
                .unwrap_or_default();
            Ok(ObservationPage {
                total: observations.len(),
                page: request.page.unwrap_or(0),
                per_page: request
                    .per_page
                    .unwrap_or_else(|| observations.len().max(1)),
                has_more: false,
                observations,
            })
        }
    }

    #[tokio::test]
    async fn stream_persists_exchange_when_model_finishes() {
        let memory = Arc::new(RecordingMemory::default());
        let agent = Agent::new(AgentConfig {
            id: "streamer".into(),
            name: "Streamer".into(),
            instructions: "Echo".into(),
            description: None,
            model: Arc::new(StaticModel::echo()),
            tools: Vec::new(),
            memory: Some(memory.clone()),
            memory_config: MemoryConfig::default(),
        });

        let events = agent
            .stream(AgentStreamRequest {
                prompt: "persist me".into(),
                thread_id: None,
                resource_id: Some("resource-1".into()),
                run_id: None,
                max_steps: None,
                request_context: RequestContext::new(),
                ..Default::default()
            })
            .collect::<Vec<_>>()
            .await;

        assert_eq!(events.len(), 1);
        let event = events
            .into_iter()
            .next()
            .expect("stream event")
            .expect("stream ok");
        match &event.event {
            ModelEvent::Done(response) => {
                assert_eq!(response.text, "persist me");
            }
            other => panic!("expected final response event, got {other:?}"),
        }

        let thread_id = event.thread_id.expect("thread id should exist");
        let persisted = memory
            .list_messages(MemoryRecallRequest {
                thread_id,
                limit: None,
                resource_id: None,
                page: None,
                per_page: None,
                message_ids: None,
                start_date: None,
                end_date: None,
                order_by: None,
            })
            .await
            .expect("messages should be persisted");

        assert_eq!(persisted.len(), 2);
        assert_eq!(persisted[0].role, MemoryRole::User);
        assert_eq!(persisted[0].content, "persist me");
        assert_eq!(persisted[1].role, MemoryRole::Assistant);
        assert_eq!(persisted[1].content, "persist me");
        assert_eq!(persisted[1].metadata, Value::Null);
    }

    #[tokio::test]
    async fn generate_executes_tool_calls_until_model_returns_final_response() {
        let seen_tool_contexts = Arc::new(RwLock::new(Vec::new()));
        let model_requests = Arc::new(RwLock::new(Vec::new()));

        let recording_requests = Arc::clone(&model_requests);
        let model = StaticModel::new(move |request| {
            let recording_requests = Arc::clone(&recording_requests);
            async move {
                let step = {
                    let mut requests = recording_requests.write();
                    let step = requests.len();
                    requests.push(request.clone());
                    step
                };

                match step {
                    0 => Ok(ModelResponse {
                        text: String::new(),
                        data: Value::Null,
                        finish_reason: FinishReason::ToolCall,
                        usage: Some(UsageStats {
                            prompt_tokens: 3,
                            completion_tokens: 1,
                        }),
                        tool_calls: vec![ModelToolCall {
                            id: "call-1".into(),
                            name: "sum".into(),
                            input: json!({ "a": 2, "b": 3 }),
                        }],
                    }),
                    1 => {
                        assert_eq!(
                            request.tool_results,
                            vec![ModelToolResult {
                                id: "call-1".into(),
                                name: "sum".into(),
                                output: json!(5),
                            }]
                        );
                        Ok(ModelResponse {
                            text: "5".into(),
                            data: json!({ "source": "tool-loop" }),
                            finish_reason: FinishReason::Stop,
                            usage: Some(UsageStats {
                                prompt_tokens: 5,
                                completion_tokens: 2,
                            }),
                            tool_calls: Vec::new(),
                        })
                    }
                    other => panic!("unexpected model step {other}"),
                }
            }
        });

        let tool_contexts = Arc::clone(&seen_tool_contexts);
        let sum_tool = Tool::new("sum", "add numbers", move |input, context| {
            let tool_contexts = Arc::clone(&tool_contexts);
            async move {
                tool_contexts.write().push(context);
                let a = input.get("a").and_then(Value::as_i64).unwrap_or_default();
                let b = input.get("b").and_then(Value::as_i64).unwrap_or_default();
                Ok(json!(a + b))
            }
        });

        let mut request_context = RequestContext::new();
        request_context.insert("trace_id", "trace-123");

        let agent = Agent::new(AgentConfig {
            id: "tool-loop".into(),
            name: "Tool Loop".into(),
            instructions: "Use tools when helpful".into(),
            description: None,
            model: Arc::new(model),
            tools: vec![sum_tool],
            memory: None,
            memory_config: MemoryConfig::default(),
        });

        let response = agent
            .generate(AgentGenerateRequest {
                prompt: "2 + 3 = ?".into(),
                thread_id: Some("thread-123".into()),
                resource_id: None,
                run_id: Some("run-123".into()),
                max_steps: Some(4),
                request_context: request_context.clone(),
                ..Default::default()
            })
            .await
            .expect("agent should resolve tool loop");

        assert_eq!(response.text, "5");
        assert_eq!(response.data, json!({ "source": "tool-loop" }));
        assert_eq!(response.run_id, "run-123");
        assert_eq!(response.finish_reason, FinishReason::Stop);
        assert_eq!(
            response.usage,
            Some(UsageStats {
                prompt_tokens: 8,
                completion_tokens: 3,
            })
        );

        let seen_tool_contexts = seen_tool_contexts.read();
        assert_eq!(seen_tool_contexts.len(), 1);
        assert_eq!(seen_tool_contexts[0].run_id.as_deref(), Some("run-123"));
        assert_eq!(
            seen_tool_contexts[0].thread_id.as_deref(),
            Some("thread-123")
        );
        assert_eq!(
            seen_tool_contexts[0].request_context.get("trace_id"),
            Some(&json!("trace-123"))
        );

        let model_requests = model_requests.read();
        assert_eq!(model_requests.len(), 2);
        assert!(model_requests[0].tool_results.is_empty());
        assert_eq!(model_requests[0].run_id.as_deref(), Some("run-123"));
        assert_eq!(model_requests[0].thread_id.as_deref(), Some("thread-123"));
    }

    #[tokio::test]
    async fn generate_includes_working_memory_and_observations_when_enabled() {
        let memory = Arc::new(RecordingMemory::default());
        let thread = memory
            .create_thread(CreateThreadRequest {
                id: Some("thread-memory".into()),
                resource_id: Some("resource-memory".into()),
                title: Some("Memory thread".into()),
                metadata: Value::Null,
            })
            .await
            .expect("thread should be created");
        memory
            .append_messages(
                &thread.id,
                vec![MemoryMessage {
                    id: uuid::Uuid::now_v7().to_string(),
                    thread_id: thread.id.clone(),
                    role: MemoryRole::User,
                    content: "prior context".into(),
                    created_at: Utc::now(),
                    metadata: Value::Null,
                }],
            )
            .await
            .expect("history should be appended");
        memory.working_memory.write().insert(
            thread.id.clone(),
            WorkingMemoryState {
                thread_id: thread.id.clone(),
                resource_id: Some("resource-memory".into()),
                scope: MemoryScope::Thread,
                format: WorkingMemoryFormat::Markdown,
                template: Some("# User Profile".into()),
                content: json!("Name: Sam"),
                updated_at: Utc::now(),
            },
        );
        memory.observations.write().insert(
            thread.id.clone(),
            vec![ObservationRecord {
                id: uuid::Uuid::now_v7().to_string(),
                thread_id: thread.id.clone(),
                resource_id: Some("resource-memory".into()),
                scope: MemoryScope::Thread,
                content: "User likes Rust.".into(),
                observed_message_ids: Vec::new(),
                metadata: Value::Null,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            }],
        );

        let model_requests = Arc::new(RwLock::new(Vec::<ModelRequest>::new()));
        let recording_requests = Arc::clone(&model_requests);
        let model = StaticModel::new(move |request| {
            let recording_requests = Arc::clone(&recording_requests);
            async move {
                recording_requests.write().push(request);
                Ok(ModelResponse {
                    text: "done".into(),
                    data: Value::Null,
                    finish_reason: FinishReason::Stop,
                    usage: None,
                    tool_calls: Vec::new(),
                })
            }
        });

        let agent = Agent::new(AgentConfig {
            id: "memory-agent".into(),
            name: "Memory Agent".into(),
            instructions: "Use memory".into(),
            description: None,
            model: Arc::new(model),
            tools: Vec::new(),
            memory: Some(memory),
            memory_config: MemoryConfig {
                last_messages: Some(10),
                read_only: false,
                working_memory: WorkingMemoryConfig {
                    enabled: true,
                    scope: MemoryScope::Thread,
                    format: WorkingMemoryFormat::Markdown,
                    template: Some("# User Profile".into()),
                },
                observational_memory: ObservationalMemoryConfig {
                    enabled: true,
                    scope: MemoryScope::Thread,
                },
            },
        });

        agent
            .generate(AgentGenerateRequest {
                prompt: "current prompt".into(),
                thread_id: Some("thread-memory".into()),
                resource_id: Some("resource-memory".into()),
                request_context: RequestContext::new(),
                ..Default::default()
            })
            .await
            .expect("agent should generate");

        let recorded = model_requests.read();
        assert_eq!(recorded.len(), 1);
        assert!(
            recorded[0]
                .memory
                .iter()
                .any(|entry| entry.contains("Working memory:"))
        );
        assert!(
            recorded[0]
                .memory
                .iter()
                .any(|entry| entry.contains("Observation: User likes Rust."))
        );
        assert!(
            recorded[0]
                .memory
                .iter()
                .any(|entry| entry.contains("User: prior context"))
        );
    }

    #[test]
    fn tools_accessor_returns_registered_tools_in_order() {
        let agent = Agent::new(AgentConfig {
            id: "tool-accessor".into(),
            name: "Tool Accessor".into(),
            instructions: "Use tools when helpful".into(),
            description: None,
            model: Arc::new(StaticModel::new(|_request| async move {
                Ok(ModelResponse {
                    text: String::new(),
                    data: Value::Null,
                    finish_reason: FinishReason::Stop,
                    usage: None,
                    tool_calls: Vec::new(),
                })
            })),
            tools: vec![
                Tool::new("sum", "add numbers", |_input, _context| async move {
                    Ok(json!(3))
                }),
                Tool::new(
                    "product",
                    "multiply numbers",
                    |_input, _context| async move { Ok(json!(6)) },
                ),
            ],
            memory: None,
            memory_config: MemoryConfig::default(),
        });

        let tools = agent.tools();
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0].id(), "sum");
        assert_eq!(tools[1].id(), "product");
    }

    #[tokio::test]
    async fn generate_persists_tool_results_into_memory_before_assistant_reply() {
        let memory = Arc::new(RecordingMemory::default());
        let model = StaticModel::new(|request| async move {
            match request.tool_results.as_slice() {
                [] => Ok(ModelResponse {
                    text: String::new(),
                    data: Value::Null,
                    finish_reason: FinishReason::ToolCall,
                    usage: None,
                    tool_calls: vec![ModelToolCall {
                        id: "tool-call-1".into(),
                        name: "sum".into(),
                        input: json!({ "a": 6, "b": 7 }),
                    }],
                }),
                [_result] => Ok(ModelResponse {
                    text: "13".into(),
                    data: json!({ "source": "tool-memory" }),
                    finish_reason: FinishReason::Stop,
                    usage: None,
                    tool_calls: Vec::new(),
                }),
                other => panic!("unexpected tool results: {other:?}"),
            }
        });
        let agent = Agent::new(AgentConfig {
            id: "tool-memory".into(),
            name: "Tool Memory".into(),
            instructions: "Use tools when helpful".into(),
            description: None,
            model: Arc::new(model),
            tools: vec![Tool::new(
                "sum",
                "add numbers",
                |input, _context| async move {
                    let a = input.get("a").and_then(Value::as_i64).unwrap_or_default();
                    let b = input.get("b").and_then(Value::as_i64).unwrap_or_default();
                    Ok(json!(a + b))
                },
            )],
            memory: Some(memory.clone()),
            memory_config: MemoryConfig::default(),
        });

        let response = agent
            .generate(AgentGenerateRequest {
                prompt: "6 + 7 = ?".into(),
                thread_id: Some("thread-tool-memory".into()),
                resource_id: None,
                run_id: Some("run-tool-memory".into()),
                max_steps: Some(4),
                request_context: RequestContext::new(),
                ..Default::default()
            })
            .await
            .expect("agent should resolve tool loop");

        assert_eq!(response.text, "13");

        let persisted = memory
            .list_messages(MemoryRecallRequest {
                thread_id: "thread-tool-memory".into(),
                limit: None,
                resource_id: None,
                page: None,
                per_page: None,
                message_ids: None,
                start_date: None,
                end_date: None,
                order_by: None,
            })
            .await
            .expect("messages should be persisted");

        assert_eq!(persisted.len(), 3);
        assert_eq!(persisted[0].role, MemoryRole::User);
        assert_eq!(persisted[1].role, MemoryRole::Tool);
        assert_eq!(persisted[1].metadata["tool_name"], "sum");
        assert_eq!(persisted[1].metadata["tool_call_id"], "tool-call-1");
        assert_eq!(persisted[1].content, "13");
        assert_eq!(persisted[2].role, MemoryRole::Assistant);
        assert_eq!(persisted[2].content, "13");
    }

    #[tokio::test]
    async fn stream_emits_tool_lifecycle_for_tool_enabled_agents() {
        let model_steps = Arc::new(RwLock::new(Vec::new()));

        let recording_steps = Arc::clone(&model_steps);
        let model = StaticModel::new(move |request| {
            let recording_steps = Arc::clone(&recording_steps);
            async move {
                let step = {
                    let mut steps = recording_steps.write();
                    let step = steps.len();
                    steps.push(request.clone());
                    step
                };

                match step {
                    0 => Ok(ModelResponse {
                        text: String::new(),
                        data: Value::Null,
                        finish_reason: FinishReason::ToolCall,
                        usage: Some(UsageStats {
                            prompt_tokens: 3,
                            completion_tokens: 1,
                        }),
                        tool_calls: vec![ModelToolCall {
                            id: "call-stream".into(),
                            name: "sum".into(),
                            input: json!({ "a": 1, "b": 4 }),
                        }],
                    }),
                    1 => Ok(ModelResponse {
                        text: "5".into(),
                        data: Value::Null,
                        finish_reason: FinishReason::Stop,
                        usage: Some(UsageStats {
                            prompt_tokens: 5,
                            completion_tokens: 2,
                        }),
                        tool_calls: Vec::new(),
                    }),
                    other => panic!("unexpected model step {other}"),
                }
            }
        });

        let sum_tool = Tool::new("sum", "add numbers", |input, _context| async move {
            let a = input.get("a").and_then(Value::as_i64).unwrap_or_default();
            let b = input.get("b").and_then(Value::as_i64).unwrap_or_default();
            Ok(json!(a + b))
        });

        let agent = Agent::new(AgentConfig {
            id: "tool-stream".into(),
            name: "Tool Stream".into(),
            instructions: "Use tools when helpful".into(),
            description: None,
            model: Arc::new(model),
            tools: vec![sum_tool],
            memory: None,
            memory_config: MemoryConfig::default(),
        });

        let events = agent
            .stream(AgentStreamRequest {
                prompt: "1 + 4 = ?".into(),
                thread_id: Some("thread-stream".into()),
                resource_id: None,
                run_id: Some("run-stream".into()),
                max_steps: Some(4),
                request_context: RequestContext::new(),
                ..Default::default()
            })
            .collect::<Vec<_>>()
            .await;

        assert_eq!(events.len(), 3);

        match &events[0].as_ref().expect("tool call event").event {
            ModelEvent::ToolCall(call) => {
                assert_eq!(call.id, "call-stream");
                assert_eq!(call.name, "sum");
                assert_eq!(call.input, json!({ "a": 1, "b": 4 }));
            }
            other => panic!("expected tool call event, got {other:?}"),
        }

        match &events[1].as_ref().expect("tool result event").event {
            ModelEvent::ToolResult(result) => {
                assert_eq!(result.id, "call-stream");
                assert_eq!(result.name, "sum");
                assert_eq!(result.output, json!(5));
            }
            other => panic!("expected tool result event, got {other:?}"),
        }

        match &events[2].as_ref().expect("done event").event {
            ModelEvent::Done(response) => {
                assert_eq!(response.text, "5");
                assert_eq!(
                    response.usage,
                    Some(UsageStats {
                        prompt_tokens: 8,
                        completion_tokens: 3,
                    })
                );
            }
            other => panic!("expected done event, got {other:?}"),
        }
    }
}
