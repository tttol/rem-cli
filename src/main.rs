mod render;
mod task;

use std::io;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::prelude::*;
use task::Task;

use crate::task::TaskStatus;

#[derive(PartialEq)]
pub enum Mode {
    Normal,
    Editing,
}

pub struct App {
    pub should_quit: bool,
    pub input_mode: Mode,
    pub input_buffer: String,
    pub tasks: Vec<Task>,
    pub selected_index: Option<usize>,
}

impl App {
    fn new() -> Self {
        let tasks = Task::load_all().unwrap_or_default();
        let selected_index = if tasks.is_empty() { None } else { Some(0) };
        Self {
            should_quit: false,
            input_mode: Mode::Normal,
            input_buffer: String::new(),
            tasks,
            selected_index,
        }
    }

    fn handle_key_event(&mut self, key_code: KeyCode) {
        match self.input_mode {
            Mode::Normal => match key_code {
                KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
                KeyCode::Char('a') => {
                    self.input_mode = Mode::Editing;
                    self.input_buffer.clear();
                }
                KeyCode::Char('j') | KeyCode::Down => self.select_next(),
                KeyCode::Char('k') | KeyCode::Up => self.select_previous(),
                KeyCode::Char('n') => self.forward_status(),
                _ => {}
            },
            Mode::Editing => match key_code {
                KeyCode::Enter => {
                    if !self.input_buffer.is_empty() {
                        let task = Task::new(self.input_buffer.clone());
                        let _ = task.save();
                        self.tasks.push(task);
                        if self.selected_index.is_none() {
                            self.selected_index = Some(0);
                        }
                    }
                    self.input_buffer.clear();
                    self.input_mode = Mode::Normal;
                }
                KeyCode::Esc => {
                    self.input_buffer.clear();
                    self.input_mode = Mode::Normal;
                }
                KeyCode::Backspace => {
                    self.input_buffer.pop();
                }
                KeyCode::Char(c) => {
                    self.input_buffer.push(c);
                }
                _ => {}
            },
        }
    }

    fn select_next(&mut self) {
        if self.tasks.is_empty() {
            return;
        }
        self.selected_index = Some(match self.selected_index {
            Some(i) => (i + 1).min(self.tasks.len() - 1),
            None => 0,
        });
    }

    fn select_previous(&mut self) {
        if self.tasks.is_empty() {
            return;
        }
        self.selected_index = Some(match self.selected_index {
            Some(i) => i.saturating_sub(1),
            None => 0,
        });
    }

    fn forward_status(&mut self) {
        if let Some(index) = self.selected_index {
            let next_status = match self.tasks[index].status {
                TaskStatus::Todo => TaskStatus::Doing,
                TaskStatus::Doing => TaskStatus::Done,
                TaskStatus::Done => return,
            };
            self.tasks[index].update_status(next_status);
        }
    }
}

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;

    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let mut app = App::new();

    while !app.should_quit {
        terminal.draw(|frame| render::render(frame, &app))?;

        if event::poll(std::time::Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            app.handle_key_event(key.code);
        }
    }

    disable_raw_mode()?;
    io::stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}
