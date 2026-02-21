use std::fs;
use std::path::PathBuf;
use crossterm::event::KeyCode;

use crate::task::{Task, TaskStatus};

#[derive(PartialEq)]
pub enum Mode {
    Normal,
    Editing,
}

/// Application state and core logic for the TUI.
pub struct App {
    pub should_quit: bool,
    pub input_mode: Mode,
    pub input_buffer: String,
    pub tasks: Vec<Task>,
    pub selected_index: Option<usize>,
    pub done_loaded: bool,
    pub preview_content: String,
    pub open_file: Option<PathBuf>,
}

impl App {
    /// Creates a new `App` instance.
    ///
    /// Loads TODO and DOING tasks from the filesystem.
    /// DONE tasks are not loaded at startup (lazy-loaded via `toggle_done`).
    pub fn new() -> Self {
        let mut tasks = Task::load_todo().unwrap_or_default();
        tasks.extend(Task::load_doing().unwrap_or_default());
        let selected_index = if tasks.is_empty() { None } else { Some(0) };
        let preview_content = match selected_index {
            Some(i) => fs::read_to_string(tasks[i].file_path()).unwrap_or_default(),
            None => String::new(),
        };
        Self {
            should_quit: false,
            input_mode: Mode::Normal,
            input_buffer: String::new(),
            tasks,
            selected_index,
            done_loaded: false,
            preview_content,
            open_file: None,
        }
    }

    /// Dispatches a key event to the appropriate handler based on the current input mode.
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
                KeyCode::Char('N') => self.backward_status(),
                KeyCode::Char('d') => self.toggle_done(),
                KeyCode::Enter => self.open_task(),
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

    /// Moves the cursor to the next task in the list.
    fn select_next(&mut self) {
        if self.tasks.is_empty() {
            return;
        }
        self.selected_index = Some(match self.selected_index {
            Some(i) => (i + 1).min(self.tasks.len() - 1),
            None => 0,
        });
        self.update_preview();
    }

    /// Moves the cursor to the previous task in the list.
    fn select_previous(&mut self) {
        if self.tasks.is_empty() {
            return;
        }
        self.selected_index = Some(match self.selected_index {
            Some(i) => i.saturating_sub(1),
            None => 0,
        });
        self.update_preview();
    }

    /// Sets the selected task's file path to `open_file` for neovim to open.
    ///
    /// The actual neovim invocation is handled in the main event loop (`main.rs`),
    /// since terminal control must be managed there.
    fn open_task(&mut self) {
        if let Some(index) = self.selected_index {
            self.open_file = Some(self.tasks[index].file_path());
        }
    }

    /// Reloads the selected task's metadata from its markdown file to reflect the latest state in memory.
    fn reload_selected_task(&mut self) {
        if let Some(index) = self.selected_index {
            if let Ok(reloaded) = self.tasks[index].reload() {
                self.tasks[index] = reloaded;
            }
        }
    }

    /// Handles post-edit cleanup after returning from neovim: reloads the task and refreshes the preview.
    pub fn after_edit(&mut self) {
        self.reload_selected_task();
        self.update_preview();
    }

    /// Reads the selected task's markdown file and updates the preview content.
    pub fn update_preview(&mut self) {
        self.preview_content = match self.selected_index {
            Some(index) => fs::read_to_string(self.tasks[index].file_path()).unwrap_or_default(),
            None => String::new(),
        };
    }

    /// Advances the selected task's status: TODO -> DOING -> DONE.
    ///
    /// Does nothing if the task is already DONE.
    fn forward_status(&mut self) {
        if let Some(index) = self.selected_index {
            let next_status = match self.tasks[index].status {
                TaskStatus::Todo => TaskStatus::Doing,
                TaskStatus::Doing => TaskStatus::Done,
                TaskStatus::Done => return,
            };
            self.tasks[index].update_status(next_status);
            self.tasks = Task::sort(self.tasks.clone());
            self.update_preview();
        }
    }

    /// Reverts the selected task's status: DONE -> DOING -> TODO.
    ///
    /// Does nothing if the task is already TODO.
    fn backward_status(&mut self) {
        if let Some(index) = self.selected_index {
            let next_status = match self.tasks[index].status {
                TaskStatus::Todo => return,
                TaskStatus::Doing => TaskStatus::Todo,
                TaskStatus::Done => TaskStatus::Doing,
            };
            self.tasks[index].update_status(next_status);
            self.tasks = Task::sort(self.tasks.clone());
            self.update_preview();
        }
    }

    /// Creates a new task from the input buffer and saves it to the filesystem.
    ///
    /// Clears the input buffer and returns to Normal mode after completion.
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
        self.update_preview();
    }

    /// Toggles the visibility of DONE tasks.
    ///
    /// When enabled, loads DONE tasks from the filesystem and appends them to the task list.
    /// When disabled, removes all DONE tasks from the in-memory list.
    fn toggle_done(&mut self) {
        if self.done_loaded {
            self.tasks.retain(|t| t.status != TaskStatus::Done);
            self.done_loaded = false;
        } else {
            if let Ok(done_tasks) = Task::load_done() {
                self.tasks.extend(done_tasks); 
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
        self.update_preview();
    }
}
