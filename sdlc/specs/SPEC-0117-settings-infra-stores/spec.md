# Infra store settings in the database: functional specification

> Id: SPEC-0117
> Nature: FEAT
> Depth: M
> Depth evidence: FEAT / depth M: 3 areas (the section and its secrets, the checks before saving, the start), 17 requirements, no new table, no new tool, no new dependency. Auto run 2, part 6 of 6 (BL-0173 part 4/4; BL-0181).
> Status: implemented
> Generated on: 2026-11-27
> Project: existing
> From backlog: BL-0181
> Depends on: SPEC-0114, SPEC-0115, SPEC-0116, SPEC-0102
> Security: stored DSNs and S3 keys are write only secrets encrypted under FSA_TOKEN_KEY; no ambient environment variable becomes readable; the server never moves data itself
> CVSS: not scored
> Affected: n/a
> Fixed in: n/a

## 1. Summary
The `infra` block of the YAML configuration (the blob store, and the meta, git and oauth relational stores) becomes the fourteenth setting section, read and changed with the three tools, the routes and the screen of SPEC-0114 and applied at the next start. Only the admin store URL (`--database-url`, or `FSA_DATABASE_URL`) and `FSA_TOKEN_KEY` stay outside the database. A change of where data lives is refused until the data is already there: the server checks the new location (it must be reachable and must hold what the current one holds) and never copies anything. The S3 keys and the DSNs are encrypted and never returned.

## 2. Current State (MANDATORY)
Thirteen sections are settings. `infra` is read from the YAML file only. `fsa migrate --from a.yaml --to b.yaml` copies the relational rows offline and reports blobs missing from the destination store; it copies no blob bytes. `ServerConfig::check()` refuses a bad relational store block (`validate_store`) and checks nothing about the blob block. The stores are opened once at the start from the configuration the stored settings produced.

### 2.1 What users can do today
- Point meta, git, oauth or blob at another place: edit the YAML, run `fsa migrate`, restart (14.1 item 1).
- Start with the admin URL and the key alone and get the stores they were set to: impossible, the defaults apply (14.1 item 2).
- Learn whether a new location holds the data before the restart: nothing checks it.

### 2.2 Specs governing this area
SPEC-0114 (store, rules, screen, boot, seed), SPEC-0116 (the secret and no ambient variable stance), SPEC-0051 (removed backends), the migration spec (`migrate.rs`), SPEC-0102 (secrets under the master key).

### 2.3 What is verified today
The store block by the configuration tests (`config.rs`), the migration by the `migrate` tests, the settings store by `e2e_0114_` to `e2e_0116_` (identifiers in 14.1).

## 3. Scope
### 3.1 In scope
- The section `infra` with the keys `meta`, `blob`, `git`, `oauth`, in the tools, routes and screen.
- Five write only secrets: `blob.access_key`, `blob.secret_key`, `meta.dsn`, `git.dsn`, `oauth.dsn`.
- The checks before saving: static rules, reachability, the data already in place.
- The start reading the section, the YAML seed, the survival through `fsa migrate`.
### 3.2 Out of scope, and why
- The admin store (`infra.admin`) and `FSA_TOKEN_KEY`: the bootstrap, they cannot be read from what they open.
- Copying data (relational rows, blob bytes): the server never does it (hard stop S-90). `fsa migrate` copies rows; blob bytes are copied by the operator with the tool of her choice.
- A live switch of a store: every consumer opens its store once, so the section is restart flagged (AUTO-038).
- The pool size keys and the schema name get the checks of the start only.
### 3.3 Constraints imposed by the context
YAML alone behaves as today and the existing configuration tests are not modified. No new check enters `ServerConfig::check()`, so a YAML file that starts today starts the same; the new rules apply to a change made through the settings only.

## 4. Actors
| Actor | Role |
|---|---|
| Platform administrator | reads and changes the section |
| Other member | refused |
| Operator | runs `fsa migrate`, copies blobs, starts the server with the admin URL and the master key |

## 4.5 Bounded contexts
Platform settings (SPEC-0114), storage stores, offline migration, server start.

## 5. Usage scenarios
### 5.1 Happy
- **S1** The administrator reads `infra` on a server never set: the four stores as the server runs them, source `default` or `yaml`, no `admin` key, the five secrets shown as set or not with their last 4.
- **S2** The operator copies the data with `fsa migrate` to a new meta directory; the administrator points `infra.meta.dir` at it: accepted, restart required; after the restart the server serves the data from there.
- **S3** The operator copies the blobs to an S3 bucket set; the administrator switches the blob store from local to S3 with its keys: accepted; the keys are stored encrypted and read back as set with their last 4 only.
- **S4** An operator starts with only the admin URL and the master key: the stores named by the stored section are the ones in use.
- **S5** The first start from a YAML file imports `infra` once; a later YAML edit does not change it.
### 5.2 Failures
- **F1** A store location change while the new place does not hold the data of the current one is refused, the message names `fsa migrate`, nothing is stored.
- **F2** A DSN or an S3 endpoint that cannot be reached, or S3 keys the endpoint rejects, is refused before saving; the message never carries a DSN or a key.
- **F3** The blob store switched from local to S3 while blobs exist that the S3 store does not hold is refused, nothing is stored.
- **F4** A value the start would refuse (unknown backend, postgres without a DSN, sqlite with a DSN) is refused with the start's message; a blob setting that cannot work (S3 without keys) is refused with its own message.
- **F5** A member who is not a platform administrator is refused on the tools, the routes and the screen.
- **F6** A key `admin`, an unknown key or a value of the wrong type is refused naming the section.
- **F7** A wrong or missing master key stops the start when a stored secret exists.
### 5.3 Edge
- **E1** A section equal to the running one, or a change of pool size only, triggers no probe and no data check.
- **E2** A source with no data (a fresh install) moves to any reachable place without a migration.

## 6. Functional requirements
### 6.1 New
- **FR-NEW-001** `infra` is a setting section, the fourteenth and last, after `auth`: the list names, in this order, safety, api, extract, doc, sqlite, db, doc_service, search, git, web, context7, server, auth, infra; the refusal of an unknown section names the same fourteen.
- **FR-NEW-002** The section reads and writes under the YAML names of `meta`, `blob`, `git`, `oauth`; a key not given keeps its value; the value of a section never set is the one the server runs on; there is no key `admin`, and one given is refused as an unknown key.
- **FR-NEW-003** The five secrets are write only: stored encrypted under the master key apart from the value, never returned, never recorded, never in an error message or a log line, redacted in `Debug`; a read shows for each `has_value` and `last4` (the last 4 characters when the secret has at least 8, else the empty text; the key `last4` is present and always the empty text for the three DSNs, whose tail may be a password); a section never set shows the secrets of the running configuration as set or not, as SPEC-0114 does; the checks of the change run after the master key check (a non empty secret without a master key is refused first) and on the secrets trimmed before anything else (a value that trims to empty clears the secret, and needs no master key); an empty text clears one, an absent key keeps it; a non empty secret with no master key is refused with `no master key is configured; set FSA_TOKEN_KEY`.
- **FR-NEW-004** A change is accepted only when the section as it will be passes the checks of the start (`validate_store` for meta, git and oauth, with its messages) and the blob rules: a known backend (`unknown infra.blob.backend '<x>', expected one of local, minio, s3`), `infra.blob.dir` not empty for `local`, `infra.blob.access_key` and `infra.blob.secret_key` set for `minio` and `s3`, `infra.blob.endpoint` set for `minio`. The blob rules run on the section as it will be, the keys trimmed, in the order listed (the first that fails is the message); a blob rule message is `infra.blob.<key> is required when infra.blob.backend is '<backend>'` (`infra.blob.access_key and infra.blob.secret_key are required when infra.blob.backend is '<backend>'` for the keys). These blob rules are not run at the start.
- **FR-NEW-005** The location of a relational store is its backend, its DSN, its schema and, for SQLite, its place: the `meta.dir` directory for meta, and for git and oauth the state root that `meta.dir` derives (its parent directory), because their files live under it. A relational store whose location differs from the one the server runs, is probed before saving: the triggers are, for meta, `backend`, `dsn`, `schema`, `dir`; for git and oauth, `backend`, `dsn`, `schema` and the state root (`pool` never triggers); the directory of a SQLite place is created, then one database is opened and its schema applied, as a normal open does, for each current project (meta and git) and one for oauth; with no project and a SQLite place only the directory is created; on PostgreSQL one open is made. A store that cannot be opened is refused with `infra.<store> cannot be reached: <reason>`, the reason with every DSN and key replaced by `<redacted>`.
- **FR-NEW-006** For a relational store whose location differs (FR-NEW-005; a DSN that differs only by its credentials still counts as moved, the counts then agree), the new place must hold the data of the current one: for meta, per project in ascending project id, the number of rows of `nodes`, then of `blob_refs`, then the set of content addresses; for git, per project, the number of rows of `git_objects`, `git_operations`, `git_refs`, `git_remotes` in that order; for oauth, the number of rows of `oauth_tokens` (global, not per project). Only the first difference is told, in that order: `project 'p': nodes 4 here, 0 there`, `project 'p': blob_refs 3 here, 0 there`, `project 'p': 2 content addresses missing there`, `project 'p': git_refs 1 here, 0 there`, `oauth_tokens 1 here, 0 there`. Any difference, in either direction, refuses. The stores are judged in the order meta, git, oauth, and the first refusal is the only one told. A difference is refused with `infra.<store> points at a store that does not hold the data of the current one (<what differs>): run fsa migrate --from <current config> --to <new config> first, the server never moves data itself`. Every per project count is scoped to its project (`volume_id`). A source with no rows passes. A git or oauth store on SQLite moves only when the state root differs (the parent of `meta.dir`), a PostgreSQL one when its backend, DSN or schema differs.
- **FR-NEW-007** A blob store is probed before saving when its backend differs from the running one, or, for `local`, its `dir` differs, or, for `minio` and `s3`, its endpoint, bucket prefix, region, access key or secret key differs (a `local` store ignores the keys, the endpoint, the prefix and the region, an object store ignores `dir`): `local` needs its directory to exist or be created; `minio` and `s3` make one object existence request (a `HEAD`) for the object `fsa-probe` in the bucket `<bucket_prefix>probe`, which is never created (the probe builds its client from that bucket name); the probe passes when the store answers present or absent, the timeout is a `tokio::time::timeout` around the request, its elapsed case is the reason `timed out`, and anything else (a refusal of the keys, any status but 404 or success, a timeout of 15 seconds on the whole probe, retries included, no connection) is `infra.blob cannot be used: <reason>`, redacted as in FR-NEW-005.
- **FR-NEW-008** For a blob store that moved, that is whose backend differs from the running one, or, for `local`, its `dir`, or, for `minio` and `s3`, its endpoint, bucket prefix or region (a change of the keys alone is not a move), every content address of the current store (the distinct `sha256` of the `nodes` of each project, as `migrate::shas_of` reads them) must exist in the new store, the bucket of a project being `<bucket_prefix><project id>` (one existence request per address; `<m>` counts the distinct addresses of each project, summed); when some do not, the change is refused with `infra.blob points at a store that does not hold <n> of <m> blobs of the current one: copy them first (fsa migrate --from <current config> --to <new config> lists the missing ones), the server never moves data itself`. No address at all passes.
- **FR-NEW-009** A refused change stores nothing, records nothing and writes no row and no object to the new place (a probe applies a relational schema, as a normal open does, nothing else); an accepted one stores the section, records one line and says `restart_required` while the stored section differs from the running one.
- **FR-NEW-010** The data checks compare against the stores the server runs, not against a stored section waiting for a restart; a change back to the running location needs no probe and no check.
- **FR-NEW-011** At the start, a stored `infra` section replaces the `meta`, `blob`, `git` and `oauth` blocks of the YAML or the defaults and leaves `infra.admin` exactly as the start gave it (the YAML block, or the database URL when one is given); the stores are opened from the result; a stored section the start's checks refuse stops the start with `the stored settings are refused: <message>`; a stored value of the wrong shape (including a key `admin`) stops it with `the stored setting 'infra' is invalid: <reason>`.
- **FR-NEW-012** A start from a YAML file imports `infra` once when no row exists, a later YAML edit is ignored with the warning of SPEC-0114; the secrets of the YAML are encrypted under the master key; with no master key a YAML that holds a secret is not imported and stays in force, as in SPEC-0114.
- **FR-NEW-013** A stored secret that cannot be read (wrong or missing master key) stops the start naming the section, as in SPEC-0114.
- **FR-NEW-014** The change record holds the names of the changed keys (`blob.endpoint`, `meta.dir`, `blob.secret_key`), never a value; a key whose secret changed is named once.
- **FR-NEW-015** The stored section and its secrets survive `fsa migrate` between two admin stores (the row is copied as it is, the ciphertext included) and read back under the same master key; the change record is not copied by `fsa migrate` (unchanged, SPEC-0114).
- **FR-NEW-016** Platform administrators only, reading included; the screen lists the fourteen sections and shows the secrets as set or not with their last 4, never a value, never prefilled.
- **FR-NEW-017** The behavior is the same on SQLite and on PostgreSQL (the admin store, and a PostgreSQL meta, git or oauth store).

## 7. Non functional requirements
Security: no secret in an answer, record, log or error; no ambient environment variable is read by this section (a DSN is a value, never the name of a variable). Reliability: a probe times out after 15 seconds per store. Operability: a refusal says what to run.

## 8. Edge cases
A source with no data; two projects of which only one was copied; a rotation of the S3 keys on the same endpoint; a DSN that differs only by the password; a stored section edited in the database to hold an `admin` key; a YAML with S3 keys and no master key; a probe against a directory under a file.

## 9. Acceptance tests (the contract)
### 9.1 Summary
52 tests: 18 happy (001, 002, 005, 008, 014, 016, 021, 023, 029, 031, 041, 042, 048, 049, 053, 054, 060, 061), 24 failure (003, 004, 006, 007, 009, 010, 011, 020, 022, 026, 027, 040, 043, 044, 045, 046, 047, 050, 051, 056, 057, 059, 062, 024), 10 edge or side effect (012, 013, 015, 017, 025, 028, 030, 052, 055, 058). 17 requirements, each with at least 1 test (section 10 is generated from the Covers column). Test names are `e2e_0117_NNN`; PostgreSQL ones (060 to 062) need `MCPFS_TEST_PG_DSN`. A message quoted below is compared whole; where only a start is quoted, the test compares that start. The S3 endpoint of the tests is a local stand in that answers every `HEAD` with a chosen status.

### 9.2 New tests
| Id | Covers | Given / When / Then |
|---|---|---|
| E2E-001 | S1 FR-NEW-001 FR-NEW-002 | Given a server never set. When she lists and reads `infra`. Then the list names the fourteen sections ending with `infra`, the value holds `meta`, `blob`, `git`, `oauth` and no `admin`, the source is `default`, restart is not required, the five secrets read `has_value` false. |
| E2E-002 | S2 FR-NEW-002 FR-NEW-009 | Given a server with no data. When she sets `{"blob":{"dir":"<other>"}}`. Then the answer holds that directory, `blob.region` and the other keys unchanged, source `database`, `restart_required` true, and the running configuration still names the old directory. |
| E2E-003 | F6 FR-NEW-002 | When she sets `{"admin":{"path":"x"}}`. Then she is refused with `unknown key 'admin' in setting 'infra'`, nothing stored. |
| E2E-004 | F6 FR-NEW-002 | When she sets `{"meta":{"nope":1}}`, `{"blob":{"region":5}}`, `{"meta":"x"}` and the section as the text `x`. Then the first is refused with `unknown key 'meta.nope' in setting 'infra'`; the second and third with a message that begins `invalid setting 'infra':`; the fourth with `the value of setting 'infra' must be an object`; nothing stored. |
| E2E-005 | S3 FR-NEW-003 FR-NEW-004 | Given the S3 stand in answering absent and no blob in the server. When she sets blob backend `s3`, the endpoint of the stand in, access key `AKIAEXAMPLE0001`, secret key `s3cr3t-value-abcdef`. Then the answer holds neither key in `value`, `secrets` holds `has_value` true for both, `last4` `0001` for the access key and `cdef` for the secret key, and the raw stored row holds no trace of either text. |
| E2E-006 | F2 FR-NEW-003 | After E2E-005. Then neither key text appears in the answers of `get` and `list`, in the record lines, in `Debug` of the configuration the settings produce, nor in the logs captured during the test; a DSN given the same way (E2E-009) is also absent from all of them. |
| E2E-007 | F7 FR-NEW-003 | Given no master key. When she sets a non empty `blob.secret_key`. Then she is refused with `no master key is configured; set FSA_TOKEN_KEY`, nothing stored; setting `blob.region` alone is accepted. |
| E2E-008 | FR-NEW-003 | Given E2E-005 stored. When she sets `blob.region` alone, then sets `blob.secret_key` to `""` together with `blob.backend` `local`. Then after the first both keys still read `has_value` true, after the second `blob.secret_key` reads `has_value` false and `blob.access_key` is still true. |
| E2E-009 | F2 FR-NEW-003 FR-NEW-005 | Given a master key and a server built with the PostgreSQL driver. When she sets `meta` backend `postgres` and the DSN `postgres://u:hunter2pw@127.0.0.1:1/db`. Then she is refused with a message that begins `infra.meta cannot be reached:` and holds neither `hunter2pw` nor the DSN; nothing stored. Without the driver she is refused with the start's message about the `postgres` feature. |
| E2E-010 | F4 FR-NEW-004 | Given a master key. When she sets, one at a time, meta backend `postgres` with no DSN, git backend `sqlite` with the DSN `postgres://x`, blob backend `ftp`, blob backend `s3` with no keys, blob backend `minio` with keys and no endpoint, blob backend `local` with an empty `dir`. Then the refusals are, in order, `infra.meta.dsn is required when infra.meta.backend is 'postgres'`, `infra.git.dsn is set but infra.git.backend is 'sqlite', which stores to a file and never reads a dsn: remove one of the two`, `unknown infra.blob.backend 'ftp', expected one of local, minio, s3`, `infra.blob.access_key and infra.blob.secret_key are required when infra.blob.backend is 's3'`, `infra.blob.endpoint is required when infra.blob.backend is 'minio'`, `infra.blob.dir is required when infra.blob.backend is 'local'`; nothing stored. |
| E2E-011 | F5 FR-NEW-016 | Given Bob, a member who is not an administrator. When he lists, reads or sets `infra` by tool and by route and opens the screen. Then each is refused as for the thirteen other sections (`ERR_FORBIDDEN: 'bob@example.com' is not a platform admin`). |
| E2E-012 | FR-NEW-014 | When she sets `blob.backend` `s3`, `blob.endpoint`, `blob.access_key` and `blob.secret_key` (stand in absent). Then the record line holds exactly the key names `blob.access_key`, `blob.backend`, `blob.endpoint`, `blob.secret_key` and none of the values; a second set of the same values records one line with no key. |
| E2E-013 | FR-NEW-009 | When a change is refused (any of E2E-003, 004, 009, 010, 020). Then the record holds no new line and the section reads as before. |
| E2E-014 | FR-NEW-002 | The section read, then set to the value read, changes nothing and records no key. |
| E2E-015 | FR-NEW-002 FR-NEW-009 | Given she set `meta.pool.max_connections` to 20. Then the answer holds 20, `meta.pool.acquire_timeout_secs` keeps 30 and restart is required; no probe is made (the stand in counts zero requests; no relational store is opened). |
| E2E-016 | FR-NEW-016 | Given an administrator on the screen with E2E-005 stored. When she opens `/app/settings`. Then fourteen sections are listed, `infra` shows `blob.secret_key` as set with `cdef`, the page holds neither key text, and the secret input is empty. |
| E2E-017 | FR-NEW-009 | Admin A sets `blob.region` `eu-west-1`, then admin B sets `us-west-2`. Then the stored region is `us-west-2` and the record holds two lines, one per administrator. |
| E2E-020 | F1 FR-NEW-006 | Given a project with 3 files and the meta store in directory `v1`, a new directory `v2` empty. When she sets `meta.dir` to `v2`. Then she is refused with a message that begins `infra.meta points at a store that does not hold the data of the current one (project 'p': nodes 4 here, 0 there)` (the root counts as a node) and contains `fsa migrate --from` and `the server never moves data itself`; nothing stored. |
| E2E-021 | S2 FR-NEW-006 | After E2E-020 the operator runs the migration from the running configuration to a configuration that differs from it by `meta.dir` only (`v2`, the same admin, git and oauth blocks; a copy of a store onto itself is expected and harmless). When she sets `meta.dir` to `v2`. Then she is accepted, `restart_required` is true. |
| E2E-022 | F1 FR-NEW-006 | Given the migration done and then a new file written to project `p` and a file to project `q` in the running store. When she sets `meta.dir` to `v2`. Then she is refused naming project `p` first (ascending project id) and its row counts; after `p` is copied again with a migration that includes `q`, accepted. |
| E2E-023 | E2 FR-NEW-006 | Given a server with no project and no oauth token, `meta.dir` `<root>/volumes`. When she sets `meta.dir` to `<root2>/volumes` (another parent, so the state root moves). Then she is accepted with no migration. |
| E2E-024 | F1 FR-NEW-006 | Given a row in `oauth_tokens` of the running oauth store, `meta.dir` set to a directory with another parent (the state root moves) and the migration to that configuration done, then the row removed from the new oauth store by the test. Then she is refused with a message that begins `infra.oauth points at a store that does not hold the data of the current one (oauth_tokens 1 here, 0 there)`; after the migration to that configuration, accepted. |
| E2E-025 | FR-NEW-006 | Given a row of `git_refs` for project `p` in the running git store, the same move of `meta.dir` and the migration done, then the row removed from the new git store by the test. Then the refusal begins `infra.git points at a store that does not hold the data of the current one (project 'p': git_refs 1 here, 0 there)`; after the migration, accepted. |
| E2E-026 | F2 FR-NEW-005 | When she sets `meta.dir` to a path below a regular file. Then she is refused with a message that begins `infra.meta cannot be reached:`, nothing stored. |
| E2E-027 | F2 FR-NEW-005 | On PostgreSQL (driver and `MCPFS_TEST_PG_DSN`): a DSN of the reachable server with a wrong password. Then she is refused with `infra.meta cannot be reached:` and no text of the DSN. |
| E2E-028 | FR-NEW-010 | Given E2E-021 stored (waiting for a restart). When she sets `meta.dir` back to the running directory. Then she is accepted without any data check (a project file written after the migration does not matter), the answer says restart is not required. |
| E2E-029 | FR-NEW-015 | Given the section stored with E2E-005. When the operator migrates the admin database to another admin database and a server starts on it with the same master key. Then `infra` reads the same value and the same `has_value` and `last4`. |
| E2E-030 | FR-NEW-009 | After E2E-020. Then the new directory holds, at most, one database file per current project with empty tables (the probe's schema), no row of `nodes` or `blob_refs`, and no blob object exists in the new blob directory. |
| E2E-031 | S4 FR-NEW-011 | Given a stored `infra` with `blob.dir` and `meta.dir` of a populated platform, a server stopped, and only the admin URL and the key given. When it starts. Then the running stores are those directories and the files are served. |
| E2E-040 | F3 FR-NEW-008 | Given one file written in a project (one blob) and the S3 stand in answering absent. When she sets blob backend `s3` with its endpoint and keys. Then she is refused with `infra.blob points at a store that does not hold 1 of 1 blobs of the current one: copy them first (fsa migrate --from <current config> --to <new config> lists the missing ones), the server never moves data itself`; nothing stored. |
| E2E-041 | S3 FR-NEW-008 | Same, the stand in answering that every object exists. Then she is accepted and `restart_required` is true. |
| E2E-042 | E2 FR-NEW-007 FR-NEW-008 | Given no blob and the stand in answering absent. When she sets the S3 settings. Then she is accepted. |
| E2E-043 | F2 FR-NEW-007 | The stand in answers `403` to every request. When she sets the S3 settings. Then she is refused with a message that begins `infra.blob cannot be used:`, holding neither key. |
| E2E-044 | F2 FR-NEW-007 | The endpoint is `http://127.0.0.1:1`. Then she is refused with a message that begins `infra.blob cannot be used:`. |
| E2E-045 | F2 FR-NEW-007 | The directory is below a regular file. Then she is refused with a message that begins `infra.blob cannot be used:`. |
| E2E-046 | F2 FR-NEW-007 | Given the server restarted on the S3 settings of E2E-005 (stand in answering absent). When she sets only `blob.secret_key` and the stand in answers `403`. Then she is refused with `infra.blob cannot be used:`; with the stand in answering absent she is accepted and no existence request of a content address is made. |
| E2E-047 | FR-NEW-008 | Given the same two blobs in two projects and the stand in holding the content addresses of one project only. Then she is refused with `... does not hold 1 of 2 blobs ...`; the count is per project bucket. |
| E2E-048 | S5 FR-NEW-012 | Given a YAML with `infra.blob.dir`, `infra.meta.dir` and a master key. When the first start runs, then the YAML is edited to other directories and the server starts again. Then the row is imported once by `system:yaml`, the directories in force are the first ones, and the record holds one `seed_setting` line for `infra`. |
| E2E-049 | FR-NEW-012 | Given a YAML with `infra.blob` backend `minio`, endpoint, access key `AKIAYAML00000001`, secret key `yaml-secret-0001` and a master key. When the server starts. Then the raw row holds neither key text, the section reads both as set with their last 4, and the running blob keys are the YAML ones. |
| E2E-050 | F7 FR-NEW-013 | Given a stored `infra` with a secret and the server started with another master key, then with none. Then each start fails with `the stored setting 'infra' holds a secret that cannot be read: set FSA_TOKEN_KEY to the key it was stored with`. |
| E2E-051 | F4 FR-NEW-011 | Given a stored `infra` edited in the database to `meta.backend` `postgres` with no DSN. Then the start fails with `the stored settings are refused: infra.meta.dsn is required when infra.meta.backend is 'postgres'`. Edited to hold an `admin` key, the start fails with a message that begins `the stored setting 'infra' is invalid:`. |
| E2E-052 | FR-NEW-012 | Given a YAML with `infra.blob` S3 keys and no master key. When the server starts. Then it starts on the YAML values, no `infra` row exists and one warning says the section holds a secret and is not imported. |
| E2E-053 | FR-NEW-011 | Given a server started with `--database-url` naming an admin store at A and a stored `infra` holding an `admin`-free section. Then the `infra.admin` of the running configuration (`state.config`) is A, the store the server opened, whatever the YAML or the stored section says; a YAML that names another `infra.admin` is not read when the URL is given; with no URL, the YAML `infra.admin` is the one in force and a stored section never changes it. |
| E2E-054 | FR-NEW-012 | Given a YAML alone (no row, today's file without a master key) holding `infra.meta.dir` and `infra.blob.dir`. When the server starts. Then the stores in use are those directories, a file written is read back, the configuration tests pass unmodified. |
| E2E-055 | FR-NEW-006 | Given data and a PostgreSQL meta store holding the same data by an earlier migration, running on it. When she sets a DSN that differs only by a connection parameter (`application_name`). Then she is accepted. |
| E2E-056 | F4 FR-NEW-004 | Given a master key. When she sets the backend `s3` with an endpoint, an access key and a secret key made only of blanks. Then she is refused with `infra.blob.access_key and infra.blob.secret_key are required when infra.blob.backend is 's3'`, nothing stored. |
| E2E-057 | FR-NEW-007 | Given the stand in that never answers and the probe timeout lowered to 300 ms through its test hook (`settings::infra_check::PROBE_TIMEOUT_MS`, an atomic read at each probe). Then she is refused with `infra.blob cannot be used: timed out` within 5 seconds. |
| E2E-058 | FR-NEW-005 | A probe error whose text names the DSN or an S3 key shows `<redacted>` in its place. |
| E2E-059 | F5 FR-NEW-016 | Bob is refused on `infra` by the data plane route as well as the tool, and the refused call opens no store (the stand in counts zero requests). |
| E2E-060 | FR-NEW-017 | On PostgreSQL she sets `blob.region`, then the S3 settings with the stand in. Then a read gives the same answer shape and `has_value` for the keys; the record holds two lines. |
| E2E-061 | FR-NEW-017 FR-NEW-006 | On PostgreSQL the meta store moves to a second schema: refused until the migration, then accepted. |
| E2E-062 | FR-NEW-017 FR-NEW-005 | On PostgreSQL a refused unreachable DSN stores and records nothing and does not echo the password. |

### 9.3 Regression tests
None beyond 9.4.

### 9.4 Modified tests [existing only]
Declared exceptions, target as small as possible: the two Debug snapshots of `config.rs` (D0), the section lists and counts (`e2e_0114_009`, `010`, `e2e_0115_005` seed count, `018`, `037`, the `e2e_0116_` tests that name the thirteen sections), the tool descriptions and the contract lines. Identifiers in 14.2.

## 10. Traceability
Scenarios: S1 E2E-001; S2 E2E-002 021; S3 E2E-005 041; S4 E2E-031; S5 E2E-048; F1 E2E-020 022; F2 E2E-006 009 026 027 043 044 045 046; F3 E2E-040; F4 E2E-010 051 056; F5 E2E-011 059; F6 E2E-003 004; F7 E2E-007 050; E1 E2E-015 028; E2 E2E-023 042.

| Requirement | Tests |
|---|---|
| FR-NEW-001 | E2E-001, E2E-016 |
| FR-NEW-002 | E2E-001, E2E-002, E2E-003, E2E-004, E2E-014 |
| FR-NEW-003 | E2E-005, E2E-006, E2E-007, E2E-008, E2E-009 |
| FR-NEW-004 | E2E-005, E2E-010, E2E-056 |
| FR-NEW-005 | E2E-009, E2E-026, E2E-027, E2E-058, E2E-062 |
| FR-NEW-006 | E2E-020, E2E-021, E2E-022, E2E-023, E2E-024, E2E-025, E2E-055, E2E-061 |
| FR-NEW-007 | E2E-042, E2E-043, E2E-044, E2E-045, E2E-046, E2E-057 |
| FR-NEW-008 | E2E-040, E2E-041, E2E-042, E2E-047 |
| FR-NEW-009 | E2E-002, E2E-013, E2E-015, E2E-017, E2E-030 |
| FR-NEW-010 | E2E-028 |
| FR-NEW-011 | E2E-031, E2E-051, E2E-053 |
| FR-NEW-012 | E2E-048, E2E-049, E2E-052, E2E-054 |
| FR-NEW-013 | E2E-050 |
| FR-NEW-014 | E2E-012 |
| FR-NEW-015 | E2E-029 |
| FR-NEW-016 | E2E-011, E2E-016, E2E-059 |
| FR-NEW-017 | E2E-060, E2E-061, E2E-062 |

## 11. Open questions
None that changes behavior.

## 12. Glossary
Section: one top level block of the configuration. Moved store: a store whose location differs from the running one. Running store: the one the process opened at its start. Stand in: a local server that answers every `HEAD` with a chosen status.

## 13. Implementability gate
Gate record: round 1 (independent auditor): NOT-IMPLEMENTABLE, 14 precision findings, merged. Round 2 (new auditor): NOT-IMPLEMENTABLE, 10 findings (one wrongly classed structural, the change record is not copied by `fsa migrate`, cut from FR-NEW-015), merged. Round 3 (new auditor): IMPLEMENTABLE with 6 precision notes (the Debug snapshots, the source of the blob addresses, the probe timeout, one Given, the test count wording, the migration Given), merged here; one note (the guard for a non object value) was checked against `server_settings.rs` and needed nothing. Closed at round 3, no further round.

## 14. Evidence appendix
### 14.1 Current State evidence
Suites named in 2.3: `config.rs` tests on `infra`, `migrate.rs` tests, `e2e_0114_`, `e2e_0115_`, `e2e_0116_`.
1. `config.rs` `InfraConfig` (meta, blob, admin, git, oauth) is read by `ServerConfig::from_yaml` and `apply_database_url`; `validate_store` at `check()`. 2. `app.rs` `build_with_env` opens the admin store from `config.infra.admin` before `server_settings_boot::apply`, then builds the `StoreManager` from the result. 3. `migrate.rs` `migrate` copies `project`, `project_member`, `server_setting`, `oauth_tokens`, then per project `nodes`, `blob_refs` and the git tables, and reports missing blobs.
### 14.2 Existing tests to change
The section lists and counts named in 9.4; the `Section name:` descriptions in `tools/settings.rs` and `mcp/server.rs`; `TOOL_CONTRACT.txt`; the regenerated golden. Detection signal: `tool_contract_golden_is_current`.
### 14.3 Design inputs
- D0 `BlobConfig` gets a hand written `Debug` that prints each of its two keys as set or unset only (as `Dsn` does); `Dsn` already redacts. The form is `access_key: set` or `access_key: unset` (same for `secret_key`). The two recorded Debug snapshots of the existing config tests (`LOCAL_TEMPLATE_SNAPSHOT`, `MINIO_TEMPLATE_SNAPSHOT`) change by those two fields only: declared exceptions.
- D1 `infra` is a view of `InfraConfig` without `admin`: a struct of the four blocks, so `check_keys` refuses `admin` as unknown and `with_section` keeps the running admin block.
- D2 `Dsn` serializes as its text: it must move to the secret store by `secret_paths`, and the answer shows no `last4` for a path ending in `.dsn`.
- D3 The checks of a change need the running configuration and the admin store (the project list): they run in `try_set` after `check()`, on the full `InfraConfig` rebuilt from the value plus the merged secrets (the kept ones decrypted, the given ones trimmed); a kept secret that cannot be read keeps today's refusal of SPEC-0114.
