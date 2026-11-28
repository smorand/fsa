---
Id: BL-0183
Title: Install with `make install` and start from an empty database with two variables
Nature: FEAT
Source: owner request of 2026-11-29, gap analysis items 6, 7 and 8
Created: 2026-11-29
---

## What
A person installs with `make install` (binary into `~/.local/bin`, `make uninstall` removes it), sets two environment variables and runs `fsa`: `FSA_DATABASE_URL` (the admin database) and `FSA_KEY_FILE` (path of the file holding the key that encrypts sensitive values in the database; created with mode 600 at the first start when absent; `FSA_TOKEN_KEY` stays as an alternative holding the key itself). No YAML file, no `run.sh`. A start on an empty database works with built in defaults, creates its data directories under the user's data folder, and opens a setup page at the root that creates the first administrator (finished by BL-0184). `./run.sh` is removed or reduced to a development helper.

## Why
Today there is no `make install`, a YAML start is needed once to import the first auth key and administrator, the key variable holds the key text, and data lives in relative `state/` folders.

## Evidence
- The Makefile has no `install` or `uninstall` target (`Makefile:1-75`); `run.sh` copies a YAML template, generates a dev keypair and builds.
- `FSA_DATABASE_URL` is read in `crates/core/src/cli.rs:23,183`; the key is read as text from `FSA_TOKEN_KEY` (`MCPFS_TOKEN_KEY` is deprecated and still set in the owner's `.env`).
- SPEC-0116 concern: bootstrapping an empty database still needs one YAML start (auth key and administrator).
- `rust-toolchain.toml` pins 1.94.1 but datafusion 55 needs 1.95 (AUTO-001); the build fails with the pin.
- The old local volume `docling-scripts` fails every purge cycle with `table nodes has no column named volume_id`.

## Notes
- Fix the toolchain pin so `make install` builds on a clean machine.
- Defaults for data directories are written to the settings (BL-0173) at the first start and can be changed there.
- Decide in its spec what the setup page shows once an administrator exists (nothing, a 404).
- Clean up what blocks a finished look: the old volume that fails the purge cycle (migrate or skip with a clear message, never delete data), and the "Proposition" label of the theme bar (fixed in `~/projects/perso/app-design`, then `scripts/sync-design.sh`).
- Hard stop S-90: nothing deletes or rewrites existing data.
