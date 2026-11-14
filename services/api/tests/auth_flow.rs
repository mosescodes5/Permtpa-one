//! HTTP-level authentication, session, CSRF, MFA and tenancy tests against real PostgreSQL + Redis.
//!   TEST_ADMIN_DATABASE_URL=postgres://postgres:pw@127.0.0.1:5432/postgres cargo test -p permtpa-api --test auth_flow -- --ignored
mod common;
use axum::http::StatusCode;
use common::*;
use permtpa_api::security;
use serde_json::json;

fn b32_decode(s: &str) -> Vec<u8> {
    let (mut out, mut buf, mut bits) = (vec![], 0u32, 0u32);
    for c in s.bytes() {
        let v = match c { b'A'..=b'Z' => c - b'A', b'2'..=b'7' => c - b'2' + 26, _ => panic!("bad base32") } as u32;
        buf = (buf << 5) | v; bits += 5;
        if bits >= 8 { out.push((buf >> (bits - 8)) as u8); bits -= 8; }
    }
    out
}
fn now() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() }

#[tokio::test]
#[ignore = "requires PostgreSQL + Redis"]
async fn register_verify_login_me_logout() {
    let app = TestApp::new("development").await;
    let (email, ip) = (uniq("ada"), rand_ip());
    let r = app.post("/v1/auth/register", json!({"fullName":"Ada Okafor","email":email,"password":PW}), Call { ip: Some(&ip), ..Default::default() }).await;
    assert_eq!(r.status, StatusCode::ACCEPTED);
    let mail = app.mailer.outbox().pop().unwrap();
    assert_eq!(mail.to, email);
    assert!(mail.text.contains("/verify-email?token="));

    let early = app.post("/v1/auth/login", json!({"email":email,"password":PW}), Call { ip: Some(&ip), ..Default::default() }).await;
    assert_eq!((early.status, early.json["code"].as_str()), (StatusCode::FORBIDDEN, Some("email_not_verified")), "unverified accounts cannot log in");

    let t = app.token_for(&email);
    assert_eq!(app.post("/v1/auth/verify-email", json!({"token":t}), Call { ip: Some(&ip), ..Default::default() }).await.status, StatusCode::OK);
    assert_eq!(app.post("/v1/auth/verify-email", json!({"token":t}), Call { ip: Some(&ip), ..Default::default() }).await.status, StatusCode::BAD_REQUEST, "verification link is single-use");

    let r = app.post("/v1/auth/login", json!({"email":email,"password":PW}), Call { ip: Some(&ip), ..Default::default() }).await;
    assert_eq!(r.status, StatusCode::OK);
    let cookie_hdr = r.headers.get("set-cookie").unwrap().to_str().unwrap().to_owned();
    assert!(cookie_hdr.contains("HttpOnly") && cookie_hdr.contains("SameSite=Lax") && cookie_hdr.contains("Path=/"));
    assert!(!r.text.contains("passwordHash") && !r.text.contains("password_hash"));
    let s = Login::from(&r);

    let me = app.get("/v1/me", s.call()).await;
    assert_eq!(me.status, StatusCode::OK);
    assert_eq!(me.json["user"]["email"], email);
    assert_eq!(me.json["user"]["emailVerified"], true);

    assert_eq!(app.post("/v1/auth/logout", json!({}), s.call()).await.status, StatusCode::NO_CONTENT);
    assert_eq!(app.get("/v1/me", s.call()).await.status, StatusCode::UNAUTHORIZED, "session is revoked server-side after logout");
    app.db.drop().await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL + Redis"]
async fn responses_do_not_reveal_whether_an_account_exists() {
    let app = TestApp::new("development").await;
    let (known, unknown, ip) = (uniq("known"), uniq("ghost"), rand_ip());
    app.signup_verified(&known, PW, &ip).await;
    let c = || Call { ip: Some(&ip), ..Default::default() };

    let a = app.post("/v1/auth/login", json!({"email":known,"password":"wrong-password-123"}), c()).await;
    let b = app.post("/v1/auth/login", json!({"email":unknown,"password":"wrong-password-123"}), c()).await;
    assert_eq!((a.status, &a.text), (b.status, &b.text), "wrong password vs unknown email must be indistinguishable");

    let fresh = uniq("fresh");
    let r1 = app.post("/v1/auth/register", json!({"fullName":"New Person","email":fresh,"password":PW}), c()).await;
    let r2 = app.post("/v1/auth/register", json!({"fullName":"New Person","email":known,"password":PW}), c()).await;
    assert_eq!((r1.status, &r1.text), (r2.status, &r2.text), "registering a taken email looks identical");
    assert!(app.mailer.outbox().iter().any(|e| e.to == known && e.subject.contains("already")), "the real owner is notified instead");

    let f1 = app.post("/v1/auth/forgot-password", json!({"email":known}), c()).await;
    let f2 = app.post("/v1/auth/forgot-password", json!({"email":unknown}), c()).await;
    assert_eq!((f1.status, &f1.text), (f2.status, &f2.text));
    assert!(!app.mailer.outbox().iter().any(|e| e.to == unknown), "no mail for unknown addresses");
    app.db.drop().await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL + Redis"]
async fn input_validation_and_mass_assignment() {
    let app = TestApp::new("development").await;
    let ip = rand_ip();
    let c = || Call { ip: Some(&ip), ..Default::default() };
    for (body, why) in [
        (json!({"fullName":"A B","email":"nope","password":PW}), "bad email"),
        (json!({"fullName":"A B","email":uniq("x"),"password":"short"}), "short password"),
        (json!({"fullName":"A B","email":uniq("x"),"password":PW,"role":"owner"}), "unknown field: role"),
        (json!({"fullName":"A B","email":uniq("x"),"password":PW,"organizationId":"00000000-0000-0000-0000-000000000000"}), "unknown field: organizationId"),
        (json!({"fullName":"A B","email":uniq("x"),"password":PW,"emailVerified":true}), "unknown field: emailVerified"),
    ] {
        assert_eq!(app.post("/v1/auth/register", body, c()).await.status, StatusCode::BAD_REQUEST, "{why}");
    }
    let r = app.call("POST", "/v1/auth/register", None, c()).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "empty body");
    assert_eq!(r.headers.get("content-type").unwrap(), "application/problem+json");
    app.db.drop().await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL + Redis"]
async fn progressive_lockout_then_reset_unlocks() {
    let app = TestApp::new("development").await;
    let (email, ip) = (uniq("lock"), rand_ip());
    app.signup_verified(&email, PW, &ip).await;
    let c = || Call { ip: Some(&ip), ..Default::default() };
    for _ in 0..5 {
        assert_eq!(app.post("/v1/auth/login", json!({"email":email,"password":"definitely-wrong-1"}), c()).await.status, StatusCode::UNAUTHORIZED);
    }
    let locked = app.post("/v1/auth/login", json!({"email":email,"password":PW}), c()).await;
    assert_eq!((locked.status, locked.json["code"].as_str()), (StatusCode::UNAUTHORIZED, Some("invalid_credentials")), "correct password is refused while locked, with the same generic answer");
    let until: bool = sqlx::query_scalar("select locked_until > now() from users where email = $1::citext").bind(&email).fetch_one(&app.db.fixture).await.unwrap();
    assert!(until);

    app.post("/v1/auth/forgot-password", json!({"email":email}), c()).await;
    let t = app.token_for(&email);
    let new_pw = "a brand new passphrase 42";
    assert_eq!(app.post("/v1/auth/reset-password", json!({"token":t,"newPassword":new_pw}), c()).await.status, StatusCode::OK);
    app.login(&email, new_pw, &ip).await;
    app.db.drop().await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL + Redis"]
async fn password_reset_revokes_sessions_and_tokens_are_single_use() {
    let app = TestApp::new("development").await;
    let (email, ip) = (uniq("reset"), rand_ip());
    app.signup_verified(&email, PW, &ip).await;
    let s = app.login(&email, PW, &ip).await;
    app.post("/v1/auth/forgot-password", json!({"email":email}), Call { ip: Some(&ip), ..Default::default() }).await;
    let t = app.token_for(&email);
    let new_pw = "another strong passphrase 7";
    let ok = app.post("/v1/auth/reset-password", json!({"token":t,"newPassword":new_pw}), Call { ip: Some(&ip), ..Default::default() }).await;
    assert_eq!(ok.status, StatusCode::OK);
    assert_eq!(app.get("/v1/me", s.call()).await.status, StatusCode::UNAUTHORIZED, "existing sessions die on password reset");
    assert_eq!(app.post("/v1/auth/reset-password", json!({"token":t,"newPassword":"yet another passphrase 9"}), Call { ip: Some(&ip), ..Default::default() }).await.status, StatusCode::BAD_REQUEST, "reset link cannot be replayed");
    let old = app.post("/v1/auth/login", json!({"email":email,"password":PW}), Call { ip: Some(&ip), ..Default::default() }).await;
    assert_eq!(old.status, StatusCode::UNAUTHORIZED);
    app.login(&email, new_pw, &ip).await;
    assert_eq!(app.post("/v1/auth/reset-password", json!({"token":"garbage","newPassword":new_pw}), Call { ip: Some(&ip), ..Default::default() }).await.status, StatusCode::BAD_REQUEST);
    app.db.drop().await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL + Redis"]
async fn csrf_protection() {
    let app = TestApp::new("development").await;
    let (email, ip) = (uniq("csrf"), rand_ip());
    app.signup_verified(&email, PW, &ip).await;
    let s = app.login(&email, PW, &ip).await;
    let body = json!({"name":"Okafor Trading","businessType":"retail"});

    let no_origin = app.post("/v1/organizations", body.clone(), Call { cookie: Some(&s.cookie), csrf: Some(&s.csrf), origin: Some(None), ..Default::default() }).await;
    assert_eq!((no_origin.status, no_origin.json["code"].as_str()), (StatusCode::FORBIDDEN, Some("csrf_origin")));
    let evil = app.post("/v1/organizations", body.clone(), Call { cookie: Some(&s.cookie), csrf: Some(&s.csrf), origin: Some(Some("https://evil.example")), ..Default::default() }).await;
    assert_eq!(evil.json["code"], "csrf_origin", "cross-site origin rejected even with a valid token");
    let no_token = app.post("/v1/organizations", body.clone(), Call { cookie: Some(&s.cookie), ..Default::default() }).await;
    assert_eq!(no_token.json["code"], "csrf_token", "cookie alone is not enough");
    let bad_token = app.post("/v1/organizations", body.clone(), Call { cookie: Some(&s.cookie), csrf: Some("AAAA"), ..Default::default() }).await;
    assert_eq!(bad_token.json["code"], "csrf_token");

    let other_email = uniq("csrf2");
    app.signup_verified(&other_email, PW, &ip).await;
    let other = app.login(&other_email, PW, &ip).await;
    let borrowed = app.post("/v1/organizations", body.clone(), Call { cookie: Some(&s.cookie), csrf: Some(&other.csrf), ..Default::default() }).await;
    assert_eq!(borrowed.json["code"], "csrf_token", "a token from another session is useless");

    let ok = app.post("/v1/organizations", body, s.call()).await;
    assert_eq!(ok.status, StatusCode::CREATED, "{}", ok.text);
    assert_eq!(ok.json["role"], "owner");
    assert_eq!(app.get("/v1/auth/csrf", s.call()).await.json["csrfToken"], s.csrf.as_str());
    app.db.drop().await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL + Redis"]
async fn sessions_list_revoke_and_rotation() {
    let app = TestApp::new("development").await;
    let (email, ip) = (uniq("sess"), rand_ip());
    app.signup_verified(&email, PW, &ip).await;
    let a = app.login(&email, PW, &ip).await;
    let b = app.login(&email, PW, &ip).await;
    let list = app.get("/v1/auth/sessions", a.call()).await;
    assert_eq!(list.json["sessions"].as_array().unwrap().len(), 2);
    assert_eq!(list.json["sessions"].as_array().unwrap().iter().filter(|s| s["current"] == true).count(), 1);
    assert!(list.json["sessions"][0]["userAgent"].as_str().unwrap().contains("integration-test"));

    // Session fixation defence: logging in WITH a cookie revokes that old session.
    let r = app.post("/v1/auth/login", json!({"email":email,"password":PW}), Call { cookie: Some(&a.cookie), ip: Some(&ip), ..Default::default() }).await;
    let c = Login::from(&r);
    assert_ne!(c.cookie, a.cookie, "new token on every login");
    assert_eq!(app.get("/v1/me", a.call()).await.status, StatusCode::UNAUTHORIZED, "presented session was rotated out");

    let bid = app.get("/v1/auth/sessions", c.call()).await.json["sessions"].as_array().unwrap().iter().find(|s| s["current"] == false).unwrap()["id"].as_str().unwrap().to_owned();
    assert_eq!(app.call("DELETE", &format!("/v1/auth/sessions/{bid}"), None, c.call()).await.status, StatusCode::NO_CONTENT);
    assert_eq!(app.get("/v1/me", b.call()).await.status, StatusCode::UNAUTHORIZED, "revoked session stops working immediately");
    assert_eq!(app.call("DELETE", &format!("/v1/auth/sessions/{bid}"), None, c.call()).await.status, StatusCode::NOT_FOUND);

    // Another user cannot revoke or even see my session.
    let (mail2, _) = (uniq("sess2"), 0);
    app.signup_verified(&mail2, PW, &ip).await;
    let intruder = app.login(&mail2, PW, &ip).await;
    let my_id = app.get("/v1/auth/sessions", c.call()).await.json["sessions"][0]["id"].as_str().unwrap().to_owned();
    assert_eq!(app.call("DELETE", &format!("/v1/auth/sessions/{my_id}"), None, intruder.call()).await.status, StatusCode::NOT_FOUND);
    assert_eq!(app.get("/v1/me", c.call()).await.status, StatusCode::OK);

    let d = app.login(&email, PW, &ip).await; // c is revoked by this login? no cookie sent -> c survives
    let n = app.post("/v1/auth/sessions/revoke-others", json!({}), d.call()).await;
    assert!(n.json["revoked"].as_u64().unwrap() >= 1);
    assert_eq!(app.get("/v1/me", c.call()).await.status, StatusCode::UNAUTHORIZED);
    assert_eq!(app.get("/v1/me", d.call()).await.status, StatusCode::OK);
    app.db.drop().await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL + Redis"]
async fn idle_sessions_expire() {
    let app = TestApp::new("development").await;
    let (email, ip) = (uniq("idle"), rand_ip());
    app.signup_verified(&email, PW, &ip).await;
    let s = app.login(&email, PW, &ip).await;
    sqlx::query("update sessions set last_seen_at = now() - interval '8 days'").execute(&app.db.fixture).await.unwrap();
    assert_eq!(app.get("/v1/me", s.call()).await.status, StatusCode::UNAUTHORIZED);
    sqlx::query("update sessions set last_seen_at = now(), expires_at = now() - interval '1 second'").execute(&app.db.fixture).await.unwrap();
    assert_eq!(app.get("/v1/me", s.call()).await.status, StatusCode::UNAUTHORIZED, "absolute expiry also enforced");
    app.db.drop().await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL + Redis"]
async fn login_is_rate_limited_per_ip() {
    let app = TestApp::new("development").await;
    let ip = rand_ip();
    let mut last = StatusCode::OK;
    for i in 0..32 {
        let r = app.post("/v1/auth/login", json!({"email":format!("nobody{i}@example.test"),"password":"whatever-pass-1"}), Call { ip: Some(&ip), ..Default::default() }).await;
        last = r.status;
        if r.status == StatusCode::TOO_MANY_REQUESTS {
            assert!(r.headers.contains_key("retry-after"));
            assert!(i >= 30, "limit should not trip early (tripped at {i})");
            break;
        }
    }
    assert_eq!(last, StatusCode::TOO_MANY_REQUESTS);
    // development-only reset clears the counters
    assert_eq!(app.post("/v1/dev/reset-rate-limits", json!({}), Call::default()).await.status, StatusCode::NO_CONTENT);
    let after = app.post("/v1/auth/login", json!({"email":"nobody@example.test","password":"whatever-pass-1"}), Call { ip: Some(&ip), ..Default::default() }).await;
    assert_eq!(after.status, StatusCode::UNAUTHORIZED, "limit was reset");
    let other_ip = rand_ip();
    let r = app.post("/v1/auth/login", json!({"email":"x@example.test","password":"whatever-pass-1"}), Call { ip: Some(&other_ip), ..Default::default() }).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED, "a different client is unaffected");
    app.db.drop().await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL + Redis"]
async fn cross_tenant_access_is_denied_over_http() {
    let app = TestApp::new("development").await;
    let ip = rand_ip();
    let (a_mail, b_mail, c_mail) = (uniq("owner-a"), uniq("owner-b"), uniq("staff-c"));
    for m in [&a_mail, &b_mail, &c_mail] { app.signup_verified(m, PW, &ip).await; }
    let (a, b, c) = (app.login(&a_mail, PW, &ip).await, app.login(&b_mail, PW, &ip).await, app.login(&c_mail, PW, &ip).await);
    let org_a = app.post("/v1/organizations", json!({"name":"Org A","businessType":"retail"}), a.call()).await.json["id"].as_str().unwrap().to_owned();
    let org_b = app.post("/v1/organizations", json!({"name":"Org B","businessType":"agriculture"}), b.call()).await.json["id"].as_str().unwrap().to_owned();

    // fixture: C joins A as sales staff (invitations arrive in Milestone 3)
    sqlx::query("insert into memberships (org_id, user_id, role) select $1::uuid, id, 'sales_staff' from users where email = $2::citext").bind(&org_a).bind(&c_mail).execute(&app.db.fixture).await.unwrap();

    let own = app.get(&format!("/v1/orgs/{org_a}/members"), a.call()).await;
    assert_eq!(own.status, StatusCode::OK);
    assert_eq!(own.json["members"].as_array().unwrap().len(), 2);
    assert!(own.json["members"].as_array().unwrap().iter().all(|m| m["email"].is_string()), "owner sees emails");

    let idor = app.get(&format!("/v1/orgs/{org_a}/members"), b.call()).await;
    let ghost = app.get("/v1/orgs/00000000-0000-4000-8000-000000000000/members", b.call()).await;
    assert_eq!(idor.status, StatusCode::NOT_FOUND);
    assert_eq!((idor.status, &idor.text), (ghost.status, &ghost.text), "another tenant's org is indistinguishable from a non-existent one");
    assert_eq!(app.get(&format!("/v1/orgs/{org_b}/members"), a.call()).await.status, StatusCode::NOT_FOUND);

    let staff = app.get(&format!("/v1/orgs/{org_a}/members"), c.call()).await;
    assert_eq!(staff.status, StatusCode::OK);
    assert!(staff.json["members"].as_array().unwrap().iter().all(|m| m["email"].is_null()), "sales staff must not see colleagues' emails");
    assert_eq!(staff.json["yourRole"], "sales_staff");

    assert_eq!(app.get(&format!("/v1/orgs/{org_a}/members"), Call::default()).await.status, StatusCode::UNAUTHORIZED);
    assert_eq!(app.post("/v1/organizations", json!({"name":"X Org","businessType":"retail"}), Call::default()).await.status, StatusCode::UNAUTHORIZED);
    let bad = app.post("/v1/organizations", json!({"name":"Bad Type Co","businessType":"cryptozoology"}), a.call()).await;
    assert_eq!(bad.json["code"], "invalid_business_type");
    let smuggle = app.post("/v1/organizations", json!({"name":"Smuggle Co","businessType":"retail","ownerId":"x"}), a.call()).await;
    assert_eq!(smuggle.status, StatusCode::BAD_REQUEST);
    app.db.drop().await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL + Redis"]
async fn mfa_totp_recovery_codes_and_replay_protection() {
    let app = TestApp::new("development").await;
    let (email, ip) = (uniq("mfa"), rand_ip());
    app.signup_verified(&email, PW, &ip).await;
    let s = app.login(&email, PW, &ip).await;

    let setup = app.post("/v1/auth/mfa/setup", json!({}), s.call()).await;
    assert_eq!(setup.status, StatusCode::OK);
    assert!(setup.json["otpauthUri"].as_str().unwrap().starts_with("otpauth://totp/PERMTPA%20ONE:"));
    let secret = b32_decode(setup.json["secret"].as_str().unwrap());
    let code_at = |step_offset: i64| format!("{:06}", security::hotp(&secret, (now() / 30).wrapping_add_signed(step_offset)));

    assert_eq!(app.post("/v1/auth/mfa/enable", json!({"code":"000000"}), s.call()).await.status, StatusCode::BAD_REQUEST, "wrong code");
    let en = app.post("/v1/auth/mfa/enable", json!({"code":code_at(0)}), s.call()).await;
    assert_eq!(en.status, StatusCode::OK, "{}", en.text);
    let codes: Vec<String> = en.json["recoveryCodes"].as_array().unwrap().iter().map(|c| c.as_str().unwrap().to_owned()).collect();
    assert_eq!(codes.len(), 10);
    assert_eq!(app.post("/v1/auth/mfa/setup", json!({}), s.call()).await.status, StatusCode::CONFLICT, "cannot silently re-seed an enabled factor");

    // at rest: secret encrypted, recovery codes hashed
    let enc: Vec<u8> = sqlx::query_scalar("select secret_enc from user_mfa").fetch_one(&app.db.fixture).await.unwrap();
    assert!(!enc.windows(secret.len()).any(|w| w == secret.as_slice()), "TOTP secret must not be stored in plaintext");
    let stored: Vec<Vec<u8>> = sqlx::query_scalar("select code_hash from mfa_recovery_codes").fetch_all(&app.db.fixture).await.unwrap();
    assert!(stored.iter().all(|h| h.len() == 32) && stored.len() == 10);

    // login now requires a second factor
    let none = app.post("/v1/auth/login", json!({"email":email,"password":PW}), Call { ip: Some(&ip), ..Default::default() }).await;
    assert_eq!((none.status, none.json["code"].as_str()), (StatusCode::UNAUTHORIZED, Some("mfa_required")));
    let wrong = app.post("/v1/auth/login", json!({"email":email,"password":PW,"mfaCode":"123456"}), Call { ip: Some(&ip), ..Default::default() }).await;
    assert_eq!(wrong.json["code"], "invalid_mfa_code");
    let replay = app.post("/v1/auth/login", json!({"email":email,"password":PW,"mfaCode":code_at(0)}), Call { ip: Some(&ip), ..Default::default() }).await;
    assert_eq!(replay.json["code"], "invalid_mfa_code", "the step used to enable MFA cannot be replayed");
    let next = code_at(1);
    let good = app.post("/v1/auth/login", json!({"email":email,"password":PW,"mfaCode":next}), Call { ip: Some(&ip), ..Default::default() }).await;
    assert_eq!(good.status, StatusCode::OK, "{}", good.text);
    let again = app.post("/v1/auth/login", json!({"email":email,"password":PW,"mfaCode":next}), Call { ip: Some(&ip), ..Default::default() }).await;
    assert_eq!(again.json["code"], "invalid_mfa_code", "a captured TOTP code cannot be reused");
    assert_eq!(app.post("/v1/auth/login", json!({"email":email,"password":"wrong-password-12","mfaCode":code_at(-1)}), Call { ip: Some(&ip), ..Default::default() }).await.json["code"], "invalid_credentials", "MFA never rescues a wrong password");

    // recovery codes: single-use
    let rc = &codes[0];
    assert_eq!(app.post("/v1/auth/login", json!({"email":email,"password":PW,"mfaCode":rc}), Call { ip: Some(&ip), ..Default::default() }).await.status, StatusCode::OK);
    assert_eq!(app.post("/v1/auth/login", json!({"email":email,"password":PW,"mfaCode":rc}), Call { ip: Some(&ip), ..Default::default() }).await.json["code"], "invalid_mfa_code", "recovery code burns on first use");

    // disabling requires password AND a second factor
    let s2 = Login::from(&good);
    assert_eq!(app.post("/v1/auth/mfa/disable", json!({"password":"wrong-password-12","code":codes[1]}), s2.call()).await.status, StatusCode::UNAUTHORIZED);
    assert_eq!(app.post("/v1/auth/mfa/disable", json!({"password":PW,"code":"999999"}), s2.call()).await.status, StatusCode::UNAUTHORIZED);
    assert_eq!(app.post("/v1/auth/mfa/disable", json!({"password":PW,"code":codes[1]}), s2.call()).await.status, StatusCode::OK);
    app.login(&email, PW, &ip).await;
    app.db.drop().await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL + Redis"]
async fn secrets_at_rest_and_audit_trail() {
    let app = TestApp::new("development").await;
    let (email, ip) = (uniq("rest"), rand_ip());
    app.signup_verified(&email, PW, &ip).await;
    app.post("/v1/auth/login", json!({"email":email,"password":"nope-nope-nope-1"}), Call { ip: Some(&ip), ..Default::default() }).await;
    let s = app.login(&email, PW, &ip).await;
    let raw_token = s.cookie.split_once('=').unwrap().1.to_owned();

    let hash: String = sqlx::query_scalar("select password_hash from users where email = $1::citext").bind(&email).fetch_one(&app.db.fixture).await.unwrap();
    assert!(hash.starts_with("$argon2id$") && !hash.contains(PW));
    let stored: Vec<u8> = sqlx::query_scalar("select token_hash from sessions").fetch_one(&app.db.fixture).await.unwrap();
    assert_ne!(stored, raw_token.as_bytes(), "only a hash of the session token is stored");
    let email_token = app.token_for(&email);
    let leaked: i64 = sqlx::query_scalar("select count(*) from email_tokens where encode(token_hash,'escape') like '%' || $1 || '%'").bind(&email_token).fetch_one(&app.db.fixture).await.unwrap();
    assert_eq!(leaked, 0, "raw email tokens are never stored");

    let actions: Vec<String> = sqlx::query_scalar("select action from audit_logs order by id").fetch_all(&app.db.fixture).await.unwrap();
    for want in ["auth.register", "auth.email_verified", "auth.login_failed", "auth.login"] {
        assert!(actions.iter().any(|a| a == want), "missing audit event {want}: {actions:?}");
    }
    let meta: String = sqlx::query_scalar("select coalesce(string_agg(metadata::text || coalesce(target_id,''),' '),'') from audit_logs").fetch_one(&app.db.fixture).await.unwrap();
    for secret in [PW, "nope-nope-nope-1", &raw_token, &email_token] { assert!(!meta.contains(secret), "audit log leaked a secret"); }
    app.db.drop().await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL + Redis"]
async fn production_mode_hardens_cookies_and_hides_dev_endpoints() {
    let dev = TestApp::new("development").await;
    assert_eq!(dev.get("/v1/dev/outbox", Call::default()).await.status, StatusCode::OK);
    dev.db.drop().await;

    let prod = TestApp::new("production").await;
    assert_eq!(prod.get("/v1/dev/outbox", Call::default()).await.status, StatusCode::NOT_FOUND, "dev outbox must not exist in production");
    assert_eq!(prod.post("/v1/dev/reset-rate-limits", json!({}), Call::default()).await.status, StatusCode::NOT_FOUND, "rate-limit reset must not exist in production");
    let (email, ip) = (uniq("prod"), rand_ip());
    prod.signup_verified(&email, PW, &ip).await;
    let r = prod.post("/v1/auth/login", json!({"email":email,"password":PW}), Call { ip: Some(&ip), ..Default::default() }).await;
    let sc = r.headers.get("set-cookie").unwrap().to_str().unwrap();
    assert!(sc.starts_with("__Host-permtpa_session=") && sc.contains("Secure") && sc.contains("HttpOnly") && sc.contains("SameSite=Lax"), "{sc}");
    prod.db.drop().await;
}
