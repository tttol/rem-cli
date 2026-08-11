pub(crate) mod app;

use crate::domain::task::{Task, TaskStatus};
use std::io;
use std::path::PathBuf;

pub(crate) trait TaskRepository {
    fn load(&self, status: TaskStatus) -> io::Result<Vec<Task>>;
    fn save(&mut self, task: &Task) -> io::Result<()>;
    fn replace(&mut self, current: &Task, updated: &Task) -> io::Result<()>;
    fn reload(&self, task: &Task) -> io::Result<Task>;
    fn path(&self, task: &Task) -> PathBuf;
}
