---
Id: BL-0182
Title: Align the browser screens with the app-design system
Nature: FEAT
Source: owner request of 2026-11-29
Created: 2026-11-29
---

## What
The seven server rendered browser screens (`/app/tokens`, `/app/trash`, `/app/deleted-projects`, `/app/models`, `/app/converters`, `/app/settings`, `/app/file-metadata`) use the design system in `~/projects/perso/app-design/`: its tokens, components and the light and dark themes, with one shared page shell (header, navigation between the screens, theme switch) instead of seven hand written pages.

## Why
Each screen builds its own HTML and styling today, so they look different and a new screen means copying one. The owner keeps one design system for every application.

## Evidence
- Seven screen modules exist: `crates/core/src/{token_screen,trash_screen,deleted_projects_screen,models_screen,converters_screen,settings_screen,file_metadata_screen}.rs`, each repeating `escape_html`, `resolve_person` and `check_csrf` helpers from `token_screen.rs`.
- The design system rules: `~/projects/perso/app-design/AGENTS.md` (colors only through `tokens.css`, three themes, no dashes in texts), `COMPONENTS.md` (which element to use when), `THEMING.md`, with `screens/settings.html` as the pattern for an administrator settings screen.

## Notes
- The assets must be embedded in the binary: no CDN, no npm, no build step at install (standard S-25), copied from the design with a sync script as `inference-proxy` does.
- Only the look and the shared shell change. Routes, session cookie, CSRF, authorization and every tool the screens call stay as they are; the existing screen tests must pass unmodified except for markup assertions.
- The Swagger UI at `/api/docs` is a third party page and stays out of scope.
- Open points for its spec: the screens have no shared navigation today (a person reaches each by its URL); what the navigation lists for a non administrator; whether the file metadata page keeps its project picker as is.
