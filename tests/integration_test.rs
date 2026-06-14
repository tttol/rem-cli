use chrono::{Days, Local};
use crossterm::event::KeyCode;
use rem_cli::app::App;
use rem_cli::task::{DEADLINE_DATE_FORMAT, TaskStatus};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

fn temporary_tasks_dir() -> PathBuf {
    std::env::temp_dir().join(format!("rem-cli-integration-test-{}", Uuid::new_v4()))
}

/// Scenario: Adding a task via rem creates a new md file.
///
/// Simulates pressing 'a', typing a task name, and pressing Enter.
/// Verifies that a corresponding md file is created in the todo/ directory.
#[test]
fn adding_task_creates_md_file() {
    // GIVEN: an App instance in Normal mode
    let tasks_dir = temporary_tasks_dir();
    let mut app = App::with_tasks_dir(tasks_dir.clone());
    let initial_task_count = app.tasks.len();

    // WHEN: press 'a' to enter Editing mode, type a task name, and press Enter
    app.handle_key_event(KeyCode::Char('a'));
    for c in "integration test task".chars() {
        app.handle_key_event(KeyCode::Char(c));
    }
    app.handle_key_event(KeyCode::Enter);

    // THEN: a new task is added and its md file exists in the todo/ directory
    assert_eq!(app.tasks.len(), initial_task_count + 1);
    let new_task = app
        .tasks
        .iter()
        .find(|t| t.name == "integration test task")
        .expect("task should exist in the list");
    assert_eq!(new_task.status, TaskStatus::Todo);
    let file_path = new_task.file_path();
    assert!(file_path.exists(), "md file should be created");
    assert!(
        file_path.to_str().unwrap().contains("/todo/"),
        "md file should be in todo/ directory"
    );
    let expected_deadline = Local::now()
        .date_naive()
        .checked_add_days(Days::new(1))
        .unwrap();
    let content = fs::read_to_string(file_path).unwrap();
    assert_eq!(new_task.deadline, expected_deadline);
    assert!(content.contains(&format!(
        "deadline: {}",
        expected_deadline.format(DEADLINE_DATE_FORMAT)
    )));

    fs::remove_dir_all(tasks_dir).unwrap();
}

/// Scenario: Pressing 'n' moves the md file to the next status directory.
///
/// Creates a task, then presses 'n' to forward status from TODO to DOING.
/// Verifies the file is moved from todo/ to doing/.
#[test]
fn forward_status_moves_md_file_to_next_directory() {
    // GIVEN: an App with a newly added task in TODO status
    let tasks_dir = temporary_tasks_dir();
    let mut app = App::with_tasks_dir(tasks_dir.clone());
    app.handle_key_event(KeyCode::Char('a'));
    for c in "forward status test".chars() {
        app.handle_key_event(KeyCode::Char(c));
    }
    app.handle_key_event(KeyCode::Enter);
    let task = app
        .tasks
        .iter()
        .find(|t| t.name == "forward status test")
        .expect("task should exist");
    let todo_path = task.file_path();
    assert!(todo_path.exists());
    let body = "## Notes\n\nsome content here\n";
    let existing = fs::read_to_string(&todo_path).unwrap();
    fs::write(&todo_path, format!("{}{}", existing, body)).unwrap();

    // WHEN: navigate to the task and press 'n' to forward status (TODO -> DOING)
    let task_index = app
        .tasks
        .iter()
        .position(|t| t.name == "forward status test")
        .unwrap();
    app.selected_index = Some(task_index);
    app.handle_key_event(KeyCode::Char('n'));

    // THEN: the file is moved from todo/ to doing/, and body content is preserved
    assert!(!todo_path.exists(), "file should no longer exist in todo/");
    let task = app
        .tasks
        .iter()
        .find(|t| t.name == "forward status test")
        .expect("task should still exist in the list");
    assert_eq!(task.status, TaskStatus::Doing);
    let doing_path = task.file_path();
    assert!(doing_path.exists(), "file should exist in doing/");
    assert!(doing_path.to_str().unwrap().contains("/doing/"));
    assert!(
        fs::read_to_string(&doing_path).unwrap().contains(body),
        "file body should be preserved after status update"
    );

    fs::remove_dir_all(tasks_dir).unwrap();
}

/// Scenario: Pressing 'N' moves the md file to the previous status directory.
///
/// Creates a task in DOING status, then presses 'N' to backward status from DOING to TODO.
/// Verifies the file is moved from doing/ to todo/.
#[test]
fn backward_status_moves_md_file_to_previous_directory() {
    // GIVEN: an App with a task forwarded to DOING status
    let tasks_dir = temporary_tasks_dir();
    let mut app = App::with_tasks_dir(tasks_dir.clone());
    app.handle_key_event(KeyCode::Char('a'));
    for c in "backward status test".chars() {
        app.handle_key_event(KeyCode::Char(c));
    }
    app.handle_key_event(KeyCode::Enter);
    let task_index = app
        .tasks
        .iter()
        .position(|t| t.name == "backward status test")
        .unwrap();
    app.selected_index = Some(task_index);
    app.handle_key_event(KeyCode::Char('n')); // TODO -> DOING
    let task = app
        .tasks
        .iter()
        .find(|t| t.name == "backward status test")
        .expect("task should exist");
    let doing_path = task.file_path();
    assert!(doing_path.exists());

    // WHEN: press 'N' to backward status (DOING -> TODO)
    let task_index = app
        .tasks
        .iter()
        .position(|t| t.name == "backward status test")
        .unwrap();
    app.selected_index = Some(task_index);
    app.handle_key_event(KeyCode::Char('N'));

    // THEN: the file is moved from doing/ to todo/
    assert!(
        !doing_path.exists(),
        "file should no longer exist in doing/"
    );
    let task = app
        .tasks
        .iter()
        .find(|t| t.name == "backward status test")
        .expect("task should still exist in the list");
    assert_eq!(task.status, TaskStatus::Todo);
    let todo_path = task.file_path();
    assert!(todo_path.exists(), "file should exist in todo/");
    assert!(todo_path.to_str().unwrap().contains("/todo/"));

    fs::remove_dir_all(tasks_dir).unwrap();
}

/// Scenario: Pressing 'N' moves a TODO task to PARKING.
#[test]
fn backward_status_moves_todo_file_to_parking_directory() {
    // GIVEN
    let tasks_dir = temporary_tasks_dir();
    let mut app = App::with_tasks_dir(tasks_dir.clone());
    app.handle_key_event(KeyCode::Char('a'));
    for c in "parking backward status test".chars() {
        app.handle_key_event(KeyCode::Char(c));
    }
    app.handle_key_event(KeyCode::Enter);
    let task_index = app
        .tasks
        .iter()
        .position(|task| task.name == "parking backward status test")
        .unwrap();
    app.selected_index = Some(task_index);
    let todo_path = app.tasks[task_index].file_path();

    // WHEN
    app.handle_key_event(KeyCode::Char('N'));

    // THEN
    assert!(!todo_path.exists());
    let task = app
        .tasks
        .iter()
        .find(|task| task.name == "parking backward status test")
        .expect("task should still exist in the list");
    let expected = TaskStatus::Parking;
    assert_eq!(task.status, expected);
    assert!(task.file_path().to_str().unwrap().contains("/parking/"));

    fs::remove_dir_all(tasks_dir).unwrap();
}

/// Scenario: Moving a task to a hidden DONE column removes it from view.
#[test]
fn moving_task_to_hidden_done_selects_nearby_visible_task() {
    // GIVEN
    let tasks_dir = temporary_tasks_dir();
    let mut app = App::with_tasks_dir(tasks_dir.clone());
    for name in ["hidden done target", "visible neighbor"] {
        app.handle_key_event(KeyCode::Char('a'));
        for c in name.chars() {
            app.handle_key_event(KeyCode::Char(c));
        }
        app.handle_key_event(KeyCode::Enter);
    }
    let target_index = app
        .tasks
        .iter()
        .position(|task| task.name == "hidden done target")
        .unwrap();
    app.selected_index = Some(target_index);
    app.handle_key_event(KeyCode::Char('n'));
    let target_index = app
        .tasks
        .iter()
        .position(|task| task.name == "hidden done target")
        .unwrap();
    app.selected_index = Some(target_index);
    let task_id = app.tasks[target_index].id;
    let doing_path = app.tasks[target_index].file_path();
    let tasks_dir = doing_path.parent().unwrap().parent().unwrap();
    let done_path = tasks_dir.join("done").join(format!("{}.md", task_id));

    // WHEN
    app.handle_key_event(KeyCode::Char('n'));

    // THEN
    assert!(done_path.exists());
    let done_content = fs::read_to_string(&done_path).unwrap();
    assert!(done_content.contains("completed_at:"));
    assert!(
        app.tasks
            .iter()
            .all(|task| task.name != "hidden done target")
    );
    let selected_task = app
        .selected_index
        .and_then(|index| app.tasks.get(index))
        .expect("a nearby task should be selected");
    let expected = "visible neighbor";
    assert_eq!(selected_task.name, expected);

    fs::remove_dir_all(tasks_dir).unwrap();
}
