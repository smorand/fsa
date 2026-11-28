---
Id: BL-0185
Title: Personal access tokens: generate your own bearer key
Nature: FEAT
Source: owner request of 2026-11-29, gap analysis item 3
Created: 2026-11-29
---

## What
A signed in person creates, lists and revokes personal access tokens from a screen. A token has a name, an optional expiry and is shown once; only a hash is stored. The token works as `Authorization: Bearer ...` on `/mcp`, `/api/fs`, the other data routes and git over HTTP, with the same rights as its owner. An administrator can list and revoke any token.

## Why
Agents and scripts need a credential for a person without sharing the password, and today only the CLI `fsa token`, which needs the server's private key file, can mint one.

## Evidence
- `fsa token` signs with the private key file (`cli.rs:101-111`); `IdentityResolver` accepts only verified RS256 JWTs (`identity.rs`).
- Git HTTP basic and bearer auth exist and use the same resolver (`git/http/`).

## Notes
- Depends on BL-0184 (accounts, sessions).
- Security: hash with a fast keyed hash (high entropy token), a visible prefix to find it, constant time compare, revoke takes effect at once, rate limit on failures, audit of creation and use, a disabled account kills its tokens, no token in logs.
- Keep the external JWT mode working unchanged.
- Decide in its spec: token format, maximum lifetime, scopes (read only, per project) or none for the first version.
