use std::fs;
use std::io;
use std::path::PathBuf;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Represents the lifecycle status of a task.
#[derive(Clone, Debug, PartialEq)]
pub enum TaskStatus {
    Todo,
    Doing,
    Done,
}

impl TaskStatus {
    /// Returns the directory name corresponding to this status (e.g. `"todo"`, `"doing"`, `"done"`).
    fn dir_name(&self) -> &str {
        match self {
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
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

/// A TODO task with metadata and lifecycle status.
#[derive(Clone)]
pub struct Task {
    pub id: Uuid,
    pub name: String,
    pub status: TaskStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Task {
    /// Creates a new task with the given name and TODO status.
    pub fn new(name: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name,
            status: TaskStatus::Todo,
            created_at: now,
            updated_at: now,
        }
    }

    /// Returns the base directory for all task files (`~/.rem-cli/tasks/`).
    fn base_dir() -> PathBuf {
        dirs::home_dir().unwrap().join(".rem-cli/tasks")
    }

    /// Returns the directory path for a given status (e.g. `~/.rem-cli/tasks/todo/`).
    fn status_dir(status: &TaskStatus) -> PathBuf {
        Self::base_dir().join(status.dir_name())
    }

    /// Returns the full file path for this task's markdown file.
    pub fn file_path(&self) -> PathBuf {
        Self::status_dir(&self.status).join(format!("{}.md", self.id))
    }

    /// Converts this task into a `TaskFrontmatter` for serialization.
    fn frontmatter(&self) -> TaskFrontmatter {
        TaskFrontmatter {
            id: self.id,
            name: self.name.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
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
        let fm: TaskFrontmatter =
            serde_yaml::from_str(yaml).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        Ok(Self {
            id: fm.id,
            name: fm.name,
            status,
            created_at: fm.created_at,
            updated_at: fm.updated_at,
        })
    }

    /// Loads all tasks from the `todo/` directory.
    pub fn load_todo() -> io::Result<Vec<Self>> {
        Self::load_by_status(&[TaskStatus::Todo])
    }

    /// Loads all tasks from the `doing/` directory.
    pub fn load_doing() -> io::Result<Vec<Self>> {
        Self::load_by_status(&[TaskStatus::Doing])
    }

    /// Loads all tasks from the `done/` directory.
    pub fn load_done() -> io::Result<Vec<Self>> {
        Self::load_by_status(&[TaskStatus::Done])
    }

    /// Loads tasks from the directories corresponding to the given statuses, sorted by `created_at`.
    fn load_by_status(statuses: &[TaskStatus]) -> io::Result<Vec<Self>> {
        let mut tasks = Vec::new();
        for status in statuses {
            let dir = Self::status_dir(status);
            if !dir.exists() {
                continue;
            }
            for entry in fs::read_dir(&dir)? {
                let path = entry?.path();
                if path.extension().is_some_and(|e| e == "md") {
                    if let Ok(task) = Self::load(&path, status.clone()) {
                        tasks.push(task);
                    }
                }
            }
        }
        tasks.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        Ok(tasks)
    }

    /// Changes this task's status and moves the file to the corresponding directory.
    pub fn update_status(&mut self, new_status: TaskStatus) {
        let old_path = self.file_path();
        self.status = new_status;
        self.updated_at = Utc::now();
        let _ = fs::create_dir_all(Self::status_dir(&self.status));
        let _ = fs::rename(old_path, self.file_path());
        let _ = self.save();
    }

    /// Sorts tasks by status group (TODO, DOING, DONE) and by `created_at` within each group.
    pub fn sort(tasks: Vec<Task>) -> Vec<Task> {
        let mut todos = Self::filter_by_status(&tasks, TaskStatus::Todo);
        let mut doings = Self::filter_by_status(&tasks, TaskStatus::Doing);
        let mut dones = Self::filter_by_status(&tasks, TaskStatus::Done);
        todos.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        doings.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        dones.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        [todos, doings, dones].concat()
    }

    /// Filters tasks by the given status, returning cloned copies.
    fn filter_by_status(tasks: &[Task], status: TaskStatus) -> Vec<Task> {
        tasks.iter()
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
        assert!(!yaml.contains("status"));
    }

    #[test]
    fn sort_groups_by_status_and_orders_by_created_at() {
        // GIVEN: tasks with mixed statuses created in different order
        let mut task_doing = Task::new("doing".to_string());
        task_doing.status = TaskStatus::Doing;
        thread::sleep(Duration::from_millis(10));
        let task_todo = Task::new("todo".to_string());
        thread::sleep(Duration::from_millis(10));
        let mut task_done = Task::new("done".to_string());
        task_done.status = TaskStatus::Done;

        // WHEN: sort is called
        let sorted = Task::sort(vec![task_done, task_doing, task_todo]);

        // THEN: tasks are grouped by status (TODO, DOING, DONE)
        assert_eq!(sorted[0].status, TaskStatus::Todo);
        assert_eq!(sorted[1].status, TaskStatus::Doing);
        assert_eq!(sorted[2].status, TaskStatus::Done);
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
        let task = Task::new("roundtrip test".to_string());
        task.save().unwrap();

        // WHEN: Task::load is called with the file path
        let loaded = Task::load(&task.file_path(), TaskStatus::Todo).unwrap();

        // THEN: the loaded task matches the original
        assert_eq!(loaded.id, task.id);
        assert_eq!(loaded.name, "roundtrip test");
        assert_eq!(loaded.status, TaskStatus::Todo);

        let _ = fs::remove_file(task.file_path());
    }

    #[test]
    fn update_status_moves_file_between_directories() {
        // GIVEN: a saved task with TODO status
        let mut task = Task::new("status move test".to_string());
        task.save().unwrap();
        let old_path = task.file_path();
        assert!(old_path.exists());

        // WHEN: update_status is called with Doing
        thread::sleep(Duration::from_millis(10));
        let before_update = task.updated_at;
        task.update_status(TaskStatus::Doing);

        // THEN: the file is moved to doing/ directory and updated_at is refreshed
        assert!(!old_path.exists());
        assert!(task.file_path().exists());
        assert!(task.file_path().to_str().unwrap().contains("/doing/"));
        assert!(task.updated_at > before_update);

        let _ = fs::remove_file(task.file_path());
    }
}
