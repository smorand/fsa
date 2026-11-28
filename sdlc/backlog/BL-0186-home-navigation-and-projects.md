---
Id: BL-0186
Title: Home page, navigation and projects: list, create, members and quota
Nature: FEAT
Source: owner request of 2026-11-29, gap analysis items 1 and 4
Created: 2026-11-29
---

## What
Signing in lands on a home page at `/` (and `/app`) built on the app-design shell: the person's projects with role, usage against the quota and last activity, a button to create a project, and the menu to every screen they may open (administration only for a platform administrator). A project page shows its members and lets an owner add or remove members and change roles, shows quota and disk usage, and opens the file explorer (BL-0187). Any signed in person can create a project and becomes its owner; a setting turns that off.

## Why
Today `/` and `/app` answer 404, projects are created only through the MCP tool `admin.create_project`, and the seven screens are reachable only by typing their URL.

## Evidence
- No route for `/` or `/app` (`crates/core/src/app.rs:198-233`).
- `GET /api/fs/roots` lists a person's projects (`api/dataplane.rs:154`); quota and usage routes exist (`:207-209`); membership and project tools exist in `tools/admin.rs`.
- The shared shell and role based navigation exist (BL-0182, `app_shell`).

## Notes
- Depends on BL-0184.
- Today platform admin manages projects and membership without file access; keep that separation (AGENTS.md convention) and say in the spec what an administrator sees of a project.
- Project creation by a user is a change of the current rule: record it as a decision, with the setting.
- Reuse the existing tools and routes, no second implementation.
