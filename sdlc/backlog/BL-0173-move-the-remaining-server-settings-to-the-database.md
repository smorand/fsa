---
Id: BL-0173
Title: Move the remaining server settings to the database, leaving the database URL and the master key
Nature: FEAT
Source: owner request of 2026-11-19: no configuration file, no environment variable beyond the database URL
Created: 2026-11-19
---

## What
After BL-0164 and BL-0165, move every other section of the YAML configuration (server, auth, blob store, safety, git, web, context7, search and the rest) to the admin interface, so a deployment needs only the database URL and the master key.

## Why
One way to set up and to deploy, as the owner asked.

## Evidence
- `ServerConfig` holds 14 sections loaded from YAML with `${VAR}` expansion (`crates/core/src/config.rs:958-975`).
- Values needed before the database is reachable (the listen address, the key that checks the callers' tokens) need a bootstrap answer.

## Notes
- This is the largest of the roadmap and may split in several specs; scope it after BL-0164 shows how the settings store behaves.
