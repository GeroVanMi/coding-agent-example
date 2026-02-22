mod tools;
mod agent_responses;

use crate::agent_responses::{AgentResponsesWidget, Message};
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
    agent_responses: AgentResponsesWidget,
    character_index: usize,
    input: String,
}

impl App {
    const FRAMES_PER_SECOND: f32 = 60.0;

    pub async fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
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

        let agent_state = {
            let mut state = self.agent_responses.state.write().unwrap();
            if state.agent_is_active.clone() {
                "Agent working ..."
            } else {
                "Agent idle"
            }
        };

        let messages_block = Block::bordered()
            .title(agent_state)
            .title_bottom("esc to quit");
        let messages_block_area = messages_block.inner(messages_area);

        let input = Paragraph::new(self.input.as_str())
            .style(Style::default().fg(Color::Yellow))
            .block(Block::bordered().title("Input"));

        frame.render_widget(title, title_area);
        frame.render_widget(input, input_area);

        frame.set_cursor_position(Position::new(
            input_area.x + self.character_index as u16 + 1,
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
            let current_index = self.character_index;
            let from_left_to_current_index = current_index - 1;

            let before_char_to_delete = self.input.chars().take(from_left_to_current_index);
            let after_char_to_delete = self.input.chars().skip(current_index);

            self.input = before_char_to_delete.chain(after_char_to_delete).collect();
            self.move_cursor_left();
        }
    }

    fn submit_message(&mut self) {
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
