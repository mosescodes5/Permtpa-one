use std::{fmt, net::SocketAddr};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    Development,
    Staging,
    Production,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("missing required environment variable {0}")]
    Missing(&'static str),
    #[error("invalid value for {0}")]
    Invalid(&'static str),
}

/// Runtime configuration. Secrets are only ever read from the environment and never printed.
#[derive(Clone)]
pub struct Config {
    pub bind: SocketAddr,
    pub environment: Environment,
    pub database_url: String,
    pub migrator_database_url: Option<String>,
    pub redis_url: String,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_lookup(|k| std::env::var(k).ok())
    }

    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let req = |k: &'static str| get(k).filter(|v| !v.trim().is_empty()).ok_or(ConfigError::Missing(k));
        let environment = match get("APP_ENV").as_deref().unwrap_or("development") {
            "development" => Environment::Development,
            "staging" => Environment::Staging,
            "production" => Environment::Production,
            _ => return Err(ConfigError::Invalid("APP_ENV")),
        };
        let bind = get("API_BIND").unwrap_or_else(|| "127.0.0.1:8080".into()).parse().map_err(|_| ConfigError::Invalid("API_BIND"))?;
        Ok(Self {
            bind,
            environment,
            database_url: req("DATABASE_URL")?,
            migrator_database_url: get("MIGRATOR_DATABASE_URL").filter(|v| !v.trim().is_empty()),
            redis_url: req("REDIS_URL")?,
        })
    }
}

// Manual Debug so connection strings (which embed passwords) can never leak into logs.
impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("bind", &self.bind)
            .field("environment", &self.environment)
            .field("database_url", &"<redacted>")
            .field("migrator_database_url", &self.migrator_database_url.as_ref().map(|_| "<redacted>"))
            .field("redis_url", &"<redacted>")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let m: HashMap<String, String> = pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        move |k| m.get(k).cloned()
    }

    #[test]
    fn requires_database_and_redis() {
        assert_eq!(Config::from_lookup(env(&[])).unwrap_err(), ConfigError::Missing("DATABASE_URL"));
        assert_eq!(Config::from_lookup(env(&[("DATABASE_URL", "postgres://x")])).unwrap_err(), ConfigError::Missing("REDIS_URL"));
    }

    #[test]
    fn rejects_bad_values() {
        let base = [("DATABASE_URL", "postgres://x"), ("REDIS_URL", "redis://x")];
        let mut bad_env = base.to_vec();
        bad_env.push(("APP_ENV", "prod-ish"));
        assert_eq!(Config::from_lookup(env(&bad_env)).unwrap_err(), ConfigError::Invalid("APP_ENV"));
        let mut bad_bind = base.to_vec();
        bad_bind.push(("API_BIND", "not-an-addr"));
        assert_eq!(Config::from_lookup(env(&bad_bind)).unwrap_err(), ConfigError::Invalid("API_BIND"));
    }

    #[test]
    fn debug_output_never_contains_secrets() {
        let c = Config::from_lookup(env(&[("DATABASE_URL", "postgres://u:supersecret@h/db"), ("REDIS_URL", "redis://:hunter2@h")])).unwrap();
        let shown = format!("{c:?}");
        assert!(!shown.contains("supersecret") && !shown.contains("hunter2"));
    }
}
