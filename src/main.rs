mod app;
mod render;
mod task;

use std::io;
use std::process::Command;
use crossterm::{
    event::{self, Event, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::prelude::*;

use crate::app::App;

fn main() -> io::Result<()> {
    if std::env::args().any(|a| a == "--version" || a == "-V") {
        println!("rem {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;

    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let mut app = App::new();

    // Polling events
    while !app.should_quit {
        terminal.draw(|frame| render::render(frame, &app))?;

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
            app.update_preview();
        }
    }

    disable_raw_mode()?;
    io::stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}
