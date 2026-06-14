use chrono::{DateTime, Datelike, Days, Local, NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub const DEADLINE_DATE_FORMAT: &str = "%Y/%m/%d";
pub const TASK_DATETIME_FORMAT: &str = "%Y/%m/%d %H:%M:%S";
const LEGACY_DEADLINE_DATE_FORMAT: &str = "%Y-%m-%d";

/// Represents the lifecycle status of a task.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TaskStatus {
    Parking,
    Todo,
    Doing,
    Done,
}

impl TaskStatus {
    /// Returns the directory name corresponding to this status (e.g. `"todo"`, `"doing"`, `"done"`).
    fn dir_name(&self) -> &str {
        match self {
            TaskStatus::Parking => "parking",
            TaskStatus::Todo => "todo",
            TaskStatus::Doing => "doing",
            TaskStatus::Done => "done",
        }
    }
}

/// Internal representation of the YAML frontmatter stored in each task's markdown file.
///
/// Does not include `status`, which is determined by the directory the file resides in.
#[derive(Clone, Serialize, Deserialize)]
struct TaskFrontmatter {
    id: Uuid,
    name: String,
    created_at: NaiveDateTime,
    updated_at: NaiveDateTime,
    #[serde(skip_serializing_if = "Option::is_none")]
    completed_at: Option<NaiveDateTime>,
    #[serde(default)]
    deadline: Option<String>,
}

#[derive(Deserialize)]
struct StoredTaskFrontmatter {
    id: Uuid,
    name: String,
    created_at: String,
    updated_at: String,
    #[serde(default)]
    completed_at: Option<String>,
    #[serde(default)]
    deadline: Option<String>,
}

/// A TODO task with metadata and lifecycle status.
#[derive(Clone)]
pub struct Task {
    pub id: Uuid,
    pub name: String,
    pub status: TaskStatus,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub completed_at: Option<NaiveDateTime>,
    pub deadline: NaiveDate,
    base_dir: PathBuf,
}

impl Task {
    fn tomorrow_deadline() -> NaiveDate {
        Local::now()
            .date_naive()
            .checked_add_days(Days::new(1))
            .expect("tomorrow should be a valid date")
    }

    fn parse_deadline(value: &str) -> io::Result<(NaiveDate, bool)> {
        if let Ok(deadline) = NaiveDate::parse_from_str(value, DEADLINE_DATE_FORMAT) {
            return Ok((deadline, false));
        }
        NaiveDate::parse_from_str(value, LEGACY_DEADLINE_DATE_FORMAT)
            .map(|deadline| (deadline, true))
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }

    fn parse_datetime(value: &str) -> io::Result<(NaiveDateTime, bool)> {
        if let Ok(datetime) = NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S%.f") {
            return Ok((datetime, false));
        }
        DateTime::parse_from_rfc3339(value)
            .map(|datetime| (datetime.naive_local(), true))
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }

    pub fn week_start(date: NaiveDate) -> NaiveDate {
        date.checked_sub_days(Days::new(date.weekday().num_days_from_monday().into()))
            .expect("week start should be a valid date")
    }

    /// Creates a new task with the given name and TODO status.
    pub fn new(name: String) -> Self {
        Self::new_in(name, Self::default_base_dir())
    }

    /// Creates a new task under the provided task storage directory.
    pub fn new_in(name: String, base_dir: PathBuf) -> Self {
        let now = Local::now().naive_local();
        Self {
            id: Uuid::new_v4(),
            name,
            status: TaskStatus::Todo,
            created_at: now,
            updated_at: now,
            completed_at: None,
            deadline: Self::tomorrow_deadline(),
            base_dir,
        }
    }

    /// Returns the base directory for all task files (`~/.rem-cli/tasks/`).
    pub fn default_base_dir() -> PathBuf {
        dirs::home_dir().unwrap().join(".rem-cli/tasks")
    }

    /// Returns the directory path for a given status (e.g. `~/.rem-cli/tasks/todo/`).
    fn status_dir(base_dir: &Path, status: TaskStatus) -> PathBuf {
        base_dir.join(status.dir_name())
    }

    /// Returns the full file path for this task's markdown file.
    pub fn file_path(&self) -> PathBuf {
        Self::status_dir(&self.base_dir, self.status).join(format!("{}.md", self.id))
    }

    /// Converts this task into a `TaskFrontmatter` for serialization.
    fn frontmatter(&self) -> TaskFrontmatter {
        TaskFrontmatter {
            id: self.id,
            name: self.name.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
            completed_at: self.completed_at,
            deadline: Some(self.deadline.format(DEADLINE_DATE_FORMAT).to_string()),
        }
    }

    /// Saves this task as a markdown file with YAML frontmatter to the appropriate status directory.
    pub fn save(&self) -> io::Result<()> {
        let path = self.file_path();
        fs::create_dir_all(path.parent().unwrap())?;
        let yaml = serde_yaml::to_string(&self.frontmatter()).map_err(io::Error::other)?;
        let content = format!("---\n{}---\n", yaml);
        fs::write(path, content)
    }

    /// Loads a task from a markdown file, assigning the given status based on its directory.
    fn load(path: &PathBuf, status: TaskStatus) -> io::Result<Self> {
        let content = fs::read_to_string(path)?;
        let yaml = content
            .trim_start_matches("---\n")
            .split("---")
            .next()
            .unwrap_or("");
        let fm: StoredTaskFrontmatter = serde_yaml::from_str(yaml)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let (created_at, created_at_needs_migration) = Self::parse_datetime(&fm.created_at)?;
        let (updated_at, updated_at_needs_migration) = Self::parse_datetime(&fm.updated_at)?;
        let parsed_completed_at = fm
            .completed_at
            .as_deref()
            .map(Self::parse_datetime)
            .transpose()?;
        let completed_at_needs_migration = parsed_completed_at
            .as_ref()
            .is_some_and(|(_, needs_migration)| *needs_migration);
        let completed_at = parsed_completed_at.map(|(datetime, _)| datetime);
        let parsed_deadline = fm
            .deadline
            .as_deref()
            .map(Self::parse_deadline)
            .transpose()?;
        let (deadline, deadline_needs_migration) =
            parsed_deadline.unwrap_or_else(|| (Self::tomorrow_deadline(), true));
        let completed_at = completed_at.or((status == TaskStatus::Done).then_some(updated_at));
        let needs_migration = created_at_needs_migration
            || updated_at_needs_migration
            || completed_at_needs_migration
            || deadline_needs_migration
            || (status == TaskStatus::Done && fm.completed_at.is_none());
        let task = Self {
            id: fm.id,
            name: fm.name,
            status,
            created_at,
            updated_at,
            completed_at,
            deadline,
            base_dir: path
                .parent()
                .and_then(Path::parent)
                .map(Path::to_path_buf)
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid task path"))?,
        };
        if needs_migration {
            let migrated = task.content_with_frontmatter(&content, task.frontmatter())?;
            Self::replace_file_content(path, &migrated, "md.migrate")?;
        }
        Ok(task)
    }

    /// Reloads this task's metadata from its markdown file on disk.
    pub fn reload(&self) -> io::Result<Self> {
        Self::load(&self.file_path(), self.status)
    }

    /// Loads all tasks from the `parking/` directory.
    pub fn load_parking() -> io::Result<Vec<Self>> {
        Self::load_parking_from(&Self::default_base_dir())
    }

    /// Loads all tasks from the `parking/` directory under the provided base directory.
    pub fn load_parking_from(base_dir: &Path) -> io::Result<Vec<Self>> {
        Self::load_by_status(base_dir, &[TaskStatus::Parking])
    }

    /// Loads all tasks from the `todo/` directory.
    pub fn load_todo() -> io::Result<Vec<Self>> {
        Self::load_todo_from(&Self::default_base_dir())
    }

    /// Loads all tasks from the `todo/` directory under the provided base directory.
    pub fn load_todo_from(base_dir: &Path) -> io::Result<Vec<Self>> {
        Self::load_by_status(base_dir, &[TaskStatus::Todo])
    }

    /// Loads all tasks from the `doing/` directory.
    pub fn load_doing() -> io::Result<Vec<Self>> {
        Self::load_doing_from(&Self::default_base_dir())
    }

    /// Loads all tasks from the `doing/` directory under the provided base directory.
    pub fn load_doing_from(base_dir: &Path) -> io::Result<Vec<Self>> {
        Self::load_by_status(base_dir, &[TaskStatus::Doing])
    }

    /// Loads all tasks from the `done/` directory.
    pub fn load_done() -> io::Result<Vec<Self>> {
        Self::load_done_from(&Self::default_base_dir())
    }

    /// Loads all tasks from the `done/` directory under the provided base directory.
    pub fn load_done_from(base_dir: &Path) -> io::Result<Vec<Self>> {
        Self::load_by_status(base_dir, &[TaskStatus::Done])
    }

    pub fn load_done_for_week_from(
        base_dir: &Path,
        week_start: NaiveDate,
    ) -> io::Result<Vec<Self>> {
        let week_end = week_start
            .checked_add_days(Days::new(7))
            .expect("week end should be a valid date");
        Self::load_done_from(base_dir).map(|tasks| {
            tasks
                .into_iter()
                .filter(|task| {
                    task.completed_at.is_some_and(|completed_at| {
                        let completed_date = completed_at.date();
                        completed_date >= week_start && completed_date < week_end
                    })
                })
                .collect()
        })
    }

    /// Loads tasks from the directories corresponding to the given statuses, sorted by `created_at`.
    fn load_by_status(base_dir: &Path, statuses: &[TaskStatus]) -> io::Result<Vec<Self>> {
        let mut tasks = Vec::new();
        for status in statuses {
            let dir = Self::status_dir(base_dir, *status);
            if !dir.exists() {
                continue;
            }
            for entry in fs::read_dir(&dir)? {
                let path = entry?.path();
                if path.extension().is_some_and(|e| e == "md") {
                    tasks.push(Self::load(&path, *status).map_err(|error| {
                        io::Error::new(
                            error.kind(),
                            format!("failed to load {}: {error}", path.display()),
                        )
                    })?);
                }
            }
        }
        tasks.sort_by_key(|task| task.created_at);
        Ok(tasks)
    }

    /// Changes this task's status and moves the file to the corresponding directory.
    pub fn update_status(&mut self, new_status: TaskStatus) -> io::Result<()> {
        let old_path = self.file_path();
        let new_path = Self::status_dir(&self.base_dir, new_status).join(format!("{}.md", self.id));
        let updated_at = Local::now().naive_local();
        let completed_at = match (self.status, new_status) {
            (TaskStatus::Doing, TaskStatus::Done) => Some(updated_at),
            (TaskStatus::Done, TaskStatus::Doing) => None,
            _ => self.completed_at,
        };
        let existing = fs::read_to_string(&old_path)?;
        let content = self.content_with_updated_frontmatter(&existing, updated_at, completed_at)?;
        fs::create_dir_all(new_path.parent().unwrap())?;
        Self::replace_file_content(&old_path, &content, "md.update")?;
        if let Err(move_error) = fs::rename(&old_path, &new_path) {
            let rollback_path = old_path.with_extension("md.rollback");
            let rollback_result = fs::write(&rollback_path, existing)
                .and_then(|()| fs::rename(&rollback_path, &old_path));
            return match rollback_result {
                Ok(()) => Err(move_error),
                Err(rollback_error) => Err(io::Error::new(
                    move_error.kind(),
                    format!("{move_error}; failed to restore original file: {rollback_error}"),
                )),
            };
        }
        self.status = new_status;
        self.updated_at = updated_at;
        self.completed_at = completed_at;
        Ok(())
    }

    /// Builds updated file content while preserving the markdown body.
    fn content_with_updated_frontmatter(
        &self,
        existing: &str,
        updated_at: NaiveDateTime,
        completed_at: Option<NaiveDateTime>,
    ) -> io::Result<String> {
        let frontmatter = TaskFrontmatter {
            updated_at,
            completed_at,
            ..self.frontmatter()
        };
        self.content_with_frontmatter(existing, frontmatter)
    }

    /// Builds file content with the provided frontmatter while preserving the markdown body.
    fn content_with_frontmatter(
        &self,
        existing: &str,
        frontmatter: TaskFrontmatter,
    ) -> io::Result<String> {
        let yaml = serde_yaml::to_string(&frontmatter).map_err(io::Error::other)?;
        let body = existing
            .strip_prefix("---\n")
            .and_then(|s| s.find("\n---\n").map(|pos| &s[pos + 5..]))
            .unwrap_or("");
        Ok(format!("---\n{}---\n{}", yaml, body))
    }

    /// Replaces a task file through a temporary file to avoid partial writes.
    fn replace_file_content(
        path: &Path,
        content: &str,
        temporary_extension: &str,
    ) -> io::Result<()> {
        let temporary_path = path.with_extension(temporary_extension);
        fs::write(&temporary_path, content)?;
        if let Err(error) = fs::rename(&temporary_path, path) {
            let cleanup_result = fs::remove_file(&temporary_path);
            return match cleanup_result {
                Ok(()) => Err(error),
                Err(cleanup_error) => Err(io::Error::new(
                    error.kind(),
                    format!("{error}; failed to remove temporary file: {cleanup_error}"),
                )),
            };
        }
        Ok(())
    }

    /// Sorts tasks by status group and by `created_at` within each group.
    pub fn sort(tasks: Vec<Task>) -> Vec<Task> {
        let mut parking = Self::filter_by_status(&tasks, TaskStatus::Parking);
        let mut todos = Self::filter_by_status(&tasks, TaskStatus::Todo);
        let mut doings = Self::filter_by_status(&tasks, TaskStatus::Doing);
        let mut dones = Self::filter_by_status(&tasks, TaskStatus::Done);
        parking.sort_by_key(|task| task.created_at);
        todos.sort_by_key(|task| task.created_at);
        doings.sort_by_key(|task| task.created_at);
        dones.sort_by_key(|task| task.created_at);
        [parking, todos, doings, dones].concat()
    }

    /// Filters tasks by the given status, returning cloned copies.
    fn filter_by_status(tasks: &[Task], status: TaskStatus) -> Vec<Task> {
        tasks
            .iter()
            .filter(|t| t.status == status)
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    fn temporary_tasks_dir() -> PathBuf {
        std::env::temp_dir().join(format!("rem-cli-task-test-{}", Uuid::new_v4()))
    }

    #[test]
    fn file_path_contains_status_dir_and_uuid() {
        // GIVEN: a task with TODO status
        let task = Task::new("path test".to_string());

        // WHEN: file_path is called
        let path = task.file_path();

        // THEN: the path contains /todo/ and ends with <uuid>.md
        assert!(path.to_str().unwrap().contains("/todo/"));
        assert!(path.to_str().unwrap().ends_with(&format!("{}.md", task.id)));
    }

    #[test]
    fn frontmatter_excludes_status() {
        // GIVEN: a task
        let task = Task::new("frontmatter test".to_string());

        // WHEN: frontmatter is serialized to YAML
        let fm = task.frontmatter();
        let yaml = serde_yaml::to_string(&fm).unwrap();

        // THEN: it contains id, name, timestamps but not status
        assert_eq!(fm.id, task.id);
        assert_eq!(fm.name, "frontmatter test");
        assert_eq!(
            fm.deadline,
            Some(task.deadline.format(DEADLINE_DATE_FORMAT).to_string())
        );
        assert!(!yaml.contains("status"));
    }

    #[test]
    fn new_task_has_tomorrow_as_deadline() {
        // GIVEN
        let expected = Task::tomorrow_deadline();

        // WHEN
        let task = Task::new("deadline test".to_string());

        // THEN
        assert_eq!(task.deadline, expected);
    }

    #[test]
    fn new_task_uses_local_naive_datetime() {
        // GIVEN
        let before = Local::now().naive_local();

        // WHEN
        let task = Task::new("local datetime test".to_string());

        // THEN
        let after = Local::now().naive_local();
        assert!(task.created_at >= before);
        assert!(task.created_at <= after);
        assert_eq!(task.updated_at, task.created_at);
        assert_eq!(task.completed_at, None);
    }

    #[test]
    fn week_start_returns_monday_across_calendar_boundaries() {
        // GIVEN
        let dates = [
            NaiveDate::from_ymd_opt(2026, 6, 15).unwrap(),
            NaiveDate::from_ymd_opt(2026, 6, 21).unwrap(),
            NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
        ];
        let expected = [
            NaiveDate::from_ymd_opt(2026, 6, 15).unwrap(),
            NaiveDate::from_ymd_opt(2026, 6, 15).unwrap(),
            NaiveDate::from_ymd_opt(2025, 12, 29).unwrap(),
        ];

        // WHEN
        let actual = dates.map(Task::week_start);

        // THEN
        assert_eq!(actual, expected);
    }

    #[test]
    fn sort_groups_by_status_and_orders_by_created_at() {
        // GIVEN: tasks with mixed statuses created in different order
        let mut task_parking = Task::new("parking".to_string());
        task_parking.status = TaskStatus::Parking;
        thread::sleep(Duration::from_millis(10));
        let mut task_doing = Task::new("doing".to_string());
        task_doing.status = TaskStatus::Doing;
        thread::sleep(Duration::from_millis(10));
        let task_todo = Task::new("todo".to_string());
        thread::sleep(Duration::from_millis(10));
        let mut task_done = Task::new("done".to_string());
        task_done.status = TaskStatus::Done;

        // WHEN: sort is called
        let sorted = Task::sort(vec![task_done, task_doing, task_todo, task_parking]);

        // THEN: tasks are grouped by status (PARKING, TODO, DOING, DONE)
        assert_eq!(sorted[0].status, TaskStatus::Parking);
        assert_eq!(sorted[1].status, TaskStatus::Todo);
        assert_eq!(sorted[2].status, TaskStatus::Doing);
        assert_eq!(sorted[3].status, TaskStatus::Done);
    }

    #[test]
    fn parking_file_path_contains_parking_directory() {
        // GIVEN
        let mut task = Task::new("parking path test".to_string());
        task.status = TaskStatus::Parking;

        // WHEN
        let path = task.file_path();

        // THEN
        assert!(path.to_str().unwrap().contains("/parking/"));
        assert!(path.to_str().unwrap().ends_with(&format!("{}.md", task.id)));
    }

    #[test]
    fn filter_by_status_returns_matching_tasks() {
        // GIVEN: tasks with mixed statuses
        let todo = Task::new("todo".to_string());
        let mut doing = Task::new("doing".to_string());
        doing.status = TaskStatus::Doing;
        let tasks = vec![todo, doing];

        // WHEN: filter_by_status is called with Todo
        let filtered = Task::filter_by_status(&tasks, TaskStatus::Todo);

        // THEN: only TODO tasks are returned
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "todo");
    }

    #[test]
    fn save_and_load_roundtrip() {
        // GIVEN: a task saved to disk
        let tasks_dir = temporary_tasks_dir();
        let task = Task::new_in("roundtrip test".to_string(), tasks_dir.clone());
        task.save().unwrap();

        // WHEN: Task::load is called with the file path
        let loaded = Task::load(&task.file_path(), TaskStatus::Todo).unwrap();

        // THEN: the loaded task matches the original
        assert_eq!(loaded.id, task.id);
        assert_eq!(loaded.name, "roundtrip test");
        assert_eq!(loaded.status, TaskStatus::Todo);
        assert_eq!(loaded.deadline, task.deadline);
        let content = fs::read_to_string(task.file_path()).unwrap();
        assert!(content.contains(&format!(
            "deadline: {}",
            task.deadline.format(DEADLINE_DATE_FORMAT)
        )));

        fs::remove_dir_all(tasks_dir).unwrap();
    }

    #[test]
    fn load_accepts_legacy_deadline_format() {
        // GIVEN
        let tasks_dir = temporary_tasks_dir();
        let task = Task::new_in("legacy deadline format".to_string(), tasks_dir.clone());
        task.save().unwrap();
        let path = task.file_path();
        let content = fs::read_to_string(&path).unwrap().replace(
            &task.deadline.format(DEADLINE_DATE_FORMAT).to_string(),
            &task
                .deadline
                .format(LEGACY_DEADLINE_DATE_FORMAT)
                .to_string(),
        );
        fs::write(&path, content).unwrap();

        // WHEN
        let loaded = Task::load(&path, TaskStatus::Todo).unwrap();

        // THEN
        assert_eq!(loaded.deadline, task.deadline);
        let migrated_content = fs::read_to_string(&path).unwrap();
        assert!(migrated_content.contains(&format!(
            "deadline: {}",
            task.deadline.format(DEADLINE_DATE_FORMAT)
        )));
        assert!(!migrated_content.contains(&format!(
            "deadline: {}",
            task.deadline.format(LEGACY_DEADLINE_DATE_FORMAT)
        )));

        fs::remove_dir_all(tasks_dir).unwrap();
    }

    #[test]
    fn load_adds_missing_deadline_without_changing_existing_content() {
        // GIVEN
        let tasks_dir = temporary_tasks_dir();
        let task = Task::new_in("legacy task".to_string(), tasks_dir.clone());
        task.save().unwrap();
        let path = task.file_path();
        let content = fs::read_to_string(&path).unwrap();
        let legacy_content = format!(
            "{}## Notes\n\nlegacy body\n",
            content
                .lines()
                .filter(|line| !line.starts_with("deadline:"))
                .collect::<Vec<_>>()
                .join("\n")
                + "\n"
        );
        fs::write(&path, &legacy_content).unwrap();
        let expected_deadline = Task::tomorrow_deadline();

        // WHEN
        let loaded = Task::load(&path, TaskStatus::Todo).unwrap();

        // THEN
        let migrated_content = fs::read_to_string(&path).unwrap();
        assert_eq!(loaded.deadline, expected_deadline);
        assert_eq!(loaded.created_at, task.created_at);
        assert_eq!(loaded.updated_at, task.updated_at);
        assert!(migrated_content.contains(&format!(
            "deadline: {}",
            expected_deadline.format(DEADLINE_DATE_FORMAT)
        )));
        assert!(migrated_content.ends_with("## Notes\n\nlegacy body\n"));

        fs::remove_dir_all(tasks_dir).unwrap();
    }

    #[test]
    fn load_migrates_utc_timestamps_without_changing_clock_time() {
        // GIVEN
        let tasks_dir = temporary_tasks_dir();
        let todo_dir = tasks_dir.join("todo");
        fs::create_dir_all(&todo_dir).unwrap();
        let id = Uuid::new_v4();
        let path = todo_dir.join(format!("{id}.md"));
        let content = format!(
            "---\nid: {id}\nname: legacy utc\ncreated_at: 2026-06-15T10:00:00Z\nupdated_at: 2026-06-15T11:30:00+09:00\ndeadline: 2026/06/16\n---\n## Notes\n"
        );
        fs::write(&path, content).unwrap();
        let expected_created_at = NaiveDate::from_ymd_opt(2026, 6, 15)
            .unwrap()
            .and_hms_opt(10, 0, 0)
            .unwrap();
        let expected_updated_at = NaiveDate::from_ymd_opt(2026, 6, 15)
            .unwrap()
            .and_hms_opt(11, 30, 0)
            .unwrap();

        // WHEN
        let task = Task::load(&path, TaskStatus::Todo).unwrap();

        // THEN
        let migrated_content = fs::read_to_string(&path).unwrap();
        assert_eq!(task.created_at, expected_created_at);
        assert_eq!(task.updated_at, expected_updated_at);
        assert!(!migrated_content.contains('Z'));
        assert!(!migrated_content.contains("+09:00"));
        assert!(migrated_content.ends_with("## Notes\n"));

        fs::remove_dir_all(tasks_dir).unwrap();
    }

    #[test]
    fn load_done_uses_updated_at_for_missing_completed_at() {
        // GIVEN
        let tasks_dir = temporary_tasks_dir();
        let todo_dir = tasks_dir.join("todo");
        let done_dir = tasks_dir.join("done");
        let task = Task::new_in("legacy done".to_string(), tasks_dir.clone());
        task.save().unwrap();
        fs::create_dir_all(&done_dir).unwrap();
        let done_path = done_dir.join(format!("{}.md", task.id));
        fs::rename(task.file_path(), &done_path).unwrap();
        let expected = task.updated_at;

        // WHEN
        let loaded = Task::load(&done_path, TaskStatus::Done).unwrap();

        // THEN
        let migrated_content = fs::read_to_string(&done_path).unwrap();
        assert_eq!(loaded.completed_at, Some(expected));
        assert!(migrated_content.contains("completed_at:"));
        assert!(!todo_dir.join(format!("{}.md", task.id)).exists());

        fs::remove_dir_all(tasks_dir).unwrap();
    }

    #[test]
    fn failed_deadline_migration_preserves_original_content() {
        // GIVEN
        let tasks_dir = temporary_tasks_dir();
        let task = Task::new_in("failed migration".to_string(), tasks_dir.clone());
        task.save().unwrap();
        let path = task.file_path();
        let original_content = fs::read_to_string(&path)
            .unwrap()
            .lines()
            .filter(|line| !line.starts_with("deadline:"))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        fs::write(&path, &original_content).unwrap();
        fs::create_dir(path.with_extension("md.migrate")).unwrap();

        // WHEN
        let result = Task::load(&path, TaskStatus::Todo);

        // THEN
        assert!(result.is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), original_content);

        fs::remove_dir_all(tasks_dir).unwrap();
    }

    #[test]
    fn load_returns_error_for_invalid_deadline() {
        // GIVEN
        let tasks_dir = temporary_tasks_dir();
        let task = Task::new_in("invalid deadline".to_string(), tasks_dir.clone());
        task.save().unwrap();
        let path = task.file_path();
        let content = fs::read_to_string(&path).unwrap().replace(
            &task.deadline.format(DEADLINE_DATE_FORMAT).to_string(),
            "2026/02/30",
        );
        fs::write(&path, content).unwrap();

        // WHEN
        let result = Task::load(&path, TaskStatus::Todo);

        // THEN
        assert!(result.is_err());

        fs::remove_dir_all(tasks_dir).unwrap();
    }

    #[test]
    fn update_status_moves_file_between_directories() {
        // GIVEN: a saved task with TODO status and a markdown body appended to the file
        let tasks_dir = temporary_tasks_dir();
        let mut task = Task::new_in("status move test".to_string(), tasks_dir.clone());
        task.save().unwrap();
        let body = "## Notes\n\nsome content here\n";
        let existing = fs::read_to_string(task.file_path()).unwrap();
        fs::write(task.file_path(), format!("{}{}", existing, body)).unwrap();
        let old_path = task.file_path();
        assert!(old_path.exists());

        // WHEN: update_status is called with Doing
        thread::sleep(Duration::from_millis(10));
        let before_update = task.updated_at;
        task.update_status(TaskStatus::Doing).unwrap();

        // THEN: the file is moved to doing/ directory, updated_at is refreshed, and body content is preserved
        assert!(!old_path.exists());
        assert!(task.file_path().exists());
        assert!(task.file_path().to_str().unwrap().contains("/doing/"));
        assert!(task.updated_at > before_update);
        let content = fs::read_to_string(task.file_path()).unwrap();
        assert!(content.contains(body));
        assert!(content.contains(&format!(
            "deadline: {}",
            task.deadline.format(DEADLINE_DATE_FORMAT)
        )));

        fs::remove_dir_all(tasks_dir).unwrap();
    }

    #[test]
    fn update_status_failure_keeps_original_state() {
        // GIVEN
        let tasks_dir = temporary_tasks_dir();
        let mut task = Task::new_in("failed status move".to_string(), tasks_dir);
        let expected_status = task.status;
        let expected_updated_at = task.updated_at;

        // WHEN
        let result = task.update_status(TaskStatus::Doing);

        // THEN
        assert!(result.is_err());
        assert_eq!(task.status, expected_status);
        assert_eq!(task.updated_at, expected_updated_at);
    }

    #[test]
    fn moving_doing_task_to_done_sets_completed_at() {
        // GIVEN
        let tasks_dir = temporary_tasks_dir();
        let mut task = Task::new_in("complete task".to_string(), tasks_dir.clone());
        task.save().unwrap();
        task.update_status(TaskStatus::Doing).unwrap();

        // WHEN
        task.update_status(TaskStatus::Done).unwrap();

        // THEN
        assert_eq!(task.completed_at, Some(task.updated_at));

        fs::remove_dir_all(tasks_dir).unwrap();
    }

    #[test]
    fn reopening_done_task_clears_completed_at() {
        // GIVEN
        let tasks_dir = temporary_tasks_dir();
        let mut task = Task::new_in("reopen task".to_string(), tasks_dir.clone());
        task.save().unwrap();
        task.update_status(TaskStatus::Doing).unwrap();
        task.update_status(TaskStatus::Done).unwrap();

        // WHEN
        task.update_status(TaskStatus::Doing).unwrap();

        // THEN
        assert_eq!(task.completed_at, None);

        fs::remove_dir_all(tasks_dir).unwrap();
    }

    #[test]
    fn completing_reopened_task_sets_new_completed_at() {
        // GIVEN
        let tasks_dir = temporary_tasks_dir();
        let mut task = Task::new_in("complete again".to_string(), tasks_dir.clone());
        task.save().unwrap();
        task.update_status(TaskStatus::Doing).unwrap();
        task.update_status(TaskStatus::Done).unwrap();
        let first_completed_at = task.completed_at;
        task.update_status(TaskStatus::Doing).unwrap();

        // WHEN
        task.update_status(TaskStatus::Done).unwrap();

        // THEN
        assert!(task.completed_at.is_some());
        assert!(task.completed_at >= first_completed_at);

        fs::remove_dir_all(tasks_dir).unwrap();
    }

    #[test]
    fn load_done_for_week_includes_monday_through_sunday() {
        // GIVEN
        let tasks_dir = temporary_tasks_dir();
        let week_start = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        let completed_dates = [
            ("monday", NaiveDate::from_ymd_opt(2026, 6, 15).unwrap()),
            ("sunday", NaiveDate::from_ymd_opt(2026, 6, 21).unwrap()),
            ("next monday", NaiveDate::from_ymd_opt(2026, 6, 22).unwrap()),
        ];
        completed_dates
            .into_iter()
            .map(|(name, date)| {
                let mut task = Task::new_in(name.to_string(), tasks_dir.clone());
                task.status = TaskStatus::Done;
                task.completed_at = date.and_hms_opt(12, 0, 0);
                task
            })
            .try_for_each(|task| task.save())
            .unwrap();
        let expected = ["monday", "sunday"];

        // WHEN
        let tasks = Task::load_done_for_week_from(&tasks_dir, week_start).unwrap();

        // THEN
        let actual = tasks
            .iter()
            .map(|task| task.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);

        fs::remove_dir_all(tasks_dir).unwrap();
    }

    #[test]
    fn update_status_move_failure_restores_original_content() {
        // GIVEN
        let tasks_dir = temporary_tasks_dir();
        let mut task = Task::new_in("rollback status move".to_string(), tasks_dir.clone());
        task.save().unwrap();
        let original_path = task.file_path();
        let original_content = fs::read_to_string(&original_path).unwrap();
        let conflicting_path = tasks_dir.join("doing").join(format!("{}.md", task.id));
        fs::create_dir_all(&conflicting_path).unwrap();

        // WHEN
        let result = task.update_status(TaskStatus::Doing);

        // THEN
        assert!(result.is_err());
        assert_eq!(task.status, TaskStatus::Todo);
        assert_eq!(fs::read_to_string(original_path).unwrap(), original_content);

        fs::remove_dir_all(tasks_dir).unwrap();
    }

    #[test]
    fn load_by_status_returns_error_for_invalid_task_file() {
        // GIVEN
        let tasks_dir = temporary_tasks_dir();
        let todo_dir = tasks_dir.join("todo");
        fs::create_dir_all(&todo_dir).unwrap();
        let invalid_path = todo_dir.join("invalid.md");
        fs::write(&invalid_path, "invalid frontmatter").unwrap();

        // WHEN
        let result = Task::load_todo_from(&tasks_dir);

        // THEN
        let error = result.err().expect("invalid task file should fail loading");
        assert!(error.to_string().contains(invalid_path.to_str().unwrap()));

        fs::remove_dir_all(tasks_dir).unwrap();
    }
}
