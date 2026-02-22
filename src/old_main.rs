use crate::tools::{ToolCall, handle_tool_call};
use async_openai::{Client, config::OpenAIConfig};
use clap::Parser;
use dotenvy;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{env, process};
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    dotenvy::dotenv().ok();

    let base_url = env::var("OPENROUTER_BASE_URL")
        .unwrap_or_else(|_| "https://openrouter.ai/api/v1".to_string());

    let api_key = env::var("OPENROUTER_API_KEY").unwrap_or_else(|_| {
        eprintln!("OPENROUTER_API_KEY is not set");
        process::exit(1);
    });

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
        }
    );

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
        }
    );

    let initial_message = Message {
        role: Some("user".to_string()),
        content: Some(args.prompt),
        tool_call_id: None,
        tool_calls: None,
    };

    let mut messages: Vec<Message> = vec![initial_message];

    let mut is_running = true;
    let mut counter = 0;

    while (is_running) {
        // for message in &mut messages {
        //     display_message(message.clone())
        // }

        counter += 1;
        if (counter > 15) {
            is_running = false;
        }

        let response: Value = client
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
            .await?;

        // You can use print statements as follows for debugging, they'll be visible when running tests.

        let assistant_response =
            match Message::deserialize(response["choices"][0]["message"].clone()) {
                Ok(message) => message,
                Err(error) => panic!("Failed to deserialize message: {:?}", error),
            };

        messages.push(assistant_response.clone());

        if let Some(tools) = assistant_response.tool_calls {
            for tool_call in tools {
                let tool_call_result = match handle_tool_call(&tool_call) {
                    Ok(tool_call_result) => tool_call_result,
                    Err(error) => panic!("Failed to handle tool call: {:?}", error),
                };

                messages.push(Message {
                    tool_call_id: Some(tool_call_result.id),
                    content: Some(tool_call_result.content.clone()),
                    role: Some("tool".to_string()),
                    tool_calls: None,
                });
            }
        } else {
            if let Some(content) = assistant_response.content {
                print!("{}", content);
            }

            is_running = false;
        }
    }

    Ok(())
}

fn display_message(message: Message) {
    println!("{}", json!(message));
}
