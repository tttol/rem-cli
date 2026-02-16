use crossterm::event::KeyCode;

use crate::task::{Task, TaskStatus};

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
    pub done_loaded: bool,
}

impl App {
    pub fn new() -> Self {
        let mut tasks = Task::load_todo().unwrap_or_default();
        tasks.extend(Task::load_doing().unwrap_or_default());
        let selected_index = if tasks.is_empty() { None } else { Some(0) };
        Self {
            should_quit: false,
            input_mode: Mode::Normal,
            input_buffer: String::new(),
            tasks,
            selected_index,
            done_loaded: false,
        }
    }

    pub fn handle_key_event(&mut self, key_code: KeyCode) {
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
                KeyCode::Char('d') => self.toggle_done(),
                _ => {}
            },
            Mode::Editing => match key_code {
                KeyCode::Enter => {
                    self.add_task();
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
            self.tasks = Task::sort(self.tasks.clone());
        }
    }

    fn add_task(&mut self) {
        if !self.input_buffer.is_empty() {
            let new_task = Task::new(self.input_buffer.clone());
            let _ = new_task.save();
            self.tasks.push(new_task);
            self.tasks = Task::sort(self.tasks.clone());
            if self.selected_index.is_none() {
                self.selected_index = Some(0);
            }
        }
        self.input_buffer.clear();
        self.input_mode = Mode::Normal;

    }

    fn toggle_done(&mut self) {
        if self.done_loaded {
            self.tasks.retain(|t| t.status != TaskStatus::Done);
            self.done_loaded = false;
        } else {
            if let Ok(done_tasks) = Task::load_done() {
                self.tasks.extend(done_tasks); 
                // self.tasks.sort_by(|a, b| a.created_at.cmp(&b.created_at));
            }
            self.done_loaded = true;
        }
        if self.tasks.is_empty() {
            self.selected_index = None;
        } else if let Some(i) = self.selected_index {
            if i >= self.tasks.len() {
                self.selected_index = Some(self.tasks.len() - 1);
            }
        }
    }
}
