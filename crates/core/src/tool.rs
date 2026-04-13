use std::collections::HashMap;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Context;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub input_schema: Value,
}

impl ToolSpec {
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema: Value::String(String::from("string")),
        }
    }

    pub fn with_input_schema(mut self, input_schema: Value) -> Self {
        self.input_schema = input_schema;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolResult {
    pub output: Value,
}

impl ToolResult {
    pub fn new(output: impl Into<Value>) -> Self {
        Self {
            output: output.into(),
        }
    }

    pub fn text(output: impl Into<String>) -> Self {
        Self::new(Value::String(output.into()))
    }

    pub fn render(&self) -> String {
        match &self.output {
            Value::String(value) => value.clone(),
            other => serde_json::to_string_pretty(other).unwrap_or_else(|_| other.to_string()),
        }
    }
}

pub trait Tool: Send + Sync {
    fn spec(&self) -> ToolSpec;

    fn call(&self, context: &Context, input: &Value) -> Result<ToolResult>;
}

#[derive(Default)]
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<T: Tool + 'static>(&mut self, tool: T) {
        let spec = tool.spec();
        self.tools.insert(spec.name, Box::new(tool));
    }

    pub fn specs(&self) -> Vec<ToolSpec> {
        let mut specs = self
            .tools
            .values()
            .map(|tool| tool.spec())
            .collect::<Vec<_>>();
        specs.sort_by(|a, b| a.name.cmp(&b.name));
        specs
    }

    pub fn call(&self, name: &str, context: &Context, input: &Value) -> Result<ToolResult> {
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| anyhow!("unknown tool: {name}"))?;
        tool.call(context, input)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }
}
