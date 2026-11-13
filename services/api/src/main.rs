use permtpa_api::{app, config::{Config, Environment}, db, mailer::Mailer};
use std::net::SocketAddr;
use std::sync::Arc;
use tracing_subscriber::{EnvFilter, fmt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = Config::from_env()?;
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,tower_http=info"));
    if cfg.environment == Environment::Development {
        fmt().with_env_filter(filter).init();
    } else {
        fmt().json().with_env_filter(filter).init();
    }
    tracing::info!(?cfg, "starting PERMTPA ONE API");

    // `permtpa-api migrate` applies migrations using the dedicated migrator role, then exits.
    if std::env::args().nth(1).as_deref() == Some("migrate") {
        let url = cfg.migrator_database_url.as_deref().ok_or("MIGRATOR_DATABASE_URL is required for `migrate`")?;
        let pool = db::connect(url, 1).await?;
        db::MIGRATOR.run(&pool).await?;
        tracing::info!("migrations applied");
        return Ok(());
    }

    let pool = db::connect(&cfg.database_url, 10).await?;
    let redis = Arc::new(redis::Client::open(cfg.redis_url.as_str())?);
    // SMTP when configured; otherwise development captures mail in memory (readable at /v1/dev/outbox) and every
    // other environment refuses to send rather than pretending to.
    let mailer = match (&cfg.smtp_url, cfg.environment) {
        (Some(url), _) => Mailer::smtp(url, &cfg.mail_from)?,
        (None, Environment::Development) => Mailer::capture(),
        (None, _) => Mailer::Disabled,
    };
    if matches!(mailer, Mailer::Disabled) {
        tracing::warn!("SMTP_URL is not set: verification and reset emails will NOT be delivered");
    }
    let state = app::AppState::new(pool, redis, cfg.clone(), mailer);
    let app = app::router(state);

    let listener = tokio::net::TcpListener::bind(cfg.bind).await?;
    tracing::info!(addr = %cfg.bind, "listening");
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).with_graceful_shutdown(shutdown_signal()).await?;
    tracing::info!("shutdown complete");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async { let _ = tokio::signal::ctrl_c().await; };
    #[cfg(unix)]
    let term = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! { _ = ctrl_c => {}, _ = term => {} }
    tracing::info!("shutdown signal received");
}
