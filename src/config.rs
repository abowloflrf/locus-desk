use std::{
    env,
    error::Error,
    fmt::{self, Display, Formatter},
    net::{AddrParseError, SocketAddr},
};

const DEFAULT_BIND: &str = "0.0.0.0:7310";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppEnvironment {
    Development,
    Test,
    Production,
}

impl Display for AppEnvironment {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Development => "development",
            Self::Test => "test",
            Self::Production => "production",
        };

        formatter.write_str(value)
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

#[derive(Clone, Debug)]
pub struct Config {
    environment: AppEnvironment,
    bind: SocketAddr,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let environment = env::var("APP_ENV").unwrap_or_else(|_| "development".to_owned());
        let bind = env::var("APP_BIND").unwrap_or_else(|_| DEFAULT_BIND.to_owned());

        Ok(Self {
            environment: AppEnvironment::parse(&environment)?,
            bind: bind.parse().map_err(ConfigError::InvalidBind)?,
        })
    }

    pub const fn environment(&self) -> AppEnvironment {
        self.environment
    }

    pub const fn bind(&self) -> SocketAddr {
        self.bind
    }
}

#[derive(Debug)]
pub enum ConfigError {
    InvalidEnvironment(String),
    InvalidBind(AddrParseError),
}

impl Display for ConfigError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEnvironment(value) => write!(
                formatter,
                "APP_ENV must be development, test, or production; got {value:?}"
            ),
            Self::InvalidBind(error) => write!(formatter, "APP_BIND is invalid: {error}"),
        }
    }
}

impl Error for ConfigError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidEnvironment(_) => None,
            Self::InvalidBind(error) => Some(error),
        }
    }
}
