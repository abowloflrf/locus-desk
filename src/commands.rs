use std::{
    env,
    error::Error,
    ffi::OsString,
    fs,
    path::{Component, Path, PathBuf},
};

use chrono::Utc;
use thiserror::Error;

use crate::{
    config::Config,
    data_management::{self, DataArtifact},
    db,
    state::AppState,
};

pub const HELP: &str = r#"Locus Desk

Usage:
  locus-desk [serve]
  locus-desk backup [FILE]
  locus-desk export <json|markdown> [FILE]
  locus-desk restore <BACKUP> <TARGET_DATA_DIR>
  locus-desk --version
  locus-desk --help

Backup files are written beneath APP_DATA_DIR/backups. Portable exports are
written beneath APP_DATA_DIR/exports. Restore only accepts an empty, absolute
target data directory and never replaces an existing database.
"#;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Command {
    Serve,
    Backup {
        file_name: Option<OsString>,
    },
    Export {
        format: ExportFormat,
        file_name: Option<OsString>,
    },
    Restore {
        backup: PathBuf,
        target_data_dir: PathBuf,
    },
    Help,
    Version,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExportFormat {
    Json,
    Markdown,
}

impl ExportFormat {
    const fn extension(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Markdown => "md",
        }
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum CommandError {
    #[error("unknown command: {0}")]
    UnknownCommand(String),
    #[error("{0}")]
    InvalidArguments(&'static str),
    #[error("managed output must be a file name without directory components")]
    InvalidFileName,
    #[error("managed output file name uses a reserved temporary-file prefix")]
    ReservedFileName,
    #[error("restore target must be an absolute path")]
    RestoreTargetMustBeAbsolute,
    #[error("restore target directory must be empty: {0}")]
    RestoreTargetNotEmpty(PathBuf),
}

pub fn parse<I>(arguments: I) -> Result<Command, CommandError>
where
    I: IntoIterator<Item = OsString>,
{
    let mut arguments = arguments.into_iter();
    let Some(command) = arguments.next() else {
        return Ok(Command::Serve);
    };
    let command_text = command.to_string_lossy();

    match command_text.as_ref() {
        "serve" => {
            ensure_finished(&mut arguments, "serve does not accept arguments")?;
            Ok(Command::Serve)
        }
        "backup" => {
            let file_name = arguments.next();
            ensure_finished(&mut arguments, "backup accepts at most one file name")?;
            if let Some(value) = &file_name {
                validate_file_name(value)?;
            }
            Ok(Command::Backup { file_name })
        }
        "export" => {
            let format = match arguments.next().as_deref().and_then(|value| value.to_str()) {
                Some("json") => ExportFormat::Json,
                Some("markdown") => ExportFormat::Markdown,
                _ => {
                    return Err(CommandError::InvalidArguments(
                        "export requires a format: json or markdown",
                    ));
                }
            };
            let file_name = arguments.next();
            ensure_finished(
                &mut arguments,
                "export accepts a format and at most one file name",
            )?;
            if let Some(value) = &file_name {
                validate_file_name(value)?;
            }
            Ok(Command::Export { format, file_name })
        }
        "restore" => {
            let backup =
                arguments
                    .next()
                    .map(PathBuf::from)
                    .ok_or(CommandError::InvalidArguments(
                        "restore requires a backup path and target data directory",
                    ))?;
            let target_data_dir =
                arguments
                    .next()
                    .map(PathBuf::from)
                    .ok_or(CommandError::InvalidArguments(
                        "restore requires a backup path and target data directory",
                    ))?;
            ensure_finished(
                &mut arguments,
                "restore accepts exactly a backup path and target data directory",
            )?;
            Ok(Command::Restore {
                backup,
                target_data_dir,
            })
        }
        "--help" | "-h" | "help" => {
            ensure_finished(&mut arguments, "help does not accept arguments")?;
            Ok(Command::Help)
        }
        "--version" | "-V" | "version" => {
            ensure_finished(&mut arguments, "version does not accept arguments")?;
            Ok(Command::Version)
        }
        _ => Err(CommandError::UnknownCommand(command_text.into_owned())),
    }
}

pub async fn execute(command: Command) -> Result<(), Box<dyn Error>> {
    match command {
        Command::Backup { file_name } => run_backup(file_name).await,
        Command::Export { format, file_name } => run_export(format, file_name).await,
        Command::Restore {
            backup,
            target_data_dir,
        } => run_restore(&backup, &target_data_dir).await,
        _ => Err(CommandError::InvalidArguments("command is not a data operation").into()),
    }
}

async fn run_backup(file_name: Option<OsString>) -> Result<(), Box<dyn Error>> {
    let config = Config::from_env()?;
    let state = AppState::initialize(config).await?;
    let backup_directory = fs::canonicalize(state.config().backups_dir())?;
    let schema_version = db::schema_version(state.pool()).await?;
    let default_name = format!(
        "backup-{}-schema-{schema_version}.sqlite3",
        Utc::now().timestamp_millis()
    );
    let destination = managed_destination(&backup_directory, file_name, &default_name)?;
    let artifact =
        data_management::create_sqlite_backup(state.pool(), &backup_directory, &destination)
            .await?;
    if let Err(error) =
        data_management::prune_managed_backups(&backup_directory, Some(&artifact.path)).await
    {
        eprintln!("Backup created, but retention cleanup failed: {error}");
    }
    state.pool().close().await;
    print_artifact("Backup created", &artifact);
    Ok(())
}

async fn run_export(
    format: ExportFormat,
    file_name: Option<OsString>,
) -> Result<(), Box<dyn Error>> {
    let config = Config::from_env()?;
    let state = AppState::initialize(config).await?;
    let export_directory = fs::canonicalize(state.config().exports_dir())?;
    let default_name = format!(
        "export-{}.{}",
        Utc::now().timestamp_millis(),
        format.extension()
    );
    let destination = managed_destination(&export_directory, file_name, &default_name)?;
    let artifact = match format {
        ExportFormat::Json => {
            data_management::export_json(state.pool(), &export_directory, &destination).await?
        }
        ExportFormat::Markdown => {
            data_management::export_markdown(state.pool(), &export_directory, &destination).await?
        }
    };
    state.pool().close().await;
    print_artifact("Export created", &artifact);
    Ok(())
}

async fn run_restore(backup: &Path, target_data_dir: &Path) -> Result<(), Box<dyn Error>> {
    if !target_data_dir.is_absolute() {
        return Err(CommandError::RestoreTargetMustBeAbsolute.into());
    }
    if target_data_dir.exists() {
        db::prepare_private_data_root(target_data_dir)?;
        if !restore_target_is_reusable(target_data_dir)? {
            return Err(CommandError::RestoreTargetNotEmpty(target_data_dir.to_owned()).into());
        }
    }

    let target_data_dir = prepare_restore_directory(target_data_dir)?;
    let target_database = target_data_dir.join("db/locus-desk.sqlite3");
    let artifact =
        data_management::restore_sqlite_backup(backup, &target_data_dir, &target_database).await?;
    print_artifact("Backup restored", &artifact);
    Ok(())
}

fn prepare_restore_directory(target_data_dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    db::prepare_private_data_root(target_data_dir)?;
    let database_directory = target_data_dir.join("db");
    db::prepare_private_managed_directory(&database_directory)?;
    Ok(fs::canonicalize(target_data_dir)?)
}

fn restore_target_is_reusable(path: &Path) -> Result<bool, Box<dyn Error>> {
    let mut entries = fs::read_dir(path)?;
    let Some(entry) = entries.next().transpose()? else {
        return Ok(true);
    };
    if entries.next().is_some() || entry.file_name() != "db" || !entry.file_type()?.is_dir() {
        return Ok(false);
    }
    data_management::cleanup_stale_temporary_files(&entry.path())?;
    Ok(fs::read_dir(entry.path())?.next().is_none())
}

fn managed_destination(
    directory: &Path,
    file_name: Option<OsString>,
    default_name: &str,
) -> Result<PathBuf, CommandError> {
    let file_name = file_name.unwrap_or_else(|| OsString::from(default_name));
    validate_file_name(&file_name)?;
    Ok(directory.join(file_name))
}

fn validate_file_name(value: &OsString) -> Result<(), CommandError> {
    if data_management::is_reserved_temporary_file_name(value) {
        return Err(CommandError::ReservedFileName);
    }
    let path = Path::new(value);
    let mut components = path.components();
    if matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none() {
        return Ok(());
    }
    Err(CommandError::InvalidFileName)
}

fn ensure_finished(
    arguments: &mut impl Iterator<Item = OsString>,
    message: &'static str,
) -> Result<(), CommandError> {
    if arguments.next().is_some() {
        return Err(CommandError::InvalidArguments(message));
    }
    Ok(())
}

fn print_artifact(action: &str, artifact: &DataArtifact) {
    println!(
        "{action}: {} ({} bytes)",
        artifact.path.display(),
        artifact.byte_len
    );
}

pub fn parse_environment() -> Result<Command, CommandError> {
    parse(env::args_os().skip(1))
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::{
        Command, CommandError, ExportFormat, parse, prepare_restore_directory,
        restore_target_is_reusable,
    };

    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn parses_server_and_data_commands() {
        assert_eq!(parse(args(&[])).unwrap(), Command::Serve);
        assert_eq!(parse(args(&["serve"])).unwrap(), Command::Serve);
        assert_eq!(
            parse(args(&["backup", "manual.sqlite3"])).unwrap(),
            Command::Backup {
                file_name: Some(OsString::from("manual.sqlite3"))
            }
        );
        assert_eq!(
            parse(args(&["export", "markdown"])).unwrap(),
            Command::Export {
                format: ExportFormat::Markdown,
                file_name: None
            }
        );
        assert_eq!(
            parse(args(&["restore", "/tmp/backup.sqlite3", "/tmp/restored"])).unwrap(),
            Command::Restore {
                backup: "/tmp/backup.sqlite3".into(),
                target_data_dir: "/tmp/restored".into()
            }
        );
    }

    #[test]
    fn rejects_ambiguous_or_escaping_arguments() {
        assert!(matches!(
            parse(args(&["backup", "../outside.sqlite3"])),
            Err(CommandError::InvalidFileName)
        ));
        assert!(matches!(
            parse(args(&["backup", ".locus-desk-tmp-v1-vacuum-123-0"])),
            Err(CommandError::ReservedFileName)
        ));
        assert!(matches!(
            parse(args(&["export", "xml"])),
            Err(CommandError::InvalidArguments(_))
        ));
        assert!(matches!(
            parse(args(&["serve", "extra"])),
            Err(CommandError::InvalidArguments(_))
        ));
    }

    #[test]
    fn failed_restore_directory_can_be_reused_when_only_empty_db_remains() {
        let root = tempfile::tempdir().expect("temporary directory should be created");
        let target = root.path().join("restored");
        std::fs::create_dir(&target).expect("target should be created");
        assert!(restore_target_is_reusable(&target).unwrap());

        std::fs::create_dir(target.join("db")).expect("database directory should be created");
        assert!(restore_target_is_reusable(&target).unwrap());

        std::fs::write(target.join("db/partial"), b"data").expect("fixture should be written");
        assert!(!restore_target_is_reusable(&target).unwrap());
    }

    #[cfg(unix)]
    #[test]
    fn restore_target_recovers_a_dead_process_temporary_file() {
        use std::os::unix::fs::PermissionsExt;

        let root = tempfile::tempdir().expect("temporary directory should be created");
        let target = root.path().join("restored");
        let database_directory = target.join("db");
        std::fs::create_dir_all(&database_directory).unwrap();
        let temporary =
            database_directory.join(format!(".locus-desk-tmp-v1-restore-{}-0", i32::MAX));
        std::fs::write(&temporary, b"partial restore").unwrap();
        std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o600)).unwrap();

        assert!(restore_target_is_reusable(&target).unwrap());
        assert!(!temporary.exists());
    }

    #[test]
    fn restore_directories_are_private_before_the_database_is_published() {
        let root = tempfile::tempdir().expect("temporary directory should be created");
        let target = root.path().join("restored");
        prepare_restore_directory(&target).expect("restore directory should be prepared");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            for directory in [&target, &target.join("db")] {
                let mode = std::fs::metadata(directory).unwrap().permissions().mode() & 0o777;
                assert_eq!(mode, 0o700);
            }
        }
    }
}
