use crossterm::{
    ExecutableCommand,
    event::{self, Event, KeyEventKind},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::prelude::*;
use rem_cli::app::App;
use rem_cli::config;
use rem_cli::render;
use std::io;
use std::process::{self, Command};

/// Entry point for the rem TUI application.
///
/// Handles `--version` / `-V` flags, sets up the terminal (raw mode, alternate screen),
/// runs the event loop, and restores the terminal on exit.
/// When `app.open_file` is set, temporarily exits the TUI to open the file in neovim.
fn main() -> io::Result<()> {
    if std::env::args().any(|a| a == "--version" || a == "-V") {
        println!("rem {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let tasks_dir = match config::tasks_dir() {
        Ok(tasks_dir) => tasks_dir,
        Err(error) => {
            eprintln!("Failed to load config: {error}");
            process::exit(1);
        }
    };

    enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;

    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let mut app = App::with_tasks_dir(tasks_dir);

    // Polling events
    while !app.should_quit {
        terminal.draw(|frame| render::render(frame, &app))?;
        app.load_parking_after_first_render();

        if event::poll(std::time::Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            app.handle_key_event(key.code);
        }

        if let Some(path) = app.open_file.take() {
            disable_raw_mode()?;
            io::stdout().execute(LeaveAlternateScreen)?;
            let _ = Command::new("nvim").arg(&path).status();
            enable_raw_mode()?;
            io::stdout().execute(EnterAlternateScreen)?;
            terminal.clear()?;
            app.after_edit();
        }
    }

    disable_raw_mode()?;
    io::stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}
