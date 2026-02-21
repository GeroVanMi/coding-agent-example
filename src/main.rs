mod tools;

use crate::tools::ToolCall;

use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use color_eyre::Result;
use crossterm::event::Event::Key;
use crossterm::event::{Event, EventStream, KeyCode};
use octocrab::Page;
use octocrab::params::Direction;
use octocrab::params::pulls::Sort;

use ratatui::{
    DefaultTerminal, Frame,
    buffer::Buffer,
    layout::{Constraint, Layout, Position, Rect},
    style::{Color, Style, Stylize},
    text::Line,
    widgets::{Block, HighlightSpacing, Paragraph, Row, StatefulWidget, Table, TableState, Widget},
};
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

// const LLM_MODEL: &str = "mistralai/devstral-2512";

#[tokio::main]
async fn main() -> Result<()> {
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

        // frame.render_widget(&self.pull_requests, body_area);
    }

    fn handle_event(&mut self, event: &Event) {
        if let Some(key) = event.as_key_press_event() {
            match key.code {
                KeyCode::Esc => self.should_quit = true,
                KeyCode::Char(to_insert) => self.enter_char(to_insert),
                // KeyCode::Enter => self.submit_message(),
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

    fn submit_answer() {}
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
    table_state: TableState,
}

impl AgentResponsesWidget {
    /// Start fetching the LLM response in the background.
    fn run(&self) {
        let this = self.clone();
        tokio::spawn(this.fetch_llm_response());
    }

    async fn fetch_llm_response(self) {
        //
        //     let api_key = env::var("OPENROUTER_API_KEY").unwrap_or_else(|_| {
        //         eprintln!("OPENROUTER_API_KEY is not set");
        //         process::exit(1);
        //     });
        //
        //     let config = OpenAIConfig::new()
        //         .with_api_base(base_url)
        //         .with_api_key(api_key);
        //
        //     let client = Client::with_config(config);
        //
        //     let read_tool = json!({
        //         "type": "function",
        //         "function": {
        //             "name": "Read",
        //             "description": "Read and return the contents of a file",
        //             "parameters": {
        //                 "type": "object",
        //                 "required": ["file_path"],
        //                 "properties": {
        //                     "file_path": {
        //                         "type": "string",
        //                         "description": "The path to the file to read",
        //                     },
        //                 },
        //             },
        //         }
        //     });
        //
        //     let write_tool = json!({
        //           "type": "function",
        //           "function": {
        //             "name": "Write",
        //             "description": "Write content to a file",
        //             "parameters": {
        //               "type": "object",
        //               "required": ["file_path", "content"],
        //               "properties": {
        //                 "file_path": {
        //                   "type": "string",
        //                   "description": "The path of the file to write to"
        //                 },
        //                 "content": {
        //                   "type": "string",
        //                   "description": "The content to write to the file"
        //                 }
        //               }
        //             }
        //           }
        //         }
        //     );
        //
        //     let bash_tool = json!({
        //           "type": "function",
        //           "function": {
        //             "name": "Bash",
        //             "description": "Execute a shell command",
        //             "parameters": {
        //               "type": "object",
        //               "required": ["command"],
        //               "properties": {
        //                 "command": {
        //                   "type": "string",
        //                   "description": "The command to execute"
        //                 }
        //               }
        //             }
        //           }
        //         }
        //     );
    }
}

// impl PullRequestListWidget {
//     /// Start fetching the pull requests in the background.
//     ///
//     /// This method spawns a background task that fetches the pull requests from the GitHub API.
//     /// The result of the fetch is then passed to the `on_load` or `on_err` methods.
//     fn run(&self) {
//         let this = self.clone(); // clone the widget to pass to the background task
//         tokio::spawn(this.fetch_pulls());
//     }
//
//     async fn fetch_pulls(self) {
//         // this runs once, but you could also run this in a loop, using a channel that accepts
//         // messages to refresh on demand, or with an interval timer to refresh every N seconds
//         self.set_loading_state(LoadingState::Loading);
//         match octocrab::instance()
//             .pulls("ratatui", "ratatui")
//             .list()
//             .sort(Sort::Updated)
//             .direction(Direction::Descending)
//             .send()
//             .await
//         {
//             Ok(page) => self.on_load(&page),
//             Err(err) => self.on_err(&err),
//         }
//     }
//     fn on_load(&self, page: &Page<OctoPullRequest>) {
//         let prs = page.items.iter().map(Into::into);
//         let mut state = self.state.write().unwrap();
//         state.loading_state = LoadingState::Loaded;
//         state.pull_requests.extend(prs);
//         if !state.pull_requests.is_empty() {
//             state.table_state.select(Some(0));
//         }
//     }
//
//     fn on_err(&self, err: &octocrab::Error) {
//         self.set_loading_state(LoadingState::Error(err.to_string()));
//     }
//
//     fn set_loading_state(&self, state: LoadingState) {
//         self.state.write().unwrap().loading_state = state;
//     }
//
//     fn scroll_down(&self) {
//         self.state.write().unwrap().table_state.scroll_down_by(1);
//     }
//
//     fn scroll_up(&self) {
//         self.state.write().unwrap().table_state.scroll_up_by(1);
//     }
// }
//
// type OctoPullRequest = octocrab::models::pulls::PullRequest;
//
// impl From<&OctoPullRequest> for PullRequest {
//     fn from(pr: &OctoPullRequest) -> Self {
//         Self {
//             id: pr.number.to_string(),
//             title: pr.title.as_ref().unwrap().to_string(),
//             url: pr
//                 .html_url
//                 .as_ref()
//                 .map(ToString::to_string)
//                 .unwrap_or_default(),
//         }
//     }
// }
//
// impl Widget for &PullRequestListWidget {
//     fn render(self, area: Rect, buf: &mut Buffer) {
//         let mut state = self.state.write().unwrap();
//
//         // a block with a right aligned title with the loading state on the right
//         let loading_state = Line::from(format!("{:?}", state.loading_state)).right_aligned();
//         let block = Block::bordered()
//             .title("Pull Requests")
//             .title(loading_state)
//             .title_bottom("j/k to scroll, q to quit");
//
//         // a table with the list of pull requests
//         let rows = state.pull_requests.iter();
//         let widths = [
//             Constraint::Length(5),
//             Constraint::Fill(1),
//             Constraint::Max(49),
//         ];
//         let table = Table::new(rows, widths)
//             .block(block)
//             .highlight_spacing(HighlightSpacing::Always)
//             .highlight_symbol(">>")
//             .row_highlight_style(Style::new().on_blue());
//
//         StatefulWidget::render(table, area, buf, &mut state.table_state);
//     }
// }
//
// impl From<&PullRequest> for Row<'_> {
//     fn from(pr: &PullRequest) -> Self {
//         let pr = pr.clone();
//         Row::new(vec![pr.id, pr.title, pr.url])
//     }
// }

// #[tokio::main]
// async fn main() -> Result<(), Box<dyn std::error::Error>> {
//     let args = Args::parse();
//
//     dotenvy::dotenv().ok();
//
//     let base_url = env::var("OPENROUTER_BASE_URL")
//         .unwrap_or_else(|_| "https://openrouter.ai/api/v1".to_string());
//
//     let api_key = env::var("OPENROUTER_API_KEY").unwrap_or_else(|_| {
//         eprintln!("OPENROUTER_API_KEY is not set");
//         process::exit(1);
//     });
//
//     let config = OpenAIConfig::new()
//         .with_api_base(base_url)
//         .with_api_key(api_key);
//
//     let client = Client::with_config(config);
//
//     let read_tool = json!({
//         "type": "function",
//         "function": {
//             "name": "Read",
//             "description": "Read and return the contents of a file",
//             "parameters": {
//                 "type": "object",
//                 "required": ["file_path"],
//                 "properties": {
//                     "file_path": {
//                         "type": "string",
//                         "description": "The path to the file to read",
//                     },
//                 },
//             },
//         }
//     });
//
//     let write_tool = json!({
//           "type": "function",
//           "function": {
//             "name": "Write",
//             "description": "Write content to a file",
//             "parameters": {
//               "type": "object",
//               "required": ["file_path", "content"],
//               "properties": {
//                 "file_path": {
//                   "type": "string",
//                   "description": "The path of the file to write to"
//                 },
//                 "content": {
//                   "type": "string",
//                   "description": "The content to write to the file"
//                 }
//               }
//             }
//           }
//         }
//     );
//
//     let bash_tool = json!({
//           "type": "function",
//           "function": {
//             "name": "Bash",
//             "description": "Execute a shell command",
//             "parameters": {
//               "type": "object",
//               "required": ["command"],
//               "properties": {
//                 "command": {
//                   "type": "string",
//                   "description": "The command to execute"
//                 }
//               }
//             }
//           }
//         }
//     );
//
//     let initial_message = Message {
//         role: Some("user".to_string()),
//         content: Some(args.prompt),
//         tool_call_id: None,
//         tool_calls: None,
//     };
//
//     let mut messages: Vec<Message> = vec![initial_message];
//
//     let mut is_running = true;
//     let mut counter = 0;
//
//     while (is_running) {
//         // for message in &mut messages {
//         //     display_message(message.clone())
//         // }
//
//         counter += 1;
//         if (counter > 15) {
//             is_running = false;
//         }
//
//         #[allow(unused_variables)]
//         let response: Value = client
//             .chat()
//             .create_byot(json!({
//                 "messages": json!(messages),
//                 "model": LLM_MODEL,
//                 "tools": [
//                     read_tool,
//                     write_tool,
//                     bash_tool,
//                 ]
//             }))
//             .await?;
//
//         // You can use print statements as follows for debugging, they'll be visible when running tests.
//
//         let assistant_response =
//             match Message::deserialize(response["choices"][0]["message"].clone()) {
//                 Ok(message) => message,
//                 Err(error) => panic!("Failed to deserialize message: {:?}", error),
//             };
//
//         messages.push(assistant_response.clone());
//
//         if let Some(tools) = assistant_response.tool_calls {
//             for tool_call in tools {
//                 let tool_call_result = match handle_tool_call(&tool_call) {
//                     Ok(tool_call_result) => tool_call_result,
//                     Err(error) => panic!("Failed to handle tool call: {:?}", error),
//                 };
//
//                 messages.push(Message {
//                     tool_call_id: Some(tool_call_result.id),
//                     content: Some(tool_call_result.content.clone()),
//                     role: Some("tool".to_string()),
//                     tool_calls: None,
//                 });
//             }
//         } else {
//             if let Some(content) = assistant_response.content {
//                 print!("{}", content);
//             }
//
//             is_running = false;
//         }
//     }
//
//     Ok(())
// }

// fn display_message(message: Message) {
//     println!("{}", json!(message));
// }
