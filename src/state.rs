use std::{fs, sync::Arc};

use sqlx::SqlitePool;

use crate::{
    auth::{self, LoginLimiter},
    clock::{Clock, SystemClock, timestamp_millis},
    config::Config,
    data_management, db,
    error::AppResult,
};

#[derive(Clone)]
pub struct AppState {
    pool: SqlitePool,
    config: Arc<Config>,
    clock: Arc<dyn Clock>,
    login_limiter: Arc<LoginLimiter>,
}

impl AppState {
    pub async fn initialize(config: Config) -> AppResult<Self> {
        Self::initialize_with_clock(config, Arc::new(SystemClock)).await
    }

    pub async fn initialize_with_clock(config: Config, clock: Arc<dyn Clock>) -> AppResult<Self> {
        db::prepare_data_directories(&config)?;
        recover_stale_data_operation_files(&config)?;
        let pool = db::connect(&config).await?;
        let now = timestamp_millis(clock.as_ref());
        backup_before_migration(&pool, &config, now).await?;
        db::migrate(&pool).await?;
        db::bootstrap(&pool, &config, now).await?;
        auth::cleanup_expired_sessions(&pool, now).await?;

        Ok(Self {
            pool,
            config: Arc::new(config),
            clock,
            login_limiter: Arc::new(LoginLimiter::default()),
        })
    }

    pub const fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn clock(&self) -> &dyn Clock {
        self.clock.as_ref()
    }

    pub fn login_limiter(&self) -> &LoginLimiter {
        &self.login_limiter
    }
}

fn recover_stale_data_operation_files(config: &Config) -> AppResult<()> {
    let database_directory = config
        .database_path()
        .parent()
        .ok_or_else(|| crate::error::AppError::Setup("database path has no parent".to_owned()))?
        .to_owned();
    for directory in [
        database_directory,
        config.backups_dir(),
        config.exports_dir(),
    ] {
        data_management::cleanup_stale_temporary_files(&directory).map_err(|error| {
            crate::error::AppError::Setup(format!(
                "could not recover stale data operation files in {}: {error}",
                directory.display()
            ))
        })?;
    }
    Ok(())
}

async fn backup_before_migration(pool: &SqlitePool, config: &Config, now: i64) -> AppResult<()> {
    let current = db::schema_version(pool).await?;
    let latest = db::latest_schema_version();
    if current == 0 || current == latest {
        return Ok(());
    }

    let backup_directory = fs::canonicalize(config.backups_dir())?;
    let destination =
        backup_directory.join(format!("pre-migration-{now}-schema-{current}.sqlite3"));
    let artifact = data_management::create_sqlite_backup(pool, &backup_directory, &destination)
        .await
        .map_err(|error| {
            crate::error::AppError::Setup(format!("pre-migration backup failed: {error}"))
        })?;
    tracing::info!(
        path = %artifact.path.display(),
        bytes = artifact.byte_len,
        current_schema = current,
        target_schema = latest,
        "created pre-migration backup"
    );
    match data_management::prune_managed_backups(&backup_directory, Some(&artifact.path)).await {
        Ok(removed) if !removed.is_empty() => {
            tracing::info!(count = removed.len(), "pruned expired managed backups");
        }
        Ok(_) => {}
        Err(error) => {
            tracing::warn!(error = %error, "could not prune managed backups");
        }
    }
    Ok(())
}
