use crate::tools::bash::execute_bash_command;
use crate::tools::read::read_file;
use crate::tools::write::write;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::error::Error;

mod bash;
pub mod read;
pub mod write;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Function {
    pub name: String,
    pub arguments: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ToolCall {
    pub id: String,
    pub r#type: String,
    pub function: Function,
}

pub struct ToolCallResponse {
    pub id: String,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct ToolError;

impl Error for ToolError {}

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "The tool call could not be handled.")
    }
}

pub fn handle_tool_call(tool_call: &ToolCall) -> Result<ToolCallResponse, ToolError> {
    let id = tool_call.id.as_str();
    let tool_name = tool_call.function.name.as_str();
    let args: Value =
        serde_json::from_str(tool_call.function.arguments.as_str()).map_err(|_| ToolError)?;

    match tool_name {
        "Read" => {
            let file_path: &str = args["file_path"].as_str().unwrap_or("");
            let contents = read_file(file_path).map_err(|_| ToolError)?;
            Ok(ToolCallResponse {
                id: id.to_string(),
                content: contents.to_string(),
            })
        }

        "Write" => {
            let file_path: &str = args["file_path"].as_str().unwrap_or("");
            let content: &str = args["content"].as_str().unwrap_or("");
            write(file_path, content).map_err(|_| ToolError)?;

            Ok(ToolCallResponse {
                id: id.to_string(),
                content: content.to_string(),
            })
        }

        "Bash" => {
            let command: &str = args["command"].as_str().unwrap_or("");
            let output = execute_bash_command(command).map_err(|_| ToolError)?;
            Ok(ToolCallResponse {
                id: id.to_string(),
                content: output,
            })
        }

        // Unknown tool name
        _ => Err(ToolError),
    }
}
