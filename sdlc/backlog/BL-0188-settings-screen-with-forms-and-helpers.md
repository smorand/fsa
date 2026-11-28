---
Id: BL-0188
Title: Settings screen with real forms and an (i) helper on every setting
Nature: FEAT
Source: owner request of 2026-11-29, gap analysis item 5
Created: 2026-11-29
---

## What
The administrator settings screen (`/app/settings`) shows each setting as a labeled form field (switch, number with unit, choice list, text, secret), with its default, its current source (default, YAML seed, database), a restart badge, an "(i)" helper that explains what it does and what a wrong value costs, inline validation with the existing messages, a reset to default, and the list of recent changes. Sections are tabs. A first run guide walks through what must be set before use (models, converters, search).

## Why
Today each section is a JSON textarea: a person must know every key, and nothing explains it. The owner wants to configure the whole application from the admin panel.

## Evidence
- `crates/core/src/settings_screen.rs` renders a JSON patch per section; the 14 sections and their keys live in `settings/server_settings*.rs` and `.agent_docs/config.md`.
- Validation messages come from `ServerConfig::check()`.

## Notes
- Depends on BL-0184 for sign in (the screen is already admin only).
- The helper texts and field types come from one table next to the section definitions so they cannot drift from the keys; a test fails when a key has no helper.
- The model registry and converters screens get the same helper treatment.
- Secrets stay write only (has value and last 4 characters).
