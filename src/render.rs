use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};
use crate::app::{App, Mode};
use crate::task::TaskStatus;

/// Renders the entire TUI layout.
///
/// Layout structure:
/// - Left 30%: Task list panels (TODO, DOING, DONE)
/// - Right 70%: Preview panel showing the selected task's markdown content
/// - Bottom: Input field (Editing mode) or keybinding help (Normal mode)
///
/// The DONE panel is minimized to a border-only row when `done_loaded` is false.
pub fn render(frame: &mut Frame, app: &App) {
    let outer = if app.input_mode == Mode::Editing {
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

    // Left 30% task lists, Right 70% preview
    let h_chunks = Layout::horizontal([
        Constraint::Percentage(30),
        Constraint::Percentage(70),
    ])
    .split(outer[0]);

    // Left: task list panels
    let list_chunks = if app.done_loaded {
        Layout::vertical([
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
        ])
        .split(h_chunks[0])
    } else {
        Layout::vertical([
            Constraint::Ratio(1, 2),
            Constraint::Ratio(1, 2),
            Constraint::Length(3),
        ])
        .split(h_chunks[0])
    };

    let active_statuses = [
        (TaskStatus::Todo, " TODO "),
        (TaskStatus::Doing, " DOING "),
    ];
    for (i, (status, title)) in active_statuses.iter().enumerate() {
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
        frame.render_stateful_widget(list, list_chunks[i], &mut state);
    }

    if app.done_loaded {
        let mut selected_in_group: Option<usize> = None;
        let items: Vec<ListItem> = app.tasks.iter().enumerate()
            .filter(|(_, t)| t.status == TaskStatus::Done)
            .enumerate()
            .map(|(group_idx, (global_idx, t))| {
                if app.selected_index == Some(global_idx) {
                    selected_in_group = Some(group_idx);
                }
                ListItem::new(t.name.as_str())
            })
            .collect();
        let list = List::new(items)
            .block(Block::default().title(" DONE ").borders(Borders::ALL))
            .highlight_style(Style::default().bg(Color::DarkGray));
        let mut state = ListState::default();
        state.select(selected_in_group);
        frame.render_stateful_widget(list, list_chunks[2], &mut state);
    } else {
        let block = Block::default().title(" DONE (d to load) ").borders(Borders::ALL);
        frame.render_widget(block, list_chunks[2]);
    }

    // Right: preview panel
    let preview = Paragraph::new(app.preview_content.as_str())
        .block(Block::default().title(" Preview ").borders(Borders::ALL))
        .wrap(ratatui::widgets::Wrap { trim: false });
    frame.render_widget(preview, h_chunks[1]);

    // Bottom: input or help
    if app.input_mode == Mode::Editing {
        let input = Paragraph::new(app.input_buffer.as_str())
            .block(Block::default().title(" New Task (Enter: confirm, Esc: cancel) ").borders(Borders::ALL));
        frame.render_widget(input, outer[1]);
    } else {
        let help = Paragraph::new(" a: add | j/k: select | n: forward | d: toggle done | q: quit ");
        frame.render_widget(help, outer[1]);
    }
}
