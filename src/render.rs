use crate::app::{App, Mode};
use crate::task::{DEADLINE_DATE_FORMAT, TASK_DATETIME_FORMAT, Task, TaskStatus};
use chrono::{Days, Local, NaiveDate};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

const OVERDUE_DEADLINE_COLOR: Color = Color::Rgb(190, 180, 120);

/// Wraps a task name to the available panel width.
fn wrap_task_name(name: &str, width: usize) -> Text<'static> {
    if width == 0 || Line::from(name).width() <= width {
        return Text::from(name.to_string());
    }
    let (mut lines, current_line) = name.chars().fold(
        (Vec::new(), String::new()),
        |(mut lines, current_line), character| {
            if character == '\n' {
                lines.push(Line::from(current_line));
                return (lines, String::new());
            }
            let candidate = format!("{current_line}{character}");
            if !current_line.is_empty() && Line::from(candidate.as_str()).width() > width {
                lines.push(Line::from(current_line));
                return (lines, character.to_string());
            }
            (lines, candidate)
        },
    );
    if !current_line.is_empty() || lines.is_empty() {
        lines.push(Line::from(current_line));
    }
    Text::from(lines)
}

/// Builds the task text with its deadline below the wrapped name.
fn task_text(task: &Task, width: usize, today: NaiveDate, is_selected: bool) -> Text<'static> {
    let is_overdue = task.deadline < today;
    let name_style = if is_overdue {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    let deadline_color = if is_overdue {
        OVERDUE_DEADLINE_COLOR
    } else if is_selected {
        Color::Gray
    } else {
        Color::DarkGray
    };
    let deadline = Line::styled(
        format!("Deadline: {}", task.deadline.format(DEADLINE_DATE_FORMAT)),
        Style::default().fg(deadline_color),
    );
    let completed = task.completed_at.map(|completed_at| {
        Line::styled(
            format!("Completed: {}", completed_at.format(TASK_DATETIME_FORMAT)),
            Style::default().fg(if is_selected {
                Color::Gray
            } else {
                Color::DarkGray
            }),
        )
    });
    Text::from(
        wrap_task_name(task.name.as_str(), width)
            .lines
            .into_iter()
            .map(|line| line.patch_style(name_style))
            .chain([deadline])
            .chain(completed)
            .collect::<Vec<_>>(),
    )
}

fn status_title_style(status: TaskStatus) -> Style {
    let background = match status {
        TaskStatus::Parking | TaskStatus::Done => Color::DarkGray,
        TaskStatus::Todo => Color::Rgb(140, 20, 20),
        TaskStatus::Doing => Color::Rgb(20, 110, 45),
    };
    Style::default().fg(Color::White).bg(background)
}

/// Renders the entire TUI layout.
///
/// Layout structure:
/// - Main area: PARKING, TODO, DOING, and optionally DONE columns
/// - Bottom: Input field (Editing mode) or keybinding help (Normal mode)
pub fn render(frame: &mut Frame, app: &App) {
    let outer = if app.input_mode == Mode::Editing {
        Layout::vertical([Constraint::Min(0), Constraint::Length(3)]).split(frame.area())
    } else {
        Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(frame.area())
    };

    let statuses = if app.done_loaded {
        let done_week_end = app
            .done_week_start
            .checked_add_days(Days::new(6))
            .expect("done week end should be a valid date");
        vec![
            (TaskStatus::Parking, " PARKING ".to_string()),
            (TaskStatus::Todo, " TODO ".to_string()),
            (TaskStatus::Doing, " DOING ".to_string()),
            (
                TaskStatus::Done,
                format!(
                    " DONE {}-{} ",
                    app.done_week_start.format(DEADLINE_DATE_FORMAT),
                    done_week_end.format(DEADLINE_DATE_FORMAT)
                ),
            ),
        ]
    } else {
        vec![
            (TaskStatus::Parking, " PARKING ".to_string()),
            (TaskStatus::Todo, " TODO ".to_string()),
            (TaskStatus::Doing, " DOING ".to_string()),
        ]
    };
    let constraints = vec![Constraint::Ratio(1, statuses.len() as u32); statuses.len()];
    let columns = Layout::horizontal(constraints).split(outer[0]);
    let today = Local::now().date_naive();

    for (column, ((status, title), area)) in statuses.iter().zip(columns.iter()).enumerate() {
        let mut selected_in_group: Option<usize> = None;
        let items: Vec<ListItem> = app
            .tasks
            .iter()
            .enumerate()
            .filter(|(_, t)| t.status == *status)
            .enumerate()
            .map(|(group_idx, (global_idx, t))| {
                let is_selected = app.selected_index == Some(global_idx);
                if is_selected {
                    selected_in_group = Some(group_idx);
                }
                ListItem::new(task_text(
                    t,
                    area.width.saturating_sub(2) as usize,
                    today,
                    is_selected,
                ))
            })
            .collect();
        let is_empty_done_selected =
            *status == TaskStatus::Done && app.done_loaded && app.selected_index.is_none();
        let border_style = if selected_in_group.is_some() || is_empty_done_selected {
            Style::default().fg(Color::Green)
        } else {
            Style::default()
        };
        let list = List::new(items)
            .block(
                Block::default()
                    .title(title.as_str())
                    .title_style(status_title_style(*status))
                    .borders(Borders::ALL)
                    .border_style(border_style),
            )
            .highlight_style(Style::default().bg(Color::DarkGray));
        let mut state = ListState::default();
        state.select(selected_in_group);
        frame.render_stateful_widget(list, columns[column], &mut state);
    }

    if app.input_mode == Mode::Editing {
        let cursor_prefix = app
            .input_buffer
            .chars()
            .take(app.input_cursor)
            .collect::<String>();
        let cursor_width = Line::from(cursor_prefix.as_str()).width() as u16;
        let input_width = outer[1].width.saturating_sub(2).max(1);
        let horizontal_offset = cursor_width.saturating_sub(input_width.saturating_sub(1));
        let input_title = app
            .error_message
            .as_deref()
            .unwrap_or("New Task (Enter: confirm, Esc: cancel)");
        let input_style = app
            .error_message
            .as_ref()
            .map_or_else(Style::default, |_| Style::default().fg(Color::Red));
        let input = Paragraph::new(app.input_buffer.as_str())
            .block(
                Block::default()
                    .title(format!(" {input_title} "))
                    .borders(Borders::ALL),
            )
            .style(input_style)
            .scroll((0, horizontal_offset));
        frame.render_widget(input, outer[1]);
        frame.set_cursor_position((
            outer[1].x + 1 + cursor_width.saturating_sub(horizontal_offset),
            outer[1].y + 1,
        ));
    } else {
        let (message, style) = app.error_message.as_deref().map_or_else(
            || {
                (
                    " a: add | j/k: up/down | G/gg: bottom/top | h/l: left/right | n/N: status | d: done | [/]: done week | q: quit ",
                    Style::default(),
                )
            },
            |error| (error, Style::default().fg(Color::Red)),
        );
        let help = Paragraph::new(message).style(style);
        frame.render_widget(help, outer[1]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::Task;
    use ratatui::backend::TestBackend;

    fn create_app(done_loaded: bool) -> App {
        App {
            should_quit: false,
            input_mode: Mode::Normal,
            input_buffer: String::new(),
            input_cursor: 0,
            tasks: Vec::new(),
            selected_index: None,
            parking_loaded: false,
            done_loaded,
            done_week_start: Task::week_start(Local::now().date_naive()),
            open_file: None,
            error_message: None,
            tasks_dir: Task::default_base_dir(),
            persistent_error: None,
            pending_g_at: None,
        }
    }

    fn rendered_text(done_loaded: bool) -> String {
        let backend = TestBackend::new(120, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = create_app(done_loaded);
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let buffer = terminal.backend().buffer();
        (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .filter_map(|x| buffer.cell((x, y)))
                    .map(|cell| cell.symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn renders_three_columns_without_done() {
        // GIVEN
        let expected_titles = ["PARKING", "TODO", "DOING"];

        // WHEN
        let actual = rendered_text(false);

        // THEN
        assert!(expected_titles.iter().all(|title| actual.contains(title)));
        assert!(!actual.contains("DONE"));
        assert!(!actual.contains("Preview"));
    }

    #[test]
    fn renders_four_columns_with_done() {
        // GIVEN
        let expected_titles = ["PARKING", "TODO", "DOING", "DONE"];

        // WHEN
        let actual = rendered_text(true);

        // THEN
        assert!(expected_titles.iter().all(|title| actual.contains(title)));
        assert!(!actual.contains("Preview"));
    }

    #[test]
    fn renders_done_week_period() {
        // GIVEN
        let mut app = create_app(true);
        app.done_week_start = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        let backend = TestBackend::new(160, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        let expected = "DONE 2026/06/15-2026/06/21";

        // WHEN
        terminal.draw(|frame| render(frame, &app)).unwrap();

        // THEN
        let buffer = terminal.backend().buffer();
        let actual = (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .filter_map(|x| buffer.cell((x, y)))
                    .map(|cell| cell.symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(actual.contains(expected));
    }

    #[test]
    fn renders_empty_done_column_as_selected_after_week_change() {
        // GIVEN
        let mut app = create_app(true);
        app.selected_index = None;
        let backend = TestBackend::new(160, 10);
        let mut terminal = Terminal::new(backend).unwrap();

        // WHEN
        terminal.draw(|frame| render(frame, &app)).unwrap();

        // THEN
        let buffer = terminal.backend().buffer();
        let done_border = buffer
            .cell((buffer.area.width - 1, 1))
            .expect("DONE border should exist");
        assert_eq!(done_border.fg, Color::Green);
    }

    #[test]
    fn renders_navigation_help() {
        // GIVEN
        let expected = "G/gg: bottom/top";

        // WHEN
        let actual = rendered_text(false);

        // THEN
        assert!(actual.contains(expected));
    }

    #[test]
    fn wraps_long_task_name_without_spaces() {
        // GIVEN
        let task_name = "長いタスクタイトル全文表示";
        let width = 8;
        let expected = Text::from(vec![
            Line::from("長いタス"),
            Line::from("クタイト"),
            Line::from("ル全文表"),
            Line::from("示"),
        ]);

        // WHEN
        let actual = wrap_task_name(task_name, width);

        // THEN
        assert_eq!(actual, expected);
    }

    #[test]
    fn task_text_displays_unselected_deadline_in_dark_gray() {
        // GIVEN
        let task = Task::new("deadline task".to_string());
        let today = task.deadline;
        let expected = Text::from(vec![
            Line::from("deadline task"),
            Line::styled(
                format!("Deadline: {}", task.deadline.format(DEADLINE_DATE_FORMAT)),
                Style::default().fg(Color::DarkGray),
            ),
        ]);

        // WHEN
        let actual = task_text(&task, 20, today, false);

        // THEN
        assert_eq!(actual, expected);
    }

    #[test]
    fn task_text_displays_selected_deadline_in_gray() {
        // GIVEN
        let task = Task::new("selected task".to_string());
        let today = task.deadline;
        let expected = Text::from(vec![
            Line::from("selected task"),
            Line::styled(
                format!("Deadline: {}", task.deadline.format(DEADLINE_DATE_FORMAT)),
                Style::default().fg(Color::Gray),
            ),
        ]);

        // WHEN
        let actual = task_text(&task, 20, today, true);

        // THEN
        assert_eq!(actual, expected);
    }

    #[test]
    fn task_text_displays_overdue_task_in_yellow() {
        // GIVEN
        let mut task = Task::new("overdue task".to_string());
        let today = task.deadline;
        task.deadline = today.pred_opt().unwrap();
        let expected = Text::from(vec![
            Line::styled("overdue task", Style::default().fg(Color::Yellow)),
            Line::styled(
                format!("Deadline: {}", task.deadline.format(DEADLINE_DATE_FORMAT)),
                Style::default().fg(OVERDUE_DEADLINE_COLOR),
            ),
        ]);

        // WHEN
        let actual = task_text(&task, 20, today, false);

        // THEN
        assert_eq!(actual, expected);
    }

    #[test]
    fn task_text_displays_completed_datetime() {
        // GIVEN
        let mut task = Task::new("completed task".to_string());
        let completed_at = NaiveDate::from_ymd_opt(2026, 6, 15)
            .unwrap()
            .and_hms_opt(10, 30, 45)
            .unwrap();
        task.status = TaskStatus::Done;
        task.completed_at = Some(completed_at);
        let today = task.deadline;
        let expected = format!("Completed: {}", completed_at.format(TASK_DATETIME_FORMAT));

        // WHEN
        let actual = task_text(&task, 30, today, false);

        // THEN
        assert_eq!(actual.lines.last().unwrap().to_string(), expected);
    }

    #[test]
    fn status_titles_have_expected_background_colors() {
        // GIVEN
        let cases = [
            (TaskStatus::Parking, Color::DarkGray),
            (TaskStatus::Todo, Color::Rgb(140, 20, 20)),
            (TaskStatus::Doing, Color::Rgb(20, 110, 45)),
            (TaskStatus::Done, Color::DarkGray),
        ];

        // WHEN
        let actual = cases.map(|(status, _)| status_title_style(status).bg);
        let expected = cases.map(|(_, color)| Some(color));

        // THEN
        assert_eq!(actual, expected);
    }
}
