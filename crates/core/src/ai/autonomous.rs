//! Autonomous Agent Patterns
//!
//! Implements planning, tool-use loops, and step-by-step execution for
//! autonomous AI agents:
//! - Goal decomposition into executable steps
//! - Plan → execute → observe → replan loop
//! - Human-in-the-loop escalation
//! - Configurable tool registry

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// Definition of a tool that an autonomous agent can invoke.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Unique tool name.
    pub name: String,
    /// Human-readable description of what the tool does.
    pub description: String,
    /// JSON Schema describing expected input.
    pub input_schema: String,
    /// JSON Schema describing expected output.
    pub output_schema: String,
}

impl ToolDefinition {
    /// Create a new tool definition.
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        input_schema: impl Into<String>,
        output_schema: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema: input_schema.into(),
            output_schema: output_schema.into(),
        }
    }
}

/// A single step in an agent's execution plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStep {
    /// Name of the tool to invoke.
    pub tool_name: String,
    /// Serialized input bytes for the tool.
    pub tool_input: Vec<u8>,
    /// Optional expected outcome description for verification.
    pub expected_outcome: Option<String>,
}

impl AgentStep {
    /// Create a new agent step.
    pub fn new(tool_name: impl Into<String>, tool_input: Vec<u8>) -> Self {
        Self {
            tool_name: tool_name.into(),
            tool_input,
            expected_outcome: None,
        }
    }

    /// Set expected outcome.
    pub fn with_expected_outcome(mut self, outcome: impl Into<String>) -> Self {
        self.expected_outcome = Some(outcome.into());
        self
    }
}

/// An agent execution plan decomposed from a goal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPlan {
    /// The overall goal the agent is trying to achieve.
    pub goal: String,
    /// Ordered steps to execute.
    pub steps: Vec<AgentStep>,
    /// Index of the current step being executed.
    pub current_step: usize,
    /// Maximum number of steps allowed.
    pub max_steps: usize,
}

impl AgentPlan {
    /// Create a new plan.
    pub fn new(goal: impl Into<String>, steps: Vec<AgentStep>, max_steps: usize) -> Self {
        Self {
            goal: goal.into(),
            steps,
            current_step: 0,
            max_steps,
        }
    }

    /// Returns `true` if all steps have been executed.
    pub fn is_complete(&self) -> bool {
        self.current_step >= self.steps.len()
    }

    /// Returns the current step (if any remain).
    pub fn current(&self) -> Option<&AgentStep> {
        self.steps.get(self.current_step)
    }

    /// Advance to the next step.
    pub fn advance(&mut self) {
        if self.current_step < self.steps.len() {
            self.current_step += 1;
        }
    }

    /// Returns the number of remaining steps.
    pub fn remaining_steps(&self) -> usize {
        self.steps.len().saturating_sub(self.current_step)
    }
}

/// Result of executing a single agent step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    /// Output from the tool execution (if successful).
    pub output: Option<Vec<u8>>,
    /// Whether human intervention is required.
    pub needs_human: bool,
    /// Error message (if the step failed).
    pub error: Option<String>,
}

impl StepResult {
    /// Create a successful step result.
    pub fn success(output: Vec<u8>) -> Self {
        Self {
            output: Some(output),
            needs_human: false,
            error: None,
        }
    }

    /// Create an error step result.
    pub fn failure(message: impl Into<String>) -> Self {
        Self {
            output: None,
            needs_human: false,
            error: Some(message.into()),
        }
    }

    /// Create a human-escalation result.
    pub fn needs_human(message: impl Into<String>) -> Self {
        Self {
            output: None,
            needs_human: true,
            error: Some(message.into()),
        }
    }

    /// Returns `true` if the step succeeded.
    pub fn is_success(&self) -> bool {
        self.output.is_some() && self.error.is_none()
    }
}

/// A tool executor function type.
pub type ToolExecutor = Box<dyn Fn(&[u8]) -> Result<Vec<u8>> + Send + Sync>;

/// An autonomous agent with planning and execution capabilities.
pub struct AutonomousAgent {
    /// Registered tools and their executors.
    tools: HashMap<String, ToolExecutor>,
    /// Tool definitions for planning.
    tool_definitions: Vec<ToolDefinition>,
    /// Current execution plan (if any).
    plan: Option<AgentPlan>,
    /// Maximum steps per plan (default: 20).
    max_steps: usize,
    /// Whether the agent requires human confirmation before executing
    /// each step.
    human_in_the_loop: bool,
}

impl AutonomousAgent {
    /// Create a new autonomous agent.
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            tool_definitions: Vec::new(),
            plan: None,
            max_steps: 20,
            human_in_the_loop: false,
        }
    }

    /// Register a tool with its executor.
    pub fn register_tool(&mut self, definition: ToolDefinition, executor: ToolExecutor) {
        self.tools.insert(definition.name.clone(), executor);
        self.tool_definitions.push(definition);
    }

    /// Set maximum steps per plan.
    pub fn with_max_steps(mut self, max: usize) -> Self {
        self.max_steps = max;
        self
    }

    /// Enable/disable human-in-the-loop mode.
    pub fn with_human_in_the_loop(mut self, enabled: bool) -> Self {
        self.human_in_the_loop = enabled;
        self
    }

    /// Decompose a goal into an execution plan.
    ///
    /// This uses a heuristic planner that matches goal keywords to
    /// available tools and creates a sequential plan.
    pub fn plan(&mut self, goal: &str, available_tools: &[ToolDefinition]) -> Result<AgentPlan> {
        let goal_lower = goal.to_lowercase();
        let mut steps = Vec::new();

        for tool_def in available_tools {
            let desc_lower = tool_def.description.to_lowercase();
            let name_lower = tool_def.name.to_lowercase();

            let goal_words: Vec<&str> = goal_lower.split_whitespace().collect();
            let desc_words: Vec<&str> = desc_lower.split_whitespace().collect();
            let name_words: Vec<&str> = name_lower.split_whitespace().collect();

            let relevance = goal_words
                .iter()
                .filter(|w| desc_words.contains(w) || name_words.contains(w))
                .count();

            if relevance > 0 || self.goal_directly_mentions_tool(&goal_lower, &tool_def.name) {
                let input = format!("goal: {}", goal);
                steps.push(
                    AgentStep::new(&tool_def.name, input.into_bytes())
                        .with_expected_outcome(format!("Apply {} toward goal", tool_def.name)),
                );
            }

            if steps.len() >= self.max_steps {
                break;
            }
        }

        if steps.is_empty() {
            return Err(Error::internal(format!("no tools matched goal: {}", goal)));
        }

        let plan = AgentPlan::new(goal, steps, self.max_steps);
        self.plan = Some(plan.clone());
        Ok(plan)
    }

    /// Execute the next step in the current plan.
    pub fn execute_step(&mut self) -> Result<StepResult> {
        let plan = self
            .plan
            .as_mut()
            .ok_or_else(|| Error::internal("no active plan"))?;

        if plan.is_complete() {
            return Ok(StepResult::success(b"plan complete".to_vec()));
        }

        let step = plan
            .current()
            .ok_or_else(|| Error::internal("no current step"))?;

        let tool_name = step.tool_name.clone();
        let tool_input = step.tool_input.clone();
        let human_in_loop = self.human_in_the_loop;

        if human_in_loop {
            plan.advance();
            return Ok(StepResult::needs_human(format!(
                "Awaiting human approval for step {}: {}",
                plan.current_step.saturating_sub(1),
                tool_name
            )));
        }

        let executor = self
            .tools
            .get(&tool_name)
            .ok_or_else(|| Error::internal(format!("tool '{}' not registered", tool_name)))?;

        match executor(&tool_input) {
            Ok(output) => {
                plan.advance();
                Ok(StepResult::success(output))
            }
            Err(e) => {
                plan.advance();
                Ok(StepResult::failure(e.to_string()))
            }
        }
    }

    /// Run the full plan to completion (or until an error/human-escalation).
    ///
    /// Returns all step results in order.
    pub fn run_plan(&mut self) -> Vec<StepResult> {
        let mut results = Vec::new();
        loop {
            match self.execute_step() {
                Ok(result) => {
                    let is_final =
                        result.needs_human || self.plan.as_ref().is_none_or(|p| p.is_complete());
                    results.push(result);
                    if is_final {
                        break;
                    }
                }
                Err(e) => {
                    results.push(StepResult::failure(e.to_string()));
                    break;
                }
            }
        }
        results
    }

    /// Get the current plan (if any).
    pub fn current_plan(&self) -> Option<&AgentPlan> {
        self.plan.as_ref()
    }

    /// Returns the list of registered tool definitions.
    pub fn tool_definitions(&self) -> &[ToolDefinition] {
        &self.tool_definitions
    }

    fn goal_directly_mentions_tool(&self, goal: &str, tool_name: &str) -> bool {
        let tool_words: Vec<&str> = tool_name.split('_').collect();
        goal.split_whitespace().any(|gw| {
            tool_words
                .iter()
                .any(|tw| tw.starts_with(gw) || gw.starts_with(tw))
        })
    }
}

impl Default for AutonomousAgent {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_search_tool() -> (ToolDefinition, ToolExecutor) {
        let def = ToolDefinition::new(
            "search",
            "Search for information",
            r#"{"type":"object","properties":{"query":{"type":"string"}}}"#,
            r#"{"type":"object","properties":{"results":{"type":"array"}}}"#,
        );
        let executor: ToolExecutor = Box::new(|input: &[u8]| {
            let input_str = String::from_utf8_lossy(input);
            if input_str.contains("error") {
                return Err(Error::internal("search failed"));
            }
            Ok(format!("results for: {}", input_str).into_bytes())
        });
        (def, executor)
    }

    fn make_write_tool() -> (ToolDefinition, ToolExecutor) {
        let def = ToolDefinition::new(
            "write",
            "Write content to a file",
            r#"{"type":"object","properties":{"path":{"type":"string"},"content":{"type":"string"}}}"#,
            r#"{"type":"object","properties":{"success":{"type":"boolean"}}}"#,
        );
        let executor: ToolExecutor = Box::new(|input: &[u8]| {
            Ok(format!("written: {}", String::from_utf8_lossy(input)).into_bytes())
        });
        (def, executor)
    }

    #[test]
    fn test_tool_definition_creation() {
        let tool = ToolDefinition::new("test", "A test tool", "{}", "{}");
        assert_eq!(tool.name, "test");
        assert_eq!(tool.description, "A test tool");
    }

    #[test]
    fn test_agent_step_creation() {
        let step =
            AgentStep::new("search", b"query=test".to_vec()).with_expected_outcome("found results");
        assert_eq!(step.tool_name, "search");
        assert_eq!(step.tool_input, b"query=test");
        assert_eq!(step.expected_outcome.as_deref(), Some("found results"));
    }

    #[test]
    fn test_plan_complete_check() {
        let plan = AgentPlan::new("test", Vec::new(), 10);
        assert!(plan.is_complete());
        assert_eq!(plan.remaining_steps(), 0);
    }

    #[test]
    fn test_plan_advance() {
        let steps = vec![AgentStep::new("a", vec![]), AgentStep::new("b", vec![])];
        let mut plan = AgentPlan::new("goal", steps, 10);
        assert_eq!(plan.current_step, 0);
        assert!(!plan.is_complete());
        plan.advance();
        assert_eq!(plan.current_step, 1);
        assert!(!plan.is_complete());
        plan.advance();
        assert!(plan.is_complete());
    }

    #[test]
    fn test_step_result_success() {
        let r = StepResult::success(b"ok".to_vec());
        assert!(r.is_success());
        assert!(!r.needs_human);
        assert!(r.error.is_none());
    }

    #[test]
    fn test_step_result_failure() {
        let r = StepResult::failure("something went wrong");
        assert!(!r.is_success());
        assert!(!r.needs_human);
        assert_eq!(r.error.as_deref(), Some("something went wrong"));
    }

    #[test]
    fn test_step_result_needs_human() {
        let r = StepResult::needs_human("uncertain action");
        assert!(r.needs_human);
        assert!(!r.is_success());
    }

    #[test]
    fn test_plan_goal_matching() {
        let (search_def, search_exec) = make_search_tool();
        let (write_def, write_exec) = make_write_tool();

        let mut agent = AutonomousAgent::new();
        agent.register_tool(search_def, search_exec);
        agent.register_tool(write_def, write_exec);

        let tool_defs = agent.tool_definitions().to_vec();
        let plan = agent
            .plan("search for Rust documentation", &tool_defs)
            .expect("plan should match search");

        assert!(!plan.steps.is_empty());
        assert!(plan.steps.iter().any(|s| s.tool_name == "search"));
    }

    #[test]
    fn test_plan_no_match() {
        let mut agent = AutonomousAgent::new();
        let result = agent.plan("do something impossible", &[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_execute_step_success() {
        let (search_def, search_exec) = make_search_tool();

        let mut agent = AutonomousAgent::new();
        agent.register_tool(search_def, search_exec);

        let tool_defs = agent.tool_definitions().to_vec();
        agent.plan("search for data", &tool_defs).expect("plan");

        let result = agent.execute_step().expect("execute");
        assert!(result.is_success());
    }

    #[test]
    fn test_execute_step_no_plan() {
        let mut agent = AutonomousAgent::new();
        let result = agent.execute_step();
        assert!(result.is_err());
    }

    #[test]
    fn test_execute_step_error_propagation() {
        let (search_def, search_exec) = make_search_tool();

        let mut agent = AutonomousAgent::new();
        agent.register_tool(search_def, search_exec);

        let tool_defs = agent.tool_definitions().to_vec();
        agent.plan("search error case", &tool_defs).expect("plan");

        let result = agent.execute_step().expect("execute");
        assert!(!result.is_success());
        assert!(result.error.is_some());
    }

    #[test]
    fn test_human_in_the_loop() {
        let (search_def, search_exec) = make_search_tool();

        let mut agent = AutonomousAgent::new().with_human_in_the_loop(true);
        agent.register_tool(search_def, search_exec);

        let tool_defs = agent.tool_definitions().to_vec();
        agent.plan("search data", &tool_defs).expect("plan");

        let result = agent.execute_step().expect("execute");
        assert!(result.needs_human);
    }

    #[test]
    fn test_run_plan_to_completion() {
        let (search_def, search_exec) = make_search_tool();

        let mut agent = AutonomousAgent::new();
        agent.register_tool(search_def, search_exec);

        let tool_defs = agent.tool_definitions().to_vec();
        agent.plan("search information", &tool_defs).expect("plan");

        let results = agent.run_plan();
        assert!(!results.is_empty());
        assert!(results.last().map_or(false, |r| r.is_success()));
    }
}
