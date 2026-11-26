# SPEC-0111: File metadata screen, design

> Spec: sdlc/specs/SPEC-0111-file-metadata-screen/spec.md
> Nature: FEAT
> Depth: M
> Security: stored XSS, CSRF
> Designed on: 2026-11-26
> Target tier: 1

## 1. Stack and skills
Existing stack, skill `rust`. No new dependency (S-22).

## 2. Approaches
### Chosen: one module `file_metadata_screen.rs` modeled on `models_screen.rs`
Reuses `token_screen::{CsrfStore, check_csrf, escape_html, resolve_person}` (one `CsrfStore` per router) and calls the typed functions of the tools. Pros: no second implementation (S-2B), the convention is identical. Cons: HTML built by `format!` as the other screens do.
### Rejected: a rust-embed UI from app-design
S-25 is for new apps; the existing screens have no assets and no script, and a page with one asset pipeline would be the only one (S-2B, AUTO-022).
### Rejected: a JSON API plus a script page
Needs a script and a second surface for the same data (S-10).

## 3. Architecture
```
app.rs .merge(file_metadata_screen::router(state))
GET  /app/file-metadata  -> show  -> resolve_person -> authorize -> read_all -> page
POST /app/file-metadata/recalculate -> act -> resolve_person -> authorize -> check_csrf -> derived::recalculate -> page(banner)
```
`read_all` calls, in order: `artifacts::document_view(view text)`, `list_images`, `list_tables`, `document_structure` (a `not found: no artifacts` answer is the empty state), `chunks::list_chunk_sets`, `list_chunks` or `get_chunk`, `derived::list_derived_jobs`.

## 4. Data model
None. The page keeps nothing but its `CsrfStore`.

## 5. Contracts and interfaces
- Query of GET: `mount_id`, `path`, `set_id`, `chunk`, `marker` (chunk list), `images_marker`, `tables_marker`, `headings_marker`.
- Form of POST: `mount_id`, `path`, `kind`, `set_id`, `csrf_token`.
- Status: 200, 401 JSON (no identity), 403, 404, 400 as the tool error maps.
- Error page: a small HTML page naming the error text, escaped.

## 6. Decisions
- DEC-001 (S-2B) `file_metadata_screen.rs`, one module, the other screens' helpers shared.
- DEC-002 (S-05) POST answers the page (200) with the job banner.
- DEC-003 (S-05) Empty sentences: `No conversion yet`, `No chunk set`, `No job`.

## 7. Files touched
`crates/core/src/file_metadata_screen.rs` (new), `file_metadata_screen/e2e.rs` (new, tests), `lib.rs`, `app.rs`; docs: README.md, AGENTS.md, `.agent_docs/architecture.md`, `.agent_docs/testing.md`.

## 8. Stories
- US-0333 the read page (S1, S2, F1, F2, F5, X1, X2, X3), mounted in `app.rs`.
- US-0334 the recalculate action (S3, F3, F4, X4), docs.

## 9. Risks
- A 10000 chunk set must never be read whole: only `list_chunks` (101 rows) and `get_chunk` (1 to 3 rows) are called. Pinned by X2.
- Unescaped value: pinned by X1 on five sources.
