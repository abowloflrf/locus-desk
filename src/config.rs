use std::{
    env,
    error::Error,
    fmt::{self, Display, Formatter},
    net::{AddrParseError, SocketAddr},
    path::{Path, PathBuf},
    str::FromStr,
};

use chrono_tz::Tz;

const DEFAULT_BIND: &str = "127.0.0.1:7310";
const DEFAULT_TIMEZONE: &str = "Asia/Singapore";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppEnvironment {
    Development,
    Test,
    Production,
}

impl Display for AppEnvironment {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Development => "development",
            Self::Test => "test",
            Self::Production => "production",
        })
    }
}

impl AppEnvironment {
    fn parse(value: &str) -> Result<Self, ConfigError> {
        match value {
            "development" => Ok(Self::Development),
            "test" => Ok(Self::Test),
            "production" => Ok(Self::Production),
            _ => Err(ConfigError::InvalidEnvironment(value.to_owned())),
        }
    }
}

#[derive(Clone)]
pub struct Config {
    environment: AppEnvironment,
    bind: SocketAddr,
    data_dir: PathBuf,
    timezone: Tz,
    admin_username: Option<String>,
    admin_password: Option<String>,
    cookie_secure: bool,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let environment = AppEnvironment::parse(
            &env::var("APP_ENV").unwrap_or_else(|_| "development".to_owned()),
        )?;
        let bind = env::var("APP_BIND")
            .unwrap_or_else(|_| DEFAULT_BIND.to_owned())
            .parse()
            .map_err(ConfigError::InvalidBind)?;
        let data_dir = match env::var("APP_DATA_DIR") {
            Ok(value) if !value.trim().is_empty() => PathBuf::from(value),
            Ok(_) => return Err(ConfigError::EmptyDataDir),
            Err(env::VarError::NotPresent) => match environment {
                AppEnvironment::Development => PathBuf::from("./var/dev"),
                AppEnvironment::Test => PathBuf::from("./var/test"),
                AppEnvironment::Production => return Err(ConfigError::MissingProductionDataDir),
            },
            Err(error) => return Err(ConfigError::InvalidUnicode("APP_DATA_DIR", error)),
        };

        if environment == AppEnvironment::Production && !data_dir.is_absolute() {
            return Err(ConfigError::ProductionDataDirMustBeAbsolute(data_dir));
        }

        let timezone_value =
            env::var("APP_TIMEZONE").unwrap_or_else(|_| DEFAULT_TIMEZONE.to_owned());
        let timezone = Tz::from_str(&timezone_value)
            .map_err(|_| ConfigError::InvalidTimezone(timezone_value))?;

        Ok(Self {
            environment,
            bind,
            data_dir,
            timezone,
            admin_username: optional_env("APP_ADMIN_USERNAME")?,
            admin_password: optional_env("APP_ADMIN_PASSWORD")?,
            cookie_secure: parse_bool_env("APP_COOKIE_SECURE", false)?,
        })
    }

    #[cfg(test)]
    pub fn for_test(
        data_dir: PathBuf,
        username: impl Into<String>,
        password: impl Into<String>,
        timezone: Tz,
    ) -> Self {
        Self {
            environment: AppEnvironment::Test,
            bind: "127.0.0.1:0".parse().expect("test bind address is valid"),
            data_dir,
            timezone,
            admin_username: Some(username.into()),
            admin_password: Some(password.into()),
            cookie_secure: false,
        }
    }

    pub const fn environment(&self) -> AppEnvironment {
        self.environment
    }

    pub const fn bind(&self) -> SocketAddr {
        self.bind
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn database_path(&self) -> PathBuf {
        self.data_dir.join("db/locus-desk.sqlite3")
    }

    pub fn backups_dir(&self) -> PathBuf {
        self.data_dir.join("backups")
    }

    pub fn exports_dir(&self) -> PathBuf {
        self.data_dir.join("exports")
    }

    pub const fn timezone(&self) -> Tz {
        self.timezone
    }

    pub fn timezone_name(&self) -> &str {
        self.timezone.name()
    }

    pub fn bootstrap_credentials(&self) -> Option<(&str, &str)> {
        self.admin_username
            .as_deref()
            .zip(self.admin_password.as_deref())
    }

    pub const fn cookie_secure(&self) -> bool {
        self.cookie_secure
    }
}

fn optional_env(name: &'static str) -> Result<Option<String>, ConfigError> {
    match env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(error) => Err(ConfigError::InvalidUnicode(name, error)),
    }
}

fn parse_bool_env(name: &'static str, default: bool) -> Result<bool, ConfigError> {
    match env::var(name) {
        Ok(value) => match value.as_str() {
            "true" => Ok(true),
            "false" => Ok(false),
            _ => Err(ConfigError::InvalidBoolean(name, value)),
        },
        Err(env::VarError::NotPresent) => Ok(default),
        Err(error) => Err(ConfigError::InvalidUnicode(name, error)),
    }
}

#[derive(Debug)]
pub enum ConfigError {
    InvalidEnvironment(String),
    InvalidBind(AddrParseError),
    InvalidTimezone(String),
    InvalidBoolean(&'static str, String),
    InvalidUnicode(&'static str, env::VarError),
    EmptyDataDir,
    MissingProductionDataDir,
    ProductionDataDirMustBeAbsolute(PathBuf),
}

impl Display for ConfigError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEnvironment(value) => write!(
                formatter,
                "APP_ENV must be development, test, or production; got {value:?}"
            ),
            Self::InvalidBind(error) => write!(formatter, "APP_BIND is invalid: {error}"),
            Self::InvalidTimezone(value) => {
                write!(
                    formatter,
                    "APP_TIMEZONE is not a valid IANA timezone: {value:?}"
                )
            }
            Self::InvalidBoolean(name, value) => {
                write!(formatter, "{name} must be true or false; got {value:?}")
            }
            Self::InvalidUnicode(name, _) => write!(formatter, "{name} contains invalid Unicode"),
            Self::EmptyDataDir => formatter.write_str("APP_DATA_DIR must not be empty"),
            Self::MissingProductionDataDir => {
                formatter.write_str("APP_DATA_DIR is required in production")
            }
            Self::ProductionDataDirMustBeAbsolute(path) => write!(
                formatter,
                "APP_DATA_DIR must be absolute in production; got {:?}",
                path
            ),
        }
    }
}

impl Error for ConfigError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidBind(error) => Some(error),
            Self::InvalidUnicode(_, error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    use super::DEFAULT_BIND;

    #[test]
    fn default_bind_is_loopback_only() {
        assert_eq!(
            DEFAULT_BIND.parse::<SocketAddr>().unwrap(),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 7310)
        );
    }
}
