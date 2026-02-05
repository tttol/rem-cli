use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};
use crate::{App, Mode};

pub fn render(frame: &mut Frame, app: &App) {
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
    let items: Vec<ListItem> = app.tasks.iter().map(|t| ListItem::new(t.name.as_str())).collect();
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
