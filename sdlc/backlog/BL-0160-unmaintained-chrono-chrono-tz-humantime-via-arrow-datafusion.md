---
Id: BL-0160
Title: Drop the unmaintained chrono, chrono-tz and humantime once arrow and datafusion stop pulling them
Nature: DEBT
Source: SPEC-0097 closing gate (cargo audit), validated by the owner on 2026-11-18
Created: 2026-11-18
Decision: the three advisories are accepted in `.cargo/audit.toml` and `deny.toml` until upstream moves
---

## What
`cargo audit` flags three crates as unmaintained: `chrono` 0.4.45 (RUSTSEC-2026-0543), `chrono-tz` 0.10.4 (RUSTSEC-2026-0555) and `humantime` 2.4.0 (RUSTSEC-2026-0542). They are warnings, not vulnerabilities.

## Why
Unmaintained crates get no security fixes. We cannot remove them alone: `chrono` is used by our code and by `arrow`; `chrono-tz` comes only from `arrow-array`; `humantime` only from `object_store`, used by `datafusion` (pinned to a git revision, BL-0089). Moving our own code off `chrono` would not clear any of the three.

## Evidence
- `cargo tree -i chrono --all-features`: `agent`, `arrow-arith`, `arrow-array` and the other `arrow-*` crates.
- `cargo tree -i chrono-tz --all-features`: `arrow-array` only.
- `cargo tree -i humantime --all-features`: `object_store` 0.14.2, then `datafusion` 55.1.0.

## Notes
Exit condition: at each `arrow` / `datafusion` / `object_store` upgrade (and each move of the BL-0089 pin), rerun the three `cargo tree -i` commands. When a crate no longer appears, remove its id from `.cargo/audit.toml` (and from `deny.toml` for `chrono`, the only direct one) and the expected list of `accepted_advisories_are_recorded_once_with_a_reason` (`crates/mcp-fs/tests/security_gate.rs`). Once arrow no longer needs `chrono`, move our own code (about 10 files in `crates/core` and `crates/agent`) to a maintained date crate so the last one goes too.
