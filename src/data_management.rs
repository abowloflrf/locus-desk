use std::{
    collections::{BTreeMap, HashSet},
    error::Error,
    fmt::{self, Display, Formatter, Write as _},
    fs::{self, File, OpenOptions},
    io::{self, Write as _},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, SystemTimeError, UNIX_EPOCH},
};

use chrono::{Datelike, Utc};
use serde::Serialize;
use sqlx::{
    AssertSqlSafe, Connection, Row, Sqlite, SqliteConnection, SqlitePool, Transaction,
    sqlite::SqliteConnectOptions,
};

const EXPORT_FORMAT_VERSION: u32 = 1;
const BACKUP_METADATA_TABLE: &str = "locus_backup_metadata";
const BACKUP_METADATA_CREATE_SQL: &str = r#"
CREATE TABLE locus_backup_metadata (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    created_at INTEGER NOT NULL,
    application_version TEXT NOT NULL CHECK (length(application_version) > 0),
    schema_version INTEGER NOT NULL CHECK (schema_version > 0),
    git_commit TEXT NOT NULL CHECK (length(git_commit) > 0)
)
"#;
const TEMPORARY_FILE_PREFIX: &str = ".locus-desk-tmp-v1-";
const TEMPORARY_PURPOSES: &[&str] = &["vacuum", "restore", "json", "markdown"];
const REQUIRED_SCHEMA_V1_TABLES: &[&str] = &[
    "users",
    "workspaces",
    "workspace_members",
    "sessions",
    "notes",
    "note_tags",
    "tasks",
    "_sqlx_migrations",
];

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);
pub type DataManagementResult<T> = Result<T, DataManagementError>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataArtifact {
    pub path: PathBuf,
    pub byte_len: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableExport {
    pub format_version: u32,
    pub application_version: String,
    pub schema_version: i64,
    pub exported_at_unix_ms: i64,
    pub users: Vec<ExportUser>,
    pub workspaces: Vec<ExportWorkspace>,
    pub notes: Vec<ExportNote>,
    pub tasks: Vec<ExportTask>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportUser {
    pub uid: String,
    pub username: String,
    pub created_at_unix_ms: i64,
    pub updated_at_unix_ms: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportWorkspace {
    pub uid: String,
    pub name: String,
    pub timezone: String,
    pub creator_uid: String,
    pub created_at_unix_ms: i64,
    pub updated_at_unix_ms: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportNote {
    pub uid: String,
    pub workspace_uid: String,
    pub creator_uid: String,
    pub content: String,
    pub status: String,
    pub pinned: bool,
    pub tags: Vec<String>,
    pub created_at_unix_ms: i64,
    pub updated_at_unix_ms: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportTask {
    pub uid: String,
    pub workspace_uid: String,
    pub creator_uid: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub priority: u8,
    pub due_date: Option<String>,
    pub due_time: Option<String>,
    pub sort_key: i64,
    pub completed_at_unix_ms: Option<i64>,
    pub created_at_unix_ms: i64,
    pub updated_at_unix_ms: i64,
}

#[derive(Debug)]
pub enum DataManagementError {
    InvalidPath {
        path: PathBuf,
        reason: String,
    },
    DestinationExists(PathBuf),
    InvalidBackup {
        path: PathBuf,
        reason: String,
    },
    InvalidDatabaseValue {
        field: &'static str,
        reason: String,
    },
    Io {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    Database(sqlx::Error),
    Serialization(serde_json::Error),
    BackgroundTask(tokio::task::JoinError),
    Clock(SystemTimeError),
}

impl Display for DataManagementError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPath { path, reason } => {
                write!(formatter, "invalid path {}: {reason}", path.display())
            }
            Self::DestinationExists(path) => {
                write!(formatter, "destination already exists: {}", path.display())
            }
            Self::InvalidBackup { path, reason } => {
                write!(
                    formatter,
                    "invalid SQLite backup {}: {reason}",
                    path.display()
                )
            }
            Self::InvalidDatabaseValue { field, reason } => {
                write!(formatter, "invalid database value for {field}: {reason}")
            }
            Self::Io {
                operation,
                path,
                source,
            } => write!(
                formatter,
                "failed to {operation} {}: {source}",
                path.display()
            ),
            Self::Database(error) => write!(formatter, "database operation failed: {error}"),
            Self::Serialization(error) => {
                write!(formatter, "failed to serialize data export: {error}")
            }
            Self::BackgroundTask(error) => {
                write!(formatter, "background file operation failed: {error}")
            }
            Self::Clock(error) => {
                write!(formatter, "system clock is before the Unix epoch: {error}")
            }
        }
    }
}

impl Error for DataManagementError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Database(error) => Some(error),
            Self::Serialization(error) => Some(error),
            Self::BackgroundTask(error) => Some(error),
            Self::Clock(error) => Some(error),
            Self::InvalidPath { .. }
            | Self::DestinationExists(_)
            | Self::InvalidBackup { .. }
            | Self::InvalidDatabaseValue { .. } => None,
        }
    }
}

impl From<sqlx::Error> for DataManagementError {
    fn from(error: sqlx::Error) -> Self {
        Self::Database(error)
    }
}

impl From<serde_json::Error> for DataManagementError {
    fn from(error: serde_json::Error) -> Self {
        Self::Serialization(error)
    }
}

impl From<tokio::task::JoinError> for DataManagementError {
    fn from(error: tokio::task::JoinError) -> Self {
        Self::BackgroundTask(error)
    }
}

impl From<SystemTimeError> for DataManagementError {
    fn from(error: SystemTimeError) -> Self {
        Self::Clock(error)
    }
}

/// Creates a compact, consistent SQLite snapshot using `VACUUM INTO`.
///
/// `destination` must be an absolute path beneath `allowed_directory`, its parent directory must
/// already exist, and the destination must not exist. The snapshot is validated before it is
/// atomically published.
pub async fn create_sqlite_backup(
    pool: &SqlitePool,
    allowed_directory: &Path,
    destination: &Path,
) -> DataManagementResult<DataArtifact> {
    let destination = validate_destination(allowed_directory, destination)?;
    cleanup_stale_temporary_files(
        destination
            .parent()
            .expect("validated destinations always include a parent"),
    )?;
    let (temporary_path, temporary_file) = create_private_temporary_file(&destination, "vacuum")?;
    drop(temporary_file);
    let mut temporary_guard = TemporaryPathGuard::new(temporary_path.clone());
    let sqlite_path = temporary_path
        .to_str()
        .ok_or_else(|| DataManagementError::InvalidPath {
            path: temporary_path.clone(),
            reason: "SQLite backup paths must contain valid UTF-8".to_owned(),
        })?;

    let mut connection = pool.acquire().await?;
    sqlx::query("VACUUM INTO ?1")
        .bind(sqlite_path)
        .execute(&mut *connection)
        .await?;
    drop(connection);

    write_backup_metadata(&temporary_path).await?;
    sync_private_file(temporary_path.clone()).await?;
    validate_sqlite_snapshot(&temporary_path).await?;
    let byte_len = file_len(&temporary_path)?;
    publish_temporary_file(&mut temporary_guard, &destination)?;

    Ok(DataArtifact {
        path: destination,
        byte_len,
    })
}

/// Loads a transactionally consistent, portable view of user-owned data.
///
/// Password hashes, sessions, internal integer IDs, and filesystem paths are never queried.
pub async fn collect_portable_export(pool: &SqlitePool) -> DataManagementResult<PortableExport> {
    let exported_at_unix_ms = unix_millis_now()?;
    let mut transaction = pool.begin().await?;
    let schema_version = read_schema_version(&mut transaction).await?;
    ensure_export_foreign_keys(&mut transaction).await?;

    let users = sqlx::query(
        r#"
        SELECT uid, username, created_at, updated_at
        FROM users
        ORDER BY created_at ASC, id ASC
        "#,
    )
    .fetch_all(&mut *transaction)
    .await?
    .into_iter()
    .map(|row| {
        Ok(ExportUser {
            uid: row.try_get("uid")?,
            username: row.try_get("username")?,
            created_at_unix_ms: row.try_get("created_at")?,
            updated_at_unix_ms: row.try_get("updated_at")?,
        })
    })
    .collect::<Result<Vec<_>, sqlx::Error>>()?;

    let workspaces = sqlx::query(
        r#"
        SELECT
            workspaces.uid,
            workspaces.name,
            workspaces.timezone,
            users.uid AS creator_uid,
            workspaces.created_at,
            workspaces.updated_at
        FROM workspaces
        JOIN users ON users.id = workspaces.created_by
        ORDER BY workspaces.created_at ASC, workspaces.id ASC
        "#,
    )
    .fetch_all(&mut *transaction)
    .await?
    .into_iter()
    .map(|row| {
        Ok(ExportWorkspace {
            uid: row.try_get("uid")?,
            name: row.try_get("name")?,
            timezone: row.try_get("timezone")?,
            creator_uid: row.try_get("creator_uid")?,
            created_at_unix_ms: row.try_get("created_at")?,
            updated_at_unix_ms: row.try_get("updated_at")?,
        })
    })
    .collect::<Result<Vec<_>, sqlx::Error>>()?;

    let mut tags_by_note = BTreeMap::<String, Vec<String>>::new();
    for row in sqlx::query(
        r#"
        SELECT notes.uid AS note_uid, note_tags.tag
        FROM note_tags
        JOIN notes ON notes.id = note_tags.note_id
        ORDER BY notes.uid ASC, note_tags.tag ASC
        "#,
    )
    .fetch_all(&mut *transaction)
    .await?
    {
        let note_uid: String = row.try_get("note_uid")?;
        let tag: String = row.try_get("tag")?;
        tags_by_note.entry(note_uid).or_default().push(tag);
    }

    let mut notes = Vec::new();
    for row in sqlx::query(
        r#"
        SELECT
            notes.uid,
            workspaces.uid AS workspace_uid,
            users.uid AS creator_uid,
            notes.content,
            notes.status,
            notes.pinned,
            notes.created_at,
            notes.updated_at
        FROM notes
        JOIN workspaces ON workspaces.id = notes.workspace_id
        JOIN users ON users.id = notes.creator_id
        ORDER BY notes.created_at ASC, notes.id ASC
        "#,
    )
    .fetch_all(&mut *transaction)
    .await?
    {
        let uid: String = row.try_get("uid")?;
        notes.push(ExportNote {
            tags: tags_by_note.remove(&uid).unwrap_or_default(),
            uid,
            workspace_uid: row.try_get("workspace_uid")?,
            creator_uid: row.try_get("creator_uid")?,
            content: row.try_get("content")?,
            status: row.try_get("status")?,
            pinned: row.try_get::<i64, _>("pinned")? != 0,
            created_at_unix_ms: row.try_get("created_at")?,
            updated_at_unix_ms: row.try_get("updated_at")?,
        });
    }

    let mut tasks = Vec::new();
    for row in sqlx::query(
        r#"
        SELECT
            tasks.uid,
            workspaces.uid AS workspace_uid,
            users.uid AS creator_uid,
            tasks.title,
            tasks.description,
            tasks.status,
            tasks.priority,
            tasks.due_date,
            tasks.due_time,
            tasks.sort_key,
            tasks.completed_at,
            tasks.created_at,
            tasks.updated_at
        FROM tasks
        JOIN workspaces ON workspaces.id = tasks.workspace_id
        JOIN users ON users.id = tasks.creator_id
        ORDER BY tasks.created_at ASC, tasks.id ASC
        "#,
    )
    .fetch_all(&mut *transaction)
    .await?
    {
        let priority: i64 = row.try_get("priority")?;
        let priority =
            u8::try_from(priority).map_err(|_| DataManagementError::InvalidDatabaseValue {
                field: "tasks.priority",
                reason: format!("{priority} cannot be represented as an unsigned byte"),
            })?;

        tasks.push(ExportTask {
            uid: row.try_get("uid")?,
            workspace_uid: row.try_get("workspace_uid")?,
            creator_uid: row.try_get("creator_uid")?,
            title: row.try_get("title")?,
            description: row.try_get("description")?,
            status: row.try_get("status")?,
            priority,
            due_date: row.try_get("due_date")?,
            due_time: row.try_get("due_time")?,
            sort_key: row.try_get("sort_key")?,
            completed_at_unix_ms: row.try_get("completed_at")?,
            created_at_unix_ms: row.try_get("created_at")?,
            updated_at_unix_ms: row.try_get("updated_at")?,
        });
    }

    transaction.commit().await?;

    Ok(PortableExport {
        format_version: EXPORT_FORMAT_VERSION,
        application_version: env!("CARGO_PKG_VERSION").to_owned(),
        schema_version,
        exported_at_unix_ms,
        users,
        workspaces,
        notes,
        tasks,
    })
}

/// Writes a portable JSON export through a same-directory temporary file and atomic rename.
pub async fn export_json(
    pool: &SqlitePool,
    allowed_directory: &Path,
    destination: &Path,
) -> DataManagementResult<DataArtifact> {
    let destination = validate_destination(allowed_directory, destination)?;
    let export = collect_portable_export(pool).await?;
    let mut bytes = serde_json::to_vec_pretty(&export)?;
    bytes.push(b'\n');
    write_atomic(destination, bytes, "json").await
}

/// Writes a human-readable Markdown export while preserving note Markdown verbatim.
pub async fn export_markdown(
    pool: &SqlitePool,
    allowed_directory: &Path,
    destination: &Path,
) -> DataManagementResult<DataArtifact> {
    let destination = validate_destination(allowed_directory, destination)?;
    let export = collect_portable_export(pool).await?;
    let markdown = render_markdown(&export);
    write_atomic(destination, markdown.into_bytes(), "markdown").await
}

/// Restores a validated SQLite snapshot to a database path that does not exist.
///
/// The caller must invoke this before opening a pool for `target_database`. Existing database,
/// WAL, shared-memory, or rollback-journal files are never replaced.
pub async fn restore_sqlite_backup(
    backup_path: &Path,
    allowed_target_directory: &Path,
    target_database: &Path,
) -> DataManagementResult<DataArtifact> {
    let target_database = validate_destination(allowed_target_directory, target_database)?;
    ensure_sqlite_companions_absent(&target_database)?;
    let target_parent = target_database
        .parent()
        .expect("validated destinations always include a parent");
    cleanup_stale_temporary_files(target_parent)?;
    let (temporary_path, temporary_file) =
        create_private_temporary_file(&target_database, "restore")?;
    let mut temporary_guard = TemporaryPathGuard::new(temporary_path.clone());

    let backup_path = backup_path.to_owned();
    let copy_target = temporary_path.clone();
    let byte_len = run_blocking(move || {
        copy_backup_to_private_file(&backup_path, &copy_target, temporary_file)
    })
    .await?;

    validate_sqlite_snapshot(&temporary_path).await?;
    ensure_sqlite_companions_absent(&target_database)?;
    publish_temporary_file(&mut temporary_guard, &target_database)?;

    Ok(DataArtifact {
        path: target_database,
        byte_len,
    })
}

/// Retains valid managed backups for seven UTC days and four older UTC weeks.
///
/// Only files created by the default `backup-*` and `pre-migration-*` naming schemes are
/// considered. Custom names, symbolic links, and snapshots that fail validation are left
/// untouched. `protected_path` is never removed, even if its timestamp sorts behind future-dated
/// files after a system clock rollback.
pub async fn prune_managed_backups(
    allowed_directory: &Path,
    protected_path: Option<&Path>,
) -> DataManagementResult<Vec<PathBuf>> {
    if !allowed_directory.is_absolute() {
        return Err(DataManagementError::InvalidPath {
            path: allowed_directory.to_owned(),
            reason: "backup directory must be absolute".to_owned(),
        });
    }
    let directory = canonicalize(allowed_directory, "canonicalize backup directory")?;
    let protected_path = protected_path
        .map(|path| normalize_protected_backup_path(&directory, path))
        .transpose()?;
    let mut backups = Vec::new();
    for entry in fs::read_dir(&directory).map_err(|source| DataManagementError::Io {
        operation: "read backup directory",
        path: directory.clone(),
        source,
    })? {
        let entry = entry.map_err(|source| DataManagementError::Io {
            operation: "read backup directory entry",
            path: directory.clone(),
            source,
        })?;
        let metadata = entry
            .file_type()
            .map_err(|source| DataManagementError::Io {
                operation: "read backup file type",
                path: entry.path(),
                source,
            })?;
        if !metadata.is_file() || metadata.is_symlink() {
            continue;
        }
        let Some((kind, timestamp)) = managed_backup_identity(&entry.file_name()) else {
            continue;
        };
        let Some(date_time) = chrono::DateTime::<Utc>::from_timestamp_millis(timestamp) else {
            continue;
        };
        let path = entry.path();
        if protected_path.as_ref() != Some(&path) && validate_sqlite_snapshot(&path).await.is_err()
        {
            continue;
        }
        backups.push((kind, timestamp, date_time.date_naive(), path));
    }
    backups.sort_by_key(|backup| std::cmp::Reverse(backup.1));

    let mut retained = protected_path.into_iter().collect::<HashSet<_>>();
    for kind in [ManagedBackupKind::Manual, ManagedBackupKind::PreMigration] {
        let mut daily_dates = HashSet::new();
        for (candidate_kind, _, date, path) in &backups {
            if *candidate_kind != kind {
                continue;
            }
            if daily_dates.len() == 7 {
                break;
            }
            if daily_dates.insert(*date) {
                retained.insert(path.clone());
            }
        }

        let mut weekly_dates = HashSet::new();
        for (candidate_kind, _, date, path) in &backups {
            if *candidate_kind != kind || daily_dates.contains(date) || weekly_dates.len() == 4 {
                continue;
            }
            let week = (date.iso_week().year(), date.iso_week().week());
            if weekly_dates.insert(week) {
                retained.insert(path.clone());
            }
        }
    }

    let mut removed = Vec::new();
    for (_, _, _, path) in backups {
        if retained.contains(&path) {
            continue;
        }
        fs::remove_file(&path).map_err(|source| DataManagementError::Io {
            operation: "remove expired managed backup",
            path: path.clone(),
            source,
        })?;
        removed.push(path);
    }
    Ok(removed)
}

fn normalize_protected_backup_path(
    allowed_directory: &Path,
    protected_path: &Path,
) -> DataManagementResult<PathBuf> {
    if !protected_path.is_absolute() {
        return Err(DataManagementError::InvalidPath {
            path: protected_path.to_owned(),
            reason: "protected backup path must be absolute".to_owned(),
        });
    }
    let metadata =
        fs::symlink_metadata(protected_path).map_err(|source| DataManagementError::Io {
            operation: "read protected backup metadata",
            path: protected_path.to_owned(),
            source,
        })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(DataManagementError::InvalidPath {
            path: protected_path.to_owned(),
            reason: "protected backup must be a regular file".to_owned(),
        });
    }
    let parent = protected_path
        .parent()
        .ok_or_else(|| DataManagementError::InvalidPath {
            path: protected_path.to_owned(),
            reason: "protected backup must include a parent directory".to_owned(),
        })?;
    let parent = canonicalize(parent, "canonicalize protected backup parent")?;
    if parent != allowed_directory {
        return Err(DataManagementError::InvalidPath {
            path: protected_path.to_owned(),
            reason: format!(
                "protected backup must be directly beneath {}",
                allowed_directory.display()
            ),
        });
    }
    let file_name = protected_path
        .file_name()
        .ok_or_else(|| DataManagementError::InvalidPath {
            path: protected_path.to_owned(),
            reason: "protected backup must include a file name".to_owned(),
        })?;
    Ok(parent.join(file_name))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ManagedBackupKind {
    Manual,
    PreMigration,
}

fn managed_backup_identity(file_name: &std::ffi::OsStr) -> Option<(ManagedBackupKind, i64)> {
    let value = file_name.to_str()?;
    let suffix = value.strip_suffix(".sqlite3")?;
    let (kind, remainder) = if let Some(value) = suffix.strip_prefix("backup-") {
        (ManagedBackupKind::Manual, value)
    } else {
        (
            ManagedBackupKind::PreMigration,
            suffix.strip_prefix("pre-migration-")?,
        )
    };
    let (timestamp, schema_version) = remainder.split_once("-schema-")?;
    let timestamp = parse_canonical_positive_integer(timestamp)?;
    parse_canonical_positive_integer(schema_version)?;
    Some((kind, timestamp))
}

fn parse_canonical_positive_integer(value: &str) -> Option<i64> {
    if value.is_empty()
        || value.starts_with('0')
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let parsed = value.parse::<i64>().ok()?;
    (parsed > 0 && parsed.to_string() == value).then_some(parsed)
}

pub fn render_markdown(export: &PortableExport) -> String {
    let mut output = String::new();
    writeln!(&mut output, "# Locus Desk Export").expect("writing to a String should not fail");
    writeln!(&mut output).expect("writing to a String should not fail");
    writeln!(&mut output, "- Format version: {}", export.format_version)
        .expect("writing to a String should not fail");
    writeln!(
        &mut output,
        "- Application version: {}",
        markdown_inline(&export.application_version)
    )
    .expect("writing to a String should not fail");
    writeln!(&mut output, "- Schema version: {}", export.schema_version)
        .expect("writing to a String should not fail");
    writeln!(
        &mut output,
        "- Exported at (Unix ms): {}",
        export.exported_at_unix_ms
    )
    .expect("writing to a String should not fail");

    writeln!(&mut output, "\n## Users").expect("writing to a String should not fail");
    if export.users.is_empty() {
        writeln!(&mut output, "\n_No users._").expect("writing to a String should not fail");
    }
    for user in &export.users {
        writeln!(
            &mut output,
            "\n- **{}** (`{}`), created at {}",
            markdown_inline(&user.username),
            markdown_code(&user.uid),
            user.created_at_unix_ms
        )
        .expect("writing to a String should not fail");
    }

    writeln!(&mut output, "\n## Workspaces").expect("writing to a String should not fail");
    if export.workspaces.is_empty() {
        writeln!(&mut output, "\n_No workspaces._").expect("writing to a String should not fail");
    }
    for workspace in &export.workspaces {
        writeln!(&mut output, "\n### {}", markdown_inline(&workspace.name))
            .expect("writing to a String should not fail");
        writeln!(&mut output, "\n- UID: `{}`", markdown_code(&workspace.uid))
            .expect("writing to a String should not fail");
        writeln!(
            &mut output,
            "- Timezone: {}",
            markdown_inline(&workspace.timezone)
        )
        .expect("writing to a String should not fail");
        writeln!(
            &mut output,
            "- Creator: `{}`",
            markdown_code(&workspace.creator_uid)
        )
        .expect("writing to a String should not fail");
    }

    writeln!(&mut output, "\n## Notes").expect("writing to a String should not fail");
    if export.notes.is_empty() {
        writeln!(&mut output, "\n_No notes._").expect("writing to a String should not fail");
    }
    for note in &export.notes {
        writeln!(&mut output, "\n### Note `{}`", markdown_code(&note.uid))
            .expect("writing to a String should not fail");
        writeln!(&mut output).expect("writing to a String should not fail");
        writeln!(
            &mut output,
            "- Workspace: `{}`",
            markdown_code(&note.workspace_uid)
        )
        .expect("writing to a String should not fail");
        writeln!(
            &mut output,
            "- Creator: `{}`",
            markdown_code(&note.creator_uid)
        )
        .expect("writing to a String should not fail");
        writeln!(&mut output, "- Status: {}", markdown_inline(&note.status))
            .expect("writing to a String should not fail");
        writeln!(&mut output, "- Pinned: {}", note.pinned)
            .expect("writing to a String should not fail");
        writeln!(
            &mut output,
            "- Created at (Unix ms): {}",
            note.created_at_unix_ms
        )
        .expect("writing to a String should not fail");
        if !note.tags.is_empty() {
            let tags = note
                .tags
                .iter()
                .map(|tag| format!("`{}`", markdown_code(tag)))
                .collect::<Vec<_>>()
                .join(", ");
            writeln!(&mut output, "- Tags: {tags}").expect("writing to a String should not fail");
        }
        writeln!(&mut output, "\n{}\n", note.content).expect("writing to a String should not fail");
        writeln!(&mut output, "---").expect("writing to a String should not fail");
    }

    writeln!(&mut output, "\n## Tasks").expect("writing to a String should not fail");
    if export.tasks.is_empty() {
        writeln!(&mut output, "\n_No tasks._").expect("writing to a String should not fail");
    }
    for task in &export.tasks {
        let checked = if task.status == "DONE" { "x" } else { " " };
        writeln!(
            &mut output,
            "\n- [{checked}] **{}** (`{}`)",
            markdown_inline(&task.title),
            markdown_code(&task.uid)
        )
        .expect("writing to a String should not fail");
        writeln!(
            &mut output,
            "  - Workspace: `{}`",
            markdown_code(&task.workspace_uid)
        )
        .expect("writing to a String should not fail");
        writeln!(&mut output, "  - Priority: {}", task.priority)
            .expect("writing to a String should not fail");
        if let Some(due_date) = &task.due_date {
            let due_time = task
                .due_time
                .as_deref()
                .map(|time| format!(" {}", markdown_inline(time)))
                .unwrap_or_default();
            writeln!(
                &mut output,
                "  - Due: {}{due_time}",
                markdown_inline(due_date)
            )
            .expect("writing to a String should not fail");
        }
        if !task.description.is_empty() {
            writeln!(&mut output, "\n{}\n", task.description)
                .expect("writing to a String should not fail");
        }
    }

    output
}

async fn read_schema_version(
    transaction: &mut Transaction<'_, Sqlite>,
) -> DataManagementResult<i64> {
    let migration_table_exists: i64 = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1
            FROM sqlite_schema
            WHERE type = 'table' AND name = '_sqlx_migrations'
        )
        "#,
    )
    .fetch_one(&mut **transaction)
    .await?;

    if migration_table_exists == 0 {
        return Ok(0);
    }

    let version: Option<i64> =
        sqlx::query_scalar("SELECT MAX(version) FROM _sqlx_migrations WHERE success = TRUE")
            .fetch_one(&mut **transaction)
            .await?;

    Ok(version.unwrap_or(0))
}

async fn ensure_export_foreign_keys(
    transaction: &mut Transaction<'_, Sqlite>,
) -> DataManagementResult<()> {
    let violation = sqlx::query("PRAGMA foreign_key_check")
        .fetch_optional(&mut **transaction)
        .await?;
    if violation.is_some() {
        return Err(DataManagementError::InvalidDatabaseValue {
            field: "foreign keys",
            reason: "foreign key validation found inconsistent rows".to_owned(),
        });
    }
    Ok(())
}

async fn validate_sqlite_snapshot(path: &Path) -> DataManagementResult<()> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .read_only(true)
        .create_if_missing(false);
    let mut connection = SqliteConnection::connect_with(&options)
        .await
        .map_err(|error| DataManagementError::InvalidBackup {
            path: path.to_owned(),
            reason: error.to_string(),
        })?;

    let quick_check = sqlx::query_scalar::<_, String>("PRAGMA quick_check")
        .fetch_all(&mut connection)
        .await
        .map_err(|error| DataManagementError::InvalidBackup {
            path: path.to_owned(),
            reason: format!("quick_check failed: {error}"),
        })?;
    if quick_check.len() != 1 || quick_check[0] != "ok" {
        return Err(DataManagementError::InvalidBackup {
            path: path.to_owned(),
            reason: format!("quick_check returned {}", quick_check.join("; ")),
        });
    }

    let schema_version: Option<i64> =
        sqlx::query_scalar("SELECT MAX(version) FROM _sqlx_migrations WHERE success = TRUE")
            .fetch_one(&mut connection)
            .await
            .map_err(|error| DataManagementError::InvalidBackup {
                path: path.to_owned(),
                reason: format!("schema version validation failed: {error}"),
            })?;
    let schema_version = schema_version.unwrap_or(0);
    if schema_version <= 0 || schema_version > crate::db::latest_schema_version() {
        return Err(DataManagementError::InvalidBackup {
            path: path.to_owned(),
            reason: format!("unsupported schema version {schema_version}"),
        });
    }
    let required_tables = match schema_version {
        1 => REQUIRED_SCHEMA_V1_TABLES,
        _ => {
            return Err(DataManagementError::InvalidBackup {
                path: path.to_owned(),
                reason: format!("no validation rules exist for schema version {schema_version}"),
            });
        }
    };
    for table in required_tables {
        let exists: i64 = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = ?1)",
        )
        .bind(table)
        .fetch_one(&mut connection)
        .await
        .map_err(|error| DataManagementError::InvalidBackup {
            path: path.to_owned(),
            reason: format!("schema validation failed: {error}"),
        })?;
        if exists == 0 {
            return Err(DataManagementError::InvalidBackup {
                path: path.to_owned(),
                reason: format!("required schema {schema_version} table {table} is missing"),
            });
        }
    }
    validate_migration_history(&mut connection, path, schema_version).await?;
    validate_schema_shape(&mut connection, path, schema_version).await?;
    validate_backup_metadata_schema(&mut connection, path).await?;

    let metadata = sqlx::query(
        r#"
        SELECT created_at, application_version, schema_version, git_commit
        FROM locus_backup_metadata
        WHERE id = 1
        "#,
    )
    .fetch_optional(&mut connection)
    .await
    .map_err(|error| DataManagementError::InvalidBackup {
        path: path.to_owned(),
        reason: format!("backup metadata validation failed: {error}"),
    })?
    .ok_or_else(|| DataManagementError::InvalidBackup {
        path: path.to_owned(),
        reason: "backup metadata is missing".to_owned(),
    })?;
    let metadata_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM locus_backup_metadata")
        .fetch_one(&mut connection)
        .await
        .map_err(|error| DataManagementError::InvalidBackup {
            path: path.to_owned(),
            reason: format!("backup metadata validation failed: {error}"),
        })?;
    let metadata_created_at: i64 =
        metadata
            .try_get("created_at")
            .map_err(|error| DataManagementError::InvalidBackup {
                path: path.to_owned(),
                reason: format!("backup metadata is invalid: {error}"),
            })?;
    let metadata_application_version: String =
        metadata.try_get("application_version").map_err(|error| {
            DataManagementError::InvalidBackup {
                path: path.to_owned(),
                reason: format!("backup metadata is invalid: {error}"),
            }
        })?;
    let metadata_schema_version: i64 =
        metadata
            .try_get("schema_version")
            .map_err(|error| DataManagementError::InvalidBackup {
                path: path.to_owned(),
                reason: format!("backup metadata is invalid: {error}"),
            })?;
    let metadata_git_commit: String =
        metadata
            .try_get("git_commit")
            .map_err(|error| DataManagementError::InvalidBackup {
                path: path.to_owned(),
                reason: format!("backup metadata is invalid: {error}"),
            })?;
    if metadata_count != 1
        || metadata_created_at <= 0
        || metadata_application_version.is_empty()
        || metadata_git_commit.is_empty()
        || metadata_schema_version != schema_version
    {
        return Err(DataManagementError::InvalidBackup {
            path: path.to_owned(),
            reason: "backup metadata does not match the snapshot schema".to_owned(),
        });
    }

    let foreign_key_violations = sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&mut connection)
        .await
        .map_err(|error| DataManagementError::InvalidBackup {
            path: path.to_owned(),
            reason: format!("foreign key validation failed: {error}"),
        })?;
    if !foreign_key_violations.is_empty() {
        return Err(DataManagementError::InvalidBackup {
            path: path.to_owned(),
            reason: "foreign key validation found inconsistent rows".to_owned(),
        });
    }

    connection.close().await?;
    Ok(())
}

async fn write_backup_metadata(path: &Path) -> DataManagementResult<()> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(false);
    let mut connection = SqliteConnection::connect_with(&options).await?;
    let schema_version: Option<i64> =
        sqlx::query_scalar("SELECT MAX(version) FROM _sqlx_migrations WHERE success = TRUE")
            .fetch_one(&mut connection)
            .await?;
    let schema_version = schema_version.unwrap_or(0);
    sqlx::query(AssertSqlSafe(format!(
        "DROP TABLE IF EXISTS {BACKUP_METADATA_TABLE}"
    )))
    .execute(&mut connection)
    .await?;
    sqlx::query(BACKUP_METADATA_CREATE_SQL)
        .execute(&mut connection)
        .await?;
    sqlx::query(AssertSqlSafe(format!(
        r#"
        INSERT INTO {BACKUP_METADATA_TABLE}
            (id, created_at, application_version, schema_version, git_commit)
        VALUES (1, ?1, ?2, ?3, ?4)
        "#
    )))
    .bind(unix_millis_now()?)
    .bind(env!("CARGO_PKG_VERSION"))
    .bind(schema_version)
    .bind(crate::version::GIT_COMMIT)
    .execute(&mut connection)
    .await?;
    connection.close().await?;
    Ok(())
}

async fn validate_migration_history(
    connection: &mut SqliteConnection,
    path: &Path,
    schema_version: i64,
) -> DataManagementResult<()> {
    let rows = sqlx::query(
        r#"
        SELECT version, description, installed_on, success, checksum, execution_time
        FROM _sqlx_migrations
        ORDER BY version ASC
        "#,
    )
    .fetch_all(&mut *connection)
    .await
    .map_err(|error| DataManagementError::InvalidBackup {
        path: path.to_owned(),
        reason: format!("migration history validation failed: {error}"),
    })?;
    let expected = crate::db::embedded_migrations()
        .filter(|migration| migration.version <= schema_version)
        .collect::<Vec<_>>();
    if rows.len() != expected.len() {
        return Err(DataManagementError::InvalidBackup {
            path: path.to_owned(),
            reason: "migration history does not match the embedded migrations".to_owned(),
        });
    }

    for (row, migration) in rows.iter().zip(expected) {
        let version: i64 =
            row.try_get("version")
                .map_err(|error| DataManagementError::InvalidBackup {
                    path: path.to_owned(),
                    reason: format!("migration history is invalid: {error}"),
                })?;
        let description: String =
            row.try_get("description")
                .map_err(|error| DataManagementError::InvalidBackup {
                    path: path.to_owned(),
                    reason: format!("migration history is invalid: {error}"),
                })?;
        let success: bool =
            row.try_get("success")
                .map_err(|error| DataManagementError::InvalidBackup {
                    path: path.to_owned(),
                    reason: format!("migration history is invalid: {error}"),
                })?;
        let checksum: Vec<u8> =
            row.try_get("checksum")
                .map_err(|error| DataManagementError::InvalidBackup {
                    path: path.to_owned(),
                    reason: format!("migration history is invalid: {error}"),
                })?;
        if version != migration.version
            || description != migration.description
            || !success
            || checksum.as_slice() != migration.checksum.as_ref()
        {
            return Err(DataManagementError::InvalidBackup {
                path: path.to_owned(),
                reason: format!("migration {version} does not match the embedded migration"),
            });
        }
    }
    Ok(())
}

async fn validate_schema_shape(
    connection: &mut SqliteConnection,
    path: &Path,
    schema_version: i64,
) -> DataManagementResult<()> {
    let actual = read_schema_objects(connection).await.map_err(|error| {
        DataManagementError::InvalidBackup {
            path: path.to_owned(),
            reason: format!("schema shape validation failed: {error}"),
        }
    })?;

    let mut reference = SqliteConnection::connect("sqlite::memory:").await?;
    for migration in
        crate::db::embedded_migrations().filter(|migration| migration.version <= schema_version)
    {
        sqlx::raw_sql(migration.sql.as_ref())
            .execute(&mut reference)
            .await?;
    }
    let expected = read_schema_objects(&mut reference).await?;
    reference.close().await?;

    if actual != expected {
        return Err(DataManagementError::InvalidBackup {
            path: path.to_owned(),
            reason: format!(
                "schema objects do not match the embedded schema version {schema_version}"
            ),
        });
    }
    Ok(())
}

async fn validate_backup_metadata_schema(
    connection: &mut SqliteConnection,
    path: &Path,
) -> DataManagementResult<()> {
    let schema = sqlx::query("SELECT type AS object_type, sql FROM sqlite_schema WHERE name = ?1")
        .bind(BACKUP_METADATA_TABLE)
        .fetch_optional(&mut *connection)
        .await
        .map_err(|error| DataManagementError::InvalidBackup {
            path: path.to_owned(),
            reason: format!("backup metadata schema validation failed: {error}"),
        })?
        .ok_or_else(|| DataManagementError::InvalidBackup {
            path: path.to_owned(),
            reason: "backup metadata table is missing".to_owned(),
        })?;
    let object_type: String =
        schema
            .try_get("object_type")
            .map_err(|error| DataManagementError::InvalidBackup {
                path: path.to_owned(),
                reason: format!("backup metadata schema is invalid: {error}"),
            })?;
    let sql: String =
        schema
            .try_get("sql")
            .map_err(|error| DataManagementError::InvalidBackup {
                path: path.to_owned(),
                reason: format!("backup metadata schema is invalid: {error}"),
            })?;
    if object_type != "table"
        || normalize_schema_sql(&sql) != normalize_schema_sql(BACKUP_METADATA_CREATE_SQL)
    {
        return Err(DataManagementError::InvalidBackup {
            path: path.to_owned(),
            reason: "backup metadata object does not match the required table schema".to_owned(),
        });
    }
    Ok(())
}

async fn read_schema_objects(
    connection: &mut SqliteConnection,
) -> Result<BTreeMap<(String, String), String>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT type AS object_type, name, sql
        FROM sqlite_schema
        WHERE type IN ('table', 'index', 'view', 'trigger')
          AND name NOT LIKE 'sqlite_%'
          AND name NOT IN ('_sqlx_migrations', 'locus_backup_metadata')
          AND sql IS NOT NULL
        ORDER BY type ASC, name ASC
        "#,
    )
    .fetch_all(connection)
    .await?;
    rows.into_iter()
        .map(|row| {
            let object_type: String = row.try_get("object_type")?;
            let name: String = row.try_get("name")?;
            let sql: String = row.try_get("sql")?;
            let normalized = normalize_schema_sql(&sql);
            Ok(((object_type, name), normalized))
        })
        .collect()
}

fn normalize_schema_sql(sql: &str) -> String {
    sql.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn validate_destination(
    allowed_directory: &Path,
    destination: &Path,
) -> DataManagementResult<PathBuf> {
    if !allowed_directory.is_absolute() {
        return Err(DataManagementError::InvalidPath {
            path: allowed_directory.to_owned(),
            reason: "allowed directory must be absolute".to_owned(),
        });
    }
    if !destination.is_absolute() {
        return Err(DataManagementError::InvalidPath {
            path: destination.to_owned(),
            reason: "destination must be absolute".to_owned(),
        });
    }

    let allowed_directory = canonicalize(allowed_directory, "canonicalize allowed directory")?;
    if !metadata(&allowed_directory, "read allowed directory metadata")?.is_dir() {
        return Err(DataManagementError::InvalidPath {
            path: allowed_directory,
            reason: "allowed directory is not a directory".to_owned(),
        });
    }

    let file_name = destination
        .file_name()
        .ok_or_else(|| DataManagementError::InvalidPath {
            path: destination.to_owned(),
            reason: "destination must include a file name".to_owned(),
        })?;
    let parent = destination
        .parent()
        .ok_or_else(|| DataManagementError::InvalidPath {
            path: destination.to_owned(),
            reason: "destination must include a parent directory".to_owned(),
        })?;
    let parent = canonicalize(parent, "canonicalize destination parent")?;
    if !parent.starts_with(&allowed_directory) {
        return Err(DataManagementError::InvalidPath {
            path: destination.to_owned(),
            reason: format!(
                "destination must remain beneath {}",
                allowed_directory.display()
            ),
        });
    }

    let destination = parent.join(file_name);
    ensure_absent(&destination)?;
    Ok(destination)
}

fn ensure_source_companions_absent(path: &Path) -> DataManagementResult<()> {
    for suffix in ["-wal", "-shm", "-journal"] {
        let companion = path_with_suffix(path, suffix);
        match fs::symlink_metadata(&companion) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Ok(_) => {
                return Err(DataManagementError::InvalidBackup {
                    path: path.to_owned(),
                    reason: format!(
                        "backup has a companion file and may be a live database: {}",
                        companion.display()
                    ),
                });
            }
            Err(source) => {
                return Err(DataManagementError::Io {
                    operation: "inspect backup companion",
                    path: companion,
                    source,
                });
            }
        }
    }
    Ok(())
}

fn ensure_sqlite_companions_absent(path: &Path) -> DataManagementResult<()> {
    for suffix in ["-wal", "-shm", "-journal"] {
        ensure_absent(&path_with_suffix(path, suffix))?;
    }
    Ok(())
}

fn path_with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(suffix);
    PathBuf::from(value)
}

fn ensure_absent(path: &Path) -> DataManagementResult<()> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Ok(_) => Err(DataManagementError::DestinationExists(path.to_owned())),
        Err(source) => Err(DataManagementError::Io {
            operation: "inspect destination",
            path: path.to_owned(),
            source,
        }),
    }
}

fn canonicalize(path: &Path, operation: &'static str) -> DataManagementResult<PathBuf> {
    fs::canonicalize(path).map_err(|source| DataManagementError::Io {
        operation,
        path: path.to_owned(),
        source,
    })
}

fn metadata(path: &Path, operation: &'static str) -> DataManagementResult<fs::Metadata> {
    fs::metadata(path).map_err(|source| DataManagementError::Io {
        operation,
        path: path.to_owned(),
        source,
    })
}

fn file_len(path: &Path) -> DataManagementResult<u64> {
    Ok(metadata(path, "read file size")?.len())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceFingerprint {
    byte_len: u64,
    modified: Option<SystemTime>,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
}

impl SourceFingerprint {
    fn from_metadata(metadata: &fs::Metadata) -> Self {
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt;

        Self {
            byte_len: metadata.len(),
            modified: metadata.modified().ok(),
            #[cfg(unix)]
            device: metadata.dev(),
            #[cfg(unix)]
            inode: metadata.ino(),
        }
    }
}

fn open_backup_source(path: &Path) -> DataManagementResult<(File, SourceFingerprint)> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK);
    }

    let file = options.open(path).map_err(|source| {
        #[cfg(unix)]
        if source.raw_os_error() == Some(libc::ELOOP) {
            return DataManagementError::InvalidPath {
                path: path.to_owned(),
                reason: "backup source must not be a symbolic link".to_owned(),
            };
        }
        DataManagementError::Io {
            operation: "open backup source without following symbolic links",
            path: path.to_owned(),
            source,
        }
    })?;
    let metadata = file.metadata().map_err(|source| DataManagementError::Io {
        operation: "read opened backup metadata",
        path: path.to_owned(),
        source,
    })?;
    if !metadata.is_file() {
        return Err(DataManagementError::InvalidPath {
            path: path.to_owned(),
            reason: "backup source must be a regular file".to_owned(),
        });
    }
    ensure_source_path_matches_file(path, &metadata)?;
    ensure_source_companions_absent(path)?;
    let fingerprint = SourceFingerprint::from_metadata(&metadata);
    Ok((file, fingerprint))
}

fn ensure_source_path_matches_file(
    path: &Path,
    opened_metadata: &fs::Metadata,
) -> DataManagementResult<()> {
    let path_metadata = fs::symlink_metadata(path).map_err(|source| DataManagementError::Io {
        operation: "recheck backup source metadata",
        path: path.to_owned(),
        source,
    })?;
    if path_metadata.file_type().is_symlink() || !path_metadata.is_file() {
        return Err(DataManagementError::InvalidPath {
            path: path.to_owned(),
            reason: "backup source must remain the opened regular file".to_owned(),
        });
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if path_metadata.dev() != opened_metadata.dev()
            || path_metadata.ino() != opened_metadata.ino()
        {
            return Err(DataManagementError::InvalidPath {
                path: path.to_owned(),
                reason: "backup source changed while it was being opened".to_owned(),
            });
        }
    }
    Ok(())
}

fn unused_temporary_path(destination: &Path, purpose: &str) -> DataManagementResult<PathBuf> {
    let parent = destination
        .parent()
        .ok_or_else(|| DataManagementError::InvalidPath {
            path: destination.to_owned(),
            reason: "destination must include a parent directory".to_owned(),
        })?;
    if !TEMPORARY_PURPOSES.contains(&purpose) {
        return Err(DataManagementError::InvalidPath {
            path: destination.to_owned(),
            reason: "unknown temporary file purpose".to_owned(),
        });
    }

    for _ in 0..100 {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let candidate = parent.join(format!(
            "{TEMPORARY_FILE_PREFIX}{purpose}-{}-{sequence}",
            std::process::id()
        ));
        match fs::symlink_metadata(&candidate) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(candidate),
            Ok(_) => continue,
            Err(source) => {
                return Err(DataManagementError::Io {
                    operation: "inspect temporary path",
                    path: candidate,
                    source,
                });
            }
        }
    }

    Err(DataManagementError::InvalidPath {
        path: destination.to_owned(),
        reason: "could not allocate a unique temporary path".to_owned(),
    })
}

pub(crate) fn cleanup_stale_temporary_files(directory: &Path) -> DataManagementResult<()> {
    #[cfg(not(unix))]
    {
        let _ = directory;
        return Ok(());
    }

    #[cfg(unix)]
    {
        let mut removed_any = false;
        for entry in fs::read_dir(directory).map_err(|source| DataManagementError::Io {
            operation: "scan managed directory for stale temporary files",
            path: directory.to_owned(),
            source,
        })? {
            let entry = entry.map_err(|source| DataManagementError::Io {
                operation: "read managed directory entry",
                path: directory.to_owned(),
                source,
            })?;
            let Some(owner_pid) = temporary_file_owner_pid(&entry.file_name()) else {
                continue;
            };
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(source) => {
                    return Err(DataManagementError::Io {
                        operation: "read temporary file type",
                        path: entry.path(),
                        source,
                    });
                }
            };
            if file_type.is_symlink() || !file_type.is_file() {
                continue;
            }
            let metadata = match fs::symlink_metadata(entry.path()) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(source) => {
                    return Err(DataManagementError::Io {
                        operation: "read temporary file metadata",
                        path: entry.path(),
                        source,
                    });
                }
            };
            if !is_private_temporary_file(&metadata) || process_may_be_running(owner_pid) {
                continue;
            }
            if remove_same_temporary_file(&entry.path(), &metadata)? {
                removed_any = true;
            }
        }
        if removed_any {
            sync_directory(directory)?;
        }
        Ok(())
    }
}

pub(crate) fn is_reserved_temporary_file_name(file_name: &std::ffi::OsStr) -> bool {
    file_name
        .to_str()
        .is_some_and(|value| value.starts_with(TEMPORARY_FILE_PREFIX))
}

fn temporary_file_owner_pid(file_name: &std::ffi::OsStr) -> Option<u32> {
    let value = file_name.to_str()?;
    let value = ["-wal", "-shm", "-journal"]
        .iter()
        .find_map(|suffix| value.strip_suffix(suffix))
        .unwrap_or(value);
    let remainder = value.strip_prefix(TEMPORARY_FILE_PREFIX)?;
    let mut parts = remainder.split('-');
    let purpose = parts.next()?;
    if !TEMPORARY_PURPOSES.contains(&purpose) {
        return None;
    }
    let pid_text = parts.next()?;
    let sequence = parts.next()?;
    if parts.next().is_some() || !is_canonical_nonnegative_integer(sequence) {
        return None;
    }
    let pid = pid_text.parse::<u32>().ok()?;
    (pid > 0 && pid.to_string() == pid_text).then_some(pid)
}

fn is_canonical_nonnegative_integer(value: &str) -> bool {
    !value.is_empty()
        && (value == "0" || !value.starts_with('0'))
        && value.bytes().all(|byte| byte.is_ascii_digit())
        && value.parse::<u64>().is_ok()
}

#[cfg(unix)]
fn is_private_temporary_file(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;

    metadata.is_file()
        && metadata.uid() == unsafe { libc::geteuid() }
        && metadata.mode() & 0o777 == 0o600
}

#[cfg(unix)]
fn process_may_be_running(pid: u32) -> bool {
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return true;
    };
    let result = unsafe { libc::kill(pid, 0) };
    if result == 0 {
        return true;
    }
    !matches!(io::Error::last_os_error().raw_os_error(), Some(libc::ESRCH))
}

#[cfg(unix)]
fn remove_same_temporary_file(path: &Path, expected: &fs::Metadata) -> DataManagementResult<bool> {
    use std::os::unix::fs::MetadataExt;

    let current = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(source) => {
            return Err(DataManagementError::Io {
                operation: "recheck stale temporary file",
                path: path.to_owned(),
                source,
            });
        }
    };
    if current.file_type().is_symlink()
        || !is_private_temporary_file(&current)
        || current.dev() != expected.dev()
        || current.ino() != expected.ino()
    {
        return Ok(false);
    }
    fs::remove_file(path).map_err(|source| DataManagementError::Io {
        operation: "remove stale temporary file",
        path: path.to_owned(),
        source,
    })?;
    Ok(true)
}

async fn write_atomic(
    destination: PathBuf,
    bytes: Vec<u8>,
    purpose: &'static str,
) -> DataManagementResult<DataArtifact> {
    let destination_for_task = destination.clone();
    let byte_len =
        u64::try_from(bytes.len()).map_err(|_| DataManagementError::InvalidDatabaseValue {
            field: "export byte length",
            reason: "export is too large to report its size".to_owned(),
        })?;

    run_blocking(move || {
        cleanup_stale_temporary_files(
            destination_for_task
                .parent()
                .expect("validated destinations always include a parent"),
        )?;
        let (temporary_path, mut temporary_file) =
            create_private_temporary_file(&destination_for_task, purpose)?;
        let mut temporary_guard = TemporaryPathGuard::new(temporary_path.clone());
        temporary_file
            .write_all(&bytes)
            .map_err(|source| DataManagementError::Io {
                operation: "write temporary export",
                path: temporary_path.clone(),
                source,
            })?;
        temporary_file
            .flush()
            .map_err(|source| DataManagementError::Io {
                operation: "flush temporary export",
                path: temporary_path.clone(),
                source,
            })?;
        temporary_file
            .sync_all()
            .map_err(|source| DataManagementError::Io {
                operation: "sync temporary export",
                path: temporary_path,
                source,
            })?;
        drop(temporary_file);
        publish_temporary_file(&mut temporary_guard, &destination_for_task)
    })
    .await?;

    Ok(DataArtifact {
        path: destination,
        byte_len,
    })
}

fn create_private_temporary_file(
    destination: &Path,
    purpose: &str,
) -> DataManagementResult<(PathBuf, File)> {
    for _ in 0..100 {
        let temporary_path = unused_temporary_path(destination, purpose)?;
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }

        match options.open(&temporary_path) {
            Ok(file) => return Ok((temporary_path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(source) => {
                return Err(DataManagementError::Io {
                    operation: "create temporary file",
                    path: temporary_path,
                    source,
                });
            }
        }
    }

    Err(DataManagementError::InvalidPath {
        path: destination.to_owned(),
        reason: "could not create a unique temporary file".to_owned(),
    })
}

async fn sync_private_file(path: PathBuf) -> DataManagementResult<()> {
    run_blocking(move || {
        set_private_permissions(&path)?;
        let file = File::open(&path).map_err(|source| DataManagementError::Io {
            operation: "open generated backup",
            path: path.clone(),
            source,
        })?;
        file.sync_all().map_err(|source| DataManagementError::Io {
            operation: "sync generated backup",
            path,
            source,
        })
    })
    .await
}

fn copy_backup_to_private_file(
    source: &Path,
    destination: &Path,
    mut destination_file: File,
) -> DataManagementResult<u64> {
    let (mut source_file, initial_fingerprint) = open_backup_source(source)?;
    let byte_len = io::copy(&mut source_file, &mut destination_file).map_err(|error| {
        DataManagementError::Io {
            operation: "copy backup",
            path: destination.to_owned(),
            source: error,
        }
    })?;
    destination_file
        .flush()
        .map_err(|error| DataManagementError::Io {
            operation: "flush temporary restore file",
            path: destination.to_owned(),
            source: error,
        })?;
    destination_file
        .sync_all()
        .map_err(|error| DataManagementError::Io {
            operation: "sync temporary restore file",
            path: destination.to_owned(),
            source: error,
        })?;
    let final_metadata =
        source_file
            .metadata()
            .map_err(|source_error| DataManagementError::Io {
                operation: "recheck opened backup metadata",
                path: source.to_owned(),
                source: source_error,
            })?;
    ensure_source_path_matches_file(source, &final_metadata)?;
    ensure_source_companions_absent(source)?;
    if SourceFingerprint::from_metadata(&final_metadata) != initial_fingerprint
        || byte_len != initial_fingerprint.byte_len
    {
        return Err(DataManagementError::InvalidBackup {
            path: source.to_owned(),
            reason: "backup source changed while it was being copied".to_owned(),
        });
    }
    Ok(byte_len)
}

fn set_private_permissions(path: &Path) -> DataManagementResult<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|source| {
            DataManagementError::Io {
                operation: "set private file permissions",
                path: path.to_owned(),
                source,
            }
        })?;
    }
    Ok(())
}

fn publish_temporary_file(
    temporary_guard: &mut TemporaryPathGuard,
    destination: &Path,
) -> DataManagementResult<()> {
    remove_temporary_companions(temporary_guard.path())?;
    match fs::hard_link(temporary_guard.path(), destination) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            return Err(DataManagementError::DestinationExists(
                destination.to_owned(),
            ));
        }
        Err(source) => {
            return Err(DataManagementError::Io {
                operation: "atomically publish file without replacement",
                path: destination.to_owned(),
                source,
            });
        }
    }
    fs::remove_file(temporary_guard.path()).map_err(|source| DataManagementError::Io {
        operation: "remove published temporary link",
        path: temporary_guard.path().to_owned(),
        source,
    })?;
    temporary_guard.disarm();
    sync_parent_directory(destination)
}

fn remove_temporary_companions(path: &Path) -> DataManagementResult<()> {
    for suffix in ["-wal", "-shm", "-journal"] {
        let companion = path_with_suffix(path, suffix);
        match fs::remove_file(&companion) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(DataManagementError::Io {
                    operation: "remove temporary SQLite companion",
                    path: companion,
                    source,
                });
            }
        }
    }
    Ok(())
}

fn sync_parent_directory(path: &Path) -> DataManagementResult<()> {
    #[cfg(unix)]
    {
        let parent = path
            .parent()
            .ok_or_else(|| DataManagementError::InvalidPath {
                path: path.to_owned(),
                reason: "published file must include a parent directory".to_owned(),
            })?;
        sync_directory(parent)?;
    }
    Ok(())
}

fn sync_directory(path: &Path) -> DataManagementResult<()> {
    #[cfg(unix)]
    {
        let directory = File::open(path).map_err(|source| DataManagementError::Io {
            operation: "open managed directory",
            path: path.to_owned(),
            source,
        })?;
        directory
            .sync_all()
            .map_err(|source| DataManagementError::Io {
                operation: "sync managed directory",
                path: path.to_owned(),
                source,
            })?;
    }
    Ok(())
}

async fn run_blocking<T, F>(operation: F) -> DataManagementResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> DataManagementResult<T> + Send + 'static,
{
    tokio::task::spawn_blocking(operation).await?
}

fn unix_millis_now() -> DataManagementResult<i64> {
    let duration = SystemTime::now().duration_since(UNIX_EPOCH)?;
    i64::try_from(duration.as_millis()).map_err(|_| DataManagementError::InvalidDatabaseValue {
        field: "exported_at_unix_ms",
        reason: "current time does not fit in a signed 64-bit integer".to_owned(),
    })
}

fn markdown_inline(value: &str) -> String {
    value
        .replace(['\r', '\n'], " ")
        .chars()
        .fold(String::new(), |mut escaped, character| {
            if matches!(
                character,
                '\\' | '`' | '*' | '_' | '{' | '}' | '[' | ']' | '<' | '>' | '#' | '|' | '!'
            ) {
                escaped.push('\\');
            }
            escaped.push(character);
            escaped
        })
}

fn markdown_code(value: &str) -> String {
    value.replace('`', "\\`").replace(['\r', '\n'], " ")
}

struct TemporaryPathGuard {
    path: PathBuf,
    armed: bool,
}

impl TemporaryPathGuard {
    fn new(path: PathBuf) -> Self {
        Self { path, armed: true }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for TemporaryPathGuard {
    fn drop(&mut self) {
        if self.armed {
            let _ = fs::remove_file(&self.path);
            for suffix in ["-wal", "-shm", "-journal"] {
                let _ = fs::remove_file(path_with_suffix(&self.path, suffix));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::Value;
    use sqlx::{
        Row, SqlitePool,
        sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    };
    use tempfile::TempDir;

    use super::{
        DataManagementError, collect_portable_export, create_sqlite_backup, export_json,
        export_markdown, prune_managed_backups, restore_sqlite_backup,
    };

    #[tokio::test]
    async fn backup_is_consistent_and_never_overwrites_a_destination() {
        let fixture = Fixture::new().await;
        let backup = fixture.root.path().join("backups/locus-desk.sqlite3");

        let artifact = create_sqlite_backup(&fixture.pool, &fixture.backups, &backup)
            .await
            .expect("backup should succeed");
        assert!(artifact.byte_len > 0);

        sqlx::query("UPDATE notes SET content = 'changed after backup'")
            .execute(&fixture.pool)
            .await
            .expect("source update should succeed");
        let backup_pool = open_existing_database(&backup).await;
        let content: String = sqlx::query_scalar("SELECT content FROM notes")
            .fetch_one(&backup_pool)
            .await
            .expect("backup note should be readable");
        assert_eq!(content, "# Original note\n\n你好, world.");
        let metadata = sqlx::query(
            "SELECT created_at, application_version, schema_version, git_commit FROM locus_backup_metadata WHERE id = 1",
        )
        .fetch_one(&backup_pool)
        .await
        .expect("backup metadata should be readable");
        assert!(metadata.get::<i64, _>("created_at") > 0);
        assert_eq!(
            metadata.get::<String, _>("application_version"),
            env!("CARGO_PKG_VERSION")
        );
        assert_eq!(metadata.get::<i64, _>("schema_version"), 1);
        assert!(!metadata.get::<String, _>("git_commit").is_empty());

        let error = create_sqlite_backup(&fixture.pool, &fixture.backups, &backup)
            .await
            .expect_err("an existing backup must not be overwritten");
        assert!(matches!(error, DataManagementError::DestinationExists(_)));
    }

    #[tokio::test]
    async fn exports_are_readable_and_exclude_secrets_and_internal_ids() {
        let fixture = Fixture::new().await;
        let json_path = fixture.root.path().join("exports/locus-desk.json");
        let markdown_path = fixture.root.path().join("exports/locus-desk.md");

        export_json(&fixture.pool, &fixture.exports, &json_path)
            .await
            .expect("JSON export should succeed");
        export_markdown(&fixture.pool, &fixture.exports, &markdown_path)
            .await
            .expect("Markdown export should succeed");

        let json = fs::read_to_string(json_path).expect("JSON export should be readable");
        assert!(!json.contains("secret-password-hash"));
        assert!(!json.contains("secret-session-hash"));
        assert!(!json.contains("9911223399"));
        assert!(!json.contains("passwordHash"));
        assert!(!json.contains("tokenHash"));
        let payload: Value = serde_json::from_str(&json).expect("JSON export should be valid");
        assert_eq!(payload["schemaVersion"], 1);
        assert_eq!(payload["notes"][0]["tags"][0], "中文");
        assert_eq!(payload["tasks"][0]["title"], "Ship the safe export");

        let markdown =
            fs::read_to_string(markdown_path).expect("Markdown export should be readable");
        assert!(markdown.contains("# Original note"));
        assert!(markdown.contains("你好, world."));
        assert!(markdown.contains("Ship the safe export"));
        assert!(!markdown.contains("secret-password-hash"));
        assert!(!markdown.contains("secret-session-hash"));
    }

    #[tokio::test]
    async fn export_rejects_orphaned_rows_in_its_snapshot() {
        let fixture = Fixture::new().await;
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(&fixture.pool)
            .await
            .expect("foreign keys should be disabled for the corrupt fixture");
        sqlx::query("INSERT INTO note_tags (note_id, tag) VALUES (999999, 'orphan')")
            .execute(&fixture.pool)
            .await
            .expect("orphaned fixture row should be inserted");

        let error = collect_portable_export(&fixture.pool)
            .await
            .expect_err("an export must reject rows that joins would omit");

        assert!(matches!(
            error,
            DataManagementError::InvalidDatabaseValue {
                field: "foreign keys",
                ..
            }
        ));
    }

    #[tokio::test]
    async fn restore_validates_the_snapshot_scope_and_empty_target() {
        let fixture = Fixture::new().await;
        let backup = fixture.root.path().join("backups/locus-desk.sqlite3");
        create_sqlite_backup(&fixture.pool, &fixture.backups, &backup)
            .await
            .expect("backup should succeed");

        let restore_directory = fixture.root.path().join("restored");
        fs::create_dir(&restore_directory).expect("restore directory should be created");
        let target = restore_directory.join("db/locus-desk.sqlite3");
        fs::create_dir(target.parent().expect("target should have a parent"))
            .expect("database directory should be created");

        let artifact = restore_sqlite_backup(&backup, &restore_directory, &target)
            .await
            .expect("restore should succeed");
        assert!(artifact.byte_len > 0);
        let restored_pool = open_existing_database(&target).await;
        let row = sqlx::query("SELECT uid, content FROM notes")
            .fetch_one(&restored_pool)
            .await
            .expect("restored note should be readable");
        assert_eq!(row.get::<String, _>("uid"), "note_01");
        assert_eq!(
            row.get::<String, _>("content"),
            "# Original note\n\n你好, world."
        );
        restored_pool.close().await;

        let restored_pool = open_writable_database(&target).await;
        let second_backup = fixture.root.path().join("backups/restored.sqlite3");
        create_sqlite_backup(&restored_pool, &fixture.backups, &second_backup)
            .await
            .expect("a restored database should be backed up again");
        restored_pool.close().await;

        let error = restore_sqlite_backup(&backup, &restore_directory, &target)
            .await
            .expect_err("restore must not replace an existing database");
        assert!(matches!(error, DataManagementError::DestinationExists(_)));

        let outside_target = fixture.root.path().join("outside.sqlite3");
        let error = restore_sqlite_backup(&backup, &restore_directory, &outside_target)
            .await
            .expect_err("restore must remain beneath the allowed directory");
        assert!(matches!(error, DataManagementError::InvalidPath { .. }));

        let invalid_history = fixture.root.path().join("backups/invalid-history.sqlite3");
        fs::copy(&backup, &invalid_history).expect("invalid fixture should be copied");
        let invalid_pool = open_writable_database(&invalid_history).await;
        sqlx::query("UPDATE _sqlx_migrations SET checksum = zeroblob(length(checksum))")
            .execute(&invalid_pool)
            .await
            .expect("migration checksum should be changed");
        invalid_pool.close().await;
        let invalid_target = fixture.root.path().join("invalid-history-target");
        fs::create_dir(&invalid_target).expect("invalid target should be created");
        let invalid_database = invalid_target.join("db/locus-desk.sqlite3");
        fs::create_dir(invalid_database.parent().unwrap()).unwrap();
        let error = restore_sqlite_backup(&invalid_history, &invalid_target, &invalid_database)
            .await
            .expect_err("a mismatched migration checksum must be rejected");
        assert!(matches!(error, DataManagementError::InvalidBackup { .. }));

        let invalid_schema = fixture.root.path().join("backups/invalid-schema.sqlite3");
        fs::copy(&backup, &invalid_schema).expect("invalid fixture should be copied");
        let invalid_pool = open_writable_database(&invalid_schema).await;
        sqlx::query("DROP INDEX idx_tasks_today")
            .execute(&invalid_pool)
            .await
            .expect("schema shape should be changed");
        invalid_pool.close().await;
        let invalid_target = fixture.root.path().join("invalid-schema-target");
        fs::create_dir(&invalid_target).expect("invalid target should be created");
        let invalid_database = invalid_target.join("db/locus-desk.sqlite3");
        fs::create_dir(invalid_database.parent().unwrap()).unwrap();
        let error = restore_sqlite_backup(&invalid_schema, &invalid_target, &invalid_database)
            .await
            .expect_err("a mismatched schema shape must be rejected");
        assert!(matches!(error, DataManagementError::InvalidBackup { .. }));

        let metadata_view = fixture.root.path().join("backups/metadata-view.sqlite3");
        fs::copy(&backup, &metadata_view).expect("invalid fixture should be copied");
        let invalid_pool = open_writable_database(&metadata_view).await;
        sqlx::query("DROP TABLE locus_backup_metadata")
            .execute(&invalid_pool)
            .await
            .expect("metadata table should be removed");
        sqlx::query(
            r#"
            CREATE VIEW locus_backup_metadata AS
            SELECT
                1 AS id,
                1 AS created_at,
                '0.1.0' AS application_version,
                1 AS schema_version,
                'fixture' AS git_commit
            "#,
        )
        .execute(&invalid_pool)
        .await
        .expect("metadata view should be created");
        invalid_pool.close().await;
        let invalid_target = fixture.root.path().join("metadata-view-target");
        fs::create_dir(&invalid_target).expect("invalid target should be created");
        let invalid_database = invalid_target.join("db/locus-desk.sqlite3");
        fs::create_dir(invalid_database.parent().unwrap()).unwrap();
        let error = restore_sqlite_backup(&metadata_view, &invalid_target, &invalid_database)
            .await
            .expect_err("a view must not impersonate the backup metadata table");
        assert!(matches!(error, DataManagementError::InvalidBackup { .. }));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn restore_does_not_follow_a_backup_symlink() {
        use std::os::unix::fs::symlink;

        let fixture = Fixture::new().await;
        let backup = fixture.root.path().join("backups/source.sqlite3");
        create_sqlite_backup(&fixture.pool, &fixture.backups, &backup)
            .await
            .expect("backup should succeed");
        let link = fixture.root.path().join("backups/source-link.sqlite3");
        symlink(&backup, &link).expect("backup symlink should be created");
        let restore_directory = fixture.root.path().join("symlink-restore");
        fs::create_dir(&restore_directory).unwrap();
        let target = restore_directory.join("db/locus-desk.sqlite3");
        fs::create_dir(target.parent().unwrap()).unwrap();

        let error = restore_sqlite_backup(&link, &restore_directory, &target)
            .await
            .expect_err("restore must open the source with no-follow semantics");

        assert!(matches!(error, DataManagementError::InvalidPath { .. }));
        assert!(!target.exists());
    }

    #[cfg(unix)]
    #[test]
    fn backup_source_open_rejects_a_fifo_without_blocking() {
        use std::{ffi::CString, os::unix::ffi::OsStrExt};

        let root = tempfile::tempdir().expect("temporary directory should be created");
        let fifo = root.path().join("backup.pipe");
        let fifo_path = CString::new(fifo.as_os_str().as_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(fifo_path.as_ptr(), 0o600) }, 0);

        let error = super::open_backup_source(&fifo)
            .expect_err("non-regular backup sources must be rejected");

        assert!(matches!(error, DataManagementError::InvalidPath { .. }));
    }

    #[tokio::test]
    async fn restore_rejects_a_source_with_sqlite_companions() {
        let fixture = Fixture::new().await;
        let backup = fixture.root.path().join("backups/source.sqlite3");
        create_sqlite_backup(&fixture.pool, &fixture.backups, &backup)
            .await
            .expect("backup should succeed");
        fs::write(super::path_with_suffix(&backup, "-wal"), b"live")
            .expect("WAL fixture should be written");
        let restore_directory = fixture.root.path().join("live-restore");
        fs::create_dir(&restore_directory).unwrap();
        let target = restore_directory.join("db/locus-desk.sqlite3");
        fs::create_dir(target.parent().unwrap()).unwrap();

        let error = restore_sqlite_backup(&backup, &restore_directory, &target)
            .await
            .expect_err("a possible live database must be rejected");

        assert!(matches!(error, DataManagementError::InvalidBackup { .. }));
        assert!(!target.exists());
    }

    #[test]
    fn publication_never_replaces_a_destination_created_after_validation() {
        let root = tempfile::tempdir().expect("temporary directory should be created");
        let destination = root.path().join("artifact");
        let (temporary, file) = super::create_private_temporary_file(&destination, "json").unwrap();
        drop(file);
        fs::write(&temporary, b"loser").expect("temporary data should be written");
        fs::write(&destination, b"winner").expect("winning artifact should be published");
        let mut guard = super::TemporaryPathGuard::new(temporary);

        let error = super::publish_temporary_file(&mut guard, &destination)
            .expect_err("publication must not replace an existing artifact");
        assert!(matches!(error, DataManagementError::DestinationExists(_)));
        assert_eq!(fs::read(&destination).unwrap(), b"winner");
    }

    #[cfg(unix)]
    #[test]
    fn stale_temporary_cleanup_only_removes_private_files_from_dead_processes() {
        use std::os::unix::fs::{PermissionsExt, symlink};

        let root = tempfile::tempdir().expect("temporary directory should be created");
        let directory = root.path();
        let dead_pid = i32::MAX as u32;
        assert!(!super::process_may_be_running(dead_pid));
        let stale = directory.join(format!(
            "{}restore-{dead_pid}-0",
            super::TEMPORARY_FILE_PREFIX
        ));
        let stale_wal = super::path_with_suffix(&stale, "-wal");
        let active = directory.join(format!(
            "{}restore-{}-1",
            super::TEMPORARY_FILE_PREFIX,
            std::process::id()
        ));
        let custom = directory.join(format!(
            "{}restore-{dead_pid}-2-custom",
            super::TEMPORARY_FILE_PREFIX
        ));
        let custom_symlink = directory.join(format!(
            "{}restore-{dead_pid}-3",
            super::TEMPORARY_FILE_PREFIX
        ));
        let non_private = directory.join(format!(
            "{}restore-{dead_pid}-4",
            super::TEMPORARY_FILE_PREFIX
        ));
        for path in [&stale, &stale_wal, &active, &custom] {
            fs::write(path, b"temporary").unwrap();
            fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
        }
        symlink(directory.join("missing-custom-target"), &custom_symlink).unwrap();
        fs::write(&non_private, b"custom").unwrap();
        fs::set_permissions(&non_private, fs::Permissions::from_mode(0o640)).unwrap();

        super::cleanup_stale_temporary_files(directory).expect("stale cleanup should succeed");

        assert!(!stale.exists());
        assert!(!stale_wal.exists());
        assert!(active.exists());
        assert!(custom.exists());
        assert!(fs::symlink_metadata(custom_symlink).is_ok());
        assert!(non_private.exists());
    }

    #[tokio::test]
    async fn backup_retention_only_removes_expired_valid_managed_files() {
        let fixture = Fixture::new().await;
        let directory = fixture.backups.canonicalize().expect("path should resolve");
        let seed = directory.join("retention-seed.sqlite3");
        create_sqlite_backup(&fixture.pool, &directory, &seed)
            .await
            .expect("seed backup should be created");
        let start = chrono::DateTime::parse_from_rfc3339("2026-08-23T12:00:00Z")
            .expect("timestamp should parse")
            .timestamp_millis();

        let mut protected = None;
        for day in 0..60 {
            let timestamp = start - day * 86_400_000;
            let path = directory.join(format!("backup-{timestamp}-schema-1.sqlite3"));
            fs::copy(&seed, &path).expect("managed backup fixture should be copied");
            if day == 59 {
                protected = Some(path);
            }
        }
        fs::write(directory.join("manual-keep.sqlite3"), b"fixture")
            .expect("custom backup fixture should be written");
        fs::copy(
            &seed,
            directory.join(format!("pre-migration-{start}-schema-1.sqlite3")),
        )
        .expect("pre-migration fixture should be copied");
        fs::write(
            directory.join(format!("backup-{start}-schema-manual.sqlite3")),
            b"fixture",
        )
        .expect("similar custom fixture should be written");
        fs::write(
            directory.join(format!("backup-0{start}-schema-+1.sqlite3")),
            b"fixture",
        )
        .expect("non-canonical custom fixture should be written");
        let corrupt = directory.join(format!("backup-{}-schema-1.sqlite3", start + 86_400_000));
        fs::write(&corrupt, b"not a SQLite snapshot")
            .expect("corrupt managed-looking fixture should be written");

        let protected = protected.expect("a protected fixture should be selected");
        let removed = prune_managed_backups(&directory, Some(&protected))
            .await
            .expect("retention should succeed");
        assert_eq!(removed.len(), 48);
        let managed_remaining = fs::read_dir(&directory)
            .expect("directory should be readable")
            .filter_map(Result::ok)
            .filter(|entry| super::managed_backup_identity(&entry.file_name()).is_some())
            .count();
        assert_eq!(managed_remaining, 14);
        assert!(protected.exists());
        assert!(corrupt.exists());
        assert!(directory.join("manual-keep.sqlite3").exists());
        assert!(
            directory
                .join(format!("pre-migration-{start}-schema-1.sqlite3"))
                .exists()
        );
        assert!(
            directory
                .join(format!("backup-{start}-schema-manual.sqlite3"))
                .exists()
        );
        assert!(
            directory
                .join(format!("backup-0{start}-schema-+1.sqlite3"))
                .exists()
        );
    }

    struct Fixture {
        root: TempDir,
        backups: std::path::PathBuf,
        exports: std::path::PathBuf,
        pool: SqlitePool,
    }

    impl Fixture {
        async fn new() -> Self {
            let root = tempfile::tempdir().expect("temporary directory should be created");
            let backups = root.path().join("backups");
            let exports = root.path().join("exports");
            fs::create_dir(&backups).expect("backup directory should be created");
            fs::create_dir(&exports).expect("export directory should be created");
            let database = root.path().join("source.sqlite3");
            let options = SqliteConnectOptions::new()
                .filename(&database)
                .create_if_missing(true);
            let pool = SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(options)
                .await
                .expect("test database should open");

            crate::db::migrate(&pool)
                .await
                .expect("embedded migrations should succeed");
            seed_data(&pool).await;

            Self {
                root,
                backups,
                exports,
                pool,
            }
        }
    }

    async fn open_existing_database(path: &std::path::Path) -> SqlitePool {
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(path)
                    .read_only(true)
                    .create_if_missing(false),
            )
            .await
            .expect("existing database should open")
    }

    async fn open_writable_database(path: &std::path::Path) -> SqlitePool {
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(path)
                    .create_if_missing(false),
            )
            .await
            .expect("existing database should open for writes")
    }

    async fn seed_data(pool: &SqlitePool) {
        for statement in [
            "INSERT INTO users (id, uid, username, password_hash, created_at, updated_at) VALUES (9911223399, 'user_01', 'owner', 'secret-password-hash', 1000, 1000)",
            "INSERT INTO workspaces (id, uid, name, timezone, created_by, created_at, updated_at) VALUES (8811223399, 'workspace_01', 'Personal', 'Asia/Singapore', 9911223399, 1000, 1000)",
            "INSERT INTO workspace_members (workspace_id, user_id, role, created_at) VALUES (8811223399, 9911223399, 'OWNER', 1000)",
            "INSERT INTO notes (id, uid, workspace_id, creator_id, content, status, pinned, created_at, updated_at) VALUES (7711223399, 'note_01', 8811223399, 9911223399, '# Original note\n\n你好, world.', 'ACTIVE', 1, 1100, 1100)",
            "INSERT INTO note_tags (note_id, tag) VALUES (7711223399, '中文')",
            "INSERT INTO tasks (id, uid, workspace_id, creator_id, title, description, status, priority, due_date, due_time, sort_key, completed_at, created_at, updated_at) VALUES (6611223399, 'task_01', 8811223399, 9911223399, 'Ship the safe export', 'Verify the generated files.', 'TODO', 1, '2026-08-23', '18:30', 0, NULL, 1200, 1200)",
            "INSERT INTO sessions (token_hash, user_id, active_workspace_id, created_at, expires_at) VALUES ('secret-session-hash', 9911223399, 8811223399, 1000, 2000)",
        ] {
            sqlx::query(statement)
                .execute(pool)
                .await
                .expect("fixture statement should succeed");
        }
    }
}
