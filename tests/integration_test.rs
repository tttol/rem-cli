use crossterm::event::KeyCode;
use rem_cli::app::App;
use rem_cli::task::TaskStatus;
use std::fs;

/// Scenario 1: Adding a task via rem creates a new md file.
///
/// Simulates pressing 'a', typing a task name, and pressing Enter.
/// Verifies that a corresponding md file is created in the todo/ directory.
#[test]
fn adding_task_creates_md_file() {
    // GIVEN: an App instance in Normal mode
    let mut app = App::new();
    let initial_task_count = app.tasks.len();

    // WHEN: press 'a' to enter Editing mode, type a task name, and press Enter
    app.handle_key_event(KeyCode::Char('a'));
    for c in "integration test task".chars() {
        app.handle_key_event(KeyCode::Char(c));
    }
    app.handle_key_event(KeyCode::Enter);

    // THEN: a new task is added and its md file exists in the todo/ directory
    assert_eq!(app.tasks.len(), initial_task_count + 1);
    let new_task = app.tasks.iter()
        .find(|t| t.name == "integration test task")
        .expect("task should exist in the list");
    assert_eq!(new_task.status, TaskStatus::Todo);
    let file_path = new_task.file_path();
    assert!(file_path.exists(), "md file should be created");
    assert!(file_path.to_str().unwrap().contains("/todo/"), "md file should be in todo/ directory");

    // Cleanup
    let _ = fs::remove_file(file_path);
}

/// Scenario 2: Pressing 'n' moves the md file to the next status directory.
///
/// Creates a task, then presses 'n' to forward status from TODO to DOING.
/// Verifies the file is moved from todo/ to doing/.
#[test]
fn forward_status_moves_md_file_to_next_directory() {
    // GIVEN: an App with a newly added task in TODO status
    let mut app = App::new();
    app.handle_key_event(KeyCode::Char('a'));
    for c in "forward status test".chars() {
        app.handle_key_event(KeyCode::Char(c));
    }
    app.handle_key_event(KeyCode::Enter);
    let task = app.tasks.iter()
        .find(|t| t.name == "forward status test")
        .expect("task should exist");
    let todo_path = task.file_path();
    assert!(todo_path.exists());

    // WHEN: navigate to the task and press 'n' to forward status (TODO -> DOING)
    let task_index = app.tasks.iter()
        .position(|t| t.name == "forward status test")
        .unwrap();
    app.selected_index = Some(task_index);
    app.handle_key_event(KeyCode::Char('n'));

    // THEN: the file is moved from todo/ to doing/
    assert!(!todo_path.exists(), "file should no longer exist in todo/");
    let task = app.tasks.iter()
        .find(|t| t.name == "forward status test")
        .expect("task should still exist in the list");
    assert_eq!(task.status, TaskStatus::Doing);
    let doing_path = task.file_path();
    assert!(doing_path.exists(), "file should exist in doing/");
    assert!(doing_path.to_str().unwrap().contains("/doing/"));

    // Cleanup
    let _ = fs::remove_file(doing_path);
}

/// Scenario 2 (reverse): Pressing 'N' moves the md file to the previous status directory.
///
/// Creates a task in DOING status, then presses 'N' to backward status from DOING to TODO.
/// Verifies the file is moved from doing/ to todo/.
#[test]
fn backward_status_moves_md_file_to_previous_directory() {
    // GIVEN: an App with a task forwarded to DOING status
    let mut app = App::new();
    app.handle_key_event(KeyCode::Char('a'));
    for c in "backward status test".chars() {
        app.handle_key_event(KeyCode::Char(c));
    }
    app.handle_key_event(KeyCode::Enter);
    let task_index = app.tasks.iter()
        .position(|t| t.name == "backward status test")
        .unwrap();
    app.selected_index = Some(task_index);
    app.handle_key_event(KeyCode::Char('n')); // TODO -> DOING
    let task = app.tasks.iter()
        .find(|t| t.name == "backward status test")
        .expect("task should exist");
    let doing_path = task.file_path();
    assert!(doing_path.exists());

    // WHEN: press 'N' to backward status (DOING -> TODO)
    let task_index = app.tasks.iter()
        .position(|t| t.name == "backward status test")
        .unwrap();
    app.selected_index = Some(task_index);
    app.handle_key_event(KeyCode::Char('N'));

    // THEN: the file is moved from doing/ to todo/
    assert!(!doing_path.exists(), "file should no longer exist in doing/");
    let task = app.tasks.iter()
        .find(|t| t.name == "backward status test")
        .expect("task should still exist in the list");
    assert_eq!(task.status, TaskStatus::Todo);
    let todo_path = task.file_path();
    assert!(todo_path.exists(), "file should exist in todo/");
    assert!(todo_path.to_str().unwrap().contains("/todo/"));

    // Cleanup
    let _ = fs::remove_file(todo_path);
}
