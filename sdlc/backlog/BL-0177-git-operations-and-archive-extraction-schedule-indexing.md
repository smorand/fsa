---
Id: BL-0177
Title: Git operations and archive extraction schedule the indexing of the files they change
Nature: FEAT
Source: cut from BL-0174 by the owner on 2026-11-20 (SPEC-0100 gate)
Created: 2026-11-20
---

## What
Files that a git operation (checkout, merge, pull, reset, revert, stash apply, cherry pick) or an archive extraction creates or changes are scheduled for indexing like any written file; files they delete or rename follow the delete and move rules of BL-0174.

## Why
Today these files are never indexed until they are written again, so search silently misses them.

## Evidence
- No indexer hook exists in git or archive code: `rg indexer:: crates/core/src` finds hooks only in write, edit, document, lifecycle, trash, admin, the MCP server and the REST plane.

## Notes
- Depends on BL-0174. Which operations count and how a rename shows up in each one is for its spec.
