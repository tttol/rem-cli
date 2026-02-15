use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};
use crate::{App, Mode};
use crate::task::TaskStatus;

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

    // Build filtered lists with selected index mapping
    let statuses = [
        (TaskStatus::Todo, " TODO "),
        (TaskStatus::Doing, " DOING "),
        (TaskStatus::Done, " DONE "),
    ];
    for (i, (status, title)) in statuses.iter().enumerate() {
        let mut selected_in_group: Option<usize> = None;
        let items: Vec<ListItem> = app.tasks.iter().enumerate()
            .filter(|(_, t)| t.status == *status)
            .enumerate()
            .map(|(group_idx, (global_idx, t))| {
                if app.selected_index == Some(global_idx) {
                    selected_in_group = Some(group_idx);
                }
                ListItem::new(t.name.as_str())
            })
            .collect();
        let list = List::new(items)
            .block(Block::default().title(*title).borders(Borders::ALL))
            .highlight_style(Style::default().bg(Color::DarkGray));
        let mut state = ListState::default();
        state.select(selected_in_group);
        frame.render_stateful_widget(list, chunks[i], &mut state);
    }

    // Input area (shown only in editing mode)
    if app.input_mode == Mode::Editing {
        let input = Paragraph::new(app.input_buffer.as_str())
            .block(Block::default().title(" New Task (Enter: confirm, Esc: cancel) ").borders(Borders::ALL));
        frame.render_widget(input, main_chunks[1]);
    } else {
        let help = Paragraph::new(" a: add task | j/k: select | n: forward status | q: quit ");
        frame.render_widget(help, main_chunks[1]);
    }
}
