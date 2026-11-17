//! Inventory over HTTP against real PostgreSQL + Redis.
mod common;
use axum::http::StatusCode;
use common::*;
use serde_json::{Value, json};
use std::sync::Arc;

struct Org {
    id: String,
    owner: Login,
    ip: String,
}

async fn new_org(app: &TestApp, name: &str) -> Org {
    let (email, ip) = (uniq("owner"), rand_ip());
    app.signup_verified(&email, PW, &ip).await;
    let owner = app.login(&email, PW, &ip).await;
    let r = app
        .post(
            "/v1/organizations",
            json!({"name":name,"businessType":"retail"}),
            owner.call(),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.text);
    Org {
        id: r.json["id"].as_str().unwrap().to_owned(),
        owner,
        ip,
    }
}

async fn member(app: &TestApp, org: &Org, role: &str) -> Login {
    let email = uniq("member");
    app.signup_verified(&email, PW, &org.ip).await;
    let login = app.login(&email, PW, &org.ip).await;
    sqlx::query("insert into memberships (org_id, user_id, role) select $1::uuid, id, $2::org_role from users where email = $3::citext").bind(&org.id).bind(role).bind(&email).execute(&app.db.fixture).await.unwrap();
    login
}

fn base(org: &Org) -> String {
    format!("/v1/orgs/{}", org.id)
}

async fn product(
    app: &TestApp,
    org: &Org,
    name: &str,
    sku: &str,
    price: i64,
    cost: Option<i64>,
    stock_units: i64,
) -> Value {
    // callers pass WHOLE units; the API takes thousandths
    let mut v = json!({"sku":sku,"priceMinor":price,"openingStock":stock_units * 1000});
    if let Some(c) = cost {
        v["costMinor"] = json!(c);
    }
    let r = app
        .post(
            &format!("{}/products", base(org)),
            json!({"name":name,"variants":[v]}),
            org.owner.call(),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.text);
    r.json
}
fn vid(p: &Value) -> String {
    p["variants"][0]["id"].as_str().unwrap().to_owned()
}

#[tokio::test]
#[ignore = "requires PostgreSQL + Redis"]
async fn create_catalogue_with_variants_costs_and_opening_stock() {
    let app = TestApp::new("development").await;
    let org = new_org(&app, "Shop A").await;
    let cat = app
        .post(
            &format!("{}/categories", base(&org)),
            json!({"name":"Groceries"}),
            org.owner.call(),
        )
        .await;
    assert_eq!(cat.status, StatusCode::CREATED);
    let r = app.post(&format!("{}/products", base(&org)), json!({
        "name":"Rice", "description":"Parboiled", "unit":"bag", "categoryId": cat.json["id"],
        "variants":[
            {"name":"50kg","sku":"RIC-050","barcode":"6001234567890","priceMinor":1600000,"costMinor":1200000,"reorderLevel":5000,"openingStock":10000},
            {"name":"25kg","sku":"RIC-025","priceMinor":900000,"costMinor":650000,"openingStock":4000}
        ]}), org.owner.call()).await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.text);
    assert_eq!(r.json["category"], "Groceries");
    let vs = r.json["variants"].as_array().unwrap();
    assert_eq!(
        (
            vs.len(),
            vs[0]["onHand"].as_i64(),
            vs[1]["onHand"].as_i64(),
            vs[0]["costMinor"].as_i64()
        ),
        (2, Some(10_000), Some(4_000), Some(1200000))
    );
    let list = app
        .get(&format!("{}/products", base(&org)), org.owner.call())
        .await;
    assert_eq!(
        (
            list.json["total"].as_i64(),
            list.json["items"].as_array().unwrap().len()
        ),
        (Some(1), 1)
    );
    assert_eq!(
        app.get(
            &format!("{}/products/{}", base(&org), r.json["id"].as_str().unwrap()),
            org.owner.call()
        )
        .await
        .json["name"],
        "Rice"
    );
    let cats = app
        .get(&format!("{}/categories", base(&org)), org.owner.call())
        .await;
    assert_eq!(cats.json["categories"][0]["productCount"], 1);
    app.db.drop().await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL + Redis"]
async fn validation_and_mass_assignment() {
    let app = TestApp::new("development").await;
    let org = new_org(&app, "Shop V").await;
    let p = format!("{}/products", base(&org));
    let ok_v = || json!({"sku":"OK-1","priceMinor":100});
    let cases: Vec<(Value, &str)> = vec![
        (json!({"name":"  ","variants":[ok_v()]}), "blank name"),
        (json!({"name":"X","variants":[]}), "no variants"),
        (
            json!({"name":"X","variants":(0..51).map(|i| json!({"sku":format!("S{i}"),"priceMinor":1})).collect::<Vec<_>>()}),
            "51 variants",
        ),
        (
            json!({"name":"X","variants":[{"sku":"A","priceMinor":-1}]}),
            "negative price",
        ),
        (
            json!({"name":"X","variants":[{"sku":"A","priceMinor":1_000_000_000_001i64}]}),
            "price above the cap",
        ),
        (
            json!({"name":"X","variants":[{"sku":"A","priceMinor":1,"costMinor":-5}]}),
            "negative cost",
        ),
        (
            json!({"name":"X","variants":[{"sku":"bad sku!","priceMinor":1}]}),
            "bad sku",
        ),
        (
            json!({"name":"X","variants":[{"sku":"-lead","priceMinor":1}]}),
            "sku starting with a symbol",
        ),
        (
            json!({"name":"X","variants":[{"sku":"A","priceMinor":1,"barcode":"12"}]}),
            "short barcode",
        ),
        (
            json!({"name":"X","variants":[{"sku":"A","priceMinor":1,"openingStock":-3}]}),
            "negative opening stock",
        ),
        (
            json!({"name":"X","variants":[{"sku":"A","priceMinor":1.5}]}),
            "fractional money",
        ),
        (
            json!({"name":"X","variants":[{"sku":"A","priceMinor":1,"onHand":999}]}),
            "setting onHand directly",
        ),
        (
            json!({"name":"X","orgId":"x","variants":[ok_v()]}),
            "unknown field orgId",
        ),
        (
            json!({"name":"X","createdBy":"x","variants":[ok_v()]}),
            "unknown field createdBy",
        ),
        (
            json!({"name":"X","categoryId":"00000000-0000-4000-8000-000000000000","variants":[ok_v()]}),
            "category that does not exist",
        ),
    ];
    for (body, why) in cases {
        let r = app.post(&p, body, org.owner.call()).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{why}: {}", r.text);
    }
    assert_eq!(
        app.get(&p, org.owner.call()).await.json["total"],
        0,
        "nothing was created by rejected requests (transactions roll back)"
    );
    app.db.drop().await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL + Redis"]
async fn uniqueness_rules_are_per_organization() {
    let app = TestApp::new("development").await;
    let (a, b) = (new_org(&app, "Shop A").await, new_org(&app, "Shop B").await);
    let pa = product(&app, &a, "Rice", "RIC-1", 100, None, 0).await;
    let dup = app
        .post(
            &format!("{}/products", base(&a)),
            json!({"name":"Other","variants":[{"sku":"ric-1","priceMinor":1}]}),
            a.owner.call(),
        )
        .await;
    assert_eq!(
        (dup.status, dup.json["code"].as_str()),
        (StatusCode::CONFLICT, Some("sku_taken")),
        "SKU is case-insensitive"
    );
    app.post(
        &format!(
            "{}/products/{}/variants",
            base(&a),
            pa["id"].as_str().unwrap()
        ),
        json!({"sku":"RIC-2","priceMinor":1,"barcode":"6009999999991"}),
        a.owner.call(),
    )
    .await;
    let bc = app.post(&format!("{}/products", base(&a)), json!({"name":"Other","variants":[{"sku":"NEW-1","priceMinor":1,"barcode":"6009999999991"}]}), a.owner.call()).await;
    assert_eq!(bc.json["code"], "barcode_taken");
    assert_eq!(
        app.post(
            &format!("{}/categories", base(&a)),
            json!({"name":"Food"}),
            a.owner.call()
        )
        .await
        .status,
        StatusCode::CREATED
    );
    assert_eq!(
        app.post(
            &format!("{}/categories", base(&a)),
            json!({"name":"FOOD"}),
            a.owner.call()
        )
        .await
        .json["code"],
        "name_taken"
    );
    // the same values are fine in another organization
    assert_eq!(app.post(&format!("{}/products", base(&b)), json!({"name":"Rice","variants":[{"sku":"RIC-1","priceMinor":1,"barcode":"6009999999991"}]}), b.owner.call()).await.status, StatusCode::CREATED);
    assert_eq!(
        app.post(
            &format!("{}/categories", base(&b)),
            json!({"name":"Food"}),
            b.owner.call()
        )
        .await
        .status,
        StatusCode::CREATED
    );
    app.db.drop().await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL + Redis"]
async fn stock_movements_history_and_rules() {
    let app = TestApp::new("development").await;
    let org = new_org(&app, "Shop S").await;
    let p = product(&app, &org, "Oil", "OIL-5", 600000, Some(450000), 10).await;
    let v = vid(&p);
    let stock = format!("{}/variants/{v}/stock", base(&org));

    let purchase = app
        .post(
            &stock,
            json!({"delta":20000,"reason":"purchase","note":"Supplier delivery"}),
            org.owner.call(),
        )
        .await;
    assert_eq!(
        (purchase.status, purchase.json["onHand"].as_i64()),
        (StatusCode::CREATED, Some(30_000))
    );
    assert_eq!(
        app.post(
            &stock,
            json!({"delta":-5000,"reason":"damage","note":"Leaking cartons"}),
            org.owner.call()
        )
        .await
        .json["onHand"],
        25_000
    );
    assert_eq!(
        app.post(
            &stock,
            json!({"delta":-2000,"reason":"adjustment","note":"Stock count"}),
            org.owner.call()
        )
        .await
        .json["onHand"],
        23_000
    );

    for (body, code, why) in [
        (
            json!({"delta":-1000,"reason":"damage"}),
            "note_required",
            "damage without a reason",
        ),
        (
            json!({"delta":3000,"reason":"adjustment"}),
            "note_required",
            "adjustment without a reason",
        ),
        (
            json!({"delta":-3000,"reason":"purchase"}),
            "invalid_quantity",
            "negative purchase",
        ),
        (
            json!({"delta":4000,"reason":"damage","note":"x"}),
            "invalid_quantity",
            "positive damage",
        ),
        (
            json!({"delta":0,"reason":"adjustment","note":"x"}),
            "invalid_quantity",
            "zero quantity",
        ),
        (
            json!({"delta":1000,"reason":"sale"}),
            "invalid_reason",
            "sale is reserved for the sales module",
        ),
        (
            json!({"delta":1000,"reason":"gift"}),
            "invalid_reason",
            "unknown reason",
        ),
        (
            json!({"delta":200_000_000_000i64,"reason":"purchase"}),
            "invalid_quantity",
            "absurd quantity",
        ),
    ] {
        let r = app.post(&stock, body, org.owner.call()).await;
        assert_eq!(
            (r.status, r.json["code"].as_str()),
            (StatusCode::BAD_REQUEST, Some(code)),
            "{why}"
        );
    }
    assert_eq!(
        app.post(
            &stock,
            json!({"delta":1000,"reason":"purchase","actorUserId":"x"}),
            org.owner.call()
        )
        .await
        .status,
        StatusCode::BAD_REQUEST,
        "actor cannot be supplied"
    );
    let over = app
        .post(
            &stock,
            json!({"delta":-24000,"reason":"adjustment","note":"too many"}),
            org.owner.call(),
        )
        .await;
    assert_eq!(
        (over.status, over.json["code"].as_str()),
        (StatusCode::CONFLICT, Some("insufficient_stock")),
        "{}",
        over.text
    );

    let h = app
        .get(
            &format!("{}/variants/{v}/movements", base(&org)),
            org.owner.call(),
        )
        .await;
    let ms = h.json["movements"].as_array().unwrap();
    assert_eq!(
        ms.iter()
            .map(|m| m["delta"].as_i64().unwrap())
            .collect::<Vec<_>>(),
        vec![-2000, -5000, 20000, 10000],
        "newest first; the rejected movement left no trace"
    );
    assert_eq!(ms[0]["resultingOnHand"], 23_000);
    assert_eq!(ms[0]["note"], "Stock count");
    assert_eq!(ms[0]["actor"]["name"], "Test User");
    let (on_hand, sum): (i64, i64) = sqlx::query_as("select (select on_hand from stock_levels where variant_id = $1::uuid), (select sum(delta) from stock_movements where variant_id = $1::uuid)::bigint").bind(&v).fetch_one(&app.db.fixture).await.unwrap();
    assert_eq!(
        (on_hand, sum),
        (23_000, 23_000),
        "balance == sum of history"
    );

    // keyset pagination
    let page1 = app
        .get(
            &format!("{}/variants/{v}/movements?limit=2", base(&org)),
            org.owner.call(),
        )
        .await;
    assert_eq!(page1.json["movements"].as_array().unwrap().len(), 2);
    let before = page1.json["nextBefore"].as_i64().unwrap();
    let page2 = app
        .get(
            &format!(
                "{}/variants/{v}/movements?limit=2&before={before}",
                base(&org)
            ),
            org.owner.call(),
        )
        .await;
    assert_eq!(
        page2.json["movements"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["delta"].as_i64().unwrap())
            .collect::<Vec<_>>(),
        vec![20000, 10000]
    );
    assert!(page2.json["nextBefore"].is_null());
    app.db.drop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires PostgreSQL + Redis"]
async fn concurrent_withdrawals_never_oversell() {
    let app = Arc::new(TestApp::new("development").await);
    let org = new_org(&app, "Shop C").await;
    let p = product(&app, &org, "Scarce item", "SCARCE-1", 100, Some(50), 10).await;
    let path = format!("{}/variants/{}/stock", base(&org), vid(&p));
    let owner = Arc::new(org.owner);
    let mut set = tokio::task::JoinSet::new();
    for _ in 0..25 {
        let (app, path, owner) = (app.clone(), path.clone(), owner.clone());
        set.spawn(async move {
            app.post(
                &path,
                json!({"delta":-1000,"reason":"adjustment","note":"race"}),
                owner.call(),
            )
            .await
            .status
        });
    }
    let mut ok = 0;
    let mut conflict = 0;
    while let Some(r) = set.join_next().await {
        match r.unwrap() {
            StatusCode::CREATED => ok += 1,
            StatusCode::CONFLICT => conflict += 1,
            other => panic!("unexpected {other}"),
        }
    }
    assert_eq!(
        (ok, conflict),
        (10, 15),
        "exactly the available stock was sold, never more"
    );
    let (on_hand, sum, n): (i64, i64, i64) = sqlx::query_as("select (select on_hand from stock_levels), (select sum(delta) from stock_movements)::bigint, (select count(*) from stock_movements)").fetch_one(&app.db.fixture).await.unwrap();
    assert_eq!(
        (on_hand, sum, n),
        (0, 0, 11),
        "no lost updates: balance, history sum and row count agree"
    );
    Arc::into_inner(app).unwrap().db.drop().await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL + Redis"]
async fn role_matrix_over_http() {
    let app = TestApp::new("development").await;
    let org = new_org(&app, "Shop R").await;
    let p = product(&app, &org, "Sugar", "SUG-1", 50000, Some(40000), 10).await;
    let (pid, v) = (p["id"].as_str().unwrap().to_owned(), vid(&p));
    let b = base(&org);
    for role in ["sales_staff", "read_only", "accountant"] {
        let l = member(&app, &org, role).await;
        let list = app.get(&format!("{b}/products"), l.call()).await;
        assert_eq!(list.status, StatusCode::OK, "{role} can browse");
        let costs_visible = !list.json["items"][0]["variants"][0]["costMinor"].is_null();
        assert_eq!(
            costs_visible,
            role == "accountant",
            "{role}: cost visibility"
        );
        assert_eq!(
            app.get(&format!("{b}/inventory/low-stock"), l.call())
                .await
                .status,
            StatusCode::OK
        );
        assert_eq!(
            app.get(&format!("{b}/inventory/lookup?sku=SUG-1"), l.call())
                .await
                .status,
            StatusCode::OK
        );
        assert_eq!(
            app.get(&format!("{b}/variants/{v}/movements"), l.call())
                .await
                .status,
            StatusCode::OK
        );
        assert_eq!(
            app.get(&format!("{b}/inventory/valuation"), l.call())
                .await
                .status,
            if role == "accountant" {
                StatusCode::OK
            } else {
                StatusCode::FORBIDDEN
            },
            "{role}: valuation"
        );
        for (m, path, body) in [
            (
                "POST",
                format!("{b}/products"),
                Some(json!({"name":"Nope","variants":[{"sku":"N-1","priceMinor":1}]})),
            ),
            (
                "PATCH",
                format!("{b}/products/{pid}"),
                Some(json!({"name":"Hacked"})),
            ),
            (
                "POST",
                format!("{b}/products/{pid}/variants"),
                Some(json!({"sku":"N-2","priceMinor":1})),
            ),
            (
                "PATCH",
                format!("{b}/variants/{v}"),
                Some(json!({"priceMinor":1})),
            ),
            (
                "PATCH",
                format!("{b}/variants/{v}"),
                Some(json!({"costMinor":1})),
            ),
            (
                "POST",
                format!("{b}/variants/{v}/stock"),
                Some(json!({"delta":5000,"reason":"purchase"})),
            ),
            (
                "POST",
                format!("{b}/categories"),
                Some(json!({"name":"Nope"})),
            ),
        ] {
            assert_eq!(
                app.call(m, &path, body, l.call()).await.status,
                StatusCode::FORBIDDEN,
                "{role}: {m} {path}"
            );
        }
    }
    for role in ["inventory_manager", "manager", "administrator"] {
        let l = member(&app, &org, role).await;
        assert!(!app.get(&format!("{b}/products"), l.call()).await.json["items"][0]["variants"][0]["costMinor"].is_null(), "{role} sees costs");
        assert_eq!(
            app.post(
                &format!("{b}/variants/{v}/stock"),
                json!({"delta":1000,"reason":"purchase"}),
                l.call()
            )
            .await
            .status,
            StatusCode::CREATED,
            "{role} can move stock"
        );
        assert_eq!(
            app.call(
                "PATCH",
                &format!("{b}/variants/{v}"),
                Some(json!({"priceMinor":51000})),
                l.call()
            )
            .await
            .status,
            StatusCode::OK,
            "{role} can change prices"
        );
    }
    // nothing the forbidden roles tried changed anything
    let name: String = sqlx::query_scalar("select name from products where id = $1::uuid")
        .bind(&pid)
        .fetch_one(&app.db.fixture)
        .await
        .unwrap();
    assert_eq!(name, "Sugar");
    app.db.drop().await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL + Redis"]
async fn cross_tenant_inventory_attacks_all_return_404() {
    let app = TestApp::new("development").await;
    let (a, b) = (new_org(&app, "Org A").await, new_org(&app, "Org B").await);
    let p = product(&app, &a, "Secret stock", "SEC-1", 100, Some(60), 5).await;
    let (pid, v) = (p["id"].as_str().unwrap().to_owned(), vid(&p));
    let cat = app
        .post(
            &format!("{}/categories", base(&a)),
            json!({"name":"Mine"}),
            a.owner.call(),
        )
        .await
        .json["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let (ab, bb) = (base(&a), base(&b));
    for (m, path, body) in [
        ("GET", format!("{ab}/products"), None),
        ("GET", format!("{ab}/products/{pid}"), None),
        (
            "PATCH",
            format!("{ab}/products/{pid}"),
            Some(json!({"name":"Pwned"})),
        ),
        (
            "POST",
            format!("{ab}/products"),
            Some(json!({"name":"Evil","variants":[{"sku":"E-1","priceMinor":1}]})),
        ),
        (
            "POST",
            format!("{ab}/products/{pid}/variants"),
            Some(json!({"sku":"E-2","priceMinor":1})),
        ),
        (
            "PATCH",
            format!("{ab}/variants/{v}"),
            Some(json!({"priceMinor":1})),
        ),
        (
            "POST",
            format!("{ab}/variants/{v}/stock"),
            Some(json!({"delta":1_000_000,"reason":"purchase"})),
        ),
        ("GET", format!("{ab}/variants/{v}/movements"), None),
        ("GET", format!("{ab}/categories"), None),
        (
            "POST",
            format!("{ab}/categories"),
            Some(json!({"name":"Evil"})),
        ),
        ("DELETE", format!("{ab}/categories/{cat}"), None),
        ("GET", format!("{ab}/inventory/lookup?sku=SEC-1"), None),
        ("GET", format!("{ab}/inventory/low-stock"), None),
        ("GET", format!("{ab}/inventory/valuation"), None),
        // A's ids addressed through B's own (legitimate) org path
        ("GET", format!("{bb}/products/{pid}"), None),
        (
            "PATCH",
            format!("{bb}/variants/{v}"),
            Some(json!({"priceMinor":1})),
        ),
        (
            "POST",
            format!("{bb}/variants/{v}/stock"),
            Some(json!({"delta":1_000_000,"reason":"purchase"})),
        ),
        ("GET", format!("{bb}/variants/{v}/movements"), None),
        (
            "POST",
            format!("{bb}/products/{pid}/variants"),
            Some(json!({"sku":"E-3","priceMinor":1})),
        ),
        ("DELETE", format!("{bb}/categories/{cat}"), None),
        ("GET", format!("{bb}/inventory/lookup?sku=SEC-1"), None),
    ] {
        let r = app.call(m, &path, body, b.owner.call()).await;
        assert_eq!(
            r.status,
            StatusCode::NOT_FOUND,
            "{m} {path} must be 404 for another tenant's owner (got {})",
            r.text
        );
    }
    let bc = app.post(&format!("{bb}/products"), json!({"name":"Borrowed category","categoryId":cat,"variants":[{"sku":"B-1","priceMinor":1}]}), b.owner.call()).await;
    assert_eq!(
        bc.status,
        StatusCode::BAD_REQUEST,
        "another tenant's category id is rejected"
    );
    let (name, on_hand): (String, i64) = sqlx::query_as("select (select name from products where id = $1::uuid), (select on_hand from stock_levels where variant_id = $2::uuid)").bind(&pid).bind(&v).fetch_one(&app.db.fixture).await.unwrap();
    assert_eq!((name.as_str(), on_hand), ("Secret stock", 5_000));
    assert_eq!(
        app.get(&format!("{bb}/products"), b.owner.call())
            .await
            .json["total"],
        0
    );
    app.db.drop().await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL + Redis"]
async fn archive_lookup_low_stock_and_valuation() {
    let app = TestApp::new("development").await;
    let org = new_org(&app, "Shop L").await;
    let b = base(&org);
    let p1 = product(&app, &org, "Rice", "RIC-1", 1600000, Some(1200000), 7).await;
    let v1 = vid(&p1);
    app.call(
        "PATCH",
        &format!("{b}/variants/{v1}"),
        Some(json!({"reorderLevel":10000,"barcode":"6001111111111"})),
        org.owner.call(),
    )
    .await;
    let p2 = product(&app, &org, "No cost item", "NC-1", 500, None, 3).await;
    let p3 = product(&app, &org, "Plenty", "PL-1", 100, Some(50), 100).await;
    app.call(
        "PATCH",
        &format!("{b}/variants/{}", vid(&p3)),
        Some(json!({"reorderLevel":10000})),
        org.owner.call(),
    )
    .await;
    let _ = p2;

    let low = app
        .get(&format!("{b}/inventory/low-stock"), org.owner.call())
        .await;
    assert_eq!(
        low.json["items"].as_array().unwrap().len(),
        1,
        "only variants at/below their reorder level"
    );
    assert_eq!(
        (
            low.json["items"][0]["sku"].as_str(),
            low.json["items"][0]["onHand"].as_i64()
        ),
        (Some("RIC-1"), Some(7_000))
    );

    // exact arithmetic: 7×1_200_000 + 100×50 = 8_405_000; one stocked variant has no cost
    let val = app
        .get(&format!("{b}/inventory/valuation"), org.owner.call())
        .await;
    assert_eq!(
        (
            val.json["totalMinor"].as_str(),
            val.json["unitsThousandths"].as_i64(),
            val.json["variantsMissingCost"].as_i64()
        ),
        (Some("8405000"), Some(110_000), Some(1))
    );

    // barcode / sku lookup
    let l = app
        .get(
            &format!("{b}/inventory/lookup?barcode=6001111111111"),
            org.owner.call(),
        )
        .await;
    assert_eq!(
        (
            l.status,
            l.json["sku"].as_str(),
            l.json["priceMinor"].as_i64()
        ),
        (StatusCode::OK, Some("RIC-1"), Some(1600000))
    );
    assert_eq!(
        app.get(&format!("{b}/inventory/lookup?sku=ric-1"), org.owner.call())
            .await
            .status,
        StatusCode::OK,
        "sku lookup is case-insensitive"
    );
    assert_eq!(
        app.get(
            &format!("{b}/inventory/lookup?barcode=0000000000"),
            org.owner.call()
        )
        .await
        .status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.get(&format!("{b}/inventory/lookup"), org.owner.call())
            .await
            .status,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        app.get(
            &format!("{b}/inventory/lookup?sku=A&barcode=B"),
            org.owner.call()
        )
        .await
        .status,
        StatusCode::BAD_REQUEST
    );

    // archiving hides from default lists, lookup, low-stock and valuation, but keeps history
    let arch = app
        .call(
            "PATCH",
            &format!("{b}/products/{}", p1["id"].as_str().unwrap()),
            Some(json!({"status":"archived"})),
            org.owner.call(),
        )
        .await;
    assert_eq!(arch.json["status"], "archived");
    assert_eq!(
        app.get(&format!("{b}/products"), org.owner.call())
            .await
            .json["total"],
        2
    );
    assert_eq!(
        app.get(&format!("{b}/products?status=archived"), org.owner.call())
            .await
            .json["total"],
        1
    );
    assert_eq!(
        app.get(&format!("{b}/products?status=all"), org.owner.call())
            .await
            .json["total"],
        3
    );
    assert_eq!(
        app.get(&format!("{b}/products?status=bogus"), org.owner.call())
            .await
            .status,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        app.get(&format!("{b}/inventory/lookup?sku=RIC-1"), org.owner.call())
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.get(&format!("{b}/inventory/low-stock"), org.owner.call())
            .await
            .json["items"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    assert_eq!(
        app.get(&format!("{b}/inventory/valuation"), org.owner.call())
            .await
            .json["totalMinor"],
        "5000"
    );
    assert_eq!(
        app.get(&format!("{b}/variants/{v1}/movements"), org.owner.call())
            .await
            .json["movements"]
            .as_array()
            .unwrap()
            .len(),
        1,
        "history survives archiving"
    );
    app.db.drop().await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL + Redis"]
async fn valuation_is_exact_beyond_64_bit_range() {
    let app = TestApp::new("development").await;
    let org = new_org(&app, "Shop Big").await;
    product(
        &app,
        &org,
        "Gold bars",
        "GOLD-1",
        1_000_000_000_000,
        Some(1_000_000_000_000),
        100_000_000,
    )
    .await;
    let val = app
        .get(
            &format!("{}/inventory/valuation", base(&org)),
            org.owner.call(),
        )
        .await;
    assert_eq!(
        val.json["totalMinor"], "100000000000000000000",
        "10^12 × 10^8 = 10^20 exceeds i64 yet must be exact"
    );
    app.db.drop().await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL + Redis"]
async fn search_pagination_and_category_rules() {
    let app = TestApp::new("development").await;
    let org = new_org(&app, "Shop P").await;
    let b = base(&org);
    for i in 0..7 {
        product(
            &app,
            &org,
            &format!("Widget {i}"),
            &format!("WID-{i}"),
            100,
            None,
            0,
        )
        .await;
    }
    let special = app.post(&format!("{b}/products"), json!({"name":"100% Cotton_Shirt","variants":[{"sku":"COT-1","barcode":"7001234500001","priceMinor":1}]}), org.owner.call()).await;
    assert_eq!(special.status, StatusCode::CREATED);
    let q = |s: &str| format!("{b}/products?search={s}");
    let p1 = app
        .get(&format!("{b}/products?limit=3&offset=0"), org.owner.call())
        .await;
    assert_eq!(
        (
            p1.json["total"].as_i64(),
            p1.json["items"].as_array().unwrap().len(),
            p1.json["limit"].as_i64()
        ),
        (Some(8), 3, Some(3))
    );
    let names = |r: &common::Resp| {
        r.json["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["name"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>()
    };
    let p3 = app
        .get(&format!("{b}/products?limit=3&offset=6"), org.owner.call())
        .await;
    assert_eq!(names(&p3).len(), 2);
    // bulk-load 105 more products through the superuser fixture, then ask for an absurd page size
    sqlx::query("with p as (insert into products (org_id, name, created_by) select $1::uuid, 'Bulk ' || g, (select user_id from memberships where org_id = $1::uuid limit 1) from generate_series(1, 105) g returning id, name) insert into variants (org_id, product_id, sku, price_minor) select $1::uuid, id, 'BULK-' || substr(name, 6), 1 from p")
        .bind(&org.id).execute(&app.db.fixture).await.unwrap();
    let big = app
        .get(&format!("{b}/products?limit=100000"), org.owner.call())
        .await;
    assert_eq!(
        (
            big.json["limit"].as_i64(),
            big.json["items"].as_array().unwrap().len(),
            big.json["total"].as_i64()
        ),
        (Some(100), 100, Some(113)),
        "page size is capped: only 100 of 113 rows are returned"
    );
    let rest = app
        .get(
            &format!("{b}/products?limit=100000&offset=100"),
            org.owner.call(),
        )
        .await;
    assert_eq!(rest.json["items"].as_array().unwrap().len(), 13);
    assert_eq!(
        app.get(&q("widget%253"), org.owner.call()).await.json["total"],
        0,
        "a literal % (encoded as %25) is not a wildcard"
    );
    assert_eq!(
        app.get(&q("widget%203"), org.owner.call()).await.json["total"],
        1,
        "%20 is just a space: 'widget 3'"
    );
    assert_eq!(
        names(&app.get(&q("widget"), org.owner.call()).await).len(),
        7
    );
    assert_eq!(
        app.get(&q("WID-4"), org.owner.call()).await.json["total"],
        1,
        "sku prefix"
    );
    assert_eq!(
        app.get(&q("7001234500001"), org.owner.call()).await.json["total"],
        1,
        "exact barcode"
    );
    assert_eq!(
        app.get(&q("%25"), org.owner.call()).await.json["total"],
        1,
        "a literal % matches only names containing it, not everything"
    );
    assert_eq!(
        app.get(&q("_"), org.owner.call()).await.json["total"],
        1,
        "a literal _ is not a wildcard"
    );
    assert_eq!(
        app.get(
            &q("%27%3B%20drop%20table%20products%3B--"),
            org.owner.call()
        )
        .await
        .json["total"],
        0,
        "SQL metacharacters are inert"
    );

    // category rules
    let c = app
        .post(
            &format!("{b}/categories"),
            json!({"name":"Textiles"}),
            org.owner.call(),
        )
        .await
        .json["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let pid = special.json["id"].as_str().unwrap();
    app.call(
        "PATCH",
        &format!("{b}/products/{pid}"),
        Some(json!({"categoryId":c})),
        org.owner.call(),
    )
    .await;
    let in_use = app
        .call(
            "DELETE",
            &format!("{b}/categories/{c}"),
            None,
            org.owner.call(),
        )
        .await;
    assert_eq!(
        (in_use.status, in_use.json["code"].as_str()),
        (StatusCode::CONFLICT, Some("category_in_use"))
    );
    assert_eq!(
        app.get(&format!("{b}/products?category={c}"), org.owner.call())
            .await
            .json["total"],
        1
    );
    app.call(
        "PATCH",
        &format!("{b}/products/{pid}"),
        Some(json!({"clearCategory":true})),
        org.owner.call(),
    )
    .await;
    assert_eq!(
        app.call(
            "DELETE",
            &format!("{b}/categories/{c}"),
            None,
            org.owner.call()
        )
        .await
        .status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        app.call(
            "DELETE",
            &format!("{b}/categories/{c}"),
            None,
            org.owner.call()
        )
        .await
        .status,
        StatusCode::NOT_FOUND
    );
    app.db.drop().await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL + Redis"]
async fn price_and_cost_changes_stock_adjustments_are_audited() {
    let app = TestApp::new("development").await;
    let org = new_org(&app, "Shop Au").await;
    let b = base(&org);
    let p = product(&app, &org, "Salt", "SAL-1", 1000, Some(600), 10).await;
    let v = vid(&p);
    let owner_id: String = sqlx::query_scalar(
        "select user_id::text from memberships where org_id = $1::uuid and role = 'owner'",
    )
    .bind(&org.id)
    .fetch_one(&app.db.fixture)
    .await
    .unwrap();
    app.call(
        "PATCH",
        &format!("{b}/variants/{v}"),
        Some(json!({"priceMinor":1200,"costMinor":700})),
        org.owner.call(),
    )
    .await;
    app.post(
        &format!("{b}/variants/{v}/stock"),
        json!({"delta":-2000,"reason":"damage","note":"wet"}),
        org.owner.call(),
    )
    .await;
    app.call(
        "PATCH",
        &format!("{b}/products/{}", p["id"].as_str().unwrap()),
        Some(json!({"status":"archived"})),
        org.owner.call(),
    )
    .await;

    let rows: Vec<(String, Option<String>, Value)> = sqlx::query_as("select action, actor_user_id::text, metadata from audit_logs where org_id = $1::uuid order by id").bind(&org.id).fetch_all(&app.db.fixture).await.unwrap();
    for a in [
        "product.create",
        "variant.create",
        "variant.cost_change",
        "variant.price_change",
        "stock.adjust",
        "product.archive",
    ] {
        assert!(
            rows.iter()
                .any(|(x, actor, _)| x == a && actor.as_deref() == Some(owner_id.as_str())),
            "missing {a} attributed to the owner: {:?}",
            rows.iter().map(|r| &r.0).collect::<Vec<_>>()
        );
    }
    let price = rows.iter().find(|r| r.0 == "variant.price_change").unwrap();
    assert_eq!(
        (price.2["from_minor"].as_i64(), price.2["to_minor"].as_i64()),
        (Some(1000), Some(1200))
    );
    let adj = rows.iter().find(|r| r.0 == "stock.adjust").unwrap();
    assert_eq!(
        (
            adj.2["delta"].as_i64(),
            adj.2["reason"].as_str(),
            adj.2["resulting_on_hand"].as_i64()
        ),
        (Some(-2000), Some("damage"), Some(8000))
    );
    assert_eq!(
        adj.2["qty_scale"].as_i64(),
        Some(1000),
        "new audit rows record the quantity scale"
    );
    app.db.drop().await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL + Redis"]
async fn variants_keep_their_insertion_order() {
    // Regression: variants created in one transaction share created_at; ordering used to fall back to random UUIDs.
    let app = TestApp::new("development").await;
    let org = new_org(&app, "Shop O").await;
    let variants: Vec<Value> = (0..12)
        .map(|i| json!({"name":format!("Size {i}"),"sku":format!("ORD-{i:02}"),"priceMinor":100}))
        .collect();
    for round in 0..3 {
        let r = app.post(&format!("{}/products", base(&org)), json!({"name":format!("Shirt {round}"),"variants":variants.iter().map(|v| { let mut v = v.clone(); v["sku"] = json!(format!("{}-{round}", v["sku"].as_str().unwrap())); v }).collect::<Vec<_>>()}), org.owner.call()).await;
        let names: Vec<String> = r.json["variants"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["name"].as_str().unwrap().to_owned())
            .collect();
        assert_eq!(
            names,
            (0..12).map(|i| format!("Size {i}")).collect::<Vec<_>>(),
            "round {round}"
        );
        let again = app
            .get(
                &format!("{}/products/{}", base(&org), r.json["id"].as_str().unwrap()),
                org.owner.call(),
            )
            .await;
        assert_eq!(
            again.json["variants"], r.json["variants"],
            "the same order on every read"
        );
    }
    app.db.drop().await;
}

// ---------------------------------------------------------------- 0009: fractional quantities (UNVERIFIED until run)
async fn frac_product(app: &TestApp, org: &Org, sku: &str, cost: i64, opening: i64) -> Value {
    let r = app.post(&format!("{}/products", base(org)), json!({"name":"Loose rice","unit":"kg","allowFractional":true,"variants":[{"sku":sku,"priceMinor":2000,"costMinor":cost,"openingStock":opening,"reorderLevel":500}]}), org.owner.call()).await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.text);
    r.json
}

#[tokio::test]
#[ignore = "requires PostgreSQL + Redis"]
async fn fractional_products_use_exact_thousandths_and_whole_products_refuse_fractions() {
    let app = TestApp::new("development").await;
    let org = new_org(&app, "Shop Frac").await;
    let b = base(&org);
    let f = frac_product(&app, &org, "KG-1", 1001, 2500).await;
    assert_eq!(
        (
            f["allowFractional"].as_bool(),
            f["variants"][0]["onHand"].as_i64(),
            f["variants"][0]["reorderLevel"].as_i64()
        ),
        (Some(true), Some(2500), Some(500))
    );
    let fv = vid(&f);
    let take = app
        .post(
            &format!("{b}/variants/{fv}/stock"),
            json!({"delta":-750,"reason":"damage","note":"spilled"}),
            org.owner.call(),
        )
        .await;
    assert_eq!(
        (take.status, take.json["onHand"].as_i64()),
        (StatusCode::CREATED, Some(1750)),
        "{}",
        take.text
    );
    // 1.750 kg x 1001 kobo = 1751.75 -> rounded once to whole kobo
    let val = app
        .get(&format!("{b}/inventory/valuation"), org.owner.call())
        .await;
    assert_eq!(
        (
            val.json["totalMinor"].as_str(),
            val.json["unitsThousandths"].as_i64()
        ),
        (Some("1752"), Some(1750)),
        "{}",
        val.text
    );

    // whole-unit product: the same fractions are refused, on every entry path
    let w = product(&app, &org, "Bag of salt", "SALT-1", 1000, Some(600), 3).await;
    let wv = vid(&w);
    assert_eq!(w["allowFractional"].as_bool(), Some(false));
    for (body, what) in [
        (
            json!({"delta":2500,"reason":"purchase"}),
            "fractional purchase",
        ),
        (
            json!({"delta":-500,"reason":"damage","note":"x"}),
            "fractional damage",
        ),
        (json!({"delta":1,"reason":"purchase"}), "0.001 purchase"),
    ] {
        let r = app
            .post(&format!("{b}/variants/{wv}/stock"), body, org.owner.call())
            .await;
        assert_eq!(
            (r.status, r.json["code"].as_str()),
            (StatusCode::BAD_REQUEST, Some("fractional_not_allowed")),
            "{what}: {}",
            r.text
        );
    }
    let bad_open = app
        .post(
            &format!("{b}/products"),
            json!({"name":"X","variants":[{"sku":"X-1","priceMinor":1,"openingStock":1500}]}),
            org.owner.call(),
        )
        .await;
    assert_eq!(
        (bad_open.status, bad_open.json["code"].as_str()),
        (StatusCode::BAD_REQUEST, Some("fractional_not_allowed")),
        "fractional opening stock"
    );
    let bad_re = app
        .call(
            "PATCH",
            &format!("{b}/variants/{wv}"),
            Some(json!({"reorderLevel":2500})),
            org.owner.call(),
        )
        .await;
    assert_eq!(
        (bad_re.status, bad_re.json["code"].as_str()),
        (StatusCode::BAD_REQUEST, Some("fractional_not_allowed")),
        "fractional reorder level"
    );
    let ok_re = app
        .call(
            "PATCH",
            &format!("{b}/variants/{fv}"),
            Some(json!({"reorderLevel":2500})),
            org.owner.call(),
        )
        .await;
    assert_eq!(
        ok_re.status,
        StatusCode::OK,
        "fractional product accepts a fractional reorder level"
    );
    assert_eq!(
        app.get(&format!("{b}/variants/{wv}/movements"), org.owner.call())
            .await
            .json["movements"]
            .as_array()
            .unwrap()
            .len(),
        1,
        "rejected fractions left no trace"
    );

    // allow_fractional can be neither supplied on update nor changed by the app role
    let pid = w["id"].as_str().unwrap();
    assert_eq!(
        app.call(
            "PATCH",
            &format!("{b}/products/{pid}"),
            Some(json!({"allowFractional":true})),
            org.owner.call()
        )
        .await
        .status,
        StatusCode::BAD_REQUEST,
        "unknown field on update"
    );
    let direct = sqlx::query("update products set allow_fractional = true where id = $1::uuid")
        .bind(pid)
        .execute(&app.db.app)
        .await;
    assert!(
        direct.is_err(),
        "the runtime role has no UPDATE privilege on allow_fractional"
    );
    app.db.drop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires PostgreSQL + Redis"]
async fn concurrent_fractional_withdrawals_never_oversell() {
    let app = Arc::new(TestApp::new("development").await);
    let org = new_org(&app, "Shop FracC").await;
    let f = frac_product(&app, &org, "KG-C", 100, 2000).await; // 2.000 kg
    let path = format!("{}/variants/{}/stock", base(&org), vid(&f));
    let owner = Arc::new(org.owner);
    let mut set = tokio::task::JoinSet::new();
    for _ in 0..25 {
        let (app, path, owner) = (app.clone(), path.clone(), owner.clone());
        set.spawn(async move {
            app.post(
                &path,
                json!({"delta":-400,"reason":"adjustment","note":"race"}),
                owner.call(),
            )
            .await
            .status
        });
    }
    let (mut ok, mut conflict) = (0, 0);
    while let Some(r) = set.join_next().await {
        match r.unwrap() {
            StatusCode::CREATED => ok += 1,
            StatusCode::CONFLICT => conflict += 1,
            other => panic!("unexpected {other}"),
        }
    }
    assert_eq!(
        (ok, conflict),
        (5, 20),
        "2.000 kg sells in exactly five 0.400 kg portions"
    );
    let (on_hand, sum): (i64, i64) = sqlx::query_as("select (select on_hand from stock_levels), (select sum(delta) from stock_movements)::bigint").fetch_one(&app.db.fixture).await.unwrap();
    assert_eq!((on_hand, sum), (0, 0));
    Arc::into_inner(app).unwrap().db.drop().await;
}
