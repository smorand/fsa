# SPEC-0103: the converter answer carries the full document metadata, design

> Spec: sdlc/specs/SPEC-0103-converter-answer-with-the-full-document-metadata/spec.md
> Nature: FEAT
> Depth: L
> Security: internal finding
> Designed on: 2026-11-21
> Target tier: 2 (default, no `Tier:` in `sdlc/constitution.md`, no `.spec.json`)

## 1. Stack and skills
| Element | Choice | Version | DEC |
|---|---|---|---|
| Language, runtime | Rust 2024, tokio | workspace manifest | existing |
| JSON | `serde_json` (`Value`, `Number`) | workspace manifest | DEC-002 |
| Relational layer | `storage::rel` on the volume meta store | in tree | DEC-006 |
| Heading rule | `chunking::blocks::heading` | in tree | DEC-007 |

No dependency is added, so no library lookup was needed. Skill loaded: `rust` (already in context). Effect: `make check` is the gate (format, clippy `-D warnings --all-features`, tests inside `crates/core/src/**`), comments explain WHY, no dashes as punctuation, no secret or file content in logs.

## 2. Approaches
### Chosen: a tolerant hand written manifest reader that returns data plus typed notes, feeding the existing capture and report
`docs/manifest.rs` reads the manifest from a `serde_json::Value` field by field, so a wrong typed field, a null, an unknown version or a bad entry becomes a note (a report line with its group and item number) instead of a whole manifest rejection. `PendingSet::converter` consumes the parsed manifest, resolves CSV files through the existing bundle map and `resolve_target`, derives the structure when the converter gave none, and `report()` orders all lines by group. The volume meta store gains three nullable columns and one table, written by the existing single transaction. Pros: every rule of the spec (null as absent, per field lines, versions) is one function with unit tests; the pipeline, the quota charge and the commit stay as they are. Cons: a hand parser is longer than serde derive. Scope: `docs/`, `storage/meta.rs`, `tools/artifacts.rs`, tests, docs.
### Rejected: serde derive with `#[serde(default)]` structs
It rejects a whole manifest on one wrong type and reports nothing, which is today's behavior and the thing FR-NEW-016 and FR-NEW-023 change (DEC-002).
### Rejected: a JSON schema validator crate
New dependency for rules that fit in about 300 lines, and a schema cannot produce the per item report lines (DEC-002).
### Rejected: store the whole manifest as one JSON blob and read it later
It would keep OCR and structure unqueryable and defeat the "set replaced as a whole" accounting of sizes (DEC-006).

## 3. Architecture
```
 converter (program folder | HTTP JSON)
        |  read_bundle_blocking / parse_api_bundle   (cap counts Markdown + files + manifest, DEC-009)
        v
 Conversion { markdown, files, manifest_raw: Option<Value> }
        |  docs::manifest::parse(raw) -> ManifestOutcome { manifest: Option<Manifest>, notes }
        v
 PendingSet::converter(conversion, source)
        |  images (+ ocr), tables (+ csv, caption, page), headings (supplied or derived)  ; failures grouped
        |  report(): header, metadata line, grouped lines     bytes(): charge incl. ocr, csv, titles
        v
 commit_artifact_set (one transaction): doc_images(+ocr_text) doc_tables(+csv_text,+page) doc_headings
        v
 fs.get_table (csv): stored exact text when present      fs.list_tables: label from stored quality
```
Bounded contexts: Documents (all of it) and Projects (quota, unchanged). Existing structure cited: `docs/service.rs:73-102` (types), `:688-741` (folder bundle), `:758-790` (HTTP bundle), `docs/artifacts/capture.rs:93-175` (converter set, report, commit), `:274-298` (target resolution), `storage/meta.rs:114-140,932-1000,1060-1125` (rows, commit, sweep), `tools/artifacts.rs:238-306` (tools), `core/fs_ops.rs:799-875` (conversion paths and charge).

## 4. Data model
Volume meta store, additive (`CREATE TABLE IF NOT EXISTS` and `ColumnMigration`, rendered per dialect):
- `doc_images.ocr_text` Text, null (OCR text, up to 100 000 characters).
- `doc_tables.csv_text` Text, null (exact CSV given by the converter); `doc_tables.page` BigInt, null; the existing `quality` column holds `exact` for a converter CSV.
- New table `doc_headings`: `volume_id` Text, `set_id` Text, `seq` BigInt, `level` BigInt, `title` Text, `page` BigInt null; primary key `(volume_id, set_id, seq)`.
Existing rows read as null in the new columns and have no heading rows (FR-NEW-020). `sweep_stale_artifact_sets` also deletes `doc_headings`; `volume_tables()` picks the new table up by its `volume_id` column; the schema golden fixtures and the `storage::meta` DDL fingerprints regenerate. Migration: additive; rollback is dropping the three columns and the table (nothing else reads them).

## 5. Contracts and interfaces
### 5.1 Manifest (the converter contract, product interface)
```
{ "version": 2,
  "pictures": { "<target>": { "caption": text, "caption_failed": bool, "page": whole>=1,
                              "ocr_text": text<=100000 chars, "ocr_failed": bool } },
  "tables":   [ { "csv": "<bundle file>", "caption": text, "page": whole>=1 } ],   // entry i is pipe table i
  "headings": [ { "level": 1..6, "title": text 1..1000 chars, "page": whole>=1 } ] }
```
Version rules and every line text are those of FR-NEW-001 to FR-NEW-023, implemented in `docs::manifest`; a whole number is `serde_json::Number` with `is_u64()` or `is_i64()` (a number written `2.0` or `1e0` parses as a float and is refused); character limits count `chars()`.
### 5.2 Internal signatures (Rust)
```rust
// docs/manifest.rs
pub struct Manifest { pub version: u32, pub pictures: BTreeMap<String, PictureMeta>, pub tables: Vec<TableEntry>, pub headings: Option<Vec<Heading>> }
pub struct PictureMeta { pub caption: String, pub caption_failed: bool, pub page: Option<i64>, pub ocr_text: String, pub ocr_failed: bool }
pub struct TableEntry { pub csv: Option<String>, pub caption: Option<String>, pub page: Option<i64> }
pub struct Heading { pub level: u8, pub title: String, pub page: Option<i64> }
pub struct Notes { pub manifest: Vec<String>, pub images: BTreeMap<String, Vec<ImageNote>>, pub tables: BTreeMap<usize, Vec<String>>, pub headings: Vec<String> }
pub struct ManifestOutcome { pub manifest: Option<Manifest>, pub notes: Notes }
pub fn parse(raw: &Value) -> ManifestOutcome;          // never fails; ignored manifest = manifest None
// docs/service.rs: Conversion { markdown, files, manifest: Option<Value> } (raw value kept, parsed in capture)
// docs/artifacts/headings.rs
pub fn derive(markdown: &str) -> Vec<Heading>;         // fence aware, chunking heading rule, pages None
// docs/artifacts/capture.rs: PendingSet gains `headings: Vec<Heading>`, `v2: bool`, grouped `notes`; report() orders lines (FR-NEW-017)
```
### 5.3 Report
`report()`: header `<N> images, <M> tables`; if the manifest is version 2 or newer, `metadata: <k> images with OCR text, <j> tables with exact CSV, <h> headings`; then the lines in the order of FR-NEW-017 (manifest, image, table, heading groups). No manifest, version 1, or an ignored manifest: no metadata line.
### 5.4 Tool behavior
`fs.get_table` with `format` csv answers the stored `csv_text` when the row has one, else the CSV rendered from the cells as today; `fs.list_tables` labels follow the stored `quality`. No tool, parameter or contract file changes (tool counts unchanged).

## 6. File structure impact
| Path | Action | Caused by | Story |
|---|---|---|---|
| `crates/core/src/docs/manifest.rs`, `docs/mod.rs` | create / modify: tolerant reader, versions, notes | FR-NEW-001..003, 016, 023 | US-0290 |
| `crates/core/src/docs/service.rs` | modify: `Conversion.manifest` raw, both forms hand it over | FR-NEW-001, 018 | US-0290, US-0294 |
| `crates/core/src/docs/artifacts/capture.rs` | modify: apply manifest, grouped report, charge | FR-NEW-004..017, 022 | US-0290..US-0294 |
| `crates/core/src/docs/artifacts/headings.rs`, `artifacts/mod.rs` | create / modify: derived headings | FR-NEW-012..015 | US-0291 |
| `crates/core/src/storage/meta.rs` | modify: columns, table, inserts, sweep, readers | FR-NEW-004, 008, 011, 012 | US-0291, US-0292, US-0293 |
| `crates/core/src/storage/rel/schema.rs`, `crates/core/tests/fixtures/schema_{sqlite,postgres}.sql` | modify / regenerate (`MCPFS_REWRITE_SCHEMA_GOLDEN=1`) | the new columns and table | US-0291..US-0293 |
| `crates/core/src/tools/artifacts.rs` | modify: exact CSV read | FR-NEW-008 | US-0293 |
| `crates/core/src/docs/manifest_e2e.rs`, `lib.rs` or `docs/mod.rs` | create (test module, all spec E2E) | all | US-0290..US-0295 |
| `README.md`, `AGENTS.md`, `.agent_docs/{architecture,tools,testing}.md` | modify: manifest version 2, report lines, test command | all | US-0295 |
Affected existing tests: `docs/service.rs` and `capture.rs` manifest tests (keep, the version 1 path is unchanged; a test that asserted a silently ignored malformed manifest is modified to expect the new `manifest ignored:` line), the `storage::meta` table list test (modify, one new table), DDL fingerprint test (modify).

## 7. Compatibility and risks
- A version 1 or versionless manifest and a converter with no manifest: same report, same data. Two declared changes (FR-MOD-001, FR-MOD-002): a malformed manifest now adds a `manifest ignored:` line, and a wrong typed field of a version 1 picture drops that field and adds a line instead of discarding the manifest.
- The HTTP form cap now counts the Markdown and the manifest (DEC-009), so an answer just under the old cap can now be refused. Folder form already counted them.
- Risk 1, untrusted converter output: CSV references reuse `resolve_target` and only the in memory bundle map (DEC-008); no file is opened by name.
- Risk 2, size: 10 000 heading rows and 1 000 pictures in one transaction; inserts are per row in the existing transaction as for images; measured by E2E-094.
- No new dependency. Rollback: revert the branch; additive schema.

## 8. Security design
Threat surface: the converter's answer is untrusted (a program run by the platform, or an HTTP service). Sinks: reads of bundle files by name, the quota and storage charge, the report text, stored text shown later by tools.
- Class sweep occurrences: (1) the picture target read `capture.rs:258` behind `resolve_target` (`capture.rs:274-298`): safe, kept as is; (3) HTTP file keys `service.rs:773-779`: keys stay map keys, never opened; (2) the new table `csv` reference: fixed in this lot by using the same `resolve_target` and the same map (DEC-008), nothing is read from the filesystem or the blob store by that name.
- Exploit tests: E2E-035 (`../../etc/passwd`, `/etc/passwd`, `http://example.com/t.csv` as `csv`; second channel: the content of the host file must appear in no answer, table or report) in US-0293; E2E-078 (`../tab.csv` in the HTTP form) in US-0293. Both are red before the story because the `csv` field is not read at all.
- Report lines contain only counts and item numbers and the field names fixed by the spec, never converter supplied text; stored OCR text, CSV and titles are data, never executed or interpreted, and are not logged.
- Least privilege and secrets: no secret is involved.

## 9. Test strategy
Every spec test is a functional e2e on the real conversion path: a stand in converter (a shell script in the program form, an axum stub in the HTTP form, as `docs/service.rs` and `tools/converter_bound_e2e.rs` do) behind `AppState` with an in memory admin store and a local volume, calling `fs.documentize`, the upload with the conversion option, `fs.list_images`, `fs.list_tables`, `fs.get_table` and the built in `fs.extract_text`, through the MCP handlers. All in `crates/core/src/docs/manifest_e2e.rs`, named `e2e_0103_<id>_<slug>`; run with `cargo test -p fsa-core --lib -- e2e_0103_`. Pure rules (parser, derived headings) also have unit tests in their modules. The table below is the map.

| Test | Level | Driver | Location | Story |
|---|---|---|---|---|
| E2E-003 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0290 |
| E2E-004 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0290 |
| E2E-005 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0290 |
| E2E-010 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0290 |
| E2E-011 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0290 |
| E2E-012 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0290 |
| E2E-013 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0290 |
| E2E-014 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0290 |
| E2E-015 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0290 |
| E2E-016 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0290 |
| E2E-017 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0290 |
| E2E-018 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0290 |
| E2E-057 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0290 |
| E2E-103 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0290 |
| E2E-050 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0291 |
| E2E-051 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0291 |
| E2E-052 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0291 |
| E2E-053 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0291 |
| E2E-054 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0291 |
| E2E-055 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0291 |
| E2E-056 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0291 |
| E2E-058 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0291 |
| E2E-059 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0291 |
| E2E-060 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0291 |
| E2E-061 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0291 |
| E2E-063 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0291 |
| E2E-089 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0291 |
| E2E-090 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0291 |
| E2E-091 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0291 |
| E2E-099 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0291 |
| E2E-100 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0291 |
| E2E-101 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0291 |
| E2E-106 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0291 |
| E2E-MOD-001 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0291 |
| E2E-020 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0292 |
| E2E-021 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0292 |
| E2E-022 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0292 |
| E2E-023 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0292 |
| E2E-024 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0292 |
| E2E-025 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0292 |
| E2E-026 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0292 |
| E2E-027 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0292 |
| E2E-028 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0292 |
| E2E-029 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0292 |
| E2E-087 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0292 |
| E2E-095 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0292 |
| E2E-097 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0292 |
| E2E-098 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0292 |
| E2E-105 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0292 |
| E2E-107 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0292 |
| E2E-064 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0292 |
| E2E-030 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0293 |
| E2E-031 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0293 |
| E2E-032 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0293 |
| E2E-033 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0293 |
| E2E-034 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0293 |
| E2E-035 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0293 |
| E2E-036 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0293 |
| E2E-037 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0293 |
| E2E-038 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0293 |
| E2E-039 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0293 |
| E2E-040 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0293 |
| E2E-041 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0293 |
| E2E-042 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0293 |
| E2E-043 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0293 |
| E2E-082 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0293 |
| E2E-084 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0293 |
| E2E-096 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0293 |
| E2E-104 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0293 |
| E2E-077 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0293 |
| E2E-078 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0293 |
| E2E-065 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0294 |
| E2E-076 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0294 |
| E2E-092 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0294 |
| E2E-093 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0294 |
| E2E-094 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0294 |
| E2E-075 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0294 |
| E2E-079 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0294 |
| E2E-066 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0294 |
| E2E-067 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0294 |
| E2E-002 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0294 |
| E2E-001 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0295 |
| E2E-062 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0295 |
| E2E-068 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0295 |
| E2E-069 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0295 |
| E2E-070 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0295 |
| E2E-071 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0295 |
| E2E-072 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0295 |
| E2E-073 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0295 |
| E2E-074 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0295 |
| E2E-080 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0295 |
| E2E-081 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0295 |
| E2E-083 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0295 |
| E2E-085 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0295 |
| E2E-086 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0295 |
| E2E-088 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0295 |
| E2E-102 | functional e2e | MCP handlers on the real conversion path, stand in converter (program or HTTP) | `docs/manifest_e2e.rs` | US-0295 |

Test data: generated (stand in converters, generated Markdown, pictures, CSV and manifests); nothing user provided.

## 10. Decisions log
- **DEC-001:** No new dependency; the stack is the repository's. **Obliged by:** n/a (no code impact): the spec names no technology. **Rationale:** `serde_json` and the relational layer cover every need. **Alternatives considered:** a JSON schema crate (cannot produce per item report lines). **Source:** existing code. **Evidence:** `Cargo.toml`.
- **DEC-002:** The manifest is read by hand from a `serde_json::Value` in `docs/manifest.rs`, field by field, returning `ManifestOutcome { manifest, notes }` and never an error; a whole number is a `Number` that is `u64` or `i64` (so `2.0` and `1e0` are refused); lengths count `chars()`. **Obliged by:** SPEC-0103/FR-NEW-001, FR-NEW-003, FR-NEW-016, FR-NEW-023. **Rationale:** serde derive rejects a whole manifest on one wrong field and cannot say which item was wrong. **Alternatives considered:** serde derive with defaults (today's silent discard), a schema validator crate. **Source:** existing code. **Evidence:** `docs/service.rs:746-754,786-790` (silent ignore today), `docs/service.rs:89-102`.
- **DEC-003:** `Conversion.manifest` keeps the raw `Value` (program form: parsed from `artifacts.json` text, a non JSON text becomes the note `not valid JSON`; HTTP form: the `manifest` value as received) and the parse happens in `PendingSet::converter`, so both forms share one reader. **Obliged by:** SPEC-0103/FR-NEW-018. **Rationale:** one contract, one code path. **Alternatives considered:** parse in each form. **Source:** existing code. **Evidence:** `docs/service.rs:73-77,746,786`, `capture.rs:93`.
- **DEC-004:** The parser returns notes grouped by item (manifest level lines, per picture target, per table entry number, heading lines); `PendingSet` maps a picture target to its image number once it exists, and `report()` writes the groups in the order of FR-NEW-017 (header, metadata, manifest, images by number, tables by number with `table-<i>` lines before `tables entry <i>` lines, headings). A picture that is not kept emits only its existing line (FR-NEW-023). **Obliged by:** SPEC-0103/FR-NEW-017, FR-NEW-023. **Rationale:** image numbers are known only after capture. **Alternatives considered:** number images in the parser (needs the Markdown parse). **Source:** existing code. **Evidence:** `capture.rs:93-135,149-151`.
- **DEC-005:** The `metadata:` line is written when the manifest version is 2 or newer after a successful parse: `k` = images kept with non empty OCR text, `j` = tables labelled exact because of a manifest CSV, `h` = headings of the stored structure; a manifest that is ignored (including an invalid version) counts as no manifest. **Obliged by:** SPEC-0103/FR-NEW-017, FR-NEW-003, FR-MOD-001. **Rationale:** matches the spec's counts one to one. **Alternatives considered:** count from the stored rows after commit (adds a read). **Source:** spec. **Evidence:** n/a.
- **DEC-006:** Storage is additive in the volume meta store: nullable `doc_images.ocr_text`, `doc_tables.csv_text`, `doc_tables.page`, and a table `doc_headings`, all written by the existing `commit_artifact_set` transaction and deleted with their set by `sweep_stale_artifact_sets`; `quality` is `exact` for a manifest CSV. **Obliged by:** SPEC-0103/FR-NEW-004, FR-NEW-008, FR-NEW-011, FR-NEW-012, FR-NEW-013, FR-NEW-019, FR-NEW-020. **Rationale:** metadata of a set is replaced with the set, and old rows stay valid with nulls. **Alternatives considered:** one JSON column per set (not queryable by BL-0167), a separate store. **Source:** existing code. **Evidence:** `storage/meta.rs:114-140,362-378,932-1000,1060-1125`.
- **DEC-007:** Derived headings come from `docs/artifacts/headings.rs`: one pass over the lines, fence tracking as `parse.rs` does (a line starting with three backticks toggles), the rule of `chunking::blocks::heading` for each other line (closing run of `#` removed, `C#` kept), titles over 1 000 characters dropped silently, at most 10 000 kept with `headings over 10000 dropped: <n>` only for a version 2 manifest. The built in extraction stores no structure (its behavior stays unchanged, FR-NEW-021). **Obliged by:** SPEC-0103/FR-NEW-013, FR-NEW-015, FR-NEW-021. **Rationale:** one heading rule for chunking and metadata; linear time. **Alternatives considered:** re-implement the rule, run a Markdown crate. **Source:** existing code. **Evidence:** `chunking/blocks.rs:61,184`, `docs/artifacts/parse.rs:39-57`.
- **DEC-008:** A table `csv` is resolved with the existing `resolve_target` and looked up only in the in memory `Conversion.files` map (never opened as a file, never read from the blob store); the bytes must be valid UTF-8 with at most 5 000 000 characters, else `table-<i>: CSV unavailable`. CSV files are not stored as files: only their text is kept in `doc_tables.csv_text`. **Obliged by:** SPEC-0103/FR-NEW-008, FR-NEW-009, FR-NEW-022. **Rationale:** the control that already stops picture traversal covers CSV for free. **Alternatives considered:** a second resolver. **Source:** existing code. **Evidence:** `capture.rs:256-298`.
- **DEC-009:** The converter output cap counts the Markdown, every bundle file and the manifest in both forms: the folder form already counts every regular file (`service.rs:716-727`); the HTTP form adds the byte length of `markdown` and of the compact JSON text of `manifest` to the decoded file bytes; the existing `over_cap` text is kept. **Obliged by:** SPEC-0103/FR-NEW-022, FR-NEW-018. **Rationale:** the same content must give the same total in both forms. **Alternatives considered:** count only files in both (would drop the folder form's Markdown). **Source:** existing code. **Evidence:** `docs/service.rs:716-727,772-783,290`.
- **DEC-010:** The storage charge `PendingSet::bytes()` adds the OCR text bytes, the exact CSV bytes (instead of the derived CSV length for such a table) and the title bytes of every heading, so the existing admission and refusal texts (`ERR_PROJECT_QUOTA_EXCEEDED`, `session write quota of <n> bytes exceeded`) apply unchanged. **Obliged by:** SPEC-0103/FR-NEW-022. **Rationale:** one charge, as today. **Alternatives considered:** a second charge. **Source:** existing code. **Evidence:** `capture.rs:137-145`, `core/fs_ops.rs:799-875`.
- **DEC-011:** `fs.get_table` in CSV returns the stored `csv_text` when the row has one, else the CSV rendered from cells; `fs.list_tables` labels from the stored `quality`. **Obliged by:** SPEC-0103/FR-NEW-008. **Rationale:** the only existing read path for table CSV. **Alternatives considered:** a new tool (BL-0167). **Source:** existing code. **Evidence:** `tools/artifacts.rs:238-306`, `docs/artifacts/render.rs:298`.
- **DEC-012:** Table entry `i` applies to pipe table `seq` `i` (every pipe table counts, malformed ones included, so numbers match the report's `table-<n>`); an entry for a malformed table is unused and gives no line. **Obliged by:** SPEC-0103/FR-NEW-007, FR-NEW-010. **Rationale:** `table-N` numbering already counts every pipe table. **Alternatives considered:** number only kept tables (would shift ids). **Source:** existing code. **Evidence:** `docs/artifacts/parse.rs:19-33`, `capture.rs:125`.
- **DEC-013:** A re-conversion and a failed conversion keep today's behavior: the new rows are part of the one `commit_artifact_set` transaction, which supersedes the previous set or fails with nothing written (blobs of a refused commit are released). **Obliged by:** SPEC-0103/FR-NEW-019, FR-NEW-024. **Rationale:** no new mechanism needed. **Alternatives considered:** none. **Source:** existing code. **Evidence:** `storage/meta.rs:932-1000`, `capture.rs:159-229`.
- **DEC-014:** Nothing converts at upgrade and no data migration runs: new columns read null and `doc_headings` is empty for older sets. **Obliged by:** SPEC-0103/FR-NEW-020. **Rationale:** additive schema. **Alternatives considered:** a background reconversion (not asked). **Source:** spec. **Evidence:** n/a.

## 11. Stories
| Story | Title | Priority | Requirements | Depends on | min_tier | Closes spec |
|---|---|---|---|---|---|---|
| US-0290 | the manifest reader, versions and manifest level report lines | P1 | FR-NEW-001, FR-NEW-002, FR-NEW-003, FR-NEW-016, FR-MOD-002, FR-NEW-023 | main | 1 | no |
| US-0291 | the document structure and the metadata line of the report | P1 | FR-NEW-012, FR-NEW-013, FR-NEW-014, FR-NEW-015, FR-NEW-017, FR-MOD-001 | US-0290 | 1 | no |
| US-0292 | OCR text of images | P1 | FR-NEW-004, FR-NEW-005, FR-NEW-006, FR-NEW-023 | US-0291 | 1 | no |
| US-0293 | tables with an exact CSV, a caption and a page | P1 | FR-NEW-007, FR-NEW-008, FR-NEW-009, FR-NEW-010, FR-NEW-011, FR-NEW-023 | US-0291, US-0292 | 1 | no |
| US-0294 | the converter output limit, the quota and the two forms | P2 | FR-NEW-018, FR-NEW-022 | US-0293 | 2 | no |
| US-0295 | replacing, upgrading, other paths and the docs | P2 | FR-NEW-019, FR-NEW-020, FR-NEW-021, FR-NEW-024 | US-0294 | 2 | yes |
