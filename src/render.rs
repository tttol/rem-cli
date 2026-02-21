use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};
use crate::app::{App, Mode};
use crate::task::TaskStatus;

/// タスク名をパネル幅に合わせて折り返す。
/// 単語の途中で折り返さず、スペース区切りでワードラップする。
fn wrap_task_name(name: &str, width: usize) -> Text<'static> {
    if width == 0 || name.chars().count() <= width {
        return Text::from(name.to_string());
    }
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut current_line = String::new();
    for word in name.split_whitespace() {
        let word_len = word.chars().count();
        let line_len = current_line.chars().count();
        if line_len == 0 {
            current_line.push_str(word);
        } else if line_len + 1 + word_len <= width {
            current_line.push(' ');
            current_line.push_str(word);
        } else {
            lines.push(Line::from(current_line.clone()));
            current_line = word.to_string();
        }
    }
    if !current_line.is_empty() {
        lines.push(Line::from(current_line));
    }
    Text::from(lines)
}

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

    // ボーダー2文字分を引いたリストパネルの実効幅
    let list_width = (frame.area().width as usize * 30 / 100).saturating_sub(2);

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
                ListItem::new(wrap_task_name(t.name.as_str(), list_width))
            })
            .collect();
        let border_style = if selected_in_group.is_some() {
            Style::default().fg(Color::Green)
        } else {
            Style::default()
        };
        let list = List::new(items)
            .block(Block::default().title(*title).borders(Borders::ALL).border_style(border_style))
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
                ListItem::new(wrap_task_name(t.name.as_str(), list_width))
            })
            .collect();
        let border_style = if selected_in_group.is_some() {
            Style::default().fg(Color::Green)
        } else {
            Style::default()
        };
        let list = List::new(items)
            .block(Block::default().title(" DONE ").borders(Borders::ALL).border_style(border_style))
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
        frame.set_cursor_position((
            outer[1].x + 1 + app.input_buffer.len() as u16,
            outer[1].y + 1,
        ));
    } else {
        let help = Paragraph::new(" a: add | j/k: select | n: forward | d: toggle done | q: quit ");
        frame.render_widget(help, outer[1]);
    }
}
