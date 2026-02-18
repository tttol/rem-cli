use std::fs;
use std::io;
use std::path::PathBuf;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Represents the lifecycle status of a task.
#[derive(Clone, PartialEq)]
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
