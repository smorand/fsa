# SPEC-0114: Generic settings store, bootstrap rule and the scalar sections, design

> Spec: sdlc/specs/SPEC-0114-settings-store-and-bootstrap/spec.md
> Nature: FEAT
> Depth: L
> Security: secrets
> Designed on: 2026-11-26
> Target tier: 1

## 1. Stack and skills
Existing stack, skill `rust`. No new dependency (S-22).

## 2. Approaches
### Chosen: one JSON row per section, validated by the real `ServerConfig::validate`, overlaid at boot
`server_setting(section PK, value TEXT json, secret_enc BLOB, updated_at, updated_by)`. A section is read and written as the JSON of its serde struct, so the YAML names are the setting names and a round trip is exact. A set merges the patch over the current section JSON, rejects keys absent from the section's default JSON, deserializes into the struct (wrong type: serde error naming the section) and runs `ServerConfig::validate` on a candidate configuration: today's messages, no second rule set (DEC-001). Secrets are declared per section (`doc_service`: `api.auth_token`), removed from `value` and stored as one AES 256 GCM blob of a JSON object under `FSA_TOKEN_KEY`.
Pros: one implementation of every rule, parts 2 to 4 add a section by one table entry. Cons: a key added to a struct later is accepted at once (the default JSON drives it, good).
### Rejected: one row per leaf key
Lists (`doc_service.extensions`, `cli.command`) and nested blocks would need an encoding and a second validator.
### Rejected: a live reload
Every consumer (`SafetyManager`, the search backend, the doc service) captures its section at build; a reload is a change per consumer, none is required by an approved scenario (DEC-004).
### Rejected: YAML wins over the database
Contradicts the approved decision "a value in the database wins".

## 3. Architecture
```
boot: cli serve (--database-url, file optional) -> ServerConfig (yaml or default) -> build_with_env
  admin open, master key -> settings::server::apply(config, store, key) : rows replace sections, seed missing (yaml origin only)
  -> Arc<ServerConfig> -> everything else unchanged
admin.set_setting -> tools::settings -> settings::ServerSettings (rules, one tx: row + setting_change)
```

## 4. Data model
Table `server_setting` in the admin schema (`CREATE TABLE IF NOT EXISTS`, schema fingerprint goldens regenerated, additive). Change record: `setting_change` with `target_kind` `setting`, `target_id` and `target_name` the section, `action` `set_setting`, `fields` the dotted key paths changed (secret paths named, never valued).

## 5. Contracts and interfaces
Tools `admin.list_settings`, `admin.get_setting(section)`, `admin.set_setting(section, value)`; REST `/api/admin/settings[/{section}]`; screen `/app/settings` (+ `/app/settings/set`); CLI `serve --database-url` (env `FSA_DATABASE_URL`). `ServerConfig` gains two serde skipped fields: `from_yaml_file` (seed allowed) and `forced` (the switches forced by flags).

## 6. Decisions
- DEC-001 (S-2B) validation is the real `validate()` on a candidate configuration.
- DEC-002 (S-26) secret keys are removed from the stored value; `get` shows `has_value` and `last4` (empty under 8 characters, so a short secret is not half shown).
- DEC-003 (S-05) a secret key set to `""` clears it; absent keeps it.
- DEC-004 (S-10) every section of this part is applied at the next start and flagged; no live reload.
- DEC-005 (S-24) seeding only from a loaded YAML file; a programmatic configuration (tests) is never seeded. A YAML section differing from the stored one warns once at boot.
- DEC-006 (S-05) `--database-url` accepts `postgres://`, `postgresql://`, `sqlite:PATH`; it replaces `infra.admin` (backend, dsn or path).

## 7. Files touched
`storage/admin.rs` (table), `settings/{mod,server_settings,server_settings_store,server_settings_rules}.rs`, `tools/settings.rs`, `tools/mod.rs`, `mcp/server.rs`, `api/admin_settings.rs`, `settings_screen.rs`, `app.rs`, `cli.rs`, `config.rs`, tests `settings/server_settings_e2e.rs`, `server_settings_pg_e2e.rs`; docs README.md, AGENTS.md, `.agent_docs/{config,tools,api,architecture,testing}.md`, `TOOL_CONTRACT.txt`, `tool-contract-golden.json`.

## 8. Stories
- US-0341 the store and rules: table, get, set, list, secrets, record (FR-001 to 005, 013, 014; E2E-001, 002, 006, 009 to 013, 016 data part).
- US-0342 boot: overlay, seed, `--database-url`, forced flags, key refusal (FR-009 to 012, 015; E2E-003, 004, 008, 015, 016, 017).
- US-0343 tools, REST, screen, access (FR-003 to 008; E2E-005, 007, 019).
- US-0344 migrate, PostgreSQL, docs and contract (FR-016, 017; E2E-014, 018).

## 9. Risks
- Seeding writes rows on a first YAML start; a test or deployment that edits the YAML afterwards sees the database win. Mitigated by the warning and the docs.
- Defaults inside a struct change between versions: a stored full section freezes today's default of keys the operator never touched. Accepted, same as any seed.
