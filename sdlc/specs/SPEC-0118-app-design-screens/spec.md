# Browser screens on the app-design system: functional specification

> Id: SPEC-0118
> Nature: FEAT
> Depth: M
> Depth evidence: FEAT / depth M: 3 areas (embedded assets, one shared shell, seven screens), 14 requirements, no new table, no new tool, one new dependency (`rust-embed`). Auto run 3, single part (BL-0182).
> Status: implemented
> Generated on: 2026-11-30
> Project: existing
> From backlog: BL-0182
> Depends on: SPEC-0014, SPEC-0102, SPEC-0106, SPEC-0111, SPEC-0114
> Security: static assets are public and read only; routes, cookie, CSRF and authorization of the screens do not change
> CVSS: not scored
> Affected: n/a
> Fixed in: n/a

## 1. Summary
The seven server rendered screens (`/app/tokens`, `/app/trash`, `/app/deleted-projects`, `/app/models`, `/app/converters`, `/app/settings`, `/app/file-metadata`) are drawn with the design system of `~/projects/perso/app-design/`: its tokens, components and themes, one shared page shell (sidebar listing the screens the person can open, top bar, page head) and a theme switch, instead of seven hand written pages. The design files are copied into the repository and embedded in the binary; no CDN, no npm, no build step. Only the look changes.

## 2. Current State (MANDATORY)
Each screen module builds a bare HTML document with `format!`, with no stylesheet and no navigation; a person reaches a screen by its URL. `escape_html`, `resolve_person`, `check_csrf` and `CsrfStore` live in `token_screen.rs` and are reused by the six others. Refusals are JSON bodies (`error_response`) or, on the models, converters and settings screens, a bare error page.

### 2.1 What users can do today
- Open each screen by typing its URL; every action is an HTML form post with a one time `csrf_token` when the cookie authenticated.
- No theme, no navigation, no confirmation before a delete: a delete form submits at once.

### 2.2 Specs governing this area
SPEC-0014 (deleted projects screen), SPEC-0011 (trash), SPEC-0102 and SPEC-0106 (models, converters), SPEC-0111 (file metadata), SPEC-0114 (settings), the token screen of the git spec.

### 2.3 What is verified today
Each screen has e2e tests (`token_screen.rs`, `trash_screen.rs`, `deleted_projects_screen.rs`, `models_screen.rs`, `converters_screen.rs`, `settings_screen.rs`, `file_metadata_screen/e2e.rs`), among them the XSS escaping and CSRF tests.

## 3. Scope
### 3.1 In scope
- The design assets (`tokens.css`, `app.css`, `app.js`, `icons-data.js`, `icons.js`, `logo.svg`, `sprite.svg`) copied into `crates/core/web/assets/`, embedded with `rust-embed`, served at `/app/assets/...`, refreshed by `scripts/sync-design.sh`.
- One module `app_shell` (shell, navigation by role, theme, modal, alert, empty state, asset route) called by all seven screens.
- The seven screens in design components; settings follows `screens/settings.html`.
- Light and dark theme chosen in the browser and kept by it.
### 3.2 Out of scope, and why
Routes, session cookie, CSRF, authorization and every tool a screen calls (behavior never changes); `/api/docs` Swagger UI (third party page); new screens; a login page; translations; the Carbon and green themes are present because the design file ships them, not promised.
### 3.3 Constraints imposed by the context
Existing tests pass unmodified except markup assertions, each declared in 9.4. No inline style, no `<style>`, no inline script, no hex colour in markup, no dash as punctuation in a text. Local only, nothing deployed. Standard S-25: nothing fetched at run time.

## 4. Actors
| Actor | Role |
|---|---|
| Platform administrator | sees every screen the server has |
| Member | sees trash, deleted projects, file metadata, tokens when git is on |
| Person with no project | sees trash, deleted projects, tokens when git is on |
| Anonymous | refused as today (401), no styled page |

## 4.5 Bounded contexts
Browser screens, static assets.

## 5. Usage scenarios
### 5.1 Happy
- **S1** A person opens a screen: sidebar with the screens she can open and the current one marked, top bar with her name, page head with title.
- **S2** She switches to dark: it applies at once and survives a reload and a move to another screen.
- **S3** On a small window the sidebar collapses to a rail.
- **S4** She removes a token, deletes a project registry entry or model: a modal asks; cancel does nothing, confirm posts the same form with the same one time token.
- **S5** An empty list shows the design empty state.
### 5.2 Failures
- **F1** No identity: 401 JSON as today, no styled page.
- **F2** Bad or reused CSRF token: 403 JSON as today.
- **F3** A refusal (403, 404, a rejected form) shown to a browser (request accepting `text/html`) is a design alert inside the shell with the same message text; a client not asking for HTML gets the JSON as today.
- **F4** An asset route: unknown name 404; traversal (`..`, `%2e%2e`, double slash, backslash) 404.
### 5.3 Edge
- **E1** Stored text with markup or script stays escaped everywhere, the person name in the shell too.
- **E2** Without script every screen is readable and every form submits; only the theme switch, the sidebar toggle and the confirmation modals need script.
- **E3** File metadata keeps 100 rows per page for 10000 rows.

## 6. Functional requirements
### 6.1 New
- **FR-NEW-001** `crates/core/web/assets/` holds the seven design files; `scripts/sync-design.sh` copies them from `$APP_DESIGN_DIR` (default `~/projects/perso/app-design`), idempotent; a test fails when an embedded copy differs from the design file.
- **FR-NEW-002** `rust-embed` (8.x, `debug-embed`) embeds the folder; `GET /app/assets/{path}` serves a file with its content type, `Cache-Control: public, max-age=3600` and `X-Content-Type-Options: nosniff`, with no authentication; an unknown name or a path with `..`, an empty segment or a backslash is 404.
- **FR-NEW-003** `app_shell` is the only producer of the page document: `<!doctype html>`, `<html lang="en">`, head with title, tokens and app stylesheets and `app.js` loaded in the head (so the stored theme is applied before first paint), body with `.shell`, `.sidebar`, `.topbar`, `.page-head`, `.content`, the icon scripts at the end.
- **FR-NEW-004** The sidebar lists, in this order, tokens (git enabled), trash, deleted projects (everyone), file metadata (member of one project at least), models, converters, settings (platform administrator); a screen the person cannot open is not listed; the current one has `aria-current="page"` and class `active`.
- **FR-NEW-005** The topbar shows the person name, escaped, with an avatar made of its initials; the theme switch is the one of `app.js`.
- **FR-NEW-006** `app_shell` offers an alert (`.alert.danger`, message escaped), an empty state (`.empty`), and a confirmation modal (`.overlay > .modal`, trigger `button type="button" data-open`, cancel `data-close`, the original form inside the modal with its fields and `csrf_token`).
- **FR-NEW-007** A page has no `style=` attribute, no `<style>`, no inline `<script>`, no hex colour, no `confirm(`.
- **FR-NEW-008** Lists are in `.card > .table-wrap > table`, actions in `.btn` (`primary`, `danger`, `ghost`), forms in `.field`; the settings screen uses `.vtabs`.
- **FR-NEW-009** Every delete or remove form (token remove, provider, model, converter delete, default clear) goes through the confirmation modal.
- **FR-NEW-010** A refusal answered to a request that accepts `text/html` is the shell with the alert, same status, same message; otherwise unchanged.
- **FR-NEW-011** `escape_html`, `resolve_person`, `check_csrf`, `CsrfStore` stay the shared helpers of `token_screen`; no screen keeps its own document builder.
- **FR-NEW-012** Routes, cookie, CSRF, authorization, tool calls and redirects are unchanged; the file metadata picker stays a `.card` with a `select`.
- **FR-NEW-013** The sidebar facts (admin, git, member) are computed by one `Viewer::load` from `AppState`, never from the request.
- **FR-NEW-014** README, AGENTS.md and `.agent_docs` describe the screens and the assets.

## 7. Non functional requirements
Security: assets read from an embedded table by exact key, never from a path; no new write. Reliability: nothing is read from disk at run time. Size: about 72 KB of assets in the binary.

## 8. Edge cases
Person name `<b>x</b>`; a project id with markup; 10000 trash or metadata rows; a git disabled server (no tokens route, no link); a request with `Accept: text/html, */*`.

## 9. Acceptance tests (the contract)
### 9.1 Summary
77 tests: shell 4 happy 3 failure 3 edge, navigation 3/5/2, assets 3/5/4, seven screens 14/10/5, theme 2/2/2, modal 2/6/2. Ids `e2e_0118_NNN`.

### 9.2 New tests
| Id | Covers | Given / When / Then |
|---|---|---|
| E2E-001 | S1 FR-NEW-003 | each of the 7 screens renders `.shell .sidebar .topbar .page-head .content`, `<!doctype html>`, `lang="en"` |
| E2E-002 | S1 FR-NEW-003 | head links `/app/assets/css/tokens.css` and `app.css`, loads `app.js` in head, icon scripts at the end of body, no `http` asset |
| E2E-003 | S1 FR-NEW-005 | the person name appears in the topbar |
| E2E-004 | S1 FR-NEW-003 | title and h1 name the screen |
| E2E-005 | F1 | no identity on each of the 7 routes: 401 JSON, no `<html` |
| E2E-006 | F2 | bad CSRF on a post: 403 JSON as today |
| E2E-007 | F3 FR-NEW-010 | a non admin asks `/app/models` with `Accept: text/html`: 403, shell, `.alert.danger` with the same detail as the JSON |
| E2E-008 | E2 FR-NEW-007 | the 7 pages have no `style=`, no `<style`, no inline `<script>` |
| E2E-009 | FR-NEW-007 | no hex colour `#xxxxxx` in the 7 pages |
| E2E-010 | E2 | every nav link is an anchor with an href, every form has method and action |
| E2E-011 | S1 FR-NEW-004 | member, git on: tokens, trash, deleted projects, file metadata exactly |
| E2E-012 | FR-NEW-004 | platform admin, git on: all seven in order |
| E2E-013 | FR-NEW-004 | owner of a project (not admin) sees the member list |
| E2E-014 | FR-NEW-004 | non admin: no models, converters, settings link |
| E2E-015 | FR-NEW-004 | person with no project: no file metadata link |
| E2E-016 | FR-NEW-012 | non admin GET `/app/models`, `/app/converters`, `/app/settings` keep their status |
| E2E-017 | FR-NEW-012 | table over the 7 routes by role gives the statuses of today |
| E2E-018 | FR-NEW-004 | git disabled: no tokens link, `/app/tokens` 404 |
| E2E-019 | FR-NEW-004 | exactly one `aria-current="page"` per page, on the screen shown |
| E2E-020 | FR-NEW-004 | nav hrefs equal route paths without query |
| E2E-021 | FR-NEW-002 | `tokens.css` answers 200 `text/css`, cache header, nosniff |
| E2E-022 | FR-NEW-002 | `app.js` answers 200 JavaScript |
| E2E-023 | FR-NEW-002 | `sprite.svg` answers 200 `image/svg+xml` |
| E2E-024 | F4 | unknown asset 404 |
| E2E-025 | F4 | `/app/assets/../x` 404 |
| E2E-026 | F4 | `/app/assets/%2e%2e/x` 404 |
| E2E-027 | F4 | `/app/assets//css/app.css` 404 |
| E2E-028 | F4 | `/app/assets/css%5capp.css` 404 |
| E2E-029 | FR-NEW-001 | embedded copies equal the design files (skipped with a message when the design folder is absent) |
| E2E-030 | FR-NEW-002 | each of the seven files is in the embedded table and served from a working directory with no `web` folder |
| E2E-031 | FR-NEW-002 | assets answer with no identity |
| E2E-032 | FR-NEW-001 | `sync-design.sh` run twice leaves the tree unchanged and exits 1 on a missing source |
| E2E-033 | FR-NEW-008 | tokens screen: list in `.card .table-wrap`, buttons `.btn` |
| E2E-034 | FR-NEW-012 | tokens screen: add and remove still redirect as today |
| E2E-035 | FR-NEW-008 | trash screen: rows in `.table-wrap`, restore `.btn` |
| E2E-036 | FR-NEW-012 | trash restore still redirects |
| E2E-037 | FR-NEW-008 | deleted projects: table in design |
| E2E-038 | FR-NEW-012 | undelete still redirects |
| E2E-039 | FR-NEW-008 | models: providers, models, defaults in cards |
| E2E-040 | FR-NEW-012 | model create still redirects |
| E2E-041 | FR-NEW-008 | converters in cards |
| E2E-042 | FR-NEW-012 | converter create still redirects |
| E2E-043 | FR-NEW-008 | settings follows `.vtabs` with one panel per section |
| E2E-044 | FR-NEW-012 | a settings change still posts as today |
| E2E-045 | FR-NEW-008 | file metadata: picker is a `.card` with `select` |
| E2E-046 | FR-NEW-008 | file metadata: tables in `.table-wrap` |
| E2E-047 | S5 | empty trash: `.empty` state |
| E2E-048 | S5 | no deleted project: `.empty` state |
| E2E-049 | S5 | no token: `.empty` state |
| E2E-050 | S5 | no provider: `.empty` state |
| E2E-051 | F3 | trash of a project the person is not in, `Accept: text/html`: shell alert, same status |
| E2E-052 | F3 | rejected model form shows `.alert.danger` with the message |
| E2E-053 | F3 | rejected converter form shows `.alert.danger` |
| E2E-054 | F3 | rejected settings change shows `.alert.danger` |
| E2E-055 | F3 | file metadata of a project the person is not in, HTML: alert |
| E2E-056 | F3 | tokens screen refusal in HTML: alert |
| E2E-057 | E1 | a person named `<b>x</b>` is escaped in the shell of the 7 screens |
| E2E-058 | E1 | a project id with markup is escaped in nav free pages (trash picker, file metadata picker) |
| E2E-059 | E3 | 10000 rows: file metadata page holds 100 rows |
| E2E-060 | E1 | stored markup in a trash path is escaped |
| E2E-061 | E1 | stored markup in a model name is escaped |
| E2E-062 | S2 FR-NEW-003 | `app.js` is a script of the head, before the body |
| E2E-063 | S2 | `app.js` stores the theme under `pyr-theme` and reads it at load |
| E2E-064 | S2 | no `data-theme` set inline, the script sets it |
| E2E-065 | S2 | `tokens.css` defines the `dark` theme |
| E2E-066 | S3 | `app.css` holds the rail rule (`.shell.collapsed`) and a narrow window rule |
| E2E-067 | S3 | the sidebar has a toggle button with `data-sidebar-toggle` |
| E2E-068 | S4 FR-NEW-009 | models page: delete model has a trigger `data-open` and an `.overlay .modal` |
| E2E-069 | S4 | the cancel button has `data-close` and is `type="button"` |
| E2E-070 | S4 | the form in the modal carries the same `csrf_token` as the page |
| E2E-071 | S4 | posting the modal form fields without script deletes as today |
| E2E-072 | F2 | the modal form with a bad token: 403 |
| E2E-073 | S4 | two rows give two distinct modal ids |
| E2E-074 | E1 | a model name with markup is escaped in the modal |
| E2E-075 | S4 | the trigger is `type="button"` |
| E2E-076 | S4 | token remove goes through a modal |
| E2E-077 | FR-NEW-007 | no `confirm(` in any page |

### 9.3 Regression tests
All existing screen tests (XSS, CSRF, authorization, redirects).

### 9.4 Modified tests [existing only]
Markup assertions that name the old bare tags (a page title, `<table>` opening tag, a bare `<button type="submit">`) are edited to the new markup, each listed in the story commit with its reason. Tool counts do not change.

## 10. Traceability
S1 E2E-001 to 004, 011 to 020; S2 E2E-062 to 065; S3 E2E-066 067; S4 E2E-068 to 076; S5 E2E-047 to 050; F1 E2E-005; F2 E2E-006 072; F3 E2E-007 051 to 056; F4 E2E-024 to 028; E1 E2E-057 to 061 074; E2 E2E-008 010; E3 E2E-059.

## 11. Open questions
None that changes behavior.

## 12. Glossary
Shell: the page frame (sidebar, top bar, head). Rail: the collapsed sidebar showing icons only.

## 13. Implementability gate
Gate record: round 1 (independent auditor): NOT-IMPLEMENTABLE, 15 findings (6 structural, 9 precision), merged as G1 to G10; round 2 (new auditor): NOT-IMPLEMENTABLE, 10 findings (2 structural on the CSRF refusal and the wording of FR-NEW-010, 8 precision), merged as G11 to G17; round 3 (new auditor): IMPLEMENTABLE with 8 precision notes, merged as G18 to G24. The gate closed at round 3 of 4.

Decisions taken on round 1:
- G1 (theme switch): the switch is the floating `.proposal-bar` that `app.js` builds on every page (buttons `data-t`: light, dark, carbon light, carbon dark, green). The contract is light and dark persisted under `localStorage` key `pyr-theme` and read by `app.js` in the head; the other themes come with the file and are not promised. E2E-063 and 064 check the asset and that the page sets no theme itself; the persistence in a browser is shown by the headless Chrome check of the run, not by a Rust test.
- G2 (refusals): the models, converters, settings and file metadata screens answer a refusal with an HTML page today (status kept, text `e.to_string()`); that page becomes the shell with the alert, always, same status, same text. The tokens, trash and deleted projects screens answer JSON today; they keep the JSON unless `Accept` contains `text/html` (case blind). `*/*` alone, `application/json` or no `Accept` is JSON. E2E-007 reads: non admin, `/app/models`, status 403, shell, alert with the text the old page held.
- G3 (statuses of today, E2E-016 and 017): GET with a valid identity: `/app/tokens` 200 (git on) or 404 (git off), `/app/trash` 200, `/app/deleted-projects` 200, `/app/file-metadata` 200, `/app/models` `/app/converters` `/app/settings` 200 for a platform administrator and 403 for anyone else; no identity 401 everywhere (404 on tokens with git off).
- G4 (tabs and modals without script): the head holds `<noscript><link rel="stylesheet" href="/app/assets/css/nojs.css"></noscript>`. `nojs.css` is the one file of ours in the assets folder (not copied from the design, not in the sync list): it shows every `.panel`, shows every `.overlay` as an inline block and hides every `[data-open]` trigger, so without script a confirmation form is a plain visible form with its confirm button. E2E-071 reads the page for the `nojs.css` link and the rules.
- G5 (member): a person with at least one project in `list_projects_for` (owner or member). Deleted projects and trash are listed for everyone because their routes answer 200 for anyone.
- G6 (current marker): by screen, on every state of it (`?mount_id=`, a refusal page).
- G7 (destructive actions): tokens `/app/tokens/revoke`; models `provider/delete`, `model/delete`, `default/clear`; converters the delete and unset actions of its `{group}/{action}` route; the other actions stay plain forms.
- G8 (texts): modal heading `Remove this token?`, `Delete this provider?`, `Delete this model?`, `Delete this converter?`, `Clear this default?`; body names the object; buttons `Cancel` and the verb. Empty states: trash `No trashed files`, deleted projects `No deleted project`, tokens `No token`, providers `No provider`, converters `No converter`, file metadata `No project`.
- G9 (E2E-029): an always run test compares the embedded files with `web/assets/MANIFEST.sha256` (written by `sync-design.sh`); the comparison with the design folder runs when the folder exists and reports the skip on stderr.
- G11 (CSRF): a bad or reused one time token is 403 JSON on every screen and for every `Accept`, as today; it is not a refusal in the sense of F3 and FR-NEW-010. E2E-006 and 072 send `Accept: text/html` and still read JSON.
- G12 (FR-NEW-010 wording): by screen, as G2. Models, converters, settings, file metadata: always the shell with the alert, for every `Accept`. Tokens, trash, deleted projects: JSON unless `Accept` contains `text/html`. E2E-051, 052, 053, 054, 055, 056 send `Accept: text/html`.
- G13 (destructive actions, complete list): tokens `revoke` (object named by host); models `provider/delete`, `model/delete` (named by name), `default/clear` (named by kind); converters `converter/delete` (named by name), `default/clear`, `project/clear` (headings `Delete this converter?`, `Clear this default?`, `Clear this project converter?`). Nothing else goes through a modal.
- G14 (settings): one tab and one panel per section returned by `list_settings`, in its order, then a last tab `Latest changes`; a panel holds the source line, the secrets, the value, and the JSON form (textarea, `Save`).
- G15 (E3, E2E-059): the 100 applies to the chunk list of file metadata (`data-chunk` rows); the next page is the `More chunks` link with its marker; the test counts 100 rows.
- G16 (member lookup): a failing `list_projects_for` hides the file metadata link and the page renders; the sidebar order is always the order of FR-NEW-004.
- G17 (S2, acceptance and the no script file): S2 has no Rust assertion beyond the asset checks (E2E-062 to 065); it is accepted by the headless Chrome check of the run (a theme stored under `pyr-theme` survives a reload and a move to another screen). `nojs.css` is served at `/app/assets/css/nojs.css`, is not in the sync list or the manifest, and also hides every `[data-close]`; E2E-071 reads it through the route.
- G18 (order of checks): on models, converters, settings and file metadata posts the identity check comes first, then the administrator or membership refusal (the shell with the alert), then the one time token (403 JSON). A non administrator with a bad token gets the shell with the alert, as the old page did. FR-NEW-009 and G7 are superseded by G13 for the list of actions.
- G19 (E2E-005): git is on. With git off `/app/tokens` is 404 whatever the identity.
- G20 (avatar): the first two letters or digits of the part of the name before `@`, upper case, `?` when none.
- G21 (empty states): trash picker `No project`; models defaults always show their two rows; no other empty state.
- G22 (modal ids): a row index (`m-remove-0`, `m-model-3`, `m-provider-2`), or for converters the converter id (`m-converter-<id>`), a project id (`m-project-<id>`) or `m-default-clear`; ids are escaped.
- G23 (files): the seven design files are `css/tokens.css`, `css/app.css`, `js/app.js`, `js/icons-data.js`, `js/icons.js`, `logo.svg`, `sprite.svg`; `nojs.css` and `MANIFEST.sha256` sit beside them.
- G24 (floating bar text): the word `Proposition` and the theme names on the bar are the design's own text, embedded as it is (a local edit would make the sync check fail); reported as a concern. E2E-016 is the subset of E2E-017 for the non administrator and is kept as one test; the counts of 9.1 are those of the table.
- E2E-058 reads: a project id or a path with markup is escaped in the trash picker and the file metadata picker.
- G10 (hex colour): the test looks for `#` followed by 6 hex digits not preceded by `href="`; it runs on markup only, not on assets.

## 14. Evidence appendix
### 14.1 Current State evidence
Seven screen modules named in 1; `app.rs` mounts them; `token_screen.rs` holds the shared helpers; `~/projects/perso/inference-proxy/crates/core/src/web/mod.rs` embeds with `rust-embed` and `scripts/sync-design.sh` copies.
### 14.2 Existing tests to change
Markup assertions only (9.4).
### 14.3 Design inputs
Design files: `AGENTS.md`, `COMPONENTS.md`, `THEMING.md`, `screens/settings.html`, `screens/dashboard.html` of app-design.
