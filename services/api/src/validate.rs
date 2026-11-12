use crate::error::ApiError;

const COMMON_PASSWORDS: &[&str] = &[
    "password1234", "123456789012", "qwertyuiopas", "letmein12345", "administrator", "welcome12345", "iloveyou1234", "passwordpassword", "1234567890123",
];

pub fn email(raw: &str) -> Result<String, ApiError> {
    let e = raw.trim().to_lowercase();
    let ok = e.len() <= 254
        && e.is_ascii()
        && !e.contains(char::is_whitespace)
        && e.matches('@').count() == 1
        && e.split_once('@').is_some_and(|(l, d)| !l.is_empty() && l.len() <= 64 && d.contains('.') && !d.starts_with('.') && !d.ends_with('.') && !d.contains(".."));
    if ok { Ok(e) } else { Err(ApiError::bad("invalid_email", "Enter a valid email address.")) }
}

/// Length-first policy (NIST SP 800-63B): 12–128 characters, no arbitrary composition rules, small deny-list.
pub fn password(pw: &str, email: Option<&str>) -> Result<(), ApiError> {
    let n = pw.chars().count();
    if !(12..=128).contains(&n) {
        return Err(ApiError::bad("invalid_password", "Use between 12 and 128 characters."));
    }
    let lower = pw.to_lowercase();
    if COMMON_PASSWORDS.contains(&lower.as_str()) || email.is_some_and(|e| lower == e || e.split('@').next().is_some_and(|l| l.len() >= 6 && lower.contains(l))) {
        return Err(ApiError::bad("weak_password", "Choose a less predictable password."));
    }
    Ok(())
}

pub fn person_name(raw: &str) -> Result<String, ApiError> {
    let n = raw.trim();
    if (2..=100).contains(&n.chars().count()) && !n.chars().any(char::is_control) { Ok(n.to_owned()) } else { Err(ApiError::bad("invalid_name", "Enter your full name.")) }
}

pub fn org_name(raw: &str) -> Result<String, ApiError> {
    let n = raw.trim();
    if (2..=120).contains(&n.chars().count()) && !n.chars().any(char::is_control) { Ok(n.to_owned()) } else { Err(ApiError::bad("invalid_name", "Enter a business name (2–120 characters).")) }
}

pub fn slugify(name: &str) -> String {
    let mut s: String = name.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    while s.contains("--") { s = s.replace("--", "-"); }
    let s = s.trim_matches('-');
    let s: String = s.chars().take(40).collect();
    let s = s.trim_matches('-').to_owned();
    if s.len() >= 2 { s } else { "org".to_owned() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emails() {
        assert_eq!(email("  Ada@Example.COM ").unwrap(), "ada@example.com");
        for bad in ["", "a", "a@b", "a@@b.co", "a b@c.co", "@c.co", "a@.co", "a@c..co", "ünï@c.co"] {
            assert!(email(bad).is_err(), "{bad}");
        }
        assert!(email(&format!("{}@c.co", "a".repeat(65))).is_err());
    }

    #[test]
    fn passwords() {
        assert!(password("correct horse battery", None).is_ok());
        assert!(password("short", None).is_err());
        assert!(password(&"a".repeat(129), None).is_err());
        assert!(password("PasswordPassword", None).is_err());
        assert!(password("ada.lovelace-2026!", Some("ada.lovelace@x.co")).is_err(), "contains email local part");
        assert!(password("12 chars long ok", None).is_ok());
    }

    #[test]
    fn slugs() {
        assert_eq!(slugify("Okafor & Sons Trading!"), "okafor-sons-trading");
        assert_eq!(slugify("***"), "org");
        assert!(slugify(&"x".repeat(200)).len() <= 40);
    }
}
