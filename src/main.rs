use crossterm::{
    ExecutableCommand,
    event::{
        self, Event, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
        PushKeyboardEnhancementFlags,
    },
    terminal::{
        EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
        supports_keyboard_enhancement,
    },
};
use ratatui::prelude::*;
use rem_cli::app::App;
use rem_cli::render;
use rem_cli::voice::VoiceService;
use std::io;
use std::process::Command;
use std::time::Instant;

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

    let keyboard_release_supported = supports_keyboard_enhancement().unwrap_or(false);
    enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;
    if keyboard_release_supported {
        enable_keyboard_release_events()?;
    }

    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let mut app = App::new();
    app.configure_voice_input(cfg!(target_os = "macos"), keyboard_release_supported);
    let voice_service = VoiceService::new();

    // Polling events
    while !app.should_quit {
        app.tick(Instant::now());
        for voice_event in voice_service.try_iter() {
            app.handle_voice_event(voice_event);
        }
        if let Some(command) = app.take_voice_command() {
            voice_service.execute(command);
        }
        terminal.draw(|frame| render::render(frame, &app))?;
        app.load_parking_after_first_render();

        if event::poll(std::time::Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
        {
            app.handle_key_event(key);
        }

        if let Some(path) = app.open_file.take() {
            if keyboard_release_supported {
                io::stdout().execute(PopKeyboardEnhancementFlags)?;
            }
            disable_raw_mode()?;
            io::stdout().execute(LeaveAlternateScreen)?;
            let _ = Command::new("nvim").arg(&path).status();
            enable_raw_mode()?;
            io::stdout().execute(EnterAlternateScreen)?;
            if keyboard_release_supported {
                enable_keyboard_release_events()?;
            }
            terminal.clear()?;
            app.after_edit();
        }
    }

    if keyboard_release_supported {
        io::stdout().execute(PopKeyboardEnhancementFlags)?;
    }
    disable_raw_mode()?;
    io::stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

fn enable_keyboard_release_events() -> io::Result<()> {
    io::stdout().execute(PushKeyboardEnhancementFlags(
        KeyboardEnhancementFlags::REPORT_EVENT_TYPES
            | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES,
    ))?;
    Ok(())
}
