use crate::application::app::{AppView, InputView, Selection};
use crate::domain::task::{DEADLINE_DATE_FORMAT, TASK_DATETIME_FORMAT, Task, TaskStatus};
use chrono::NaiveDate;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

const OVERDUE_DEADLINE_COLOR: Color = Color::Rgb(190, 180, 120);

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

fn task_text(task: &Task, width: usize, today: NaiveDate, is_selected: bool) -> Text<'static> {
    let is_overdue = task.deadline() < today;
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
        format!("Deadline: {}", task.deadline().format(DEADLINE_DATE_FORMAT)),
        Style::default().fg(deadline_color),
    );
    let completed = task.completed_at().map(|completed_at| {
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
        wrap_task_name(task.name(), width)
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

pub(crate) fn render(frame: &mut Frame, view: &AppView<'_>, today: NaiveDate) {
    let is_editing = matches!(view.input, InputView::Editing { .. });
    let outer = if is_editing {
        Layout::vertical([Constraint::Min(0), Constraint::Length(3)]).split(frame.area())
    } else {
        Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(frame.area())
    };
    let main = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).split(outer[0]);
    let statuses = view.done_week.map_or_else(
        || {
            vec![
                (TaskStatus::Parking, " PARKING ".to_string()),
                (TaskStatus::Todo, " TODO ".to_string()),
                (TaskStatus::Doing, " DOING ".to_string()),
            ]
        },
        |week| {
            vec![
                (TaskStatus::Parking, " PARKING ".to_string()),
                (TaskStatus::Todo, " TODO ".to_string()),
                (TaskStatus::Doing, " DOING ".to_string()),
                (
                    TaskStatus::Done,
                    format!(
                        " DONE {}-{} ",
                        week.start().format(DEADLINE_DATE_FORMAT),
                        week.end_inclusive().format(DEADLINE_DATE_FORMAT)
                    ),
                ),
            ]
        },
    );
    let constraints = vec![Constraint::Ratio(1, statuses.len() as u32); statuses.len()];
    let columns = Layout::horizontal(constraints).split(main[1]);
    let last_updated = Paragraph::new(format!(
        " last updated: {}",
        view.last_updated_at.format(TASK_DATETIME_FORMAT)
    ))
    .alignment(Alignment::Right);
    frame.render_widget(last_updated, main[0]);

    for ((status, title), area) in statuses.iter().zip(columns.iter()) {
        let group: Vec<_> = view
            .tasks
            .iter()
            .filter(|task| task.status() == *status)
            .collect();
        let selected_in_group = group
            .iter()
            .position(|task| matches!(view.selection, Selection::Task(id) if id == task.id()));
        let items: Vec<ListItem> = group
            .iter()
            .map(|task| {
                let is_selected = matches!(view.selection, Selection::Task(id) if id == task.id());
                ListItem::new(task_text(
                    task,
                    area.width.saturating_sub(2) as usize,
                    today,
                    is_selected,
                ))
            })
            .collect();
        let is_empty_column_selected =
            matches!(view.selection, Selection::EmptyColumn(selected) if selected == *status);
        let border_style = if selected_in_group.is_some() || is_empty_column_selected {
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
        let mut state = ListState::default().with_selected(selected_in_group);
        frame.render_stateful_widget(list, *area, &mut state);
    }

    match view.input {
        InputView::Editing { buffer, cursor } => {
            let cursor_prefix = buffer.chars().take(cursor).collect::<String>();
            let cursor_width = Line::from(cursor_prefix.as_str()).width() as u16;
            let input_width = outer[1].width.saturating_sub(2).max(1);
            let horizontal_offset = cursor_width.saturating_sub(input_width.saturating_sub(1));
            let input_title = view
                .error_message
                .as_deref()
                .unwrap_or("New Task (Enter: confirm, Esc: cancel)");
            let input_style = view
                .error_message
                .as_ref()
                .map_or_else(Style::default, |_| Style::default().fg(Color::Red));
            let input = Paragraph::new(buffer)
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
        }
        InputView::Normal => {
            let (message, style) = view.error_message.as_deref().map_or_else(
                || {
                    (
                        " a: add | j/k: up/down | G/gg: bottom/top | h/l: left/right | n/N: status | r: reload | d: done | [/]: done week | q: quit ",
                        Style::default(),
                    )
                },
                |error| (error, Style::default().fg(Color::Red)),
            );
            frame.render_widget(Paragraph::new(message).style(style), outer[1]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::task::WeekRange;
    use chrono::NaiveDateTime;
    use ratatui::backend::TestBackend;
    use uuid::Uuid;

    fn datetime() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 6, 15)
            .and_then(|date| date.and_hms_opt(9, 30, 0))
            .expect("test datetime should be valid")
    }

    fn task(name: &str, status: TaskStatus) -> Task {
        Task::from_stored(
            Uuid::new_v4(),
            name.to_string(),
            status,
            datetime(),
            datetime(),
            (status == TaskStatus::Done).then_some(datetime()),
            NaiveDate::from_ymd_opt(2026, 6, 16).expect("test date should be valid"),
        )
    }

    fn rendered_text(tasks: &[Task], selection: Selection, done: bool) -> String {
        let done_week = done.then(|| {
            WeekRange::containing(
                NaiveDate::from_ymd_opt(2026, 6, 15).expect("test date should be valid"),
            )
            .expect("week should be valid")
        });
        let view = AppView {
            input: InputView::Normal,
            tasks,
            selection,
            done_week,
            last_updated_at: datetime(),
            error_message: None,
        };
        let backend = TestBackend::new(120, 20);
        let mut terminal = Terminal::new(backend).expect("test terminal should initialize");
        terminal
            .draw(|frame| {
                render(
                    frame,
                    &view,
                    NaiveDate::from_ymd_opt(2026, 6, 15).expect("test date should be valid"),
                );
            })
            .expect("render should succeed");
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
    fn renders_three_or_four_status_columns_based_on_done_visibility() {
        // GIVEN
        let tasks = vec![task("todo", TaskStatus::Todo)];

        // WHEN
        let actual = [
            rendered_text(&tasks, Selection::Task(tasks[0].id()), false),
            rendered_text(&tasks, Selection::Task(tasks[0].id()), true),
        ];

        // THEN
        assert!(actual[0].contains("PARKING"));
        assert!(actual[0].contains("TODO"));
        assert!(actual[0].contains("DOING"));
        assert!(!actual[0].contains("DONE 2026"));
        assert!(actual[1].contains("DONE 2026/06/15-2026/06/21"));
    }

    #[test]
    fn renders_task_metadata_and_navigation_help() {
        // GIVEN
        let tasks = vec![task("a long task name", TaskStatus::Todo)];

        // WHEN
        let actual = rendered_text(&tasks, Selection::Task(tasks[0].id()), false);

        // THEN
        assert!(actual.contains("a long task name"));
        assert!(actual.contains("Deadline: 2026/06/16"));
        assert!(actual.contains("last updated: 2026/06/15 09:30:00"));
        assert!(actual.contains("a: add"));
    }

    #[test]
    fn empty_done_column_can_remain_selected() {
        // GIVEN
        let tasks = vec![task("todo", TaskStatus::Todo)];

        // WHEN
        let actual = rendered_text(&tasks, Selection::EmptyColumn(TaskStatus::Done), true);

        // THEN
        assert!(actual.contains("DONE 2026/06/15-2026/06/21"));
    }

    #[test]
    fn wraps_long_task_names_by_display_width() {
        // GIVEN
        let name = "長いタスクタイトル全文表示";
        let width = 8;
        let expected = Text::from(vec![
            Line::from("長いタス"),
            Line::from("クタイト"),
            Line::from("ル全文表"),
            Line::from("示"),
        ]);

        // WHEN
        let actual = wrap_task_name(name, width);

        // THEN
        assert_eq!(actual, expected);
    }

    #[test]
    fn overdue_tasks_use_warning_styles() {
        // GIVEN
        let overdue = Task::from_stored(
            Uuid::new_v4(),
            "overdue".to_string(),
            TaskStatus::Todo,
            datetime(),
            datetime(),
            None,
            NaiveDate::from_ymd_opt(2026, 6, 14).expect("test date should be valid"),
        );
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).expect("test date should be valid");
        let expected = Text::from(vec![
            Line::styled("overdue", Style::default().fg(Color::Yellow)),
            Line::styled(
                "Deadline: 2026/06/14",
                Style::default().fg(OVERDUE_DEADLINE_COLOR),
            ),
        ]);

        // WHEN
        let actual = task_text(&overdue, 20, today, false);

        // THEN
        assert_eq!(actual, expected);
    }

    #[test]
    fn selected_task_deadline_uses_the_selected_color() {
        // GIVEN
        let selected = task("selected", TaskStatus::Todo);
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).expect("test date should be valid");
        let expected = Text::from(vec![
            Line::from("selected"),
            Line::styled("Deadline: 2026/06/16", Style::default().fg(Color::Gray)),
        ]);

        // WHEN
        let actual = task_text(&selected, 20, today, true);

        // THEN
        assert_eq!(actual, expected);
    }
}
