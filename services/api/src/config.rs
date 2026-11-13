use base64::{Engine, engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD}};
use sha2::{Digest, Sha256};
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
    /// Public origin of the web app (used in email links).
    pub public_url: String,
    /// Origins allowed to make state-changing requests (CSRF Origin check).
    pub allowed_origins: Vec<String>,
    /// Derived 256-bit key for CSRF tokens.
    pub csrf_key: [u8; 32],
    /// 256-bit key encrypting MFA secrets at rest.
    pub mfa_key: [u8; 32],
    /// Number of trusted reverse-proxy hops in X-Forwarded-For (0 = use the socket address).
    pub trusted_proxy_hops: usize,
    pub smtp_url: Option<String>,
    pub mail_from: String,
}

impl Config {
    pub fn cookie_secure(&self) -> bool {
        self.environment != Environment::Development
    }

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
        let database_url = req("DATABASE_URL")?;
        let redis_url = req("REDIS_URL")?;
        let csrf_secret = req("CSRF_SECRET")?;
        if csrf_secret.len() < 32 {
            return Err(ConfigError::Invalid("CSRF_SECRET"));
        }
        let csrf_key: [u8; 32] = Sha256::digest(csrf_secret.as_bytes()).into();
        let mfa_raw = req("MFA_ENCRYPTION_KEY")?;
        let mfa_bytes = STANDARD.decode(mfa_raw.trim()).or_else(|_| URL_SAFE_NO_PAD.decode(mfa_raw.trim())).map_err(|_| ConfigError::Invalid("MFA_ENCRYPTION_KEY"))?;
        let mfa_key: [u8; 32] = mfa_bytes.try_into().map_err(|_| ConfigError::Invalid("MFA_ENCRYPTION_KEY"))?;
        let public_url = get("PUBLIC_URL").unwrap_or_else(|| "http://localhost:3000".into()).trim_end_matches('/').to_owned();
        let allowed_origins = get("ALLOWED_ORIGINS").map(|v| v.split(',').map(|o| o.trim().trim_end_matches('/').to_owned()).filter(|o| !o.is_empty()).collect()).unwrap_or_else(|| vec![public_url.clone()]);
        let trusted_proxy_hops = get("TRUSTED_PROXY_HOPS").map(|v| v.parse().map_err(|_| ConfigError::Invalid("TRUSTED_PROXY_HOPS"))).transpose()?.unwrap_or(0);
        let smtp_url = get("SMTP_URL").filter(|v| !v.trim().is_empty());
        // Credentials and mail contents must never cross the network in clear text outside development.
        if let Some(u) = &smtp_url {
            let tls_ok = u.starts_with("smtps://") || u.contains("tls=required");
            if environment != Environment::Development && !tls_ok {
                return Err(ConfigError::Invalid("SMTP_URL"));
            }
        }
        Ok(Self {
            public_url,
            allowed_origins,
            csrf_key,
            mfa_key,
            trusted_proxy_hops,
            smtp_url,
            mail_from: get("MAIL_FROM").unwrap_or_else(|| "PERMTPA ONE <no-reply@localhost>".into()),
            bind,
            environment,
            database_url,
            migrator_database_url: get("MIGRATOR_DATABASE_URL").filter(|v| !v.trim().is_empty()),
            redis_url,
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
            .field("public_url", &self.public_url)
            .field("allowed_origins", &self.allowed_origins)
            .field("csrf_key", &"<redacted>")
            .field("mfa_key", &"<redacted>")
            .field("smtp_url", &self.smtp_url.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env_fn(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let m: HashMap<String, String> = pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        move |k| m.get(k).cloned()
    }

    #[test]
    fn requires_database_redis_and_secrets() {
        assert_eq!(Config::from_lookup(env_fn(&[])).unwrap_err(), ConfigError::Missing("DATABASE_URL"));
        assert_eq!(Config::from_lookup(env_fn(&[("DATABASE_URL", "postgres://x")])).unwrap_err(), ConfigError::Missing("REDIS_URL"));
        assert_eq!(Config::from_lookup(env_fn(&[("DATABASE_URL", "x"), ("REDIS_URL", "y")])).unwrap_err(), ConfigError::Missing("CSRF_SECRET"));
    }

    const GOOD: [(&str, &str); 4] = [("DATABASE_URL", "postgres://x"), ("REDIS_URL", "redis://x"), ("CSRF_SECRET", "0123456789abcdef0123456789abcdef"), ("MFA_ENCRYPTION_KEY", "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=")];

    #[test]
    fn rejects_weak_or_malformed_secrets() {
        fn with(k: &'static str, v: &'static str) -> Vec<(&'static str, &'static str)> {
            let mut e: Vec<(&str, &str)> = GOOD.iter().filter(|(x, _)| *x != k).cloned().collect();
            e.push((k, v));
            e
        }
        assert_eq!(Config::from_lookup(env_fn(&with("CSRF_SECRET", "short"))).unwrap_err(), ConfigError::Invalid("CSRF_SECRET"));
        assert_eq!(Config::from_lookup(env_fn(&with("MFA_ENCRYPTION_KEY", "AAAA"))).unwrap_err(), ConfigError::Invalid("MFA_ENCRYPTION_KEY"));
        assert!(Config::from_lookup(env_fn(&GOOD)).is_ok());
    }

    fn with_smtp(app_env: &str, url: &str) -> Result<Config, ConfigError> {
        let mut e = GOOD.to_vec();
        e.push(("APP_ENV", app_env));
        e.push(("SMTP_URL", url));
        Config::from_lookup(env_fn(&e))
    }

    #[test]
    fn smtp_must_use_tls_outside_development() {
        assert!(with_smtp("development", "smtp://127.0.0.1:1025").is_ok(), "plaintext allowed for a local dev sink");
        assert_eq!(with_smtp("production", "smtp://mail.example.com:587").unwrap_err(), ConfigError::Invalid("SMTP_URL"));
        assert!(with_smtp("production", "smtps://u:p@mail.example.com:465").is_ok());
        assert!(with_smtp("staging", "smtp://mail.example.com:587?tls=required").is_ok());
    }

    #[test]
    fn cookies_are_secure_outside_development() {
        let mut e = GOOD.to_vec();
        assert!(!Config::from_lookup(env_fn(&e)).unwrap().cookie_secure());
        e.push(("APP_ENV", "production"));
        assert!(Config::from_lookup(env_fn(&e)).unwrap().cookie_secure());
    }

    #[test]
    fn rejects_bad_values() {
        let base = GOOD;
        let mut bad_env = base.to_vec();
        bad_env.push(("APP_ENV", "prod-ish"));
        assert_eq!(Config::from_lookup(env_fn(&bad_env)).unwrap_err(), ConfigError::Invalid("APP_ENV"));
        let mut bad_bind = base.to_vec();
        bad_bind.push(("API_BIND", "not-an-addr"));
        assert_eq!(Config::from_lookup(env_fn(&bad_bind)).unwrap_err(), ConfigError::Invalid("API_BIND"));
    }

    #[test]
    fn debug_output_never_contains_secrets() {
        let c = Config::from_lookup(env_fn(&[("DATABASE_URL", "postgres://u:supersecret@h/db"), ("REDIS_URL", "redis://:hunter2@h"), ("CSRF_SECRET", "0123456789abcdef0123456789abcdef"), ("MFA_ENCRYPTION_KEY", "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8="), ("SMTP_URL", "smtp://u:smtppw@h")])).unwrap();
        let shown = format!("{c:?}");
        for secret in ["supersecret", "hunter2", "smtppw", "0123456789abcdef", "AAECAwQF"] { assert!(!shown.contains(secret), "leaked {secret}"); }
    }
}
