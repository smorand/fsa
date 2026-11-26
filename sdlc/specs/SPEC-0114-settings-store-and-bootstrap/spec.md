# Generic settings store, bootstrap rule and the scalar sections: functional specification

> Id: SPEC-0114
> Nature: FEAT
> Depth: L
> Depth evidence: FEAT / depth L: 5 areas (store and rules, the three admin tools plus REST plus screen, boot overlay and seed, bootstrap by database URL, eight sections), 17 requirements, one new table, three new tools, one new dependency free. Auto run 2, part 3 of 6 (BL-0173 part 1/4; parts 2 to 4 are BL-0179, BL-0180, BL-0181).
> Status: implemented
> Generated on: 2026-11-26
> Project: existing
> From backlog: BL-0173
> Depends on: SPEC-0102, SPEC-0106
> Security: secrets (write only, encrypted under FSA_TOKEN_KEY, never returned, never logged), admin only access
> CVSS: not scored
> Affected: n/a
> Fixed in: n/a

## 1. Summary
Eight sections of the YAML configuration (`safety`, `api`, `extract`, `doc`, `sqlite`, `db`, `doc_service`, `search`) become settings a platform administrator reads and changes through three tools, REST routes and a browser screen, stored in the admin database with their secrets encrypted. The YAML stays an optional seed and the fallback: a section with a stored value takes the stored value, a section with none takes the YAML or the default. The server can start with nothing but `--database-url` (the admin store) and `FSA_TOKEN_KEY`.

## 2. Current State (MANDATORY)
`ServerConfig` (`config.rs:1114`) holds 14 sections read once from a YAML file by `ServerConfig::load` (`config.rs:1133`); `cmd_serve` (`cli.rs`) refuses to start without a file (`load_config`). `build_with_env` (`app.rs:89`) builds the admin store from `config.infra.admin`, then everything else from the immutable `Arc<ServerConfig>`. The SPEC-0102 registry already keeps providers, models, converters and a record of changes (`setting_change`) in the admin store, with secrets under `FSA_TOKEN_KEY` (`settings/secrets.rs`). The only secret of the eight sections is `doc_service.api.auth_token` (`config.rs:1060`); the embedding, reranking and OCR keys are names of environment variables.

### 2.1 What users can do today
- Change a limit, a mode or a switch of these eight sections: edit the YAML and restart (14.1 row 1).
- Start the server without a YAML file: impossible (14.1 row 2).
- See which values the server runs on: read the file (14.1 row 3).

### 2.2 Specs governing this area
SPEC-0102 (registry, secrets, record of changes, admin screen conventions), SPEC-0106 (admin tools, screen), SPEC-0045 and SPEC-0100 (the validated keys), SPEC-0097 (web lists, part 2).

### 2.3 What is verified today
Each section by its own config tests (`config.rs` tests, `e2e_0100_`, `e2e_0089_`); the registry by `e2e_0102_`. No test boots from the database.

## 3. Scope
### 3.1 In scope
- A store of one row per section, a rule set (known sections, known keys, today's YAML checks, secrets), three tools, REST routes, a screen, the change record.
- The overlay at boot, the YAML seed (once), the bootstrap by `--database-url` / `FSA_DATABASE_URL`.
- The eight sections above, all applied at the next start, flagged `restart_required`.
### 3.2 Out of scope, and why
- `server`, `auth`, `git`, `web`, `context7`, `infra`: parts 2 to 4 (BL-0179, BL-0180, BL-0181). Until part 3, a database only start still needs auth keys from a YAML or the defaults.
- Applying a change without a restart: the eight sections are read once at boot by their consumers; a live reload is a separate piece of work per consumer (AUTO-038).
- The OCR, embedding and reranking key variables stay environment variable names (no new secret store for them).
### 3.3 Constraints imposed by the context
YAML alone behaves as today. The tool contract grows by three tools (155 non search). The existing config tests are not modified.

## 4. Actors
| Actor | Role |
|---|---|
| Platform administrator | lists, reads and changes the sections |
| Other member | refused with `ERR_FORBIDDEN` |
| Operator | starts the server with a database URL and the master key |

## 4.5 Bounded contexts
Platform settings (SPEC-0102 registry), server boot, MCP tools, REST admin plane, browser screens.

## 5. Usage scenarios
### 5.1 Happy
- **S1** The administrator sets `safety.max_read_lines` to 500: the answer says `restart_required: true`; after a restart the server reads 500.
- **S2** The administrator sets the doc service token: the answer and every later read show `has_value: true` and the last 4 characters, never the token; the converter receives the token after a restart.
- **S3** The operator starts with only `--database-url` and `FSA_TOKEN_KEY`: the server reads the stored sections and serves.
- **S4** The first start from a YAML file imports the eight sections once; editing the YAML later does not change the database values.
- **S5** The administrator edits a section on `/app/settings` and sees the change in the record.
### 5.2 Failures
- **F1** A value that today's YAML check refuses (for example `safety.max_diff_file_bytes: 0`, `search.mode: nope`) is refused with the same message and nothing is stored.
- **F2** A non administrator is refused `ERR_FORBIDDEN` on every tool, route and the screen.
- **F3** A stored secret and a missing or other master key: the boot stops with a message naming the section.
- **F4** An unknown section, an unknown key or a value of the wrong type is refused naming the section.

## 6. Requirements
- **FR-NEW-001** Table `server_setting(section, value, secret_enc, updated_at, updated_by)`, primary key `section`, in the admin schema; one row per section; `value` is the JSON of the section without its secrets.
- **FR-NEW-002** Sections: `safety`, `api`, `extract`, `doc`, `sqlite`, `db`, `doc_service`, `search`. Any other name: `ERR_INVALID_ARGUMENT`, `unknown setting section 'x', expected one of ...`.
- **FR-NEW-003** `admin.list_settings`: every section with `source` (`database`, `yaml` or `default`), `restart_required`, `updated_at`, `updated_by`.
- **FR-NEW-004** `admin.get_setting(section)`: `value` (secrets absent), `secrets` (path to `{has_value, last4}`, last4 empty under 8 characters), `source`, `restart_required`, `updated_at`, `updated_by`. The value of a section never set is the one the server runs on.
- **FR-NEW-005** `admin.set_setting(section, value)`: `value` is an object merged over the current section (objects merge, other values replace); an unknown key at any depth or a value of the wrong type is refused naming the section; the merged section must pass `ServerConfig::validate` (today's messages); a secret key set to a string stores it encrypted, set to `""` clears it, absent keeps it; refused without a master key when a secret is given. One transaction holds the row and the change record.
- **FR-NEW-006** All three tools, their REST routes and the screen are for platform administrators, reading included.
- **FR-NEW-007** REST: `GET /api/admin/settings`, `GET` and `PUT /api/admin/settings/{section}`, same bodies and refusals as the tools.
- **FR-NEW-008** Screen `/app/settings`: one page listing the sections, one form per section (JSON patch, same checks), session cookie and CSRF like `/app/models`, no secret rendered.
- **FR-NEW-009** At boot, after the admin store is open and the master key read: each section with a row replaces the section of the configuration (secrets decrypted); a section with no row keeps the YAML or default value.
- **FR-NEW-010** When the configuration came from a YAML file, each section with no row is imported once (secret encrypted); a section holding a secret is not imported when no master key is set (the YAML value stays in force, one warning). A YAML section that differs from a stored one logs one warning, the database wins.
- **FR-NEW-011** `fsa serve --database-url URL` (or `FSA_DATABASE_URL`) sets the admin store (`postgres://` or `postgresql://` is PostgreSQL, `sqlite:PATH` is SQLite); the config file becomes optional when it is given; it wins over the file's `infra.admin`.
- **FR-NEW-012** A stored secret and no master key, or a master key that cannot decrypt it, stops the boot naming the section; a malformed `FSA_TOKEN_KEY` stops the boot (unchanged).
- **FR-NEW-013** A secret is never in a log line, an answer, a refusal, a `Debug` print or the change record; the record holds the changed key names only.
- **FR-NEW-014** `restart_required` is true when the stored section differs from the one the process runs on.
- **FR-NEW-015** The flags `--sqlite`, `--db`, `--doc` still force the switch on over a stored value.
- **FR-NEW-016** The rows are copied by `fsa migrate` (schema driven).
- **FR-NEW-017** The same behavior on SQLite and PostgreSQL.

## 7. Failure and edge tests (derived, S-06)
Per scenario: invalid, missing dependency, repeated or concurrent action. Listed in 14.2.

## 14. Verification
### 14.1 Current behavior (evidence)
1. `cli.rs` `load_config`: no file, no start. 2. `ServerConfig::from_yaml` is the only reader. 3. no tool reads the configuration.
### 14.2 Tests
| Id | Covers | Test |
|---|---|---|
| E2E-001 | S1 FR-005 FR-014 | set one scalar, answer restart_required, restart reads it |
| E2E-002 | S2 FR-005 FR-013 | set the doc service token, has_value and last4, never the value, record holds names |
| E2E-003 | S3 FR-009 FR-011 | boot with only the database URL and the key reads the stored sections |
| E2E-004 | S4 FR-010 | yaml seed imports once, a later YAML edit does not override |
| E2E-005 | S5 FR-008 | screen lists, applies a patch, shows the change |
| E2E-006 | F1 FR-005 | today's YAML check message, nothing stored (three keys) |
| E2E-007 | F2 FR-006 | non admin refused on the three tools, the routes and the screen |
| E2E-008 | F3 FR-012 | stored secret with no key, with another key: boot stops naming the section; malformed key stops |
| E2E-009 | F4 FR-002 FR-005 | unknown section, unknown key, wrong type name the section |
| E2E-010 | FR-003 FR-004 | list and get shapes, source database, yaml, default |
| E2E-011 | FR-005 | concurrent sets: last write wins, both recorded |
| E2E-012 | FR-013 | Debug, logs, answers, refusals and the record never hold the secret |
| E2E-013 | FR-005 | each of the eight sections round trips (get then set the same value changes nothing) |
| E2E-014 | FR-016 | value survives `fsa migrate` |
| E2E-015 | FR-015 | forced switch wins over a stored false |
| E2E-016 | FR-010 FR-009 | today's YAML alone behaves identically (a boot from YAML equals the YAML config on the eight sections) |
| E2E-017 | FR-011 | database URL parsing, flag over env over file, no file and no URL still refused with today's message |
| E2E-018 | FR-017 | the store, rules and boot on PostgreSQL |
| E2E-019 | FR-007 | REST get, put, refusals equal the tools |
