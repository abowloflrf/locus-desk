use std::time::Duration;

use sha2::{Digest, Sha256};
use sqlx::{FromRow, Sqlite, SqlitePool, Transaction};
use tokio::sync::watch;
use ulid::Ulid;
use url::Url;

use crate::{
    content::{
        ExtractedDocument, MAX_READER_BYTES, MAX_SOURCE_BYTES, PageFetcher, SecurePageFetcher,
    },
    error::{AppError, AppResult},
    state::AppState,
};

const JOB_TYPE_FETCH_LIBRARY_ITEM: &str = "FETCH_LIBRARY_ITEM";
const DEFAULT_MAX_ATTEMPTS: i64 = 5;
const LEASE_DURATION_MS: i64 = 60_000;
const IDLE_POLL_INTERVAL: Duration = Duration::from_millis(750);
const ERROR_POLL_INTERVAL: Duration = Duration::from_secs(2);
const MAX_AUTHOR_CHARACTERS: usize = 500;
const MAX_EXCERPT_BYTES: usize = 65_536;

#[derive(Debug, FromRow)]
struct ClaimedJob {
    id: i64,
    uid: String,
    workspace_id: i64,
    object_id: i64,
    attempt_count: i64,
    max_attempts: i64,
}

#[derive(Debug, FromRow)]
struct LibraryFetchTarget {
    normalized_url: String,
}

pub(crate) async fn enqueue_library_fetch(
    transaction: &mut Transaction<'_, Sqlite>,
    workspace_id: i64,
    object_id: i64,
    now: i64,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO jobs (
          uid, workspace_id, object_id, job_type, status,
          attempt_count, max_attempts, run_after,
          lease_owner, lease_expires_at, last_error, created_at, updated_at
        )
        SELECT ?, o.workspace_id, o.id, ?, 'PENDING', 0, ?, ?, NULL, NULL, NULL, ?, ?
        FROM objects o
        WHERE o.id = ? AND o.workspace_id = ? AND o.object_type = 'LIBRARY_ITEM'
        ON CONFLICT(workspace_id, object_id, job_type)
          WHERE status IN ('PENDING', 'RUNNING', 'RETRY')
        DO NOTHING
        "#,
    )
    .bind(Ulid::generate().to_string())
    .bind(JOB_TYPE_FETCH_LIBRARY_ITEM)
    .bind(DEFAULT_MAX_ATTEMPTS)
    .bind(now)
    .bind(now)
    .bind(now)
    .bind(object_id)
    .bind(workspace_id)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

pub async fn run_worker(state: AppState, mut shutdown: watch::Receiver<bool>) {
    let worker_id = Ulid::generate().to_string();
    let fetcher = SecurePageFetcher::default();

    loop {
        if *shutdown.borrow() {
            break;
        }
        match process_one_with_now(state.pool(), &fetcher, &worker_id, || {
            state.clock().now().timestamp_millis()
        })
        .await
        {
            Ok(true) => continue,
            Ok(false) => {
                if wait_or_shutdown(&mut shutdown, IDLE_POLL_INTERVAL).await {
                    break;
                }
            }
            Err(error) => {
                tracing::error!(error = %error, "content worker iteration failed");
                if wait_or_shutdown(&mut shutdown, ERROR_POLL_INTERVAL).await {
                    break;
                }
            }
        }
    }
}

pub async fn process_one<F: PageFetcher + ?Sized>(
    pool: &SqlitePool,
    fetcher: &F,
    worker_id: &str,
    now: i64,
) -> AppResult<bool> {
    process_one_with_now(pool, fetcher, worker_id, || now).await
}

async fn process_one_with_now<F, N>(
    pool: &SqlitePool,
    fetcher: &F,
    worker_id: &str,
    mut now: N,
) -> AppResult<bool>
where
    F: PageFetcher + ?Sized,
    N: FnMut() -> i64,
{
    let claim_time = now();
    let Some(job) = claim_next(pool, worker_id, claim_time).await? else {
        return Ok(false);
    };
    if job.attempt_count > job.max_attempts {
        let finished_at = now();
        fail_claimed_job(
            pool,
            &job,
            worker_id,
            finished_at,
            "Fetch stopped after the final worker lease expired",
        )
        .await?;
        return Ok(true);
    }

    let target = sqlx::query_as::<_, LibraryFetchTarget>(
        r#"
        SELECT li.normalized_url
        FROM library_items li
        JOIN objects o
          ON o.id = li.object_id AND o.workspace_id = li.workspace_id
        WHERE li.object_id = ? AND li.workspace_id = ?
          AND o.workspace_id = ? AND o.object_type = 'LIBRARY_ITEM'
        "#,
    )
    .bind(job.object_id)
    .bind(job.workspace_id)
    .bind(job.workspace_id)
    .fetch_optional(pool)
    .await?;

    let Some(target) = target else {
        mark_obsolete(pool, &job, worker_id, now()).await?;
        return Ok(true);
    };

    match fetcher.fetch(&target.normalized_url).await {
        Ok(document) => {
            let finished_at = now();
            if let Err(error) =
                complete_claimed_job(pool, &job, worker_id, document, finished_at).await
            {
                tracing::error!(job_uid = %job.uid, error = %error, "Library content could not be stored");
                fail_claimed_job(
                    pool,
                    &job,
                    worker_id,
                    finished_at,
                    "The extracted page content could not be stored",
                )
                .await?;
            }
        }
        Err(error) => {
            fail_claimed_job(pool, &job, worker_id, now(), error.public_message()).await?;
        }
    }
    Ok(true)
}

async fn claim_next(pool: &SqlitePool, worker_id: &str, now: i64) -> AppResult<Option<ClaimedJob>> {
    Ok(sqlx::query_as::<_, ClaimedJob>(
        r#"
        UPDATE jobs
        SET
          status = 'RUNNING',
          attempt_count = attempt_count + 1,
          lease_owner = ?,
          lease_expires_at = ?,
          updated_at = ?
        WHERE id = (
          SELECT id
          FROM jobs
          WHERE job_type = ?
            AND (
              (status IN ('PENDING', 'RETRY') AND attempt_count < max_attempts AND run_after <= ?)
              OR (status = 'RUNNING' AND lease_expires_at IS NOT NULL AND lease_expires_at <= ?)
            )
          ORDER BY run_after ASC, id ASC
          LIMIT 1
        )
        RETURNING id, uid, workspace_id, object_id, attempt_count, max_attempts
        "#,
    )
    .bind(worker_id)
    .bind(now.saturating_add(LEASE_DURATION_MS))
    .bind(now)
    .bind(JOB_TYPE_FETCH_LIBRARY_ITEM)
    .bind(now)
    .bind(now)
    .fetch_optional(pool)
    .await?)
}

async fn complete_claimed_job(
    pool: &SqlitePool,
    job: &ClaimedJob,
    worker_id: &str,
    document: ExtractedDocument,
    now: i64,
) -> AppResult<()> {
    validate_extracted_document(&document)?;
    let mut transaction = pool.begin().await?;
    if !extend_claim(&mut transaction, job, worker_id, now).await? {
        transaction.rollback().await?;
        return Ok(());
    }

    let source_blob = store_blob(
        &mut transaction,
        job.workspace_id,
        "text/html; profile=source",
        &document.source_html,
        now,
    )
    .await?;
    let reader_html_blob = store_blob(
        &mut transaction,
        job.workspace_id,
        "text/html; profile=reader",
        document.safe_html.as_bytes(),
        now,
    )
    .await?;
    let reader_text_blob = store_blob(
        &mut transaction,
        job.workspace_id,
        "text/plain; charset=utf-8",
        document.plain_text.as_bytes(),
        now,
    )
    .await?;
    link_blob(
        &mut transaction,
        job.workspace_id,
        job.object_id,
        source_blob,
        "SOURCE_HTML",
    )
    .await?;
    link_blob(
        &mut transaction,
        job.workspace_id,
        job.object_id,
        reader_html_blob,
        "READER_HTML",
    )
    .await?;
    link_blob(
        &mut transaction,
        job.workspace_id,
        job.object_id,
        reader_text_blob,
        "READER_TEXT",
    )
    .await?;

    let canonical_url = choose_canonical_url(&mut transaction, job, &document).await?;
    let site_name = Url::parse(&document.final_url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned));
    let title = document.title.as_deref().unwrap_or_default();
    let author = document.author.as_deref().map(|value| {
        value
            .trim()
            .chars()
            .take(MAX_AUTHOR_CHARACTERS)
            .collect::<String>()
    });
    let excerpt = truncate_utf8(document.excerpt.trim(), MAX_EXCERPT_BYTES);
    let result = sqlx::query(
        r#"
        UPDATE library_items
        SET
          canonical_url = COALESCE(?, canonical_url),
          title = CASE WHEN title = '' AND ? <> '' THEN ? ELSE title END,
          site_name = COALESCE(?, site_name),
          author = ?,
          published_at = ?,
          excerpt = ?,
          item_kind = 'ARTICLE',
          processing_status = 'READY',
          last_error = NULL,
          fetched_at = ?,
          content_hash = ?,
          content_version = content_version + 1
        WHERE object_id = ? AND workspace_id = ?
        "#,
    )
    .bind(canonical_url.as_deref())
    .bind(title)
    .bind(title)
    .bind(site_name.as_deref())
    .bind(author.as_deref())
    .bind(document.published_at)
    .bind(excerpt)
    .bind(now)
    .bind(&document.content_hash)
    .bind(job.object_id)
    .bind(job.workspace_id)
    .execute(&mut *transaction)
    .await?;
    if result.rows_affected() != 1 {
        transaction.rollback().await?;
        return Ok(());
    }

    sqlx::query(
        "UPDATE objects SET updated_at = MAX(updated_at, ?) WHERE id = ? AND workspace_id = ? AND object_type = 'LIBRARY_ITEM'",
    )
    .bind(now)
    .bind(job.object_id)
    .bind(job.workspace_id)
    .execute(&mut *transaction)
    .await?;
    sqlx::query(
        r#"
        UPDATE jobs
        SET status = 'COMPLETED', lease_owner = NULL, lease_expires_at = NULL,
            last_error = NULL, updated_at = ?
        WHERE id = ? AND workspace_id = ? AND status = 'RUNNING' AND lease_owner = ?
          AND lease_expires_at > ?
        "#,
    )
    .bind(now)
    .bind(job.id)
    .bind(job.workspace_id)
    .bind(worker_id)
    .bind(now)
    .execute(&mut *transaction)
    .await?;
    cleanup_orphan_blobs(&mut transaction, job.workspace_id).await?;
    transaction.commit().await?;
    tracing::info!(job_uid = %job.uid, "Library content fetch completed");
    Ok(())
}

async fn fail_claimed_job(
    pool: &SqlitePool,
    job: &ClaimedJob,
    worker_id: &str,
    now: i64,
    message: &str,
) -> AppResult<()> {
    let final_attempt = job.attempt_count >= job.max_attempts;
    let next_status = if final_attempt { "DEAD" } else { "RETRY" };
    let processing_status = if final_attempt { "FAILED" } else { "PENDING" };
    let run_after = now.saturating_add(retry_delay_ms(job.attempt_count));
    let mut transaction = pool.begin().await?;
    let result = sqlx::query(
        r#"
        UPDATE jobs
        SET status = ?, run_after = ?, lease_owner = NULL, lease_expires_at = NULL,
            last_error = ?, updated_at = ?
        WHERE id = ? AND workspace_id = ? AND status = 'RUNNING' AND lease_owner = ?
          AND lease_expires_at > ?
        "#,
    )
    .bind(next_status)
    .bind(run_after)
    .bind(message)
    .bind(now)
    .bind(job.id)
    .bind(job.workspace_id)
    .bind(worker_id)
    .bind(now)
    .execute(&mut *transaction)
    .await?;
    if result.rows_affected() == 0 {
        transaction.rollback().await?;
        return Ok(());
    }
    sqlx::query(
        r#"
        UPDATE library_items
        SET processing_status = ?, last_error = ?
        WHERE object_id = ? AND workspace_id = ?
        "#,
    )
    .bind(processing_status)
    .bind(message)
    .bind(job.object_id)
    .bind(job.workspace_id)
    .execute(&mut *transaction)
    .await?;
    sqlx::query(
        "UPDATE objects SET updated_at = MAX(updated_at, ?) WHERE id = ? AND workspace_id = ? AND object_type = 'LIBRARY_ITEM'",
    )
    .bind(now)
    .bind(job.object_id)
    .bind(job.workspace_id)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    tracing::warn!(job_uid = %job.uid, final_attempt, "Library content fetch failed");
    Ok(())
}

async fn mark_obsolete(
    pool: &SqlitePool,
    job: &ClaimedJob,
    worker_id: &str,
    now: i64,
) -> AppResult<()> {
    sqlx::query(
        r#"
        UPDATE jobs
        SET status = 'COMPLETED', lease_owner = NULL, lease_expires_at = NULL,
            last_error = NULL, updated_at = ?
        WHERE id = ? AND workspace_id = ? AND status = 'RUNNING' AND lease_owner = ?
          AND lease_expires_at > ?
        "#,
    )
    .bind(now)
    .bind(job.id)
    .bind(job.workspace_id)
    .bind(worker_id)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(())
}

async fn extend_claim(
    transaction: &mut Transaction<'_, Sqlite>,
    job: &ClaimedJob,
    worker_id: &str,
    now: i64,
) -> AppResult<bool> {
    let result = sqlx::query(
        r#"
        UPDATE jobs
        SET lease_expires_at = ?, updated_at = ?
        WHERE id = ? AND workspace_id = ? AND status = 'RUNNING' AND lease_owner = ?
          AND lease_expires_at > ?
        "#,
    )
    .bind(now.saturating_add(LEASE_DURATION_MS))
    .bind(now)
    .bind(job.id)
    .bind(job.workspace_id)
    .bind(worker_id)
    .bind(now)
    .execute(&mut **transaction)
    .await?;
    Ok(result.rows_affected() == 1)
}

async fn store_blob(
    transaction: &mut Transaction<'_, Sqlite>,
    workspace_id: i64,
    mime_type: &str,
    body: &[u8],
    now: i64,
) -> AppResult<i64> {
    let sha256 = sha256_hex(body);
    sqlx::query(
        r#"
        INSERT INTO blobs (uid, workspace_id, sha256, mime_type, byte_len, body, created_at)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(workspace_id, sha256) DO NOTHING
        "#,
    )
    .bind(Ulid::generate().to_string())
    .bind(workspace_id)
    .bind(&sha256)
    .bind(mime_type)
    .bind(i64::try_from(body.len()).map_err(|_| {
        AppError::Internal("Library content length cannot be represented".to_owned())
    })?)
    .bind(body)
    .bind(now)
    .execute(&mut **transaction)
    .await?;

    let (id, stored_body) = sqlx::query_as::<_, (i64, Vec<u8>)>(
        "SELECT id, body FROM blobs WHERE workspace_id = ? AND sha256 = ?",
    )
    .bind(workspace_id)
    .bind(&sha256)
    .fetch_one(&mut **transaction)
    .await?;
    if stored_body != body {
        return Err(AppError::Internal(
            "Library blob hash collision detected".to_owned(),
        ));
    }
    Ok(id)
}

async fn link_blob(
    transaction: &mut Transaction<'_, Sqlite>,
    workspace_id: i64,
    object_id: i64,
    blob_id: i64,
    purpose: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO object_blobs (object_id, workspace_id, blob_id, purpose)
        VALUES (?, ?, ?, ?)
        ON CONFLICT(object_id, purpose)
        DO UPDATE SET workspace_id = excluded.workspace_id, blob_id = excluded.blob_id
        "#,
    )
    .bind(object_id)
    .bind(workspace_id)
    .bind(blob_id)
    .bind(purpose)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

pub(crate) async fn cleanup_orphan_blobs(
    transaction: &mut Transaction<'_, Sqlite>,
    workspace_id: i64,
) -> AppResult<()> {
    sqlx::query(
        r#"
        DELETE FROM blobs
        WHERE workspace_id = ?
          AND NOT EXISTS (
            SELECT 1 FROM object_blobs ob
            WHERE ob.blob_id = blobs.id AND ob.workspace_id = blobs.workspace_id
          )
        "#,
    )
    .bind(workspace_id)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn choose_canonical_url(
    transaction: &mut Transaction<'_, Sqlite>,
    job: &ClaimedJob,
    document: &ExtractedDocument,
) -> AppResult<Option<String>> {
    let candidate = document
        .canonical_url
        .as_deref()
        .or(Some(document.final_url.as_str()));
    let Some(candidate) = candidate else {
        return Ok(None);
    };
    let conflicts = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT EXISTS(
          SELECT 1 FROM library_items
          WHERE workspace_id = ? AND object_id <> ?
            AND (normalized_url = ? OR canonical_url = ?)
        )
        "#,
    )
    .bind(job.workspace_id)
    .bind(job.object_id)
    .bind(candidate)
    .bind(candidate)
    .fetch_one(&mut **transaction)
    .await?;
    Ok((conflicts == 0).then(|| candidate.to_owned()))
}

fn validate_extracted_document(document: &ExtractedDocument) -> AppResult<()> {
    if document.safe_html.is_empty() || document.plain_text.trim().is_empty() {
        return Err(AppError::Internal(
            "content extractor returned an empty reader document".to_owned(),
        ));
    }
    if document.source_html.len() > MAX_SOURCE_BYTES
        || document.safe_html.len() > MAX_READER_BYTES
        || document.plain_text.len() > MAX_READER_BYTES
    {
        return Err(AppError::Internal(
            "content extractor returned an oversized document".to_owned(),
        ));
    }
    let computed = sha256_hex(document.safe_html.as_bytes());
    if computed != document.content_hash {
        return Err(AppError::Internal(
            "content extractor returned an invalid content hash".to_owned(),
        ));
    }
    Ok(())
}

fn retry_delay_ms(attempt_count: i64) -> i64 {
    let exponent = u32::try_from(attempt_count.saturating_sub(1).clamp(0, 6)).unwrap_or(6);
    5_000_i64.saturating_mul(2_i64.saturating_pow(exponent))
}

fn truncate_utf8(value: &str, max_bytes: usize) -> &str {
    if value.len() <= max_bytes {
        return value;
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    &value[..end]
}

fn sha256_hex(body: &[u8]) -> String {
    format!("{:x}", Sha256::digest(body))
}

async fn wait_or_shutdown(shutdown: &mut watch::Receiver<bool>, duration: Duration) -> bool {
    tokio::select! {
        _ = tokio::time::sleep(duration) => false,
        result = shutdown.changed() => result.is_err() || *shutdown.borrow(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use tempfile::TempDir;

    use super::{
        ClaimedJob, claim_next, complete_claimed_job, fail_claimed_job, process_one,
        process_one_with_now, sha256_hex,
    };
    use crate::{
        config::Config,
        content::{ContentError, ExtractedDocument, PageFetcher},
        library::{
            self, CreateLibraryItemRequest, LibraryProcessingStatus, ListLibraryItemsOptions,
        },
        state::AppState,
    };

    struct StaticFetcher;

    #[async_trait::async_trait]
    impl PageFetcher for StaticFetcher {
        async fn fetch(&self, _url: &str) -> Result<ExtractedDocument, ContentError> {
            Ok(document())
        }
    }

    struct FailingFetcher;

    #[async_trait::async_trait]
    impl PageFetcher for FailingFetcher {
        async fn fetch(&self, _url: &str) -> Result<ExtractedDocument, ContentError> {
            Err(ContentError::RequestFailed)
        }
    }

    async fn fixture() -> (TempDir, AppState, i64, i64) {
        let directory = tempfile::tempdir().unwrap();
        let config = Config::for_test(
            directory.path().join("data"),
            "admin",
            "correct horse battery staple",
            "UTC".parse().unwrap(),
        );
        let state = AppState::initialize(config).await.unwrap();
        let identity = sqlx::query_as::<_, (i64, i64)>(
            "SELECT workspaces.id, users.id FROM workspaces JOIN users ON users.id = workspaces.created_by",
        )
        .fetch_one(state.pool())
        .await
        .unwrap();
        (directory, state, identity.0, identity.1)
    }

    async fn create_item(state: &AppState, workspace_id: i64, user_id: i64) -> String {
        library::create(
            state.pool(),
            workspace_id,
            user_id,
            CreateLibraryItemRequest {
                url: "https://example.com/article".to_owned(),
                title: None,
                selection: None,
                note: None,
                tags: None,
                idempotency_key: None,
            },
            1_000,
        )
        .await
        .unwrap()
        .item
        .uid
    }

    fn document() -> ExtractedDocument {
        let safe_html =
            "<article><h1>Fetched title</h1><p>A reader-only searchable phrase.</p></article>"
                .to_owned();
        ExtractedDocument {
            final_url: "https://example.com/article".to_owned(),
            canonical_url: Some("https://example.com/article".to_owned()),
            title: Some("Fetched title".to_owned()),
            author: Some("Reader Author".to_owned()),
            published_at: Some(1_750_000_000_000),
            excerpt: "A reader-only searchable phrase.".to_owned(),
            content_hash: sha256_hex(safe_html.as_bytes()),
            safe_html,
            plain_text: "Fetched title A reader-only searchable phrase.".to_owned(),
            source_html: b"<html><body><article>source</article></body></html>".to_vec(),
        }
    }

    #[tokio::test]
    async fn worker_persists_reader_content_searches_it_and_cleans_blobs() {
        let (_directory, state, workspace_id, user_id) = fixture().await;
        let uid = create_item(&state, workspace_id, user_id).await;
        let pending = library::get(state.pool(), workspace_id, &uid)
            .await
            .unwrap();
        assert_eq!(pending.processing_status, LibraryProcessingStatus::Pending);
        assert!(!pending.content_available);

        assert!(
            process_one(state.pool(), &StaticFetcher, "worker-a", 2_000)
                .await
                .unwrap()
        );
        let ready = library::get(state.pool(), workspace_id, &uid)
            .await
            .unwrap();
        assert_eq!(ready.processing_status, LibraryProcessingStatus::Ready);
        assert!(ready.content_available);
        assert_eq!(ready.title, "Fetched title");
        assert_eq!(ready.author.as_deref(), Some("Reader Author"));
        assert_eq!(ready.content_version, 1);
        let content = library::get_content(state.pool(), workspace_id, &uid)
            .await
            .unwrap();
        assert!(content.safe_html.contains("reader-only searchable"));
        assert!(content.plain_text.contains("reader-only searchable"));

        let search = library::list(
            state.pool(),
            workspace_id,
            ListLibraryItemsOptions {
                query: Some("reader-only searchable"),
                ..ListLibraryItemsOptions::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(search.total, 1);
        assert!(search.items[0].captures.is_empty());

        let retried = library::retry_fetch(state.pool(), workspace_id, &uid, 3_000)
            .await
            .unwrap();
        assert_eq!(retried.processing_status, LibraryProcessingStatus::Pending);
        assert!(
            process_one(state.pool(), &StaticFetcher, "worker-a", 3_001)
                .await
                .unwrap()
        );
        assert_eq!(
            library::get(state.pool(), workspace_id, &uid)
                .await
                .unwrap()
                .content_version,
            2
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM blobs")
                .fetch_one(state.pool())
                .await
                .unwrap(),
            3,
            "identical refetches should reuse content-addressed blobs"
        );

        library::delete(state.pool(), workspace_id, &uid)
            .await
            .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM blobs")
                .fetch_one(state.pool())
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM jobs")
                .fetch_one(state.pool())
                .await
                .unwrap(),
            0
        );
    }

    #[tokio::test]
    async fn worker_retries_with_backoff_then_marks_the_item_failed() {
        let (_directory, state, workspace_id, user_id) = fixture().await;
        let uid = create_item(&state, workspace_id, user_id).await;
        let mut now = 1_000;

        for attempt in 1..=5 {
            let job: ClaimedJob = claim_next(state.pool(), "worker-a", now)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(job.attempt_count, attempt);
            fail_claimed_job(
                state.pool(),
                &job,
                "worker-a",
                now,
                "The page could not be fetched",
            )
            .await
            .unwrap();
            if attempt < 5 {
                let (status, run_after) = sqlx::query_as::<_, (String, i64)>(
                    "SELECT status, run_after FROM jobs WHERE id = ?",
                )
                .bind(job.id)
                .fetch_one(state.pool())
                .await
                .unwrap();
                assert_eq!(status, "RETRY");
                assert!(run_after > now);
                now = run_after;
            }
        }

        let item = library::get(state.pool(), workspace_id, &uid)
            .await
            .unwrap();
        assert_eq!(item.processing_status, LibraryProcessingStatus::Failed);
        assert_eq!(
            item.last_error.as_deref(),
            Some("The page could not be fetched")
        );
        assert_eq!(
            sqlx::query_scalar::<_, String>("SELECT status FROM jobs")
                .fetch_one(state.pool())
                .await
                .unwrap(),
            "DEAD"
        );
    }

    #[tokio::test]
    async fn an_expired_lease_cannot_publish_after_another_worker_reclaims_it() {
        let (_directory, state, workspace_id, user_id) = fixture().await;
        let uid = create_item(&state, workspace_id, user_id).await;
        let stale = claim_next(state.pool(), "worker-a", 1_000)
            .await
            .unwrap()
            .unwrap();
        let current = claim_next(state.pool(), "worker-b", 61_000)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stale.id, current.id);
        assert_eq!(current.attempt_count, 2);

        complete_claimed_job(state.pool(), &stale, "worker-a", document(), 61_001)
            .await
            .unwrap();
        assert_eq!(
            library::get(state.pool(), workspace_id, &uid)
                .await
                .unwrap()
                .processing_status,
            LibraryProcessingStatus::Pending
        );

        complete_claimed_job(state.pool(), &current, "worker-b", document(), 61_002)
            .await
            .unwrap();
        assert_eq!(
            library::get(state.pool(), workspace_id, &uid)
                .await
                .unwrap()
                .processing_status,
            LibraryProcessingStatus::Ready
        );
    }

    #[tokio::test]
    async fn an_expired_uncontested_lease_must_still_be_reclaimed_before_publish() {
        let (_directory, state, workspace_id, user_id) = fixture().await;
        let uid = create_item(&state, workspace_id, user_id).await;
        let expired = claim_next(state.pool(), "worker-a", 1_000)
            .await
            .unwrap()
            .unwrap();

        complete_claimed_job(state.pool(), &expired, "worker-a", document(), 61_000)
            .await
            .unwrap();
        assert_eq!(
            library::get(state.pool(), workspace_id, &uid)
                .await
                .unwrap()
                .processing_status,
            LibraryProcessingStatus::Pending
        );

        let reclaimed = claim_next(state.pool(), "worker-b", 61_000)
            .await
            .unwrap()
            .unwrap();
        complete_claimed_job(state.pool(), &reclaimed, "worker-b", document(), 61_001)
            .await
            .unwrap();
        assert_eq!(
            library::get(state.pool(), workspace_id, &uid)
                .await
                .unwrap()
                .processing_status,
            LibraryProcessingStatus::Ready
        );
    }

    #[tokio::test]
    async fn retry_backoff_uses_fetch_completion_time() {
        let (_directory, state, workspace_id, user_id) = fixture().await;
        create_item(&state, workspace_id, user_id).await;
        let mut times = VecDeque::from([1_000, 41_000]);

        assert!(
            process_one_with_now(state.pool(), &FailingFetcher, "worker-a", || {
                times.pop_front().expect("test clock has enough values")
            })
            .await
            .unwrap()
        );

        let (status, updated_at, run_after) = sqlx::query_as::<_, (String, i64, i64)>(
            "SELECT status, updated_at, run_after FROM jobs",
        )
        .fetch_one(state.pool())
        .await
        .unwrap();
        assert_eq!(status, "RETRY");
        assert_eq!(updated_at, 41_000);
        assert_eq!(run_after, 46_000);
    }

    #[tokio::test]
    async fn worker_completion_does_not_move_object_time_backwards() {
        let (_directory, state, workspace_id, user_id) = fixture().await;
        create_item(&state, workspace_id, user_id).await;
        let claimed = claim_next(state.pool(), "worker-a", 1_000)
            .await
            .unwrap()
            .unwrap();
        sqlx::query("UPDATE objects SET updated_at = 50_000 WHERE id = ? AND workspace_id = ?")
            .bind(claimed.object_id)
            .bind(workspace_id)
            .execute(state.pool())
            .await
            .unwrap();

        complete_claimed_job(state.pool(), &claimed, "worker-a", document(), 2_000)
            .await
            .unwrap();

        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT updated_at FROM objects WHERE id = ? AND workspace_id = ?",
            )
            .bind(claimed.object_id)
            .bind(workspace_id)
            .fetch_one(state.pool())
            .await
            .unwrap(),
            50_000
        );
    }

    #[tokio::test]
    async fn canonical_conflicts_preserve_the_last_saved_canonical_url() {
        let (_directory, state, workspace_id, user_id) = fixture().await;
        let uid = create_item(&state, workspace_id, user_id).await;
        sqlx::query(
            "UPDATE library_items SET canonical_url = 'https://example.com/previous' WHERE workspace_id = ?",
        )
        .bind(workspace_id)
        .execute(state.pool())
        .await
        .unwrap();
        library::create(
            state.pool(),
            workspace_id,
            user_id,
            CreateLibraryItemRequest {
                url: "https://example.com/conflict".to_owned(),
                title: None,
                selection: None,
                note: None,
                tags: None,
                idempotency_key: None,
            },
            1_100,
        )
        .await
        .unwrap();
        let claimed = claim_next(state.pool(), "worker-a", 2_000)
            .await
            .unwrap()
            .unwrap();
        let mut conflicting = document();
        conflicting.canonical_url = Some("https://example.com/conflict".to_owned());

        complete_claimed_job(state.pool(), &claimed, "worker-a", conflicting, 2_001)
            .await
            .unwrap();

        assert_eq!(
            library::get(state.pool(), workspace_id, &uid)
                .await
                .unwrap()
                .canonical_url
                .as_deref(),
            Some("https://example.com/previous")
        );
    }
}
