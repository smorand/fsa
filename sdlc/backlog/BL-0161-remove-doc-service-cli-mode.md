---
Id: BL-0161
Title: Remove the CLI mode of doc_service, keep the API mode
Nature: FEAT
Source: SPEC-0098 class sweep, approved by the owner on 2026-11-18
Created: 2026-11-18
Decision: the owner stated on 2026-11-18 that conversion goes through an external API and that the CLI service is legacy and must be removed; this also closes the unverifiable SPEC-0098 sweep occurrence at `docs/service.rs:394`
---

## What
Remove `doc_service.mode: cli` and the `doc_service.cli` section: the child process launch, its sandbox tempdir, the `{outdir}` and `{document}` placeholders and their tests. Only the HTTP mode (`doc_service.mode: api`) stays, as the one way to convert a document to Markdown for `fs.write_bytes` (`trigger_documentation_service`), the REST `/upload` and `fs.documentize`.

## Why
Legacy path, per the owner. It also runs an external program on member documents, whose resource fetching the server cannot bound (SPEC-0098, class sweep).

## Evidence
- `crates/core/src/docs/service.rs:394`: `tokio::process::Command::new(program)` on a member document.
- `crates/core/src/config.rs:838-853`: `doc_service_mode::CLI` is also the default mode (`d_doc_service_mode`), so the default and the documented example change.
- `crates/core/src/config.rs:881`: `DocServiceCliConfig`.

## Notes
Changes a config contract: a configuration with `mode: cli` must be refused at boot with a clear message. Docs to update: README, `.agent_docs/config.md`, `config/*.template`, `scripts/doc_service_fake.py` usage. Not part of SPEC-0098, which does not depend on it.
