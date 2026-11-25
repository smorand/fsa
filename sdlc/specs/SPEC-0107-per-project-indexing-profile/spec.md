# Per project indexing profile: functional specification

> Id: SPEC-0107
> Nature: FEAT
> Depth: M
> Depth evidence: FEAT / depth M: 3 areas (profile storage and rules, admin tools, models screen), 14 requirements, 1 new persisted concept (the profile, columns of the project row). Auto run, part 1 of 5 (BL-0170); later parts (vectors, search, file metadata screen) are out of this spec.
> Status: implemented
> Generated on: 2026-11-25
> Project: existing
> From backlog: BL-0170
> Depends on: none
> Security: none
> CVSS: not scored
> Affected: n/a
> Fixed in: n/a

## 1. Summary
Today one embedding model, set in the configuration file or as the server default of the model registry, serves every project, and the default chunk strategy is a server wide setting. This increment gives each project an indexing profile: the default chunk strategy with its settings, the text embedding model and the image embedding model, each model chosen from the registry. The owner of a project or a platform administrator reads and changes it with two admin tools; the models screen shows it. Changing a model marks the vectors of the project as out of date, because a vector from one model cannot be compared with one from another. A project with no profile keeps behaving as today.

## 2. Current State (MANDATORY, every depth)
The platform has a model registry (providers, models of kinds `text_embedding` and `image_embedding` with a dimension, server defaults per kind) edited by platform administrators, and a configuration block `search.embedding` used when the registry has no default. A project has an `index_mode` and a converter; it has no model and no chunk choice of its own. Chunk sets are built with the server wide chunk defaults of the configuration. The vector width is one setting for the whole platform. Nothing records which model a project uses, so nothing can say its vectors are out of date.

### 2.1 What users can do today
- Choose the search index mode of a project (14.1 row 1).
- Choose one embedding model for the whole platform (14.1 row 2).
- Nothing lets a project name its own models or chunk strategy (14.1 row 3).

### 2.2 Specs governing this area
- SPEC-0102 (settings registry): models, kinds, defaults, record of changes, `/app/models`; reused and extended (deleting a used model is refused; the record gets one new target kind).
- SPEC-0101 (chunk sets): the strategies and the setting checks; reused, not changed.
- SPEC-0106 (converters): the per project column pattern; reused.

### 2.3 What is verified today
`e2e_0102_` tests cover the registry, the screen and the record; nothing covers a per project model or chunk choice (14.1 row 4).

## 3. Scope
### 3.1 In scope
- A profile per project: chunk strategy and settings, text model, image model.
- `admin.get_indexing_profile` and `admin.set_indexing_profile`.
- The out of date marker of the vectors when a model changes.
- Refusal to delete a model a profile uses.
- The profile shown on `/app/models`.
- The record of changes for a profile change.
### 3.2 Out of scope, and why
- Building vectors, vector storage per dimension, recalculation: BL-0171 (part 2).
- Using the profile in search or in building the default chunk set: parts 2 to 4; this part stores and exposes it.
- A REST route for the profile: no requirement asks for it (YAGNI).
- Editing the profile from the screen: the screen shows it; the tools change it.
### 3.3 Constraints imposed by the context
- Tool count moves from 146 to 148 and the tool contract is regenerated.
- The `project` table gains nullable columns through `ColumnMigration`, so an upgrade needs no manual step.
- No secret appears in any answer, record or log (SPEC-0102 rule).

## 4. Actors
| Actor | Role |
|---|---|
| Owner | owner of a project; reads and sets its profile |
| Member | reads the profile |
| Platform administrator | reads and sets the profile of any project; manages models |
| Outsider | a person with no part in the project; refused |

## 4.5 Bounded contexts
Settings (registry and profile) only. The profile lives on the project row and is read through the registry.

## 5. Usage scenarios
### SC-001: Choose the models of a project
Bob owns `proj`. He sets the text model and the image model; reading the profile shows both with provider, kind and dimension, and the vectors marked out of date.
### SC-002: Choose the chunk strategy
Bob sets strategy `fixed` with `size_tokens` 512; reading shows the requested settings and the ones the server fills in.
### SC-003: Fall back and clear
A project with no profile reads the server default text model, else the configuration model, with the source named. Bob clears a field with an empty text; the fallback returns.
### SC-004: See the profile on the screen
The platform administrator opens `/app/models` and sees, per project, the chunk strategy, the two models and the out of date marker, with every value escaped.
### SC-005: Refusals
An invalid strategy or setting, an unknown model, a model of the wrong kind, a person who is not the owner, an unknown project: each is refused with a plain text and changes nothing.
### SC-006: Repeat and concurrency
Setting what is already set changes and records nothing. Two concurrent sets leave one whole profile.
### SC-007: A used model stays
Deleting a model a profile uses is refused until the profile no longer names it.
### SC-008: Upgrade and restart
A platform upgraded from before this increment reads every project with an empty profile; a profile survives a restart and an offline migration.

## 6. Functional requirements
### 6.1 New
- FR-NEW-001: The platform SHALL store for each project an indexing profile made of a chunk strategy with its requested settings, a text embedding model and an image embedding model, each optional, none set for an existing or new project.
- FR-NEW-002: WHEN the owner or a platform administrator calls `admin.set_indexing_profile` THE platform SHALL change only the fields given, validate them, and answer the whole profile as `admin.get_indexing_profile` does.
- FR-NEW-003: WHEN a member or a platform administrator calls `admin.get_indexing_profile` THE platform SHALL answer `project_id`, `chunking` (`strategy`, `settings` as requested, `filled` with the server defaults, `source`), `text_model` and `image_model` (each `null` or the model with `model_id`, `name`, `provider`, `kind`, `dimension`, `source`), `vectors_out_of_date` and `vectors_out_of_date_since`.
- FR-NEW-004: THE platform SHALL accept as chunk strategy and settings exactly what `fs.create_chunk_set` accepts, and answer `ERR_INVALID_ARGUMENT` with the same text otherwise; settings without a strategy given or stored are refused with `chunk_settings need a chunk_strategy`.
- FR-NEW-005: THE platform SHALL accept as `text_model_id` a model of kind `text_embedding` and as `image_model_id` a model of kind `image_embedding`; an unknown id is `ERR_NOT_FOUND` `model not found: <id>`; another kind is `ERR_INVALID_ARGUMENT` `model '<name>' is not a text_embedding model` (or image_embedding).
- FR-NEW-006: An empty text for a field SHALL clear it; a field not given SHALL keep its value.
- FR-NEW-007: WHEN a model field takes a value different from the stored one THE platform SHALL set the vectors of the project out of date with the time; setting the stored value again, or changing only the chunk fields, SHALL NOT change the marker.
- FR-NEW-008: WHEN the text model of a project is not set THE platform SHALL read it as the server default text embedding model (`source` `server_default`), else as the model of `search.embedding` when its endpoint is set (`source` `configuration`, `model_id` null), else `null`; the image model has no fallback; a set one has `source` `project`.
- FR-NEW-009: WHEN a profile changes THE platform SHALL record `set_indexing_profile` with the target kind `project_profile`, the project id and the names of the fields changed, and no value; setting the same values SHALL record nothing.
- FR-NEW-010: WHEN a model is named by the profile of a project (a project in its grace period included) THE platform SHALL refuse `admin.delete_model` with `model '<name>' is used by <n> project(s)`.
- FR-NEW-011: THE platform SHALL refuse `admin.set_indexing_profile` to anyone but the owner or a platform administrator (`ERR_FORBIDDEN`), and either tool for an unknown project (`ERR_NOT_FOUND`); a read by a non member is refused.
- FR-NEW-012: THE models screen SHALL list every project with its chunk strategy, text and image model names, and out of date marker, escaped.
- FR-NEW-013: THE platform SHALL keep a profile over a restart and over `fsa migrate`.
- FR-NEW-014: THE platform SHALL answer no secret, and SHALL keep the registry and project rows of other projects untouched by a change of one project.
### 6.2 Modified
- FR-MOD-001 (SPEC-0102 `admin.delete_model`): refuses a model a profile names (FR-NEW-010); nothing else changes.
### 6.3 Removed
None.

## 7. Non functional requirements (user observable)
### 7.1 Performance
Reading or setting a profile is one database transaction; no new background work.
### 7.2 Security and access
Owner or administrator to set, member or administrator to read; the screen is platform administrator only as today.
### 7.3 Privacy and compliance
No personal data added; the record keeps field names only.
### 7.4 Usability and accessibility
Refusal texts are plain sentences; the screen is plain HTML.
### 7.5 Reliability
A set is atomic: all fields or none. Concurrent sets never leave a mix of two requests inside one field.
### 7.6 Operability
Upgrade adds nullable columns; an older binary ignores them.

## 8. Edge cases
- Strategy given with settings of the other strategy: refused as `fs.create_chunk_set`.
- Strategy changed while old settings are stored: the old settings are replaced (cleared) unless given again.
- A model deleted while a set is in flight: the transaction of the delete checks the usage; the set refuses a missing model.
- A project in its grace period keeps its profile and can be read by an administrator.

## 9. Acceptance tests (the contract)
### 9.1 Summary
Seventeen tests: 4 happy (E2E-001 to 004), 5 failures (E2E-005 to 009), 6 extra (E2E-010 to 015), 2 added by review (E2E-016 concurrent sets of different fields, E2E-017 a stored choice the limits no longer accept). All in `crates/core/src/settings/profile_e2e.rs`, run by `cargo test -p fsa-core --lib -- e2e_0107_`. Driver: the real `McpServer` tool methods, the real screen router, a SQLite admin store reopened for restarts.

### 9.2 New tests
- E2E-001 (SC-001, FR-NEW-002, 003, 005, 007): Given providers and a text and an image model; When Bob sets both; Then the answer shows both with provider, kind, dimension, `source` `project`, and `vectors_out_of_date` true with a time.
- E2E-002 (SC-002, FR-NEW-004, 009): When Bob sets `fixed` with `size_tokens` 512; Then get shows settings `{size_tokens:512}`, filled with `overlap_tokens` 0, and the record holds one `set_indexing_profile` line naming `chunk_strategy` and `chunk_settings`, and the marker stays unchanged.
- E2E-003 (SC-003, FR-NEW-006, 008): Given a server default text model; Then a project with no profile reads it with `source` `server_default`; Bob sets then clears the field with an empty text, and the read returns to the default.
- E2E-004 (SC-004, FR-NEW-012): The platform administrator GETs `/app/models` and finds the project section with the strategy, both model names and the out of date marker; a project named `<script>` style content in a model name is escaped.
- E2E-005 (SC-005, FR-NEW-004): An unknown strategy, a setting out of range, a setting of the other strategy and settings with no strategy are each `ERR_INVALID_ARGUMENT` with the text of `fs.create_chunk_set` (or the FR-NEW-004 text); the profile is unchanged.
- E2E-006 (SC-005, FR-NEW-005): An unknown model id is `ERR_NOT_FOUND`; the profile is unchanged; an unknown project is `ERR_NOT_FOUND`.
- E2E-007 (SC-005, FR-NEW-005): A reranker model, and an image model given as text model, are refused with the kind text; unchanged.
- E2E-008 (SC-005, FR-NEW-011): A member who is not owner and an outsider cannot set (`ERR_FORBIDDEN`); an outsider cannot read; a member can read; the platform administrator can do both.
- E2E-009 (SC-006, FR-NEW-002): Two concurrent sets with different model pairs both succeed; the final profile equals one of the two requests whole.
- E2E-010 (SC-008, FR-NEW-014): A set on one project leaves another project's profile empty.
- E2E-011 (SC-008, FR-NEW-013): A profile survives a restart over the same files and an offline copy by `fsa migrate`; a database created before the columns reads an empty profile.
- E2E-012 (FR-NEW-014): Every answer, refusal and log line of a registry with a bearer secret holds no secret.
- E2E-013 (SC-007, FR-NEW-010): `admin.delete_model` of a used model is refused with the count; after Bob clears the field the delete succeeds.
- E2E-014 (SC-003, FR-NEW-008): With no default model and a configured `search.embedding` model, the text model reads `source` `configuration`; with neither, `null`.
- E2E-015 (SC-006, FR-NEW-007, 009): Setting the same models twice keeps the first marker time and records one line.
### 9.3 Exploit tests
E2E-012 and E2E-004 (escaping) are the literal payload tests.
### 9.4 Modified tests
`tools::admin::every_admin_tool_is_registered` (39 to 41), the registry and catalogue counts (146 to 148), the golden contract and the SPEC-0102 delete model tests stay unchanged in assertions.
### 9.5 Removed tests
None.

## 10. Traceability
| Requirement | Tests |
|---|---|
| FR-NEW-001 | E2E-001, 011 |
| FR-NEW-002 | E2E-001, 009 |
| FR-NEW-003 | E2E-001, 002, 003 |
| FR-NEW-004 | E2E-002, 005 |
| FR-NEW-005 | E2E-001, 006, 007 |
| FR-NEW-006 | E2E-003 |
| FR-NEW-007 | E2E-001, 002, 015 |
| FR-NEW-008 | E2E-003, 014 |
| FR-NEW-009 | E2E-002, 015 |
| FR-NEW-010 | E2E-013 |
| FR-NEW-011 | E2E-006, 008 |
| FR-NEW-012 | E2E-004 |
| FR-NEW-013 | E2E-011 |
| FR-NEW-014 | E2E-010, 012 |
| FR-MOD-001 | E2E-013 |

## 11. Open questions
None. Assumption (S-03): the profile is stored but not yet consumed by search or the default chunk set (parts 2 to 4). Assumption (S-05): an empty text clears a field.

## 12. Glossary (MANDATORY)
| Term | Meaning |
|---|---|
| Indexing profile | the chunk strategy, text model and image model of a project |
| Out of date | vectors built with a model other than the one now chosen |
| Source | where a model comes from: `project`, `server_default`, `configuration` |

## 13. Implementability gate (MANDATORY)
Every requirement has a test; every test names scenario and requirement; no open question; counts 14 requirements and 15 tests are inside the depth M budget. Pass.

## 14. Evidence appendix
### 14.1 Current State evidence
| Row | Evidence |
|---|---|
| 1 | `crates/core/src/storage/admin.rs:219-225` (`index_mode` column), `tools/admin.rs:224-262` |
| 2 | `crates/core/src/config.rs:754-800` (`search.embedding`), `settings/registry.rs:693-730` (`RegistryTargets`) |
| 3 | `rg "text_model|image_model" crates/core/src` finds nothing |
| 4 | `settings/e2e.rs` (e2e_0102_), no profile test |
### 14.2 Class sweep
Per project column pattern: `project.converter_id` (`storage/admin.rs:253-262`, `settings/converter_store.rs:189-210`); model delete guard pattern: `converters.rs:276`.
### 14.3 Design inputs
Reuse `Registry` and `SettingsStore`; reuse `chunking::service::Requested::parse` for the chunk checks.
