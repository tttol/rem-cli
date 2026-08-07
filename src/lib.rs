mod application;
mod domain;
mod infrastructure;
mod presentation;

#[cfg(test)]
mod integration_tests;

use crate::application::app::{App, AppEffect, EventTime};
use crate::infrastructure::file_task_repository::FileTaskRepository;
use crate::presentation::terminal::TerminalSession;
use chrono::Local;
use crossterm::event::{self, Event, KeyEventKind};
use std::io;
use std::time::{Duration, Instant};

pub fn run() -> io::Result<()> {
    if std::env::args().any(|argument| argument == "--version" || argument == "-V") {
        println!("rem {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let tasks_dir = infrastructure::config::tasks_dir()
        .map_err(|error| io::Error::new(error.kind(), format!("Failed to load config: {error}")))?;
    let repository = FileTaskRepository::new(tasks_dir);
    let mut app = App::new(repository, Local::now().naive_local());
    let mut terminal = TerminalSession::start()?;
    let run_result = run_event_loop(&mut terminal, &mut app);
    let finish_result = terminal.finish();
    match (run_result, finish_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(run_error), Err(finish_error)) => Err(io::Error::new(
            run_error.kind(),
            format!("{run_error}; failed to restore terminal: {finish_error}"),
        )),
    }
}

fn run_event_loop(
    terminal: &mut TerminalSession,
    app: &mut App<FileTaskRepository>,
) -> io::Result<()> {
    loop {
        let today = Local::now().date_naive();
        terminal.draw(|frame| presentation::render::render(frame, &app.view(), today))?;
        app.load_parking_after_first_render();
        if !event::poll(Duration::from_millis(100))? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        let effect = app.handle_key_event(
            key.code,
            EventTime {
                local: Local::now().naive_local(),
                monotonic: Instant::now(),
            },
        );
        match effect {
            AppEffect::None => {}
            AppEffect::Quit => return Ok(()),
            AppEffect::OpenTask(path) => {
                let editor_error = terminal.open_editor(&path)?;
                app.after_edit();
                if let Some(error) = editor_error {
                    app.report_runtime_error(error);
                }
            }
        }
    }
}
