use crate::application::TaskRepository;
use crate::domain::task::{StatusDirection, Task, TaskStatus, WeekRange};
use chrono::{NaiveDate, NaiveDateTime};
use crossterm::event::KeyCode;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use uuid::Uuid;

const DOUBLE_KEY_TIMEOUT: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, Debug)]
pub(crate) struct EventTime {
    pub(crate) local: NaiveDateTime,
    pub(crate) monotonic: Instant,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum AppEffect {
    None,
    Quit,
    OpenTask(PathBuf),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Selection {
    Task(Uuid),
    EmptyColumn(TaskStatus),
    None,
}

#[derive(Debug)]
enum InputMode {
    Normal,
    Editing { buffer: String, cursor: usize },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HorizontalDirection {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Command {
    Quit,
    BeginEditing,
    ConfirmEditing,
    CancelEditing,
    MoveCursorLeft,
    MoveCursorRight,
    DeleteCharacter,
    InsertCharacter(char),
    SelectNext,
    SelectPrevious,
    SelectHorizontal(HorizontalDirection),
    SelectLast,
    ChangeStatus(StatusDirection),
    Reload,
    ToggleDone,
    PreviousDoneWeek,
    NextDoneWeek,
    OpenTask,
    Ignore,
}

pub(crate) enum InputView<'a> {
    Normal,
    Editing { buffer: &'a str, cursor: usize },
}

pub(crate) struct AppView<'a> {
    pub(crate) input: InputView<'a>,
    pub(crate) tasks: &'a [Task],
    pub(crate) selection: Selection,
    pub(crate) done_week: Option<WeekRange>,
    pub(crate) last_updated_at: NaiveDateTime,
    pub(crate) error_message: Option<String>,
}

pub(crate) struct App<R: TaskRepository> {
    repository: R,
    input_mode: InputMode,
    tasks: Vec<Task>,
    selection: Selection,
    parking_loaded: bool,
    done_week: Option<WeekRange>,
    last_updated_at: NaiveDateTime,
    persistent_error: Option<String>,
    operation_error: Option<String>,
    pending_g_at: Option<Instant>,
}

impl<R: TaskRepository> App<R> {
    pub(crate) fn new(repository: R, now: NaiveDateTime) -> Self {
        let todo_result = repository.load(TaskStatus::Todo);
        let doing_result = repository.load(TaskStatus::Doing);
        let persistent_error = todo_result
            .as_ref()
            .err()
            .or_else(|| doing_result.as_ref().err())
            .map(|error| format!("Failed to load tasks: {error}"));
        let tasks = Task::sorted(
            todo_result
                .unwrap_or_default()
                .into_iter()
                .chain(doing_result.unwrap_or_default())
                .collect(),
        );
        let selection = tasks
            .first()
            .map_or(Selection::None, |task| Selection::Task(task.id()));
        Self {
            repository,
            input_mode: InputMode::Normal,
            tasks,
            selection,
            parking_loaded: false,
            done_week: None,
            last_updated_at: now,
            persistent_error,
            operation_error: None,
            pending_g_at: None,
        }
    }

    pub(crate) fn view(&self) -> AppView<'_> {
        let input = match &self.input_mode {
            InputMode::Normal => InputView::Normal,
            InputMode::Editing { buffer, cursor } => InputView::Editing {
                buffer,
                cursor: *cursor,
            },
        };
        AppView {
            input,
            tasks: &self.tasks,
            selection: self.selection,
            done_week: self.done_week,
            last_updated_at: self.last_updated_at,
            error_message: self.error_message(),
        }
    }

    pub(crate) fn load_parking_after_first_render(&mut self) {
        if self.parking_loaded {
            return;
        }
        let parking_tasks = match self.repository.load(TaskStatus::Parking) {
            Ok(tasks) => tasks,
            Err(error) => {
                self.set_operation_error(format!("Failed to load PARKING tasks: {error}"));
                return;
            }
        };
        let selected_id = self.selected_task().map(Task::id);
        self.tasks.extend(parking_tasks);
        self.tasks = Task::sorted(std::mem::take(&mut self.tasks));
        self.parking_loaded = true;
        self.operation_error = None;
        self.selection = selected_id
            .and_then(|id| self.task_index(id).map(|_| Selection::Task(id)))
            .or_else(|| self.tasks.first().map(|task| Selection::Task(task.id())))
            .unwrap_or(Selection::None);
    }

    pub(crate) fn handle_key_event(
        &mut self,
        key_code: KeyCode,
        event_time: EventTime,
    ) -> AppEffect {
        if matches!(self.input_mode, InputMode::Normal) && key_code == KeyCode::Char('g') {
            return self.handle_g(event_time.monotonic);
        }
        if matches!(self.input_mode, InputMode::Normal) {
            self.pending_g_at = None;
        }
        let command = Self::command_for(&self.input_mode, key_code);
        self.execute(command, event_time)
    }

    pub(crate) fn after_edit(&mut self) {
        let Some(current) = self.selected_task().cloned() else {
            return;
        };
        match self.repository.reload(&current) {
            Ok(reloaded) => {
                if let Some(index) = self.task_index(current.id()) {
                    self.tasks[index] = reloaded;
                    self.operation_error = None;
                }
            }
            Err(error) => {
                self.set_operation_error(format!("Failed to reload edited task: {error}"))
            }
        }
    }

    pub(crate) fn report_runtime_error(&mut self, message: String) {
        self.set_operation_error(message);
    }

    fn command_for(input_mode: &InputMode, key_code: KeyCode) -> Command {
        match input_mode {
            InputMode::Normal => match key_code {
                KeyCode::Char('q') | KeyCode::Esc => Command::Quit,
                KeyCode::Char('a') => Command::BeginEditing,
                KeyCode::Char('j') | KeyCode::Down => Command::SelectNext,
                KeyCode::Char('k') | KeyCode::Up => Command::SelectPrevious,
                KeyCode::Char('h') | KeyCode::Left => {
                    Command::SelectHorizontal(HorizontalDirection::Left)
                }
                KeyCode::Char('l') | KeyCode::Right => {
                    Command::SelectHorizontal(HorizontalDirection::Right)
                }
                KeyCode::Char('G') => Command::SelectLast,
                KeyCode::Char('n') => Command::ChangeStatus(StatusDirection::Forward),
                KeyCode::Char('N') => Command::ChangeStatus(StatusDirection::Backward),
                KeyCode::Char('r') => Command::Reload,
                KeyCode::Char('d') => Command::ToggleDone,
                KeyCode::Char('[') => Command::PreviousDoneWeek,
                KeyCode::Char(']') => Command::NextDoneWeek,
                KeyCode::Enter => Command::OpenTask,
                KeyCode::Backspace
                | KeyCode::Char(_)
                | KeyCode::F(_)
                | KeyCode::Home
                | KeyCode::End
                | KeyCode::PageUp
                | KeyCode::PageDown
                | KeyCode::Tab
                | KeyCode::BackTab
                | KeyCode::Delete
                | KeyCode::Insert
                | KeyCode::Null
                | KeyCode::CapsLock
                | KeyCode::ScrollLock
                | KeyCode::NumLock
                | KeyCode::PrintScreen
                | KeyCode::Pause
                | KeyCode::Menu
                | KeyCode::KeypadBegin
                | KeyCode::Media(_)
                | KeyCode::Modifier(_) => Command::Ignore,
            },
            InputMode::Editing { .. } => match key_code {
                KeyCode::Enter => Command::ConfirmEditing,
                KeyCode::Esc => Command::CancelEditing,
                KeyCode::Left => Command::MoveCursorLeft,
                KeyCode::Right => Command::MoveCursorRight,
                KeyCode::Backspace => Command::DeleteCharacter,
                KeyCode::Char(character) => Command::InsertCharacter(character),
                KeyCode::F(_)
                | KeyCode::Home
                | KeyCode::End
                | KeyCode::PageUp
                | KeyCode::PageDown
                | KeyCode::Up
                | KeyCode::Down
                | KeyCode::Tab
                | KeyCode::BackTab
                | KeyCode::Delete
                | KeyCode::Insert
                | KeyCode::Null
                | KeyCode::CapsLock
                | KeyCode::ScrollLock
                | KeyCode::NumLock
                | KeyCode::PrintScreen
                | KeyCode::Pause
                | KeyCode::Menu
                | KeyCode::KeypadBegin
                | KeyCode::Media(_)
                | KeyCode::Modifier(_) => Command::Ignore,
            },
        }
    }

    fn execute(&mut self, command: Command, event_time: EventTime) -> AppEffect {
        match command {
            Command::Quit => AppEffect::Quit,
            Command::BeginEditing => {
                self.input_mode = InputMode::Editing {
                    buffer: String::new(),
                    cursor: 0,
                };
                AppEffect::None
            }
            Command::ConfirmEditing => {
                self.add_task(event_time.local);
                AppEffect::None
            }
            Command::CancelEditing => {
                self.input_mode = InputMode::Normal;
                AppEffect::None
            }
            Command::MoveCursorLeft => {
                self.move_input_cursor_left();
                AppEffect::None
            }
            Command::MoveCursorRight => {
                self.move_input_cursor_right();
                AppEffect::None
            }
            Command::DeleteCharacter => {
                self.delete_character_before_cursor();
                AppEffect::None
            }
            Command::InsertCharacter(character) => {
                self.insert_character_at_cursor(character);
                AppEffect::None
            }
            Command::SelectNext => {
                self.select_vertical(true);
                AppEffect::None
            }
            Command::SelectPrevious => {
                self.select_vertical(false);
                AppEffect::None
            }
            Command::SelectHorizontal(direction) => {
                self.select_horizontal(direction);
                AppEffect::None
            }
            Command::SelectLast => {
                self.select_last();
                AppEffect::None
            }
            Command::ChangeStatus(direction) => {
                self.change_status(direction, event_time.local);
                AppEffect::None
            }
            Command::Reload => {
                self.reload_tasks(event_time.local);
                AppEffect::None
            }
            Command::ToggleDone => {
                self.toggle_done(event_time.local.date());
                AppEffect::None
            }
            Command::PreviousDoneWeek => {
                self.change_done_week(false);
                AppEffect::None
            }
            Command::NextDoneWeek => {
                self.show_next_done_week(event_time.local.date());
                AppEffect::None
            }
            Command::OpenTask => self
                .selected_task()
                .map(|task| AppEffect::OpenTask(self.repository.path(task)))
                .unwrap_or(AppEffect::None),
            Command::Ignore => AppEffect::None,
        }
    }

    fn handle_g(&mut self, now: Instant) -> AppEffect {
        let is_double_g = self.pending_g_at.is_some_and(|started_at| {
            now.saturating_duration_since(started_at) <= DOUBLE_KEY_TIMEOUT
        });
        if is_double_g {
            self.select_first();
            self.pending_g_at = None;
        } else {
            self.pending_g_at = Some(now);
        }
        AppEffect::None
    }

    fn insert_character_at_cursor(&mut self, character: char) {
        let InputMode::Editing { buffer, cursor } = &mut self.input_mode else {
            return;
        };
        let byte_index = buffer
            .char_indices()
            .nth(*cursor)
            .map_or(buffer.len(), |(index, _)| index);
        buffer.insert(byte_index, character);
        *cursor += 1;
    }

    fn delete_character_before_cursor(&mut self) {
        let InputMode::Editing { buffer, cursor } = &mut self.input_mode else {
            return;
        };
        if *cursor == 0 {
            return;
        }
        let start = buffer
            .char_indices()
            .nth(*cursor - 1)
            .map_or(0, |(index, _)| index);
        let end = buffer
            .char_indices()
            .nth(*cursor)
            .map_or(buffer.len(), |(index, _)| index);
        buffer.replace_range(start..end, "");
        *cursor -= 1;
    }

    fn move_input_cursor_left(&mut self) {
        if let InputMode::Editing { cursor, .. } = &mut self.input_mode {
            *cursor = cursor.saturating_sub(1);
        }
    }

    fn move_input_cursor_right(&mut self) {
        if let InputMode::Editing { buffer, cursor } = &mut self.input_mode {
            *cursor = (*cursor + 1).min(buffer.chars().count());
        }
    }

    fn select_vertical(&mut self, forward: bool) {
        let Some(index) = self.selected_task_index() else {
            return;
        };
        let indices = self.indices_for_status(self.tasks[index].status());
        let row = indices
            .iter()
            .position(|candidate| *candidate == index)
            .unwrap_or(0);
        let selected_row = if forward {
            (row + 1).min(indices.len().saturating_sub(1))
        } else {
            row.saturating_sub(1)
        };
        self.selection = indices.get(selected_row).map_or(self.selection, |index| {
            Selection::Task(self.tasks[*index].id())
        });
    }

    fn select_first(&mut self) {
        let Some(task) = self.selected_task() else {
            return;
        };
        self.selection = self
            .indices_for_status(task.status())
            .first()
            .map_or(self.selection, |index| {
                Selection::Task(self.tasks[*index].id())
            });
    }

    fn select_last(&mut self) {
        let Some(task) = self.selected_task() else {
            return;
        };
        self.selection = self
            .indices_for_status(task.status())
            .last()
            .map_or(self.selection, |index| {
                Selection::Task(self.tasks[*index].id())
            });
    }

    fn select_horizontal(&mut self, direction: HorizontalDirection) {
        let Some(index) = self.selected_task_index() else {
            return;
        };
        let current_status = self.tasks[index].status();
        let current_indices = self.indices_for_status(current_status);
        let row = current_indices
            .iter()
            .position(|candidate| *candidate == index)
            .unwrap_or(0);
        let statuses = self.visible_statuses();
        let Some(column) = statuses.iter().position(|status| *status == current_status) else {
            return;
        };
        let columns: Vec<usize> = match direction {
            HorizontalDirection::Left => (0..column).rev().collect(),
            HorizontalDirection::Right => ((column + 1)..statuses.len()).collect(),
        };
        self.selection = columns
            .into_iter()
            .map(|candidate| self.indices_for_status(statuses[candidate]))
            .find(|indices| !indices.is_empty())
            .and_then(|indices| indices.get(row.min(indices.len() - 1)).copied())
            .map_or(self.selection, |index| {
                Selection::Task(self.tasks[index].id())
            });
    }

    fn reload_tasks(&mut self, now: NaiveDateTime) {
        let previous_selection = self.selection_details();
        let loaded_tasks = match self.load_visible_tasks() {
            Ok(tasks) => Task::sorted(tasks),
            Err(error) => {
                self.set_operation_error(format!("Failed to reload tasks: {error}"));
                return;
            }
        };
        self.tasks = loaded_tasks;
        self.parking_loaded = true;
        self.last_updated_at = now;
        self.operation_error = None;
        self.selection = previous_selection
            .and_then(|(id, status, row)| {
                self.task_index(id)
                    .map(|_| Selection::Task(id))
                    .or_else(|| self.nearby_selection(status, row))
            })
            .or_else(|| self.tasks.first().map(|task| Selection::Task(task.id())))
            .unwrap_or(Selection::None);
    }

    fn load_visible_tasks(&self) -> std::io::Result<Vec<Task>> {
        let done_tasks = self.done_week.map_or_else(
            || Ok(Vec::new()),
            |week| {
                self.repository.load(TaskStatus::Done).map(|tasks| {
                    tasks
                        .into_iter()
                        .filter(|task| {
                            task.completed_at()
                                .is_some_and(|completed_at| week.contains(completed_at.date()))
                        })
                        .collect()
                })
            },
        )?;
        Ok(self
            .repository
            .load(TaskStatus::Parking)?
            .into_iter()
            .chain(self.repository.load(TaskStatus::Todo)?)
            .chain(self.repository.load(TaskStatus::Doing)?)
            .chain(done_tasks)
            .collect())
    }

    fn change_status(&mut self, direction: StatusDirection, now: NaiveDateTime) {
        let Some(index) = self.selected_task_index() else {
            return;
        };
        let current = self.tasks[index].clone();
        let Some(updated) = current.transitioned(direction, now) else {
            return;
        };
        let previous_status = current.status();
        let previous_row = self
            .indices_for_status(previous_status)
            .iter()
            .position(|candidate| *candidate == index)
            .unwrap_or(0);
        if let Err(error) = self.repository.replace(&current, &updated) {
            self.set_operation_error(format!("Failed to update task status: {error}"));
            return;
        }
        self.operation_error = None;
        self.tasks[index] = updated.clone();
        let belongs_to_visible_done_week = self.done_week.is_some_and(|week| {
            updated.status() == TaskStatus::Done
                && updated
                    .completed_at()
                    .is_some_and(|completed_at| week.contains(completed_at.date()))
        });
        if updated.status() == TaskStatus::Done && !belongs_to_visible_done_week {
            self.tasks.retain(|task| task.id() != updated.id());
            self.tasks = Task::sorted(std::mem::take(&mut self.tasks));
            self.selection = self
                .nearby_selection(previous_status, previous_row)
                .unwrap_or(Selection::None);
            return;
        }
        self.tasks = Task::sorted(std::mem::take(&mut self.tasks));
        self.selection = Selection::Task(updated.id());
    }

    fn add_task(&mut self, now: NaiveDateTime) {
        let InputMode::Editing { buffer, .. } = &self.input_mode else {
            return;
        };
        if buffer.is_empty() {
            self.input_mode = InputMode::Normal;
            self.operation_error = None;
            return;
        }
        let new_task = match Task::new(buffer.clone(), now) {
            Ok(task) => task,
            Err(error) => {
                self.set_operation_error(format!("Failed to add task: {error}"));
                return;
            }
        };
        if let Err(error) = self.repository.save(&new_task) {
            self.set_operation_error(format!("Failed to add task: {error}"));
            return;
        }
        self.tasks.push(new_task);
        self.tasks = Task::sorted(std::mem::take(&mut self.tasks));
        if self.selection == Selection::None {
            self.selection = self
                .tasks
                .first()
                .map_or(Selection::None, |task| Selection::Task(task.id()));
        }
        self.input_mode = InputMode::Normal;
        self.operation_error = None;
    }

    fn toggle_done(&mut self, today: NaiveDate) {
        if self.done_week.is_some() {
            let selected_details = self.selection_details();
            let selected_done = self
                .selected_task()
                .is_some_and(|task| task.status() == TaskStatus::Done);
            self.tasks.retain(|task| task.status() != TaskStatus::Done);
            self.done_week = None;
            if selected_done {
                self.selection = selected_details
                    .and_then(|(_, _, row)| self.nearby_selection(TaskStatus::Doing, row))
                    .unwrap_or(Selection::None);
            } else if matches!(self.selection, Selection::EmptyColumn(TaskStatus::Done)) {
                self.selection = Selection::None;
            }
            return;
        }
        let week = match WeekRange::containing(today) {
            Ok(week) => week,
            Err(error) => {
                self.set_operation_error(format!("Failed to show DONE tasks: {error}"));
                return;
            }
        };
        if self.load_done_week(week) {
            self.done_week = Some(week);
        }
    }

    fn change_done_week(&mut self, forward: bool) {
        let Some(current) = self.done_week else {
            return;
        };
        let target = if forward {
            current.next()
        } else {
            current.previous()
        };
        let target = match target {
            Ok(week) => week,
            Err(error) => {
                self.set_operation_error(format!("Failed to change DONE week: {error}"));
                return;
            }
        };
        if self.load_done_week(target) {
            self.done_week = Some(target);
            self.select_done_column();
        }
    }

    fn show_next_done_week(&mut self, today: NaiveDate) {
        let Some(current) = self.done_week else {
            return;
        };
        let current_week = match WeekRange::containing(today) {
            Ok(week) => week,
            Err(error) => {
                self.set_operation_error(format!("Failed to change DONE week: {error}"));
                return;
            }
        };
        let Ok(next) = current.next() else {
            self.set_operation_error(
                "Failed to change DONE week: next week is outside the supported date range"
                    .to_string(),
            );
            return;
        };
        if next.start() <= current_week.start() && self.load_done_week(next) {
            self.done_week = Some(next);
            self.select_done_column();
        }
    }

    fn load_done_week(&mut self, week: WeekRange) -> bool {
        let selected_id = self.selected_task().map(Task::id);
        let done_tasks = match self.repository.load(TaskStatus::Done) {
            Ok(tasks) => tasks
                .into_iter()
                .filter(|task| {
                    task.completed_at()
                        .is_some_and(|completed_at| week.contains(completed_at.date()))
                })
                .collect::<Vec<_>>(),
            Err(error) => {
                self.set_operation_error(format!("Failed to load DONE tasks: {error}"));
                return false;
            }
        };
        self.tasks.retain(|task| task.status() != TaskStatus::Done);
        self.tasks.extend(done_tasks);
        self.tasks = Task::sorted(std::mem::take(&mut self.tasks));
        self.selection = selected_id
            .and_then(|id| self.task_index(id).map(|_| Selection::Task(id)))
            .or_else(|| {
                self.tasks
                    .iter()
                    .find(|task| task.status() == TaskStatus::Done)
                    .map(|task| Selection::Task(task.id()))
            })
            .or_else(|| self.tasks.first().map(|task| Selection::Task(task.id())))
            .unwrap_or(Selection::None);
        self.operation_error = None;
        true
    }

    fn select_done_column(&mut self) {
        self.selection = self
            .tasks
            .iter()
            .find(|task| task.status() == TaskStatus::Done)
            .map_or(Selection::EmptyColumn(TaskStatus::Done), |task| {
                Selection::Task(task.id())
            });
    }

    fn visible_statuses(&self) -> Vec<TaskStatus> {
        TaskStatus::ALL
            .into_iter()
            .filter(|status| *status != TaskStatus::Done || self.done_week.is_some())
            .collect()
    }

    fn indices_for_status(&self, status: TaskStatus) -> Vec<usize> {
        self.tasks
            .iter()
            .enumerate()
            .filter_map(|(index, task)| (task.status() == status).then_some(index))
            .collect()
    }

    fn nearby_selection(&self, preferred_status: TaskStatus, row: usize) -> Option<Selection> {
        let preferred = self.indices_for_status(preferred_status);
        if let Some(index) = preferred.get(row.min(preferred.len().saturating_sub(1))) {
            return Some(Selection::Task(self.tasks[*index].id()));
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
            .map(|index| Selection::Task(self.tasks[index].id()))
    }

    fn selection_details(&self) -> Option<(Uuid, TaskStatus, usize)> {
        let index = self.selected_task_index()?;
        let task = &self.tasks[index];
        let row = self
            .indices_for_status(task.status())
            .iter()
            .position(|candidate| *candidate == index)
            .unwrap_or(0);
        Some((task.id(), task.status(), row))
    }

    fn selected_task(&self) -> Option<&Task> {
        self.selected_task_index()
            .and_then(|index| self.tasks.get(index))
    }

    fn selected_task_index(&self) -> Option<usize> {
        let Selection::Task(id) = self.selection else {
            return None;
        };
        self.task_index(id)
    }

    fn task_index(&self, id: Uuid) -> Option<usize> {
        self.tasks.iter().position(|task| task.id() == id)
    }

    fn error_message(&self) -> Option<String> {
        match (&self.persistent_error, &self.operation_error) {
            (Some(persistent), Some(operation)) => Some(format!("{persistent} | {operation}")),
            (Some(persistent), None) => Some(persistent.clone()),
            (None, Some(operation)) => Some(operation.clone()),
            (None, None) => None,
        }
    }

    fn set_operation_error(&mut self, message: String) {
        self.operation_error = Some(message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use std::io;

    #[derive(Default)]
    struct FakeTaskRepository {
        tasks: Vec<Task>,
        load_failure: Option<TaskStatus>,
        save_fails: bool,
        replace_fails: bool,
    }

    impl TaskRepository for FakeTaskRepository {
        fn load(&self, status: TaskStatus) -> io::Result<Vec<Task>> {
            if self.load_failure == Some(status) {
                return Err(io::Error::other(format!("failed to load {status:?}")));
            }
            Ok(self
                .tasks
                .iter()
                .filter(|task| task.status() == status)
                .cloned()
                .collect())
        }

        fn save(&mut self, task: &Task) -> io::Result<()> {
            if self.save_fails {
                return Err(io::Error::other("save failed"));
            }
            self.tasks.push(task.clone());
            Ok(())
        }

        fn replace(&mut self, current: &Task, updated: &Task) -> io::Result<()> {
            if self.replace_fails {
                return Err(io::Error::other("replace failed"));
            }
            let index = self
                .tasks
                .iter()
                .position(|task| task.id() == current.id())
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "task not found"))?;
            self.tasks[index] = updated.clone();
            Ok(())
        }

        fn reload(&self, task: &Task) -> io::Result<Task> {
            self.tasks
                .iter()
                .find(|stored| stored.id() == task.id())
                .cloned()
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "task not found"))
        }

        fn path(&self, task: &Task) -> PathBuf {
            PathBuf::from(format!("/{:?}/{}.md", task.status(), task.id()))
        }
    }

    fn datetime(day: u32, hour: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 6, day)
            .and_then(|date| date.and_hms_opt(hour, 0, 0))
            .expect("test datetime should be valid")
    }

    fn task(name: &str, status: TaskStatus, created_at: NaiveDateTime) -> Task {
        Task::from_stored(
            Uuid::new_v4(),
            name.to_string(),
            status,
            created_at,
            created_at,
            (status == TaskStatus::Done).then_some(created_at),
            created_at.date(),
        )
    }

    fn event_time(local: NaiveDateTime) -> EventTime {
        EventTime {
            local,
            monotonic: Instant::now(),
        }
    }

    fn enter_task(app: &mut App<FakeTaskRepository>, name: &str, now: NaiveDateTime) {
        app.handle_key_event(KeyCode::Char('a'), event_time(now));
        name.chars().for_each(|character| {
            app.handle_key_event(KeyCode::Char(character), event_time(now));
        });
        app.handle_key_event(KeyCode::Enter, event_time(now));
    }

    #[test]
    fn startup_loads_todo_and_doing_before_parking() {
        // GIVEN
        let repository = FakeTaskRepository {
            tasks: vec![
                task("parking", TaskStatus::Parking, datetime(15, 8)),
                task("todo", TaskStatus::Todo, datetime(15, 9)),
                task("doing", TaskStatus::Doing, datetime(15, 10)),
            ],
            ..FakeTaskRepository::default()
        };
        let mut app = App::new(repository, datetime(15, 12));

        // WHEN
        let before: Vec<_> = app
            .view()
            .tasks
            .iter()
            .map(|task| task.name().to_string())
            .collect();
        app.load_parking_after_first_render();
        let after: Vec<_> = app
            .view()
            .tasks
            .iter()
            .map(|task| task.name().to_string())
            .collect();

        // THEN
        assert_eq!(before, vec!["todo", "doing"]);
        assert_eq!(after, vec!["parking", "todo", "doing"]);
    }

    #[test]
    fn startup_keeps_successful_statuses_when_another_load_fails() {
        // GIVEN
        let repository = FakeTaskRepository {
            tasks: vec![task("doing", TaskStatus::Doing, datetime(15, 10))],
            load_failure: Some(TaskStatus::Todo),
            ..FakeTaskRepository::default()
        };

        // WHEN
        let app = App::new(repository, datetime(15, 12));

        // THEN
        let view = app.view();
        let actual: Vec<_> = view.tasks.iter().map(Task::name).collect();
        assert_eq!(actual, vec!["doing"]);
        assert_eq!(
            view.error_message,
            Some("Failed to load tasks: failed to load Todo".to_string())
        );
    }

    #[test]
    fn editing_creates_and_selects_a_persisted_task() {
        // GIVEN
        let repository = FakeTaskRepository::default();
        let mut app = App::new(repository, datetime(15, 12));

        // WHEN
        enter_task(&mut app, "new task", datetime(15, 13));

        // THEN
        let view = app.view();
        let expected_name = "new task";
        assert_eq!(view.tasks.len(), 1);
        assert_eq!(view.tasks[0].name(), expected_name);
        assert_eq!(view.tasks[0].status(), TaskStatus::Todo);
        assert_eq!(view.selection, Selection::Task(view.tasks[0].id()));
        assert!(matches!(view.input, InputView::Normal));
    }

    #[test]
    fn failed_save_keeps_the_editor_and_input_available_for_retry() {
        // GIVEN
        let repository = FakeTaskRepository {
            save_fails: true,
            ..FakeTaskRepository::default()
        };
        let mut app = App::new(repository, datetime(15, 12));

        // WHEN
        enter_task(&mut app, "retry me", datetime(15, 13));

        // THEN
        let view = app.view();
        assert!(matches!(
            view.input,
            InputView::Editing {
                buffer: "retry me",
                cursor: 8
            }
        ));
        assert_eq!(view.tasks, &[]);
        assert_eq!(
            view.error_message,
            Some("Failed to add task: save failed".to_string())
        );
    }

    #[test]
    fn navigation_tracks_task_identity_across_columns() {
        // GIVEN
        let todo_one = task("todo one", TaskStatus::Todo, datetime(15, 8));
        let todo_two = task("todo two", TaskStatus::Todo, datetime(15, 9));
        let doing = task("doing", TaskStatus::Doing, datetime(15, 10));
        let expected = doing.id();
        let repository = FakeTaskRepository {
            tasks: vec![todo_one, todo_two, doing],
            ..FakeTaskRepository::default()
        };
        let mut app = App::new(repository, datetime(15, 12));
        app.handle_key_event(KeyCode::Char('j'), event_time(datetime(15, 12)));

        // WHEN
        app.handle_key_event(KeyCode::Char('l'), event_time(datetime(15, 12)));

        // THEN
        assert_eq!(app.view().selection, Selection::Task(expected));
    }

    #[test]
    fn deterministic_double_g_selects_the_first_task_in_the_column() {
        // GIVEN
        let first = task("first", TaskStatus::Todo, datetime(15, 8));
        let second = task("second", TaskStatus::Todo, datetime(15, 9));
        let expected = first.id();
        let repository = FakeTaskRepository {
            tasks: vec![first, second],
            ..FakeTaskRepository::default()
        };
        let mut app = App::new(repository, datetime(15, 12));
        app.handle_key_event(KeyCode::Char('j'), event_time(datetime(15, 12)));
        let started_at = Instant::now();
        app.handle_key_event(
            KeyCode::Char('g'),
            EventTime {
                local: datetime(15, 12),
                monotonic: started_at,
            },
        );

        // WHEN
        app.handle_key_event(
            KeyCode::Char('g'),
            EventTime {
                local: datetime(15, 12),
                monotonic: started_at + Duration::from_millis(100),
            },
        );

        // THEN
        assert_eq!(app.view().selection, Selection::Task(expected));
    }

    #[test]
    fn failed_status_change_keeps_the_visible_task_unchanged() {
        // GIVEN
        let todo = task("todo", TaskStatus::Todo, datetime(15, 8));
        let expected = todo.clone();
        let repository = FakeTaskRepository {
            tasks: vec![todo],
            replace_fails: true,
            ..FakeTaskRepository::default()
        };
        let mut app = App::new(repository, datetime(15, 12));

        // WHEN
        app.handle_key_event(KeyCode::Char('n'), event_time(datetime(15, 13)));

        // THEN
        let view = app.view();
        assert_eq!(view.tasks, &[expected]);
        assert_eq!(
            view.error_message,
            Some("Failed to update task status: replace failed".to_string())
        );
    }

    #[test]
    fn completing_a_task_while_done_is_hidden_selects_a_visible_neighbor() {
        // GIVEN
        let target = task("target", TaskStatus::Doing, datetime(15, 8));
        let neighbor = task("neighbor", TaskStatus::Doing, datetime(15, 9));
        let expected = neighbor.id();
        let repository = FakeTaskRepository {
            tasks: vec![target, neighbor],
            ..FakeTaskRepository::default()
        };
        let mut app = App::new(repository, datetime(15, 12));

        // WHEN
        app.handle_key_event(KeyCode::Char('n'), event_time(datetime(15, 13)));

        // THEN
        let view = app.view();
        assert_eq!(view.tasks.len(), 1);
        assert_eq!(view.tasks[0].id(), expected);
        assert_eq!(view.selection, Selection::Task(expected));
    }

    #[test]
    fn changing_to_an_empty_done_week_selects_the_empty_column() {
        // GIVEN
        let done = task("done", TaskStatus::Done, datetime(15, 8));
        let repository = FakeTaskRepository {
            tasks: vec![done],
            ..FakeTaskRepository::default()
        };
        let mut app = App::new(repository, datetime(15, 12));
        app.handle_key_event(KeyCode::Char('d'), event_time(datetime(15, 12)));

        // WHEN
        app.handle_key_event(KeyCode::Char('['), event_time(datetime(15, 12)));

        // THEN
        let view = app.view();
        assert_eq!(view.selection, Selection::EmptyColumn(TaskStatus::Done));
        assert_eq!(
            view.done_week.map(WeekRange::start),
            Some(NaiveDate::from_ymd_opt(2026, 6, 8).expect("test date should be valid"))
        );
    }

    #[test]
    fn quit_and_open_keys_return_runtime_effects() {
        // GIVEN
        let todo = task("todo", TaskStatus::Todo, datetime(15, 8));
        let expected_path = PathBuf::from(format!("/Todo/{}.md", todo.id()));
        let repository = FakeTaskRepository {
            tasks: vec![todo],
            ..FakeTaskRepository::default()
        };
        let mut app = App::new(repository, datetime(15, 12));

        // WHEN
        let actual = [
            app.handle_key_event(KeyCode::Enter, event_time(datetime(15, 12))),
            app.handle_key_event(KeyCode::Char('q'), event_time(datetime(15, 12))),
        ];

        // THEN
        let expected = [AppEffect::OpenTask(expected_path), AppEffect::Quit];
        assert_eq!(actual, expected);
    }

    #[test]
    fn parking_load_failure_is_reported_and_retried() {
        // GIVEN
        let parking = task("parking", TaskStatus::Parking, datetime(15, 8));
        let expected = parking.clone();
        let repository = FakeTaskRepository {
            tasks: vec![parking],
            load_failure: Some(TaskStatus::Parking),
            ..FakeTaskRepository::default()
        };
        let mut app = App::new(repository, datetime(15, 12));
        app.load_parking_after_first_render();
        app.repository.load_failure = None;

        // WHEN
        app.load_parking_after_first_render();

        // THEN
        let view = app.view();
        assert_eq!(view.tasks, &[expected]);
        assert_eq!(view.error_message, None);
    }

    #[test]
    fn reload_failure_keeps_tasks_selection_and_timestamp_unchanged() {
        // GIVEN
        let todo = task("todo", TaskStatus::Todo, datetime(15, 8));
        let expected_task = todo.clone();
        let expected_selection = Selection::Task(todo.id());
        let expected_time = datetime(15, 12);
        let repository = FakeTaskRepository {
            tasks: vec![todo],
            ..FakeTaskRepository::default()
        };
        let mut app = App::new(repository, expected_time);
        app.repository.load_failure = Some(TaskStatus::Parking);

        // WHEN
        app.handle_key_event(KeyCode::Char('r'), event_time(datetime(15, 13)));

        // THEN
        let view = app.view();
        assert_eq!(view.tasks, &[expected_task]);
        assert_eq!(view.selection, expected_selection);
        assert_eq!(view.last_updated_at, expected_time);
        assert_eq!(
            view.error_message,
            Some("Failed to reload tasks: failed to load Parking".to_string())
        );
    }

    #[test]
    fn done_toggle_loads_only_tasks_from_the_current_week() {
        // GIVEN
        let previous = task("previous", TaskStatus::Done, datetime(8, 8));
        let current = task("current", TaskStatus::Done, datetime(15, 8));
        let expected = current.clone();
        let repository = FakeTaskRepository {
            tasks: vec![previous, current],
            ..FakeTaskRepository::default()
        };
        let mut app = App::new(repository, datetime(15, 12));

        // WHEN
        app.handle_key_event(KeyCode::Char('d'), event_time(datetime(15, 12)));

        // THEN
        let done_tasks: Vec<_> = app
            .view()
            .tasks
            .iter()
            .filter(|task| task.status() == TaskStatus::Done)
            .cloned()
            .collect();
        assert_eq!(done_tasks, vec![expected]);
    }

    #[test]
    fn next_done_week_does_not_move_beyond_the_current_week() {
        // GIVEN
        let repository = FakeTaskRepository::default();
        let mut app = App::new(repository, datetime(15, 12));
        app.handle_key_event(KeyCode::Char('d'), event_time(datetime(15, 12)));
        let expected = app.view().done_week;

        // WHEN
        app.handle_key_event(KeyCode::Char(']'), event_time(datetime(15, 12)));

        // THEN
        assert_eq!(app.view().done_week, expected);
    }

    #[test]
    fn editing_cursor_handles_multibyte_insertion_and_deletion() {
        // GIVEN
        let repository = FakeTaskRepository::default();
        let mut app = App::new(repository, datetime(15, 12));
        app.handle_key_event(KeyCode::Char('a'), event_time(datetime(15, 12)));
        app.handle_key_event(KeyCode::Char('日'), event_time(datetime(15, 12)));
        app.handle_key_event(KeyCode::Char('本'), event_time(datetime(15, 12)));
        app.handle_key_event(KeyCode::Left, event_time(datetime(15, 12)));
        app.handle_key_event(KeyCode::Char('語'), event_time(datetime(15, 12)));

        // WHEN
        app.handle_key_event(KeyCode::Backspace, event_time(datetime(15, 12)));

        // THEN
        assert!(matches!(
            app.view().input,
            InputView::Editing {
                buffer: "日本",
                cursor: 1
            }
        ));
    }
}
