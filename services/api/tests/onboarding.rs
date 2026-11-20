#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Business-type onboarding and the deletable sample catalogue, over HTTP against real PostgreSQL + Redis.
//! UNVERIFIED until run by CI: `cargo test -p permtpa-api --test onboarding -- --ignored`.
mod common;
use axum::http::StatusCode;
use common::*;
use serde_json::{Value, json};
use std::sync::Arc;

struct Org {
    id: String,
    owner: Login,
}

async fn new_org(app: &TestApp, name: &str, business_type: &str) -> Org {
    let (email, ip) = (uniq("owner"), rand_ip());
    app.signup_verified(&email, PW, &ip).await;
    let owner = app.login(&email, PW, &ip).await;
    let r = app
        .post(
            "/v1/organizations",
            json!({"name":name,"businessType":business_type}),
            owner.call(),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.text);
    Org {
        id: r.json["id"].as_str().unwrap().to_owned(),
        owner,
    }
}

async fn member(app: &TestApp, org: &Org, role: &str) -> Login {
    // each member gets its own address: registrations share a per-IP rate limit
    let (email, ip) = (uniq("member"), rand_ip());
    app.signup_verified(&email, PW, &ip).await;
    let login = app.login(&email, PW, &ip).await;
    sqlx::query("insert into memberships (org_id, user_id, role) select $1::uuid, id, $2::org_role from users where email = $3::citext")
        .bind(&org.id)
        .bind(role)
        .bind(&email)
        .execute(&app.db.fixture)
        .await
        .unwrap();
    login
}

fn samples(org: &Org) -> String {
    format!("/v1/orgs/{}/sample-catalogue", org.id)
}

async fn count(app: &TestApp, sql: &'static str, org: &Org) -> i64 {
    sqlx::query_scalar::<_, i64>(sql)
        .bind(&org.id)
        .fetch_one(&app.db.fixture)
        .await
        .unwrap()
}

async fn list_products(app: &TestApp, org: &Org, who: &Login) -> Vec<Value> {
    let r = app
        .get(
            &format!("/v1/orgs/{}/products?limit=100", org.id),
            who.call(),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text);
    r.json["items"].as_array().unwrap().clone()
}

const SAMPLE_PRODUCTS: &str =
    "select count(*) from products where org_id = $1::uuid and sample_key is not null";

#[tokio::test]
#[ignore = "requires PostgreSQL + Redis"]
async fn business_types_are_listed_with_their_capability_flags() {
    let app = TestApp::new("development").await;
    let org = new_org(&app, "Types Co", "general_trade").await;
    let r = app.get("/v1/business-types", org.owner.call()).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text);
    let items = r.json["items"].as_array().unwrap();
    let codes: Vec<&str> = items.iter().map(|i| i["code"].as_str().unwrap()).collect();
    assert_eq!(
        codes,
        [
            "electronics",
            "fashion",
            "restaurant",
            "pharmacy",
            "salon_spa",
            "supermarket",
            "building_materials",
            "car_wash",
            "services",
            "hospitality",
            "general_trade",
            "other"
        ],
        "exactly the 12 offered types, in display order; legacy codes are hidden"
    );
    let by = |code: &str| items.iter().find(|i| i["code"] == code).unwrap();
    assert_eq!(by("electronics")["label"], "Electronics");
    assert_eq!(
        by("electronics")["description"],
        "Phones, gadgets, accessories"
    );
    assert_eq!(by("hospitality")["label"], "Hotel and hospitality");
    assert_eq!(by("pharmacy")["capabilities"]["tracksExpiry"], true);
    assert_eq!(by("pharmacy")["capabilities"]["soldByWeight"], false);
    assert_eq!(
        by("building_materials")["capabilities"]["soldByWeight"],
        true
    );
    assert_eq!(by("restaurant")["capabilities"]["hasTables"], true);
    assert_eq!(by("services")["capabilities"]["hasBookings"], true);
    assert_eq!(by("electronics")["capabilities"]["tracksExpiry"], false);
    // sign-in is required
    let anon = app.get("/v1/business-types", Call::default()).await;
    assert_eq!(anon.status, StatusCode::UNAUTHORIZED);
    app.db.drop().await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL + Redis"]
async fn capabilities_come_from_the_database_and_follow_the_type() {
    let app = TestApp::new("development").await;
    let org = new_org(&app, "Chemist", "pharmacy").await;
    let p = app
        .get(&format!("/v1/orgs/{}", org.id), org.owner.call())
        .await;
    assert_eq!(p.status, StatusCode::OK, "{}", p.text);
    assert_eq!(p.json["businessType"], "pharmacy");
    assert_eq!(p.json["capabilities"]["tracksExpiry"], true);
    assert_eq!(p.json["capabilities"]["soldByWeight"], false);

    let changed = app
        .call(
            "PATCH",
            &format!("/v1/orgs/{}", org.id),
            Some(json!({"businessType":"supermarket"})),
            org.owner.call(),
        )
        .await;
    assert_eq!(changed.status, StatusCode::OK, "{}", changed.text);
    assert_eq!(changed.json["capabilities"]["tracksExpiry"], false);
    assert_eq!(changed.json["capabilities"]["soldByWeight"], true);

    // the runtime role cannot edit the flags or the type list, and cannot read the cost-bearing sample data
    for sql in [
        "update business_types set tracks_expiry = true where code = 'electronics'",
        "insert into business_types (code, label) values ('crypto_shop', 'Crypto shop')",
        "select * from sample_catalogue_items",
    ] {
        assert!(
            sqlx::query(sql).execute(&app.db.app).await.is_err(),
            "the app role must not be able to run: {sql}"
        );
    }
    app.db.drop().await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL + Redis"]
async fn phone_is_optional_validated_and_owner_or_admin_only() {
    let app = TestApp::new("development").await;
    let org = new_org(&app, "Phone Co", "electronics").await;
    let path = format!("/v1/orgs/{}", org.id);
    let set = app
        .call(
            "PATCH",
            &path,
            Some(json!({"phone":"+234 803 123 4567"})),
            org.owner.call(),
        )
        .await;
    assert_eq!(set.status, StatusCode::OK, "{}", set.text);
    assert_eq!(set.json["phone"], "+234 803 123 4567");
    assert_eq!(set.json["name"], "Phone Co", "other fields are untouched");

    let bad = app
        .call(
            "PATCH",
            &path,
            Some(json!({"phone":"call me maybe"})),
            org.owner.call(),
        )
        .await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);
    assert_eq!(bad.json["code"], "invalid_phone");

    for role in ["manager", "inventory_manager", "accountant", "sales_staff"] {
        let l = member(&app, &org, role).await;
        let r = app
            .call(
                "PATCH",
                &path,
                Some(json!({"phone":"08000000000"})),
                l.call(),
            )
            .await;
        assert_eq!(
            r.status,
            StatusCode::FORBIDDEN,
            "{role} must not edit the profile"
        );
    }
    let got = app.get(&path, org.owner.call()).await;
    assert_eq!(
        got.json["phone"], "+234 803 123 4567",
        "rejected edits changed nothing"
    );

    let cleared = app
        .call("PATCH", &path, Some(json!({"phone":""})), org.owner.call())
        .await;
    assert_eq!(cleared.status, StatusCode::OK, "{}", cleared.text);
    assert!(cleared.json["phone"].is_null(), "an empty phone clears it");
    app.db.drop().await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL + Redis"]
async fn loading_is_idempotent_flagged_audited_and_respects_units() {
    let app = TestApp::new("development").await;
    let org = new_org(&app, "Builders Depot", "building_materials").await;
    let before = app.get(&samples(&org), org.owner.call()).await;
    assert_eq!(before.status, StatusCode::OK, "{}", before.text);
    assert_eq!(before.json["loadable"], 6);
    assert_eq!(before.json["present"], 0);

    let first = app.post(&samples(&org), json!({}), org.owner.call()).await;
    assert_eq!(first.status, StatusCode::OK, "{}", first.text);
    assert_eq!(first.json["loaded"], 6);
    let second = app.post(&samples(&org), json!({}), org.owner.call()).await;
    assert_eq!(second.status, StatusCode::OK, "{}", second.text);
    assert_eq!(second.json["loaded"], 0);
    assert_eq!(second.json["alreadyPresent"], 6);
    assert_eq!(
        count(&app, SAMPLE_PRODUCTS, &org).await,
        6,
        "no duplicates after two loads"
    );
    assert_eq!(
        count(
            &app,
            "select count(*) from variants where org_id = $1::uuid",
            &org
        )
        .await,
        6
    );

    let items = list_products(&app, &org, &org.owner).await;
    assert_eq!(items.len(), 6);
    for p in &items {
        assert_eq!(p["isSample"], true, "every loaded row is flagged");
        let v = &p["variants"][0];
        let fractional = p["allowFractional"].as_bool().unwrap();
        let on_hand = v["onHand"].as_i64().unwrap();
        let reorder = v["reorderLevel"].as_i64().unwrap();
        if !fractional {
            assert_eq!(on_hand % 1000, 0, "{}: whole-unit stock", p["name"]);
            assert_eq!(reorder % 1000, 0, "{}: whole-unit reorder level", p["name"]);
        }
        assert!(v["costMinor"].is_i64(), "the owner sees sample costs");
    }
    let find = |name: &str| items.iter().find(|p| p["name"] == name).unwrap();
    assert_eq!(find("Sharp sand (per tonne)")["allowFractional"], true);
    assert_eq!(
        find("Sharp sand (per tonne)")["variants"][0]["onHand"],
        12_500
    );
    assert_eq!(
        find("Nails 3 inch (per kg)")["variants"][0]["onHand"],
        60_250
    );
    assert_eq!(find("Cement 50kg bag")["allowFractional"], false);
    assert_eq!(find("Cement 50kg bag")["variants"][0]["onHand"], 200_000);

    // a real product is not a sample
    let real = app
        .post(
            &format!("/v1/orgs/{}/products", org.id),
            json!({"name":"My own bolts","variants":[{"sku":"BOLT-1","priceMinor":100}]}),
            org.owner.call(),
        )
        .await;
    assert_eq!(real.status, StatusCode::CREATED, "{}", real.text);
    assert_eq!(real.json["isSample"], false);

    // audit: the real actor, with the number loaded
    let audited: i64 = count(
        &app,
        "select count(*) from audit_logs where org_id = $1::uuid and action = 'catalogue.sample_load' and (metadata->>'loaded')::int = 6",
        &org,
    )
    .await;
    assert_eq!(
        audited, 1,
        "one audit row for the one load that changed anything"
    );
    app.db.drop().await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL + Redis"]
async fn delete_removes_only_untouched_samples_and_keeps_real_stock_history() {
    let app = TestApp::new("development").await;
    let org = new_org(&app, "Phone Hub", "electronics").await;
    let base = format!("/v1/orgs/{}", org.id);
    let real = app
        .post(
            &format!("{base}/products"),
            json!({"name":"Real item","variants":[{"sku":"REAL-1","priceMinor":5000,"costMinor":3000,"openingStock":4000}]}),
            org.owner.call(),
        )
        .await;
    assert_eq!(real.status, StatusCode::CREATED, "{}", real.text);
    let real_id = real.json["id"].as_str().unwrap().to_owned();
    assert_eq!(
        app.post(&samples(&org), json!({}), org.owner.call())
            .await
            .json["loaded"],
        5
    );

    // a REAL purchase on one sample: it now has real stock history
    let items = list_products(&app, &org, &org.owner).await;
    let phone = items
        .iter()
        .find(|p| p["name"] == "Smartphone 64GB")
        .unwrap();
    let phone_variant = phone["variants"][0]["id"].as_str().unwrap().to_owned();
    let bought = app
        .post(
            &format!("{base}/variants/{phone_variant}/stock"),
            json!({"delta":5000,"reason":"purchase"}),
            org.owner.call(),
        )
        .await;
    assert_eq!(bought.status, StatusCode::CREATED, "{}", bought.text);

    let status = app.get(&samples(&org), org.owner.call()).await;
    assert_eq!(status.json["deletable"], 4);
    assert_eq!(status.json["keptWithHistory"], 1);

    let del = app
        .call("DELETE", &samples(&org), None, org.owner.call())
        .await;
    assert_eq!(del.status, StatusCode::OK, "{}", del.text);
    assert_eq!(del.json["deleted"], 4);
    assert_eq!(del.json["kept"].as_array().unwrap().len(), 1);
    assert_eq!(
        del.json["kept"][0]["name"], "Smartphone 64GB",
        "the API says which sample was kept"
    );

    // after: the kept sample and the real product, with exact balances; nothing orphaned
    let after = list_products(&app, &org, &org.owner).await;
    assert_eq!(after.len(), 2);
    let kept = after
        .iter()
        .find(|p| p["name"] == "Smartphone 64GB")
        .unwrap();
    assert_eq!(
        kept["variants"][0]["onHand"], 13_000,
        "8 opening + 5 purchased"
    );
    assert_eq!(kept["isSample"], true);
    let mine = after.iter().find(|p| p["id"] == real_id.as_str()).unwrap();
    assert_eq!(mine["variants"][0]["onHand"], 4_000);
    assert_eq!(mine["variants"][0]["costMinor"], 3_000);
    assert_eq!(mine["isSample"], false);
    assert_eq!(
        count(
            &app,
            "select count(*) from stock_levels where org_id = $1::uuid",
            &org
        )
        .await,
        2
    );
    assert_eq!(
        count(
            &app,
            "select count(*) from stock_movements where org_id = $1::uuid",
            &org
        )
        .await,
        3,
        "real opening + kept sample's opening and purchase"
    );

    // a second delete changes nothing; a reload restores the four and does not duplicate the kept one
    let again = app
        .call("DELETE", &samples(&org), None, org.owner.call())
        .await;
    assert_eq!(again.json["deleted"], 0);
    assert_eq!(again.json["kept"].as_array().unwrap().len(), 1);
    let reload = app.post(&samples(&org), json!({}), org.owner.call()).await;
    assert_eq!(reload.json["loaded"], 4);
    assert_eq!(count(&app, SAMPLE_PRODUCTS, &org).await, 5);

    // mass assignment: nothing sent over HTTP can create or alter the flag
    let forged = app
        .post(
            &format!("{base}/products"),
            json!({"name":"Forged","isSample":true,"variants":[{"sku":"F-1","priceMinor":1}]}),
            org.owner.call(),
        )
        .await;
    assert_eq!(forged.status, StatusCode::BAD_REQUEST);
    let forged_key = app
        .post(
            &format!("{base}/products"),
            json!({"name":"Forged","sampleKey":"x-01","variants":[{"sku":"F-1","priceMinor":1}]}),
            org.owner.call(),
        )
        .await;
    assert_eq!(forged_key.status, StatusCode::BAD_REQUEST);
    let flip = app
        .call(
            "PATCH",
            &format!("{base}/products/{real_id}"),
            Some(json!({"sampleKey":"electronics-01"})),
            org.owner.call(),
        )
        .await;
    assert_eq!(flip.status, StatusCode::BAD_REQUEST);
    let still = app
        .call("DELETE", &samples(&org), None, org.owner.call())
        .await;
    assert_eq!(still.status, StatusCode::OK);
    let survivors = list_products(&app, &org, &org.owner).await;
    assert!(
        survivors.iter().any(|p| p["id"] == real_id.as_str()),
        "the real product survives every delete"
    );
    app.db.drop().await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL + Redis"]
async fn a_sample_that_was_adjusted_or_sold_down_is_also_kept() {
    let app = TestApp::new("development").await;
    let org = new_org(&app, "Fashion Place", "fashion").await;
    let base = format!("/v1/orgs/{}", org.id);
    app.post(&samples(&org), json!({}), org.owner.call()).await;
    let items = list_products(&app, &org, &org.owner).await;
    let v = items[0]["variants"][0]["id"].as_str().unwrap().to_owned();
    let adj = app
        .post(
            &format!("{base}/variants/{v}/stock"),
            json!({"delta":-1000,"reason":"damage","note":"torn"}),
            org.owner.call(),
        )
        .await;
    assert_eq!(adj.status, StatusCode::CREATED, "{}", adj.text);
    let del = app
        .call("DELETE", &samples(&org), None, org.owner.call())
        .await;
    assert_eq!(del.json["deleted"], 4);
    assert_eq!(del.json["kept"].as_array().unwrap().len(), 1);
    app.db.drop().await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL + Redis"]
async fn other_tenants_cannot_load_read_or_delete_samples() {
    let app = TestApp::new("development").await;
    let a = new_org(&app, "Shop A", "electronics").await;
    let b = new_org(&app, "Shop B", "fashion").await;
    assert_eq!(
        app.post(&samples(&a), json!({}), a.owner.call()).await.json["loaded"],
        5
    );
    // B's owner aims every call at A's id
    for (method, body) in [("GET", None), ("POST", Some(json!({}))), ("DELETE", None)] {
        let r = app.call(method, &samples(&a), body, b.owner.call()).await;
        assert_eq!(
            r.status,
            StatusCode::NOT_FOUND,
            "{method} on another tenant"
        );
    }
    assert_eq!(
        count(&app, SAMPLE_PRODUCTS, &a).await,
        5,
        "A's samples are intact"
    );
    // B's own load only touches B
    assert_eq!(
        app.post(&samples(&b), json!({}), b.owner.call()).await.json["loaded"],
        5
    );
    assert_eq!(count(&app, SAMPLE_PRODUCTS, &a).await, 5);
    assert_eq!(count(&app, SAMPLE_PRODUCTS, &b).await, 5);
    assert_eq!(
        count(
            &app,
            "select count(*) from products where org_id = $1::uuid and sample_key like 'fashion-%'",
            &a
        )
        .await,
        0,
        "no fashion samples leaked into the electronics shop"
    );
    // B deleting its samples leaves A's alone
    let del = app.call("DELETE", &samples(&b), None, b.owner.call()).await;
    assert_eq!(del.json["deleted"], 5);
    assert_eq!(count(&app, SAMPLE_PRODUCTS, &a).await, 5);
    // signed out
    for method in ["GET", "POST", "DELETE"] {
        let r = app.call(method, &samples(&a), None, Call::default()).await;
        assert_eq!(
            r.status,
            StatusCode::UNAUTHORIZED,
            "{method} without a session"
        );
    }
    app.db.drop().await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL + Redis"]
async fn sample_catalogue_role_matrix() {
    let app = TestApp::new("development").await;
    let org = new_org(&app, "Role Shop", "electronics").await;
    for role in ["sales_staff", "read_only", "accountant"] {
        let l = member(&app, &org, role).await;
        let load = app.post(&samples(&org), json!({}), l.call()).await;
        assert_eq!(
            load.status,
            StatusCode::FORBIDDEN,
            "{role} must not load samples"
        );
        let del = app.call("DELETE", &samples(&org), None, l.call()).await;
        assert_eq!(
            del.status,
            StatusCode::FORBIDDEN,
            "{role} must not delete samples"
        );
        let status = app.get(&samples(&org), l.call()).await;
        assert_eq!(status.status, StatusCode::OK, "{role} may read the counts");
    }
    assert_eq!(
        count(&app, SAMPLE_PRODUCTS, &org).await,
        0,
        "rejected attempts created nothing"
    );

    // allowed roles: each can load all 5 and then delete all 5 (nothing has real stock history)
    for role in ["inventory_manager", "manager", "administrator"] {
        let l = member(&app, &org, role).await;
        let load = app.post(&samples(&org), json!({}), l.call()).await;
        assert_eq!(load.status, StatusCode::OK, "{role}: {}", load.text);
        let del = app.call("DELETE", &samples(&org), None, l.call()).await;
        assert_eq!(del.status, StatusCode::OK, "{role}: {}", del.text);
        assert_eq!(del.json["deleted"], 5, "{role}");
    }
    assert_eq!(
        app.post(&samples(&org), json!({}), org.owner.call())
            .await
            .status,
        StatusCode::OK
    );

    // profit visibility: sales staff and read-only users never receive sample costs
    for role in ["sales_staff", "read_only"] {
        let l = member(&app, &org, role).await;
        let items = list_products(&app, &org, &l).await;
        assert_eq!(items.len(), 5);
        for p in &items {
            assert!(
                p["variants"][0]["costMinor"].is_null(),
                "{role} must not see sample costs"
            );
        }
    }
    app.db.drop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires PostgreSQL + Redis"]
async fn concurrent_loads_never_duplicate() {
    let app = Arc::new(TestApp::new("development").await);
    let org = new_org(&app, "Race Shop", "electronics").await;
    let path = samples(&org);
    let owner = Arc::new(org.owner);
    let mut set = tokio::task::JoinSet::new();
    for _ in 0..8 {
        let (app, path, owner) = (app.clone(), path.clone(), owner.clone());
        set.spawn(async move { app.post(&path, json!({}), owner.call()).await });
    }
    let mut loaded_total = 0;
    while let Some(r) = set.join_next().await {
        let r = r.unwrap();
        assert_eq!(r.status, StatusCode::OK, "{}", r.text);
        loaded_total += r.json["loaded"].as_i64().unwrap();
    }
    assert_eq!(
        loaded_total, 5,
        "across all racers, each sample was created exactly once"
    );
    let (products, variants, movements): (i64, i64, i64) = sqlx::query_as(
        "select (select count(*) from products), (select count(*) from variants), (select count(*) from stock_movements)",
    )
    .fetch_one(&app.db.fixture)
    .await
    .unwrap();
    assert_eq!((products, variants, movements), (5, 5, 5));
    Arc::into_inner(app).unwrap().db.drop().await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL + Redis"]
async fn a_real_item_with_a_sample_sku_is_never_overwritten() {
    let app = TestApp::new("development").await;
    let org = new_org(&app, "Sku Shop", "electronics").await;
    let real = app
        .post(
            &format!("/v1/orgs/{}/products", org.id),
            json!({"name":"My own phone","variants":[{"sku":"SMP-ELECTRONICS-01","priceMinor":100}]}),
            org.owner.call(),
        )
        .await;
    assert_eq!(real.status, StatusCode::CREATED, "{}", real.text);
    let load = app.post(&samples(&org), json!({}), org.owner.call()).await;
    assert_eq!(load.status, StatusCode::OK, "{}", load.text);
    assert_eq!(load.json["loaded"], 4);
    assert_eq!(load.json["skippedSkuTaken"], 1);
    let del = app
        .call("DELETE", &samples(&org), None, org.owner.call())
        .await;
    assert_eq!(del.json["deleted"], 4);
    let left = list_products(&app, &org, &org.owner).await;
    assert_eq!(left.len(), 1);
    assert_eq!(left[0]["name"], "My own phone");
    assert_eq!(left[0]["isSample"], false);
    app.db.drop().await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL + Redis"]
async fn changing_the_business_type_changes_what_can_be_loaded() {
    let app = TestApp::new("development").await;
    let org = new_org(&app, "Switcher", "electronics").await;
    assert_eq!(
        app.post(&samples(&org), json!({}), org.owner.call())
            .await
            .json["loaded"],
        5
    );
    let patch = app
        .call(
            "PATCH",
            &format!("/v1/orgs/{}", org.id),
            Some(json!({"businessType":"fashion"})),
            org.owner.call(),
        )
        .await;
    assert_eq!(patch.status, StatusCode::OK, "{}", patch.text);
    let status = app.get(&samples(&org), org.owner.call()).await;
    assert_eq!(status.json["loadable"], 5);
    assert_eq!(status.json["present"], 5);
    assert_eq!(
        app.post(&samples(&org), json!({}), org.owner.call())
            .await
            .json["loaded"],
        5
    );
    assert_eq!(count(&app, SAMPLE_PRODUCTS, &org).await, 10);
    // a legacy type (kept valid for old organizations) has no sample set: a harmless no-op
    app.call(
        "PATCH",
        &format!("/v1/orgs/{}", org.id),
        Some(json!({"businessType":"retail"})),
        org.owner.call(),
    )
    .await;
    let none = app.post(&samples(&org), json!({}), org.owner.call()).await;
    assert_eq!(none.json["loaded"], 0);
    app.db.drop().await;
}

/// The one exception to "stock_movements is append-only" is narrow: only flagged sample rows, only under the purge flag.
/// Run as a superuser so the TRIGGER (not the missing DELETE privilege) is what is being tested.
#[tokio::test]
#[ignore = "requires PostgreSQL + Redis"]
async fn the_append_only_exception_is_limited_to_flagged_samples_under_the_purge_flag() {
    let app = TestApp::new("development").await;
    let org = new_org(&app, "Trigger Shop", "electronics").await;
    let base = format!("/v1/orgs/{}", org.id);
    app.post(&samples(&org), json!({}), org.owner.call()).await;
    let real = app
        .post(
            &format!("{base}/products"),
            json!({"name":"Real","variants":[{"sku":"R-1","priceMinor":10,"openingStock":1000}]}),
            org.owner.call(),
        )
        .await;
    assert_eq!(real.status, StatusCode::CREATED, "{}", real.text);

    // a REAL movement can never be deleted, flag or no flag
    let mut tx = app.db.fixture.begin().await.unwrap();
    sqlx::query("select set_config('app.sample_purge', 'on', true)")
        .execute(&mut *tx)
        .await
        .unwrap();
    let r = sqlx::query("delete from stock_movements where not is_sample")
        .execute(&mut *tx)
        .await;
    assert!(r.is_err(), "real movements are never deletable");
    drop(tx);

    // a flagged sample movement cannot be deleted WITHOUT the purge flag
    let r = sqlx::query("delete from stock_movements where is_sample")
        .execute(&app.db.fixture)
        .await;
    assert!(r.is_err(), "sample movements need the purge flag");

    // ...nor with the flag set to anything but 'on'
    let mut tx = app.db.fixture.begin().await.unwrap();
    sqlx::query("select set_config('app.sample_purge', 'off', true)")
        .execute(&mut *tx)
        .await
        .unwrap();
    let r = sqlx::query("delete from stock_movements where is_sample")
        .execute(&mut *tx)
        .await;
    assert!(r.is_err());
    drop(tx);

    // updates and truncation stay blocked for everything
    assert!(
        sqlx::query("update stock_movements set note = 'x'")
            .execute(&app.db.fixture)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("truncate stock_movements")
            .execute(&app.db.fixture)
            .await
            .is_err()
    );
    let n: i64 = sqlx::query_scalar("select count(*) from stock_movements")
        .fetch_one(&app.db.fixture)
        .await
        .unwrap();
    assert_eq!(
        n, 6,
        "5 sample openings + 1 real opening: nothing was removed"
    );
    app.db.drop().await;
}
