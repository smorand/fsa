# Recognize `.tbz2` and `.tbz` as bzip2 compressed tar archives: functional specification

> Id: SPEC-0096
> Nature: FEAT
> Depth: S
> Depth evidence: 1 observable behavior (which file names count as a bzip2 compressed tar), 1 module (archive extraction), about 4 requirements, no persisted change, no change to any answer shape.
> Status: implemented
> Generated on: 2026-11-15
> Project: existing
> From backlog: BL-0133
> Depends on: none
> Security: n/a
> CVSS: n/a
> Affected: n/a
> Fixed in: unreleased

## 1. Summary

A member who holds a bzip2 compressed tar archive named `.tbz2` or `.tbz`, two common short names for the same format, is refused today with `unsupported archive extension: '.tbz2'`, while the same archive named `.tar.bz2` or `.tb2` is extracted. This increment makes the extraction treat `.tbz2` and `.tbz` exactly like `.tar.bz2` and `.tb2`. The decoding bound and every refusal that protect the other compressed tar names apply to them unchanged.

## 2. Current State (MANDATORY, every depth)

### 2.1 What users can do today

- A member extracts `.zip`, `.7z`, `.tar`, `.tar.gz`/`.tgz`, `.tar.bz2`/`.tb2` and `.tar.xz`/`.txz` archives, and bare `.gz`, `.bz2` and `.xz` files, from the assistant tool and from the HTTP API. The format is read from the file name alone, ignoring letter case. [14.1 row 1]
- A file named `.tbz2` or `.tbz` is refused with `ERR_NOT_SUPPORTED` and the message `unsupported archive extension: '.tbz2'` (or `'.tbz'`). The message quotes the last extension as typed. [14.1 row 2]
- A compressed tar that is extracted lands by default in a folder named like the archive minus its suffix, is stopped when its decompressed size passes the lower of the project's remaining storage and the member's remaining session write allowance (plus 1 MiB of tar bookkeeping), and is refused when a password is given, when the data is corrupt, or when a destination file already exists without `overwrite`. [14.1 rows 3 and 4]

### 2.2 Specs governing this area

- SPEC-0084/FR-NEW-011 and its test E2E-042 state that `.tbz2` keeps being refused. This increment deviates from that one rule on purpose (FR-MOD-001). SPEC-0084 itself is not modified.
- SPEC-0018 and SPEC-0083 list the recognized suffixes; they are not modified, this spec adds two.

### 2.3 What is verified today

- Name detection, `.tb2` extraction within the limits and `.tbz2` refusal each have an automated test, as does every refusal listed in 2.1. Nothing exercises `.tbz` anywhere. [14.1 row 5]

## 3. Scope

### 3.1 In scope

- The names `.tbz2` and `.tbz`, in any letter case, extracted as bzip2 compressed tar archives from both doors (the assistant tool and the HTTP API).

### 3.2 Out of scope, and why

- Other short names (`.tlz`, `.tar.lz`, `.tar.zst`, `.tar.Z`): no request for them. No backlog entry is written.
- Any change to how bzip2 data is decoded (several bzip2 streams inside a tar stay refused as today): not asked.
- A file named only `.tbz2` with nothing before the dot: it behaves as a file named only `.tb2` does today, no new rule.

### 3.3 Constraints imposed by the context

- None.

## 5. Usage scenarios

### SC-001: A member extracts a `.tbz2` or `.tbz` archive
**Actor:** a project member.
**Preconditions:** the project holds `/uploads/pack.tbz2`, a bzip2 compressed tar holding one entry `a.txt` with the 5 bytes `hello`. Nothing exists at `/uploads/pack`.
**Flow:**
1. The member asks to extract `/uploads/pack.tbz2` → the archive is extracted to `/uploads/pack` and the answer reports 1 file written and 5 bytes written, the same answer a `.tar.bz2` of the same content gives.
**Postconditions:** `/uploads/pack/a.txt` holds `hello`.
**Exceptions:**
- EXC-001a: the member also gives a destination → the files land there instead of the default folder.
- EXC-001b: the file name is written in upper or mixed case (`PACK.TBZ2`, `Pack.Tbz`) → treated the same.
**Interactions:** SC-002 for refusals.

### SC-002: The refusals and the bound protect `.tbz2` and `.tbz` like any compressed tar
**Actor:** a project member, or a hostile one.
**Preconditions:** a `.tbz2` or `.tbz` file whose decompressed archive is far larger than the allowance, or holds junk, or collides with existing files.
**Flow:**
1. The member extracts it → extraction stops at the bound or the refusal fires, with the same code and message as for `.tar.bz2`.
**Postconditions:** nothing is written on any refusal.
**Exceptions:**
- EXC-002a: a password is given → refused.
- EXC-002b: a destination file exists and `overwrite` is not set → refused, naming it.
**Interactions:** SC-001.

### SC-003: Names close to the new ones are still refused
**Actor:** a project member.
**Preconditions:** a file named `/uploads/pack.tbz3`, `/uploads/pack.tb` or `/uploads/pack.tbz2x`.
**Flow:**
1. The member extracts it → refused with `unsupported archive extension: '<extension as typed>'`.
**Postconditions:** nothing is written.
**Exceptions:** none.
**Interactions:** none.

## 6. Functional requirements

### 6.1 New

#### FR-NEW-001 [EARS-E]: `.tbz2` and `.tbz` are bzip2 compressed tar archives
> WHEN a member extracts a file whose name ends in `.tbz2` or `.tbz`, in any letter case, THE archive extraction SHALL extract it as a bzip2 compressed tar archive, exactly as it extracts a file ending in `.tar.bz2` or `.tb2`.

- **Business rules:** the default destination is the archive path minus its suffix (`/uploads/pack.tbz2` gives `/uploads/pack`, `/u2/PACK.TBZ2` gives `/u2/PACK`); a given destination replaces it; the answer has the same fields and counts as for `.tar.bz2`. Both doors behave the same.
- **User visible text:** none new.
- **Scenario:** SC-001
- **Priority:** Must

#### FR-NEW-002 [EARS-U]: Same bound and same refusals
> THE archive extraction SHALL apply to a `.tbz2` or `.tbz` file, in any letter case, the decoding bound and every refusal, with the same codes, messages and order, that apply to a `.tar.bz2` file.

- **Business rules:** the bound is the lower of the project's remaining storage (plus the bytes already stored under the destination) and the session write allowance, plus 1 048 576 bytes. A corrupt file is refused with a message starting `archive is corrupt or not a valid tar.bz2 file:`. A password is refused with `password is not applicable to this archive format`. A collision is refused with `'<path>' already exists; pass overwrite=true to replace it`.
- **User visible text:** `session write quota of 1000 bytes exceeded` (the session limit used in the tests), the three messages above.
- **Scenario:** SC-002
- **Priority:** Must

#### FR-NEW-003 [EARS-UB]: Nothing else moves
> THE archive extraction SHALL NOT change how it extracts or refuses any file whose name does not end in `.tbz2` or `.tbz`.

- **Business rules:** `.tar.bz2`, `.tb2`, bare `.bz2` and every other format and suffix keep today's behavior. A name that merely resembles the new ones (`.tbz3`, `.tb`, `.tbz2x`) is refused with `unsupported archive extension: '<extension as typed>'`.
- **User visible text:** `unsupported archive extension: '.tbz3'`, `'.tb'`, `'.tbz2x'` for the three cases above.
- **Scenario:** SC-003
- **Priority:** Must

### 6.2 Modified

#### FR-MOD-001 [EARS-E]: `.tbz2` is no longer refused (modifies SPEC-0084/FR-NEW-011)
> WHEN a member extracts a file ending in `.tbz2`, THE archive extraction SHALL extract it as FR-NEW-001 states instead of refusing it with `unsupported archive extension: '.tbz2'`.

- **Before:** the file is refused with `ERR_NOT_SUPPORTED` and `unsupported archive extension: '.tbz2'` (SPEC-0084 E2E-042).
- **After:** the statement above; `.tbz` follows the same rule.
- **Reason:** both are common names of the same format; BL-0133.

### 6.3 Removed

None.

## 9. Acceptance tests (the contract)

> Behavioral and technology agnostic. A bzip2 compressed tar holding `a.txt` (`hello`) is called "the pack" below.

### 9.1 Summary

| Test | Action | Category | Scenario | Requirements | Priority |
|---|---|---|---|---|---|
| E2E-001 | New | happy | SC-001 | FR-NEW-001 | Critical |
| E2E-002 | New | happy, edge | SC-001 | FR-NEW-001 | High |
| E2E-003 | New | happy | SC-001 | FR-NEW-001 | Medium |
| E2E-004 | New | happy | SC-001 | FR-NEW-001, FR-NEW-002 | High |
| E2E-005 | New | failure, security | SC-002 | FR-NEW-002 | Critical |
| E2E-006 | New | failure, security | SC-002 | FR-NEW-002 | Critical |
| E2E-007 | New | failure | SC-002 | FR-NEW-002 | High |
| E2E-008 | New | failure | SC-002 | FR-NEW-002 | High |
| E2E-009 | New | failure, side effect | SC-002 | FR-NEW-002 | High |
| E2E-010 | New | failure, edge | SC-003 | FR-NEW-003 | High |
| E2E-011 | New | happy, regression | SC-001 | FR-NEW-003 | High |
| E2E-MOD-001 | Modified | state transition | SC-001 | FR-MOD-001 | Critical |

Coverage: happy 5 (001, 002, 003, 004, 011), failure 6 (005 to 010), side effect 1 (009), edge 2 (002, 010), state transition 1 (MOD-001), security 2 (005, 006).
Happy to failure ratio 1:1.2 (beats 1:1).

### 9.2 New tests

#### E2E-001: `.tbz2` extracts
- **Category:** journey
- **Scenario:** SC-001
- **Requirements:** FR-NEW-001
- **Given** a project member and `/uploads/pack.tbz2`, the pack; nothing at `/uploads/pack`
- **When** the member extracts `/uploads/pack.tbz2`
- **Then** the answer gives the destination `/uploads/pack`, 1 file written, 5 bytes written
- **And** `/uploads/pack/a.txt` holds `hello`
- **Priority:** Critical

#### E2E-002: `.tbz` and letter case
- **Category:** edge
- **Scenario:** SC-001
- **Requirements:** FR-NEW-001
- **Given** the pack stored as `/uploads/pack.tbz`, `/u2/PACK.TBZ2` and `/u3/Pack.Tbz`
- **When** the member extracts each in turn
- **Then** the destinations are `/uploads/pack`, `/u2/PACK` and `/u3/Pack`, each with 1 file and 5 bytes written
- **And** each `a.txt` holds `hello`
- **Priority:** High

#### E2E-003: explicit destination
- **Category:** requirement
- **Scenario:** SC-001
- **Requirements:** FR-NEW-001
- **Given** `/uploads/pack.tbz2`, the pack
- **When** the member extracts it with the destination `/out/here`
- **Then** the destination is `/out/here` with 1 file and 5 bytes written
- **And** `/out/here/a.txt` holds `hello` and `/uploads/pack` does not exist
- **Priority:** Medium

#### E2E-004: the HTTP API gives the same answer
- **Category:** requirement
- **Scenario:** SC-001
- **Requirements:** FR-NEW-001, FR-NEW-002
- **Given** `/uploads/pack.tbz2`, the pack, and `/uploads/other.tar.bz2`, the same pack
- **When** the member extracts `/uploads/pack.tbz2` through the HTTP API, then extracts `/uploads/other.tar.bz2` through the assistant tool
- **Then** the HTTP answer for the `.tbz2` has the same fields and counts as the assistant tool answer for the `.tar.bz2`, with destinations `/uploads/pack` and `/uploads/other`
- **And** `/uploads/pack/a.txt` holds `hello`
- **Priority:** High

#### E2E-005: `.tbz2` bomb against the session allowance
- **Category:** failure
- **Scenario:** SC-002
- **Requirements:** FR-NEW-002
- **Given** a project without a storage limit, a session write allowance of 1 000 bytes, and `/uploads/bomb.tbz2` holding `zeros.bin` of 100 000 000 zero bytes (a few hundred bytes compressed)
- **When** the member extracts it
- **Then** the refusal is `ERR_WRITE_QUOTA_EXCEEDED` with the message `session write quota of 1000 bytes exceeded`
- **And** the decompressed bytes held never exceed 1 049 576, `/uploads/bomb` does not exist, and a following write of a 1 000 byte file `/uploads/small.bin` succeeds
- **Priority:** Critical

#### E2E-006: `.tbz` bomb against the storage limit
- **Category:** failure
- **Scenario:** SC-002
- **Requirements:** FR-NEW-002
- **Given** 1 000 bytes of remaining storage and `/uploads/bomb.tbz` holding `zeros.bin` as in E2E-005
- **When** the member extracts it
- **Then** the refusal is `ERR_PROJECT_QUOTA_EXCEEDED`
- **And** the decompressed bytes held never exceed 1 049 576, `/uploads/bomb` does not exist, and the remaining storage is still 1 000 bytes
- **Priority:** Critical

#### E2E-007: junk named `.tbz2`
- **Category:** failure
- **Scenario:** SC-002
- **Requirements:** FR-NEW-002
- **Given** `/uploads/junk.tbz2` holding the 20 bytes `this is not bzip two`
- **When** the member extracts it
- **Then** the refusal is `ERR_INVALID_ARGUMENT` with a message starting `archive is corrupt or not a valid tar.bz2 file:`
- **And** `/uploads/junk` does not exist
- **Priority:** High

#### E2E-008: password on a `.tbz2`
- **Category:** failure
- **Scenario:** SC-002
- **Requirements:** FR-NEW-002
- **Given** `/uploads/pack.tbz2`, the pack
- **When** the member extracts it with the password `secret`
- **Then** the refusal is `ERR_INVALID_ARGUMENT` with the message `password is not applicable to this archive format`
- **And** `/uploads/pack` does not exist
- **Priority:** High

#### E2E-009: collision on a `.tbz2`
- **Category:** side effect
- **Scenario:** SC-002
- **Requirements:** FR-NEW-002
- **Given** `/uploads/pack.tbz2`, the pack, and an existing `/uploads/pack/a.txt` holding `old`
- **When** the member extracts it without `overwrite`, then again with `overwrite`
- **Then** the first call is refused with `ERR_NO_CLOBBER` and the message `'/uploads/pack/a.txt' already exists; pass overwrite=true to replace it`; the second succeeds with 1 file and 5 bytes written
- **And** after the first call `/uploads/pack/a.txt` still holds `old`; after the second it holds `hello`
- **Priority:** High

#### E2E-010: close names are still refused
- **Category:** edge
- **Scenario:** SC-003
- **Requirements:** FR-NEW-003
- **Given** the pack stored as `/uploads/pack.tbz3`, `/uploads/pack.tb` and `/uploads/pack.tbz2x`
- **When** the member extracts each in turn
- **Then** each is refused with `ERR_NOT_SUPPORTED` and the messages `unsupported archive extension: '.tbz3'`, `unsupported archive extension: '.tb'` and `unsupported archive extension: '.tbz2x'`
- **And** `/uploads/pack` does not exist
- **Priority:** High

#### E2E-011: the other bzip2 names are unchanged
- **Category:** regression
- **Scenario:** SC-001
- **Requirements:** FR-NEW-003
- **Given** the pack stored as `/uploads/pack.tar.bz2` and as `/u2/PACK.TB2`, and a bare bzip2 file `/u3/note.txt.bz2` holding `hello`
- **When** the member extracts each in turn
- **Then** the first two give destinations `/uploads/pack` and `/u2/PACK`, 1 file and 5 bytes written each; the third gives `/u3/note.txt` holding `hello`
- **And** none of the three answers differs from what it was before this increment
- **Priority:** High

### 9.4 Modified tests

#### E2E-MOD-001: `.tbz2` extracts (was SPEC-0084 E2E-042)
- **Validated before:** `.tbz2` is refused with `unsupported archive extension: '.tbz2'`.
- **Validates now:** `.tbz2` extracts.
- **Given** `/uploads/pack.tbz2`, the pack
- **When** the member extracts it
- **Then** the destination is `/uploads/pack` with 1 file and 5 bytes written, and no refusal is raised
- **And** the refusal the old test pinned is now covered by E2E-010 on `.tbz3`

### 9.5 Removed tests

None.

## 10. Traceability

| Scenario | Requirements | Happy | Failure | Edge |
|---|---|---|---|---|
| SC-001 | FR-NEW-001, FR-MOD-001 | E2E-001, E2E-003, E2E-004, E2E-011, E2E-MOD-001 | E2E-007 (junk, reached by the same journey) | E2E-002 |
| SC-002 | FR-NEW-002 | E2E-004 | E2E-005, E2E-006, E2E-007, E2E-008, E2E-009 | E2E-009 |
| SC-003 | FR-NEW-003 | E2E-011 | E2E-010 | E2E-010 |

| Requirement | Tests |
|---|---|
| FR-NEW-001 | E2E-001, E2E-002, E2E-003, E2E-004, E2E-MOD-001 |
| FR-NEW-002 | E2E-004, E2E-005, E2E-006, E2E-007, E2E-008, E2E-009 |
| FR-NEW-003 | E2E-010, E2E-011 |
| FR-MOD-001 | E2E-MOD-001, E2E-010 |

## 11. Open questions

None.

## 12. Glossary (MANDATORY)

| Term | Definition | Context |
|---|---|---|
| The pack | a bzip2 compressed tar archive holding `a.txt` with the 5 bytes `hello` | Section 9 |
| Bzip2 compressed tar | a tar archive inside one bzip2 compression, named `.tar.bz2`, `.tb2`, and now `.tbz2` or `.tbz` | all |
| Decoding bound | the lower of the project's remaining storage (plus bytes already stored under the destination) and the session write allowance, plus 1 048 576 bytes, past which decompression stops | FR-NEW-002 |
| Session write allowance | the number of bytes a member may still write in the current session | FR-NEW-002 |
| HTTP API | the web interface that offers the same extraction as the assistant tool | E2E-004 |

## 13. Implementability gate (MANDATORY)

| Round | F | A | Verdict |
|---|---|---|---|
| 1 | 0 | 0 | IMPLEMENTABLE |

Inline checklist (depth S), read back from disk: forced choices closed (defaults, destination, messages, limits spelled); no implementation detail outside Section 14 (file suffixes are the user typed names; `HTTP API` is a product door named by SPEC-0018 and SPEC-0084); every test concrete with exact Then; every requirement has 3 or more tests; failure tests outnumber happy ones; EARS clean, no forbidden modal, no nested condition; Security n/a (no flaw, no advisory; the bound is existing).

**Amendments applied:** none.
**No implementation detail outside Section 14:** PASS

## 14. Evidence appendix (for /sdlc-design and the auditor, not for the product owner)

### 14.1 Current State evidence

| # | Claim (Section 2) | Evidence |
|---|---|---|
| 1 | suffix table, case insensitive name detection | `crates/core/src/tools/archive.rs:48-69` (table), `:73-84` (detection) |
| 2 | `.tbz2`/`.tbz` absent from the table; refusal quotes the last extension | `crates/core/src/tools/archive.rs:48-69` (no row), `:80-83` |
| 3 | destination is the path minus the matched suffix length | `crates/core/src/tools/archive.rs:73-79` (returns `suffix.len()`), `:173`, `:210` (default destination) |
| 4 | password refused for a tar before any byte is read; collision message | `crates/core/src/tools/archive.rs:174-178` (password), `:243` (collision); bzip2 tar decoder `:459` |
| 5 | tests: detection table 1 file; `.tb2` extraction `archive_tar_bound_e2e.rs:555`, `archive_single.rs:1466`; `.tbz2` pin `archive_tar_bound_e2e.rs:612-619`; no `.tbz` hit by `rg 'tbz'` outside `sdlc/` | `crates/core/src/tools/archive.rs:1360-1375` |

Test command: `cargo test -p mcp-fs-core --lib -- archive` (the full gate stays `./test.sh`).

### 14.3 Design inputs

- The only code change expected is two rows in the suffix table (`archive.rs:48-69`). Row order does not matter for the new rows: `x.tbz2` and `x.tbz` end in neither `.bz2` (no dot before `bz2`) nor in each other. Signal if wrong: the detection unit table (`archive.rs:1360`) and E2E-001 fail.
- `e2e_0084_042_tbz2_still_unsupported` (`archive_tar_bound_e2e.rs:612`) must be rewritten (E2E-MOD-001); E2E-010 keeps a refusal test for names that stay unsupported.
- Docs to update at implement time: `README.md:294` and `:310`, `.agent_docs/` archive page if it lists suffixes, the `<format>` label in a corruption message is `tar.bz2` for these names (derived from the bzip2 tar kind, no new text).
- The HTTP API door calls the same detection as the assistant tool (`api/dataplane.rs:176`, per SPEC-0084 "both doors"); E2E-004 pins parity.
