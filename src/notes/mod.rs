use std::collections::BTreeSet;

use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use ulid::Ulid;

use crate::{
    clock::format_timestamp,
    error::{AppError, AppResult},
    patch::{PatchField, deserialize_patch_field},
};

const MAX_NOTE_BYTES: usize = 256 * 1024;
const MAX_TAG_CHARACTERS: usize = 64;
const MAX_TAGS_PER_NOTE: usize = 64;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NoteStatus {
    #[default]
    Active,
    Archived,
}

impl NoteStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "ACTIVE",
            Self::Archived => "ARCHIVED",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateNoteRequest {
    pub content: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateNoteRequest {
    #[serde(default, deserialize_with = "deserialize_patch_field")]
    pub content: PatchField<String>,
    #[serde(default, deserialize_with = "deserialize_patch_field")]
    pub status: PatchField<NoteStatus>,
    #[serde(default, deserialize_with = "deserialize_patch_field")]
    pub pinned: PatchField<bool>,
}

impl UpdateNoteRequest {
    pub fn is_empty(&self) -> bool {
        self.content.is_missing() && self.status.is_missing() && self.pinned.is_missing()
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub uid: String,
    pub content: String,
    pub status: NoteStatus,
    pub pinned: bool,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListNotesResponse {
    pub items: Vec<Note>,
    pub page: u32,
    pub page_size: u32,
    pub total: i64,
}

#[derive(FromRow)]
struct NoteRow {
    id: i64,
    uid: String,
    content: String,
    status: String,
    pinned: bool,
    created_at: i64,
    updated_at: i64,
}

pub async fn create(
    pool: &SqlitePool,
    workspace_id: i64,
    creator_id: i64,
    request: CreateNoteRequest,
    now: i64,
) -> AppResult<Note> {
    validate_content(&request.content)?;
    let tags = extract_tags(&request.content)?;
    let uid = Ulid::generate().to_string();
    let mut transaction = pool.begin().await?;
    let object = sqlx::query(
        r#"
        INSERT INTO objects
          (uid, workspace_id, creator_id, object_type, created_at, updated_at)
        VALUES (?, ?, ?, 'NOTE', ?, ?)
        "#,
    )
    .bind(&uid)
    .bind(workspace_id)
    .bind(creator_id)
    .bind(now)
    .bind(now)
    .execute(&mut *transaction)
    .await?;
    let object_id = object.last_insert_rowid();
    let result = sqlx::query(
        r#"
        INSERT INTO notes
          (object_id, uid, workspace_id, creator_id, content, status, pinned, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, 'ACTIVE', 0, ?, ?)
        "#,
    )
    .bind(object_id)
    .bind(&uid)
    .bind(workspace_id)
    .bind(creator_id)
    .bind(&request.content)
    .bind(now)
    .bind(now)
    .execute(&mut *transaction)
    .await?;
    let note_id = result.last_insert_rowid();
    insert_tags(&mut transaction, note_id, object_id, &tags).await?;
    transaction.commit().await?;
    get(pool, workspace_id, &uid).await
}

pub async fn list(
    pool: &SqlitePool,
    workspace_id: i64,
    status: NoteStatus,
    query: Option<&str>,
    tag: Option<&str>,
    page: u32,
    page_size: u32,
) -> AppResult<ListNotesResponse> {
    if page == 0 {
        return Err(AppError::bad_request("page must be at least 1"));
    }
    if !(1..=100).contains(&page_size) {
        return Err(AppError::bad_request("page_size must be between 1 and 100"));
    }
    let search_pattern = query
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(like_pattern);
    let normalized_tag = tag
        .map(normalize_tag)
        .transpose()?
        .filter(|value| !value.is_empty());
    let offset = i64::from(page.saturating_sub(1)) * i64::from(page_size);

    let rows = sqlx::query_as::<_, NoteRow>(
        r#"
        SELECT n.id, n.uid, n.content, n.status, n.pinned, n.created_at, n.updated_at
        FROM notes n
        WHERE n.workspace_id = ?
          AND n.status = ?
          AND (? IS NULL OR n.content LIKE ? ESCAPE '\')
          AND (
            ? IS NULL OR EXISTS (
              SELECT 1 FROM note_tags nt WHERE nt.note_id = n.id AND nt.tag = ?
            )
          )
        ORDER BY n.pinned DESC, n.created_at DESC, n.id DESC
        LIMIT ? OFFSET ?
        "#,
    )
    .bind(workspace_id)
    .bind(status.as_str())
    .bind(search_pattern.as_deref())
    .bind(search_pattern.as_deref())
    .bind(normalized_tag.as_deref())
    .bind(normalized_tag.as_deref())
    .bind(i64::from(page_size))
    .bind(offset)
    .fetch_all(pool)
    .await?;

    let total = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM notes n
        WHERE n.workspace_id = ?
          AND n.status = ?
          AND (? IS NULL OR n.content LIKE ? ESCAPE '\')
          AND (
            ? IS NULL OR EXISTS (
              SELECT 1 FROM note_tags nt WHERE nt.note_id = n.id AND nt.tag = ?
            )
          )
        "#,
    )
    .bind(workspace_id)
    .bind(status.as_str())
    .bind(search_pattern.as_deref())
    .bind(search_pattern.as_deref())
    .bind(normalized_tag.as_deref())
    .bind(normalized_tag.as_deref())
    .fetch_one(pool)
    .await?;

    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        items.push(row_to_note(pool, row).await?);
    }
    Ok(ListNotesResponse {
        items,
        page,
        page_size,
        total,
    })
}

pub async fn get(pool: &SqlitePool, workspace_id: i64, uid: &str) -> AppResult<Note> {
    let row = fetch_row(pool, workspace_id, uid)
        .await?
        .ok_or_else(|| AppError::not_found("Memo"))?;
    row_to_note(pool, row).await
}

pub async fn update(
    pool: &SqlitePool,
    workspace_id: i64,
    uid: &str,
    request: UpdateNoteRequest,
    now: i64,
) -> AppResult<Note> {
    if request.is_empty() {
        return Err(AppError::bad_request(
            "PATCH must include at least one editable field",
        ));
    }
    let UpdateNoteRequest {
        content,
        status,
        pinned,
    } = request;
    let content = content.into_required("content")?;
    let status = status.into_required("status")?;
    let pinned = pinned.into_required("pinned")?;
    if let Some(content) = &content {
        validate_content(content)?;
    }

    let status_value = status.map(NoteStatus::as_str);
    let tags = content.as_deref().map(extract_tags).transpose()?;
    let mut transaction = pool.begin().await?;
    let result = sqlx::query(
        r#"
        UPDATE notes SET
          content = CASE WHEN ? THEN ? ELSE content END,
          status = CASE WHEN ? THEN ? ELSE status END,
          pinned = CASE WHEN ? THEN ? ELSE pinned END,
          updated_at = ?
        WHERE workspace_id = ? AND uid = ?
        "#,
    )
    .bind(content.is_some())
    .bind(content.as_deref())
    .bind(status.is_some())
    .bind(status_value)
    .bind(pinned.is_some())
    .bind(pinned)
    .bind(now)
    .bind(workspace_id)
    .bind(uid)
    .execute(&mut *transaction)
    .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::not_found("Memo"));
    }
    let (note_id, object_id) = sqlx::query_as::<_, (i64, i64)>(
        "SELECT id, object_id FROM notes WHERE workspace_id = ? AND uid = ?",
    )
    .bind(workspace_id)
    .bind(uid)
    .fetch_one(&mut *transaction)
    .await?;
    if let Some(tags) = tags.as_deref() {
        sqlx::query("DELETE FROM note_tags WHERE note_id = ?")
            .bind(note_id)
            .execute(&mut *transaction)
            .await?;
        sqlx::query("DELETE FROM object_tags WHERE object_id = ?")
            .bind(object_id)
            .execute(&mut *transaction)
            .await?;
        insert_tags(&mut transaction, note_id, object_id, tags).await?;
    }
    let object_update = sqlx::query(
        "UPDATE objects SET updated_at = ? WHERE id = ? AND workspace_id = ? AND object_type = 'NOTE'",
    )
    .bind(now)
    .bind(object_id)
    .bind(workspace_id)
    .execute(&mut *transaction)
    .await?;
    if object_update.rows_affected() != 1 {
        return Err(AppError::Internal("Memo object is missing".to_owned()));
    }
    transaction.commit().await?;
    get(pool, workspace_id, uid).await
}

pub async fn delete(pool: &SqlitePool, workspace_id: i64, uid: &str) -> AppResult<()> {
    let result = sqlx::query(
        r#"
        DELETE FROM objects
        WHERE workspace_id = ? AND uid = ? AND object_type = 'NOTE'
          AND id IN (
            SELECT object_id FROM notes WHERE workspace_id = ? AND uid = ?
          )
        "#,
    )
    .bind(workspace_id)
    .bind(uid)
    .bind(workspace_id)
    .bind(uid)
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::not_found("Memo"));
    }
    Ok(())
}

pub async fn list_tags(pool: &SqlitePool, workspace_id: i64) -> AppResult<Vec<String>> {
    Ok(sqlx::query_scalar::<_, String>(
        r#"
        SELECT DISTINCT nt.tag
        FROM note_tags nt
        JOIN notes n ON n.id = nt.note_id
        WHERE n.workspace_id = ?
        ORDER BY nt.tag ASC
        "#,
    )
    .bind(workspace_id)
    .fetch_all(pool)
    .await?)
}

async fn fetch_row(pool: &SqlitePool, workspace_id: i64, uid: &str) -> AppResult<Option<NoteRow>> {
    Ok(sqlx::query_as::<_, NoteRow>(
        r#"
        SELECT id, uid, content, status, pinned, created_at, updated_at
        FROM notes WHERE workspace_id = ? AND uid = ?
        "#,
    )
    .bind(workspace_id)
    .bind(uid)
    .fetch_optional(pool)
    .await?)
}

async fn row_to_note(pool: &SqlitePool, row: NoteRow) -> AppResult<Note> {
    let status = match row.status.as_str() {
        "ACTIVE" => NoteStatus::Active,
        "ARCHIVED" => NoteStatus::Archived,
        value => {
            return Err(AppError::Internal(format!(
                "invalid stored note status: {value}"
            )));
        }
    };
    let tags = sqlx::query_scalar::<_, String>(
        "SELECT tag FROM note_tags WHERE note_id = ? ORDER BY tag ASC",
    )
    .bind(row.id)
    .fetch_all(pool)
    .await?;
    Ok(Note {
        uid: row.uid,
        content: row.content,
        status,
        pinned: row.pinned,
        tags,
        created_at: format_timestamp(row.created_at)?,
        updated_at: format_timestamp(row.updated_at)?,
    })
}

async fn insert_tags(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    note_id: i64,
    object_id: i64,
    tags: &[String],
) -> AppResult<()> {
    for tag in tags {
        sqlx::query("INSERT INTO note_tags (note_id, tag) VALUES (?, ?)")
            .bind(note_id)
            .bind(tag)
            .execute(&mut **transaction)
            .await?;
        sqlx::query("INSERT INTO object_tags (object_id, tag) VALUES (?, ?)")
            .bind(object_id)
            .bind(tag)
            .execute(&mut **transaction)
            .await?;
    }
    Ok(())
}

fn validate_content(content: &str) -> AppResult<()> {
    if content.trim().is_empty() {
        return Err(AppError::validation("Memo content must not be empty"));
    }
    if content.len() > MAX_NOTE_BYTES {
        return Err(AppError::validation(format!(
            "Memo content must not exceed {MAX_NOTE_BYTES} bytes"
        )));
    }
    Ok(())
}

fn like_pattern(value: &str) -> String {
    let escaped = value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
}

fn normalize_tag(value: &str) -> AppResult<String> {
    let normalized = value.trim().trim_start_matches('#').to_lowercase();
    if normalized.is_empty() || !normalized.chars().all(is_tag_character) {
        return Err(AppError::bad_request("tag is invalid"));
    }
    if normalized.chars().count() > MAX_TAG_CHARACTERS {
        return Err(AppError::bad_request(format!(
            "tag must not exceed {MAX_TAG_CHARACTERS} characters"
        )));
    }
    Ok(normalized)
}

fn extract_tags(content: &str) -> AppResult<Vec<String>> {
    let mut tags = BTreeSet::new();
    let mut code_block_depth = 0_u32;
    for event in Parser::new(content) {
        match event {
            Event::Start(Tag::CodeBlock(_)) => code_block_depth += 1,
            Event::End(TagEnd::CodeBlock) => code_block_depth = code_block_depth.saturating_sub(1),
            Event::Text(text) if code_block_depth == 0 => scan_tags(&text, &mut tags)?,
            _ => {}
        }
    }
    Ok(tags.into_iter().collect())
}

fn scan_tags(text: &str, tags: &mut BTreeSet<String>) -> AppResult<()> {
    let characters: Vec<char> = text.chars().collect();
    let mut index = 0;
    while index < characters.len() {
        if characters[index] != '#'
            || (index > 0 && !is_tag_boundary(characters[index - 1]))
            || index + 1 >= characters.len()
            || !characters[index + 1].is_alphanumeric()
        {
            index += 1;
            continue;
        }
        let start = index + 1;
        let mut end = start;
        while end < characters.len() && is_tag_character(characters[end]) {
            end += 1;
        }
        let raw: String = characters[start..end].iter().collect();
        let normalized = raw.trim_end_matches(['-', '/']).to_lowercase();
        if !normalized.is_empty() {
            if normalized.chars().count() > MAX_TAG_CHARACTERS {
                return Err(AppError::validation(format!(
                    "Tags must not exceed {MAX_TAG_CHARACTERS} characters"
                )));
            }
            tags.insert(normalized);
            if tags.len() > MAX_TAGS_PER_NOTE {
                return Err(AppError::validation(format!(
                    "A memo must not contain more than {MAX_TAGS_PER_NOTE} unique tags"
                )));
            }
        }
        index = end;
    }
    Ok(())
}

fn is_tag_character(character: char) -> bool {
    character.is_alphanumeric() || matches!(character, '_' | '-' | '/')
}

fn is_tag_boundary(character: char) -> bool {
    character != '#' && !is_tag_character(character)
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_NOTE_BYTES, MAX_TAG_CHARACTERS, MAX_TAGS_PER_NOTE, extract_tags, normalize_tag,
        validate_content,
    };

    #[test]
    fn extracts_normalized_tags_outside_code() {
        assert_eq!(
            extract_tags("Hello #Rust #中文，#标点 #rust\n`#inline`\n```sh\n#blocked\n```")
                .unwrap(),
            vec!["rust", "中文", "标点"]
        );
    }

    #[test]
    fn enforces_tag_character_and_unique_count_limits() {
        let exact = "界".repeat(MAX_TAG_CHARACTERS);
        assert_eq!(extract_tags(&format!("#{exact}")).unwrap(), vec![exact]);
        assert!(extract_tags(&format!("#{}", "界".repeat(MAX_TAG_CHARACTERS + 1))).is_err());
        assert!(normalize_tag(&"a".repeat(MAX_TAG_CHARACTERS)).is_ok());
        assert!(normalize_tag(&"a".repeat(MAX_TAG_CHARACTERS + 1)).is_err());

        let maximum = (0..MAX_TAGS_PER_NOTE)
            .map(|index| format!("#tag{index}"))
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(extract_tags(&maximum).unwrap().len(), MAX_TAGS_PER_NOTE);
        assert!(extract_tags(&format!("{maximum} #overflow")).is_err());
        assert_eq!(extract_tags("#same #same #SAME").unwrap(), vec!["same"]);
    }

    #[test]
    fn validates_content_by_trimmed_value_and_bytes() {
        assert!(validate_content(" \n ").is_err());
        assert!(validate_content(&"a".repeat(MAX_NOTE_BYTES)).is_ok());
        assert!(validate_content(&"a".repeat(MAX_NOTE_BYTES + 1)).is_err());
    }
}
