use crossterm::{
    ExecutableCommand,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend, prelude::Frame};
use std::io::{self, Stdout};
use std::path::Path;
use std::process::Command;

pub(crate) struct TerminalSession {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    active: bool,
}

impl TerminalSession {
    pub(crate) fn start() -> io::Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        if let Err(error) = stdout.execute(EnterAlternateScreen) {
            let cleanup_result = disable_raw_mode();
            return Err(Self::combined_error(
                error,
                cleanup_result,
                "disable raw mode",
            ));
        }
        match Terminal::new(CrosstermBackend::new(stdout)) {
            Ok(terminal) => Ok(Self {
                terminal,
                active: true,
            }),
            Err(error) => {
                let cleanup_result = Self::restore_terminal();
                Err(Self::combined_error(
                    error,
                    cleanup_result,
                    "restore terminal",
                ))
            }
        }
    }

    pub(crate) fn draw(&mut self, render: impl FnOnce(&mut Frame)) -> io::Result<()> {
        self.terminal.draw(render).map(|_| ())
    }

    pub(crate) fn open_editor(&mut self, path: &Path) -> io::Result<Option<String>> {
        self.suspend()?;
        let editor_result = Command::new("nvim").arg(path).status();
        self.resume()?;
        match editor_result {
            Err(error) => Ok(Some(format!(
                "failed to launch nvim for {}: {error}",
                path.display()
            ))),
            Ok(status) if !status.success() => Ok(Some(format!(
                "nvim exited unsuccessfully for {} with {status}",
                path.display()
            ))),
            Ok(_) => Ok(None),
        }
    }

    pub(crate) fn finish(&mut self) -> io::Result<()> {
        self.suspend()
    }

    fn suspend(&mut self) -> io::Result<()> {
        if !self.active {
            return Ok(());
        }
        let result = Self::restore_terminal();
        if result.is_ok() {
            self.active = false;
        }
        result
    }

    fn resume(&mut self) -> io::Result<()> {
        enable_raw_mode()?;
        if let Err(error) = io::stdout().execute(EnterAlternateScreen) {
            let cleanup_result = disable_raw_mode();
            return Err(Self::combined_error(
                error,
                cleanup_result,
                "disable raw mode",
            ));
        }
        self.active = true;
        self.terminal.clear()?;
        Ok(())
    }

    fn restore_terminal() -> io::Result<()> {
        let raw_result = disable_raw_mode();
        let screen_result = io::stdout().execute(LeaveAlternateScreen).map(|_| ());
        match (raw_result, screen_result) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(raw_error), Ok(())) => Err(raw_error),
            (Ok(()), Err(screen_error)) => Err(screen_error),
            (Err(raw_error), Err(screen_error)) => Err(io::Error::new(
                raw_error.kind(),
                format!("{raw_error}; failed to leave alternate screen: {screen_error}"),
            )),
        }
    }

    fn combined_error(
        primary: io::Error,
        cleanup: io::Result<()>,
        cleanup_action: &str,
    ) -> io::Error {
        match cleanup {
            Ok(()) => primary,
            Err(cleanup_error) => io::Error::new(
                primary.kind(),
                format!("{primary}; failed to {cleanup_action}: {cleanup_error}"),
            ),
        }
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        if self.active
            && let Err(error) = Self::restore_terminal()
        {
            eprintln!("Failed to restore terminal: {error}");
        }
    }
}
