# SPEC-0107: Per project indexing profile, design

> Spec: sdlc/specs/SPEC-0107-per-project-indexing-profile/spec.md
> Nature: FEAT
> Depth: M
> Security: none
> Designed on: 2026-11-25
> Target tier: 1

## 1. Stack and skills
Existing stack (Rust 2024, axum, rmcp, rusqlite and sqlx). No new dependency (S-22). Skill `rust` (already in context). Gate: `make check` equivalents named in AGENTS.md.

## 2. Approaches
### Chosen: columns on the project row, read and written through the settings registry
Same pattern as SPEC-0106 `project.converter_id`: five nullable columns, `impl Registry` methods in a new `settings/profile.rs`, SQL in `settings/profile_store.rs`, two admin tools, a section on the models screen. Pros: no new table, a hard removal deletes the profile with the row, volume isolation by project id. Cons: the project row grows. Best when the profile is small and per project.
### Rejected: a `project_profile` table
A join and a second lifecycle for four fields (DEC-001).
### Rejected: JSON blob column for the whole profile
Cannot count model usage with an indexed equality; the delete guard needs `WHERE text_model_id=?` (DEC-001).

## 3. Architecture
```
 mcp/server.rs (2 #[tool])  ->  tools/profile.rs (auth, JSON)  ->  settings::Registry (profile.rs)  ->  profile_store.rs  ->  project row
 models_screen.rs  ->  tools/profile.rs::list_profiles (admin)
 Registry::try_delete_model  ->  profile_store::count_model_projects
```

## 4. Data model
Table `project`, five nullable columns added by `ColumnMigration`: `chunk_strategy` text, `chunk_settings` text (JSON object as requested), `text_model_id` text, `image_model_id` text, `vectors_stale_at` text (RFC 3339 UTC seconds). Additive; an older binary ignores them. The `production_ddl_is_unchanged_on_every_engine` fingerprints change once.

## 5. Contracts and interfaces
- `admin.get_indexing_profile(project_id)`; `admin.set_indexing_profile(project_id, chunk_strategy?, chunk_settings?, text_model_id?, image_model_id?)` (empty text clears; `chunk_settings` an object; clearing the strategy clears its settings).
- `Registry::project_profile(project_id) -> Profile`, `Registry::set_project_profile(person, project_id, ProfileChange) -> Profile`, `Registry::list_profiles()` for the screen.
- Fallback text model: server default from the registry, else `config.search.embedding.model` with an endpoint set.
- Record: action `set_indexing_profile`, target kind `project_profile`.

## 6. Decisions
- DEC-001 (S-2B): columns on `project`, see 2. Obliged by FR-NEW-001, FR-NEW-010, FR-NEW-014.
- DEC-002 (S-10): chunk checks reuse `chunking::service::Requested::parse` with `config.search.chunking()`; the stored value is the settings as requested, the filled view is computed on read. Obliged by FR-NEW-004.
- DEC-003 (S-05): empty text clears; a strategy change without settings clears the stored settings so a stale `size_tokens` never meets `structural`. Obliged by FR-NEW-006.
- DEC-004: one transaction reads the project row, checks models, writes one UPDATE of all five columns, records, commits; `run` retry as the converters do. Obliged by FR-NEW-002, FR-NEW-007.
- DEC-005 (S-10): no REST route and no screen form; the screen shows. Obliged by spec 3.2.
- DEC-006: out of date is `vectors_stale_at` only; part 2 clears it when vectors are rebuilt. Obliged by FR-NEW-007.

## 7. File structure impact
| Path | Action | Story |
|---|---|---|
| `crates/core/src/storage/admin.rs` | modify: five column migrations | US-0322 |
| `crates/core/src/settings/profile.rs`, `profile_store.rs` | create | US-0322 |
| `crates/core/src/settings/profile_e2e.rs` | create: 15 tests | US-0322, US-0323 |
| `crates/core/src/settings/mod.rs`, `registry.rs` | modify: module, delete guard | US-0322, US-0323 |
| `crates/core/src/tools/profile.rs`, `tools/mod.rs`, `tools/admin.rs`, `tools/all.rs` | create and modify: tools, test registry | US-0322 |
| `crates/core/src/mcp/server.rs` | modify: two tools, args | US-0322 |
| `crates/core/src/models_screen.rs` | modify: profiles section | US-0323 |
| `TOOL_CONTRACT.txt`, `tool-contract-golden.json`, `README.md`, `AGENTS.md`, `.agent_docs/*` | modify | US-0323 |

## 8. Stories
| Story | Title | Tier | Depends |
|---|---|---|---|
| US-0322 | profile storage, rules and the two tools | 1 | none |
| US-0323 | delete guard, models screen, contract and docs (closes spec) | 2 | US-0322 |

## 9. Risks
Schema fingerprint tests need their hash updated; tool count assertions in about ten places.
