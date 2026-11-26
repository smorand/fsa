---
Id: BL-0180
Title: Move the server and auth settings to the database, part 3/4
Nature: FEAT
Source: auto cut of BL-0173
Created: 2026-11-26
Depends on: BL-0179 (part 2/4)
---

## What
Move the `server` section (listen address, flagged restart) and the `auth` section (the keys that verify callers' tokens) to the generic settings store of part 1.

## Why
One way to set up and to deploy: nothing but the admin database URL and the master key (owner request of 2026-11-19, BL-0173). Cut from BL-0173 by the run 2 cut protocol; part 1/4 (the generic settings store) lands first and every later part reuses its store, tools, screen and change record.

## Scenarios and tests (copied verbatim from the run 2 approval)
Part 5 of the run 2 order, BL-0173 3/4: server (listen address flagged restart), auth (keys that verify tokens; bad key refused before saving; rotation of two keys).

Part specific extras from the approved list: 30s clock skew kept; header precedence unchanged; no auth keys anywhere refuses boot.

Decisions (approved, run 2): YAML stays as optional seed and fallback (dual maintenance, like the SPEC-0102 registry); a value set in the database wins; start with only the admin DSN (--database-url) and FSA_TOKEN_KEY must work. Bootstrap = admin store DSN + FSA_TOKEN_KEY only; the rest moves to the database, restart flagged where needed.

Every BL-0173 part (verbatim): scenarios admin sets value live or flagged restart; start with only db url and key reads db; failures: invalid value refused with today's YAML check message, non admin ERR_FORBIDDEN, secret never returned (has_x + last 4), wrong/missing master key stops boot. Regression: start from today's YAML alone behaves identically, golden config tests unmodified.

BL-0173 extra tests (verbatim): yaml seed imports once, later yaml edit does not override db; restart flagged key not live; secrets redacted in Debug and change record; each section round trips; malformed FSA_TOKEN_KEY stops boot; concurrent set last write wins both recorded; value survives fsa migrate; unknown key/wrong type refused with section name; boot with only url+key end to end; duplicate git host refused (e2e_new_006 unchanged); web lists checked at set time; host token untouched on host list change; git size limit applies next op; context7 off stays off; 30s clock skew kept; header precedence unchanged; no auth keys anywhere refuses boot; blob local->S3 refused when blobs exist unmigrated; PG and SQLite same suite; volume_id scoping kept.

## Notes
- Source of truth for the shared mechanism: SPEC-0114 (part 1/4).
