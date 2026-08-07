use crate::application::TaskRepository;
use crate::domain::task::{DEADLINE_DATE_FORMAT, Task, TaskStatus};
use chrono::{DateTime, Days, Local, NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const LEGACY_DEADLINE_DATE_FORMAT: &str = "%Y-%m-%d";

#[derive(Clone, Serialize)]
struct TaskFrontmatter {
    id: Uuid,
    name: String,
    created_at: NaiveDateTime,
    updated_at: NaiveDateTime,
    #[serde(skip_serializing_if = "Option::is_none")]
    completed_at: Option<NaiveDateTime>,
    deadline: String,
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

pub(crate) struct FileTaskRepository {
    base_dir: PathBuf,
}

impl FileTaskRepository {
    pub(crate) fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    fn status_dir(&self, status: TaskStatus) -> PathBuf {
        let directory = match status {
            TaskStatus::Parking => "parking",
            TaskStatus::Todo => "todo",
            TaskStatus::Doing => "doing",
            TaskStatus::Done => "done",
        };
        self.base_dir.join(directory)
    }

    fn load_path(&self, path: &Path, status: TaskStatus) -> io::Result<Task> {
        let content = fs::read_to_string(path)?;
        let yaml = content
            .strip_prefix("---\n")
            .and_then(|document| document.split("---").next())
            .unwrap_or("");
        let stored: StoredTaskFrontmatter = serde_yaml::from_str(yaml)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        let (created_at, created_at_needs_migration) = Self::parse_datetime(&stored.created_at)?;
        let (updated_at, updated_at_needs_migration) = Self::parse_datetime(&stored.updated_at)?;
        let parsed_completed_at = stored
            .completed_at
            .as_deref()
            .map(Self::parse_datetime)
            .transpose()?;
        let completed_at_needs_migration = parsed_completed_at
            .as_ref()
            .is_some_and(|(_, needs_migration)| *needs_migration);
        let completed_at = parsed_completed_at.map(|(datetime, _)| datetime);
        let parsed_deadline = stored
            .deadline
            .as_deref()
            .map(Self::parse_deadline)
            .transpose()?;
        let (deadline, deadline_needs_migration) = match parsed_deadline {
            Some(parsed) => parsed,
            None => (Self::tomorrow()?, true),
        };
        let completed_at = completed_at.or((status == TaskStatus::Done).then_some(updated_at));
        let needs_migration = created_at_needs_migration
            || updated_at_needs_migration
            || completed_at_needs_migration
            || deadline_needs_migration
            || (status == TaskStatus::Done && stored.completed_at.is_none());
        let task = Task::from_stored(
            stored.id,
            stored.name,
            status,
            created_at,
            updated_at,
            completed_at,
            deadline,
        );
        if needs_migration {
            let migrated = Self::content_with_frontmatter(&content, Self::frontmatter(&task))?;
            Self::replace_file_content(path, &migrated, "md.migrate")?;
        }
        Ok(task)
    }

    fn frontmatter(task: &Task) -> TaskFrontmatter {
        TaskFrontmatter {
            id: task.id(),
            name: task.name().to_string(),
            created_at: task.created_at(),
            updated_at: task.updated_at(),
            completed_at: task.completed_at(),
            deadline: task.deadline().format(DEADLINE_DATE_FORMAT).to_string(),
        }
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

    fn tomorrow() -> io::Result<NaiveDate> {
        Local::now()
            .date_naive()
            .checked_add_days(Days::new(1))
            .ok_or_else(|| io::Error::other("tomorrow is outside the supported date range"))
    }

    fn content_with_frontmatter(
        existing: &str,
        frontmatter: TaskFrontmatter,
    ) -> io::Result<String> {
        let yaml = serde_yaml::to_string(&frontmatter).map_err(io::Error::other)?;
        let body = existing
            .strip_prefix("---\n")
            .and_then(|document| {
                document
                    .find("\n---\n")
                    .map(|position| &document[position + 5..])
            })
            .unwrap_or("");
        Ok(format!("---\n{yaml}---\n{body}"))
    }

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
}

impl TaskRepository for FileTaskRepository {
    fn load(&self, status: TaskStatus) -> io::Result<Vec<Task>> {
        let directory = self.status_dir(status);
        if !directory.exists() {
            return Ok(Vec::new());
        }
        let tasks: io::Result<Vec<Option<Task>>> = fs::read_dir(&directory)?
            .map(|entry_result| {
                let path = entry_result?.path();
                if path.extension().is_none_or(|extension| extension != "md") {
                    return Ok(None);
                }
                self.load_path(&path, status).map(Some).map_err(|error| {
                    io::Error::new(
                        error.kind(),
                        format!("failed to load {}: {error}", path.display()),
                    )
                })
            })
            .collect();
        Ok(Task::sorted(tasks?.into_iter().flatten().collect()))
    }

    fn save(&mut self, task: &Task) -> io::Result<()> {
        let directory = self.status_dir(task.status());
        fs::create_dir_all(&directory)?;
        let yaml = serde_yaml::to_string(&Self::frontmatter(task)).map_err(io::Error::other)?;
        fs::write(self.path(task), format!("---\n{yaml}---\n"))
    }

    fn replace(&mut self, current: &Task, updated: &Task) -> io::Result<()> {
        let old_path = self.path(current);
        let new_directory = self.status_dir(updated.status());
        let new_path = self.path(updated);
        let existing = fs::read_to_string(&old_path)?;
        let content = Self::content_with_frontmatter(&existing, Self::frontmatter(updated))?;
        fs::create_dir_all(new_directory)?;
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
        Ok(())
    }

    fn reload(&self, task: &Task) -> io::Result<Task> {
        self.load_path(&self.path(task), task.status())
    }

    fn path(&self, task: &Task) -> PathBuf {
        self.status_dir(task.status())
            .join(format!("{}.md", task.id()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::task::StatusDirection;

    struct TestDirectory {
        path: PathBuf,
    }

    impl TestDirectory {
        fn new() -> Self {
            Self {
                path: std::env::temp_dir()
                    .join(format!("rem-cli-repository-test-{}", Uuid::new_v4())),
            }
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _cleanup_result = fs::remove_dir_all(&self.path);
        }
    }

    fn datetime(year: i32, month: u32, day: u32, hour: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(year, month, day)
            .and_then(|date| date.and_hms_opt(hour, 0, 0))
            .expect("test datetime should be valid")
    }

    #[test]
    fn save_and_load_preserve_the_complete_task() {
        // GIVEN
        let directory = TestDirectory::new();
        let now = datetime(2026, 6, 15, 9);
        let task = Task::new("roundtrip".to_string(), now).expect("task should be valid");
        let mut repository = FileTaskRepository::new(directory.path.clone());
        repository.save(&task).expect("task should save");

        // WHEN
        let actual = repository
            .load(TaskStatus::Todo)
            .expect("tasks should load");

        // THEN
        let expected = vec![task];
        assert_eq!(actual, expected);
    }

    #[test]
    fn replace_moves_the_file_and_preserves_its_body() {
        // GIVEN
        let directory = TestDirectory::new();
        let now = datetime(2026, 6, 15, 9);
        let current = Task::new("move".to_string(), now).expect("task should be valid");
        let updated = current
            .transitioned(StatusDirection::Forward, datetime(2026, 6, 15, 10))
            .expect("TODO should transition to DOING");
        let mut repository = FileTaskRepository::new(directory.path.clone());
        repository.save(&current).expect("task should save");
        let body = "## Notes\n\nKeep this body.\n";
        let original_path = repository.path(&current);
        let existing = fs::read_to_string(&original_path).expect("task should be readable");
        fs::write(&original_path, format!("{existing}{body}")).expect("body should be appended");

        // WHEN
        repository
            .replace(&current, &updated)
            .expect("status should update");

        // THEN
        let updated_path = repository.path(&updated);
        let actual = fs::read_to_string(&updated_path).expect("updated task should be readable");
        let reloaded = repository
            .load(TaskStatus::Doing)
            .expect("updated task should load");
        assert!(!original_path.exists());
        assert!(updated_path.exists());
        assert!(actual.contains(body));
        assert_eq!(reloaded, vec![updated]);
    }

    #[test]
    fn legacy_frontmatter_is_migrated_without_changing_the_body() {
        // GIVEN
        let directory = TestDirectory::new();
        let todo_directory = directory.path.join("todo");
        let id = Uuid::new_v4();
        let path = todo_directory.join(format!("{id}.md"));
        fs::create_dir_all(&todo_directory).expect("todo directory should be created");
        let body = "## Notes\nKeep this body.\n";
        let content = format!(
            "---\nid: {id}\nname: legacy\ncreated_at: 2026-06-15T09:00:00Z\nupdated_at: 2026-06-15T10:00:00Z\ndeadline: 2026-06-16\n---\n{body}"
        );
        fs::write(&path, content).expect("legacy task should be written");
        let repository = FileTaskRepository::new(directory.path.clone());

        // WHEN
        let actual = repository.load(TaskStatus::Todo).expect("task should load");

        // THEN
        let migrated = fs::read_to_string(path).expect("migrated task should be readable");
        assert_eq!(actual.len(), 1);
        assert_eq!(
            actual[0]
                .deadline()
                .format(DEADLINE_DATE_FORMAT)
                .to_string(),
            "2026/06/16"
        );
        assert!(migrated.contains(body));
        assert!(migrated.contains("deadline: 2026/06/16"));
    }

    #[test]
    fn invalid_task_errors_include_the_file_path() {
        // GIVEN
        let directory = TestDirectory::new();
        let todo_directory = directory.path.join("todo");
        let path = todo_directory.join("invalid.md");
        fs::create_dir_all(&todo_directory).expect("todo directory should be created");
        fs::write(&path, "invalid frontmatter").expect("invalid task should be written");
        let repository = FileTaskRepository::new(directory.path.clone());

        // WHEN
        let actual = repository.load(TaskStatus::Todo);

        // THEN
        let error = actual.expect_err("invalid task should fail");
        assert!(error.to_string().contains(&path.display().to_string()));
    }

    #[test]
    fn missing_deadline_is_added_without_changing_the_body() {
        // GIVEN
        let directory = TestDirectory::new();
        let now = datetime(2026, 6, 15, 9);
        let task = Task::new("missing deadline".to_string(), now).expect("task should be valid");
        let mut repository = FileTaskRepository::new(directory.path.clone());
        repository.save(&task).expect("task should save");
        let path = repository.path(&task);
        let body = "## Notes\nKeep this body.\n";
        let content = fs::read_to_string(&path).expect("task should be readable");
        let without_deadline = content
            .lines()
            .filter(|line| !line.starts_with("deadline:"))
            .chain(body.lines())
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(&path, format!("{without_deadline}\n")).expect("legacy task should be written");

        // WHEN
        let actual = repository.load(TaskStatus::Todo).expect("task should load");

        // THEN
        let migrated = fs::read_to_string(path).expect("migrated task should be readable");
        assert_eq!(actual.len(), 1);
        assert!(migrated.contains("deadline:"));
        assert!(migrated.contains(body));
    }

    #[test]
    fn done_task_without_completed_at_uses_its_updated_at() {
        // GIVEN
        let directory = TestDirectory::new();
        let now = datetime(2026, 6, 15, 9);
        let task = Task::new("legacy done".to_string(), now).expect("task should be valid");
        let expected = task.updated_at();
        let mut repository = FileTaskRepository::new(directory.path.clone());
        repository.save(&task).expect("task should save");
        let todo_path = repository.path(&task);
        let done_directory = directory.path.join("done");
        let done_path = done_directory.join(format!("{}.md", task.id()));
        fs::create_dir_all(&done_directory).expect("done directory should be created");
        fs::rename(todo_path, &done_path).expect("task should move to done");

        // WHEN
        let actual = repository.load(TaskStatus::Done).expect("task should load");

        // THEN
        let migrated = fs::read_to_string(done_path).expect("migrated task should be readable");
        assert_eq!(actual[0].completed_at(), Some(expected));
        assert!(migrated.contains("completed_at:"));
    }

    #[test]
    fn move_failure_restores_the_original_task_content() {
        // GIVEN
        let directory = TestDirectory::new();
        let now = datetime(2026, 6, 15, 9);
        let current = Task::new("rollback".to_string(), now).expect("task should be valid");
        let updated = current
            .transitioned(StatusDirection::Forward, datetime(2026, 6, 15, 10))
            .expect("TODO should transition to DOING");
        let mut repository = FileTaskRepository::new(directory.path.clone());
        repository.save(&current).expect("task should save");
        let original_path = repository.path(&current);
        let original_content = fs::read_to_string(&original_path).expect("task should be readable");
        let conflicting_path = repository.path(&updated);
        fs::create_dir_all(&conflicting_path).expect("conflicting directory should be created");

        // WHEN
        let actual = repository.replace(&current, &updated);

        // THEN
        assert!(actual.is_err());
        assert_eq!(
            fs::read_to_string(original_path).expect("original task should be restored"),
            original_content
        );
    }

    #[test]
    fn invalid_deadline_returns_an_invalid_data_error() {
        // GIVEN
        let directory = TestDirectory::new();
        let now = datetime(2026, 6, 15, 9);
        let task = Task::new("invalid deadline".to_string(), now).expect("task should be valid");
        let mut repository = FileTaskRepository::new(directory.path.clone());
        repository.save(&task).expect("task should save");
        let path = repository.path(&task);
        let content = fs::read_to_string(&path)
            .expect("task should be readable")
            .replace("deadline: 2026/06/16", "deadline: invalid");
        fs::write(path, content).expect("invalid task should be written");

        // WHEN
        let actual = repository.load(TaskStatus::Todo);

        // THEN
        assert_eq!(
            actual.expect_err("invalid deadline should fail").kind(),
            io::ErrorKind::InvalidData
        );
    }
}
