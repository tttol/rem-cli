use crossterm::event::KeyCode;
use std::path::PathBuf;

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
    pub input_cursor: usize,
    pub tasks: Vec<Task>,
    pub selected_index: Option<usize>,
    pub parking_loaded: bool,
    pub done_loaded: bool,
    pub open_file: Option<PathBuf>,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    /// Creates a new `App` instance.
    ///
    /// Loads TODO and DOING tasks from the filesystem.
    /// PARKING and DONE tasks are not loaded at startup.
    pub fn new() -> Self {
        let mut tasks = Task::load_todo().unwrap_or_default();
        tasks.extend(Task::load_doing().unwrap_or_default());
        let tasks = Task::sort(tasks);
        let selected_index = if tasks.is_empty() { None } else { Some(0) };
        Self {
            should_quit: false,
            input_mode: Mode::Normal,
            input_buffer: String::new(),
            input_cursor: 0,
            tasks,
            selected_index,
            parking_loaded: false,
            done_loaded: false,
            open_file: None,
        }
    }

    /// Loads PARKING tasks once after the first frame has been rendered.
    pub fn load_parking_after_first_render(&mut self) {
        if self.parking_loaded {
            return;
        }
        let selected_id = self
            .selected_index
            .and_then(|index| self.tasks.get(index))
            .map(|task| task.id);
        if let Ok(parking_tasks) = Task::load_parking() {
            self.tasks.extend(parking_tasks);
            self.tasks = Task::sort(self.tasks.clone());
        }
        self.parking_loaded = true;
        self.selected_index = selected_id
            .and_then(|id| self.tasks.iter().position(|task| task.id == id))
            .or_else(|| (!self.tasks.is_empty()).then_some(0));
    }

    /// Dispatches a key event to the appropriate handler based on the current input mode.
    pub fn handle_key_event(&mut self, key_code: KeyCode) {
        match self.input_mode {
            Mode::Normal => match key_code {
                KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
                KeyCode::Char('a') => {
                    self.input_mode = Mode::Editing;
                    self.input_buffer.clear();
                    self.input_cursor = 0;
                }
                KeyCode::Char('j') | KeyCode::Down => self.select_next(),
                KeyCode::Char('k') | KeyCode::Up => self.select_previous(),
                KeyCode::Char('h') | KeyCode::Left => self.select_left(),
                KeyCode::Char('l') | KeyCode::Right => self.select_right(),
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
                    self.input_cursor = 0;
                    self.input_mode = Mode::Normal;
                }
                KeyCode::Left => self.input_cursor = self.input_cursor.saturating_sub(1),
                KeyCode::Right => {
                    self.input_cursor =
                        (self.input_cursor + 1).min(self.input_buffer.chars().count());
                }
                KeyCode::Backspace => self.delete_character_before_cursor(),
                KeyCode::Char(c) => self.insert_character_at_cursor(c),
                _ => {}
            },
        }
    }

    fn insert_character_at_cursor(&mut self, character: char) {
        let byte_index = self
            .input_buffer
            .char_indices()
            .nth(self.input_cursor)
            .map_or(self.input_buffer.len(), |(index, _)| index);
        self.input_buffer.insert(byte_index, character);
        self.input_cursor += 1;
    }

    fn delete_character_before_cursor(&mut self) {
        if self.input_cursor == 0 {
            return;
        }
        let start = self
            .input_buffer
            .char_indices()
            .nth(self.input_cursor - 1)
            .map_or(0, |(index, _)| index);
        let end = self
            .input_buffer
            .char_indices()
            .nth(self.input_cursor)
            .map_or(self.input_buffer.len(), |(index, _)| index);
        self.input_buffer.replace_range(start..end, "");
        self.input_cursor -= 1;
    }

    /// Moves the cursor to the next task in the current status column.
    fn select_next(&mut self) {
        let Some(index) = self.selected_index else {
            return;
        };
        let status = self.tasks[index].status;
        let status_indices = self.indices_for_status(status);
        let row = status_indices
            .iter()
            .position(|candidate| *candidate == index)
            .unwrap_or(0);
        self.selected_index = status_indices
            .get((row + 1).min(status_indices.len() - 1))
            .copied();
    }

    /// Moves the cursor to the previous task in the current status column.
    fn select_previous(&mut self) {
        let Some(index) = self.selected_index else {
            return;
        };
        let status = self.tasks[index].status;
        let status_indices = self.indices_for_status(status);
        let row = status_indices
            .iter()
            .position(|candidate| *candidate == index)
            .unwrap_or(0);
        self.selected_index = status_indices.get(row.saturating_sub(1)).copied();
    }

    fn select_left(&mut self) {
        self.select_horizontal(-1);
    }

    fn select_right(&mut self) {
        self.select_horizontal(1);
    }

    fn select_horizontal(&mut self, direction: isize) {
        let Some(index) = self.selected_index else {
            return;
        };
        let current_status = self.tasks[index].status;
        let current_indices = self.indices_for_status(current_status);
        let row = current_indices
            .iter()
            .position(|candidate| *candidate == index)
            .unwrap_or(0);
        let statuses = self.visible_statuses();
        let Some(column) = statuses.iter().position(|status| *status == current_status) else {
            return;
        };
        let candidates: Box<dyn Iterator<Item = usize>> = if direction < 0 {
            Box::new((0..column).rev())
        } else {
            Box::new((column + 1)..statuses.len())
        };
        self.selected_index = candidates
            .map(|candidate| self.indices_for_status(statuses[candidate]))
            .find(|indices| !indices.is_empty())
            .and_then(|indices| indices.get(row.min(indices.len() - 1)).copied())
            .or(self.selected_index);
    }

    fn visible_statuses(&self) -> Vec<TaskStatus> {
        let statuses = [
            TaskStatus::Parking,
            TaskStatus::Todo,
            TaskStatus::Doing,
            TaskStatus::Done,
        ];
        statuses
            .into_iter()
            .filter(|status| *status != TaskStatus::Done || self.done_loaded)
            .collect()
    }

    fn indices_for_status(&self, status: TaskStatus) -> Vec<usize> {
        self.tasks
            .iter()
            .enumerate()
            .filter_map(|(index, task)| (task.status == status).then_some(index))
            .collect()
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
        if let Some(index) = self.selected_index
            && let Ok(reloaded) = self.tasks[index].reload()
        {
            self.tasks[index] = reloaded;
        }
    }

    /// Handles post-edit cleanup after returning from neovim.
    pub fn after_edit(&mut self) {
        self.reload_selected_task();
    }

    /// Advances the selected task's status: PARKING -> TODO -> DOING -> DONE.
    ///
    /// Does nothing if the task is already DONE.
    fn forward_status(&mut self) {
        if let Some(index) = self.selected_index {
            let next_status = match self.tasks[index].status {
                TaskStatus::Parking => TaskStatus::Todo,
                TaskStatus::Todo => TaskStatus::Doing,
                TaskStatus::Doing => TaskStatus::Done,
                TaskStatus::Done => return,
            };
            self.change_status(index, next_status);
        }
    }

    /// Reverts the selected task's status: DONE -> DOING -> TODO -> PARKING.
    ///
    /// Does nothing if the task is already PARKING.
    fn backward_status(&mut self) {
        if let Some(index) = self.selected_index {
            let next_status = match self.tasks[index].status {
                TaskStatus::Parking => return,
                TaskStatus::Todo => TaskStatus::Parking,
                TaskStatus::Doing => TaskStatus::Todo,
                TaskStatus::Done => TaskStatus::Doing,
            };
            self.change_status(index, next_status);
        }
    }

    fn change_status(&mut self, index: usize, next_status: TaskStatus) {
        let id = self.tasks[index].id;
        let previous_status = self.tasks[index].status;
        let previous_indices = self.indices_for_status(previous_status);
        let previous_row = previous_indices
            .iter()
            .position(|candidate| *candidate == index)
            .unwrap_or(0);
        self.tasks[index].update_status(next_status);
        if next_status == TaskStatus::Done && !self.done_loaded {
            self.tasks.retain(|task| task.id != id);
            self.tasks = Task::sort(self.tasks.clone());
            self.selected_index = self.nearby_selection(previous_status, previous_row);
            return;
        }
        self.tasks = Task::sort(self.tasks.clone());
        self.selected_index = self.tasks.iter().position(|task| task.id == id);
    }

    fn nearby_selection(&self, preferred_status: TaskStatus, row: usize) -> Option<usize> {
        let preferred = self.indices_for_status(preferred_status);
        if !preferred.is_empty() {
            return preferred.get(row.min(preferred.len() - 1)).copied();
        }
        let statuses = self.visible_statuses();
        let preferred_column = statuses
            .iter()
            .position(|status| *status == preferred_status)?;
        (1..statuses.len())
            .flat_map(|distance| {
                [
                    preferred_column.checked_sub(distance),
                    preferred_column
                        .checked_add(distance)
                        .filter(|column| *column < statuses.len()),
                ]
            })
            .flatten()
            .map(|column| self.indices_for_status(statuses[column]))
            .find(|indices| !indices.is_empty())
            .and_then(|indices| indices.get(row.min(indices.len() - 1)).copied())
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
        self.input_cursor = 0;
        self.input_mode = Mode::Normal;
    }

    /// Toggles the visibility of DONE tasks.
    ///
    /// When enabled, loads DONE tasks from the filesystem and appends them to the task list.
    /// When disabled, removes all DONE tasks from the in-memory list.
    fn toggle_done(&mut self) {
        if self.done_loaded {
            let selection = self.selected_index.and_then(|index| {
                let status = self.tasks[index].status;
                let row = self
                    .indices_for_status(status)
                    .iter()
                    .position(|candidate| *candidate == index)?;
                Some((status, row))
            });
            self.tasks.retain(|t| t.status != TaskStatus::Done);
            self.done_loaded = false;
            if selection.is_some_and(|(status, _)| status == TaskStatus::Done) {
                self.selected_index =
                    selection.and_then(|(_, row)| self.nearby_selection(TaskStatus::Doing, row));
            }
        } else {
            if let Ok(done_tasks) = Task::load_done() {
                self.tasks.extend(done_tasks);
                self.tasks = Task::sort(self.tasks.clone());
            }
            self.done_loaded = true;
        }
        if self.tasks.is_empty() {
            self.selected_index = None;
        } else if let Some(i) = self.selected_index
            && i >= self.tasks.len()
        {
            self.selected_index = Some(self.tasks.len() - 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_app(tasks: Vec<Task>, selected_index: Option<usize>) -> App {
        App {
            should_quit: false,
            input_mode: Mode::Normal,
            input_buffer: String::new(),
            input_cursor: 0,
            tasks: Task::sort(tasks),
            selected_index,
            parking_loaded: true,
            done_loaded: false,
            open_file: None,
        }
    }

    fn create_task(name: &str, status: TaskStatus) -> Task {
        let mut task = Task::new(name.to_string());
        task.status = status;
        task
    }

    #[test]
    fn parking_is_not_loaded_during_initialization() {
        // GIVEN
        let app = App::new();

        // WHEN
        let parking_tasks = app
            .tasks
            .iter()
            .filter(|task| task.status == TaskStatus::Parking)
            .count();

        // THEN
        assert!(!app.parking_loaded);
        assert_eq!(parking_tasks, 0);
    }

    #[test]
    fn parking_is_loaded_after_first_render() {
        // GIVEN
        let mut app = App::new();

        // WHEN
        app.load_parking_after_first_render();

        // THEN
        assert!(app.parking_loaded);
    }

    #[test]
    fn vertical_navigation_stays_within_current_status() {
        // GIVEN
        let tasks = vec![
            create_task("todo one", TaskStatus::Todo),
            create_task("todo two", TaskStatus::Todo),
            create_task("doing", TaskStatus::Doing),
        ];
        let mut app = create_app(tasks, Some(1));

        // WHEN
        app.handle_key_event(KeyCode::Char('j'));

        // THEN
        assert_eq!(app.selected_index, Some(1));
        assert_eq!(app.tasks[1].status, TaskStatus::Todo);
    }

    #[test]
    fn horizontal_navigation_skips_empty_columns_and_clamps_row() {
        // GIVEN
        let tasks = vec![
            create_task("parking one", TaskStatus::Parking),
            create_task("parking two", TaskStatus::Parking),
            create_task("doing", TaskStatus::Doing),
        ];
        let mut app = create_app(tasks, Some(1));
        let expected = app
            .tasks
            .iter()
            .position(|task| task.status == TaskStatus::Doing);

        // WHEN
        app.handle_key_event(KeyCode::Char('l'));

        // THEN
        assert_eq!(app.selected_index, expected);
    }

    #[test]
    fn hiding_done_selects_nearby_visible_task() {
        // GIVEN
        let tasks = vec![
            create_task("doing", TaskStatus::Doing),
            create_task("done", TaskStatus::Done),
        ];
        let mut app = create_app(tasks, Some(1));
        app.done_loaded = true;

        // WHEN
        app.handle_key_event(KeyCode::Char('d'));

        // THEN
        assert!(!app.done_loaded);
        assert_eq!(app.tasks.len(), 1);
        assert_eq!(app.selected_index, Some(0));
        assert_eq!(app.tasks[0].status, TaskStatus::Doing);
    }

    #[test]
    fn editing_cursor_moves_and_inserts_at_selected_position() {
        // GIVEN
        let mut app = create_app(Vec::new(), None);
        app.handle_key_event(KeyCode::Char('a'));
        for character in "ac".chars() {
            app.handle_key_event(KeyCode::Char(character));
        }

        // WHEN
        app.handle_key_event(KeyCode::Left);
        app.handle_key_event(KeyCode::Char('b'));

        // THEN
        assert_eq!(app.input_buffer, "abc");
        assert_eq!(app.input_cursor, 2);
    }

    #[test]
    fn backspace_deletes_multibyte_character_before_cursor() {
        // GIVEN
        let mut app = create_app(Vec::new(), None);
        app.handle_key_event(KeyCode::Char('a'));
        for character in "あいう".chars() {
            app.handle_key_event(KeyCode::Char(character));
        }
        app.handle_key_event(KeyCode::Left);

        // WHEN
        app.handle_key_event(KeyCode::Backspace);

        // THEN
        assert_eq!(app.input_buffer, "あう");
        assert_eq!(app.input_cursor, 1);
    }
}
