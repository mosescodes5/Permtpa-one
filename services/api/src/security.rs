//! Cryptographic primitives: tokens, hashing, CSRF derivation, TOTP, secret encryption.
use aes_gcm::{Aes256Gcm, KeyInit, aead::Aead};
use argon2::{
    Argon2,
    password_hash::{PasswordHasher, PasswordVerifier, phc::PasswordHash},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("randomness unavailable")]
    Random,
    #[error("hashing failed")]
    Hash,
    #[error("decryption failed")]
    Decrypt,
}

pub fn random_bytes<const N: usize>() -> Result<[u8; N], CryptoError> {
    let mut b = [0u8; N];
    getrandom::fill(&mut b).map_err(|_| CryptoError::Random)?;
    Ok(b)
}

/// 256-bit opaque token (session cookies, email links), URL-safe base64.
pub fn new_token() -> Result<String, CryptoError> {
    Ok(URL_SAFE_NO_PAD.encode(random_bytes::<32>()?))
}

/// SHA-256 of a token. Only this hash is ever stored; high-entropy tokens do not need a slow KDF.
pub fn hash_token(token: &str) -> [u8; 32] {
    Sha256::digest(token.as_bytes()).into()
}

pub fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    a.ct_eq(b).into()
}

// ---- passwords (Argon2id, library defaults = OWASP-recommended parameters) ----
pub fn hash_password(password: &str) -> Result<String, CryptoError> {
    Argon2::default()
        .hash_password(password.as_bytes())
        .map(|h| h.to_string())
        .map_err(|_| CryptoError::Hash)
}

pub fn verify_password(phc: &str, password: &str) -> bool {
    match PasswordHash::new(phc) {
        Ok(parsed) => Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok(),
        Err(_) => false,
    }
}

// ---- CSRF: token bound to the session (HMAC of the session token hash) ----
pub fn csrf_token(secret: &[u8; 32], session_token_hash: &[u8; 32]) -> String {
    #[allow(clippy::expect_used)] // HMAC accepts keys of any length, so this cannot fail
    let mut mac = <Hmac<Sha256> as hmac::KeyInit>::new_from_slice(secret)
        .expect("hmac accepts any key length");
    mac.update(b"permtpa-one:csrf:v1:");
    mac.update(session_token_hash);
    URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
}

// ---- TOTP (RFC 6238, HMAC-SHA1, 6 digits, 30 s) ----
pub fn hotp(secret: &[u8], counter: u64) -> u32 {
    #[allow(clippy::expect_used)] // HMAC accepts keys of any length, so this cannot fail
    let mut mac = <Hmac<sha1::Sha1> as hmac::KeyInit>::new_from_slice(secret)
        .expect("hmac accepts any key length");
    mac.update(&counter.to_be_bytes());
    let h = mac.finalize().into_bytes();
    let off = (h[h.len() - 1] & 0x0f) as usize;
    let bin = u32::from_be_bytes([h[off] & 0x7f, h[off + 1], h[off + 2], h[off + 3]]);
    bin % 1_000_000
}

/// Returns the matched time-step if `code` is valid within Â±1 step AND that step is newer than the last one used
/// (so a captured code cannot be replayed).
pub fn verify_totp(secret: &[u8], code: &str, now_unix: u64, last_used_step: i64) -> Option<u64> {
    if code.len() != 6 || !code.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let step_now = now_unix / 30;
    let mut found = None;
    for step in step_now.saturating_sub(1)..=step_now + 1 {
        let expected = format!("{:06}", hotp(secret, step));
        // evaluate every candidate (no early exit) to keep timing uniform
        if ct_eq(expected.as_bytes(), code.as_bytes()) && (step as i64) > last_used_step {
            found = Some(step);
        }
    }
    found
}

pub fn base32(data: &[u8]) -> String {
    const A: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let (mut out, mut buf, mut bits) = (String::new(), 0u32, 0u32);
    for &b in data {
        buf = (buf << 8) | b as u32;
        bits += 8;
        while bits >= 5 {
            out.push(A[((buf >> (bits - 5)) & 31) as usize] as char);
            bits -= 5;
        }
    }
    if bits > 0 {
        out.push(A[((buf << (5 - bits)) & 31) as usize] as char);
    }
    out
}

/// Recovery code like `k3x9m-q2w7d` (50 bits of entropy), stored only as a SHA-256 hash.
pub fn new_recovery_code() -> Result<String, CryptoError> {
    let s = base32(&random_bytes::<7>()?).to_lowercase();
    Ok(format!("{}-{}", &s[0..5], &s[5..10]))
}

pub fn normalize_recovery_code(input: &str) -> String {
    input.trim().to_lowercase()
}

// ---- encryption of MFA secrets at rest (AES-256-GCM, random 96-bit nonce prepended) ----
pub fn encrypt(key: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| CryptoError::Decrypt)?;
    let nonce_bytes = random_bytes::<12>()?;
    let ct = cipher
        .encrypt(&nonce_bytes.into(), plaintext)
        .map_err(|_| CryptoError::Decrypt)?;
    let mut out = nonce_bytes.to_vec();
    out.extend(ct);
    Ok(out)
}

pub fn decrypt(key: &[u8; 32], blob: &[u8]) -> Result<Vec<u8>, CryptoError> {
    if blob.len() < 13 {
        return Err(CryptoError::Decrypt);
    }
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| CryptoError::Decrypt)?;
    let (nonce, ct) = blob.split_at(12);
    let nonce: [u8; 12] = nonce.try_into().map_err(|_| CryptoError::Decrypt)?;
    cipher
        .decrypt(&nonce.into(), ct)
        .map_err(|_| CryptoError::Decrypt)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn totp_matches_rfc6238_vectors() {
        // RFC 6238 Appendix B (SHA-1, secret "12345678901234567890"); 6-digit truncation of the 8-digit vectors.
        let secret = b"12345678901234567890";
        for (t, want) in [
            (59u64, 287082u32),
            (1111111109, 81804),
            (1111111111, 50471),
            (1234567890, 5924),
            (2000000000, 279037),
        ] {
            assert_eq!(hotp(secret, t / 30), want, "T={t}");
        }
    }

    #[test]
    fn totp_window_and_replay_protection() {
        let s = b"12345678901234567890";
        let code = format!("{:06}", hotp(s, 59 / 30));
        assert_eq!(verify_totp(s, &code, 59, 0), Some(1));
        assert_eq!(
            verify_totp(s, &code, 59 + 30, 0),
            Some(1),
            "Â±1 step tolerance"
        );
        assert_eq!(verify_totp(s, &code, 59 + 90, 0), None, "outside window");
        assert_eq!(
            verify_totp(s, &code, 59, 1),
            None,
            "same step cannot be replayed"
        );
        assert_eq!(verify_totp(s, "12345", 59, 0), None);
        assert_eq!(verify_totp(s, "abcdef", 59, 0), None);
    }

    #[test]
    fn base32_rfc4648_vectors() {
        assert_eq!(base32(b"foobar"), "MZXW6YTBOI");
        assert_eq!(base32(b"fo"), "MZXQ");
    }

    #[test]
    fn argon2id_roundtrip_and_rejects_wrong_or_garbage() {
        let h = hash_password("correct horse battery staple").unwrap();
        assert!(h.starts_with("$argon2id$"));
        assert!(verify_password(&h, "correct horse battery staple"));
        assert!(!verify_password(&h, "wrong"));
        assert!(!verify_password("not-a-hash", "x"));
        assert_ne!(
            h,
            hash_password("correct horse battery staple").unwrap(),
            "salts differ"
        );
    }

    #[test]
    fn tokens_are_unique_and_long() {
        let (a, b) = (new_token().unwrap(), new_token().unwrap());
        assert_ne!(a, b);
        assert!(a.len() >= 43);
        assert_eq!(hash_token(&a).len(), 32);
    }

    #[test]
    fn csrf_is_session_bound_and_deterministic() {
        let k = [7u8; 32];
        let (h1, h2) = (hash_token("s1"), hash_token("s2"));
        assert_eq!(csrf_token(&k, &h1), csrf_token(&k, &h1));
        assert_ne!(csrf_token(&k, &h1), csrf_token(&k, &h2));
        assert_ne!(csrf_token(&k, &h1), csrf_token(&[8u8; 32], &h1));
    }

    #[test]
    fn encryption_roundtrips_and_detects_tampering() {
        let k = [9u8; 32];
        let ct = encrypt(&k, b"totp-secret").unwrap();
        assert_eq!(decrypt(&k, &ct).unwrap(), b"totp-secret");
        let mut bad = ct.clone();
        *bad.last_mut().unwrap() ^= 1;
        assert!(decrypt(&k, &bad).is_err());
        assert!(decrypt(&[1u8; 32], &ct).is_err());
        assert!(decrypt(&k, b"short").is_err());
    }

    #[test]
    fn recovery_codes_look_right() {
        let c = new_recovery_code().unwrap();
        assert_eq!(c.len(), 11);
        assert_eq!(c.as_bytes()[5], b'-');
        assert_eq!(normalize_recovery_code("  K3X9M-Q2W7D "), "k3x9m-q2w7d");
    }
}
