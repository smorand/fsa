# Browser screens and the design system

Created 2026-11-30 (SPEC-0118, BL-0182). Read when you add or change one of the seven screens.

## What it is
The seven server rendered screens (`/app/tokens`, `/app/trash`, `/app/deleted-projects`, `/app/file-metadata`,
`/app/models`, `/app/converters`, `/app/settings`) are drawn with the design system of
`~/projects/perso/app-design/` (tokens, components, themes). Only the look is shared: each screen keeps its
routes, identity, one time token and tool calls.

## Where it lives
- `crates/core/src/app_shell/mod.rs`: the one producer of the page document. `page(viewer, Page{screen,title,body,alert})`
  (head, sidebar by role, top bar with the person, page head), `alert`, `empty_state`, `confirm` (modal around the
  original form), `refusal`, `refuse`, `wants_html`, `Viewer::load` (admin, git, member facts read from `AppState`).
- `crates/core/src/app_shell/assets.rs`: `GET /app/assets/{*path}`, no authentication, exact key lookup in the
  `rust-embed` table (`debug-embed` so a debug build and a test read the binary's bytes), `Cache-Control: public,
  max-age=3600`, `nosniff`, 404 for an unknown key or a path with `..`, an empty segment or a backslash.
- `crates/core/web/assets/`: `css/tokens.css`, `css/app.css`, `js/app.js`, `js/icons-data.js`, `js/icons.js`, `logo.svg`,
  `sprite.svg` copied from the design by `scripts/sync-design.sh` (idempotent, writes `MANIFEST.sha256`), plus
  `css/nojs.css` which is ours and loaded only in `<noscript>`.
- Tests: `app_shell/{assets,shell,screens}_e2e.rs`, `cargo test -p fsa-core --lib -- e2e_0118_`. The embedded copies
  are checked against `MANIFEST.sha256` always and against the design folder when it exists.

## Rules
- A screen never writes a `<!doctype>`: it builds a body and calls `app_shell::page`.
- No `style=`, no `<style>`, no inline `<script>`, no hex colour, no `confirm(`; behaviour comes from `app.js`.
- A delete or remove form goes through `app_shell::confirm`; the form stays the same (same fields, same one time
  token), only wrapped in a modal. Without script the modal shows inline (`nojs.css`).
- A refusal: models, converters, settings, file metadata answer the shell with an alert (status and text kept);
  tokens, trash, deleted projects answer JSON unless `Accept` contains `text/html`. A bad CSRF token is 403 JSON everywhere.
- Update the design: `scripts/sync-design.sh` then run the e2e_0118 tests (they fail while the copies drift).
- Theme: the floating bar of `app.js` (light, dark, carbon, green), stored in `localStorage` key `pyr-theme`.
