# SPEC-0118: Browser screens on the app-design system, design

> Spec: sdlc/specs/SPEC-0118-app-design-screens/spec.md
> Nature: FEAT
> Depth: M
> Security: public read only static assets; screens keep their identity, CSRF and authorization
> Designed on: 2026-11-30
> Target tier: 1

## 1. Stack and skills
Existing stack, skill `rust`. One new dependency, `rust-embed` 8.14 with `debug-embed` (S-22; version checked with context7 and `cargo search`).

## 2. Approaches
### Chosen: one `app_shell` module and the design files embedded with `rust-embed`
The screens keep their handlers and their tool calls and hand a body to `app_shell::page`. The assets are copied from `app-design` by a sync script, embedded in the binary, served from an exact key table. This is what `inference-proxy` does, so the owner keeps one pattern.
### Rejected: `include_dir`
It embeds a folder too, but gives no content type, no debug-embed switch and no exclude rules; `rust-embed` is the crate `inference-proxy` already uses (DEC-001).
### Rejected: a template engine (askama, maud)
Seven screens of generated strings already exist with tests on their markup; a template engine rewrites every one for no behavior gain and adds a dependency (S-22).
### Rejected: link the stylesheet from a CDN or read the design folder at run time
S-25: nothing fetched or read from outside the binary at run time.

## 3. Architecture
```
GET /app/<screen> -> resolve_person -> tool call as today -> body html
   -> Viewer::load(state, person)  (admin, git, member) -> app_shell::page(viewer, Page{screen,title,body,alert})
GET /app/assets/{*path} -> is_plain_key -> Assets::get(key) -> bytes + type + cache + nosniff
refusal: app_shell::refuse(...) -> JSON as before, or the shell with an alert (G2, G11, G12)
```

## 4. Data model
None.

## 5. Contracts and interfaces
`app_shell::{Screen, Viewer, Page, page, alert, empty_state, confirm, refusal, refuse, wants_html, html, icon}`; `app_shell::assets::{Assets, router}`. Screens keep `escape_html`, `resolve_person`, `check_csrf`, `CsrfStore` from `token_screen` (the trash and deleted projects screens keep the copies they already had, left as they are). No tool, route, cookie or contract change.

## 6. File structure impact
New `crates/core/src/app_shell/{mod,assets}.rs` and tests, `crates/core/web/assets/` (seven design files, `MANIFEST.sha256`, our `css/nojs.css`), `scripts/sync-design.sh`. The seven screen modules lose their document builders.

## 7. Compatibility and risks
Markup assertions in two existing tests change (9.4). The `app.js` floating theme bar is the design's own widget (G1). Risk: `app.js` loaded in the head runs before the body exists, it only touches `documentElement` at once and waits for `DOMContentLoaded` (checked in the file).

## 8. Security design
Assets read by exact key (traversal has nothing to reach; `..`, empty segment and backslash refused anyway). Person names and every stored text pass `escape_html`; a page has no inline script or style. `X-Content-Type-Options: nosniff` on assets.

## 9. Test strategy
`e2e_0118_NNN` in `app_shell/{assets,shell,screens}_e2e.rs`, written first for assets and shell (RED shown), group B tests after the screens. Existing XSS and CSRF tests run unmodified.

## 10. Decisions log
- DEC-001 (S-22) `rust-embed` over `include_dir`.
- DEC-002 (S-25) sync script plus manifest, test fails on drift.
- DEC-003 (S-5A) one shell module, the screens never write a document.
- DEC-004 (S-10) refusals by screen as G2 and G11.
- DEC-005 `nojs.css` is ours, loaded in `<noscript>`.

## 11. Stories
| Story | Covers | Tier |
|---|---|---|
| US-0358 embedded assets and sync | FR-NEW-001, 002 | 1 |
| US-0359 the shell and the tokens, trash, deleted projects screens | FR-NEW-003 to 007, 011, 013 | 1 |
| US-0360 models, converters, settings screens | FR-NEW-008 to 010, 012 | 1 |
| US-0361 file metadata screen, docs | FR-NEW-012, 014 | 1 |

### Test to story map
US-0358: E2E-021 to 032, 063, 065 to 067. US-0359: E2E-001 to 006, 008 to 015, 018 to 020, 033 to 038, 047 to 049, 051, 056, 057, 058, 060, 062, 064, 076, 077. US-0360: E2E-007, 016, 017, 039 to 044, 050, 052 to 054, 061, 068 to 075. US-0361: E2E-045, 046, 055, 059.

## 12. Drift notes (implementation)
- DRIFT-001 Test ids are merged into 43 test functions named by the ids they cover (`e2e_0118_041_042_053_...`); E2E-059 is covered by the existing `e2e_0111` 10000 chunk test, which passes unmodified.
- DRIFT-002 E2E-046 asserts the recalculate refusal (search is off in the test server) instead of a jobs table; the tables of the file metadata page keep their `table-wrap` through the existing `e2e_0111` tests.
- DRIFT-003 Below 900 px the design shows an off canvas menu opened by a button, not a rail (the rail is the wide window collapse).
