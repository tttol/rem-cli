use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::task::{Task, TaskStatus};
use crate::voice::{VoiceCommand, VoiceEvent};

const VOICE_LONG_PRESS_DURATION: Duration = Duration::from_millis(400);
const VOICE_FALLBACK_TAP_DURATION: Duration = Duration::from_millis(700);
const VOICE_RECORDING_LIMIT: Duration = Duration::from_secs(30);

#[derive(Debug, PartialEq)]
pub enum Mode {
    Normal,
    Editing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoiceState {
    Idle,
    Authorizing,
    Recording,
    Recognizing,
    Failed(String),
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
    pub error_message: Option<String>,
    pub voice_state: VoiceState,
    pub voice_partial: String,
    pub(crate) tasks_dir: PathBuf,
    pub(crate) persistent_error: Option<String>,
    pub(crate) voice_enabled: bool,
    pub(crate) keyboard_release_supported: bool,
    pub(crate) voice_key_pressed_at: Option<Instant>,
    pub(crate) voice_recording_started_at: Option<Instant>,
    pub(crate) pending_voice_command: Option<VoiceCommand>,
    pub(crate) voice_stop_requested: bool,
    pub(crate) voice_last_key_event_at: Option<Instant>,
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
        Self::with_tasks_dir(Task::default_base_dir())
    }

    /// Creates an `App` using the provided task storage directory.
    pub fn with_tasks_dir(tasks_dir: PathBuf) -> Self {
        let todo_result = Task::load_todo_from(&tasks_dir);
        let doing_result = Task::load_doing_from(&tasks_dir);
        let error_message = todo_result
            .as_ref()
            .err()
            .or_else(|| doing_result.as_ref().err())
            .map(|error| format!("Failed to load tasks: {error}"));
        let mut tasks = todo_result.unwrap_or_default();
        tasks.extend(doing_result.unwrap_or_default());
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
            error_message: error_message.clone(),
            voice_state: VoiceState::Idle,
            voice_partial: String::new(),
            tasks_dir,
            persistent_error: error_message,
            voice_enabled: false,
            keyboard_release_supported: false,
            voice_key_pressed_at: None,
            voice_recording_started_at: None,
            pending_voice_command: None,
            voice_stop_requested: false,
            voice_last_key_event_at: None,
        }
    }

    /// Configures whether voice input and key-release events are available.
    pub fn configure_voice_input(&mut self, voice_enabled: bool, keyboard_release_supported: bool) {
        self.voice_enabled = voice_enabled;
        self.keyboard_release_supported = keyboard_release_supported;
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
        let parking_tasks = match Task::load_parking_from(&self.tasks_dir) {
            Ok(tasks) => tasks,
            Err(error) => {
                self.error_message = Some(
                    self.error_with_persistent(format!("Failed to load PARKING tasks: {error}")),
                );
                return;
            }
        };
        self.tasks.extend(parking_tasks);
        self.tasks = Task::sort(self.tasks.clone());
        self.parking_loaded = true;
        self.error_message = self.persistent_error.clone();
        self.selected_index = selected_id
            .and_then(|id| self.tasks.iter().position(|task| task.id == id))
            .or_else(|| (!self.tasks.is_empty()).then_some(0));
    }

    /// Dispatches a key event to the appropriate handler based on the current input mode.
    pub fn handle_key_event(&mut self, key_event: impl Into<KeyEvent>) {
        self.handle_key_event_at(key_event.into(), Instant::now());
    }

    fn handle_key_event_at(&mut self, key_event: KeyEvent, now: Instant) {
        if self.input_mode == Mode::Editing
            && self.voice_enabled
            && !self.keyboard_release_supported
            && self.voice_key_pressed_at.is_some()
            && key_event.code != KeyCode::Char('v')
            && key_event.kind != KeyEventKind::Release
        {
            self.voice_key_pressed_at = None;
            self.insert_character_at_cursor('v');
        }
        match self.input_mode {
            Mode::Normal if key_event.kind != KeyEventKind::Release => match key_event.code {
                KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
                KeyCode::Char('a') => {
                    self.input_mode = Mode::Editing;
                    self.input_buffer.clear();
                    self.input_cursor = 0;
                    self.voice_state = VoiceState::Idle;
                    self.voice_partial.clear();
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
            Mode::Normal => {}
            Mode::Editing if key_event.code == KeyCode::Char('v') && self.voice_enabled => {
                self.handle_voice_key(key_event.kind, now);
            }
            Mode::Editing if key_event.kind == KeyEventKind::Release => {}
            Mode::Editing => match key_event.code {
                KeyCode::Enter => {
                    if !self.voice_is_active() {
                        self.add_task();
                    }
                }
                KeyCode::Esc => {
                    if self.voice_is_active() {
                        self.cancel_voice_input();
                    } else {
                        self.input_buffer.clear();
                        self.input_cursor = 0;
                        self.input_mode = Mode::Normal;
                        self.voice_state = VoiceState::Idle;
                        self.voice_partial.clear();
                    }
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

    fn handle_voice_key(&mut self, kind: KeyEventKind, now: Instant) {
        let previous_key_event = self.voice_last_key_event_at.replace(now);
        if matches!(self.voice_state, VoiceState::Failed(_)) {
            self.voice_state = VoiceState::Idle;
        }
        if self.voice_state == VoiceState::Recording && kind == KeyEventKind::Press {
            if !self.keyboard_release_supported
                && previous_key_event.is_some_and(|previous| {
                    now.duration_since(previous) < Duration::from_millis(250)
                })
            {
                return;
            }
            self.stop_voice_input();
            return;
        }
        if self.voice_is_active() && kind != KeyEventKind::Release {
            return;
        }
        match kind {
            KeyEventKind::Press if self.voice_key_pressed_at.is_none() => {
                self.voice_key_pressed_at = Some(now);
            }
            KeyEventKind::Press | KeyEventKind::Repeat => {
                let elapsed = self
                    .voice_key_pressed_at
                    .map(|started| now.duration_since(started));
                if !self.keyboard_release_supported
                    && kind == KeyEventKind::Press
                    && elapsed.is_some_and(|duration| duration < VOICE_LONG_PRESS_DURATION)
                {
                    self.voice_key_pressed_at = Some(now);
                    self.insert_character_at_cursor('v');
                } else if elapsed.is_some_and(|duration| duration >= VOICE_LONG_PRESS_DURATION) {
                    self.start_voice_input();
                }
            }
            KeyEventKind::Release => {
                if self.voice_state == VoiceState::Recording {
                    self.stop_voice_input();
                    return;
                }
                if self.voice_state == VoiceState::Authorizing {
                    self.voice_key_pressed_at = None;
                    self.voice_stop_requested = true;
                    return;
                }
                if self.voice_state == VoiceState::Recognizing {
                    self.voice_key_pressed_at = None;
                    return;
                }
                let is_long_press = self.voice_key_pressed_at.is_some_and(|started| {
                    now.duration_since(started) >= VOICE_LONG_PRESS_DURATION
                });
                self.voice_key_pressed_at = None;
                if is_long_press {
                    if self.voice_state == VoiceState::Recording {
                        self.stop_voice_input();
                    } else if self.voice_state == VoiceState::Idle {
                        self.start_voice_input();
                    }
                } else {
                    self.insert_character_at_cursor('v');
                }
            }
        }
    }

    /// Advances time-based voice input behavior.
    pub fn tick(&mut self, now: Instant) {
        if self.keyboard_release_supported
            && self.voice_state == VoiceState::Idle
            && self
                .voice_key_pressed_at
                .is_some_and(|started| now.duration_since(started) >= VOICE_LONG_PRESS_DURATION)
        {
            self.start_voice_input();
        } else if !self.keyboard_release_supported
            && self.voice_state == VoiceState::Idle
            && self
                .voice_key_pressed_at
                .is_some_and(|started| now.duration_since(started) >= VOICE_FALLBACK_TAP_DURATION)
        {
            self.voice_key_pressed_at = None;
            self.insert_character_at_cursor('v');
        }
        if self.voice_state == VoiceState::Recording
            && self
                .voice_recording_started_at
                .is_some_and(|started| now.duration_since(started) >= VOICE_RECORDING_LIMIT)
        {
            self.stop_voice_input();
        }
    }

    fn start_voice_input(&mut self) {
        if !self.keyboard_release_supported {
            self.voice_key_pressed_at = None;
        }
        self.voice_partial.clear();
        self.voice_stop_requested = false;
        self.voice_state = VoiceState::Authorizing;
        self.pending_voice_command = Some(VoiceCommand::Start);
    }

    fn stop_voice_input(&mut self) {
        self.voice_key_pressed_at = None;
        self.voice_recording_started_at = None;
        self.voice_state = VoiceState::Recognizing;
        self.pending_voice_command = Some(VoiceCommand::Stop);
    }

    fn cancel_voice_input(&mut self) {
        self.voice_key_pressed_at = None;
        self.voice_recording_started_at = None;
        self.voice_partial.clear();
        self.voice_stop_requested = false;
        self.voice_last_key_event_at = None;
        self.voice_state = VoiceState::Idle;
        self.pending_voice_command = Some(VoiceCommand::Cancel);
    }

    fn voice_is_active(&self) -> bool {
        matches!(
            self.voice_state,
            VoiceState::Authorizing | VoiceState::Recording | VoiceState::Recognizing
        )
    }

    /// Returns the next command for the platform speech service.
    pub fn take_voice_command(&mut self) -> Option<VoiceCommand> {
        self.pending_voice_command.take()
    }

    /// Applies an asynchronous event received from the platform speech service.
    pub fn handle_voice_event(&mut self, event: VoiceEvent) {
        match event {
            VoiceEvent::Authorizing => self.voice_state = VoiceState::Authorizing,
            VoiceEvent::Recording => {
                if self.voice_stop_requested {
                    self.stop_voice_input();
                } else {
                    self.voice_state = VoiceState::Recording;
                    self.voice_recording_started_at = Some(Instant::now());
                }
            }
            VoiceEvent::Recognizing => {
                self.voice_state = VoiceState::Recognizing;
                self.voice_recording_started_at = None;
            }
            VoiceEvent::Partial(text) => self.voice_partial = text,
            VoiceEvent::Final(text) => {
                let transcript = text.trim();
                if transcript.is_empty() {
                    self.voice_state = VoiceState::Failed("No speech was recognized".to_string());
                } else {
                    self.insert_text_at_cursor(transcript);
                    self.voice_state = VoiceState::Idle;
                }
                self.voice_partial.clear();
                self.voice_recording_started_at = None;
                self.voice_stop_requested = false;
            }
            VoiceEvent::PermissionDenied(message)
            | VoiceEvent::Unavailable(message)
            | VoiceEvent::Error(message) => {
                self.voice_state = VoiceState::Failed(message);
                self.voice_partial.clear();
                self.voice_recording_started_at = None;
                self.voice_stop_requested = false;
            }
        }
    }

    fn insert_character_at_cursor(&mut self, character: char) {
        if matches!(self.voice_state, VoiceState::Failed(_)) {
            self.voice_state = VoiceState::Idle;
        }
        let byte_index = self
            .input_buffer
            .char_indices()
            .nth(self.input_cursor)
            .map_or(self.input_buffer.len(), |(index, _)| index);
        self.input_buffer.insert(byte_index, character);
        self.input_cursor += 1;
    }

    fn insert_text_at_cursor(&mut self, text: &str) {
        let byte_index = self
            .input_buffer
            .char_indices()
            .nth(self.input_cursor)
            .map_or(self.input_buffer.len(), |(index, _)| index);
        self.input_buffer.insert_str(byte_index, text);
        self.input_cursor += text.chars().count();
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
        if let Err(error) = self.tasks[index].update_status(next_status) {
            self.error_message =
                Some(self.error_with_persistent(format!("Failed to update task status: {error}")));
            return;
        }
        self.error_message = self.persistent_error.clone();
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
            let new_task = Task::new_in(self.input_buffer.clone(), self.tasks_dir.clone());
            if let Err(error) = new_task.save() {
                self.error_message =
                    Some(self.error_with_persistent(format!("Failed to add task: {error}")));
                return;
            }
            self.tasks.push(new_task);
            self.tasks = Task::sort(self.tasks.clone());
            if self.selected_index.is_none() {
                self.selected_index = Some(0);
            }
        }
        self.input_buffer.clear();
        self.input_cursor = 0;
        self.input_mode = Mode::Normal;
        self.voice_state = VoiceState::Idle;
        self.voice_partial.clear();
        self.error_message = self.persistent_error.clone();
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
            let done_tasks = match Task::load_done_from(&self.tasks_dir) {
                Ok(tasks) => tasks,
                Err(error) => {
                    self.error_message = Some(
                        self.error_with_persistent(format!("Failed to load DONE tasks: {error}")),
                    );
                    return;
                }
            };
            self.tasks.extend(done_tasks);
            self.tasks = Task::sort(self.tasks.clone());
            self.done_loaded = true;
            self.error_message = self.persistent_error.clone();
        }
        if self.tasks.is_empty() {
            self.selected_index = None;
        } else if let Some(i) = self.selected_index
            && i >= self.tasks.len()
        {
            self.selected_index = Some(self.tasks.len() - 1);
        }
    }

    fn error_with_persistent(&self, error: String) -> String {
        self.persistent_error
            .as_deref()
            .map_or(error.clone(), |persistent| {
                format!("{persistent} | {error}")
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;
    use std::fs;
    use uuid::Uuid;

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
            error_message: None,
            voice_state: VoiceState::Idle,
            voice_partial: String::new(),
            tasks_dir: Task::default_base_dir(),
            persistent_error: None,
            voice_enabled: false,
            keyboard_release_supported: false,
            voice_key_pressed_at: None,
            voice_recording_started_at: None,
            pending_voice_command: None,
            voice_stop_requested: false,
            voice_last_key_event_at: None,
        }
    }

    fn create_task(name: &str, status: TaskStatus) -> Task {
        let mut task = Task::new(name.to_string());
        task.status = status;
        task
    }

    fn temporary_tasks_dir() -> PathBuf {
        std::env::temp_dir().join(format!("rem-cli-app-test-{}", Uuid::new_v4()))
    }

    fn voice_key(kind: KeyEventKind) -> KeyEvent {
        KeyEvent::new_with_kind(KeyCode::Char('v'), KeyModifiers::NONE, kind)
    }

    #[test]
    fn parking_is_not_loaded_during_initialization() {
        // GIVEN
        let tasks_dir = temporary_tasks_dir();
        let app = App::with_tasks_dir(tasks_dir.clone());

        // WHEN
        let parking_tasks = app
            .tasks
            .iter()
            .filter(|task| task.status == TaskStatus::Parking)
            .count();

        // THEN
        assert!(!app.parking_loaded);
        assert_eq!(parking_tasks, 0);
        assert!(!tasks_dir.exists());
    }

    #[test]
    fn parking_is_loaded_after_first_render() {
        // GIVEN
        let tasks_dir = temporary_tasks_dir();
        let mut app = App::with_tasks_dir(tasks_dir.clone());

        // WHEN
        app.load_parking_after_first_render();

        // THEN
        assert!(app.parking_loaded);
        assert!(!tasks_dir.exists());
    }

    #[test]
    fn parking_load_failure_is_retried() {
        // GIVEN
        let tasks_dir = temporary_tasks_dir();
        fs::create_dir_all(&tasks_dir).unwrap();
        fs::write(tasks_dir.join("parking"), "not a directory").unwrap();
        let mut app = App::with_tasks_dir(tasks_dir.clone());

        // WHEN
        app.load_parking_after_first_render();

        // THEN
        assert!(!app.parking_loaded);
        assert!(app.error_message.is_some());

        fs::remove_file(tasks_dir.join("parking")).unwrap();
        app.load_parking_after_first_render();
        assert!(app.parking_loaded);
        assert!(app.error_message.is_none());
        fs::remove_dir_all(tasks_dir).unwrap();
    }

    #[test]
    fn parking_load_success_keeps_initial_load_error() {
        // GIVEN
        let tasks_dir = temporary_tasks_dir();
        fs::create_dir_all(&tasks_dir).unwrap();
        fs::write(tasks_dir.join("todo"), "not a directory").unwrap();
        let mut app = App::with_tasks_dir(tasks_dir.clone());
        let expected = app.error_message.clone();

        // WHEN
        app.load_parking_after_first_render();

        // THEN
        assert!(app.parking_loaded);
        assert_eq!(app.error_message, expected);

        fs::remove_dir_all(tasks_dir).unwrap();
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

    #[test]
    fn short_voice_key_press_inserts_v() {
        // GIVEN
        let mut app = create_app(Vec::new(), None);
        app.input_mode = Mode::Editing;
        app.configure_voice_input(true, true);
        let pressed_at = Instant::now();
        app.handle_key_event_at(voice_key(KeyEventKind::Press), pressed_at);
        let expected = "v";

        // WHEN
        app.handle_key_event_at(
            voice_key(KeyEventKind::Release),
            pressed_at + Duration::from_millis(100),
        );

        // THEN
        assert_eq!(app.input_buffer, expected);
        assert_eq!(app.voice_state, VoiceState::Idle);
    }

    #[test]
    fn long_voice_key_press_requests_recording() {
        // GIVEN
        let mut app = create_app(Vec::new(), None);
        app.input_mode = Mode::Editing;
        app.configure_voice_input(true, true);
        let pressed_at = Instant::now();
        app.handle_key_event_at(voice_key(KeyEventKind::Press), pressed_at);
        let expected = Some(VoiceCommand::Start);

        // WHEN
        app.tick(pressed_at + VOICE_LONG_PRESS_DURATION);

        // THEN
        assert_eq!(app.voice_state, VoiceState::Authorizing);
        assert_eq!(app.take_voice_command(), expected);
    }

    #[test]
    fn releasing_voice_key_stops_recording() {
        // GIVEN
        let mut app = create_app(Vec::new(), None);
        app.input_mode = Mode::Editing;
        app.configure_voice_input(true, true);
        let pressed_at = Instant::now();
        app.handle_key_event_at(voice_key(KeyEventKind::Press), pressed_at);
        app.tick(pressed_at + VOICE_LONG_PRESS_DURATION);
        app.take_voice_command();
        app.handle_voice_event(VoiceEvent::Recording);
        let expected = Some(VoiceCommand::Stop);

        // WHEN
        app.handle_key_event_at(
            voice_key(KeyEventKind::Release),
            pressed_at + Duration::from_secs(1),
        );

        // THEN
        assert_eq!(app.voice_state, VoiceState::Recognizing);
        assert_eq!(app.take_voice_command(), expected);
    }

    #[test]
    fn fallback_ignores_key_repeat_and_stops_on_next_press() {
        // GIVEN
        let mut app = create_app(Vec::new(), None);
        app.input_mode = Mode::Editing;
        app.configure_voice_input(true, false);
        let pressed_at = Instant::now();
        app.handle_key_event_at(voice_key(KeyEventKind::Press), pressed_at);
        app.handle_key_event_at(
            voice_key(KeyEventKind::Press),
            pressed_at + VOICE_LONG_PRESS_DURATION,
        );
        app.take_voice_command();
        app.handle_voice_event(VoiceEvent::Recording);
        app.handle_key_event_at(
            voice_key(KeyEventKind::Press),
            pressed_at + Duration::from_millis(450),
        );
        let expected = Some(VoiceCommand::Stop);

        // WHEN
        app.handle_key_event_at(
            voice_key(KeyEventKind::Press),
            pressed_at + Duration::from_millis(800),
        );

        // THEN
        assert_eq!(app.voice_state, VoiceState::Recognizing);
        assert_eq!(app.take_voice_command(), expected);
    }

    #[test]
    fn fallback_preserves_v_before_following_text() {
        // GIVEN
        let mut app = create_app(Vec::new(), None);
        app.input_mode = Mode::Editing;
        app.configure_voice_input(true, false);
        let pressed_at = Instant::now();
        app.handle_key_event_at(voice_key(KeyEventKind::Press), pressed_at);
        let expected = "vi";

        // WHEN
        app.handle_key_event_at(
            KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE),
            pressed_at + Duration::from_millis(100),
        );

        // THEN
        assert_eq!(app.input_buffer, expected);
    }

    #[test]
    fn fallback_preserves_quick_repeated_v_characters() {
        // GIVEN
        let mut app = create_app(Vec::new(), None);
        app.input_mode = Mode::Editing;
        app.configure_voice_input(true, false);
        let pressed_at = Instant::now();
        app.handle_key_event_at(voice_key(KeyEventKind::Press), pressed_at);
        app.handle_key_event_at(
            voice_key(KeyEventKind::Press),
            pressed_at + Duration::from_millis(100),
        );
        let expected = "vv";

        // WHEN
        app.tick(pressed_at + Duration::from_secs(1));

        // THEN
        assert_eq!(app.input_buffer, expected);
        assert_eq!(app.voice_state, VoiceState::Idle);
    }

    #[test]
    fn final_transcript_is_inserted_at_unicode_cursor() {
        // GIVEN
        let mut app = create_app(Vec::new(), None);
        app.input_mode = Mode::Editing;
        app.input_buffer = "あう".to_string();
        app.input_cursor = 1;
        app.voice_state = VoiceState::Recognizing;
        let expected = ("あ認識結果う".to_string(), 5);

        // WHEN
        app.handle_voice_event(VoiceEvent::Final(" 認識結果 ".to_string()));

        // THEN
        assert_eq!((app.input_buffer, app.input_cursor), expected);
        assert_eq!(app.voice_state, VoiceState::Idle);
    }

    #[test]
    fn empty_transcript_sets_failed_state() {
        // GIVEN
        let mut app = create_app(Vec::new(), None);
        app.voice_state = VoiceState::Recognizing;
        let expected = VoiceState::Failed("No speech was recognized".to_string());

        // WHEN
        app.handle_voice_event(VoiceEvent::Final("  ".to_string()));

        // THEN
        assert_eq!(app.voice_state, expected);
    }

    #[test]
    fn escape_cancels_voice_input_without_leaving_editing_mode() {
        // GIVEN
        let mut app = create_app(Vec::new(), None);
        app.input_mode = Mode::Editing;
        app.voice_state = VoiceState::Recording;
        let expected = Some(VoiceCommand::Cancel);

        // WHEN
        app.handle_key_event(KeyCode::Esc);

        // THEN
        assert_eq!(app.input_mode, Mode::Editing);
        assert_eq!(app.voice_state, VoiceState::Idle);
        assert_eq!(app.take_voice_command(), expected);
    }

    #[test]
    fn enter_does_not_add_task_during_voice_input() {
        // GIVEN
        let mut app = create_app(Vec::new(), None);
        app.input_mode = Mode::Editing;
        app.input_buffer = "unfinished".to_string();
        app.input_cursor = app.input_buffer.chars().count();
        app.voice_state = VoiceState::Recording;
        let expected = 0;

        // WHEN
        app.handle_key_event(KeyCode::Enter);

        // THEN
        assert_eq!(app.tasks.len(), expected);
        assert_eq!(app.input_mode, Mode::Editing);
    }

    #[test]
    fn permission_denial_sets_failed_state() {
        // GIVEN
        let mut app = create_app(Vec::new(), None);
        app.voice_state = VoiceState::Authorizing;
        let message = "Speech recognition permission was denied".to_string();
        let expected = VoiceState::Failed(message.clone());

        // WHEN
        app.handle_voice_event(VoiceEvent::PermissionDenied(message));

        // THEN
        assert_eq!(app.voice_state, expected);
    }

    #[test]
    fn recording_stops_after_thirty_seconds() {
        // GIVEN
        let mut app = create_app(Vec::new(), None);
        let started_at = Instant::now();
        app.voice_state = VoiceState::Recording;
        app.voice_recording_started_at = Some(started_at);
        let expected = Some(VoiceCommand::Stop);

        // WHEN
        app.tick(started_at + VOICE_RECORDING_LIMIT);

        // THEN
        assert_eq!(app.voice_state, VoiceState::Recognizing);
        assert_eq!(app.take_voice_command(), expected);
    }

    #[test]
    fn unavailable_on_device_recognition_sets_failed_state() {
        // GIVEN
        let mut app = create_app(Vec::new(), None);
        app.voice_state = VoiceState::Authorizing;
        let message = "On-device Japanese speech recognition is unavailable".to_string();
        let expected = VoiceState::Failed(message.clone());

        // WHEN
        app.handle_voice_event(VoiceEvent::Unavailable(message));

        // THEN
        assert_eq!(app.voice_state, expected);
    }

    #[test]
    fn status_update_failure_keeps_task_visible_and_unchanged() {
        // GIVEN
        let tasks_dir = temporary_tasks_dir();
        let task = Task::new_in("missing file".to_string(), tasks_dir.clone());
        let mut app = App {
            should_quit: false,
            input_mode: Mode::Normal,
            input_buffer: String::new(),
            input_cursor: 0,
            tasks: vec![task],
            selected_index: Some(0),
            parking_loaded: true,
            done_loaded: false,
            open_file: None,
            error_message: None,
            voice_state: VoiceState::Idle,
            voice_partial: String::new(),
            tasks_dir,
            persistent_error: None,
            voice_enabled: false,
            keyboard_release_supported: false,
            voice_key_pressed_at: None,
            voice_recording_started_at: None,
            pending_voice_command: None,
            voice_stop_requested: false,
            voice_last_key_event_at: None,
        };

        // WHEN
        app.handle_key_event(KeyCode::Char('n'));

        // THEN
        assert_eq!(app.tasks.len(), 1);
        assert_eq!(app.tasks[0].status, TaskStatus::Todo);
        assert_eq!(app.selected_index, Some(0));
        assert!(app.error_message.is_some());
    }
}
