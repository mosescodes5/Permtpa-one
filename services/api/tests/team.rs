//! Team management over HTTP against real PostgreSQL + Redis.
mod common;
use axum::http::StatusCode;
use common::*;
use serde_json::json;

struct Org { id: String, owner: Login, owner_email: String, ip: String }

async fn new_org(app: &TestApp, name: &str) -> Org {
    let (email, ip) = (uniq("owner"), rand_ip());
    app.signup_verified(&email, PW, &ip).await;
    let owner = app.login(&email, PW, &ip).await;
    let r = app.post("/v1/organizations", json!({"name":name,"businessType":"retail"}), owner.call()).await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.text);
    Org { id: r.json["id"].as_str().unwrap().to_owned(), owner, owner_email: email, ip }
}

/// A verified user logged in; optionally added to `org` directly through the superuser fixture connection.
async fn member(app: &TestApp, org: &Org, role: Option<&str>) -> (Login, String, String) {
    let email = uniq("member");
    app.signup_verified(&email, PW, &org.ip).await;
    let login = app.login(&email, PW, &org.ip).await;
    let uid: String = sqlx::query_scalar("select id::text from users where email = $1::citext").bind(&email).fetch_one(&app.db.fixture).await.unwrap();
    if let Some(r) = role {
        sqlx::query("insert into memberships (org_id, user_id, role) values ($1::uuid, $2::uuid, $3::org_role)").bind(&org.id).bind(&uid).bind(r).execute(&app.db.fixture).await.unwrap();
    }
    (login, uid, email)
}

fn invite_token(app: &TestApp, email: &str) -> String { app.token_for(email) }

#[tokio::test]
#[ignore = "requires PostgreSQL + Redis"]
async fn invite_and_accept_flow() {
    let app = TestApp::new("development").await;
    let org = new_org(&app, "Okafor Trading").await;
    let (x, _, x_email) = member(&app, &org, None).await;

    let inv = app.post(&format!("/v1/orgs/{}/invitations", org.id), json!({"email":x_email,"role":"manager"}), org.owner.call()).await;
    assert_eq!(inv.status, StatusCode::CREATED, "{}", inv.text);
    assert!(!inv.text.contains("token"), "the API never returns the invitation token");
    let mail = app.mailer.outbox().into_iter().rev().find(|e| e.to == x_email).unwrap();
    assert!(mail.text.contains("Okafor Trading") && mail.text.contains("manager") && mail.text.contains("/accept-invite?token="));

    let pending = app.get(&format!("/v1/orgs/{}/invitations", org.id), org.owner.call()).await;
    assert_eq!(pending.json["invitations"].as_array().unwrap().len(), 1);

    let token = invite_token(&app, &x_email);
    let ok = app.post("/v1/invitations/accept", json!({"token":token}), x.call()).await;
    assert_eq!(ok.status, StatusCode::OK, "{}", ok.text);
    assert_eq!(ok.json["orgId"], org.id.as_str());
    let me = app.get("/v1/me", x.call()).await;
    assert_eq!(me.json["memberships"][0]["role"], "manager");
    assert_eq!(app.post("/v1/invitations/accept", json!({"token":token}), x.call()).await.status, StatusCode::BAD_REQUEST, "single-use");
    assert_eq!(app.get(&format!("/v1/orgs/{}/invitations", org.id), org.owner.call()).await.json["invitations"].as_array().unwrap().len(), 0);
    assert_eq!(app.post("/v1/invitations/accept", json!({"token":token}), Call::default()).await.status, StatusCode::UNAUTHORIZED);
    app.db.drop().await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL + Redis"]
async fn invitation_is_bound_to_the_invited_email_and_hashed_at_rest() {
    let app = TestApp::new("development").await;
    let org = new_org(&app, "Bound Co").await;
    let (x, _, x_email) = member(&app, &org, None).await;
    let (y, _, _) = member(&app, &org, None).await;
    app.post(&format!("/v1/orgs/{}/invitations", org.id), json!({"email":x_email,"role":"sales_staff"}), org.owner.call()).await;
    let token = invite_token(&app, &x_email);

    let stolen = app.post("/v1/invitations/accept", json!({"token":token}), y.call()).await;
    assert_eq!((stolen.status, stolen.json["code"].as_str()), (StatusCode::BAD_REQUEST, Some("invalid_token")), "a forwarded link works for nobody else");
    assert_eq!(app.get("/v1/me", y.call()).await.json["memberships"].as_array().unwrap().len(), 0);

    let stored: Vec<Vec<u8>> = sqlx::query_scalar("select token_hash from invitations").fetch_all(&app.db.fixture).await.unwrap();
    assert!(stored.iter().all(|h| h.len() == 32 && h != token.as_bytes()), "only a hash of the token is stored");
    assert_eq!(app.post("/v1/invitations/accept", json!({"token":token}), x.call()).await.status, StatusCode::OK);
    app.db.drop().await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL + Redis"]
async fn invitation_validation_reinvite_expiry_and_revocation() {
    let app = TestApp::new("development").await;
    let org = new_org(&app, "Rules Co").await;
    let p = format!("/v1/orgs/{}/invitations", org.id);
    let (x, _, x_email) = member(&app, &org, None).await;

    for (body, why) in [
        (json!({"email":x_email,"role":"owner"}), "owner role"),
        (json!({"email":x_email,"role":"superuser"}), "unknown role"),
        (json!({"email":"not-an-email","role":"manager"}), "bad email"),
        (json!({"email":x_email,"role":"manager","orgId":"x"}), "unknown field"),
    ] {
        assert_eq!(app.post(&p, body, org.owner.call()).await.status, StatusCode::BAD_REQUEST, "{why}");
    }
    let existing = app.post(&p, json!({"email":org.owner_email,"role":"manager"}), org.owner.call()).await;
    assert_eq!((existing.status, existing.json["code"].as_str()), (StatusCode::CONFLICT, Some("already_member")));

    // re-inviting replaces the older link
    app.post(&p, json!({"email":x_email,"role":"manager"}), org.owner.call()).await;
    let old = invite_token(&app, &x_email);
    app.post(&p, json!({"email":x_email,"role":"read_only"}), org.owner.call()).await;
    let new = invite_token(&app, &x_email);
    assert_ne!(old, new);
    assert_eq!(app.post("/v1/invitations/accept", json!({"token":old}), x.call()).await.status, StatusCode::BAD_REQUEST, "older link is dead");
    assert_eq!(app.get(&p, org.owner.call()).await.json["invitations"].as_array().unwrap().len(), 1);

    // expiry
    sqlx::query("update invitations set expires_at = now() - interval '1 minute' where revoked_at is null").execute(&app.db.fixture).await.unwrap();
    assert_eq!(app.post("/v1/invitations/accept", json!({"token":new}), x.call()).await.status, StatusCode::BAD_REQUEST, "expired");
    assert_eq!(app.get(&p, org.owner.call()).await.json["invitations"].as_array().unwrap().len(), 0, "expired invitations are not listed");

    // revocation
    let again = app.post(&p, json!({"email":x_email,"role":"accountant"}), org.owner.call()).await;
    let id = again.json["id"].as_str().unwrap().to_owned();
    let token = invite_token(&app, &x_email);
    assert_eq!(app.call("DELETE", &format!("{p}/{id}"), None, org.owner.call()).await.status, StatusCode::NO_CONTENT);
    assert_eq!(app.call("DELETE", &format!("{p}/{id}"), None, org.owner.call()).await.status, StatusCode::NOT_FOUND);
    assert_eq!(app.post("/v1/invitations/accept", json!({"token":token}), x.call()).await.status, StatusCode::BAD_REQUEST, "revoked");
    app.db.drop().await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL + Redis"]
async fn only_owners_and_administrators_manage_the_team() {
    let app = TestApp::new("development").await;
    let org = new_org(&app, "Roles Co").await;
    let (_, victim, _) = member(&app, &org, Some("sales_staff")).await;
    for role in ["manager", "accountant", "inventory_manager", "sales_staff", "read_only"] {
        let (l, _, _) = member(&app, &org, Some(role)).await;
        let o = format!("/v1/orgs/{}", org.id);
        assert_eq!(app.get(&o, l.call()).await.status, StatusCode::OK, "{role} can view the org");
        assert_eq!(app.post(&format!("{o}/invitations"), json!({"email":"new@example.test","role":"read_only"}), l.call()).await.status, StatusCode::FORBIDDEN, "{role} invite");
        assert_eq!(app.get(&format!("{o}/invitations"), l.call()).await.status, StatusCode::FORBIDDEN, "{role} list invites");
        assert_eq!(app.call("PATCH", &format!("{o}/members/{victim}"), Some(json!({"role":"manager"})), l.call()).await.status, StatusCode::FORBIDDEN, "{role} change role");
        assert_eq!(app.call("DELETE", &format!("{o}/members/{victim}"), None, l.call()).await.status, StatusCode::FORBIDDEN, "{role} remove");
        assert_eq!(app.call("PATCH", &o, Some(json!({"name":"Hijacked Co"})), l.call()).await.status, StatusCode::FORBIDDEN, "{role} rename org");
    }
    // administrators: may manage lower roles, never owners
    let (admin, admin_id, _) = member(&app, &org, Some("administrator")).await;
    let o = format!("/v1/orgs/{}", org.id);
    assert_eq!(app.post(&format!("{o}/invitations"), json!({"email":"newhire@example.test","role":"manager"}), admin.call()).await.status, StatusCode::CREATED);
    assert_eq!(app.call("PATCH", &format!("{o}/members/{victim}"), Some(json!({"role":"manager"})), admin.call()).await.status, StatusCode::OK);
    let owner_id: String = sqlx::query_scalar("select id::text from users where email = $1::citext").bind(&org.owner_email).fetch_one(&app.db.fixture).await.unwrap();
    assert_eq!(app.call("PATCH", &format!("{o}/members/{admin_id}"), Some(json!({"role":"owner"})), admin.call()).await.status, StatusCode::FORBIDDEN, "admin cannot self-promote to owner");
    assert_eq!(app.call("PATCH", &format!("{o}/members/{victim}"), Some(json!({"role":"owner"})), admin.call()).await.status, StatusCode::FORBIDDEN, "admin cannot grant ownership");
    assert_eq!(app.call("PATCH", &format!("{o}/members/{owner_id}"), Some(json!({"role":"read_only"})), admin.call()).await.status, StatusCode::FORBIDDEN, "admin cannot demote an owner");
    assert_eq!(app.call("DELETE", &format!("{o}/members/{owner_id}"), None, admin.call()).await.status, StatusCode::FORBIDDEN, "admin cannot remove an owner");
    assert_eq!(app.call("PATCH", &format!("{o}/members/{owner_id}"), Some(json!({"status":"suspended"})), admin.call()).await.status, StatusCode::FORBIDDEN, "admin cannot suspend an owner");
    assert_eq!(app.call("PATCH", &o, Some(json!({"name":"Renamed By Admin"})), admin.call()).await.status, StatusCode::OK);
    app.db.drop().await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL + Redis"]
async fn ownership_transfer_and_last_owner_protection() {
    let app = TestApp::new("development").await;
    let org = new_org(&app, "Owners Co").await;
    let o = format!("/v1/orgs/{}", org.id);
    let owner_id: String = sqlx::query_scalar("select id::text from users where email = $1::citext").bind(&org.owner_email).fetch_one(&app.db.fixture).await.unwrap();
    let (b, b_id, _) = member(&app, &org, Some("administrator")).await;

    let sole_demote = app.call("PATCH", &format!("{o}/members/{owner_id}"), Some(json!({"role":"administrator"})), org.owner.call()).await;
    assert_eq!((sole_demote.status, sole_demote.json["code"].as_str()), (StatusCode::CONFLICT, Some("last_owner")));
    assert_eq!(app.call("DELETE", &format!("{o}/members/{owner_id}"), None, org.owner.call()).await.json["code"], "last_owner");
    assert_eq!(app.call("PATCH", &format!("{o}/members/{owner_id}"), Some(json!({"status":"suspended"})), org.owner.call()).await.json["code"], "last_owner");

    // an owner may promote another owner; then the first can step down
    assert_eq!(app.call("PATCH", &format!("{o}/members/{b_id}"), Some(json!({"role":"owner"})), org.owner.call()).await.status, StatusCode::OK);
    assert_eq!(app.call("PATCH", &format!("{o}/members/{owner_id}"), Some(json!({"role":"administrator"})), org.owner.call()).await.status, StatusCode::OK);
    assert_eq!(app.get(&o, b.call()).await.json["yourRole"], "owner");
    assert_eq!(app.call("DELETE", &format!("{o}/members/{b_id}"), None, b.call()).await.json["code"], "last_owner", "the new sole owner is protected too");
    app.db.drop().await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL + Redis"]
async fn cross_tenant_team_attacks_all_return_404() {
    let app = TestApp::new("development").await;
    let a = new_org(&app, "Org A").await;
    let b = new_org(&app, "Org B").await;
    let (_, a_member, a_member_email) = member(&app, &a, Some("sales_staff")).await;
    let inv = app.post(&format!("/v1/orgs/{}/invitations", a.id), json!({"email":a_member_email.replace("member","pending"),"role":"read_only"}), a.owner.call()).await;
    let inv_id = inv.json["id"].as_str().unwrap().to_owned();
    let ao = format!("/v1/orgs/{}", a.id);

    for (m, path, body) in [
        ("GET", ao.clone(), None),
        ("PATCH", ao.clone(), Some(json!({"name":"Pwned Inc"}))),
        ("GET", format!("{ao}/members"), None),
        ("PATCH", format!("{ao}/members/{a_member}"), Some(json!({"role":"administrator"}))),
        ("DELETE", format!("{ao}/members/{a_member}"), None),
        ("GET", format!("{ao}/invitations"), None),
        ("POST", format!("{ao}/invitations"), Some(json!({"email":"evil@example.test","role":"administrator"}))),
        ("DELETE", format!("{ao}/invitations/{inv_id}"), None),
        ("DELETE", format!("/v1/orgs/{}/invitations/{inv_id}", b.id), None), // right id, wrong org
    ] {
        let r = app.call(m, &path, body, b.owner.call()).await;
        assert_eq!(r.status, StatusCode::NOT_FOUND, "{m} {path} must be 404 for another tenant's owner");
    }
    // nothing changed
    assert_eq!(app.get(&ao, a.owner.call()).await.json["name"], "Org A");
    assert_eq!(app.get(&format!("{ao}/members"), a.owner.call()).await.json["members"].as_array().unwrap().len(), 2);
    assert_eq!(app.get(&format!("{ao}/invitations"), a.owner.call()).await.json["invitations"].as_array().unwrap().len(), 1);
    app.db.drop().await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL + Redis"]
async fn suspended_members_lose_access_until_reinstated() {
    let app = TestApp::new("development").await;
    let org = new_org(&app, "Suspend Co").await;
    let o = format!("/v1/orgs/{}", org.id);
    let (m, m_id, _) = member(&app, &org, Some("manager")).await;
    assert_eq!(app.get(&o, m.call()).await.status, StatusCode::OK);
    assert_eq!(app.call("PATCH", &format!("{o}/members/{m_id}"), Some(json!({"status":"suspended"})), org.owner.call()).await.json["status"], "suspended");
    assert_eq!(app.get(&o, m.call()).await.status, StatusCode::NOT_FOUND, "suspended members are treated as non-members");
    assert_eq!(app.get("/v1/me", m.call()).await.json["memberships"].as_array().unwrap().len(), 0);
    app.call("PATCH", &format!("{o}/members/{m_id}"), Some(json!({"status":"active"})), org.owner.call()).await;
    assert_eq!(app.get(&o, m.call()).await.status, StatusCode::OK);
    app.db.drop().await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL + Redis"]
async fn mass_assignment_is_rejected_everywhere() {
    let app = TestApp::new("development").await;
    let org = new_org(&app, "Mass Co").await;
    let o = format!("/v1/orgs/{}", org.id);
    let (_, m_id, _) = member(&app, &org, Some("manager")).await;
    for (method, path, body, why) in [
        ("PATCH", format!("{o}/members/{m_id}"), json!({"role":"manager","orgId":"x"}), "member + orgId"),
        ("PATCH", format!("{o}/members/{m_id}"), json!({"role":"manager","userId":"x"}), "member + userId"),
        ("PATCH", o.clone(), json!({"name":"Fine Name","slug":"stolen"}), "org + slug"),
        ("PATCH", o.clone(), json!({"name":"Fine Name","currency":"USD"}), "org + currency"),
        ("PATCH", o.clone(), json!({"createdBy":"x"}), "org + createdBy"),
        ("PATCH", format!("{o}/members/{m_id}"), json!({}), "empty member patch"),
        ("PATCH", format!("{o}/members/{m_id}"), json!({"status":"deleted"}), "bad status"),
    ] {
        assert_eq!(app.call(method, &path, Some(body), org.owner.call()).await.status, StatusCode::BAD_REQUEST, "{why}");
    }
    app.db.drop().await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL + Redis"]
async fn team_changes_are_audited_with_the_real_actor() {
    let app = TestApp::new("development").await;
    let org = new_org(&app, "Audit Co").await;
    let o = format!("/v1/orgs/{}", org.id);
    let owner_id: String = sqlx::query_scalar("select id::text from users where email = $1::citext").bind(&org.owner_email).fetch_one(&app.db.fixture).await.unwrap();
    let (x, x_id, x_email) = member(&app, &org, None).await;
    app.post(&format!("{o}/invitations"), json!({"email":x_email,"role":"manager"}), org.owner.call()).await;
    app.post("/v1/invitations/accept", json!({"token":invite_token(&app, &x_email)}), x.call()).await;
    app.call("PATCH", &format!("{o}/members/{x_id}"), Some(json!({"role":"accountant"})), org.owner.call()).await;
    app.call("PATCH", &o, Some(json!({"name":"Audit Co Renamed"})), org.owner.call()).await;
    app.call("DELETE", &format!("{o}/members/{x_id}"), None, org.owner.call()).await;

    let rows: Vec<(String, Option<String>)> = sqlx::query_as("select action, actor_user_id::text from audit_logs where org_id = $1::uuid order by id").bind(&org.id).fetch_all(&app.db.fixture).await.unwrap();
    let find = |a: &str| rows.iter().filter(|(x, _)| x == a).collect::<Vec<_>>();
    for action in ["org.create", "invitation.create", "invitation.accepted", "member.role_change", "org.update", "member.remove"] {
        assert!(!find(action).is_empty(), "missing {action}: {rows:?}");
    }
    assert!(find("member.role_change").iter().all(|(_, a)| a.as_deref() == Some(owner_id.as_str())), "role change attributed to the owner who made it");
    assert!(find("invitation.accepted").iter().all(|(_, a)| a.as_deref() == Some(x_id.as_str())), "acceptance attributed to the invitee");
    let leaked: i64 = sqlx::query_scalar("select count(*) from audit_logs where metadata::text ilike '%token%'").fetch_one(&app.db.fixture).await.unwrap();
    assert_eq!(leaked, 0, "no tokens in audit metadata");
    app.db.drop().await;
}
