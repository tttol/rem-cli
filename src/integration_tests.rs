use crate::application::app::{App, EventTime};
use crate::infrastructure::file_task_repository::FileTaskRepository;
use chrono::{NaiveDate, NaiveDateTime};
use crossterm::event::KeyCode;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;
use uuid::Uuid;

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new() -> Self {
        Self {
            path: std::env::temp_dir().join(format!("rem-cli-integration-{}", Uuid::new_v4())),
        }
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _cleanup_result = fs::remove_dir_all(&self.path);
    }
}

fn now() -> NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 6, 15)
        .and_then(|date| date.and_hms_opt(12, 0, 0))
        .expect("test datetime should be valid")
}

fn event_time() -> EventTime {
    EventTime {
        local: now(),
        monotonic: Instant::now(),
    }
}

fn add_task(app: &mut App<FileTaskRepository>, name: &str) {
    app.handle_key_event(KeyCode::Char('a'), event_time());
    name.chars().for_each(|character| {
        app.handle_key_event(KeyCode::Char(character), event_time());
    });
    app.handle_key_event(KeyCode::Enter, event_time());
}

#[test]
fn task_lifecycle_moves_the_markdown_file_and_preserves_its_body() {
    // GIVEN
    let directory = TestDirectory::new();
    let repository = FileTaskRepository::new(directory.path.clone());
    let mut app = App::new(repository, now());
    add_task(&mut app, "integration task");
    let task_id = app.view().tasks[0].id();
    let todo_path = directory.path.join("todo").join(format!("{task_id}.md"));
    let body = "## Notes\n\nKeep this body.\n";
    let existing = fs::read_to_string(&todo_path).expect("task should be readable");
    fs::write(&todo_path, format!("{existing}{body}")).expect("body should be appended");

    // WHEN
    app.handle_key_event(KeyCode::Char('n'), event_time());
    app.handle_key_event(KeyCode::Char('n'), event_time());

    // THEN
    let done_path = directory.path.join("done").join(format!("{task_id}.md"));
    let actual = fs::read_to_string(&done_path).expect("completed task should be readable");
    assert!(!todo_path.exists());
    assert!(done_path.exists());
    assert!(actual.contains(body));
    assert!(actual.contains("completed_at:"));
    assert!(app.view().tasks.is_empty());
}
