mod tools;

use crate::tools::{ToolCall, handle_tool_call};

use async_openai::error::OpenAIError;
use async_openai::types::chat::CreateChatCompletionResponse;
use async_openai::{Client, config::OpenAIConfig};
use color_eyre::Result;
use color_eyre::owo_colors::OwoColorize;
use crossterm::event::Event::Key;
use crossterm::event::{Event, EventStream, KeyCode};
use dotenvy;
use ratatui::macros::constraints;
use ratatui::{
    DefaultTerminal, Frame,
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Position, Rect},
    style::{Color, Style, Stylize},
    text::Line,
    widgets::{
        Block, HighlightSpacing, List, Paragraph, Row, StatefulWidget, Table, TableState, Widget,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::env;
use std::process;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio_stream::StreamExt;

// #[derive(Parser)]
// #[command(author, version, about)]
// struct Args {
//     #[arg(short = 'p', long)]
//     prompt: String,
// }

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Message {
    role: Option<String>,
    content: Option<String>,
    tool_call_id: Option<String>,
    tool_calls: Option<Vec<ToolCall>>,
}

const LLM_MODEL: &str = "mistralai/devstral-2512";

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    color_eyre::install()?;
    let terminal = ratatui::init();
    let app_result = App::default().run(terminal).await;
    ratatui::restore();
    app_result
}

#[derive(Debug, Default)]
struct App {
    should_quit: bool,
    // pull_requests: PullRequestListWidget,
    agent_responses: AgentResponsesWidget,
    character_index: usize,
    input: String,
}

impl App {
    const FRAMES_PER_SECOND: f32 = 60.0;

    pub async fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        // self.pull_requests.run();

        let period = Duration::from_secs_f32(1.0 / Self::FRAMES_PER_SECOND);
        let mut interval = tokio::time::interval(period);
        let mut events = EventStream::new();

        while !self.should_quit {
            tokio::select! {
                _ = interval.tick() => { terminal.draw(|frame| self.render(frame))?; },
                Some(Ok(event)) = events.next() => self.handle_event(&event),
            }
        }
        Ok(())
    }

    fn render(&self, frame: &mut Frame) {
        let layout = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Fill(1),
        ]);
        let [title_area, input_area, messages_area] = frame.area().layout(&layout);
        let title = Line::from("Gero's Coding-Agent").centered().bold();

        let messages_block = Block::bordered()
            .title("Responses")
            .title_bottom("esc to quit");
        let messages_block_area = messages_block.inner(messages_area);

        let input = Paragraph::new(self.input.as_str())
            .style(Style::default().fg(Color::Yellow))
            .block(Block::bordered().title("Input"));

        frame.render_widget(title, title_area);
        frame.render_widget(input, input_area);

        frame.set_cursor_position(Position::new(
            // Draw the cursor at the current position in the input field.
            // This position is can be controlled via the left and right arrow key
            input_area.x + self.character_index as u16 + 1,
            // Move one line down, from the border to the input line
            input_area.y + 1,
        ));

        frame.render_widget(messages_block, messages_area);
        frame.render_widget(&self.agent_responses, messages_block_area);
    }

    fn handle_event(&mut self, event: &Event) {
        if let Some(key) = event.as_key_press_event() {
            match key.code {
                KeyCode::Esc => self.should_quit = true,
                KeyCode::Char(to_insert) => self.enter_char(to_insert),
                KeyCode::Enter => self.submit_message(),
                KeyCode::Backspace => self.delete_char(),
                KeyCode::Left => self.move_cursor_left(),
                KeyCode::Right => self.move_cursor_right(),
                _ => {}
            }
        }
    }

    /**
     * This enables
     */
    fn enter_char(&mut self, new_char: char) {
        let index = self.byte_index();
        self.input.insert(index, new_char);
        self.move_cursor_right();
    }

    fn byte_index(&self) -> usize {
        self.input
            .char_indices()
            .map(|(i, _)| i)
            .nth(self.character_index)
            .unwrap_or(self.input.len())
    }

    fn move_cursor_left(&mut self) {
        let cursor_moved_left = self.character_index.saturating_sub(1);
        self.character_index = self.clamp_cursor(cursor_moved_left);
    }

    fn move_cursor_right(&mut self) {
        let cursor_moved_right = self.character_index.saturating_add(1);
        self.character_index = self.clamp_cursor(cursor_moved_right);
    }
    fn clamp_cursor(&self, new_cursor_pos: usize) -> usize {
        new_cursor_pos.clamp(0, self.input.chars().count())
    }

    fn delete_char(&mut self) {
        let is_not_cursor_leftmost = self.character_index != 0;
        if is_not_cursor_leftmost {
            // Method "remove" is not used on the saved text for deleting the selected char.
            // Reason: Using remove on String works on bytes instead of the chars.
            // Using remove would require special care because of char boundaries.

            let current_index = self.character_index;
            let from_left_to_current_index = current_index - 1;

            // Getting all characters before the selected character.
            let before_char_to_delete = self.input.chars().take(from_left_to_current_index);
            // Getting all characters after selected character.
            let after_char_to_delete = self.input.chars().skip(current_index);

            // Put all characters together except the selected one.
            // By leaving the selected one out, it is forgotten and therefore deleted.
            self.input = before_char_to_delete.chain(after_char_to_delete).collect();
            self.move_cursor_left();
        }
    }

    fn submit_message(&mut self) {
        // TODO: Check if the last message has been processed

        // Abort if the input is empty
        if self.input.is_empty() {
            return;
        }

        if (self.input == "/clear") {
            self.clear_messages();
            self.clear_input();
            return;
        }

        self.agent_responses.run(self.input.clone());
        self.clear_input();
    }

    fn clear_input(&mut self) {
        self.input = String::new();
        self.character_index = 0;
    }

    fn clear_messages(&mut self) {
        self.agent_responses.clear_messages();
    }
}

// #[derive(Debug, Clone, Default)]
// struct PullRequestListWidget {
//     state: Arc<RwLock<PullRequestListState>>,
// }
//
// #[derive(Debug, Default)]
// struct PullRequestListState {
//     pull_requests: Vec<PullRequest>,
//     loading_state: LoadingState,
//     table_state: TableState,
// }
//
// #[derive(Debug, Clone)]
// struct PullRequest {
//     id: String,
//     title: String,
//     url: String,
// }
//

/// Not sure if this is needed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
enum LoadingState {
    #[default]
    Idle,
    Loading,
    Loaded,
    Error(String),
}

#[derive(Debug, Clone, Default)]
struct AgentResponsesWidget {
    state: Arc<RwLock<AgentResponsesState>>,
}
#[derive(Debug, Default)]
struct AgentResponsesState {
    messages: Vec<Message>,
    loading_state: LoadingState,
    agent_is_active: bool,
    // table_state: TableState,
}

impl AgentResponsesWidget {
    /// Start fetching the LLM response in the background.
    fn run(&self, user_message: String) {
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

    async fn fetch_llm_response(self) {
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

    fn clear_messages(&self) {
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
        // let mut role = "".to_string();
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
