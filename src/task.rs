use std::fs;
use std::io;
use std::path::PathBuf;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, PartialEq)]
pub enum TaskStatus {
    Todo,
    Doing,
    Done,
}

impl TaskStatus {
    fn dir_name(&self) -> &str {
        match self {
            TaskStatus::Todo => "todo",
            TaskStatus::Doing => "doing",
            TaskStatus::Done => "done",
        }
    }

}

#[derive(Clone, Serialize, Deserialize)]
struct TaskFrontmatter {
    id: Uuid,
    name: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct Task {
    pub id: Uuid,
    pub name: String,
    pub status: TaskStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Task {
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

    fn base_dir() -> PathBuf {
        dirs::home_dir().unwrap().join(".rem-cli/tasks")
    }

    fn status_dir(status: &TaskStatus) -> PathBuf {
        Self::base_dir().join(status.dir_name())
    }

    fn file_path(&self) -> PathBuf {
        Self::status_dir(&self.status).join(format!("{}.md", self.id))
    }

    fn frontmatter(&self) -> TaskFrontmatter {
        TaskFrontmatter {
            id: self.id,
            name: self.name.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }

    pub fn save(&self) -> io::Result<()> {
        let path = self.file_path();
        fs::create_dir_all(path.parent().unwrap())?;
        let yaml = serde_yaml::to_string(&self.frontmatter()).map_err(io::Error::other)?;
        let content = format!("---\n{}---\n", yaml);
        fs::write(path, content)
    }

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

    pub fn load_todo() -> io::Result<Vec<Self>> {
        Self::load_by_status(&[TaskStatus::Todo])
    }

    pub fn load_doing() -> io::Result<Vec<Self>> {
        Self::load_by_status(&[TaskStatus::Doing])
    }

    pub fn load_done() -> io::Result<Vec<Self>> {
        Self::load_by_status(&[TaskStatus::Done])
    }

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

    pub fn update_status(&mut self, new_status: TaskStatus) {
        let old_path = self.file_path();
        self.status = new_status;
        self.updated_at = Utc::now();
        let _ = fs::create_dir_all(Self::status_dir(&self.status));
        let _ = fs::rename(old_path, self.file_path());
        let _ = self.save();
    }

    pub fn sort(tasks: Vec<Task>) -> Vec<Task> {
        let mut todos = Self::filter_by_status(&tasks, TaskStatus::Todo);
        let mut doings = Self::filter_by_status(&tasks, TaskStatus::Doing);
        let mut dones = Self::filter_by_status(&tasks, TaskStatus::Done);
        todos.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        doings.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        dones.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        [todos, doings, dones].concat()
    }

    fn filter_by_status(tasks: &[Task], status: TaskStatus) -> Vec<Task> {
        tasks.iter()
            .filter(|t| t.status == status)
            .cloned()
            .collect()
    }
}
