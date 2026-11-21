---
Id: BL-0174
Title: Derived metadata is rebuilt after the file stops changing, and can be rebuilt by hand
Nature: FEAT
Source: owner request of 2026-11-20 during the chunking scope
Created: 2026-11-20
---

## What
Chunk sets (BL-0168) and document conversion (CLI or API converter, BL-0165, BL-0166) are rebuilt automatically when their file is written, but only after the file has been quiet for a settle delay. Each new write restarts the delay, so a file that keeps changing is not recalculated until it stops. The delay is configurable. Any derived set can also be recalculated by hand through an API call and a tool, and the UI (BL-0175) offers the same action. A job reports its state.

## Why
Recalculating on every write of an often modified file wastes conversion and embedding work.

## Evidence
- Today the auto indexer runs on every write of a file (`crates/core/src/search/indexer.rs`, `on_write`), and conversion runs only when asked (`fs.documentize`, `trigger_documentation_service` on upload).

## Notes
- Foundation of BL-0168: build it first.
- Open for its spec: the default delay, per project override, what happens to a job in flight when the file changes, and what a reader sees while a rebuild is pending.
