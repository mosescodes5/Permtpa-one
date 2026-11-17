#![allow(dead_code)]
//! Shared integration-test harness: a throwaway PostgreSQL database with the real migrations applied,
//! reached through the real least-privilege runtime role.
use permtpa_api::db::MIGRATOR;
use sqlx::{AssertSqlSafe, PgPool, postgres::PgPoolOptions};
use uuid::Uuid;

/// Roles are cluster-wide; creating/altering them from parallel tests races, so serialise that step.
static ROLE_SETUP: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub struct TestDb {
    pub admin: PgPool,
    pub name: String,
    /// Runtime role (`permtpa_app`): what the API itself uses.
    pub app: PgPool,
    /// Superuser connection INTO the test database — fixtures and assertions about data at rest only.
    pub fixture: PgPool,
}

impl TestDb {
    pub async fn create(app_connections: u32) -> Self {
        let admin_url = std::env::var("TEST_ADMIN_DATABASE_URL")
            .expect("set TEST_ADMIN_DATABASE_URL to run integration tests");
        let admin = PgPoolOptions::new()
            .max_connections(2)
            .connect(&admin_url)
            .await
            .expect("admin connect");
        let name = format!("permtpa_it_{}", Uuid::new_v4().simple());
        let _guard = ROLE_SETUP.lock().await;
        for role in ["permtpa_migrator", "permtpa_app"] {
            sqlx::query(AssertSqlSafe(format!("do $$ begin if not exists (select from pg_roles where rolname='{role}') then create role {role} login password 'it_pw' nosuperuser nobypassrls; end if; end $$"))).execute(&admin).await.unwrap();
            sqlx::query(AssertSqlSafe(format!("alter role {role} password 'it_pw'")))
                .execute(&admin)
                .await
                .unwrap();
        }
        sqlx::query(AssertSqlSafe(format!(
            "create database {name} owner permtpa_migrator"
        )))
        .execute(&admin)
        .await
        .unwrap();
        sqlx::query(AssertSqlSafe(format!(
            "revoke all on database {name} from public"
        )))
        .execute(&admin)
        .await
        .unwrap();
        sqlx::query(AssertSqlSafe(format!(
            "grant connect on database {name} to permtpa_app"
        )))
        .execute(&admin)
        .await
        .unwrap();
        drop(_guard);

        let (scheme, rest) = admin_url.split_once("://").unwrap();
        let host_and_db = rest.rsplit_once('@').map(|(_, h)| h).unwrap();
        let host = host_and_db.rsplit_once('/').map(|(h, _)| h).unwrap();
        let url_for = |user: &str, pw: &str| format!("{scheme}://{user}:{pw}@{host}/{name}");
        let admin_cred = rest.rsplit_once('@').map(|(c, _)| c).unwrap();

        let migrator = PgPoolOptions::new()
            .max_connections(1)
            .connect(&url_for("permtpa_migrator", "it_pw"))
            .await
            .expect("migrator connect");
        MIGRATOR.run(&migrator).await.expect("migrations apply");
        migrator.close().await;
        let app = PgPoolOptions::new()
            .max_connections(app_connections)
            .connect(&url_for("permtpa_app", "it_pw"))
            .await
            .expect("app connect");
        let fixture = PgPoolOptions::new()
            .max_connections(2)
            .connect(&format!("{scheme}://{admin_cred}@{host}/{name}"))
            .await
            .expect("fixture connect");
        Self {
            admin,
            name,
            app,
            fixture,
        }
    }

    pub async fn drop(self) {
        self.app.close().await;
        self.fixture.close().await;
        sqlx::query(AssertSqlSafe(format!(
            "drop database if exists {} with (force)",
            self.name
        )))
        .execute(&self.admin)
        .await
        .unwrap();
    }
}

use axum::{
    Router,
    body::Body,
    extract::ConnectInfo,
    http::{HeaderMap, Request, StatusCode},
};
use http_body_util::BodyExt;
use permtpa_api::{
    app::{AppState, router},
    config::Config,
    mailer::Mailer,
};
use serde_json::Value;
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tower::ServiceExt;

pub const ORIGIN: &str = "http://localhost:3000";

pub fn test_config(app_env: &str) -> Config {
    let redis = std::env::var("TEST_REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".into());
    let m: HashMap<&str, String> = HashMap::from([
        ("DATABASE_URL", "postgres://unused".into()),
        ("REDIS_URL", redis),
        ("APP_ENV", app_env.into()),
        (
            "CSRF_SECRET",
            "test-csrf-secret-0123456789abcdef0123".into(),
        ),
        (
            "MFA_ENCRYPTION_KEY",
            "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=".into(),
        ),
    ]);
    Config::from_lookup(|k| m.get(k).cloned()).expect("valid test config")
}

pub struct Resp {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub json: Value,
    pub text: String,
}

#[derive(Default, Clone)]
pub struct Call<'a> {
    pub cookie: Option<&'a str>,
    pub csrf: Option<&'a str>,
    pub origin: Option<Option<&'a str>>, // None = default origin; Some(None) = omit header; Some(Some(x)) = custom
    pub ip: Option<&'a str>,
}

pub struct TestApp {
    pub router: Router,
    pub mailer: Mailer,
    pub db: TestDb,
}

impl TestApp {
    pub async fn new(app_env: &str) -> Self {
        let db = TestDb::create(5).await;
        let redis = Arc::new(
            redis::Client::open(
                std::env::var("TEST_REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".into()),
            )
            .unwrap(),
        );
        let mailer = Mailer::capture();
        let state = AppState::new(db.app.clone(), redis, test_config(app_env), mailer.clone());
        Self {
            router: router(state),
            mailer,
            db,
        }
    }

    pub async fn call(&self, method: &str, path: &str, body: Option<Value>, c: Call<'_>) -> Resp {
        let mut b = Request::builder()
            .method(method)
            .uri(path)
            .header("content-type", "application/json")
            .header("user-agent", "integration-test/1.0");
        match c.origin {
            None => b = b.header("origin", ORIGIN),
            Some(Some(o)) => b = b.header("origin", o),
            Some(None) => {}
        }
        if let Some(ck) = c.cookie {
            b = b.header("cookie", ck);
        }
        if let Some(t) = c.csrf {
            b = b.header("x-csrf-token", t);
        }
        let mut req = b
            .body(
                body.map(|v| Body::from(v.to_string()))
                    .unwrap_or_else(Body::empty),
            )
            .unwrap();
        let ip: SocketAddr = format!("{}:50000", c.ip.unwrap_or("203.0.113.1"))
            .parse()
            .unwrap();
        req.extensions_mut().insert(ConnectInfo(ip));
        let res = self.router.clone().oneshot(req).await.unwrap();
        let (parts, body) = res.into_parts();
        let text = String::from_utf8_lossy(&body.collect().await.unwrap().to_bytes()).to_string();
        Resp {
            status: parts.status,
            headers: parts.headers,
            json: serde_json::from_str(&text).unwrap_or(Value::Null),
            text,
        }
    }

    pub async fn post(&self, path: &str, body: Value, c: Call<'_>) -> Resp {
        self.call("POST", path, Some(body), c).await
    }
    pub async fn get(&self, path: &str, c: Call<'_>) -> Resp {
        self.call("GET", path, None, c).await
    }

    /// Latest emailed token (from the capture mailer) for an address.
    pub fn token_for(&self, to: &str) -> String {
        let mail = self
            .mailer
            .outbox()
            .into_iter()
            .rev()
            .find(|e| e.to == to)
            .unwrap_or_else(|| panic!("no email for {to}"));
        let after = mail.text.split("token=").nth(1).expect("token in email");
        after.split_whitespace().next().unwrap().to_owned()
    }

    pub async fn signup_verified(&self, email: &str, pw: &str, ip: &str) {
        let r = self
            .post(
                "/v1/auth/register",
                serde_json::json!({"fullName":"Test User","email":email,"password":pw}),
                Call {
                    ip: Some(ip),
                    ..Default::default()
                },
            )
            .await;
        assert_eq!(r.status, StatusCode::ACCEPTED, "{}", r.text);
        let t = self.token_for(email);
        let v = self
            .post(
                "/v1/auth/verify-email",
                serde_json::json!({"token":t}),
                Call {
                    ip: Some(ip),
                    ..Default::default()
                },
            )
            .await;
        assert_eq!(v.status, StatusCode::OK, "{}", v.text);
    }

    pub async fn login(&self, email: &str, pw: &str, ip: &str) -> Login {
        let r = self
            .post(
                "/v1/auth/login",
                serde_json::json!({"email":email,"password":pw}),
                Call {
                    ip: Some(ip),
                    ..Default::default()
                },
            )
            .await;
        assert_eq!(r.status, StatusCode::OK, "login failed: {}", r.text);
        Login::from(&r)
    }
}

pub struct Login {
    pub cookie: String,
    pub csrf: String,
}
impl Login {
    pub fn from(r: &Resp) -> Self {
        let sc = r
            .headers
            .get("set-cookie")
            .expect("set-cookie")
            .to_str()
            .unwrap();
        Login {
            cookie: sc.split(';').next().unwrap().to_owned(),
            csrf: r.json["csrfToken"].as_str().unwrap().to_owned(),
        }
    }
    pub fn call(&self) -> Call<'_> {
        Call {
            cookie: Some(&self.cookie),
            csrf: Some(&self.csrf),
            ..Default::default()
        }
    }
}

pub fn uniq(prefix: &str) -> String {
    format!(
        "{prefix}-{}@example.test",
        &Uuid::new_v4().simple().to_string()[..12]
    )
}
pub fn rand_ip() -> String {
    let b = Uuid::new_v4().into_bytes();
    format!("10.{}.{}.{}", b[0], b[1], b[2].max(1))
}
pub const PW: &str = "correct horse battery staple";
