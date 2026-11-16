# SPEC-0096: Recognize `.tbz2` and `.tbz` as bzip2 compressed tar archives, design

> Spec: sdlc/specs/SPEC-0096-recognize-tbz2-and-tbz/spec.md
> Nature: FEAT
> Depth: S
> Security: n/a
> Designed on: 2026-11-15
> Target tier: 2 (default, no `--tier`, no constitution field, no `.spec.json`)

## 1. Stack and skills
| Element | Choice | Version | DEC |
|---|---|---|---|
| Language, edition | Rust 2024, existing workspace | per `Cargo.toml` | none, unchanged |
| bzip2 tar decoding | existing `bzip2::read::BzDecoder` | per `Cargo.toml`, no change | DEC-002 |

No dependency added or changed, so no context7 lookup is needed. Skills loaded: `rust`. Effect: gate is `cargo test --workspace`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo fmt --all -- --check` (project `AGENTS.md`, which governs over the skill's generic Makefile text); tests are `#[tokio::test]` functions named `e2e_NNNN_...` beside the code, the project convention.

## 2. Approaches
### Chosen: two rows in the suffix table
Add `.tbz2` and `.tbz` to the table that maps a name suffix to a format. Pros: one place decides the format, the default destination length comes with the match, the bound and refusals follow the format and need no new code. Cons: none found. Affected scope: `tools/archive.rs` only. Best suited when the new names are plain aliases, which they are.
### Rejected: an alias normalisation step before detection
Rewrite `x.tbz2` to `x.tar.bz2` then match. Rejected (DEC-001): the destination default strips the matched suffix length, so a rewritten name would strip the wrong length and need a second correction.

## 3. Architecture
Unchanged. `detect_format(path)` returns `(ArchiveFormat, suffix_len)` from the table (`crates/core/src/tools/archive.rs:73-84`); the extraction entry point reads it at `:173`, refuses a password for a tar at `:174-178`, and takes the default destination from `suffix_len` at `:210`. The bzip2 tar decoder is chosen on `TarCompression::Bzip2` at `:459`. Both doors (assistant tool and `POST /api/fs/{mount_id}/extract-archive`, `crates/core/src/api/dataplane.rs:176`) call the same function.

## 4. Data model
None.

## 5. Contracts and interfaces
- Suffix table gains `(".tbz2", ArchiveFormat::Tar(TarCompression::Bzip2))` and `(".tbz", ArchiveFormat::Tar(TarCompression::Bzip2))`.
- Error contract unchanged: `unsupported archive extension: '<ext as typed>'` for any name not in the table (`archive.rs:80-83`), `password is not applicable to this archive format`, `archive is corrupt or not a valid tar.bz2 file: <detail>`, `'<path>' already exists; pass overwrite=true to replace it` (`ERR_NO_CLOBBER`).
- Tool schema and descriptions do not list suffixes, so `TOOL_CONTRACT.txt` and `tool-contract-golden.json` do not change (`rg 'tb2' TOOL_CONTRACT.txt` finds nothing).

## 6. File structure impact
| Path | Action | Caused by | Story |
|---|---|---|---|
| `crates/core/src/tools/archive.rs` | modify: two table rows, two rows in the detection unit table (`:1359`) | FR-NEW-001, FR-MOD-001 | US-0245 |
| `crates/core/src/tools/archive_tar_bound_e2e.rs` | modify: rewrite `e2e_0084_042_tbz2_still_unsupported` (`:612`), add the new tests | FR-NEW-001 to 003, FR-MOD-001 | US-0245 |
| `crates/core/src/api/dataplane.rs` | modify: one HTTP door test | FR-NEW-001, FR-NEW-002 (E2E-004) | US-0245 |
| `README.md` | modify lines 294 and 310 | FR-NEW-001 | US-0245 |
| `.agent_docs/tools.md` | modify lines 181 and 283 | FR-NEW-001 | US-0245 |

Affected existing tests: 1 modified (`e2e_0084_042_tbz2_still_unsupported`, justified by FR-MOD-001); the rest keep. Affected documentation: the two above. `AGENTS.md` and SPEC-0018 / SPEC-0083 / SPEC-0084 are not edited (specs are not modified; `AGENTS.md:173` lists no `.tb2`).

## 7. Compatibility and risks
Only visible change: `.tbz2` and `.tbz` move from refused to extracted, declared by FR-MOD-001. Risk: a hostile `.tbz2` meets the same bound as `.tar.bz2` (E2E-005, E2E-006). Rollback: remove the two rows. No dependency added or removed.

## 9. Test strategy
| Test | Level | Driver | Location | Story |
|---|---|---|---|---|
| E2E-001 | functional e2e | tool function | `archive_tar_bound_e2e.rs` | US-0245 |
| E2E-002 | functional e2e | tool function | `archive_tar_bound_e2e.rs` | US-0245 |
| E2E-003 | functional e2e | tool function | `archive_tar_bound_e2e.rs` | US-0245 |
| E2E-004 | functional e2e | REST harness plus tool function | `dataplane.rs` | US-0245 |
| E2E-005 | functional e2e | tool function | `archive_tar_bound_e2e.rs` | US-0245 |
| E2E-006 | functional e2e | tool function | `archive_tar_bound_e2e.rs` | US-0245 |
| E2E-007 | functional e2e | tool function | `archive_tar_bound_e2e.rs` | US-0245 |
| E2E-008 | functional e2e | tool function | `archive_tar_bound_e2e.rs` | US-0245 |
| E2E-009 | functional e2e | tool function | `archive_tar_bound_e2e.rs` | US-0245 |
| E2E-010 | functional e2e | tool function | `archive_tar_bound_e2e.rs` | US-0245 |
| E2E-011 | functional e2e | tool function | `archive_tar_bound_e2e.rs` | US-0245 |
| E2E-MOD-001 | functional e2e | tool function | `archive_tar_bound_e2e.rs` | US-0245 |

Test data: generated by the existing helpers `tar_bytes`, `tar_bomb`, `bz2`, `seed`, `seed_bomb` (no fixture file). Unit rows in `archive.rs:1359` give the fast signal first.

## 10. Decisions log
- **DEC-001:** add the two suffixes as rows of the suffix table, after `.tb2`. **Obliged by:** SPEC-0096/FR-NEW-001, FR-MOD-001. **Rationale:** detection is `ends_with` over the table and the matched length drives the default destination, so a row gives the right destination for free. Order does not matter: `x.tbz2` does not end in `.tbz` (the last four characters are `bz2`, five with the dot are `.tbz2`) and not in `.bz2` (the character before `bz2` is `t`, not a dot); `x.tbz` ends in neither `.tbz2` nor `.bz2`. **Alternatives considered:** a name normalisation step before detection (wrong stripped length). **Source:** existing code. **Evidence:** `crates/core/src/tools/archive.rs:48-69`, `:73-84`, `:210`.
- **DEC-002:** change nothing in the decoder, the bound or the refusal order. **Obliged by:** SPEC-0096/FR-NEW-002, FR-NEW-003. **Rationale:** all of them key on `ArchiveFormat::Tar(TarCompression::Bzip2)`, not on the suffix, so the new rows inherit them; the tests prove it. **Alternatives considered:** a per suffix bound (needless). **Source:** existing code. **Evidence:** `crates/core/src/tools/archive.rs:174-178`, `:459`; SPEC-0084 DEC-004.
- **DEC-003:** put the new tests in `archive_tar_bound_e2e.rs` (the tar family e2e file, already holding `.tb2` and the `.tbz2` pin) and the HTTP door test in `dataplane.rs` (the only place with the REST harness); no new module. **Obliged by:** SPEC-0096/FR-NEW-001, FR-NEW-002, FR-MOD-001. **Rationale:** keeps files touched at 5 and reuses `seed_bomb`, `tar`, `fixture_allowance`. **Alternatives considered:** a new `archive_tbz_e2e.rs` (adds a file and a `tools/mod.rs` edit). **Source:** existing code. **Evidence:** `crates/core/src/tools/archive_tar_bound_e2e.rs:555-619`, `crates/core/src/api/dataplane.rs:3464`.
- **DEC-004:** update `README.md` and `.agent_docs/tools.md`, not `AGENTS.md` or the contract files. **Obliged by:** SPEC-0096/FR-NEW-001 (documentation of the new names). **Rationale:** only those four lines list the suffixes. **Alternatives considered:** none. **Source:** `rg tb2`. **Evidence:** `README.md:294`, `:310`, `.agent_docs/tools.md:181`, `:283`.

## 11. Stories
| Story | Title | Priority | Requirements | Depends on | min_tier | Closes spec |
|---|---|---|---|---|---|---|
| US-0245 | recognize `.tbz2` and `.tbz` as bzip2 compressed tar archives | P1 | FR-NEW-001, FR-NEW-002, FR-NEW-003, FR-MOD-001 | main | 2 | yes |
