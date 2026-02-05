use std::io;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

#[derive(PartialEq)]
enum Mode {
    Normal,
    Editing,
}

struct App {
    should_quit: bool,
    input_mode: Mode,
    input_buffer: String,
    todos: Vec<String>,
    selected_index: Option<usize>,
}

impl App {
    fn new() -> Self {
        Self {
            should_quit: false,
            input_mode: Mode::Normal,
            input_buffer: String::new(),
            todos: Vec::new(),
            selected_index: None,
        }
    }

    fn handle_key_event(&mut self, key_code: KeyCode) {
        match self.input_mode {
            Mode::Normal => match key_code {
                KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
                KeyCode::Char('a') => {
                    self.input_mode = Mode::Editing;
                    self.input_buffer.clear();
                }
                KeyCode::Char('j') | KeyCode::Down => self.select_next(),
                KeyCode::Char('k') | KeyCode::Up => self.select_previous(),
                _ => {}
            },
            Mode::Editing => match key_code {
                KeyCode::Enter => {
                    if !self.input_buffer.is_empty() {
                        self.todos.push(self.input_buffer.clone());
                        if self.selected_index.is_none() {
                            self.selected_index = Some(0);
                        }
                    }
                    self.input_buffer.clear();
                    self.input_mode = Mode::Normal;
                }
                KeyCode::Esc => {
                    self.input_buffer.clear();
                    self.input_mode = Mode::Normal;
                }
                KeyCode::Backspace => {
                    self.input_buffer.pop();
                }
                KeyCode::Char(c) => {
                    self.input_buffer.push(c);
                }
                _ => {}
            },
        }
    }

    fn select_next(&mut self) {
        if self.todos.is_empty() {
            return;
        }
        self.selected_index = Some(match self.selected_index {
            Some(i) => (i + 1).min(self.todos.len() - 1),
            None => 0,
        });
    }

    fn select_previous(&mut self) {
        if self.todos.is_empty() {
            return;
        }
        self.selected_index = Some(match self.selected_index {
            Some(i) => i.saturating_sub(1),
            None => 0,
        });
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

fn render(frame: &mut Frame, app: &App) {
    let main_chunks = if app.input_mode == Mode::Editing {
        Layout::vertical([
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(frame.area())
    } else {
        Layout::vertical([
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(frame.area())
    };

    let chunks = Layout::vertical([
        Constraint::Ratio(1, 3),
        Constraint::Ratio(1, 3),
        Constraint::Ratio(1, 3),
    ])
    .split(main_chunks[0]);

    // TODO block with list
    let items: Vec<ListItem> = app.todos.iter().map(|t| ListItem::new(t.as_str())).collect();
    let list = List::new(items)
        .block(Block::default().title(" TODO ").borders(Borders::ALL))
        .highlight_style(Style::default().bg(Color::DarkGray));
    let mut state = ListState::default();
    state.select(app.selected_index);
    frame.render_stateful_widget(list, chunks[0], &mut state);

    // DOING block
    let doing_block = Block::default()
        .title(" DOING ")
        .borders(Borders::ALL);
    frame.render_widget(doing_block, chunks[1]);

    // DONE block
    let done_block = Block::default()
        .title(" DONE ")
        .borders(Borders::ALL);
    frame.render_widget(done_block, chunks[2]);

    // Input area (shown only in editing mode)
    if app.input_mode == Mode::Editing {
        let input = Paragraph::new(app.input_buffer.as_str())
            .block(Block::default().title(" New Task (Enter: confirm, Esc: cancel) ").borders(Borders::ALL));
        frame.render_widget(input, main_chunks[1]);
    } else {
        let help = Paragraph::new(" a: add task | j/k: select | q: quit ");
        frame.render_widget(help, main_chunks[1]);
    }
}
