#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Real SMTP delivery against a local sink (scripts/smtp-sink.py).
//!   python3 scripts/smtp-sink.py 1025 /tmp/smtp-sink.log &
//!   TEST_SMTP_URL=smtp://127.0.0.1:1025 TEST_SMTP_LOG=/tmp/smtp-sink.log cargo test -p permtpa-api --test smtp -- --ignored
use permtpa_api::mailer::{Email, Mailer};
use std::time::Duration;

fn env() -> (String, String) {
    (
        std::env::var("TEST_SMTP_URL").expect("TEST_SMTP_URL"),
        std::env::var("TEST_SMTP_LOG").expect("TEST_SMTP_LOG"),
    )
}
/// Minimal quoted-printable decoder (what any mail client does): drops soft line breaks, decodes =XX.
fn qp_decode(s: &str) -> String {
    let joined = s.replace("=\r\n", "").replace("=\n", "");
    let b = joined.as_bytes();
    let (mut out, mut i) = (Vec::new(), 0);
    while i < b.len() {
        if b[i] == b'='
            && i + 2 < b.len()
            && let Ok(v) = u8::from_str_radix(&joined[i + 1..i + 3], 16)
        {
            out.push(v);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}
async fn wait_for(log: &str, needle: &str) -> bool {
    for _ in 0..30 {
        if std::fs::read_to_string(log)
            .map(|s| qp_decode(&s).contains(needle))
            .unwrap_or(false)
        {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    false
}

#[tokio::test]
#[ignore = "requires an SMTP sink"]
async fn delivers_over_real_smtp() {
    let (url, log) = env();
    let m = Mailer::smtp(&url, "PERMTPA ONE <no-reply@permtpa.test>").unwrap();
    let marker = format!("marker-{}", uuid::Uuid::new_v4().simple());
    m.send(Email {
        to: "ada@example.test".into(),
        subject: "Verify your PERMTPA ONE email".into(),
        text: format!("Open https://one.example/verify-email?token={marker}"),
    })
    .await
    .unwrap();
    assert!(
        wait_for(&log, &format!("verify-email?token={marker}")).await,
        "message (or its intact link) never reached the SMTP server"
    );
    let all = std::fs::read_to_string(&log).unwrap();
    assert!(all.contains("RCPT TO:ada@example.test") && all.contains("no-reply@permtpa.test"));
}

#[tokio::test]
#[ignore = "requires an SMTP sink"]
async fn recipient_header_injection_is_rejected() {
    let (url, log) = env();
    let m = Mailer::smtp(&url, "no-reply@permtpa.test").unwrap();
    let evil = "victim@example.test\r\nBcc: attacker@evil.test";
    let r = m
        .send(Email {
            to: evil.into(),
            subject: "x".into(),
            text: format!("inject-{}", uuid::Uuid::new_v4().simple()),
        })
        .await;
    assert!(r.is_err(), "CRLF in the recipient must be refused");
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        !qp_decode(&std::fs::read_to_string(&log).unwrap_or_default())
            .contains("attacker@evil.test")
    );
}

#[tokio::test]
async fn invalid_configuration_is_reported_not_swallowed() {
    assert!(Mailer::smtp("not a url", "a@b.co").is_err());
    assert!(Mailer::smtp("smtp://127.0.0.1:25", "not an address").is_err());
    assert!(
        Mailer::Disabled
            .send(Email {
                to: "a@b.co".into(),
                subject: "s".into(),
                text: "t".into()
            })
            .await
            .is_err()
    );
}
