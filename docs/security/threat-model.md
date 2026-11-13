# Security: authentication, sessions and tenancy (through Milestone 2)

This is a living document. "Tested by" names the automated test that fails if the control is removed
(each was also verified by deliberately breaking the control — see PROGRESS.md mutation results).

## Design
- **Passwords**: Argon2id (library defaults: 19 MiB, t=2, p=1; not yet tuned for production hardware). Length-first policy (12–128), small deny-list. Hash cost is paid for unknown accounts too (dummy hash) to limit timing differences.
- **Sessions**: 256-bit random opaque token; only its SHA-256 is stored. Cookie `__Host-permtpa_session` (Secure, HttpOnly, SameSite=Lax, Path=/) outside development. 30-day absolute and 7-day idle expiry. New token on every login; a presented session is revoked (fixation). Password reset revokes all sessions.
- **CSRF**: every state-changing request needs an allow-listed `Origin`; authenticated requests also need `X-CSRF-Token` = HMAC(secret, session-token-hash). Pre-auth endpoints rely on the Origin check.
- **Brute force**: Redis fixed-window limits (per IP and per email hash; fails closed if Redis is down) + progressive account lockout from the 5th failure (1, 2, 4 … 60 min). Locked accounts get the same generic 401.
- **Enumeration**: register / login / forgot / resend return identical bodies and statuses for known and unknown emails.
- **Email tokens**: single-use, 24 h, purpose-bound, stored hashed; issuing a new one voids older links. Verification is a POST, so link prefetchers cannot consume it.
- **MFA**: TOTP (RFC 6238, ±1 step) with per-step replay protection; secret AES-256-GCM encrypted with a key held only by the API; 10 single-use recovery codes stored hashed. Disabling requires password + a second factor.
- **Tenancy**: see docs/architecture/overview.md (server-resolved context + membership-checked org + RLS + role rules in the database).

## Control → evidence
| Threat | Control | Tested by |
|---|---|---|
| CSRF / login-CSRF | Origin allow-list + session-bound token | `csrf_protection` |
| Session fixation | Rotate on login | `sessions_list_revoke_and_rotation` |
| Stolen session after reset | Revoke all on reset | `password_reset_revokes_sessions…`, SQL suite |
| Brute force | Rate limit + lockout | `login_is_rate_limited_per_ip`, `progressive_lockout_then_reset_unlocks` |
| Account enumeration | Uniform responses | `responses_do_not_reveal_whether_an_account_exists` |
| Mass assignment | `deny_unknown_fields` | `input_validation_and_mass_assignment` |
| IDOR / cross-tenant | 404 + RLS + membership-checked org | `cross_tenant_access_is_denied_over_http`, 67 SQL attacks |
| TOTP replay / recovery-code reuse | step ledger, atomic burn | `mfa_totp_recovery_codes_and_replay_protection` |
| Secrets at rest | hashed tokens, encrypted TOTP, no secrets in audit | `secrets_at_rest_and_audit_trail` |
| Dev endpoints in prod | route not registered | `production_mode_hardens_cookies_and_hides_dev_endpoints` |
| Open redirect via `?next=` | same-site relative paths only | `safeNext` unit tests |
| Header injection in email | address parsing rejects CR/LF | `recipient_header_injection_is_rejected` |
| Plaintext SMTP in prod | config refuses it | `smtp_must_use_tls_outside_development` |

## Residual risks / not yet done
- Client IP trust depends on `TRUSTED_PROXY_HOPS` matching the real topology; wrong values either collapse all users into one rate-limit bucket or let attackers spoof IPs.
- No breached-password corpus check (only a tiny deny-list); no CAPTCHA/bot scoring; account lockout can be used to temporarily deny a victim (capped at 60 min).
- Logged-in password change, email change, recovery-code regeneration and account deletion are not implemented yet.
- The UI route guard is client-side (UX only). Authorization is enforced by the API and the database, never by the UI.
- Content-Security-Policy for the web app, `cargo audit`/`cargo deny`, container scanning and penetration testing are scheduled for Milestone 8.
- SMTP was verified against a local plaintext sink only; TLS (`smtps://`) delivery to a real provider is untested.
