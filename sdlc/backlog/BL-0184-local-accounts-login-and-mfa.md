---
Id: BL-0184
Title: Local accounts: password login, sessions and optional MFA with an authenticator app
Nature: FEAT
Source: owner request of 2026-11-29, gap analysis item 2
Created: 2026-11-29
---

## What
The server has its own accounts. The first visit on an empty database shows a setup page that creates the first administrator. After that an administrator creates users (no self sign up). A person signs in with user and password at a login page, gets a session cookie, and signs out. Each person can set up MFA with an authenticator app (TOTP, with recovery codes), change their password, and an administrator can require MFA, reset a password or MFA, disable an account and see a list of users. The external identity mode (a gateway sets the verified token header) stays available next to local login.

## Why
Today nothing can sign a person in: identity comes only from a token minted elsewhere, so the screens answer 403 in a plain browser.

## Evidence
- `crates/core/src/identity.rs` only verifies RS256 tokens; `mcpfs_token` is read by the screens (`token_screen.rs:270`) and set by nothing.
- Platform admin and project owner and member roles exist (`storage/admin.rs`); users are only identity strings.
- No password, session or TOTP code exists (search for `argon`, `totp`, `login` finds none in production code).

## Notes
- Depends on BL-0183 (first start, key file).
- Security work: a separate review pass and attacking tests are part of the entry (password hashing with argon2id, constant time compare, login throttling and lockout, session fixation and expiry, cookie flags HttpOnly Secure SameSite, CSRF on every form, MFA brute force, recovery code single use, account enumeration, secrets encrypted under the key file, no secret in logs).
- Accounts, hashes and TOTP secrets live in the admin store; TOTP secrets are encrypted.
- Decide in its spec: lockout policy, session length, whether a gateway user is auto created, the login page and setup page built on the app-design shell (BL-0182).
