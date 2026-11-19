---
Id: BL-0165
Title: Document converters configured through the admin interface, CLI and API
Nature: FEAT
Source: owner request of 2026-11-19; replaces BL-0161 (the CLI mode stays)
Created: 2026-11-19
Decision: both converter forms stay, the owner wants a program that takes a file and an HTTP API
---

## What
Move the `doc_service` block from the YAML file to the admin interface (BL-0164). A converter has a name and a form: a program (`PROGRAM <file>`, result on stdout or in an output folder) or an HTTP endpoint, with its secret kept encrypted. A project, or the platform default, names the converter to use.

## Why
Same reason as BL-0164, and the converter is where the document metadata (BL-0166) comes from, so it must be easy to set and to switch.

## Evidence
- `doc_service` block in `crates/core/src/config.rs:931-943` (`mode`, `cli`, `api`, `max_input_bytes`, `extensions`); the token comes from an environment variable (`config/local.yaml.template:71`).
- The program form launches an argv list in a per call temporary directory (`crates/core/src/docs/service.rs:394`).

## Notes
- Security: an administrator who can set an argv can run a program on the server. Only platform administrators may, the change is audited, and the argv stays a list, never a shell string. The spec must say so in its requirements.
- Boot validation of today's YAML becomes validation at save time.
- Keep the converter answer cap `max_input_bytes` and the sandbox rules of SPEC-0006 and SPEC-0089.
