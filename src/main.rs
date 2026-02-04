use std::io;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};

struct App {
    should_quit: bool,
}

impl App {
    fn new() -> Self {
        Self { should_quit: false }
    }

    fn handle_key_event(&mut self, key_code: KeyCode) {
        match key_code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            _ => {}
        }
    }
}

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;

    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let mut app = App::new();

    while !app.should_quit {
        terminal.draw(|frame| render(frame, &app))?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    app.handle_key_event(key.code);
                }
            }
        }
    }

    disable_raw_mode()?;
    io::stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

fn render(frame: &mut Frame, _app: &App) {
    let block = Block::default()
        .title(" rem-cli ")
        .borders(Borders::ALL);

    let paragraph = Paragraph::new("Press 'q' or Esc to quit")
        .block(block)
        .alignment(Alignment::Center);

    frame.render_widget(paragraph, frame.area());

    let chunks = Layout::vertical([
        Constraint::Ratio(1, 3),
        Constraint::Ratio(1, 3),
        Constraint::Ratio(1, 3),
    ])
    .split(frame.area());

    let todo_block = Block::default()
        .title(" TODO ")
        .borders(Borders::ALL);
    frame.render_widget(todo_block, chunks[0]);

    let doing_block = Block::default()
        .title(" DOING ")
        .borders(Borders::ALL);
    frame.render_widget(doing_block, chunks[1]);

    let done_block = Block::default()
        .title(" DONE ")
        .borders(Borders::ALL);
    frame.render_widget(done_block, chunks[2]);
}
