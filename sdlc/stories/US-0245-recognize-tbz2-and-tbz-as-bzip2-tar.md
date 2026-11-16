# US-0245: recognize `.tbz2` and `.tbz` as bzip2 compressed tar archives

> Spec: SPEC-0096 (sdlc/specs/SPEC-0096-recognize-tbz2-and-tbz/)
> Nature: FEAT
> Priority: P1
> Depends on: main
> min_tier: 2
> Files touched: 5
> Closes spec: yes

## Objective
A member who extracts a bzip2 compressed tar named `.tbz2` or `.tbz` gets it extracted like a `.tar.bz2`, with the same destination, bound and refusals, from the assistant tool and the HTTP API. The one thing that changes is that these two names stop being refused.

## Requirements covered
- SPEC-0096/FR-NEW-001: WHEN a member extracts a file whose name ends in `.tbz2` or `.tbz`, in any letter case, THE archive extraction SHALL extract it as a bzip2 compressed tar archive, exactly as it extracts a file ending in `.tar.bz2` or `.tb2`.
- SPEC-0096/FR-NEW-002: THE archive extraction SHALL apply to a `.tbz2` or `.tbz` file, in any letter case, the decoding bound and every refusal, with the same codes, messages and order, that apply to a `.tar.bz2` file.
- SPEC-0096/FR-NEW-003: THE archive extraction SHALL NOT change how it extracts or refuses any file whose name does not end in `.tbz2` or `.tbz`.
- SPEC-0096/FR-MOD-001: WHEN a member extracts a file ending in `.tbz2`, THE archive extraction SHALL extract it as FR-NEW-001 states instead of refusing it with `unsupported archive extension: '.tbz2'`.

## Independent Test
`cargo test -p mcp-fs-core --lib -- e2e_0096_ detect_format_maps_every_suffix e2e_0084_ e2e_0083_ e2e_0086_ e2e_new_031`: all pass, including the 12 new `e2e_0096_` tests; then `./test.sh` stays green. Real assertions: bytes read back from the store at `/uploads/pack/a.txt`, refusal codes and messages, and `exists()` false after each refusal.

## Design excerpt
### Decisions
- **DEC-001 (SPEC-0096):** add the two suffixes as rows of the suffix table, after `.tb2`. Row order does not matter (`x.tbz2` ends in neither `.tbz` nor `.bz2`). Alternative rejected: a name normalisation step (wrong stripped length).
- **DEC-002 (SPEC-0096):** change nothing in the decoder, the bound or the refusal order: they key on `ArchiveFormat::Tar(TarCompression::Bzip2)`.
- **DEC-003 (SPEC-0096):** new tests go in `archive_tar_bound_e2e.rs`; the HTTP door test goes in `dataplane.rs`; no new module.
- **DEC-004 (SPEC-0096):** docs: `README.md` lines 294 and 310, `.agent_docs/tools.md` lines 181 and 283.
### Data model
None.
### Contracts
Table rows to add in `SUFFIXES` (`crates/core/src/tools/archive.rs:48`), right after the `.tb2` row:
```rust
    (".tbz2", ArchiveFormat::Tar(TarCompression::Bzip2)),
    (".tbz", ArchiveFormat::Tar(TarCompression::Bzip2)),
```
Messages stay as they are: `unsupported archive extension: '<ext as typed>'`, `password is not applicable to this archive format`, `archive is corrupt or not a valid tar.bz2 file: <detail>`, `'<path>' already exists; pass overwrite=true to replace it`.
### Paths
| Path | Action |
|---|---|
| `crates/core/src/tools/archive.rs` | two rows in `SUFFIXES`; two rows `("/a/x.tbz2", Tar(Bzip2), 5)` and `("/a/x.tbz", Tar(Bzip2), 4)` in the table at `:1359` |
| `crates/core/src/tools/archive_tar_bound_e2e.rs` | rewrite `e2e_0084_042_tbz2_still_unsupported` (`:612`) as E2E-MOD-001; add E2E-001 to 003 and 005 to 011 |
| `crates/core/src/api/dataplane.rs` | add E2E-004 beside `e2e_new_031_extract_archive_happy_path` (`:3464`) |
| `README.md` | add `.tbz2`/`.tbz` to the lists at `:294` and `:310` |
| `.agent_docs/tools.md` | add them to the lists at `:181` and `:283` |
### Existing patterns
- Extraction tests: `crates/core/src/tools/archive_tar_bound_e2e.rs:555-566` (`e2e_0084_002`: `fixture()`, `seed(&f, path, &bz2(&tar()))`, `extract(&f, path, None, false, None)`, `extracted_one_file`, `read`).
- Bombs: `:262-273` and `:275-298` (`seed_bomb(&f, path, bz2(&tar_bomb()))`, `fixture_allowance(1000)`, `limit_remaining(&f, 1000)`, `project_refusal`, `session_refusal`, `held_reset`, `held_at_most(1_049_576)`).
- Corrupt: `refused_corrupt` at `:49-55` uses `corrupt_refusal(&v, "tar.xz")`; use `"tar.bz2"`.
- HTTP door: `dataplane.rs:3464-3476` (`Harness::new()`, `h.post(&u("extract-archive"), json!({"path": ...}))`).

## Acceptance tests
> 100% must pass, run through the project gate (`./test.sh`, `cargo clippy ... -D warnings`, `cargo fmt --all -- --check`).

### SPEC-0096/E2E-001: `.tbz2` extracts
- Level / Driver / Location: functional e2e / tool function / `archive_tar_bound_e2e.rs`, function `e2e_0096_001_tbz2_extracts`
- Given a member and `/uploads/pack.tbz2` = `bz2(tar())` (the pack: `a.txt` = `hello`); nothing at `/uploads/pack`
- When the member extracts `/uploads/pack.tbz2`
- Then the answer has destination `/uploads/pack`, `files_written` 1, `bytes_written` 5
- And `/uploads/pack/a.txt` holds `hello`

### SPEC-0096/E2E-002: `.tbz` and letter case
- Level / Driver / Location: functional e2e / tool function / `archive_tar_bound_e2e.rs`, function `e2e_0096_002_tbz_and_letter_case`
- Given the pack stored as `/uploads/pack.tbz`, `/u2/PACK.TBZ2` and `/u3/Pack.Tbz`
- When each is extracted in turn
- Then destinations are `/uploads/pack`, `/u2/PACK` and `/u3/Pack`, each with 1 file and 5 bytes written
- And each `a.txt` under them holds `hello`

### SPEC-0096/E2E-003: explicit destination
- Level / Driver / Location: functional e2e / tool function / `archive_tar_bound_e2e.rs`, function `e2e_0096_003_explicit_destination`
- Given `/uploads/pack.tbz2`, the pack
- When it is extracted with destination `/out/here`
- Then destination `/out/here`, 1 file, 5 bytes
- And `/out/here/a.txt` holds `hello` and `/uploads/pack` does not exist

### SPEC-0096/E2E-004: the HTTP API gives the same answer
- Level / Driver / Location: functional e2e / REST harness / `dataplane.rs`, function `e2e_0096_004_tbz2_through_the_http_api`
- Given `/uploads/pack.tbz2` and `/uploads/other.tar.bz2`, both the pack
- When `/uploads/pack.tbz2` is extracted through `h.post(&u("extract-archive"), ...)`, then `/uploads/other.tar.bz2` through the same route
- Then both statuses are 200, the first answer has `destination` `/uploads/pack`, `files_written` 1 and `bytes_written` 5, and the two answers are equal in every field except `destination` (`/uploads/other` for the second)
- And `/uploads/pack/a.txt` holds `hello`

### SPEC-0096/E2E-005: `.tbz2` bomb against the session allowance
- Level / Driver / Location: functional e2e / tool function / `archive_tar_bound_e2e.rs`, function `e2e_0096_005_tbz2_bomb_against_the_session_allowance`
- Given `fixture_allowance(1000)` and `/uploads/bomb.tbz2` = `bz2(&tar_bomb())` through `seed_bomb`
- When it is extracted (after `held_reset()`)
- Then `session_refusal(&v, 1000)`: `ERR_WRITE_QUOTA_EXCEEDED`, `session write quota of 1000 bytes exceeded`
- And `held_at_most(1_049_576)`, `/uploads/bomb` does not exist, and a write of 1000 bytes to `/uploads/small.bin` succeeds (copy the write call of `e2e_0084_009`)

### SPEC-0096/E2E-006: `.tbz` bomb against the storage limit
- Level / Driver / Location: functional e2e / tool function / `archive_tar_bound_e2e.rs`, function `e2e_0096_006_tbz_bomb_against_the_storage_limit`
- Given `fixture()`, `/uploads/bomb.tbz` = `bz2(&tar_bomb())` through `seed_bomb`, `limit_remaining(&f, 1000)`
- When it is extracted (after `held_reset()`)
- Then `project_refusal(&v)` (`ERR_PROJECT_QUOTA_EXCEEDED`)
- And `held_at_most(1_049_576)`, `/uploads/bomb` does not exist, `remaining(&f)` is still 1000

### SPEC-0096/E2E-007: junk named `.tbz2`
- Level / Driver / Location: functional e2e / tool function / `archive_tar_bound_e2e.rs`, function `e2e_0096_007_junk_named_tbz2`
- Given `/uploads/junk.tbz2` holding the 20 bytes `this is not bzip two`
- When it is extracted
- Then `corrupt_refusal(&v, "tar.bz2")`: `ERR_INVALID_ARGUMENT`, message starting `archive is corrupt or not a valid tar.bz2 file:`
- And `/uploads/junk` does not exist

### SPEC-0096/E2E-008: password on a `.tbz2`
- Level / Driver / Location: functional e2e / tool function / `archive_tar_bound_e2e.rs`, function `e2e_0096_008_password_on_tbz2`
- Given `/uploads/pack.tbz2`, the pack
- When it is extracted with `Some("secret")`
- Then `ERR_INVALID_ARGUMENT`, message `password is not applicable to this archive format`
- And `/uploads/pack` does not exist

### SPEC-0096/E2E-009: collision on a `.tbz2`
- Level / Driver / Location: functional e2e / tool function / `archive_tar_bound_e2e.rs`, function `e2e_0096_009_collision_on_tbz2`
- Given `/uploads/pack.tbz2`, the pack, and `/uploads/pack/a.txt` holding `old` (seeded)
- When extracted with `overwrite` false, then with `overwrite` true
- Then the first is `ERR_NO_CLOBBER` with message `'/uploads/pack/a.txt' already exists; pass overwrite=true to replace it`; the second succeeds with 1 file and 5 bytes
- And `a.txt` holds `old` after the first call and `hello` after the second

### SPEC-0096/E2E-010: close names are still refused
- Level / Driver / Location: functional e2e / tool function / `archive_tar_bound_e2e.rs`, function `e2e_0096_010_close_names_are_still_refused`
- Given the pack stored as `/uploads/pack.tbz3`, `/uploads/pack.tb` and `/uploads/pack.tbz2x`
- When each is extracted
- Then each is `ERR_NOT_SUPPORTED` (`crate::errors::code::NOT_SUPPORTED`) with messages `unsupported archive extension: '.tbz3'`, `unsupported archive extension: '.tb'`, `unsupported archive extension: '.tbz2x'`
- And `/uploads/pack` does not exist

### SPEC-0096/E2E-011: the other bzip2 names are unchanged
- Level / Driver / Location: functional e2e / tool function / `archive_tar_bound_e2e.rs`, function `e2e_0096_011_other_bzip2_names_unchanged`
- Given the pack as `/uploads/pack.tar.bz2` and `/u2/PACK.TB2`, and `/u3/note.txt.bz2` = `bz2(b"hello")`
- When each is extracted
- Then destinations `/uploads/pack` and `/u2/PACK` (1 file, 5 bytes each); the bare file gives `/u3/note.txt` holding `hello`
- And the answers equal what `e2e_0084_002` and `e2e_0083_*` already assert (no change)

### SPEC-0096/E2E-MOD-001: `.tbz2` extracts (was SPEC-0084/E2E-042)
- Level / Driver / Location: functional e2e / tool function / `archive_tar_bound_e2e.rs`, function `e2e_0096_mod_001_tbz2_extracts` (replaces `e2e_0084_042_tbz2_still_unsupported`, same file, same fixture lines)
- Given `/uploads/pack.tbz2` = `bz2(&tar())`
- When it is extracted
- Then `extracted_one_file(&v, "/uploads/pack")` holds (no refusal)
- And `assert_hello(&f)` holds
- Red before the fix because: the old table refuses with `unsupported archive extension: '.tbz2'`, so `extracted_one_file` fails on an `Err`.

Red evidence: E2E-001 to 004, 005, 006, 007, 008, 009 and MOD-001 fail before the table rows (refused as unsupported); E2E-010 and 011 are green on arrival and characterize rules already built (mutation check: add `(".tbz3", ...)` to the table, E2E-010 must fail).

## Constraints
- Files not to touch: `TOOL_CONTRACT.txt`, `tool-contract-golden.json`, any `sdlc/specs/SPEC-00*/` other than SPEC-0096, `AGENTS.md`.
- Dependencies not to add: none.
- Patterns to avoid: a name normalisation step; touching the decoder or the bound.
- Scope boundary: no other suffix (`.tlz`, `.tar.zst`, ...).

## Definition of done
- [ ] RED evidence recorded for every new test before the production code (E2E-010 and 011 by mutation).
- [ ] Every acceptance test above green; `./test.sh`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo fmt --all -- --check` clean on the work branch after this commit.
- [ ] Non regression: every `e2e_0083_`, `e2e_0084_` (except the one rewritten), `e2e_0086_` and `detect_format_*` test passes unmodified; `tool_contract` tests green with no golden diff.
- [ ] Docs updated: `README.md:294`, `:310`, `.agent_docs/tools.md:181`, `:283`.
- [ ] Self review: does the unit table in `archive.rs` hold both new rows with lengths 5 and 4; does E2E-004 compare the two answers apart from `destination`; no dash punctuation in comments or docs; no other file changed.
- [ ] Closes spec: the spec's full acceptance set green (E2E-001 to 011, E2E-MOD-001), and FR-NEW-001 to 003 and FR-MOD-001 cited by this story.
- [ ] One commit `US-0245: recognize .tbz2 and .tbz as bzip2 compressed tar archives` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0096` and the requirement ids.
