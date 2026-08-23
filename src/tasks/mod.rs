use chrono::{NaiveDate, NaiveTime};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use ulid::Ulid;

use crate::{
    clock::{format_timestamp, local_day_bounds, today},
    error::{AppError, AppResult},
    patch::{PatchField, deserialize_patch_field},
};

const MAX_TITLE_CHARACTERS: usize = 500;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TaskStatus {
    #[default]
    Todo,
    Done,
}

impl TaskStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Todo => "TODO",
            Self::Done => "DONE",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskScope {
    Today,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CreateTaskRequest {
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub priority: u8,
    pub due_date: Option<String>,
    pub due_time: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct UpdateTaskRequest {
    #[serde(default, deserialize_with = "deserialize_patch_field")]
    pub title: PatchField<String>,
    #[serde(default, deserialize_with = "deserialize_patch_field")]
    pub description: PatchField<String>,
    #[serde(default, deserialize_with = "deserialize_patch_field")]
    pub status: PatchField<TaskStatus>,
    #[serde(default, deserialize_with = "deserialize_patch_field")]
    pub priority: PatchField<u8>,
    #[serde(default, deserialize_with = "deserialize_patch_field")]
    pub due_date: PatchField<String>,
    #[serde(default, deserialize_with = "deserialize_patch_field")]
    pub due_time: PatchField<String>,
    #[serde(default, deserialize_with = "deserialize_patch_field")]
    pub sort_key: PatchField<i64>,
}

impl UpdateTaskRequest {
    pub fn is_empty(&self) -> bool {
        self.title.is_missing()
            && self.description.is_missing()
            && self.status.is_missing()
            && self.priority.is_missing()
            && self.due_date.is_missing()
            && self.due_time.is_missing()
            && self.sort_key.is_missing()
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub uid: String,
    pub title: String,
    pub description: String,
    pub status: TaskStatus,
    pub priority: u8,
    pub due_date: Option<String>,
    pub due_time: Option<String>,
    pub sort_key: i64,
    pub completed_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct ListTasksResponse {
    pub items: Vec<Task>,
}

#[derive(FromRow)]
struct TaskRow {
    uid: String,
    title: String,
    description: String,
    status: String,
    priority: i64,
    due_date: Option<String>,
    due_time: Option<String>,
    sort_key: i64,
    completed_at: Option<i64>,
    created_at: i64,
    updated_at: i64,
}

pub async fn create(
    pool: &SqlitePool,
    workspace_id: i64,
    creator_id: i64,
    request: CreateTaskRequest,
    now: i64,
) -> AppResult<Task> {
    validate_title(&request.title)?;
    validate_priority(request.priority)?;
    validate_schedule(request.due_date.as_deref(), request.due_time.as_deref())?;
    let uid = Ulid::generate().to_string();
    sqlx::query(
        r#"
        INSERT INTO tasks (
          uid, workspace_id, creator_id, title, description, status, priority,
          due_date, due_time, sort_key, completed_at, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, 'TODO', ?, ?, ?, 0, NULL, ?, ?)
        "#,
    )
    .bind(&uid)
    .bind(workspace_id)
    .bind(creator_id)
    .bind(&request.title)
    .bind(&request.description)
    .bind(i64::from(request.priority))
    .bind(request.due_date.as_deref())
    .bind(request.due_time.as_deref())
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    get(pool, workspace_id, &uid).await
}

pub async fn list(
    pool: &SqlitePool,
    workspace_id: i64,
    scope: Option<TaskScope>,
    status: Option<TaskStatus>,
    now: chrono::DateTime<chrono::Utc>,
    timezone: chrono_tz::Tz,
) -> AppResult<ListTasksResponse> {
    let today = today(now, timezone);
    let (day_start, day_end) = local_day_bounds(today, timezone)?;
    let is_today = i64::from(matches!(scope, Some(TaskScope::Today)));
    let rows = sqlx::query_as::<_, TaskRow>(
        r#"
        SELECT
          uid, title, description, status, priority, due_date, due_time, sort_key,
          completed_at, created_at, updated_at
        FROM tasks
        WHERE workspace_id = ?
          AND (
            ? = 0 OR (
              (status = 'TODO' AND due_date IS NOT NULL AND due_date <= ?)
              OR (status = 'DONE' AND completed_at >= ? AND completed_at < ?)
            )
          )
          AND (? IS NULL OR status = ?)
        ORDER BY
          CASE status WHEN 'TODO' THEN 0 ELSE 1 END ASC,
          priority DESC,
          CASE WHEN due_date IS NULL THEN 1 ELSE 0 END ASC,
          due_date ASC,
          CASE WHEN due_time IS NULL THEN 1 ELSE 0 END ASC,
          due_time ASC,
          sort_key ASC,
          created_at ASC,
          id ASC
        "#,
    )
    .bind(workspace_id)
    .bind(is_today)
    .bind(today.to_string())
    .bind(day_start)
    .bind(day_end)
    .bind(status.map(TaskStatus::as_str))
    .bind(status.map(TaskStatus::as_str))
    .fetch_all(pool)
    .await?;

    let items = rows
        .into_iter()
        .map(row_to_task)
        .collect::<AppResult<Vec<_>>>()?;
    Ok(ListTasksResponse { items })
}

pub async fn get(pool: &SqlitePool, workspace_id: i64, uid: &str) -> AppResult<Task> {
    let row = fetch_row(pool, workspace_id, uid)
        .await?
        .ok_or_else(|| AppError::not_found("Task"))?;
    row_to_task(row)
}

pub async fn update(
    pool: &SqlitePool,
    workspace_id: i64,
    uid: &str,
    request: UpdateTaskRequest,
    now: i64,
) -> AppResult<Task> {
    if request.is_empty() {
        return Err(AppError::bad_request(
            "PATCH must include at least one editable field",
        ));
    }
    let UpdateTaskRequest {
        title,
        description,
        status,
        priority,
        due_date,
        due_time,
        sort_key,
    } = request;
    let title = title.into_required("title")?;
    let description = description.into_required("description")?;
    let status = status.into_required("status")?;
    let priority = priority.into_required("priority")?;
    let sort_key = sort_key.into_required("sortKey")?;
    let due_date = due_date.into_nullable();
    let due_time = due_time.into_nullable();

    if let Some(title) = title.as_deref() {
        validate_title(title)?;
    }
    if let Some(priority) = priority {
        validate_priority(priority)?;
    }
    if let Some(Some(value)) = due_date.as_ref() {
        validate_due_date(value)?;
    }
    if let Some(Some(value)) = due_time.as_ref() {
        validate_due_time(value)?;
    }

    let status_value = status.map(TaskStatus::as_str);
    let priority_value = priority.map(i64::from);
    let due_date_value = due_date
        .as_ref()
        .and_then(|value| value.as_ref().map(String::as_str));
    let due_time_value = due_time
        .as_ref()
        .and_then(|value| value.as_ref().map(String::as_str));
    let result = sqlx::query(
        r#"
        UPDATE tasks
        SET
          title = CASE WHEN ? THEN ? ELSE title END,
          description = CASE WHEN ? THEN ? ELSE description END,
          status = CASE WHEN ? THEN ? ELSE status END,
          priority = CASE WHEN ? THEN ? ELSE priority END,
          due_date = CASE WHEN ? THEN ? ELSE due_date END,
          due_time = CASE WHEN ? THEN ? ELSE due_time END,
          sort_key = CASE WHEN ? THEN ? ELSE sort_key END,
          completed_at = CASE
            WHEN ? = 0 THEN completed_at
            WHEN ? = 'DONE' THEN COALESCE(completed_at, ?)
            ELSE NULL
          END,
          updated_at = ?
        WHERE workspace_id = ? AND uid = ?
          AND (
            (CASE WHEN ? THEN ? ELSE due_time END) IS NULL
            OR (CASE WHEN ? THEN ? ELSE due_date END) IS NOT NULL
          )
        "#,
    )
    .bind(title.is_some())
    .bind(title.as_deref())
    .bind(description.is_some())
    .bind(description.as_deref())
    .bind(status.is_some())
    .bind(status_value)
    .bind(priority.is_some())
    .bind(priority_value)
    .bind(due_date.is_some())
    .bind(due_date_value)
    .bind(due_time.is_some())
    .bind(due_time_value)
    .bind(sort_key.is_some())
    .bind(sort_key)
    .bind(i64::from(status.is_some()))
    .bind(status_value)
    .bind(now)
    .bind(now)
    .bind(workspace_id)
    .bind(uid)
    .bind(due_time.is_some())
    .bind(due_time_value)
    .bind(due_date.is_some())
    .bind(due_date_value)
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        let exists = sqlx::query_scalar::<_, i64>(
            "SELECT EXISTS(SELECT 1 FROM tasks WHERE workspace_id = ? AND uid = ?)",
        )
        .bind(workspace_id)
        .bind(uid)
        .fetch_one(pool)
        .await?;
        if exists == 0 {
            return Err(AppError::not_found("Task"));
        }
        return Err(AppError::validation("dueTime requires dueDate"));
    }
    get(pool, workspace_id, uid).await
}

pub async fn delete(pool: &SqlitePool, workspace_id: i64, uid: &str) -> AppResult<()> {
    let result = sqlx::query("DELETE FROM tasks WHERE workspace_id = ? AND uid = ?")
        .bind(workspace_id)
        .bind(uid)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::not_found("Task"));
    }
    Ok(())
}

async fn fetch_row(pool: &SqlitePool, workspace_id: i64, uid: &str) -> AppResult<Option<TaskRow>> {
    Ok(sqlx::query_as::<_, TaskRow>(
        r#"
        SELECT
          uid, title, description, status, priority, due_date, due_time, sort_key,
          completed_at, created_at, updated_at
        FROM tasks WHERE workspace_id = ? AND uid = ?
        "#,
    )
    .bind(workspace_id)
    .bind(uid)
    .fetch_optional(pool)
    .await?)
}

fn row_to_task(row: TaskRow) -> AppResult<Task> {
    Ok(Task {
        uid: row.uid,
        title: row.title,
        description: row.description,
        status: parse_status(&row.status)?,
        priority: u8::try_from(row.priority).map_err(|_| {
            AppError::Internal(format!("invalid stored task priority: {}", row.priority))
        })?,
        due_date: row.due_date,
        due_time: row.due_time,
        sort_key: row.sort_key,
        completed_at: row.completed_at.map(format_timestamp).transpose()?,
        created_at: format_timestamp(row.created_at)?,
        updated_at: format_timestamp(row.updated_at)?,
    })
}

fn parse_status(value: &str) -> AppResult<TaskStatus> {
    match value {
        "TODO" => Ok(TaskStatus::Todo),
        "DONE" => Ok(TaskStatus::Done),
        _ => Err(AppError::Internal(format!(
            "invalid stored task status: {value}"
        ))),
    }
}

fn validate_title(title: &str) -> AppResult<()> {
    if title.trim().is_empty() {
        return Err(AppError::validation("Task title must not be empty"));
    }
    if title.chars().count() > MAX_TITLE_CHARACTERS {
        return Err(AppError::validation(format!(
            "Task title must not exceed {MAX_TITLE_CHARACTERS} characters"
        )));
    }
    Ok(())
}

fn validate_priority(priority: u8) -> AppResult<()> {
    if priority > 1 {
        return Err(AppError::validation("Task priority must be 0 or 1"));
    }
    Ok(())
}

fn validate_schedule(due_date: Option<&str>, due_time: Option<&str>) -> AppResult<()> {
    if let Some(value) = due_date {
        validate_due_date(value)?;
    }
    if let Some(value) = due_time {
        if due_date.is_none() {
            return Err(AppError::validation("dueTime requires dueDate"));
        }
        validate_due_time(value)?;
    }
    Ok(())
}

fn validate_due_date(value: &str) -> AppResult<()> {
    if value.len() != 10 || NaiveDate::parse_from_str(value, "%Y-%m-%d").is_err() {
        return Err(AppError::validation(
            "dueDate must use the YYYY-MM-DD format",
        ));
    }
    Ok(())
}

fn validate_due_time(value: &str) -> AppResult<()> {
    if value.len() != 5 || NaiveTime::parse_from_str(value, "%H:%M").is_err() {
        return Err(AppError::validation("dueTime must use the HH:mm format"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{UpdateTaskRequest, validate_schedule, validate_title};
    use crate::patch::PatchField;

    #[test]
    fn patch_distinguishes_missing_null_and_value() {
        let missing: UpdateTaskRequest = serde_json::from_value(json!({"title": "Next"})).unwrap();
        assert!(matches!(missing.due_date, PatchField::Missing));
        assert!(matches!(missing.title, PatchField::Value(_)));

        let null: UpdateTaskRequest = serde_json::from_value(json!({"dueDate": null})).unwrap();
        assert!(matches!(null.due_date, PatchField::Null));

        let null_title: UpdateTaskRequest = serde_json::from_value(json!({"title": null})).unwrap();
        assert!(matches!(null_title.title, PatchField::Null));

        let value: UpdateTaskRequest =
            serde_json::from_value(json!({"dueDate": "2026-08-23"})).unwrap();
        assert!(matches!(value.due_date, PatchField::Value(_)));
    }

    #[test]
    fn validates_unicode_titles_and_strict_calendar_values() {
        assert!(validate_title(" ").is_err());
        assert!(validate_title(&"界".repeat(500)).is_ok());
        assert!(validate_title(&"界".repeat(501)).is_err());
        assert!(validate_schedule(Some("2026-02-29"), None).is_err());
        assert!(validate_schedule(None, Some("09:30")).is_err());
        assert!(validate_schedule(Some("2026-08-23"), Some("9:30")).is_err());
        assert!(validate_schedule(Some("2026-08-23"), Some("09:30")).is_ok());
    }
}
