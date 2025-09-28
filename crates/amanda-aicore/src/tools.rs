use amanda_shared::async_trait::async_trait;
use amanda_shared::color_eyre;
use amanda_shared::serde_json::Value;
use genai::chat::Tool as GenaiTool;

/// Trait for defining tools that can be used with the genai crate.
/// Implementors provide the name, description, JSON schema, and execution logic.
#[async_trait]
pub trait Tool {
    /// Returns the name of the tool.
    fn name(&self) -> &str;

    /// Returns a description of what the tool does.
    fn description(&self) -> &str;

    /// Returns the JSON schema for the tool's parameters.
    fn schema(&self) -> Value;

    async fn execute(&self, args: Value) -> color_eyre::Result<Value>;
}

/// A helper struct for managing a collection of tools.
/// It allows adding tools, generating GenaiTool instances for the chat API,
/// and executing tools by name.
#[derive(Default)]
pub struct ToolStore {
    tools: Vec<Box<dyn Tool + Send + Sync>>,
}

impl ToolStore {
    /// Creates a new empty ToolHelper.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a tool to the helper.
    /// The tool must be Send + Sync to ensure thread-safety.
    pub fn add_tool<T>(&mut self, tool: T)
    where
        T: Tool + Send + Sync + 'static,
    {
        self.tools.push(Box::new(tool));
    }

    /// Returns a list of GenaiTool instances for all registered tools.
    /// This can be used when building a ChatRequest.
    pub fn get_genai_tools(&self) -> Vec<GenaiTool> {
        self.tools
            .iter()
            .map(|tool| {
                GenaiTool::new(tool.name())
                    .with_description(tool.description())
                    .with_schema(tool.schema())
            })
            .collect()
    }

    /// Executes a tool by name with the given arguments.
    /// Returns the result or an error if the tool is not found or execution fails.
    pub async fn execute_tool(&self, name: &str, args: Value) -> color_eyre::Result<Value> {
        for tool in &self.tools {
            if tool.name() == name {
                return tool.execute(args).await;
            }
        }

        Err(color_eyre::eyre::eyre!(format!(
            "Tool '{}' not found",
            name
        )))
    }
}
