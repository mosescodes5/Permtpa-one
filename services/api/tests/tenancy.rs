#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Integration tests that exercise tenant isolation through the REAL Rust code path (`begin_tenant_tx`)
//! against a real PostgreSQL. Run with:
//!   TEST_ADMIN_DATABASE_URL=postgres://postgres:pw@127.0.0.1:5432/postgres cargo test -p permtpa-api -- --ignored
mod common;
use common::TestDb;
use permtpa_api::db::{TenantContext, begin_tenant_tx};
use sqlx::{AssertSqlSafe, PgPool};
use uuid::Uuid;

async fn register(pool: &PgPool, email: &str) -> Uuid {
    let mut tx = begin_tenant_tx(pool, TenantContext::default())
        .await
        .unwrap();
    let id: Uuid = sqlx::query_scalar("select app.register_user($1::citext, $2, $3)")
        .bind(email)
        .bind("Test User")
        .bind("$argon2id$stub")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    id
}

async fn create_org(pool: &PgPool, owner: Uuid, slug: &str) -> Uuid {
    let mut tx = begin_tenant_tx(
        pool,
        TenantContext {
            user_id: Some(owner),
            org_id: None,
        },
    )
    .await
    .unwrap();
    let id: Uuid = sqlx::query_scalar("select app.create_organization($1, $2::citext, 'retail')")
        .bind(format!("Org {slug}"))
        .bind(slug)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    id
}

async fn count(pool: &PgPool, ctx: TenantContext, table: &str) -> i64 {
    let mut tx = begin_tenant_tx(pool, ctx).await.unwrap();
    let n: i64 = sqlx::query_scalar(AssertSqlSafe(format!("select count(*) from {table}")))
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    tx.rollback().await.unwrap();
    n
}

#[tokio::test]
#[ignore = "requires PostgreSQL: set TEST_ADMIN_DATABASE_URL"]
async fn tenants_are_isolated_through_the_rust_path() {
    let db = TestDb::create(1).await;
    let (a, b) = (
        register(&db.app, "a@a.test").await,
        register(&db.app, "b@b.test").await,
    );
    let (org_a, org_b) = (
        create_org(&db.app, a, "org-a").await,
        create_org(&db.app, b, "org-b").await,
    );

    let ctx = |u, o| TenantContext {
        user_id: Some(u),
        org_id: Some(o),
    };
    assert_eq!(count(&db.app, ctx(a, org_a), "organizations").await, 1);
    assert_eq!(count(&db.app, ctx(b, org_b), "organizations").await, 1);
    // Forged/mistaken org context: B asks for A's tenant.
    let mut tx = begin_tenant_tx(&db.app, ctx(b, org_a)).await.unwrap();
    let leaked: i64 = sqlx::query_scalar("select count(*) from memberships where org_id = $1")
        .bind(org_a)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(leaked, 0, "a forged org id must reveal nothing");
    let audit_write = sqlx::query(
        "insert into audit_logs (org_id, actor_user_id, action) values ($1, $2, 'evil')",
    )
    .bind(org_a)
    .bind(b)
    .execute(&mut *tx)
    .await;
    assert!(
        audit_write.is_err(),
        "writing into another tenant's audit log must be rejected by RLS"
    );
    drop(tx);
    db.drop().await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL: set TEST_ADMIN_DATABASE_URL"]
async fn request_context_never_leaks_across_pooled_connections() {
    let db = TestDb::create(1).await; // pool of exactly one connection => guaranteed reuse
    let a = register(&db.app, "a@a.test").await;
    let org_a = create_org(&db.app, a, "org-a").await;

    assert_eq!(
        count(
            &db.app,
            TenantContext {
                user_id: Some(a),
                org_id: Some(org_a)
            },
            "organizations"
        )
        .await,
        1
    );
    // Next "request" on the same connection with NO context must see nothing.
    assert_eq!(
        count(&db.app, TenantContext::default(), "organizations").await,
        0
    );
    assert_eq!(
        count(&db.app, TenantContext::default(), "memberships").await,
        0
    );
    // And a committed transaction must not leave the setting behind on the connection.
    let mut conn = db.app.acquire().await.unwrap();
    let leftover: Option<String> =
        sqlx::query_scalar("select current_setting('app.user_id', true)")
            .fetch_one(&mut *conn)
            .await
            .unwrap();
    assert!(
        leftover.unwrap_or_default().is_empty(),
        "app.user_id leaked onto the pooled connection"
    );
    drop(conn);
    db.drop().await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL: set TEST_ADMIN_DATABASE_URL"]
async fn app_role_cannot_bypass_rls_or_read_secrets() {
    let db = TestDb::create(1).await;
    let a = register(&db.app, "a@a.test").await;
    let mut tx = begin_tenant_tx(
        &db.app,
        TenantContext {
            user_id: Some(a),
            org_id: None,
        },
    )
    .await
    .unwrap();
    assert!(
        sqlx::query("select password_hash from users")
            .fetch_all(&mut *tx)
            .await
            .is_err(),
        "password_hash must not be selectable"
    );
    drop(tx);
    let is_super: bool = sqlx::query_scalar(
        "select rolsuper or rolbypassrls from pg_roles where rolname = current_user",
    )
    .fetch_one(&db.app)
    .await
    .unwrap();
    assert!(!is_super, "runtime role must not be superuser or BYPASSRLS");
    db.drop().await;
}
