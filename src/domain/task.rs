use chrono::{Datelike, Days, NaiveDate, NaiveDateTime};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use uuid::Uuid;

pub(crate) const DEADLINE_DATE_FORMAT: &str = "%Y/%m/%d";
pub(crate) const TASK_DATETIME_FORMAT: &str = "%Y/%m/%d %H:%M:%S";

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum TaskStatus {
    Parking,
    Todo,
    Doing,
    Done,
}

impl TaskStatus {
    pub(crate) const ALL: [Self; 4] = [Self::Parking, Self::Todo, Self::Doing, Self::Done];

    pub(crate) fn transition(self, direction: StatusDirection) -> Option<Self> {
        match (self, direction) {
            (Self::Parking, StatusDirection::Forward) => Some(Self::Todo),
            (Self::Todo, StatusDirection::Forward) => Some(Self::Doing),
            (Self::Doing, StatusDirection::Forward) => Some(Self::Done),
            (Self::Done, StatusDirection::Forward) => None,
            (Self::Parking, StatusDirection::Backward) => None,
            (Self::Todo, StatusDirection::Backward) => Some(Self::Parking),
            (Self::Doing, StatusDirection::Backward) => Some(Self::Todo),
            (Self::Done, StatusDirection::Backward) => Some(Self::Doing),
        }
    }

    fn sort_order(self) -> u8 {
        match self {
            Self::Parking => 0,
            Self::Todo => 1,
            Self::Doing => 2,
            Self::Done => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StatusDirection {
    Forward,
    Backward,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct WeekRange {
    start: NaiveDate,
    end_exclusive: NaiveDate,
}

impl WeekRange {
    pub(crate) fn containing(date: NaiveDate) -> Result<Self, TaskDateError> {
        const WEEK_LENGTH: u64 = 7;
        const MONDAY_OFFSET_LIMIT: u64 = 6;
        let monday_offset = u64::from(date.weekday().num_days_from_monday());
        let start = date
            .checked_sub_days(Days::new(monday_offset.min(MONDAY_OFFSET_LIMIT)))
            .ok_or(TaskDateError::new(
                "week start is outside the supported date range",
            ))?;
        let end_exclusive =
            start
                .checked_add_days(Days::new(WEEK_LENGTH))
                .ok_or(TaskDateError::new(
                    "week end is outside the supported date range",
                ))?;
        Ok(Self {
            start,
            end_exclusive,
        })
    }

    pub(crate) fn previous(self) -> Result<Self, TaskDateError> {
        const WEEK_LENGTH: u64 = 7;
        let previous_date =
            self.start
                .checked_sub_days(Days::new(WEEK_LENGTH))
                .ok_or(TaskDateError::new(
                    "previous week is outside the supported date range",
                ))?;
        Self::containing(previous_date)
    }

    pub(crate) fn next(self) -> Result<Self, TaskDateError> {
        Self::containing(self.end_exclusive)
    }

    pub(crate) fn start(self) -> NaiveDate {
        self.start
    }

    pub(crate) fn end_inclusive(self) -> NaiveDate {
        self.end_exclusive.pred_opt().unwrap_or(self.start)
    }

    pub(crate) fn contains(self, date: NaiveDate) -> bool {
        date >= self.start && date < self.end_exclusive
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TaskDateError {
    message: &'static str,
}

impl TaskDateError {
    const fn new(message: &'static str) -> Self {
        Self { message }
    }
}

impl Display for TaskDateError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message)
    }
}

impl Error for TaskDateError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Task {
    id: Uuid,
    name: String,
    status: TaskStatus,
    created_at: NaiveDateTime,
    updated_at: NaiveDateTime,
    completed_at: Option<NaiveDateTime>,
    deadline: NaiveDate,
}

impl Task {
    pub(crate) fn new(name: String, now: NaiveDateTime) -> Result<Self, TaskDateError> {
        const DEFAULT_DEADLINE_DAYS: u64 = 1;
        let deadline = now
            .date()
            .checked_add_days(Days::new(DEFAULT_DEADLINE_DAYS))
            .ok_or(TaskDateError::new(
                "task deadline is outside the supported date range",
            ))?;
        Ok(Self {
            id: Uuid::new_v4(),
            name,
            status: TaskStatus::Todo,
            created_at: now,
            updated_at: now,
            completed_at: None,
            deadline,
        })
    }

    pub(crate) fn from_stored(
        id: Uuid,
        name: String,
        status: TaskStatus,
        created_at: NaiveDateTime,
        updated_at: NaiveDateTime,
        completed_at: Option<NaiveDateTime>,
        deadline: NaiveDate,
    ) -> Self {
        Self {
            id,
            name,
            status,
            created_at,
            updated_at,
            completed_at,
            deadline,
        }
    }

    pub(crate) fn transitioned(
        &self,
        direction: StatusDirection,
        updated_at: NaiveDateTime,
    ) -> Option<Self> {
        let status = self.status.transition(direction)?;
        let completed_at = match (self.status, status) {
            (TaskStatus::Doing, TaskStatus::Done) => Some(updated_at),
            (TaskStatus::Done, TaskStatus::Doing) => None,
            (TaskStatus::Parking, TaskStatus::Todo)
            | (TaskStatus::Todo, TaskStatus::Parking)
            | (TaskStatus::Todo, TaskStatus::Doing)
            | (TaskStatus::Doing, TaskStatus::Todo) => self.completed_at,
            (TaskStatus::Parking, TaskStatus::Parking)
            | (TaskStatus::Parking, TaskStatus::Doing)
            | (TaskStatus::Parking, TaskStatus::Done)
            | (TaskStatus::Todo, TaskStatus::Todo)
            | (TaskStatus::Todo, TaskStatus::Done)
            | (TaskStatus::Doing, TaskStatus::Parking)
            | (TaskStatus::Doing, TaskStatus::Doing)
            | (TaskStatus::Done, TaskStatus::Parking)
            | (TaskStatus::Done, TaskStatus::Todo)
            | (TaskStatus::Done, TaskStatus::Done) => self.completed_at,
        };
        Some(Self {
            status,
            updated_at,
            completed_at,
            ..self.clone()
        })
    }

    pub(crate) fn id(&self) -> Uuid {
        self.id
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn status(&self) -> TaskStatus {
        self.status
    }

    pub(crate) fn created_at(&self) -> NaiveDateTime {
        self.created_at
    }

    pub(crate) fn updated_at(&self) -> NaiveDateTime {
        self.updated_at
    }

    pub(crate) fn completed_at(&self) -> Option<NaiveDateTime> {
        self.completed_at
    }

    pub(crate) fn deadline(&self) -> NaiveDate {
        self.deadline
    }

    pub(crate) fn sorted(mut tasks: Vec<Self>) -> Vec<Self> {
        tasks.sort_by_key(|task| (task.status.sort_order(), task.created_at));
        tasks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn datetime(year: i32, month: u32, day: u32, hour: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(year, month, day)
            .and_then(|date| date.and_hms_opt(hour, 0, 0))
            .expect("test datetime should be valid")
    }

    fn task(status: TaskStatus, created_at: NaiveDateTime) -> Task {
        Task::from_stored(
            Uuid::new_v4(),
            format!("{status:?}"),
            status,
            created_at,
            created_at,
            None,
            created_at.date(),
        )
    }

    #[test]
    fn status_transitions_follow_the_lifecycle() {
        // GIVEN
        let statuses = TaskStatus::ALL;

        // WHEN
        let actual = statuses.map(|status| {
            (
                status.transition(StatusDirection::Forward),
                status.transition(StatusDirection::Backward),
            )
        });

        // THEN
        let expected = [
            (Some(TaskStatus::Todo), None),
            (Some(TaskStatus::Doing), Some(TaskStatus::Parking)),
            (Some(TaskStatus::Done), Some(TaskStatus::Todo)),
            (None, Some(TaskStatus::Doing)),
        ];
        assert_eq!(actual, expected);
    }

    #[test]
    fn completing_and_reopening_a_task_updates_completion_metadata_immutably() {
        // GIVEN
        let created_at = datetime(2026, 6, 15, 9);
        let completed_at = datetime(2026, 6, 16, 10);
        let doing = task(TaskStatus::Doing, created_at);

        // WHEN
        let done = doing
            .transitioned(StatusDirection::Forward, completed_at)
            .expect("DOING should transition to DONE");
        let reopened = done
            .transitioned(StatusDirection::Backward, datetime(2026, 6, 17, 11))
            .expect("DONE should transition to DOING");

        // THEN
        assert_eq!(doing.status(), TaskStatus::Doing);
        assert_eq!(doing.completed_at(), None);
        assert_eq!(done.status(), TaskStatus::Done);
        assert_eq!(done.completed_at(), Some(completed_at));
        assert_eq!(reopened.status(), TaskStatus::Doing);
        assert_eq!(reopened.completed_at(), None);
    }

    #[test]
    fn week_range_spans_monday_through_sunday() {
        // GIVEN
        let date = NaiveDate::from_ymd_opt(2026, 6, 21).expect("test date should be valid");

        // WHEN
        let actual = WeekRange::containing(date).expect("week should be representable");

        // THEN
        let expected_start =
            NaiveDate::from_ymd_opt(2026, 6, 15).expect("test date should be valid");
        let expected_end = NaiveDate::from_ymd_opt(2026, 6, 21).expect("test date should be valid");
        assert_eq!(actual.start(), expected_start);
        assert_eq!(actual.end_inclusive(), expected_end);
        assert!(actual.contains(date));
    }

    #[test]
    fn dates_outside_a_complete_week_return_an_error() {
        // GIVEN
        let date = NaiveDate::MAX;

        // WHEN
        let actual = WeekRange::containing(date);

        // THEN
        assert_eq!(
            actual,
            Err(TaskDateError::new(
                "week end is outside the supported date range"
            ))
        );
    }

    #[test]
    fn sorted_orders_status_groups_then_creation_time() {
        // GIVEN
        let early = datetime(2026, 6, 15, 9);
        let late = datetime(2026, 6, 15, 10);
        let tasks = vec![
            task(TaskStatus::Done, early),
            task(TaskStatus::Todo, late),
            task(TaskStatus::Parking, late),
            task(TaskStatus::Todo, early),
        ];

        // WHEN
        let actual: Vec<_> = Task::sorted(tasks)
            .into_iter()
            .map(|task| (task.status(), task.created_at()))
            .collect();

        // THEN
        let expected = vec![
            (TaskStatus::Parking, late),
            (TaskStatus::Todo, early),
            (TaskStatus::Todo, late),
            (TaskStatus::Done, early),
        ];
        assert_eq!(actual, expected);
    }
}
