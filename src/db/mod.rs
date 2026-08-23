//! SQLite persistence boundary.

use std::{fs, path::Path, str::FromStr, time::Duration};

use sqlx::{
    SqlitePool,
    migrate::Migrator,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
};
use tracing::info;
use ulid::Ulid;

use crate::{
    auth,
    config::Config,
    error::{AppError, AppResult},
};

static MIGRATOR: Migrator = sqlx::migrate!();

pub(crate) fn embedded_migrations() -> impl Iterator<Item = &'static sqlx::migrate::Migration> {
    MIGRATOR.iter()
}

pub async fn connect(config: &Config) -> AppResult<SqlitePool> {
    let database_path = config.database_path();
    let parent = database_path
        .parent()
        .ok_or_else(|| AppError::Setup("database path has no parent directory".to_owned()))?;
    prepare_private_managed_directory(parent)?;
    ensure_managed_file_absent_or_regular(&database_path)?;
    for suffix in ["-wal", "-shm", "-journal"] {
        ensure_managed_file_absent_or_regular(&path_with_suffix(&database_path, suffix))?;
    }

    let options = SqliteConnectOptions::from_str("sqlite://placeholder")
        .map_err(AppError::Database)?
        .filename(&database_path)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(Duration::from_secs(5));

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .min_connections(1)
        .connect_with(options)
        .await?;
    set_private_file_permissions(&database_path)?;
    for suffix in ["-wal", "-shm", "-journal"] {
        let path = path_with_suffix(&database_path, suffix);
        if path.exists() {
            set_private_file_permissions(&path)?;
        }
    }
    Ok(pool)
}

pub fn prepare_data_directories(config: &Config) -> AppResult<()> {
    prepare_private_data_root(config.data_dir())?;
    let database_directory = config
        .database_path()
        .parent()
        .ok_or_else(|| AppError::Setup("database path has no parent directory".to_owned()))?
        .to_owned();
    for directory in [
        database_directory,
        config.backups_dir(),
        config.exports_dir(),
    ] {
        prepare_private_managed_directory(&directory)?;
    }
    Ok(())
}

pub(crate) fn prepare_private_data_root(path: &Path) -> AppResult<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                return Err(AppError::Setup(format!(
                    "APP_DATA_DIR must not be a symbolic link: {}",
                    path.display()
                )));
            }
            if !metadata.is_dir() {
                return Err(AppError::Setup(format!(
                    "APP_DATA_DIR must be a directory: {}",
                    path.display()
                )));
            }
            let canonical = fs::canonicalize(path)?;
            if canonical.parent().is_none() {
                return Err(AppError::Setup(
                    "APP_DATA_DIR must not be a filesystem root".to_owned(),
                ));
            }
            ensure_existing_data_root_is_private(path, &metadata)?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            create_private_directory_tree(path)?;
            let metadata = fs::symlink_metadata(path)?;
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(AppError::Setup(format!(
                    "APP_DATA_DIR could not be created as a private directory: {}",
                    path.display()
                )));
            }
            set_private_directory_permissions(path)?;
        }
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

pub(crate) fn prepare_private_managed_directory(path: &Path) -> AppResult<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(AppError::Setup(format!(
                    "managed data path must be a regular directory: {}",
                    path.display()
                )));
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            create_private_directory_tree(path)?;
        }
        Err(error) => return Err(error.into()),
    }
    set_private_directory_permissions(path)
}

fn create_private_directory_tree(path: &Path) -> AppResult<()> {
    let mut missing = Vec::new();
    let mut current = path.to_owned();
    loop {
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    return Err(AppError::Setup(format!(
                        "data directory ancestor must be a regular directory: {}",
                        current.display()
                    )));
                }
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                missing.push(current.clone());
                current = match current.parent() {
                    Some(parent) if !parent.as_os_str().is_empty() => parent.to_owned(),
                    _ => Path::new(".").to_owned(),
                };
            }
            Err(error) => return Err(error.into()),
        }
    }

    for directory in missing.iter().rev() {
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(directory)?;
        set_private_directory_permissions(directory)?;
        sync_directory(directory)?;
        if let Some(parent) = directory
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            sync_directory(parent)?;
        } else {
            sync_directory(Path::new("."))?;
        }
    }
    Ok(())
}

fn sync_directory(path: &Path) -> AppResult<()> {
    #[cfg(unix)]
    {
        use std::fs::File;

        File::open(path)?.sync_all()?;
    }
    Ok(())
}

fn ensure_existing_data_root_is_private(path: &Path, metadata: &fs::Metadata) -> AppResult<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;

        if !data_root_is_private(metadata.mode() & 0o777, metadata.uid(), unsafe {
            libc::geteuid()
        }) {
            return Err(AppError::Setup(format!(
                "existing APP_DATA_DIR must be owned by the current user and have permissions 0700: {}",
                path.display()
            )));
        }
    }
    Ok(())
}

#[cfg(unix)]
const fn data_root_is_private(mode: u32, owner_uid: u32, effective_uid: u32) -> bool {
    mode == 0o700 && owner_uid == effective_uid
}

pub(crate) fn set_private_directory_permissions(path: &Path) -> AppResult<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn set_private_file_permissions(path: &Path) -> AppResult<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(AppError::Setup(format!(
            "managed database path must be a regular file: {}",
            path.display()
        )));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

fn ensure_managed_file_absent_or_regular(path: &Path) -> AppResult<()> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => Ok(()),
        Ok(_) => Err(AppError::Setup(format!(
            "managed database path must be absent or a regular file: {}",
            path.display()
        ))),
        Err(error) => Err(error.into()),
    }
}

fn path_with_suffix(path: &Path, suffix: &str) -> std::path::PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(suffix);
    value.into()
}

pub async fn migrate(pool: &SqlitePool) -> AppResult<()> {
    MIGRATOR.run(pool).await?;
    Ok(())
}

pub async fn schema_version(pool: &SqlitePool) -> AppResult<i64> {
    let migration_table_exists = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT EXISTS(
          SELECT 1 FROM sqlite_schema
          WHERE type = 'table' AND name = '_sqlx_migrations'
        )
        "#,
    )
    .fetch_one(pool)
    .await?;
    if migration_table_exists == 0 {
        return Ok(0);
    }

    let version = sqlx::query_scalar::<_, Option<i64>>(
        "SELECT MAX(version) FROM _sqlx_migrations WHERE success = 1",
    )
    .fetch_one(pool)
    .await?
    .unwrap_or(0);
    Ok(version)
}

pub fn latest_schema_version() -> i64 {
    MIGRATOR
        .iter()
        .map(|migration| migration.version)
        .max()
        .unwrap_or(0)
}

pub async fn is_initialized(pool: &SqlitePool) -> AppResult<bool> {
    Ok(sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await?
        > 0)
}

pub async fn bootstrap(pool: &SqlitePool, config: &Config, now: i64) -> AppResult<()> {
    if is_initialized(pool).await? {
        if config.bootstrap_credentials().is_some() {
            info!("database is already initialized; bootstrap credentials were ignored");
        }
        return Ok(());
    }

    let Some((username, password)) = config.bootstrap_credentials() else {
        return Err(AppError::Setup(
            "APP_ADMIN_USERNAME and APP_ADMIN_PASSWORD are required for an empty database"
                .to_owned(),
        ));
    };
    let username = username.trim();
    if username.is_empty() || username.chars().count() > 100 {
        return Err(AppError::Setup(
            "APP_ADMIN_USERNAME must contain between 1 and 100 characters".to_owned(),
        ));
    }
    validate_bootstrap_password(password)?;

    let password_hash = auth::hash_password(password.to_owned()).await?;
    let mut transaction = pool.begin().await?;
    let existing = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users")
        .fetch_one(&mut *transaction)
        .await?;
    if existing > 0 {
        transaction.commit().await?;
        return Ok(());
    }

    let user_uid = Ulid::generate().to_string();
    let user = sqlx::query(
        "INSERT INTO users (uid, username, password_hash, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&user_uid)
    .bind(username)
    .bind(password_hash)
    .bind(now)
    .bind(now)
    .execute(&mut *transaction)
    .await?;
    let user_id = user.last_insert_rowid();

    let workspace_uid = Ulid::generate().to_string();
    let workspace = sqlx::query(
        "INSERT INTO workspaces (uid, name, timezone, created_by, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&workspace_uid)
    .bind("Personal")
    .bind(config.timezone_name())
    .bind(user_id)
    .bind(now)
    .bind(now)
    .execute(&mut *transaction)
    .await?;
    let workspace_id = workspace.last_insert_rowid();

    sqlx::query(
        "INSERT INTO workspace_members (workspace_id, user_id, role, created_at) VALUES (?, ?, 'OWNER', ?)",
    )
    .bind(workspace_id)
    .bind(user_id)
    .bind(now)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;

    info!(username, workspace_uid, "initialized owner workspace");
    Ok(())
}

fn validate_bootstrap_password(password: &str) -> AppResult<()> {
    if password.len() < 8 {
        return Err(AppError::Setup(
            "APP_ADMIN_PASSWORD must contain at least 8 bytes".to_owned(),
        ));
    }
    if password.len() > auth::MAX_PASSWORD_BYTES {
        return Err(AppError::Setup(format!(
            "APP_ADMIN_PASSWORD must not exceed {} bytes",
            auth::MAX_PASSWORD_BYTES
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use chrono_tz::Asia::Singapore;

    #[cfg(unix)]
    use super::data_root_is_private;
    use super::{prepare_data_directories, validate_bootstrap_password};
    use crate::{config::Config, error::AppError};

    #[test]
    fn creates_private_data_directories() {
        let parent = tempfile::tempdir().expect("temporary directory should be created");
        let data_root = parent.path().join("data");
        let config = Config::for_test(data_root.clone(), "owner", "password", Singapore);

        prepare_data_directories(&config).expect("data directories should be prepared");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            for directory in [
                data_root,
                config.database_path().parent().unwrap().to_owned(),
                config.backups_dir(),
                config.exports_dir(),
            ] {
                let mode = fs::metadata(directory).unwrap().permissions().mode() & 0o777;
                assert_eq!(mode, 0o700);
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn rejects_shared_existing_data_root_without_changing_its_mode() {
        use std::os::unix::fs::PermissionsExt;

        let parent = tempfile::tempdir().expect("temporary directory should be created");
        let data_root = parent.path().join("shared");
        fs::create_dir(&data_root).expect("shared directory should be created");
        fs::set_permissions(&data_root, fs::Permissions::from_mode(0o755)).unwrap();
        let config = Config::for_test(data_root.clone(), "owner", "password", Singapore);

        let error = prepare_data_directories(&config)
            .expect_err("an existing shared directory must be rejected");

        assert!(matches!(error, AppError::Setup(_)));
        let mode = fs::metadata(data_root).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o755);
    }

    #[cfg(unix)]
    #[test]
    fn private_data_root_requires_mode_and_effective_owner() {
        assert!(data_root_is_private(0o700, 1000, 1000));
        assert!(!data_root_is_private(0o700, 1001, 1000));
        assert!(!data_root_is_private(0o750, 1000, 1000));
        assert!(!data_root_is_private(0o770, 1001, 1000));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_filesystem_root_without_changing_its_mode() {
        use std::os::unix::fs::PermissionsExt;

        let root = std::path::PathBuf::from("/");
        let before = fs::metadata(&root).unwrap().permissions().mode() & 0o777;
        let config = Config::for_test(root.clone(), "owner", "password", Singapore);

        let error = prepare_data_directories(&config)
            .expect_err("the filesystem root must never be a data directory");

        assert!(matches!(error, AppError::Setup(_)));
        let after = fs::metadata(root).unwrap().permissions().mode() & 0o777;
        assert_eq!(after, before);
    }

    #[cfg(unix)]
    #[test]
    fn rejects_managed_directory_symlinks_without_touching_the_target() {
        use std::os::unix::fs::{PermissionsExt, symlink};

        let parent = tempfile::tempdir().expect("temporary directory should be created");
        let data_root = parent.path().join("data");
        fs::create_dir(&data_root).unwrap();
        fs::set_permissions(&data_root, fs::Permissions::from_mode(0o700)).unwrap();
        let external = parent.path().join("external");
        fs::create_dir(&external).unwrap();
        fs::set_permissions(&external, fs::Permissions::from_mode(0o755)).unwrap();
        symlink(&external, data_root.join("backups")).unwrap();
        let config = Config::for_test(data_root, "owner", "password", Singapore);

        let error = prepare_data_directories(&config)
            .expect_err("managed directory symlinks must be rejected");

        assert!(matches!(error, AppError::Setup(_)));
        let mode = fs::metadata(external).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o755);
    }

    #[test]
    fn bootstrap_password_uses_the_login_byte_limit() {
        let exact_ascii = "a".repeat(crate::auth::MAX_PASSWORD_BYTES);
        let exact_multibyte = format!(
            "{}a",
            "界".repeat((crate::auth::MAX_PASSWORD_BYTES - 1) / "界".len())
        );
        assert_eq!(exact_multibyte.len(), crate::auth::MAX_PASSWORD_BYTES);

        assert!(validate_bootstrap_password(&exact_ascii).is_ok());
        assert!(validate_bootstrap_password(&exact_multibyte).is_ok());
        assert!(validate_bootstrap_password(&(exact_ascii + "a")).is_err());
        assert!(validate_bootstrap_password(&(exact_multibyte + "a")).is_err());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn rejects_database_and_companion_symlinks_without_touching_targets() {
        use std::os::unix::fs::{PermissionsExt, symlink};

        for suffix in ["", "-wal", "-shm"] {
            let parent = tempfile::tempdir().expect("temporary directory should be created");
            let data_root = parent.path().join("data");
            let config = Config::for_test(data_root, "owner", "password", Singapore);
            prepare_data_directories(&config).unwrap();
            let external = parent.path().join("external");
            fs::write(&external, b"not a database").unwrap();
            fs::set_permissions(&external, fs::Permissions::from_mode(0o644)).unwrap();
            symlink(
                &external,
                super::path_with_suffix(&config.database_path(), suffix),
            )
            .unwrap();

            let error = super::connect(&config)
                .await
                .expect_err("managed database symlinks must be rejected");

            assert!(matches!(error, AppError::Setup(_)));
            let mode = fs::metadata(external).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o644);
        }
    }
}
