use crate::tools::{ToolCall, handle_tool_call};

use async_openai::error::OpenAIError;
use async_openai::types::chat::CreateChatCompletionResponse;
use async_openai::{Client, config::OpenAIConfig};
use color_eyre::Result;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style, Stylize},
    widgets::{Paragraph, Widget},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::env;
use std::process;
use std::sync::{Arc, RwLock};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Message {
    pub role: Option<String>,
    pub content: Option<String>,
    pub tool_call_id: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
}

const LLM_MODEL: &str = "mistralai/devstral-2512";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum LoadingState {
    #[default]
    Idle,
    Loading,
    Loaded,
    Error(String),
}

#[derive(Debug, Clone, Default)]
pub struct AgentResponsesWidget {
    pub state: Arc<RwLock<AgentResponsesState>>,
}

#[derive(Debug, Default)]
pub struct AgentResponsesState {
    pub messages: Vec<Message>,
    pub loading_state: LoadingState,
    pub agent_is_active: bool,
}

impl AgentResponsesWidget {
    /// Start fetching the LLM response in the background.
    pub fn run(&self, user_message: String) {
        let this = self.clone();
        let mut state = self.state.write().unwrap();

        // TODO: This is a stopgap measure to prevent user from starting multiple agent loops
        //       There needs to be some user feedback for this.
        if state.agent_is_active {
            return;
        }

        state.messages.push(Message {
            role: Some("user".to_string()),
            tool_calls: None,
            tool_call_id: None,
            content: Some(user_message),
        });
        state.agent_is_active = true;

        tokio::spawn(this.fetch_llm_response());
    }

    pub async fn fetch_llm_response(self) {
        let api_key = env::var("OPENROUTER_API_KEY").unwrap_or_else(|_| {
            eprintln!("OPENROUTER_API_KEY is not set");
            process::exit(1);
        });

        let base_url = env::var("OPENROUTER_BASE_URL")
            .unwrap_or_else(|_| "https://openrouter.ai/api/v1".to_string());

        let config = OpenAIConfig::new()
            .with_api_base(base_url)
            .with_api_key(api_key);

        let client = Client::with_config(config);

        let read_tool = json!({
            "type": "function",
            "function": {
                "name": "Read",
                "description": "Read and return the contents of a file",
                "parameters": {
                    "type": "object",
                    "required": ["file_path"],
                    "properties": {
                        "file_path": {
                            "type": "string",
                            "description": "The path to the file to read",
                        },
                    },
                },
            }
        });

        let write_tool = json!({
          "type": "function",
          "function": {
            "name": "Write",
            "description": "Write content to a file",
            "parameters": {
              "type": "object",
              "required": ["file_path", "content"],
              "properties": {
                "file_path": {
                  "type": "string",
                  "description": "The path of the file to write to"
                },
                "content": {
                  "type": "string",
                  "description": "The content to write to the file"
                }
              }
            }
          }
        });

        let bash_tool = json!({
          "type": "function",
          "function": {
            "name": "Bash",
            "description": "Execute a shell command",
            "parameters": {
              "type": "object",
              "required": ["command"],
              "properties": {
                "command": {
                  "type": "string",
                  "description": "The command to execute"
                }
              }
            }
          }
        });

        let mut counter = 0;

        let mut is_active = true;
        while is_active {
            let messages = {
                let state = self.state.write().unwrap();
                state.messages.clone()
            };

            counter += 1;
            if counter > 6 {
                is_active = false;
                {
                    let mut state = self.state.write().unwrap();
                    state.agent_is_active = false;
                }
                break;
            }

            let openrouter_response: Result<Value, OpenAIError> = client
                .chat()
                .create_byot(json!({
                    "messages": json!(messages),
                    "model": LLM_MODEL,
                    "tools": [
                        read_tool,
                        write_tool,
                        bash_tool,
                    ]
                }))
                .await;

            match openrouter_response {
                Ok(response) => {
                    let assistant_response =
                        match Message::deserialize(response["choices"][0]["message"].clone()) {
                            Ok(message) => message,
                            Err(error) => panic!("Failed to deserialize message: {:?}", error),
                        };

                    // Lock the state, write to it and unlock it again.
                    {
                        let mut state = self.state.write().unwrap();
                        state.messages.push(assistant_response.clone());
                    }

                    if let Some(tools) = assistant_response.tool_calls {
                        for tool_call in tools {
                            let tool_call_result = match handle_tool_call(&tool_call) {
                                Ok(tool_call_result) => tool_call_result,
                                Err(error) => panic!("Failed to handle tool call: {:?}", error),
                            };

                            {
                                let mut state = self.state.write().unwrap();
                                state.messages.push(Message {
                                    tool_call_id: Some(tool_call_result.id),
                                    content: Some(tool_call_result.content.clone()),
                                    role: Some("tool".to_string()),
                                    tool_calls: None,
                                });
                            }
                        }
                    } else {
                        {
                            let mut state = self.state.write().unwrap();
                            state.agent_is_active = false;
                        }
                        is_active = false;
                    }
                }
                Err(error) => {
                    // eprintln!("Agent error occurred while fetching llm response. {}");
                    {
                        let mut state = self.state.write().unwrap();
                        state.agent_is_active = false;
                    }
                    is_active = false;
                    break;
                }
            }
        }
    }

    pub fn clear_messages(&self) {
        let mut state = self.state.write().unwrap();
        state.messages.clear();
    }
}

impl Widget for &AgentResponsesWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let messages = {
            let state = self.state.write().unwrap();
            state.messages.clone()
        };
        let agent_is_active = {
            let state = self.state.read().unwrap();
            state.agent_is_active.clone()
        };

        let mut last_response_message = "".to_string();
        let mut agent_state = if agent_is_active {
            "Agent Active"
        } else {
            "Agent Idle"
        }
        .to_string();
        let mut is_tool_call = false;

        const MAX_NUMBER_MESSAGES: usize = 4;
        let areas = Layout::vertical([
            Constraint::Percentage(20),
            Constraint::Percentage(20),
            Constraint::Percentage(20),
            Constraint::Percentage(20),
        ])
        .spacing(1)
        .margin(1)
        .split(area);

        for (index, message) in messages.iter().rev().enumerate() {
            if index >= MAX_NUMBER_MESSAGES {
                return;
            }

            let message = message.clone();
            let content = message.content.unwrap_or("".to_string()).clone();
            is_tool_call = !message.tool_calls.is_none() || !message.tool_call_id.is_none();

            let color = if is_tool_call {
                Color::Gray
            } else {
                Color::White
            };

            let paragraph = Paragraph::new(content)
                .style(Style::default().fg(color))
                .render(areas[index], buf);
        }
    }
}
