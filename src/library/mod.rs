use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Sqlite, SqlitePool, Transaction};
use ulid::Ulid;
use url::Url;

use crate::{
    clock::format_timestamp,
    error::{AppError, AppResult},
    jobs,
    patch::{PatchField, deserialize_patch_field},
};

const MAX_URL_BYTES: usize = 8_192;
const MAX_TITLE_CHARACTERS: usize = 1_000;
const MAX_SITE_NAME_CHARACTERS: usize = 255;
const MAX_CAPTURE_BYTES: usize = 262_144;
const MAX_IDEMPOTENCY_KEY_CHARACTERS: usize = 255;
const MAX_TAG_CHARACTERS: usize = 64;
const MAX_TAGS: usize = 64;
const DEFAULT_PAGE_SIZE: u32 = 30;
const MAX_PAGE_SIZE: u32 = 100;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LibraryStatus {
    #[default]
    Active,
    Archived,
}

impl LibraryStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "ACTIVE",
            Self::Archived => "ARCHIVED",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LibraryItemKind {
    Bookmark,
    Article,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LibraryProcessingStatus {
    NotFetched,
    Pending,
    Ready,
    Failed,
}

impl LibraryProcessingStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotFetched => "NOT_FETCHED",
            Self::Pending => "PENDING",
            Self::Ready => "READY",
            Self::Failed => "FAILED",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LibraryRefreshStatus {
    Idle,
    Pending,
    Failed,
    Review,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CreateLibraryItemRequest {
    pub url: String,
    pub title: Option<String>,
    pub selection: Option<String>,
    pub note: Option<String>,
    pub tags: Option<Vec<String>>,
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct UpdateLibraryItemRequest {
    #[serde(default, deserialize_with = "deserialize_patch_field")]
    pub title: PatchField<String>,
    #[serde(default, deserialize_with = "deserialize_patch_field")]
    pub status: PatchField<LibraryStatus>,
    #[serde(default, deserialize_with = "deserialize_patch_field")]
    pub read: PatchField<bool>,
    #[serde(default, deserialize_with = "deserialize_patch_field")]
    pub starred: PatchField<bool>,
    #[serde(default, deserialize_with = "deserialize_patch_field")]
    pub tags: PatchField<Vec<String>>,
}

impl UpdateLibraryItemRequest {
    fn is_empty(&self) -> bool {
        self.title.is_missing()
            && self.status.is_missing()
            && self.read.is_missing()
            && self.starred.is_missing()
            && self.tags.is_missing()
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryCapture {
    pub uid: String,
    pub selected_text: String,
    pub note: String,
    pub captured_title: Option<String>,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryItem {
    pub uid: String,
    pub original_url: String,
    pub normalized_url: String,
    pub canonical_url: Option<String>,
    pub title: String,
    pub site_name: Option<String>,
    pub author: Option<String>,
    pub published_at: Option<String>,
    pub excerpt: String,
    pub item_kind: LibraryItemKind,
    pub status: LibraryStatus,
    pub read_at: Option<String>,
    pub starred: bool,
    pub processing_status: LibraryProcessingStatus,
    pub last_error: Option<String>,
    pub refresh_status: LibraryRefreshStatus,
    pub refresh_error: Option<String>,
    pub fetched_at: Option<String>,
    pub content_version: u32,
    pub content_available: bool,
    pub current_text_byte_len: Option<u64>,
    pub candidate_content_version: Option<u32>,
    pub candidate_text_byte_len: Option<u64>,
    pub tags: Vec<String>,
    pub captures: Vec<LibraryCapture>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryContent {
    pub safe_html: String,
    pub plain_text: String,
    pub fetched_at: String,
    pub content_version: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListLibraryItemsResponse {
    pub items: Vec<LibraryItem>,
    pub page: u32,
    pub page_size: u32,
    pub total: i64,
}

#[derive(Clone, Copy, Debug)]
pub struct ListLibraryItemsOptions<'a> {
    pub status: LibraryStatus,
    pub query: Option<&'a str>,
    pub tag: Option<&'a str>,
    pub read: Option<bool>,
    pub starred: Option<bool>,
    pub page: u32,
    pub page_size: u32,
}

impl Default for ListLibraryItemsOptions<'_> {
    fn default() -> Self {
        Self {
            status: LibraryStatus::Active,
            query: None,
            tag: None,
            read: None,
            starred: None,
            page: 1,
            page_size: DEFAULT_PAGE_SIZE,
        }
    }
}

#[derive(Debug)]
pub struct CreateLibraryItemOutcome {
    pub item: LibraryItem,
    pub created: bool,
}

#[derive(Debug, FromRow)]
struct LibraryItemRow {
    id: i64,
    object_id: i64,
    uid: String,
    original_url: String,
    normalized_url: String,
    canonical_url: Option<String>,
    title: String,
    site_name: Option<String>,
    author: Option<String>,
    published_at: Option<i64>,
    excerpt: String,
    item_kind: String,
    status: String,
    read_at: Option<i64>,
    starred: bool,
    processing_status: String,
    last_error: Option<String>,
    refresh_status: String,
    refresh_error: Option<String>,
    fetched_at: Option<i64>,
    content_version: i64,
    content_available: bool,
    current_text_byte_len: Option<i64>,
    candidate_content_version: Option<i64>,
    candidate_text_byte_len: Option<i64>,
    created_at: i64,
    updated_at: i64,
}

#[derive(Debug, FromRow)]
struct LibraryCaptureRow {
    uid: String,
    selected_text: String,
    note: String,
    captured_title: Option<String>,
    created_at: i64,
}

#[derive(Debug, FromRow)]
struct LibraryContentRow {
    safe_html: Vec<u8>,
    plain_text: Vec<u8>,
    fetched_at: Option<i64>,
    content_version: i64,
}

#[derive(Debug)]
struct NormalizedUrl {
    original: String,
    normalized: String,
    site_name: Option<String>,
}

pub async fn create(
    pool: &SqlitePool,
    workspace_id: i64,
    creator_id: i64,
    request: CreateLibraryItemRequest,
    now: i64,
) -> AppResult<CreateLibraryItemOutcome> {
    let normalized_url = normalize_url(&request.url)?;
    let title = request.title.unwrap_or_default();
    validate_title(&title)?;
    let selection = request.selection.unwrap_or_default();
    let note = request.note.unwrap_or_default();
    validate_capture_text("selection", &selection)?;
    validate_capture_text("note", &note)?;
    let tags = normalize_tags(request.tags.unwrap_or_default())?;
    let idempotency_key = normalize_idempotency_key(request.idempotency_key)?;

    let mut transaction = pool.begin().await?;
    acquire_workspace_write_lock(&mut transaction, workspace_id).await?;
    if let Some(key) = idempotency_key.as_deref()
        && let Some((uid, existing_url)) =
            find_idempotent_item(&mut transaction, workspace_id, key).await?
    {
        transaction.commit().await?;
        if existing_url != normalized_url.normalized {
            return Err(AppError::conflict(
                "Idempotency key was already used for a different URL",
            ));
        }
        return Ok(CreateLibraryItemOutcome {
            item: get(pool, workspace_id, &uid).await?,
            created: false,
        });
    }

    let object_uid = Ulid::generate().to_string();
    let object_result = sqlx::query(
        r#"
        INSERT INTO objects
          (uid, workspace_id, creator_id, object_type, created_at, updated_at)
        VALUES (?, ?, ?, 'LIBRARY_ITEM', ?, ?)
        "#,
    )
    .bind(&object_uid)
    .bind(workspace_id)
    .bind(creator_id)
    .bind(now)
    .bind(now)
    .execute(&mut *transaction)
    .await?;
    let object_id = object_result.last_insert_rowid();

    let item_result = sqlx::query(
        r#"
        INSERT INTO library_items (
          object_id, workspace_id, original_url, normalized_url, canonical_url,
          title, site_name, item_kind, status, read_at, starred,
          processing_status, last_error, refresh_status, refresh_error
        )
        VALUES (?, ?, ?, ?, NULL, ?, ?, 'BOOKMARK', 'ACTIVE', NULL, 0, 'PENDING', NULL, 'PENDING', NULL)
        ON CONFLICT(workspace_id, normalized_url) DO NOTHING
        "#,
    )
    .bind(object_id)
    .bind(workspace_id)
    .bind(&normalized_url.original)
    .bind(&normalized_url.normalized)
    .bind(&title)
    .bind(normalized_url.site_name.as_deref())
    .execute(&mut *transaction)
    .await?;

    let (item_id, item_object_id, item_uid, created) = if item_result.rows_affected() == 1 {
        (
            item_result.last_insert_rowid(),
            object_id,
            object_uid.clone(),
            true,
        )
    } else {
        sqlx::query(
            "DELETE FROM objects WHERE id = ? AND workspace_id = ? AND object_type = 'LIBRARY_ITEM'",
        )
        .bind(object_id)
        .bind(workspace_id)
        .execute(&mut *transaction)
        .await?;
        let (id, object_id, uid) =
            find_item_identity_by_url(&mut transaction, workspace_id, &normalized_url.normalized)
                .await?
                .ok_or_else(|| {
                    AppError::Internal(
                        "normalized URL conflict did not identify an item".to_owned(),
                    )
                })?;
        (id, object_id, uid, false)
    };

    if !created {
        sqlx::query(
            r#"
            UPDATE library_items
            SET
              title = CASE WHEN title = '' AND ? <> '' THEN ? ELSE title END,
              site_name = COALESCE(site_name, ?)
            WHERE id = ? AND workspace_id = ?
            "#,
        )
        .bind(&title)
        .bind(&title)
        .bind(normalized_url.site_name.as_deref())
        .bind(item_id)
        .bind(workspace_id)
        .execute(&mut *transaction)
        .await?;
    }

    if !tags.is_empty() {
        insert_tags(&mut transaction, workspace_id, item_object_id, &tags).await?;
    }

    let capture_uid = Ulid::generate().to_string();
    let capture_result = sqlx::query(
        r#"
        INSERT INTO library_captures (
          uid, library_item_id, workspace_id, idempotency_key,
          selected_text, note, captured_title, created_at
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&capture_uid)
    .bind(item_id)
    .bind(workspace_id)
    .bind(idempotency_key.as_deref())
    .bind(&selection)
    .bind(&note)
    .bind((!title.is_empty()).then_some(title.as_str()))
    .bind(now)
    .execute(&mut *transaction)
    .await;

    if let Err(error) = capture_result {
        transaction.rollback().await?;
        if idempotency_key.is_some() && is_unique_violation(&error) {
            return resolve_idempotent_race(
                pool,
                workspace_id,
                idempotency_key.as_deref().expect("key was checked"),
                &normalized_url.normalized,
            )
            .await;
        }
        return Err(error.into());
    }

    if created {
        jobs::enqueue_library_fetch(&mut transaction, workspace_id, item_object_id, now).await?;
    }

    sqlx::query(
        "UPDATE objects SET updated_at = ? WHERE id = ? AND workspace_id = ? AND object_type = 'LIBRARY_ITEM'",
    )
    .bind(now)
    .bind(item_object_id)
    .bind(workspace_id)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;

    Ok(CreateLibraryItemOutcome {
        item: get(pool, workspace_id, &item_uid).await?,
        created,
    })
}

pub async fn list(
    pool: &SqlitePool,
    workspace_id: i64,
    options: ListLibraryItemsOptions<'_>,
) -> AppResult<ListLibraryItemsResponse> {
    if options.page == 0 {
        return Err(AppError::bad_request("page must be at least 1"));
    }
    if !(1..=MAX_PAGE_SIZE).contains(&options.page_size) {
        return Err(AppError::bad_request("page_size must be between 1 and 100"));
    }
    let search_pattern = options
        .query
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(like_pattern);
    let tag = options.tag.map(normalize_tag).transpose()?;
    let tag = tag.as_deref().filter(|value| !value.is_empty());
    let read = options.read.map(i64::from);
    let starred = options.starred.map(i64::from);
    let offset = i64::from(options.page.saturating_sub(1)) * i64::from(options.page_size);
    let mut transaction = pool.begin().await?;

    let rows = sqlx::query_as::<_, LibraryItemRow>(LIST_ITEMS_SQL)
        .bind(workspace_id)
        .bind(workspace_id)
        .bind(options.status.as_str())
        // Search enabled, title, URLs, author, excerpt, and reader text.
        .bind(search_pattern.as_deref())
        .bind(search_pattern.as_deref())
        .bind(search_pattern.as_deref())
        .bind(search_pattern.as_deref())
        .bind(search_pattern.as_deref())
        .bind(search_pattern.as_deref())
        .bind(search_pattern.as_deref())
        .bind(search_pattern.as_deref())
        // Capture workspace plus selection, note, and captured title.
        .bind(workspace_id)
        .bind(search_pattern.as_deref())
        .bind(search_pattern.as_deref())
        .bind(search_pattern.as_deref())
        // Search tag workspace/pattern, then exact tag filter.
        .bind(workspace_id)
        .bind(search_pattern.as_deref())
        .bind(tag)
        .bind(workspace_id)
        .bind(tag)
        .bind(read)
        .bind(read)
        .bind(read)
        .bind(starred)
        .bind(starred)
        .bind(i64::from(options.page_size))
        .bind(offset)
        .fetch_all(&mut *transaction)
        .await?;

    let total = sqlx::query_scalar::<_, i64>(COUNT_ITEMS_SQL)
        .bind(workspace_id)
        .bind(workspace_id)
        .bind(options.status.as_str())
        // Keep this binding order identical to LIST_ITEMS_SQL above.
        .bind(search_pattern.as_deref())
        .bind(search_pattern.as_deref())
        .bind(search_pattern.as_deref())
        .bind(search_pattern.as_deref())
        .bind(search_pattern.as_deref())
        .bind(search_pattern.as_deref())
        .bind(search_pattern.as_deref())
        .bind(search_pattern.as_deref())
        .bind(workspace_id)
        .bind(search_pattern.as_deref())
        .bind(search_pattern.as_deref())
        .bind(search_pattern.as_deref())
        .bind(workspace_id)
        .bind(search_pattern.as_deref())
        .bind(tag)
        .bind(workspace_id)
        .bind(tag)
        .bind(read)
        .bind(read)
        .bind(read)
        .bind(starred)
        .bind(starred)
        .fetch_one(&mut *transaction)
        .await?;

    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        items.push(row_to_item(&mut transaction, workspace_id, row, false).await?);
    }
    transaction.commit().await?;
    Ok(ListLibraryItemsResponse {
        items,
        page: options.page,
        page_size: options.page_size,
        total,
    })
}

pub async fn get(pool: &SqlitePool, workspace_id: i64, uid: &str) -> AppResult<LibraryItem> {
    let mut transaction = pool.begin().await?;
    let row = fetch_row(&mut transaction, workspace_id, uid)
        .await?
        .ok_or_else(|| AppError::not_found("Library item"))?;
    let item = row_to_item(&mut transaction, workspace_id, row, true).await?;
    transaction.commit().await?;
    Ok(item)
}

pub async fn get_content(
    pool: &SqlitePool,
    workspace_id: i64,
    uid: &str,
) -> AppResult<LibraryContent> {
    let mut transaction = pool.begin().await?;
    let row = sqlx::query_as::<_, LibraryContentRow>(
        r#"
        SELECT
          reader_html.body AS safe_html,
          reader_text.body AS plain_text,
          li.fetched_at,
          li.content_version
        FROM library_items li
        JOIN objects o
          ON o.id = li.object_id AND o.workspace_id = li.workspace_id
        JOIN object_blobs html_link
          ON html_link.object_id = o.id
         AND html_link.workspace_id = o.workspace_id
         AND html_link.purpose = 'READER_HTML'
        JOIN blobs reader_html
          ON reader_html.id = html_link.blob_id
         AND reader_html.workspace_id = html_link.workspace_id
        JOIN object_blobs text_link
          ON text_link.object_id = o.id
         AND text_link.workspace_id = o.workspace_id
         AND text_link.purpose = 'READER_TEXT'
        JOIN blobs reader_text
          ON reader_text.id = text_link.blob_id
         AND reader_text.workspace_id = text_link.workspace_id
        WHERE li.workspace_id = ? AND o.workspace_id = ?
          AND o.uid = ? AND o.object_type = 'LIBRARY_ITEM'
        "#,
    )
    .bind(workspace_id)
    .bind(workspace_id)
    .bind(uid)
    .fetch_optional(&mut *transaction)
    .await?;
    let Some(row) = row else {
        let exists = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT EXISTS(
              SELECT 1
              FROM library_items li
              JOIN objects o
                ON o.id = li.object_id AND o.workspace_id = li.workspace_id
              WHERE li.workspace_id = ? AND o.workspace_id = ?
                AND o.uid = ? AND o.object_type = 'LIBRARY_ITEM'
            )
            "#,
        )
        .bind(workspace_id)
        .bind(workspace_id)
        .bind(uid)
        .fetch_one(&mut *transaction)
        .await?;
        if exists == 0 {
            return Err(AppError::not_found("Library item"));
        }
        return Err(AppError::conflict("Library content is not ready"));
    };

    let fetched_at = row
        .fetched_at
        .ok_or_else(|| AppError::Internal("Library content has no fetch timestamp".to_owned()))?;
    let content = LibraryContent {
        safe_html: String::from_utf8(row.safe_html)
            .map_err(|_| AppError::Internal("stored reader HTML is not UTF-8".to_owned()))?,
        plain_text: String::from_utf8(row.plain_text)
            .map_err(|_| AppError::Internal("stored reader text is not UTF-8".to_owned()))?,
        fetched_at: format_timestamp(fetched_at)?,
        content_version: parse_content_version(row.content_version)?,
    };
    transaction.commit().await?;
    Ok(content)
}

pub async fn retry_fetch(
    pool: &SqlitePool,
    workspace_id: i64,
    uid: &str,
    now: i64,
) -> AppResult<LibraryItem> {
    let mut transaction = pool.begin().await?;
    acquire_workspace_write_lock(&mut transaction, workspace_id).await?;
    let (object_id, processing_status, content_available) =
        sqlx::query_as::<_, (i64, String, bool)>(
            r#"
        SELECT o.id, li.processing_status,
          EXISTS (
            SELECT 1 FROM library_content_versions version
            WHERE version.library_item_id = li.id
              AND version.workspace_id = li.workspace_id
              AND version.status = 'CURRENT'
          ) AS content_available
        FROM library_items li
        JOIN objects o
          ON o.id = li.object_id AND o.workspace_id = li.workspace_id
        WHERE li.workspace_id = ? AND o.workspace_id = ?
          AND o.uid = ? AND o.object_type = 'LIBRARY_ITEM'
        "#,
        )
        .bind(workspace_id)
        .bind(workspace_id)
        .bind(uid)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or_else(|| AppError::not_found("Library item"))?;

    if processing_status != LibraryProcessingStatus::Pending.as_str() || content_available {
        jobs::enqueue_library_fetch(&mut transaction, workspace_id, object_id, now).await?;
        sqlx::query(
            r#"
            UPDATE library_items
            SET
              processing_status = CASE WHEN ? THEN 'READY' ELSE 'PENDING' END,
              last_error = CASE WHEN ? THEN NULL ELSE last_error END,
              refresh_status = 'PENDING',
              refresh_error = NULL
            WHERE object_id = ? AND workspace_id = ?
            "#,
        )
        .bind(content_available)
        .bind(content_available)
        .bind(object_id)
        .bind(workspace_id)
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            "UPDATE objects SET updated_at = ? WHERE id = ? AND workspace_id = ? AND object_type = 'LIBRARY_ITEM'",
        )
        .bind(now)
        .bind(object_id)
        .bind(workspace_id)
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await?;
    get(pool, workspace_id, uid).await
}

pub async fn accept_refresh_candidate(
    pool: &SqlitePool,
    workspace_id: i64,
    uid: &str,
    now: i64,
) -> AppResult<LibraryItem> {
    let mut transaction = pool.begin().await?;
    acquire_workspace_write_lock(&mut transaction, workspace_id).await?;
    let identity = find_candidate_identity(&mut transaction, workspace_id, uid).await?;

    sqlx::query(
        "UPDATE library_content_versions SET status = 'HISTORICAL' WHERE library_item_id = ? AND workspace_id = ? AND status = 'CURRENT'",
    )
    .bind(identity.0)
    .bind(workspace_id)
    .execute(&mut *transaction)
    .await?;
    sqlx::query(
        "UPDATE library_content_versions SET status = 'CURRENT' WHERE id = ? AND workspace_id = ? AND status = 'CANDIDATE'",
    )
    .bind(identity.2)
    .bind(workspace_id)
    .execute(&mut *transaction)
    .await?;

    for (purpose, blob_id) in [
        ("SOURCE_HTML", identity.3),
        ("READER_HTML", identity.4),
        ("READER_TEXT", identity.5),
    ] {
        sqlx::query(
            r#"
            INSERT INTO object_blobs (object_id, workspace_id, blob_id, purpose)
            VALUES (?, ?, ?, ?)
            ON CONFLICT(object_id, purpose)
            DO UPDATE SET workspace_id = excluded.workspace_id, blob_id = excluded.blob_id
            "#,
        )
        .bind(identity.1)
        .bind(workspace_id)
        .bind(blob_id)
        .bind(purpose)
        .execute(&mut *transaction)
        .await?;
    }

    sqlx::query(
        r#"
        UPDATE library_items
        SET
          canonical_url = COALESCE(candidate.canonical_url, library_items.canonical_url),
          title = CASE
            WHEN library_items.title = '' AND candidate.title <> '' THEN candidate.title
            ELSE library_items.title
          END,
          site_name = COALESCE(candidate.site_name, library_items.site_name),
          author = candidate.author,
          published_at = candidate.published_at,
          excerpt = candidate.excerpt,
          item_kind = 'ARTICLE',
          processing_status = 'READY',
          last_error = NULL,
          refresh_status = 'IDLE',
          refresh_error = NULL,
          fetched_at = candidate.fetched_at,
          content_hash = candidate.content_hash,
          content_version = candidate.version_number
        FROM library_content_versions candidate
        WHERE library_items.id = ? AND library_items.workspace_id = ?
          AND candidate.id = ? AND candidate.workspace_id = library_items.workspace_id
        "#,
    )
    .bind(identity.0)
    .bind(workspace_id)
    .bind(identity.2)
    .execute(&mut *transaction)
    .await?;
    sqlx::query(
        "UPDATE objects SET updated_at = MAX(updated_at, ?) WHERE id = ? AND workspace_id = ? AND object_type = 'LIBRARY_ITEM'",
    )
    .bind(now)
    .bind(identity.1)
    .bind(workspace_id)
    .execute(&mut *transaction)
    .await?;
    jobs::cleanup_orphan_blobs(&mut transaction, workspace_id).await?;
    transaction.commit().await?;
    get(pool, workspace_id, uid).await
}

pub async fn discard_refresh_candidate(
    pool: &SqlitePool,
    workspace_id: i64,
    uid: &str,
    now: i64,
) -> AppResult<LibraryItem> {
    let mut transaction = pool.begin().await?;
    acquire_workspace_write_lock(&mut transaction, workspace_id).await?;
    let identity = find_candidate_identity(&mut transaction, workspace_id, uid).await?;
    sqlx::query(
        "DELETE FROM library_content_versions WHERE id = ? AND workspace_id = ? AND status = 'CANDIDATE'",
    )
    .bind(identity.2)
    .bind(workspace_id)
    .execute(&mut *transaction)
    .await?;
    sqlx::query(
        "UPDATE library_items SET refresh_status = 'IDLE', refresh_error = NULL WHERE id = ? AND workspace_id = ?",
    )
    .bind(identity.0)
    .bind(workspace_id)
    .execute(&mut *transaction)
    .await?;
    sqlx::query(
        "UPDATE objects SET updated_at = MAX(updated_at, ?) WHERE id = ? AND workspace_id = ? AND object_type = 'LIBRARY_ITEM'",
    )
    .bind(now)
    .bind(identity.1)
    .bind(workspace_id)
    .execute(&mut *transaction)
    .await?;
    jobs::cleanup_orphan_blobs(&mut transaction, workspace_id).await?;
    transaction.commit().await?;
    get(pool, workspace_id, uid).await
}

async fn find_candidate_identity(
    transaction: &mut Transaction<'_, Sqlite>,
    workspace_id: i64,
    uid: &str,
) -> AppResult<(i64, i64, i64, i64, i64, i64)> {
    sqlx::query_as(
        r#"
        SELECT li.id, o.id, candidate.id, candidate.source_blob_id,
          candidate.reader_html_blob_id, candidate.reader_text_blob_id
        FROM library_items li
        JOIN objects o
          ON o.id = li.object_id AND o.workspace_id = li.workspace_id
        JOIN library_content_versions candidate
          ON candidate.library_item_id = li.id
         AND candidate.workspace_id = li.workspace_id
         AND candidate.status = 'CANDIDATE'
        WHERE li.workspace_id = ? AND o.workspace_id = ?
          AND o.uid = ? AND o.object_type = 'LIBRARY_ITEM'
        "#,
    )
    .bind(workspace_id)
    .bind(workspace_id)
    .bind(uid)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or_else(|| AppError::conflict("Library item has no refresh candidate"))
}

pub async fn update(
    pool: &SqlitePool,
    workspace_id: i64,
    uid: &str,
    request: UpdateLibraryItemRequest,
    now: i64,
) -> AppResult<LibraryItem> {
    if request.is_empty() {
        return Err(AppError::bad_request(
            "PATCH must include at least one editable field",
        ));
    }
    let UpdateLibraryItemRequest {
        title,
        status,
        read,
        starred,
        tags,
    } = request;
    let title = title.into_required("title")?;
    let status = status.into_required("status")?;
    let read = read.into_required("read")?;
    let starred = starred.into_required("starred")?;
    let tags = tags
        .into_required("tags")?
        .map(normalize_tags)
        .transpose()?;
    if let Some(title) = title.as_deref() {
        validate_title(title)?;
    }

    let mut transaction = pool.begin().await?;
    acquire_workspace_write_lock(&mut transaction, workspace_id).await?;
    let identity = sqlx::query_as::<_, (i64, i64)>(
        r#"
        SELECT li.id, o.id
        FROM library_items li
        JOIN objects o
          ON o.id = li.object_id AND o.workspace_id = li.workspace_id
        WHERE li.workspace_id = ? AND o.workspace_id = ?
          AND o.uid = ? AND o.object_type = 'LIBRARY_ITEM'
        "#,
    )
    .bind(workspace_id)
    .bind(workspace_id)
    .bind(uid)
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or_else(|| AppError::not_found("Library item"))?;

    sqlx::query(
        r#"
        UPDATE library_items
        SET
          title = CASE WHEN ? THEN ? ELSE title END,
          status = CASE WHEN ? THEN ? ELSE status END,
          read_at = CASE
            WHEN ? = 0 THEN read_at
            WHEN ? = 1 THEN COALESCE(read_at, ?)
            ELSE NULL
          END,
          starred = CASE WHEN ? THEN ? ELSE starred END
        WHERE id = ? AND workspace_id = ?
        "#,
    )
    .bind(title.is_some())
    .bind(title.as_deref())
    .bind(status.is_some())
    .bind(status.map(LibraryStatus::as_str))
    .bind(i64::from(read.is_some()))
    .bind(read.map(i64::from))
    .bind(now)
    .bind(starred.is_some())
    .bind(starred.map(i64::from))
    .bind(identity.0)
    .bind(workspace_id)
    .execute(&mut *transaction)
    .await?;

    if let Some(tags) = tags.as_deref() {
        sqlx::query(
            r#"
            DELETE FROM object_tags
            WHERE object_id = ?
              AND EXISTS (
                SELECT 1 FROM objects o
                WHERE o.id = object_tags.object_id AND o.workspace_id = ?
              )
            "#,
        )
        .bind(identity.1)
        .bind(workspace_id)
        .execute(&mut *transaction)
        .await?;
        insert_tags(&mut transaction, workspace_id, identity.1, tags).await?;
    }

    sqlx::query(
        "UPDATE objects SET updated_at = ? WHERE id = ? AND workspace_id = ? AND object_type = 'LIBRARY_ITEM'",
    )
    .bind(now)
    .bind(identity.1)
    .bind(workspace_id)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    get(pool, workspace_id, uid).await
}

pub async fn delete(pool: &SqlitePool, workspace_id: i64, uid: &str) -> AppResult<()> {
    let mut transaction = pool.begin().await?;
    acquire_workspace_write_lock(&mut transaction, workspace_id).await?;
    let result = sqlx::query(
        r#"
        DELETE FROM objects
        WHERE workspace_id = ? AND uid = ? AND object_type = 'LIBRARY_ITEM'
          AND EXISTS (
            SELECT 1 FROM library_items li
            WHERE li.object_id = objects.id AND li.workspace_id = ?
          )
        "#,
    )
    .bind(workspace_id)
    .bind(uid)
    .bind(workspace_id)
    .execute(&mut *transaction)
    .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::not_found("Library item"));
    }
    jobs::cleanup_orphan_blobs(&mut transaction, workspace_id).await?;
    transaction.commit().await?;
    Ok(())
}

async fn fetch_row(
    transaction: &mut Transaction<'_, Sqlite>,
    workspace_id: i64,
    uid: &str,
) -> AppResult<Option<LibraryItemRow>> {
    Ok(sqlx::query_as::<_, LibraryItemRow>(
        r#"
        SELECT
          li.id, li.object_id, o.uid, li.original_url, li.normalized_url,
          li.canonical_url, li.title, li.site_name, li.author, li.published_at,
          li.excerpt, li.item_kind, li.status,
          li.read_at, li.starred, li.processing_status, li.last_error,
          li.refresh_status, li.refresh_error,
          li.fetched_at, li.content_version,
          EXISTS (
            SELECT 1
            FROM object_blobs html_link
            JOIN object_blobs text_link
              ON text_link.object_id = html_link.object_id
             AND text_link.workspace_id = html_link.workspace_id
             AND text_link.purpose = 'READER_TEXT'
            WHERE html_link.object_id = o.id
              AND html_link.workspace_id = o.workspace_id
              AND html_link.purpose = 'READER_HTML'
          ) AS content_available,
          current_version.text_byte_len AS current_text_byte_len,
          candidate_version.version_number AS candidate_content_version,
          candidate_version.text_byte_len AS candidate_text_byte_len,
          o.created_at, o.updated_at
        FROM library_items li
        JOIN objects o
          ON o.id = li.object_id AND o.workspace_id = li.workspace_id
        LEFT JOIN library_content_versions current_version
          ON current_version.library_item_id = li.id
         AND current_version.workspace_id = li.workspace_id
         AND current_version.status = 'CURRENT'
        LEFT JOIN library_content_versions candidate_version
          ON candidate_version.library_item_id = li.id
         AND candidate_version.workspace_id = li.workspace_id
         AND candidate_version.status = 'CANDIDATE'
        WHERE li.workspace_id = ? AND o.workspace_id = ?
          AND o.uid = ? AND o.object_type = 'LIBRARY_ITEM'
        "#,
    )
    .bind(workspace_id)
    .bind(workspace_id)
    .bind(uid)
    .fetch_optional(&mut **transaction)
    .await?)
}

async fn row_to_item(
    transaction: &mut Transaction<'_, Sqlite>,
    workspace_id: i64,
    row: LibraryItemRow,
    include_captures: bool,
) -> AppResult<LibraryItem> {
    let tags = sqlx::query_scalar::<_, String>(
        r#"
        SELECT ot.tag
        FROM object_tags ot
        JOIN objects o ON o.id = ot.object_id
        WHERE ot.object_id = ? AND o.workspace_id = ?
        ORDER BY ot.tag ASC
        "#,
    )
    .bind(row.object_id)
    .bind(workspace_id)
    .fetch_all(&mut **transaction)
    .await?;
    let captures = if include_captures {
        let capture_rows = sqlx::query_as::<_, LibraryCaptureRow>(
            r#"
            SELECT uid, selected_text, note, captured_title, created_at
            FROM library_captures
            WHERE library_item_id = ? AND workspace_id = ?
            ORDER BY created_at ASC, id ASC
            "#,
        )
        .bind(row.id)
        .bind(workspace_id)
        .fetch_all(&mut **transaction)
        .await?;
        capture_rows
            .into_iter()
            .map(|capture| {
                Ok(LibraryCapture {
                    uid: capture.uid,
                    selected_text: capture.selected_text,
                    note: capture.note,
                    captured_title: capture.captured_title,
                    created_at: format_timestamp(capture.created_at)?,
                })
            })
            .collect::<AppResult<Vec<_>>>()?
    } else {
        Vec::new()
    };

    Ok(LibraryItem {
        uid: row.uid,
        original_url: row.original_url,
        normalized_url: row.normalized_url,
        canonical_url: row.canonical_url,
        title: row.title,
        site_name: row.site_name,
        author: row.author,
        published_at: row.published_at.map(format_timestamp).transpose()?,
        excerpt: row.excerpt,
        item_kind: parse_item_kind(&row.item_kind)?,
        status: parse_status(&row.status)?,
        read_at: row.read_at.map(format_timestamp).transpose()?,
        starred: row.starred,
        processing_status: parse_processing_status(&row.processing_status)?,
        last_error: row.last_error,
        refresh_status: parse_refresh_status(&row.refresh_status)?,
        refresh_error: row.refresh_error,
        fetched_at: row.fetched_at.map(format_timestamp).transpose()?,
        content_version: parse_content_version(row.content_version)?,
        content_available: row.content_available,
        current_text_byte_len: parse_optional_u64(
            row.current_text_byte_len,
            "current text length",
        )?,
        candidate_content_version: row
            .candidate_content_version
            .map(parse_content_version)
            .transpose()?,
        candidate_text_byte_len: parse_optional_u64(
            row.candidate_text_byte_len,
            "candidate text length",
        )?,
        tags,
        captures,
        created_at: format_timestamp(row.created_at)?,
        updated_at: format_timestamp(row.updated_at)?,
    })
}

async fn find_idempotent_item(
    transaction: &mut Transaction<'_, Sqlite>,
    workspace_id: i64,
    key: &str,
) -> AppResult<Option<(String, String)>> {
    Ok(sqlx::query_as::<_, (String, String)>(
        r#"
        SELECT o.uid, li.normalized_url
        FROM library_captures lc
        JOIN library_items li
          ON li.id = lc.library_item_id AND li.workspace_id = lc.workspace_id
        JOIN objects o
          ON o.id = li.object_id AND o.workspace_id = li.workspace_id
        WHERE lc.workspace_id = ? AND li.workspace_id = ? AND o.workspace_id = ?
          AND lc.idempotency_key = ? AND o.object_type = 'LIBRARY_ITEM'
        "#,
    )
    .bind(workspace_id)
    .bind(workspace_id)
    .bind(workspace_id)
    .bind(key)
    .fetch_optional(&mut **transaction)
    .await?)
}

async fn find_item_identity_by_url(
    transaction: &mut Transaction<'_, Sqlite>,
    workspace_id: i64,
    normalized_url: &str,
) -> AppResult<Option<(i64, i64, String)>> {
    Ok(sqlx::query_as::<_, (i64, i64, String)>(
        r#"
        SELECT li.id, o.id, o.uid
        FROM library_items li
        JOIN objects o
          ON o.id = li.object_id AND o.workspace_id = li.workspace_id
        WHERE li.workspace_id = ? AND o.workspace_id = ?
          AND li.normalized_url = ? AND o.object_type = 'LIBRARY_ITEM'
        "#,
    )
    .bind(workspace_id)
    .bind(workspace_id)
    .bind(normalized_url)
    .fetch_optional(&mut **transaction)
    .await?)
}

async fn resolve_idempotent_race(
    pool: &SqlitePool,
    workspace_id: i64,
    key: &str,
    normalized_url: &str,
) -> AppResult<CreateLibraryItemOutcome> {
    let row = sqlx::query_as::<_, (String, String)>(
        r#"
        SELECT o.uid, li.normalized_url
        FROM library_captures lc
        JOIN library_items li
          ON li.id = lc.library_item_id AND li.workspace_id = lc.workspace_id
        JOIN objects o
          ON o.id = li.object_id AND o.workspace_id = li.workspace_id
        WHERE lc.workspace_id = ? AND li.workspace_id = ? AND o.workspace_id = ?
          AND lc.idempotency_key = ? AND o.object_type = 'LIBRARY_ITEM'
        "#,
    )
    .bind(workspace_id)
    .bind(workspace_id)
    .bind(workspace_id)
    .bind(key)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::Internal("idempotency conflict could not be resolved".to_owned()))?;
    if row.1 != normalized_url {
        return Err(AppError::conflict(
            "Idempotency key was already used for a different URL",
        ));
    }
    Ok(CreateLibraryItemOutcome {
        item: get(pool, workspace_id, &row.0).await?,
        created: false,
    })
}

async fn insert_tags(
    transaction: &mut Transaction<'_, Sqlite>,
    workspace_id: i64,
    object_id: i64,
    tags: &[String],
) -> AppResult<()> {
    let existing = sqlx::query_scalar::<_, String>(
        r#"
        SELECT ot.tag
        FROM object_tags ot
        JOIN objects o ON o.id = ot.object_id
        WHERE ot.object_id = ? AND o.workspace_id = ?
        "#,
    )
    .bind(object_id)
    .bind(workspace_id)
    .fetch_all(&mut **transaction)
    .await?;
    let mut union = existing.into_iter().collect::<BTreeSet<_>>();
    union.extend(tags.iter().cloned());
    if union.len() > MAX_TAGS {
        return Err(AppError::validation(format!(
            "Library item must not have more than {MAX_TAGS} tags"
        )));
    }
    for tag in tags {
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO object_tags (object_id, tag)
            SELECT id, ? FROM objects
            WHERE id = ? AND workspace_id = ? AND object_type = 'LIBRARY_ITEM'
            "#,
        )
        .bind(tag)
        .bind(object_id)
        .bind(workspace_id)
        .execute(&mut **transaction)
        .await?;
    }
    Ok(())
}

async fn acquire_workspace_write_lock(
    transaction: &mut Transaction<'_, Sqlite>,
    workspace_id: i64,
) -> AppResult<()> {
    // Lock before reading state so concurrent mutations never upgrade a stale WAL snapshot.
    sqlx::query("UPDATE workspaces SET updated_at = updated_at WHERE id = ?")
        .bind(workspace_id)
        .execute(&mut **transaction)
        .await?;
    Ok(())
}

fn normalize_url(input: &str) -> AppResult<NormalizedUrl> {
    let original = input.trim();
    if original.is_empty() {
        return Err(AppError::validation("URL must not be empty"));
    }
    if original.len() > MAX_URL_BYTES {
        return Err(AppError::validation(format!(
            "URL must not exceed {MAX_URL_BYTES} bytes"
        )));
    }
    if authority_contains_userinfo(original) {
        return Err(AppError::validation(
            "URL must not contain user information",
        ));
    }
    let mut parsed = Url::parse(original)
        .map_err(|_| AppError::validation("URL must be a valid absolute HTTP or HTTPS URL"))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(AppError::validation("URL scheme must be HTTP or HTTPS"));
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(AppError::validation(
            "URL must not contain user information",
        ));
    }
    let site_name = parsed
        .host_str()
        .map(str::to_owned)
        .ok_or_else(|| AppError::validation("URL must include a host"))?;
    if site_name.chars().count() > MAX_SITE_NAME_CHARACTERS {
        return Err(AppError::validation(format!(
            "URL host must not exceed {MAX_SITE_NAME_CHARACTERS} characters"
        )));
    }
    parsed.set_fragment(None);
    let normalized = parsed.as_str().to_owned();
    if normalized.len() > MAX_URL_BYTES {
        return Err(AppError::validation(format!(
            "Normalized URL must not exceed {MAX_URL_BYTES} bytes"
        )));
    }
    Ok(NormalizedUrl {
        original: original.to_owned(),
        normalized,
        site_name: Some(site_name),
    })
}

fn authority_contains_userinfo(input: &str) -> bool {
    let Some((_, remainder)) = input.split_once("://") else {
        return false;
    };
    remainder
        .split(['/', '?', '#'])
        .next()
        .is_some_and(|authority| authority.contains('@'))
}

fn normalize_idempotency_key(value: Option<String>) -> AppResult<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value.trim();
    if value.is_empty() {
        return Err(AppError::validation(
            "idempotencyKey must not be empty when provided",
        ));
    }
    if value.chars().count() > MAX_IDEMPOTENCY_KEY_CHARACTERS {
        return Err(AppError::validation(format!(
            "idempotencyKey must not exceed {MAX_IDEMPOTENCY_KEY_CHARACTERS} characters"
        )));
    }
    Ok(Some(value.to_owned()))
}

fn validate_title(value: &str) -> AppResult<()> {
    if value.chars().count() > MAX_TITLE_CHARACTERS {
        return Err(AppError::validation(format!(
            "Library title must not exceed {MAX_TITLE_CHARACTERS} characters"
        )));
    }
    Ok(())
}

fn validate_capture_text(field: &'static str, value: &str) -> AppResult<()> {
    if value.len() > MAX_CAPTURE_BYTES {
        return Err(AppError::validation(format!(
            "Library {field} must not exceed {MAX_CAPTURE_BYTES} bytes"
        )));
    }
    Ok(())
}

fn normalize_tags(tags: Vec<String>) -> AppResult<Vec<String>> {
    let mut normalized = Vec::new();
    for tag in tags {
        if tag.trim().trim_start_matches('#').trim().is_empty() {
            continue;
        }
        let tag = normalize_tag(&tag)?;
        if normalized.contains(&tag) {
            continue;
        }
        normalized.push(tag);
        if normalized.len() > MAX_TAGS {
            return Err(AppError::validation(format!(
                "Library item must not have more than {MAX_TAGS} tags"
            )));
        }
    }
    normalized.sort();
    Ok(normalized)
}

fn normalize_tag(tag: &str) -> AppResult<String> {
    let tag = tag.trim().trim_start_matches('#').trim().to_lowercase();
    if tag.is_empty()
        || !tag
            .chars()
            .all(|character| character.is_alphanumeric() || matches!(character, '_' | '-' | '/'))
    {
        return Err(AppError::bad_request("tag is invalid"));
    }
    if tag.chars().count() > MAX_TAG_CHARACTERS {
        return Err(AppError::bad_request(format!(
            "tag must not exceed {MAX_TAG_CHARACTERS} characters"
        )));
    }
    Ok(tag)
}

fn like_pattern(value: &str) -> String {
    let escaped = value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
}

fn is_unique_violation(error: &sqlx::Error) -> bool {
    matches!(error, sqlx::Error::Database(database) if database.is_unique_violation())
}

fn parse_item_kind(value: &str) -> AppResult<LibraryItemKind> {
    match value {
        "BOOKMARK" => Ok(LibraryItemKind::Bookmark),
        "ARTICLE" => Ok(LibraryItemKind::Article),
        _ => Err(AppError::Internal(format!(
            "invalid stored library item kind: {value}"
        ))),
    }
}

fn parse_status(value: &str) -> AppResult<LibraryStatus> {
    match value {
        "ACTIVE" => Ok(LibraryStatus::Active),
        "ARCHIVED" => Ok(LibraryStatus::Archived),
        _ => Err(AppError::Internal(format!(
            "invalid stored library status: {value}"
        ))),
    }
}

fn parse_processing_status(value: &str) -> AppResult<LibraryProcessingStatus> {
    match value {
        "NOT_FETCHED" => Ok(LibraryProcessingStatus::NotFetched),
        "PENDING" => Ok(LibraryProcessingStatus::Pending),
        "READY" => Ok(LibraryProcessingStatus::Ready),
        "FAILED" => Ok(LibraryProcessingStatus::Failed),
        _ => Err(AppError::Internal(format!(
            "invalid stored library processing status: {value}"
        ))),
    }
}

fn parse_refresh_status(value: &str) -> AppResult<LibraryRefreshStatus> {
    match value {
        "IDLE" => Ok(LibraryRefreshStatus::Idle),
        "PENDING" => Ok(LibraryRefreshStatus::Pending),
        "FAILED" => Ok(LibraryRefreshStatus::Failed),
        "REVIEW" => Ok(LibraryRefreshStatus::Review),
        _ => Err(AppError::Internal(format!(
            "invalid stored Library refresh status: {value}"
        ))),
    }
}

fn parse_optional_u64(value: Option<i64>, field: &str) -> AppResult<Option<u64>> {
    value
        .map(|value| {
            u64::try_from(value)
                .map_err(|_| AppError::Internal(format!("invalid stored {field}: {value}")))
        })
        .transpose()
}

fn parse_content_version(value: i64) -> AppResult<u32> {
    u32::try_from(value)
        .map_err(|_| AppError::Internal(format!("invalid stored Library content version: {value}")))
}

const LIST_ITEMS_SQL: &str = r#"
SELECT
  li.id, li.object_id, o.uid, li.original_url, li.normalized_url,
  li.canonical_url, li.title, li.site_name, li.author, li.published_at,
  li.excerpt, li.item_kind, li.status,
  li.read_at, li.starred, li.processing_status, li.last_error,
  li.refresh_status, li.refresh_error,
  li.fetched_at, li.content_version,
  EXISTS (
    SELECT 1
    FROM object_blobs html_link
    JOIN object_blobs text_link
      ON text_link.object_id = html_link.object_id
     AND text_link.workspace_id = html_link.workspace_id
     AND text_link.purpose = 'READER_TEXT'
    WHERE html_link.object_id = o.id
      AND html_link.workspace_id = o.workspace_id
      AND html_link.purpose = 'READER_HTML'
  ) AS content_available,
  current_version.text_byte_len AS current_text_byte_len,
  candidate_version.version_number AS candidate_content_version,
  candidate_version.text_byte_len AS candidate_text_byte_len,
  o.created_at, o.updated_at
FROM library_items li
JOIN objects o ON o.id = li.object_id AND o.workspace_id = li.workspace_id
LEFT JOIN library_content_versions current_version
  ON current_version.library_item_id = li.id
 AND current_version.workspace_id = li.workspace_id
 AND current_version.status = 'CURRENT'
LEFT JOIN library_content_versions candidate_version
  ON candidate_version.library_item_id = li.id
 AND candidate_version.workspace_id = li.workspace_id
 AND candidate_version.status = 'CANDIDATE'
WHERE li.workspace_id = ? AND o.workspace_id = ? AND o.object_type = 'LIBRARY_ITEM'
  AND li.status = ?
  AND (
    ? IS NULL
    OR li.title LIKE ? ESCAPE '\'
    OR li.original_url LIKE ? ESCAPE '\'
    OR li.normalized_url LIKE ? ESCAPE '\'
    OR COALESCE(li.canonical_url, '') LIKE ? ESCAPE '\'
    OR COALESCE(li.author, '') LIKE ? ESCAPE '\'
    OR li.excerpt LIKE ? ESCAPE '\'
    OR EXISTS (
      SELECT 1
      FROM object_blobs content_link
      JOIN blobs content_blob
        ON content_blob.id = content_link.blob_id
       AND content_blob.workspace_id = content_link.workspace_id
      WHERE content_link.object_id = o.id
        AND content_link.workspace_id = o.workspace_id
        AND content_link.purpose = 'READER_TEXT'
        AND CAST(content_blob.body AS TEXT) LIKE ? ESCAPE '\'
    )
    OR EXISTS (
      SELECT 1 FROM library_captures lc
      WHERE lc.library_item_id = li.id AND lc.workspace_id = ?
        AND (
          lc.selected_text LIKE ? ESCAPE '\'
          OR lc.note LIKE ? ESCAPE '\'
          OR COALESCE(lc.captured_title, '') LIKE ? ESCAPE '\'
        )
    )
    OR EXISTS (
      SELECT 1 FROM object_tags ot
      JOIN objects tagged ON tagged.id = ot.object_id
      WHERE ot.object_id = o.id AND tagged.workspace_id = ? AND ot.tag LIKE ? ESCAPE '\'
    )
  )
  AND (
    ? IS NULL OR EXISTS (
      SELECT 1 FROM object_tags ot
      JOIN objects tagged ON tagged.id = ot.object_id
      WHERE ot.object_id = o.id AND tagged.workspace_id = ? AND ot.tag = ?
    )
  )
  AND (? IS NULL OR (? = 1 AND li.read_at IS NOT NULL) OR (? = 0 AND li.read_at IS NULL))
  AND (? IS NULL OR li.starred = ?)
ORDER BY COALESCE(li.published_at, o.created_at) DESC, o.id DESC
LIMIT ? OFFSET ?
"#;

const COUNT_ITEMS_SQL: &str = r#"
SELECT COUNT(*)
FROM library_items li
JOIN objects o ON o.id = li.object_id AND o.workspace_id = li.workspace_id
WHERE li.workspace_id = ? AND o.workspace_id = ? AND o.object_type = 'LIBRARY_ITEM'
  AND li.status = ?
  AND (
    ? IS NULL
    OR li.title LIKE ? ESCAPE '\'
    OR li.original_url LIKE ? ESCAPE '\'
    OR li.normalized_url LIKE ? ESCAPE '\'
    OR COALESCE(li.canonical_url, '') LIKE ? ESCAPE '\'
    OR COALESCE(li.author, '') LIKE ? ESCAPE '\'
    OR li.excerpt LIKE ? ESCAPE '\'
    OR EXISTS (
      SELECT 1
      FROM object_blobs content_link
      JOIN blobs content_blob
        ON content_blob.id = content_link.blob_id
       AND content_blob.workspace_id = content_link.workspace_id
      WHERE content_link.object_id = o.id
        AND content_link.workspace_id = o.workspace_id
        AND content_link.purpose = 'READER_TEXT'
        AND CAST(content_blob.body AS TEXT) LIKE ? ESCAPE '\'
    )
    OR EXISTS (
      SELECT 1 FROM library_captures lc
      WHERE lc.library_item_id = li.id AND lc.workspace_id = ?
        AND (
          lc.selected_text LIKE ? ESCAPE '\'
          OR lc.note LIKE ? ESCAPE '\'
          OR COALESCE(lc.captured_title, '') LIKE ? ESCAPE '\'
        )
    )
    OR EXISTS (
      SELECT 1 FROM object_tags ot
      JOIN objects tagged ON tagged.id = ot.object_id
      WHERE ot.object_id = o.id AND tagged.workspace_id = ? AND ot.tag LIKE ? ESCAPE '\'
    )
  )
  AND (
    ? IS NULL OR EXISTS (
      SELECT 1 FROM object_tags ot
      JOIN objects tagged ON tagged.id = ot.object_id
      WHERE ot.object_id = o.id AND tagged.workspace_id = ? AND ot.tag = ?
    )
  )
  AND (? IS NULL OR (? = 1 AND li.read_at IS NOT NULL) OR (? = 0 AND li.read_at IS NULL))
  AND (? IS NULL OR li.starred = ?)
"#;

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tempfile::TempDir;
    use ulid::Ulid;

    use super::{
        CreateLibraryItemRequest, ListLibraryItemsOptions, MAX_TAGS, UpdateLibraryItemRequest,
        create, list, normalize_idempotency_key, normalize_tags, normalize_url, update,
    };
    use crate::{config::Config, patch::PatchField, state::AppState};

    async fn fixture() -> (TempDir, AppState, i64, i64) {
        let directory = tempfile::tempdir().unwrap();
        let config = Config::for_test(
            directory.path().join("data"),
            "admin",
            "correct horse battery staple",
            "UTC".parse().unwrap(),
        );
        let state = AppState::initialize(config).await.unwrap();
        let (workspace_id, creator_id) = sqlx::query_as::<_, (i64, i64)>(
            r#"
            SELECT w.id, u.id
            FROM workspaces w
            JOIN users u ON u.id = w.created_by
            WHERE w.id = ?
            "#,
        )
        .bind(1_i64)
        .fetch_one(state.pool())
        .await
        .unwrap();
        (directory, state, workspace_id, creator_id)
    }

    #[tokio::test]
    async fn list_uses_published_time_then_saved_creation_time_not_update_time() {
        let (_directory, state, workspace_id, creator_id) = fixture().await;
        for (url, now) in [
            ("https://example.com/old-saved", 1_000),
            ("https://example.com/middle-saved", 2_000),
            ("https://example.com/new-saved", 3_000),
        ] {
            create(
                state.pool(),
                workspace_id,
                creator_id,
                capture_request(url, Some(url), None),
                now,
            )
            .await
            .unwrap();
        }
        sqlx::query(
            r#"
            UPDATE library_items
            SET published_at = 4_000
            WHERE normalized_url = 'https://example.com/old-saved' AND workspace_id = ?
            "#,
        )
        .bind(workspace_id)
        .execute(state.pool())
        .await
        .unwrap();
        sqlx::query(
            r#"
            UPDATE objects SET updated_at = 9_000
            WHERE id = (
              SELECT object_id FROM library_items
              WHERE normalized_url = 'https://example.com/middle-saved' AND workspace_id = ?
            )
            "#,
        )
        .bind(workspace_id)
        .execute(state.pool())
        .await
        .unwrap();

        let items = list(
            state.pool(),
            workspace_id,
            ListLibraryItemsOptions::default(),
        )
        .await
        .unwrap()
        .items;
        assert_eq!(
            items
                .iter()
                .map(|item| item.normalized_url.as_str())
                .collect::<Vec<_>>(),
            vec![
                "https://example.com/old-saved",
                "https://example.com/new-saved",
                "https://example.com/middle-saved",
            ]
        );
    }

    fn capture_request(
        url: &str,
        title: Option<&str>,
        idempotency_key: Option<&str>,
    ) -> CreateLibraryItemRequest {
        CreateLibraryItemRequest {
            url: url.to_owned(),
            title: title.map(str::to_owned),
            selection: Some("Selected text".to_owned()),
            note: Some("Capture note".to_owned()),
            tags: Some(vec!["Rust".to_owned()]),
            idempotency_key: idempotency_key.map(str::to_owned),
        }
    }

    #[test]
    fn normalizes_http_urls_and_removes_fragments() {
        let value = normalize_url(" HTTPS://Example.COM:443/a/../b?x=1#section ").unwrap();
        assert_eq!(value.original, "HTTPS://Example.COM:443/a/../b?x=1#section");
        assert_eq!(value.normalized, "https://example.com/b?x=1");
        assert_eq!(value.site_name.as_deref(), Some("example.com"));

        assert!(normalize_url("ftp://example.com/file").is_err());
        assert!(normalize_url("https://user:secret@example.com/").is_err());
        assert!(normalize_url("https://@example.com/").is_err());
        assert!(normalize_url("https://").is_err());
        let long_host = format!("https://{}example/", "a.".repeat(127));
        assert_eq!(
            normalize_url(&long_host).unwrap_err().to_string(),
            "URL host must not exceed 255 characters"
        );
    }

    #[test]
    fn validates_idempotency_keys_and_normalizes_tags() {
        assert_eq!(
            normalize_idempotency_key(Some(" capture-1 ".to_owned())).unwrap(),
            Some("capture-1".to_owned())
        );
        assert!(normalize_idempotency_key(Some(" ".to_owned())).is_err());
        assert_eq!(
            normalize_tags(vec![
                " #Rust ".to_owned(),
                "rust".to_owned(),
                "中文".to_owned(),
                "  ".to_owned(),
            ])
            .unwrap(),
            vec!["rust", "中文"]
        );
        assert!(normalize_tags(vec!["two words".to_owned()]).is_err());
        assert!(normalize_tags(vec!["line\nbreak".to_owned()]).is_err());
    }

    #[test]
    fn patch_distinguishes_missing_null_and_values() {
        let request: UpdateLibraryItemRequest = serde_json::from_value(json!({
            "read": true,
            "tags": ["rust"]
        }))
        .unwrap();
        assert!(matches!(request.title, PatchField::Missing));
        assert!(matches!(request.read, PatchField::Value(true)));
        assert!(matches!(request.tags, PatchField::Value(_)));

        let null: UpdateLibraryItemRequest =
            serde_json::from_value(json!({"starred": null})).unwrap();
        assert!(matches!(null.starred, PatchField::Null));
        assert!(
            serde_json::from_value::<UpdateLibraryItemRequest>(json!({"unknown": true})).is_err()
        );
    }

    #[tokio::test]
    async fn reuses_urls_and_idempotency_keys_with_capture_semantics() {
        let (_directory, state, workspace_id, creator_id) = fixture().await;
        let first = create(
            state.pool(),
            workspace_id,
            creator_id,
            capture_request("https://example.com/article#first", None, Some("capture-1")),
            1_000,
        )
        .await
        .unwrap();
        assert!(first.created);

        let retry = create(
            state.pool(),
            workspace_id,
            creator_id,
            capture_request(
                "https://example.com/article#second",
                None,
                Some("capture-1"),
            ),
            2_000,
        )
        .await
        .unwrap();
        assert!(!retry.created);
        assert_eq!(retry.item.uid, first.item.uid);
        assert_eq!(retry.item.captures.len(), 1);

        let recapture = create(
            state.pool(),
            workspace_id,
            creator_id,
            capture_request(
                "https://EXAMPLE.com:443/article",
                Some("Filled title"),
                Some("capture-2"),
            ),
            3_000,
        )
        .await
        .unwrap();
        assert!(!recapture.created);
        assert_eq!(recapture.item.uid, first.item.uid);
        assert_eq!(recapture.item.title, "Filled title");
        assert_eq!(recapture.item.site_name.as_deref(), Some("example.com"));
        assert_eq!(recapture.item.captures.len(), 2);

        let conflict = create(
            state.pool(),
            workspace_id,
            creator_id,
            capture_request("https://example.net/other", None, Some("capture-1")),
            4_000,
        )
        .await
        .unwrap_err();
        assert!(matches!(
            conflict,
            crate::error::AppError::Client {
                status: axum::http::StatusCode::CONFLICT,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn searches_the_normalized_url() {
        let (_directory, state, workspace_id, creator_id) = fixture().await;
        create(
            state.pool(),
            workspace_id,
            creator_id,
            capture_request(
                "HTTPS://Example.COM:443/a/../normalized-target#fragment",
                None,
                None,
            ),
            1_000,
        )
        .await
        .unwrap();

        let result = list(
            state.pool(),
            workspace_id,
            ListLibraryItemsOptions {
                query: Some("example.com/normalized-target"),
                ..ListLibraryItemsOptions::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(result.total, 1);
        assert_eq!(result.items.len(), 1);
        assert!(result.items[0].captures.is_empty());
        assert_eq!(
            super::get(state.pool(), workspace_id, &result.items[0].uid)
                .await
                .unwrap()
                .captures
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn serializes_concurrent_idempotent_captures() {
        let (_directory, state, workspace_id, creator_id) = fixture().await;
        let first = create(
            state.pool(),
            workspace_id,
            creator_id,
            capture_request("https://example.com/concurrent", None, Some("same-key")),
            1_000,
        );
        let second = create(
            state.pool(),
            workspace_id,
            creator_id,
            capture_request(
                "https://example.com/concurrent#fragment",
                None,
                Some("same-key"),
            ),
            1_001,
        );
        let (first, second) = tokio::join!(first, second);
        let first = first.unwrap();
        let second = second.unwrap();
        assert_eq!(first.item.uid, second.item.uid);

        let item = super::get(state.pool(), workspace_id, &first.item.uid)
            .await
            .unwrap();
        assert_eq!(item.captures.len(), 1);
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM library_items WHERE workspace_id = ?"
            )
            .bind(workspace_id)
            .fetch_one(state.pool())
            .await
            .unwrap(),
            1
        );
    }

    #[tokio::test]
    async fn serializes_concurrent_url_upserts_and_key_conflicts() {
        let (_directory, state, workspace_id, creator_id) = fixture().await;
        let left = create(
            state.pool(),
            workspace_id,
            creator_id,
            capture_request("https://example.com/shared#left", None, Some("left-key")),
            1_000,
        );
        let right = create(
            state.pool(),
            workspace_id,
            creator_id,
            capture_request(
                "https://EXAMPLE.com:443/shared#right",
                None,
                Some("right-key"),
            ),
            1_001,
        );
        let (left, right) = tokio::join!(left, right);
        let left = left.unwrap();
        let right = right.unwrap();
        assert_eq!(left.item.uid, right.item.uid);
        assert_eq!(
            super::get(state.pool(), workspace_id, &left.item.uid)
                .await
                .unwrap()
                .captures
                .len(),
            2
        );

        let first = create(
            state.pool(),
            workspace_id,
            creator_id,
            capture_request("https://example.com/key-a", None, Some("contested-key")),
            2_000,
        );
        let second = create(
            state.pool(),
            workspace_id,
            creator_id,
            capture_request("https://example.com/key-b", None, Some("contested-key")),
            2_001,
        );
        let (first, second) = tokio::join!(first, second);
        assert!(first.is_ok() ^ second.is_ok());
        let conflict = if let Err(error) = first {
            error
        } else {
            second.unwrap_err()
        };
        assert!(matches!(
            conflict,
            crate::error::AppError::Client {
                status: axum::http::StatusCode::CONFLICT,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn serializes_concurrent_partial_updates_without_losing_fields() {
        let (_directory, state, workspace_id, creator_id) = fixture().await;
        let item = create(
            state.pool(),
            workspace_id,
            creator_id,
            capture_request("https://example.com/patch", None, None),
            1_000,
        )
        .await
        .unwrap()
        .item;
        let title: UpdateLibraryItemRequest =
            serde_json::from_value(json!({"title": "Updated title"})).unwrap();
        let starred: UpdateLibraryItemRequest =
            serde_json::from_value(json!({"starred": true})).unwrap();

        let (title, starred) = tokio::join!(
            update(state.pool(), workspace_id, &item.uid, title, 2_000),
            update(state.pool(), workspace_id, &item.uid, starred, 2_001)
        );
        assert!(title.is_ok());
        assert!(starred.is_ok());
        let item = super::get(state.pool(), workspace_id, &item.uid)
            .await
            .unwrap();
        assert_eq!(item.title, "Updated title");
        assert!(item.starred);
    }

    #[tokio::test]
    async fn rejects_tag_union_overflow_without_appending_a_capture() {
        let (_directory, state, workspace_id, creator_id) = fixture().await;
        let initial = create(
            state.pool(),
            workspace_id,
            creator_id,
            CreateLibraryItemRequest {
                url: "https://example.com/tags".to_owned(),
                title: None,
                selection: None,
                note: None,
                tags: Some((0..MAX_TAGS).map(|index| format!("tag{index}")).collect()),
                idempotency_key: Some("initial-tags".to_owned()),
            },
            1_000,
        )
        .await
        .unwrap()
        .item;

        let overflow = create(
            state.pool(),
            workspace_id,
            creator_id,
            CreateLibraryItemRequest {
                url: "https://example.com/tags#again".to_owned(),
                title: None,
                selection: None,
                note: None,
                tags: Some(vec!["overflow".to_owned()]),
                idempotency_key: Some("overflow-tags".to_owned()),
            },
            2_000,
        )
        .await
        .unwrap_err();
        assert!(matches!(
            overflow,
            crate::error::AppError::Client {
                status: axum::http::StatusCode::UNPROCESSABLE_ENTITY,
                ..
            }
        ));
        let unchanged = super::get(state.pool(), workspace_id, &initial.uid)
            .await
            .unwrap();
        assert_eq!(unchanged.tags.len(), MAX_TAGS);
        assert_eq!(unchanged.captures.len(), 1);
    }

    #[tokio::test]
    async fn delete_rejects_an_orphan_library_object() {
        let (_directory, state, workspace_id, creator_id) = fixture().await;
        let uid = Ulid::generate().to_string();
        sqlx::query(
            r#"
            INSERT INTO objects
              (uid, workspace_id, creator_id, object_type, created_at, updated_at)
            VALUES (?, ?, ?, 'LIBRARY_ITEM', 0, 0)
            "#,
        )
        .bind(&uid)
        .bind(workspace_id)
        .bind(creator_id)
        .execute(state.pool())
        .await
        .unwrap();

        let error = super::delete(state.pool(), workspace_id, &uid)
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            crate::error::AppError::Client {
                status: axum::http::StatusCode::NOT_FOUND,
                ..
            }
        ));
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM objects WHERE workspace_id = ? AND uid = ?"
            )
            .bind(workspace_id)
            .bind(uid)
            .fetch_one(state.pool())
            .await
            .unwrap(),
            1
        );
    }
}
