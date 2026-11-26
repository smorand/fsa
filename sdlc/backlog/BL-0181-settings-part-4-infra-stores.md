---
Id: BL-0181
Title: Move the infra store settings to the database, part 4/4
Nature: FEAT
Source: auto cut of BL-0173
Created: 2026-11-26
Depends on: BL-0180 (part 3/4)
---

## What
Move the `infra` section (blob store, meta, git and oauth stores) to the generic settings store of part 1. Only the admin store DSN stays outside the database.

## Why
One way to set up and to deploy: nothing but the admin database URL and the master key (owner request of 2026-11-19, BL-0173). Cut from BL-0173 by the run 2 cut protocol; part 1/4 (the generic settings store) lands first and every later part reuses its store, tools, screen and change record.

## Scenarios and tests (copied verbatim from the run 2 approval)
Part 6 of the run 2 order, BL-0173 4/4: infra: blob store and meta, git, oauth stores; only admin store DSN stays outside; DSN change refused until fsa migrate done; unreachable DSN refused before saving; never moves data itself (S-90). S3 secret encrypted.

Part specific extras from the approved list: blob local->S3 refused when blobs exist unmigrated; value survives fsa migrate; volume_id scoping kept.

Decisions (approved, run 2): YAML stays as optional seed and fallback (dual maintenance, like the SPEC-0102 registry); a value set in the database wins; start with only the admin DSN (--database-url) and FSA_TOKEN_KEY must work. Bootstrap = admin store DSN + FSA_TOKEN_KEY only; the rest moves to the database, restart flagged where needed.

Every BL-0173 part (verbatim): scenarios admin sets value live or flagged restart; start with only db url and key reads db; failures: invalid value refused with today's YAML check message, non admin ERR_FORBIDDEN, secret never returned (has_x + last 4), wrong/missing master key stops boot. Regression: start from today's YAML alone behaves identically, golden config tests unmodified.

BL-0173 extra tests (verbatim): yaml seed imports once, later yaml edit does not override db; restart flagged key not live; secrets redacted in Debug and change record; each section round trips; malformed FSA_TOKEN_KEY stops boot; concurrent set last write wins both recorded; value survives fsa migrate; unknown key/wrong type refused with section name; boot with only url+key end to end; duplicate git host refused (e2e_new_006 unchanged); web lists checked at set time; host token untouched on host list change; git size limit applies next op; context7 off stays off; 30s clock skew kept; header precedence unchanged; no auth keys anywhere refuses boot; blob local->S3 refused when blobs exist unmigrated; PG and SQLite same suite; volume_id scoping kept.

## Notes
- Source of truth for the shared mechanism: SPEC-0114 (part 1/4).
