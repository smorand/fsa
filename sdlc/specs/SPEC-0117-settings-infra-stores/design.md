# SPEC-0117: Infra store settings in the database, design

> Spec: sdlc/specs/SPEC-0117-settings-infra-stores/spec.md
> Nature: FEAT
> Depth: M
> Security: stored DSNs and S3 keys are write only secrets (see DEC-002, DEC-005)
> Designed on: 2026-11-27
> Target tier: 1

## 1. Stack and skills
Existing stack, skill `rust`. No new dependency (S-22). No new table, no new tool: the `server_setting` table and the three tools of SPEC-0114 are reused (S-2B).

## 2. Approaches
### Chosen: one more section on the SPEC-0114 mechanism, plus one module of change time checks
`SECTIONS` grows to fourteen; `section_json` and `with_section` get an `infra` arm over a four block view (`InfraStores`); `secret_paths("infra")` names the five secrets; the boot overlay and the seed iterate `SECTIONS` already. What is new lives in `settings/infra_rules.rs` (static blob rules) and `settings/infra_check.rs` (probes and data checks), both called from `ServerSettings::try_set` after `check()` for the `infra` section only.
### Rejected: put the blob rules in `ServerConfig::check()`
It would refuse YAML files that start today (unknown backend only failed at first use). The rules are change time rules (FR-NEW-004, AUTO-068).
### Rejected: have the server run the migration or copy the blobs on change
Moving data is irreversible on real data (S-90). The server verifies and refuses.
### Rejected: a "migrated" marker row
A marker can lie (data written after the copy). Comparing the stores is the proof, and needs no state.

## 3. Architecture
```
set_setting(infra) -> try_set -> names/types -> check_keys (admin unknown) -> merge -> secrets -> with_section
   -> candidate.check()                       (validate_store messages, today's)
   -> infra_rules::blob(candidate)            (change only)
   -> infra_check::run(running, candidate, admin)   probes, then data checks, order meta, git, oauth, blob
   -> row + record (one tx)
boot: build_with_env -> apply: rows replace sections (infra keeps config.infra.admin) -> check() -> stores built from the result
```

## 4. Data model
None new. The `infra` row is the JSON of `InfraStores {meta, blob, git, oauth}` with the five secret paths removed and held in the encrypted secret blob of the row.

## 5. Contracts and interfaces
- `config.rs`: `InfraStores` (serde view, `deny_unknown_fields`), `InfraConfig::stores()` and `InfraConfig::with_stores(self, InfraStores)` (keeps `admin`); `BlobConfig` hand written `Debug` (keys as `set` or `unset`).
- `settings/server_settings.rs`: `SECTIONS: [&str; 14]`; arms; `secret_paths("infra")`; `last4` empty for a path ending `.dsn`.
- `settings/infra_rules.rs`: `blob_rules(&ServerConfig) -> Result<()>` messages of spec FR-NEW-004.
- `settings/infra_check.rs`: `pub(crate) static PROBE_TIMEOUT_MS: AtomicU64` (15000), `redact(text, &[secrets])`, `async fn run(running: &ServerConfig, candidate: &ServerConfig, admin: &dyn AdminBackend) -> Result<()>`; relational moves use `storage::open_{meta,git,oauth}_db` on a private `RelationalRegistry`, `migrate::{count_rows, shas_of}` (made `pub(crate)`); blob probe `S3BlobStore::for_bucket(cfg, bucket)` (a constructor over `new`); blob existence through `build_blob_store(candidate, project)`.
- `settings/server_settings_boot.rs`: unchanged shape; `with_section` keeps `admin`; the YAML seed imports `infra` through `yaml_importable` default arm.
- Tool descriptions and the contract: the section list of fourteen (spec 14.2).

## 6. File structure impact
`config.rs` (view, Debug), `settings/server_settings.rs`, new `settings/infra_rules.rs`, `settings/infra_check.rs`, `migrate.rs` (visibility), `storage/blob/s3.rs` (constructor), `tools/settings.rs`, `mcp/server.rs` (descriptions), tests `settings/server_settings10_e2e.rs` (US-0354), `settings/server_settings10_check_e2e.rs` (US-0355), `settings/server_settings10_boot_e2e.rs` (US-0356), `settings/server_settings10_pg_e2e.rs` (US-0357), docs, `TOOL_CONTRACT.txt`, golden.

## 7. Compatibility and risks
YAML alone behaves as today: nothing new in `check()`; `InfraConfig` is not serialized anywhere else (checked with `rg`). Risk: the seed writes an `infra` row at the first start of a YAML deployment, so every later start reads the database: a deployment that edits its YAML infra afterwards is ignored, with the SPEC-0114 warning. Risk: a probe on SQLite creates a project database file at the new place (empty schema); stated in FR-NEW-005. Risk: the S3 existence request on a path style endpoint needs the bucket; a stand in answers any path.

## 8. Security design
Five secrets under the SPEC-0114 secret machinery; the DSN `last4` is always empty; errors from drivers pass through `redact` (every DSN, key and the endpoint userinfo replaced by `<redacted>`); `BlobConfig` Debug redacts; no environment variable is read (a DSN is a value). The probe sends no credential anywhere but to the endpoint the administrator typed: this is the destination class of AUTO-059 (an administrator choosing where a stored secret is sent), kept and reported, not restricted here.

## 9. Test strategy
`e2e_0117_NNN` in the four files above, written first and run RED on the unchanged production code, per story. The S3 stand in is a `tokio` TCP listener answering each request with a chosen status and counting requests by path. PostgreSQL tests (009, 027, 055, 060 to 062) skip without `MCPFS_TEST_PG_DSN` and need `--features postgres`; real clock.

## 10. Decisions log
- DEC-001 (S-2B) one section view `InfraStores` without `admin`: `check_keys` refuses `admin` by itself and the admin block cannot be overwritten.
- DEC-002 (S-26) the DSNs are secrets as a whole text: a DSN carries a password; a DSN of a SQLite store is empty.
- DEC-003 (S-90) the checks compare, never copy; equality in both directions refuses.
- DEC-004 (S-10) restart flagged, no live switch.
- DEC-005 (S-26) `last4` of a DSN is the empty text.
- DEC-006 (S-05) the probe bucket is `<prefix>probe`, never created.

## 11. Stories
| Story | Covers | Tier |
|---|---|---|
| US-0354 the section, its secrets and the static rules | FR-NEW-001 to 004, 009 (refusals), 014, 016 | 1 |
| US-0355 probes and data checks | FR-NEW-005 to 010 | 1 |
| US-0356 the start, the seed and migrate | FR-NEW-011 to 013, 015 | 1 |
| US-0357 PostgreSQL, docs and contract | FR-NEW-017 | 1 |

### Test to story map
US-0354: E2E-001 to 008, 010 to 014, 016, 017, 056, 059.
US-0355: E2E-015, 020 to 026, 028, 030, 040 to 047, 057, 058.
US-0356: E2E-029, 031, 048 to 054.
US-0357: E2E-009, 027, 055, 060, 061, 062 and the regenerated golden, the docs.

## 12. Drift notes (implementation)
- DRIFT-001 The `Fx` platform holds `/a.txt` in `proj` (one blob), so E2E-002 changes keys that move nothing (region, prefix of a local store), E2E-005 and 012 give the S3 stand in the answer "present" (the blob is there), and the no data cases (023, 042, 046) boot a platform with `server_settings_e2e::boot`. E2E-020 asserts `nodes <n> here, 0 there` with the real count.
- DRIFT-002 `PROBE_TIMEOUT_MS` and the probe counter are thread locals, not atomics: the tests run one runtime per thread and must not lower the timeout of their neighbours.
- DRIFT-003 E2E-006 searches the log for the secret key only: the AWS SDK prints its credentials provider (the access key id) at TRACE level, third party and unchanged.
- DRIFT-004 E2E-055 uses a DSN that differs by a connection parameter (no second role needed).
- DRIFT-005 US-0356 adds no production code: the SPEC-0114 overlay and seed already serve the section; its tests are guards shown by a mutation (the overlay skipping `infra`: 9 of 10 fail).
- DRIFT-006 E2E-058 is a unit level test of `redact`: the drivers do not echo a password, so the integration tests 009, 027 and 062 pass without redaction (defense in depth).
