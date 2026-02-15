use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub enum TaskStatus {
    #[serde(rename = "todo")]
    Todo,
    #[serde(rename = "doing")]
    Doing,
    #[serde(rename = "done")]
    Done,
}

#[derive(Clone, Serialize, Deserialize)]
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

    fn tasks_dir() -> PathBuf {
        dirs::home_dir().unwrap().join(".rem-cli/tasks")
    }

    fn file_path(&self) -> PathBuf {
        Self::tasks_dir().join(format!("{}.md", self.id))
    }

    pub fn save(&self) -> io::Result<()> {
        let path = self.file_path();
        fs::create_dir_all(path.parent().unwrap())?;
        let yaml = serde_yaml::to_string(self).map_err(io::Error::other)?;
        let content = format!("---\n{}---\n", yaml);
        fs::write(path, content)
    }

    fn load(path: &Path) -> io::Result<Self> {
        let content = fs::read_to_string(path)?;
        let yaml = content
            .trim_start_matches("---\n")
            .split("---")
            .next()
            .unwrap_or("");
        serde_yaml::from_str(yaml).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    pub fn load_all() -> io::Result<Vec<Self>> {
        let dir = Self::tasks_dir();
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut tasks = Vec::new();
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            if path.extension().is_some_and(|e| e == "md")
                && let Ok(task) = Self::load(&path)
            {
                tasks.push(task);
            }
        }
        tasks.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        Ok(tasks)
    }


    pub fn update_status(&mut self, status: TaskStatus) {
        self.status = status;
        self.updated_at = Utc::now();
        let _ = self.save();
    }
}
