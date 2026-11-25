# Tool reference (146 tools)

Facts below come from `TOOL_CONTRACT.txt` (captured from the running reference
server) and the `tools/` modules. Parameters are listed as
`name:type=default`; no `=` means required. `mount_id` is required on every
`fs.*` and `git.*` tool.

Authorization column: **member** = project membership (`AppState::authorize`),
**admin** = platform admin, **owner/admin** = project owner or platform admin,
**auth** = any verified identity, no project involved. Details in
[Authorization model](#authorization-model).

Annotations column: the `tools/list` behavior hints (`ToolAnnotations`), one
abbreviation per hint that is actually set on the tool, `-` when none are set.
`D`/`!D` = destructiveHint true/false, `RO`/`!RO` = readOnlyHint,
`I`/`!I` = idempotentHint, `OW`/`!OW` = openWorldHint. A hint absent from a
tool's schema is omitted here too, never guessed.

## fs read (8)

| Tool | Purpose | Parameters | Returns | Annotations | Auth |
|---|---|---|---|---|---|
| `fs.read` | line numbered, paged window over a text file | `path`, `offset_lines:int=0`, `limit_lines:int=2000`, `line_numbered:bool=true` | `content`, `total_lines`, `truncated`, `next_offset` | RO,I,!OW | member |
| `fs.read_bytes` | raw bytes, base64, with MIME type | `path`, `offset_bytes:int=0`, `length_bytes:int=65536` | `base64`, `mime_type`, `length` | RO,I,!OW | member |
| `fs.read_lines` | inclusive line range | `path`, `start_line:int`, `end_line:int` | `content`, `total_lines` | RO,I,!OW | member |
| `fs.read_section` | indentation block around an anchor line | `path`, `anchor_line:int`, `max_lines:int=200` | `content`, `start_line`, `end_line` | RO,I,!OW | member |
| `fs.read_many` | batch read, per file error isolation | `paths:array`, `per_file_cap_lines:int=500` | `files[]` of `{path, content, truncated}` or `{path, error}` | RO,I,!OW | member |
| `fs.head` | first N lines | `path`, `lines:int=20` | `content` | RO,I,!OW | member |
| `fs.tail` | last N lines | `path`, `lines:int=20` | `content` | RO,I,!OW | member |
| `fs.count_lines` | line count without content | `path` | `total_lines` | RO,I,!OW | member |

## fs write (4)

| Tool | Purpose | Parameters | Returns | Annotations | Auth |
|---|---|---|---|---|---|
| `fs.write` | create or overwrite, atomic | `path`, `content`, `overwrite:bool=false`, `create_parents:bool=true` | `path`, `bytes_written`, `overwritten`, `diff` | D,!RO,!I,!OW | member |
| `fs.append` | append, optionally create | `path`, `content`, `create:bool=false` | `path`, `bytes_appended` | !D,!RO,!I,!OW | member |
| `fs.create_empty` | touch | `path`, `exist_ok:bool=false` | `path`, `created` | !D,!RO,I,!OW | member |
| `fs.write_bytes` | write raw bytes, base64 | `path`, `base64`, `overwrite:bool=false`, `create_parents:bool=true`, `trigger_documentation_service:bool=false` | `path`, `bytes_written`, `overwritten`, `documentation` | D,!RO,!I,!OW | member |

`fs.write` on an existing file needs a prior read in the session and returns the
unified diff of the change. Overwriting without `overwrite=true` is
`ERR_NO_CLOBBER`.

`fs.write_bytes` is the only way to put binary content in a volume through MCP;
`base64` is the standard alphabet with padding, the one `fs.read_bytes` returns,
and anything else is `ERR_INVALID_ARGUMENT`. With
`trigger_documentation_service` the file is also converted to Markdown by the
configured `doc_service` and the companion stored beside it (`deck.pptx` ->
`deck.md`): `documentation` is then `{md_path, bytes_written}`, or
`{error: {code, message}}` when the conversion failed, and `null` when the flag
was off. Eligibility (PowerPoint, Word, PDF, audio, video) and the size cap are
checked **before anything is written**, so a flag on a `.txt` is
`ERR_NOT_SUPPORTED` with no file stored; a conversion that fails afterwards never
rolls the upload back.

## fs edit (5)

| Tool | Purpose | Parameters | Returns | Annotations | Auth |
|---|---|---|---|---|---|
| `fs.edit` | replace a unique string | `path`, `old_string`, `new_string`, `replace_all:bool=false`, `dry_run:bool=false` | `path`, `applied`, `diff` | D,!RO,!I,!OW | member |
| `fs.multi_edit` | several edits, all or nothing | `path`, `edits:array`, `dry_run:bool=false` | `path`, `applied`, `edits`, `diff` | D,!RO,!I,!OW | member |
| `fs.search_replace` | replace a multi line block | `path`, `search_block`, `replace_block`, `fuzzy:bool=false` | `path`, `applied`, `diff` | D,!RO,!I,!OW | member |
| `fs.insert_at_line` | insert before a 1 based line | `path`, `line:int`, `content` | `path`, `applied`, `line` | D,!RO,!I,!OW | member |
| `fs.apply_patch` | multi file V4A patch in one volume | `patch_text` | `files[]` of `{path, op, moved_to?}` (`op`: add / update / delete) | D,!RO,!I,!OW | member |

Every tool here enforces the read guard (`fs.apply_patch` on its update and
delete operations, not on an add, which creates a new file). A non unique
`old_string` is `ERR_AMBIGUOUS_MATCH`, an absent one `ERR_NO_MATCH`. `edits[]` items are
`{old_string, new_string, replace_all?}`. `dry_run` returns the diff and writes
nothing.

## fs search (4)

| Tool | Purpose | Parameters | Returns | Annotations | Auth |
|---|---|---|---|---|---|
| `fs.glob` | files by glob, newest first, cap 100 | `pattern`, `root:string="/"`, `exclude_patterns=null` | `matches`, `truncated` | RO,I,!OW | member |
| `fs.grep` | content search | `pattern`, `root="/"`, `include_glob=null`, `exclude_glob=null`, `regex:bool=true`, `case_sensitive:bool=true`, `output_mode="content"`, `context_lines:int=0`, `max_matches:int=100` | `content`: `matches[{path,line,text,context}]` + `truncated`; `files`: `files[]`; `count`: `count`, `files` | RO,I,!OW | member |
| `fs.find_definition` | symbol definition, language aware | `name`, `root="/"`, `kind=null` | `definitions[{path,name,kind,line}]` | RO,I,!OW | member |
| `fs.find_references` | identifier references | `name`, `root="/"` | `references[{path,line,kind}]` | RO,I,!OW | member |

Walks prune `.git`, `node_modules`, `target`, `dist`, `.build`, `coverage`,
`.mcp_trash` and stop at 5000 files. Symbol lookup uses tree-sitter where a
grammar is bundled and falls back to a lexical index otherwise.

## fs listing (2)

| Tool | Purpose | Parameters | Returns | Annotations | Auth |
|---|---|---|---|---|---|
| `fs.list_dir` | flat listing | `path="/"`, `include_hidden:bool=false`, `sort_by="name"` (or `size`), `with_sizes:bool=false` | `path`, `entries[{name,kind,size?}]`, `total` | RO,I,!OW | member |
| `fs.tree` | recursive JSON tree | `path="/"`, `max_depth:int=3`, `exclude_patterns=null`, `with_sizes:bool=false` | `path`, `tree[]`, `truncated` | RO,I,!OW | member |

`fs.tree` stops at 2000 nodes. Its exclude set does not hide the trash
directory, matching the reference.

## fs compare (1)

| Tool | Purpose | Parameters | Returns | Annotations | Auth |
|---|---|---|---|---|---|
| `fs.diff_files` | unified diff of two text files, possibly in two projects | `mount_id_a`, `path_a`, `mount_id_b`, `path_b`, `context_lines` (optional) | `identical`, `diff` | RO,I,!OW | member of both projects |

Side A is the old side, B the new one; the diff has 3 lines of context by default and its
two header lines are `--- <mount_id_a>:<path_a>` and `+++ <mount_id_b>:<path_b>`.
`context_lines` (2026-10-20, SPEC-0046) is optional, default 3: a whole number from 0 to
10000000, given as a number or as digit text (leading zeros allowed); absent or null means 3.
Anything else (negative, fractional, `3.0`, `1e3`, a boolean, a list, empty or padded text) is
refused with `ERR_INVALID_ARGUMENT: 'context_lines' must be a whole number from 0 to 10000000`, and the value is checked before
anything else (before paths, access or files). `fs.write`, `fs.edit` and `fs.multi_edit`
keep 3 lines of context. Identical texts answer `{"identical": true, "diff": ""}`. Both paths are
normalized, then both projects authorized (a non member learns nothing about the
other project's files), then A is read completely before B. Each file must be
text (valid UTF-8, no NUL byte) and at most `safety.max_diff_file_bytes` (5 MiB
by default). The tool never writes: it bumps the access time of both files on
success only and does not count as a read for read before edit. The diff runs in
`spawn_blocking`; two 5 MiB files differing on 1000 lines answer in well under
the 10 second bound (E2E-230). Errors, all `ERR_INVALID_ARGUMENT` unless noted:
`'<mount>:<path>' is a directory`, `'<mount>:<path>' is not a text file`,
`'<mount>:<path>' is <n> bytes, over the diff limit of <limit> bytes`;
`ERR_NOT_FOUND` `'<mount>:<path>' not found`; `ERR_FORBIDDEN` for a project the
caller is not a member of. REST twin: `GET /api/fs/diff`
(2026-10-20, SPEC-0045).

## fs metadata (3)

| Tool | Purpose | Parameters | Returns | Annotations | Auth |
|---|---|---|---|---|---|
| `fs.stat` | POSIX metadata | `path` | `path`, `size`, `mode` (e.g. `"0o644"`), `kind`, `mtime`, `ctime`, `atime`, `uid`, `gid` | RO,I,!OW | member |
| `fs.exists` | probe path and kind | `path` | `exists`, `kind` (null when absent) | RO,I,!OW | member |
| `fs.hash` | content hash | `path`, `algo="sha256"` (md5, sha1, sha256, sha512) | `path`, `algo`, `hash`, `size` | RO,I,!OW | member |

`uid`/`gid` are the synthetic constants 1000/1000: a volume has no real POSIX
owner.

## fs lifecycle (10)

| Tool | Purpose | Parameters | Returns | Annotations | Auth |
|---|---|---|---|---|---|
| `fs.mkdir` | create a directory | `path`, `parents:bool=true`, `exist_ok:bool=true` | `path`, `created` | !D,!RO,I,!OW | member |
| `fs.delete` | delete, trash by default | `path`, `recursive:bool=false`, `trash:bool=true` | `path`, `trashed`, `trash_path` | D,!RO,I,!OW | member |
| `fs.move` | rename or relocate | `source`, `destination`, `overwrite:bool=false` | `source`, `destination` | D,!RO,!I,!OW | member |
| `fs.copy` | copy a file or tree | `source`, `destination`, `overwrite:bool=false`, `recursive:bool=false` | `source`, `destination` | D,!RO,!I,!OW | member |
| `fs.list_allowed_roots` | volumes the caller can reach | (`mount_id` only) | `person`, `roots[{mount_id, root, owner}]` | RO,I,!OW | member |
| `fs.audit_log` | mutations recorded this session | `since:number=null`, `limit:int=20` | `entries[{timestamp, op, path, detail}]` | RO,I,!OW | member |
| `fs.trash_list` | list trashed files, paginated | `path_prefix:string=""`, `limit:int=200`, `offset:int=0` | `entries[{trash_path, original_path, size, kind, deleted_at, deleted_by, purge_in_days}]`, `total` | RO,I,!OW | member |
| `fs.trash_restore` | restore a trashed entry to its original path | `trash_path` | `restored_path`, `trash_path` | !D,!RO,!I,!OW | member |
| `fs.list_derived_jobs` | rebuild jobs of derived data (SPEC-0100), 100 per page | `path:string=""`, `marker:string=""` | `jobs[{job_id, path, kind, state, cause, created_at, seconds_left?, ended_at?, reason?}]`, `marker?` | RO,I,!OW | member |
| `fs.recalculate` | rebuild the search entries (`search_index`, SPEC-0100) or one chunk set (`chunks`, SPEC-0101) of one file now; a waiting rebuild of the kind is replaced, a running one ends `superseded` | `path`, `kind` (`search_index` or `chunks`), `set_id:string=""` (kind `chunks` only) | the job: `job_id, path, kind, state, cause, created_at, set_id? (chunks), seconds_left?, ended_at?, reason?` | !D,!RO,!I,!OW | member |

`trash=false` needs `safety.allow_hard_delete` on the server, otherwise
`ERR_NOT_SUPPORTED`. `fs.list_allowed_roots` and `fs.audit_log` still take and
check `mount_id` even though they never open the volume.

`fs.trash_list` backfills any trash-directory node not yet tracked in
`trash_entries` before paging, so it also covers files trashed outside the MCP
surface. `purge_in_days` is `null` when the project has no file retention
configured. `fs.trash_restore` renames to `_restoredN` on a collision at the
original path rather than failing (SPEC-0011).

`fs.recalculate` (2026-11-21, SPEC-0100 FR-NEW-008, FR-NEW-009) is MCP only, like
`fs.list_derived_jobs` (DEC-012). Refusals, first match wins, nothing changed:
project absent or deleted `ERR_PROJECT_NOT_FOUND`; non member `ERR_FORBIDDEN`;
kind other than `search_index` or `chunks` `ERR_INVALID_ARGUMENT: kind must be
'search_index' or 'chunks'`; `ERR_NOT_FOUND: file not found: <path>`; a folder (the root
included) `ERR_INVALID_ARGUMENT: path must be a file`; then for `search_index`: `set_id` given
`ERR_INVALID_ARGUMENT: set_id applies to kind 'chunks' only`, search off `ERR_NOT_SUPPORTED:
search is not enabled; set search.enabled: true in config`, index mode `none`
`ERR_NOT_SUPPORTED: search indexing is not active for project <mount_id>`; for `chunks`:
set unknown (or of another file) `ERR_NOT_FOUND: chunk set not found: <set_id>`, no default set
(empty `set_id`) `ERR_NOT_FOUND: the file has no default chunk set`, no text
`ERR_NOT_SUPPORTED: no text to cut for <path>`. (SPEC-0101 FR-MOD-003, FR-NEW-019: before it,
search off came right after the member check; `chunks` works with search off and in a
project that does not index.) Each
call creates a job (cause `manual`) and drops the file's waiting job without a
record. Nothing running: it starts in the call, answered `running`. A rebuild
running: that one ends `superseded` and the answer is `waiting` with
`seconds_left` 0, started when the running one ends. A `chunks` job is one per file and kind
whatever the set, never touches the waiting `search_index` job, and answers `set_id`. With
search off no scheduler exists: the cut runs on a task of its own and the job is not
recorded, so `fs.list_derived_jobs` (empty with search off) never shows it.

## fs chunks (5)

| Tool | Purpose | Params | Returns | Annotations | Who |
|---|---|---|---|---|---|
| `fs.list_chunk_sets` | chunk sets of one file (SPEC-0101); default set first, then `built_at` oldest first, then `set_id` | `path` | `sets[{set_id, strategy, settings, default, fallback_from?, chunk_count, token_count, state, built_at}]` | RO,I,!OW | member |
| `fs.list_chunks` | chunks of one set by number, 100 per page, preview of the first 200 characters | `path`, `set_id`, `marker:string=""` | `chunks[{number, unit, heading_path, page_start, page_end, token_count, figures, document_title, preview}]`, `marker?` | RO,I,!OW | member |
| `fs.get_chunk` | chunk `number` in full with `before` and `after` neighbours (0 to 20 each) | `path`, `set_id`, `number:int`, `before:int=0`, `after:int=0` | `chunks[{number, unit, heading_path, page_start, page_end, token_count, figures, document_title, text}]` | RO,I,!OW | member |
| `fs.create_chunk_set` | the set of this strategy and settings, built from the file text when absent, else the equal set (SPEC-0101 FR-NEW-013, FR-NEW-029) | `path`, `strategy` (`fixed` or `structural`), `settings:object={}` | `Set` as in `fs.list_chunk_sets` | !D,!RO,I,!OW | member |
| `fs.delete_chunk_set` | removes an extra set and its chunks; the default set is refused | `path`, `set_id` | `{deleted: true}` | D,!RO,!I,!OW | member |

(2026-11-22, SPEC-0101 US-0276) `fs.create_chunk_set` checks, in order: strategy, each setting in
request order (unknown, does not apply, not a whole number, out of range), the overlap against the
size, then the text (UTF-8 bytes, else the document view of a current conversion, else
`ERR_NOT_SUPPORTED: no text to cut for <path>`), then at most 5 sets per file inside the insert
transaction (`ChunkStore::insert_set`). The equal request is found on the primary key
`(volume_id, path, request_key)`, so two members racing get one set. `fixed` cuts in `cl100k_base`
tokens (`chunking::tokens`, `tiktoken-rs`) on the blocking pool. `structural` is validated, then
answers `ERR_INTERNAL_ERROR: strategy structural is not implemented` until US-0277.

(2026-11-22, SPEC-0101 US-0275) MCP only (DEC-015); engines in `tools/chunks.rs`, rows in the
volume meta tables `chunk_sets` and `chunks` through `chunking::ChunkStore`
(`MetaBackend::chunk_store()`). `state` is computed: `out_of_date` when the set's
`source_sha` differs from the file's `sha256`. A set is reached only through its own file.
Refusals, first match wins, nothing changed: `ERR_PROJECT_NOT_FOUND`; `ERR_FORBIDDEN:
'<person>' is not a member of '<mount_id>'`; `ERR_NOT_FOUND: file not found: <path>`;
a folder (the root included) `ERR_INVALID_ARGUMENT: path must be a file`; `ERR_NOT_FOUND:
chunk set not found: <set_id>`; `ERR_NOT_FOUND: chunk <number> not found in set <set_id>`;
`ERR_INVALID_ARGUMENT: before and after must be between 0 and 20`; `ERR_INVALID_ARGUMENT:
marker is not valid` (the marker is base64 url safe of `["<set_id>", <last number>]`, valid
for its own set only). No tool builds sets yet, so a file lists `{sets: []}` until later stories.

## fs export (1)

| Tool | Purpose | Params | Returns | Annotations | Who |
|---|---|---|---|---|---|
| `fs.export_zip` | zip a selection, return a single use download URL | `paths:string[]` | `url` | !D,!RO,!I,!OW | member |

All or nothing (2026-10-08, SPEC-0012 US-0002): an empty `paths` or an empty
entry is `ERR_INVALID_ARGUMENT`, any `..` climbing above the root is
`ERR_PATH_OUT_OF_BOUNDS` (checked on the raw path, since `normalize_path`
clamps `/../x` to `/x`), a missing entry is `ERR_NOT_FOUND` naming it; nothing
is stored on any error. Directories are walked recursively, each zip entry is
the full project relative path, overlapping entries are deduplicated. The zip
is buffered in memory (no size limit, like `download-zip`), stored at blob key
`export:{token}` with an `export_links` row expiring 300 s later. `url` is
`{server.public_base_url}/exports/{token}`, relative when that key is empty.
The full token is never logged, only its first 8 characters.

## fs archive (1)

| Tool | Purpose | Params | Returns | Annotations | Who |
|---|---|---|---|---|---|
| `fs.extract_archive` | extract a stored archive in place | `path`, `destination?`, `overwrite=false`, `password?` | `destination,files_written,dirs_created,bytes_written` | D,!RO,!I,!OW | member |

(2026-10-09, SPEC-0015.) Formats by extension, case insensitive: `.zip`, `.7z`,
`.tar`, `.tar.gz`/`.tgz`, `.tar.bz2`/`.tb2`/`.tbz2`/`.tbz`, `.tar.xz`/`.txz`, a bare `.gz`, `.bz2` or `.xz`
(see below), and `.rar` with the cargo feature `rar` (see below); anything else (including `.rar`
on a build without the feature) is `ERR_NOT_SUPPORTED`. Default destination: the archive
path minus its suffix. Stateless password retry: an encrypted zip or 7z with no
or a wrong `password` is `ERR_PASSWORD_REQUIRED` (HTTP 428), the caller just
retries with it; a password on a tar family archive is `ERR_INVALID_ARGUMENT`.
All or nothing: one pre scan rejects the whole
call, before any write, on a symlink/hardlink/device entry (`ERR_NOT_SUPPORTED`),
an entry escaping the destination (`ERR_PATH_OUT_OF_BOUNDS`), a collision with
`overwrite=false` (`ERR_NO_CLOBBER`, first collision in archive order), a
decoded size above the declared one (`ERR_INVALID_ARGUMENT`), or a declared size
over a limit (`ERR_PROJECT_QUOTA_EXCEEDED`, `ERR_WRITE_QUOTA_EXCEEDED`, charged once, counter
untouched on failure; for zip, 7z and RAR the limits are checked before decoding, see SPEC-0085). The password is never logged. REST twin: `POST
/api/fs/{mount_id}/extract-archive`.

(2026-11-08, SPEC-0080.) `.rar` (RAR 4 and RAR 5, case insensitive, cargo feature `rar`, decoder
`tools/archive_rar.rs`) goes through the same pipeline as zip and 7z. Rules:
- Passwords: the same `ERR_PASSWORD_REQUIRED` and the same two messages as zip and 7z (missing:
  `password required to extract this archive; call again with the 'password' parameter`; wrong or
  empty: `incorrect password for this archive; call again with the correct 'password' parameter`).
  A password on an archive with no encrypted entry is ignored. Encrypted file names are read in a
  listing pass, so a missing or wrong password is reported before anything else about the content.
- Only a symbolic link is refused (`ERR_NOT_SUPPORTED`, naming the entry). A hard link cannot be
  told from a file through the reader, so it extracts as an empty file (SPEC-0080 erratum 2).
- A multi volume name (`name.partN.rar`) is `ERR_NOT_SUPPORTED`, decided on the name alone, before
  the file is read.
- An entry that decodes to a different size from the declared one is `ERR_INVALID_ARGUMENT` naming
  the entry; a truncated archive is `ERR_INVALID_ARGUMENT` `archive is corrupt or not a valid rar
  file: ...`. A truncated archive of either version is refused, see the SPEC-0087 paragraph.
- Solid archives are decoded front to back. Since SPEC-0085 the declared sizes are admitted before
  anything is decoded (see the SPEC-0085 paragraph).
- Order of refusals (FR-NEW-015, reordered by SPEC-0085 and SPEC-0086): membership, missing path or
  folder, multi volume name, a `destination` climbing above the root (SPEC-0086, see its paragraph),
  encrypted names with a missing or wrong password, corrupt (a cut archive included, SPEC-0087), the first link or escaping
  name in archive order, encrypted data with a missing or wrong password, project storage limit,
  session write allowance (both on declared sizes), oversized entry, collision, then the two limits again
  at the moment of writing. The tests that pin each pair are `e2e_0080_047` to `e2e_0080_081`,
  except the storage limit versus collision pair, now `e2e_0085_mod_001` (was `e2e_0080_068`).

(2026-11-09, SPEC-0087.) Every truncated RAR archive is refused, RAR 4 and RAR 5, with
`ERR_INVALID_ARGUMENT` `archive is corrupt or not a valid rar file: unexpected end of archive`, through the
tool and the REST twin alike, and nothing is written. `ensure_rar_complete` (`tools/archive_rar.rs`) walks the
blocks from the marker and calls an archive complete only when it reaches the end block (RAR 4 included); an
archive that ends anywhere else, between two blocks or inside one, is cut. An old RAR 1.5 style archive
with no end block is therefore refused too. Stored data, a recovery record, a comment, a folder and
a solid archive all count as blocks. With encrypted file names the walk needs the headers, so it decrypts
them itself when `password` is the right one (`tools/archive_rar_crypt.rs`): RAR 5 with PBKDF2 HMAC SHA-256
and AES 256 CBC, RAR 4 with its SHA-1 key schedule and AES 128 CBC; `aes`, `cbc`, `hmac` and `sha2` are
optional dependencies behind the `rar` feature, and the code is safe Rust (no `unsafe`). The password
questions keep their place before the truncation: a missing password, an empty one or a wrong one is
answered as before, a cut archive with the right password is the corrupt refusal.

(2026-11-12, SPEC-0090.) RAR 4 with encrypted file names and a non empty password: RAR 4 has no check value,
so the password is judged on the first protected block, the one after the 20 byte leading part (marker, main
header, 8 byte salt included). Fewer than the 24 bytes of the block start (salt plus first AES block): cut.
A start that is not a possible block (kind 0x74 to 0x7b, declared size at least the kind's fixed part):
wrong password (`ERR_PASSWORD_REQUIRED`, `incorrect password for this archive; call again with the correct
'password' parameter`), even if the archive is also cut. A possible start but the archive ends before the
declared block: cut. The whole block failing its integrity check (CRC16): wrong password; damage inside
that very block with the right password reads the same, the lesser evil. Whole and passing: the walk goes
on as before. This closes the gap that was BL-0136 (a wrong RAR 4 password read as corrupt). Accepted limit:
a wrong password whose decoding looks like a possible block that runs past the end of the archive reads as a
cut; measured at most 9 of 300 generated passwords, pinned by E2E-019 with `wrong135` and bounded by
E2E-020 (291 or more of 300 are told wrong). Unchanged: no password (`password required`), an empty password,
readable file names, RAR 5. The REST `extract-archive` route gives the same codes and messages (428 for a
wrong password, 400 for a cut).

(2026-11-05, SPEC-0081.) A bare gzip file (suffix `.gz` that is not `.tar.gz`, flow in
`tools/archive_single.rs`) holds one file. Output name: the archive file name minus
`.gz`, case insensitive (`DATA.CSV.GZ` gives `DATA.CSV`); a name with no stem is
`ERR_INVALID_ARGUMENT`. Default destination: the folder of the archive (not a folder named after it);
a missing destination is created. Several gzip members are decoded as one stream. No clobber: an
existing file at the output path is `ERR_NO_CLOBBER` (`'<out>' already exists; pass
overwrite=true to replace it`), a folder is `ERR_NO_CLOBBER` even with `overwrite`
(`'<out>' is a folder and cannot be replaced`). A destination that is a file or sits under
one is `ERR_INVALID_ARGUMENT` (`'<path>' is a file, not a folder`). A password, even empty, is
`ERR_INVALID_ARGUMENT` (`password is not applicable to this archive format`). A corrupt or non gzip file is
`ERR_INVALID_ARGUMENT`, `archive is corrupt or not a valid gz file: <detail>`. The decoder never
holds more than the lower of two limits: the project's remaining storage
(`ERR_PROJECT_QUOTA_EXCEEDED`) and the session write allowance (`ERR_WRITE_QUOTA_EXCEEDED`,
charged the full output); a tie goes to the project. Order of refusals, first one wins:
1. caller not a member (`ERR_FORBIDDEN`); 2. path missing (`ERR_NOT_FOUND`); 3. path is a folder
(`ERR_INVALID_ARGUMENT`); 4. password given; 5. unusable output name; 6. destination climbing above
the root (`ERR_PATH_OUT_OF_BOUNDS`, `path escapes the volume root: <destination as typed>`, SPEC-0086);
7. destination is a file; 8. collision; 9. corrupt, project limit or session limit, whichever
the decoding meets first. Nothing is written, and no folder created, before the decode succeeds.

(2026-11-05, SPEC-0083.) A bare `.bz2` (not `.tar.bz2`/`.tb2`/`.tbz2`/`.tbz`) follows the same flow, name rule
(minus `.bz2`, case insensitive), limits and order of refusals as `.gz`. Several bzip2 streams are
decoded as one stream; bytes after a stream that are not another stream are refused. A corrupt or non
bzip2 file is `ERR_INVALID_ARGUMENT`, `archive is corrupt or not a valid bz2 file: <detail>`.

(2026-11-08, SPEC-0082.) A bare `.xz` (not `.tar.xz`/`.txz`) follows the same flow, name rule
(minus `.xz`, case insensitive), limits and order of refusals as `.gz` and `.bz2`. It is decoded by
liblzma (C code) through its multi stream decoder, with no memory limit: memory follows the bytes
decoded, which the cap bounds. Several xz streams are decoded as one stream; zero padding between
or after streams is accepted only as a multiple of 4 bytes; other bytes after a stream are refused.
BCJ filtered files and SHA-256 checks decode; an unknown check type is refused, and so is
lzma-alone (`.lzma`) data, as corrupt. A corrupt or non xz file is `ERR_INVALID_ARGUMENT`,
`archive is corrupt or not a valid xz file: <detail>`. `.tar.xz`/`.txz` is decoded by the same
liblzma decoder since SPEC-0084 (several streams and 4 byte padding accepted, `lzma-rs` removed).

(2026-11-09, SPEC-0084.) A compressed tar (`.tar.gz`/`.tgz`, `.tar.bz2`/`.tb2`/`.tbz2`/`.tbz`, `.tar.xz`/`.txz`) is
decoded under a bound instead of fully in memory (`decode_bounded_tar` in `tools/archive.rs`, the
capped reader of `archive_single.rs`). The bound is the lower of two sides plus `TAR_FRAMING`
(1 048 576 bytes of tar headers and padding): the project side is the remaining storage plus the
bytes already stored under the destination, no side when the project has no
limit; the session side is the remaining write allowance. The decode stops there and the call is
refused with the project storage or session allowance error of the limits, nothing written or
charged. Order of refusals: membership, missing path, folder, password, destination climbing above the
root (SPEC-0086), corrupt data or the bound
(whichever decoding meets first), the entry checks in archive order,
collision, project storage limit, session write allowance; the two limits are still
checked on the files' sizes. A plain `.tar` is not decoded under this bound. Every other unsupported
extension stays `ERR_NOT_SUPPORTED`. The HTTP twin answers and audits the same.
(2026-11-15, SPEC-0096.) `.tbz2` and `.tbz` are two more rows of the `SUFFIXES` table for the bzip2 tar:
the bound and the refusals key on the format, not the suffix, so nothing else changed.

(2026-11-09, SPEC-0085.) `.zip`, `.7z` and `.rar` admit their declared sizes before any entry is
decoded. Three steps in `tools/archive.rs` (`tools/archive_rar.rs` for RAR): a listing in
`spawn_blocking` that reads headers only and applies the header refusals; `quota::check_declared`,
which charges nothing and holds no lock across decoding; then the decode, each entry read through
the capped reader at its declared size. `check_declared` sums the declared sizes in `u128`/`i128`:
the declared total goes against the session write allowance, the declared net growth (last
occurrence per target path, minus the size of the file already stored there, whatever `overwrite`
says) goes against the remaining project storage. Same two refusals and messages as before, project
first, session second; equality is accepted; a size, total or growth above `i64::MAX` counts as too
large. Order of refusals per format, first one wins:
- zip: membership, missing path, folder, destination climbing above the root, unreadable archive, per entry in archive order a link, an
  escaping name or a duplicate name, missing password, local header disagreeing with the
  directory, project limit, session limit, then what decoding finds (wrong password, damaged data,
  entry past its declared size), collision, final admission.
- 7z: membership, missing path, folder, destination climbing above the root, encrypted or unreadable header, missing password, per entry
  an escaping name then a link or special type (moved from the decode to the listing, so a hostile
  entry is no longer hidden behind an earlier entry's wrong password), then the same as zip from the
  project limit on.
- RAR: membership, missing path, folder, multi volume name, destination climbing above the root, SPEC-0080 items 4 to 6, missing
  password, then the same as zip from the project limit on.
Every entry is read through `read_capped` at its declared size (RAR keeps UnRAR's own stop and the
length check), so a lying header, including a zip64 or 7z one, stops at the declared size plus at
most one 64 KiB chunk and the call is `ERR_INVALID_ARGUMENT`
`archive entry '<name>' decompressed to more bytes than declared <n>`. A refused archive holds 0
decoded bytes (the `LAST_HIGH_WATER` hook). The final `admit_paths` still runs under the admission
lock, so a concurrent write is caught. Plain `.tar`, compressed tar and bare `.gz`/`.bz2`/`.xz` are
unchanged; the HTTP twin answers and audits the same.

(2026-11-09, SPEC-0086.) One destination rule for every format, `resolve_destination` in
`tools/archive_single.rs`, used by the archive flow (`tools/archive.rs`) and the bare file flow. The
`destination` is judged on its own text: a NUL byte is refused (`path contains a NUL byte`), then the
segments are walked on `/` with a depth counter; `.` and empty segments stay, a `..` met at depth 0 is
refused with `ERR_PATH_OUT_OF_BOUNDS` and `path escapes the volume root: <destination as typed>`, any
other segment goes down; then the text is collapsed from the root with `PosixPath::normpath`. Judging
while walking refuses `a/../../x` and `/a/../../b` too, even though a clamp would land inside. It
applies to zip, 7z, RAR, every tar and the bare `.gz`, `.bz2`, `.xz` alike: the old behaviour where
the other formats clamped a climb to the root is gone. `..out` and `...` are plain folder names now
(they were refused by `normalize_path`'s prefix test, which this rule does not call). Place in the
order of refusals: right after membership, missing path, folder, unsupported extension, tar password
and multi volume RAR name (for a bare file also after the unusable output name), and before the archive
is read, so a refused call reads no byte, writes nothing, charges no quota and leaves no audit line.
A destination inside the project resolves as before: `out`, `/out`, `./out/./x`, `out//x` and
`a/../b` give `/out`, `/out`, `/out/x`, `/out/x`, `/b`; `a/..` gives `/`; no destination keeps its
default. The answer and the audit line name the resolved folder. The other path taking tools still
clamp a climb to the root (BL-0135). The tests are `e2e_0086_`; `e2e_0085_028` is replaced by
`e2e_0086_mod_001`.

Security note (2026-11-09, SPEC-0085): the signed cast bypass. The declared size of an entry was
cast `as i64`, so a declared size of 2^63 or more counted as a negative number, lowered the sums
(the project admission, the session counter in `safety.rs`) and passed both limits; the archive
then decoded in memory with no bound. Fixed: sizes convert with `i64::try_from(..).unwrap_or(i64::MAX)`,
the sums saturate (`admit_paths`, `check_and_charge`, `charge_write`, which never charges a negative
number), and the early check computes in `u128`/`i128`. The exploit tests are `e2e_0085_sec_001` to
`e2e_0085_sec_005`. The Office text extraction gap this note
left open is closed by SPEC-0088 (below).

Security note (2026-11-10, SPEC-0088): memory exhaustion in Office text extraction, the same class as
the SPEC-0085 note. `docs/extract.rs` decoded every part of a `.docx`, `.xlsx` or `.pptx` whole with no
bound, so a document of under 1 MB holding a part that inflates to hundreds of megabytes (or a
part that lies about its size) exhausted the memory of the server. Fixed: `OfficeBudget` checks the
declared size before decoding, caps the decode at the declared size and bounds the content. The exploit
tests are `e2e_0088_sec_001` to `e2e_0088_sec_013`. The two gaps this note left open (converter output,
PDF streams) are closed by SPEC-0089 (below).

Security note (2026-11-10, SPEC-0089; documented 2026-11-11): memory exhaustion in PDF extraction and in
the document converter's answer, the same class. The PDF reader inflated every stream whole, so a file of
a few kilobytes exhausted the memory of the server, and the converter's standard output, web body and
bundle were read whole. Fixed: `docs/pdf_bound.rs` measures every stream before the reader decodes it
and the converter answer is capped at `doc_service.max_input_bytes` (details below). The exploit tests
are `e2e_0089_sec_001` to `e2e_0089_sec_017`. Known gaps of the class: BL-0148 to BL-0153 (below); BL-0147
(form recursion) is closed by SPEC-0092.

Security note (2026-11-11, SPEC-0092): uncontrolled recursion (CWE-674) in PDF extraction and in the code
search. Seven kinds of small file aborted the whole server with `fatal runtime error: stack overflow`
(signal 6, the abort ends every project served by the process): a form that draws itself or two forms
drawing each other (862 B and 1 025 B), a chain of 400 or more distinct forms (165 KB for 1 000), a page
whose `Parent` is a loop (2.5 KB), a parent chain of 10 000 page groups (740 KB), a character map nested
1 000 lists or 100 000 strings or dictionaries, a composite font code map nested 10 000 lists, and a
Python, C or JavaScript source nested 10 000 parentheses or lists (the code search). Reproduced
before the fix on a 2 MiB thread (the worker default), the loops also on 8 and 64 MiB threads.
Fixed: the walk of `docs/pdf_bound.rs` refuses the three PDF nestings before the reader runs, and
`docs/symbols.rs` scans with explicit stacks. The exploit tests are `e2e_0092_sec_001` to
`e2e_0092_sec_013`, each in a child process. Accepted limits: folder trees and git trees nested deeply
are not covered by this fix; the limits were measured in a debug build (a release build has other
thresholds, the loops overflow in both).

(2026-10-19, SPEC-0041 US-0048.) A zip whose local headers disagree with the
central directory (name, alternate name, compression, encryption, AES settings,
data descriptor flag, mtime, CRC, sizes), that holds two entries of one raw name
(was "last wins", SPEC-0036 DR-008), or whose entries overlap is refused whole:
`ERR_INVALID_ARGUMENT`, `archive is corrupt or not a valid zip file: header
inconsistency detected: <zip detail>`. Order in `decode_zip`: pre scan (symlink,
path, decoded name clash) then the missing password answer then this check
(`docs/zip_check.rs::header_inconsistency`, zip's
`validate_header_consistency`, headers only, no decryption) then decode; so a
wrong password on a malformed archive gets the header refusal. Streamed zips
(zero local sizes plus data descriptor) pass. Test archives are crafted with
`zip_check::craft` (`rename_local`, `set_local_mtime`,
`set_local_compressed_size`, `overlap`).

## fs artifacts (5)

| Tool | Purpose | Params | Returns | Annotations | Who |
|---|---|---|---|---|---|
| `fs.get_document_structure` | headings of a document's last conversion, in document order, 100 per page, `page` null when unknown | `path`, `marker?` | `headings[{level,title,page}],marker` | RO,I,!OW | member |
| `fs.list_images` | images kept by a document's last conversion, `page` null outside PDF/PowerPoint | `path`, `marker?` | `images[{id,caption,page,ocr_text}],marker` (`ocr_text` null when none, SPEC-0104) | RO,I,!OW | member |
| `fs.get_image` | one kept image, picture byte identical as base64, format `png`/`jpeg`/`gif`/`webp`/`tiff`/`bmp` | `path`, `id` | `id,format,caption,page,ocr_text,base64`; the picture file alone is `GET /api/fs/{mount_id}/image-file?path=&id=` (REST only, no tool, SPEC-0104) | RO,I,!OW | member |
| `fs.list_tables` | tables kept by a document's last conversion | `path`, `marker?` | `tables[{id,caption,rows,columns,csv_quality}],marker` | RO,I,!OW | member |
| `fs.get_table` | one kept table, `markdown` (default) or `csv`, FR-NEW-002 bytes; `csv` answers the converter's exact file verbatim when the manifest gave one (`CSV quality: exact`, SPEC-0103) | `path`, `id`, `format?` | `id,format,content` | RO,I,!OW | member |
| `fs.get_document_view` | the stored conversion text with `[Image image-n: caption]` lines and tables as `markdown` (default), `csv-reference` or `both`; `view` (SPEC-0104): `standard` (default), `text` (the converter text as is) or `full` (adds `[OCR image-n: text]` after each image line and `structure`); never reads the sibling | `path`, `captions?`=true, `table_mode?`, `view?` | `path,text` (`full` adds `structure`) | RO,I,!OW | member |

Rules (2026-10-10, SPEC-0019):

* A set is kept by every conversion: `fs.extract_text` (built in tables, `CSV quality: exact`,
  only when it rewrites the sibling), `fs.documentize`, a documented `fs.write_bytes` or
  upload (converter tables `approximate`, images). Ids `image-n`/`table-n` follow reading order.
* Lists page by 100. `marker` is base64url of `v1\n<project>\n<kind>\n<path>\n<offset>`:
  valid for that project, list kind and document only, not bound to the person (another
  member may continue it); anything else is `Invalid continuation marker: <value>`
  (`ERR_INVALID_ARGUMENT`), spelled as issued. An offset past the end is an empty page.
* Check order: authorize, path is a file (`not a file: <path>`, `ERR_NOT_FOUND`), argument
  validation (format, table mode, marker), current set with a matching node `rev` (`No
  artifacts: document not extracted`, `ERR_NOT_FOUND`), id lookup (`Not found: <id>`).
* A changed or overwritten document loses its artifacts at once (rev mismatch). A move, a trash
  and a restore keep them; a copy gives the copy its own set (SPEC-0105); none calls the converter.
* `fs.move` refuses `source` equal to, inside or above `destination` (`ERR_INVALID_ARGUMENT:
  '<source>' cannot be moved onto or into itself`) before any overwrite deletes anything.
* `fs.copy` charges the session write allowance the file bytes plus the weight of the artifacts
  it copies, checked before anything is written: a refusal writes no file, set, audit entry or charge.
  Artifacts are not files: no listing, glob, grep, tree or export shows them.
* The set, sibling and conversion text are charged once to the session write quota before
  any write; a refusal keeps the previous set.
* REST twins (same functions, byte identical payloads, FR-NEW-020): `GET /api/fs/{mount_id}/`
  `images`, `image`, `tables`, `table`, `document-view`.
* The real `doc-convert` emits PNG only and deduplicates identical figures, so one picture referenced twice is one image. Manifest version 2 is defined (SPEC-0103, README "Manifest version 2"): without a manifest, images carry no page and tables an approximate CSV.

## fs document (3)

| Tool | Purpose | Parameters | Returns | Annotations | Auth |
|---|---|---|---|---|---|
| `fs.extract_text` | document to Markdown, stored as a companion `.md` | `path`, `max_chars:int=200000`, `preview_chars:int=4000`, `ocr:bool=true`, `refresh:bool=false` | `path`, `md_path`, `format`, `chars`, `cached`, `preview` (+ `note` on a PDF, see SPEC-0094 below) | !D,!RO,I,OW | member |
| `fs.write_docx` | render Markdown into a `.docx` | `path`, `markdown`, `title=null`, `overwrite:bool=false` | `path`, `bytes_written`, `overwritten` | D,!RO,!I,!OW | member |
| `fs.documentize` | convert a stored document through the external `doc_service` | `path`, `overwrite:bool=false` | `path`, `md_path`, `bytes_written`, `overwritten` | !D,!RO,I,OW | member |

`fs.documentize` is the retry surface of the document service: it reads the file
already in the volume, converts it and writes the companion at the same path
`fs.extract_text` uses, so the built-in extractor then answers `cached: true`.
Unlike an upload it honours the caller's `overwrite`, so an existing companion is
`ERR_NO_CLOBBER`. It answers `ERR_NOT_SUPPORTED` when `doc_service` is disabled or
the extension is not eligible, and `ERR_NOT_FOUND` when the path is not a file.
(2026-10-19, SPEC-0041 US-0049.) A `.docx`/`.xlsx`/`.xlsm`/`.pptx`/`.pptm`/`.potx`/`.ppsx`
whose parts disagree with its directory (`zip_check::ooxml_header_inconsistency`, keyed on
those seven extensions, never on `doc_service.extensions`) is refused before the converter
runs: `could not convert <path>: not a valid OOXML package: <reason>`
(`ERR_INVALID_ARGUMENT`), no companion. On a documented `fs.write_bytes` or upload the
source is still stored and the same error lands in `documentation.error`.

`fs.extract_text` reuses an up to date companion (`cached: true`, nothing
written) and handles PDF, DOCX, PPTX, XLSX, HTML, CSV, images (OCR through a
configured multimodal provider, disabled by default) and text. Audio and video
are unsupported (`ERR_NOT_SUPPORTED`). `fs.write_docx` requires a `.docx` path.
(2026-10-19, SPEC-0041 US-0049.) A malformed OOXML package (local header vs directory
mismatch, duplicate part names, overlapping parts) is refused with `could not extract
<path>: not a valid OOXML package: <reason>` (`ERR_INVALID_ARGUMENT`), even when an up to
date companion exists: the cached branch re reads the source bytes for those extensions
(one extra blob read per cached OOXML call) and leaves the companion untouched.

(2026-11-10, SPEC-0088.) The Office readers (`.docx`, `.xlsx`, `.xlsm`, `.pptx`, `.pptm`, `.potx`,
`.ppsx`, any letter case) are bounded by three limits: a part of at most 104857600 bytes decoded
(checked on the declared size, before decoding), at most 268435456 declared bytes of parts per call
(a part read twice counts twice), and at most 268435456 bytes of extracted content. A refusal is
`ERR_INVALID_ARGUMENT` (HTTP 400 on `POST /api/fs/{mount_id}/extract-text`, same code and message)
and its message starts `could not extract <path>: ` followed by one of:
`document part '<part>' is <n> bytes once decoded, over the limit of 104857600 bytes`,
`the document parts read total more than 268435456 bytes once decoded`,
`the extracted content exceeds 268435456 bytes`,
`document part '<part>' decodes to more bytes than declared <n>`.
Parts are read in reading order, so the first refusal wins. A part holding more than it declares
is now refused; before, it was read as absent and a lying sheet or slide vanished from the text.
Parts never read (images, embedded files, nested archives) are not counted, and an up to date cached
companion is answered without reading the document. A refusal writes no companion, artifact, charge
or audit line and logs one INFO line; `log_tool_failure` is wired at both doors (`mcp/server.rs`,
`api/dataplane.rs`). The tool description is unchanged (`TOOL_CONTRACT.txt` does not name the limits).
Known gaps: BL-0144 (a damaged
part is read as absent), BL-0145 (uncounted transient copies).

(2026-11-12, SPEC-0091.) `fs.extract_text` leaves other requests alone: the six built in readers (docx,
pptx, xlsx, pdf, html, csv) decode on blocking threads behind `DecodePlaces`, a fair ceiling equal to the
number of runtime worker threads; waiting extractions start in arrival order; a request refused before
decoding (membership, missing path) and a cached companion never wait. Another request is delayed by at
most 250 ms while documents decode (with one worker a trivial request waited 5.5 s behind a PDF at the
stream limit). A reader that stops (panic, runtime shutdown) answers `ERR_INTERNAL_ERROR` `could not
extract <path>: the document reader stopped unexpectedly` (HTTP 500 with `error` and `detail` on the REST
door), nothing left, place freed. An abandoned extraction leaves nothing; its place returns when the
decoding ends (the decoding is not interrupted). Answers, refusals and figures are unchanged. No tool
contract change. Accepted limits: `Extractor::extract_text` builds its own places (tests only); code
search and Markdown to Word rendering still run on the request thread.

(2026-11-11, SPEC-0089.) PDF limits of `fs.extract_text` (`docs/pdf_bound.rs`, none configurable).
Measure, defuse, arm: every stream is measured before the reader decodes it (Flate, LZW through `weezl`
and ASCII85 chains; a stream with more than 8 filters is left stored); one that passes its limit is
defused by renaming its filter so lopdf sees raw bytes; the reader is armed with a sink that stops at
the limit. An encrypted file has its `/Encrypt` renamed for the measure, then lopdf decrypts it; cross
reference streams are measured before the load. Limits: 1 000 000 objects; 104857600 bytes per decoded
stream; 268435456 bytes for the compressed object streams together at load, per page, and for the
extracted text. Refusals (`ERR_INVALID_ARGUMENT`, prefix `could not extract <path>: `): `the PDF holds
more than 1000000 objects`, `a PDF stream decodes to more than 104857600 bytes`, `the PDF object streams
decode to more than 268435456 bytes together`, `page <n> decodes more than 268435456 bytes of PDF
streams`, `the extracted content exceeds 268435456 bytes`, `pdf: the cross reference stream could not be
measured` (new), `pdf: the encryption dictionary could not be isolated`. A refusal leaves no trace (no
companion, artifact, charge, audit line). The test only hook `PDF_HELD` records the bytes held; the heavy
tests take `heavy()`. The REST door answers the same code and message (`e2e_0089_002`, `023`).
(2026-11-11, SPEC-0092.) Nesting refusals of `fs.extract_text` (same walk, `ERR_INVALID_ARGUMENT`, prefix
`could not extract <path>: `): forms nested more than 64 deep (`page <N> draws PDF forms nested more than
64 deep`, loops included), a page tree climb past 256 page groups for `Resources` or `MediaBox` (`page <N>
inherits from more than 256 levels of the PDF page tree`), a character or code map nested more than 64
levels (`a PDF character map is nested more than 64 deep`). A refusal leaves no trace (no companion
change, artifact, charge, audit line); an up to date companion still answers without reading the PDF. The
REST door answers the same code and message (`e2e_0092_002`, `015`). `fs.find_definition` and
`fs.find_references` answer a source of any nesting (explicit stacks, never refused for depth).
Accepted test limits (2026-11-11, audit G3 and G5): E2E-SEC-012/032 cannot isolate a reintroduced second
unbounded load of the title; the held figures are logical bytes and the real peak can reach about twice
that; parsed operations of 100 MB of tiny operators are not counted (BL-0151); the e2e with one colour profile in 300 saved states proves the page still extracts, not that the reader's copies are empty stand ins (that needs an allocation counter, so only the structural unit test guards DEC-009).

(2026-11-12, SPEC-0094.) A PDF answer of `fs.extract_text` (REST `extract-text` too, same engine) carries
`note`. Fresh, it is `pdf: damaged or cut content on page 2; the text may be incomplete` (`pages 1, 3` in
the plural), naming each page whose content, or the content of a form it draws, the strict parse
(`Content::decode_strict`) refused; the lenient parse still gives the text, so the text is unchanged. A
drawn XObject counts only when its dictionary has `/Subtype /Form` (image bytes are not content). At most
20 page numbers are listed, then `and N more` (`pages 1, 2, ..., 20 and 5 more`). It is empty when no page
is damaged, and a scan keeps its own message (`no extractable text layer (scanned PDF?); ...`). A cached
PDF answer carries the note of the extraction that stored the copy, read from the nullable
`doc_artifact_sets.note` column by `current_set_note(path, sha256 of the stored text)`, so the PDF is not
read again; the note is present even when empty. A copy stored before SPEC-0094 has no note and answers an
empty one until `refresh: true` reads the PDF again (a replaced PDF newer than its copy is read again
without it). An empty text stores no copy, so it is always answered afresh. A document that is not a PDF
never has a damage note, and its cached answer has no `note` key. Known limit (DEC-003): pages after a
point where the reader stops are not examined and not listed. The DDL fingerprints of `storage::meta`
moved with the additive column.

(2026-11-11, SPEC-0089.) `fs.documentize` and the documented `fs.write_bytes` / upload: the converter's
answer is capped by `doc_service.max_input_bytes` on every channel (cli stdout, web body, bundle files).
Past the cap the read stops, the child is killed and reaped before its working folder is removed, and the
call is refused with `ERR_INTERNAL_ERROR` `document service output is over the doc_service.max_input_bytes
limit of <cap>` (an upload keeps the source and reports it in `documentation.error`; documentize over
HTTP answers 500). No companion, artifact, charge or audit line. Diagnostic output keeps the last 8 192
bytes; in bundle mode stdout is discarded. Known gaps: BL-0148 (web fetch tools read whole), BL-0149 (quadratic page list), BL-0150 (growing bundle file),
BL-0151 (parsed structures larger than their bytes), BL-0152 (converter's own children survive the kill),
BL-0153 (no total across cross reference streams).

## admin (17)

| Tool | Purpose | Parameters | Returns | Annotations | Auth |
|---|---|---|---|---|---|
| `admin.create_project` | create a project and provision its volume | `project_id`, `owner`, `autopurge_enabled:bool=false`, `use_internal_purge:bool=false`, `file_retention_days:int=null`, `project_retention_days:int=null` | `project_id`, `owner`, `created_at` | !D,!RO,!I,!OW | admin |
| `admin.delete_project` | delete a project and tear down its volume | `project_id` | `project_id`, `deleted` | D,!RO,I,!OW | owner/admin |
| `admin.list_projects` | projects the caller can access | (none) | `projects[{project_id, owner, created_at, index_mode, is_owner}]` | RO,I,!OW | auth |
| `admin.list_all_projects` | every project | (none) | `projects[{project_id, owner, created_at, index_mode}]` | RO,I,!OW | admin |
| `admin.list_deleted_projects` | soft-deleted projects visible to the caller, with a countdown to permanent removal | (none) | `deleted_projects[{project_id, owner, deleted_at, days_until_permanent_removal}]` | RO,I,!OW | auth, admin sees all |
| `admin.list_users` | every known person plus platform admins | (none) | `users[{person, is_admin}]` | RO,I,!OW | admin |
| `admin.add_member` | add a member | `project_id`, `person` | `project_id`, `person`, `role` | !D,!RO,I,!OW | owner/admin |
| `admin.remove_member` | remove a member | `project_id`, `person` | `project_id`, `person`, `removed` | D,!RO,I,!OW | owner/admin |
| `admin.list_members` | members of a project | `project_id` | `project_id`, `members[{person, role, added_by}]` | RO,I,!OW | member or admin |
| `admin.set_index_mode` | set the search index mode and wipe or rebuild the index | `project_id`, `mode` | `project_id`, `index_mode`, `previous_mode`, `reindex_started` | D,!RO,I,!OW | owner/admin |
| `admin.get_index_mode` | read the search index mode | `project_id` | `project_id`, `index_mode` | RO,I,!OW | member or admin |
| `admin.set_purge_config` | configure auto-purge: enable flag, internal/external driver, file and project retention | `project_id`, `autopurge_enabled`, `use_internal_purge`, `file_retention_days`?, `project_retention_days`? | `project_id`, `autopurge_enabled`, `use_internal_purge`, `file_retention_days`, `project_retention_days` | !D,!RO,I,!OW | owner/admin |
| `admin.undelete_project` | clear a project's soft-delete flag, immediately restoring access | `project_id` | `project_id`, `undeleted` | !D,!RO,!I,!OW | owner/admin |
| `admin.set_project_quota` | set or clear a project's maximum storage size (SPEC-0010) | `project_id`, `max_mb`? | `project_id`, `max_mb` | !D,!RO,I,!OW | admin |
| `admin.get_project_disk_usage` | list a project's files from the largest to the smallest, trashed files included (SPEC-0047); also `GET /api/admin/projects/{project_id}/disk-usage?limit=` | `project_id`, `limit`? (whole number 1 to 500, default 100) | `project_id`, `max_mb` (null when unlimited), `used_bytes`, `file_count`, `truncated`, `entries[]` of `path`, `size`, `state` (`Active` or `In trash`), `original_path` (null when active) | RO,I,!OW | admin or owner, same refusals as `admin.get_project_quota`; cap refusal `limit must be between 1 and 500`; ties ordered by path in code point order |
| `admin.get_project_folder_usage` | read one folder of a project: its size and file count and its direct folders and files, largest first, a folder counting everything below it (SPEC-0049); also `GET /api/admin/projects/{project_id}/folder-usage?folder=&limit=` | `project_id`, `folder`? (absolute path, default `/`), `limit`? (whole number 1 to 500, default 100) | `project_id`, `max_mb` (null when unlimited), `used_bytes`, `folder{path,size,file_count}`, `item_count`, `truncated`, `items[]` of `path`, `kind` (`Folder` or `File`), `size`, `file_count` (folders only) | RO,I,!OW | admin or owner, same refusals as `admin.get_project_quota`; cap refusal `limit must be between 1 and 500`; folder refusals `folder '<path>' not found` (`ERR_NOT_FOUND`), `'<path>' is not a folder`, `path must start with /` (`ERR_INVALID_ARGUMENT`) |
| `admin.create_provider` | register a model provider (SPEC-0102); name trimmed, 1 to 64 characters, unique without regard to case; endpoint http or https with a host, no credentials, at most 2048 characters | `name`, `endpoint`, `auth_scheme="none"` (`none` or `bearer`), `secret=""` (required for `bearer`, refused for `none`, at most 4096 characters, stored AES 256 GCM under `FSA_TOKEN_KEY`) | `provider_id` (`prov-` plus 12 hex), `name`, `endpoint`, `auth_scheme`, `has_secret`, `created_at`, `updated_at` (UTC, seconds) | !D,!RO,!I,!OW | admin; refusals in order `name must be 1 to 64 characters`, `a provider named '<name>' already exists`, `endpoint must be an http or https URL`, `auth_scheme must be 'none' or 'bearer'`, `auth_scheme 'none' takes no secret`, `auth_scheme 'bearer' needs a secret`, `secret must be at most 4096 characters`, then `ERR_NOT_SUPPORTED: no master key is configured; set FSA_TOKEN_KEY` |
| `admin.update_provider` | change the fields given; the checks apply to the provider after the update; a no op answers it unchanged and is not recorded | `provider_id`, `name`?, `endpoint`?, `auth_scheme`?, `secret`? (empty keeps), `clear_secret=false` | the provider | !D,!RO,I,!OW | admin; first `provider not found: <id>` (`ERR_NOT_FOUND`), then `secret and clear_secret cannot be given together`, then the create order |
| `admin.delete_provider` | delete a provider and its secret; a provider that still has models is refused | `provider_id` | `deleted: true` | D,!RO,!I,!OW | admin; `provider not found: <id>`, `provider '<name>' still has <n> model(s)` (`ERR_INVALID_ARGUMENT`) |
| `admin.list_providers` | providers by name without regard to case, 100 per answer | `marker=""` | `providers[]`, `marker` (absent when none remain) | RO,I,!OW | admin; `marker is not valid` |
| `admin.create_model` | register a model of a provider (SPEC-0102); name trimmed, 1 to 128 characters, unique within its provider without regard to case | `provider_id`, `name`, `kind` (`text_embedding`, `image_embedding`, `reranker`, `ocr`, `chat`), `dimension`? (1 to 8192, required for the two embedding kinds, refused for the others), `max_input_tokens`? (1 to 1000000) | `model_id` (`mdl-` plus 12 hex), `provider_id`, `name`, `kind`, `dimension`, `max_input_tokens` (null when absent), `is_default` (false until US-0285), `created_at`, `updated_at` | !D,!RO,!I,!OW | admin; refusals in order `provider not found: <id>` (`ERR_NOT_FOUND`), `name must be 1 to 128 characters`, `kind must be one of text_embedding, image_embedding, reranker, ocr, chat`, `dimension is required for kind '<kind>'`, `dimension applies to embedding kinds only`, `dimension must be between 1 and 8192`, `max_input_tokens must be between 1 and 1000000`, `a model named '<name>' already exists for provider '<provider>'` |
| `admin.update_model` | change `name`, `dimension` or `max_input_tokens`, the others keep their value; the checks apply to the model after the update; a no op answers it unchanged and is not recorded; provider and kind never change | `model_id`, `name`?, `dimension`?, `max_input_tokens`? (0 removes the limit) | the model | !D,!RO,I,!OW | admin; `model not found: <id>` (`ERR_NOT_FOUND`), `the dimension of a server default model cannot change` (`ERR_INVALID_ARGUMENT`, a different value only), then the create order |
| `admin.delete_model` | delete a model; a server default is refused (`model '<name>' is a server default; clear the default first`, `ERR_INVALID_ARGUMENT`) | `model_id` | `deleted: true` | D,!RO,!I,!OW | admin; `model not found: <id>` |
| `admin.list_models` | models by provider name then model name without regard to case, 100 per answer | `provider_id=""`, `kind=""`, `marker=""` | `models[]`, `marker` (absent when none remain) | RO,I,!OW | admin; `provider not found: <id>` (`ERR_NOT_FOUND`), the kind text, `marker is not valid` |
| `admin.set_default_model` | make a model the server default of its kind, replacing the previous one (SPEC-0102, US-0285); a text embedding model must have dimension `search.embedding.dimensions` (1536 unless configured); the model already default is returned and writes no record; search does not read it yet | `kind` (`text_embedding` or `reranker`), `model_id` | the model with `is_default: true` | !D,!RO,I,!OW | admin; `kind must be 'text_embedding' or 'reranker'`, `model '<name>' is of kind '<k>', not '<kind>'`, `the default text embedding model must have dimension <n>` (`ERR_INVALID_ARGUMENT`), `model not found: <id>` (`ERR_NOT_FOUND`) |
| `admin.clear_default_model` | remove the server default of a kind; a kind with none answers the same and writes no record | `kind` | `cleared: true` | !D,!RO,I,!OW | admin; the kind text |
| `admin.get_default_models` | the server default of each kind | (none) | `text_embedding` and `reranker`, each a model or null | RO,I,!OW | admin |
| `admin.list_setting_changes` | the record of changes, newest first (`change_id` descending), 100 per answer; a refused or no op change writes nothing; a secret appears as the field name `secret` only | `marker=""` | `changes[{change_id (chg- plus 12 digits), at, person (lower case), action, target_kind, target_id, target_name, fields[]}]`, `marker` | RO,I,!OW | admin; `marker is not valid` |
| `admin.create_converter` | register a document converter (SPEC-0106, US-0309: the `program` form; the `http` form is refused with `ERR_NOT_SUPPORTED` until US-0310); every input is untyped and checked by `settings/converter_rules.rs`, `null` equals omitted | `name`, `form` (`program` or `http`), `command` (list of texts, `{document}` once, `{outdir}` at most once), `extensions?`, `max_input_bytes?` (default 536870912), `timeout_seconds?` (default 900); the http fields `url`, `auth_header`, `secret`, `file_field`, `response_field` are refused on a program converter, an empty text included | `converter_id` (`conv-` plus 12 hex), `name`, `form`, `command`, `url`, `auth_header`, `has_secret`, `file_field`, `response_field`, `extensions`, `max_input_bytes`, `timeout_seconds`, `created_at`, `updated_at` (null for a field of the other form) | !D,!RO,!I,!OW | admin first; then `name must be 1 to 64 characters`, `a converter named '<name>' already exists`, `form must be 'program' or 'http'`, `command must be a list of text arguments`, `command is required for a program converter`, the two placeholder counts, `<field> is not valid for a program converter`; recorded as `create_converter` |
| `admin.update_converter` | change a converter, never its form (SPEC-0106, US-0312); a field not given keeps its value, a blank `secret` keeps the stored one, `clear_secret` removes it; the creation checks run on the converter after the update; an answer sent back is accepted | `converter_id`, the creation fields but `form` (given only with the stored value), `clear_secret?` (untyped, boolean) | the converter, as `admin.create_converter` (a no op returns it unchanged, `updated_at` included) | !D,!RO,I,!OW | admin first; `converter not found: <id>`, `form cannot be changed`, `converter_id in the body does not match`, `secret and clear_secret cannot be given together`, `clear_secret is not valid for a program converter`, `clear_secret must be true or false`, then the creation texts; recorded as `update_converter` with the field names, no value |
| `admin.list_converters` | converters by name without regard to case, 100 per answer (SPEC-0106) | `marker` (untyped, `null` or empty reads the first page) | `converters[]`, `marker` (`null` on the last page) | RO,I,!OW | admin first; `marker is not valid` |
| `admin.delete_converter` | delete a converter and its secret (SPEC-0106, US-0314) | `converter_id` | `converter_id` | D,!RO,!I,!OW | admin first; `converter not found: <id>`; `converter '<name>' is the platform default`; `converter '<name>' is used by <n> project(s)` (a project in its grace period counts); recorded as `delete_converter` |
| `admin.set_default_converter` | make a converter the platform default (SPEC-0106, US-0313; one row in `converter_default`) | `converter_id` | `converter` | !D,!RO,I,!OW | admin first; `converter not found: <id>`; recorded as `set_default_converter` (target kind `default_converter`), none when it is already the default |
| `admin.clear_default_converter` | remove the platform default | none | `converter: null` | !D,!RO,I,!OW | admin first; recorded as `clear_default_converter`, none when no default was set |
| `admin.get_default_converter` | read the platform default | none | `converter` or null | RO,I,!OW | admin first |
| `admin.set_project_converter` | make a converter the converter of a project (`project.converter_id`); a soft removed project included, kept over a restore, dropped with a removal for good; gives no file access | `project_id`, `converter_id` | `project_id`, `converter` | !D,!RO,I,!OW | admin first; `project '<id>' not found` before `converter not found: <id>`; recorded as `set_project_converter` (target kind `project_converter`, id the project), none when unchanged |
| `admin.clear_project_converter` | remove the converter of a project, which then uses the default | `project_id` | `project_id`, `converter: null` | !D,!RO,I,!OW | admin first; `project '<id>' not found`; recorded as `clear_project_converter`, none when none was set |
| `admin.get_project_converter` | read the converter of a project | `project_id` | `project_id`, `converter` or null | RO,I,!OW | admin first; `project '<id>' not found` |
| `admin.get_project_quota` | read a project's limit and current usage, trashed files included (SPEC-0020); also `GET /api/admin/projects/{project_id}/quota` | `project_id` | `project_id`, `max_mb` (null when unlimited), `used_bytes` | RO,I,!OW | admin or owner; a non admin gets the owner refusal even for a missing project |

2026-10-12 (SPEC-0020): the limit refuses every growing `fs.*`/`git.*` operation on its net change with `ERR_PROJECT_QUOTA_EXCEEDED` (`project storage quota of <N> MB exceeded: <U> bytes used, <W> bytes requested`, HTTP 507), before the session quota. Moves, trash, restore, aborts and pushes are never checked; a limit below usage removes nothing.

`project_id` must be 3 to 32 characters of lowercase letters, digits and hyphens,
with alphanumeric first and last characters. Creation provisions the volume and
rolls the ACL row back if provisioning fails. Deletion also purges
`state/git/{id}.db` and the bare repo directory when git is enabled, so a
recreated id never inherits stale refs.

## git (44, registered only when `git.enabled`)

| Tool | Purpose | Parameters | Returns | Annotations | Auth |
|---|---|---|---|---|---|
| `git.init` | make the volume a git repository | (`mount_id`) | `mount_id`, `initialized`, `message` | !D,!RO,I,!OW | member |
| `git.status` | HEAD, branch, refs, in-progress operation | (`mount_id`) | `mount_id`, `head`, `branch`, `refs[{name, sha}]`, `operation` (null, or `{op_type, source_ref, current_step, total_steps, remaining_conflicts[], continue_with, abort_with}`) | RO,I,!OW | member |
| `git.branches` | branches with their sha, the current marker and the divergence against `refs/remotes/origin/{name}` | (`mount_id`) | `mount_id`, `branches[{name, full_ref, sha, current, upstream, ahead, behind}]` (`upstream`/`ahead`/`behind` are null with no tracking ref, never 0) | RO,I,!OW | member |
| `git.branch_create` | create a branch at a start point | `name`, `start_point=""`, `checkout:bool=false` | `branch`, `sha`, `checked_out` | !D,!RO,!I,!OW | member |
| `git.branch_switch` | move HEAD to an existing branch and rewrite the volume to its tree; refuses a dirty volume | `name` | `branch`, `sha`, `changed`, `files_changed` | D,!RO,I,!OW | member |
| `git.branch_delete` | remove `refs/heads/{name}`; refuses the checked-out branch, and an unmerged one without `force` | `name`, `force:bool=false` | `branch`, `sha`, `forced` | D,!RO,I,!OW | member |
| `git.branch_reset` | force-move a branch pointer; rewrites the volume only for the checked-out branch | `name`, `target_commit`, `force:bool=false` | `branch`, `old_sha`, `new_sha`, `checked_out`, `files_changed` | D,!RO,I,!OW | member |
| `git.reset` | move the CURRENT branch's pointer; `soft` moves the ref alone and never touches the volume (`hard` rewrites the volume, US-018) | `target_ref`, `mode` (`soft`\|`hard`, required, no `mixed`) | `mode`, `old_sha`, `new_sha`, `files_changed` (0 for soft) | D,!RO,I,!OW | member |
| `git.tags` | tags, ascending by name bytes; `sha` is the peeled commit, `object` the ref target | (`mount_id`) | `mount_id`, `tags[{name, full_ref, sha, object, kind}]`; `kind` is `lightweight` or `annotated`, an annotated entry adds `message` (exactly as stored), `tagger`, `tagger_email`, `tagged_at` (UTC `YYYY-MM-DDTHH:MM:SSZ`) | RO,I,!OW | member |
| `git.tag_create` | create `refs/tags/{name}` at a branch, tag or commit (default HEAD); a non empty `message` makes an annotated tag (message kept byte for byte, tagger built like a commit author); refuses, in this order, a paused operation, an invalid name, a project with no commit, an unknown target and an existing name without `overwrite` (`Tag '{name}' already exists. Choose another name or confirm overwriting it.`); audited `git.tag_create` or `git.tag_overwrite` | `name`, `target=""`, `overwrite:bool=false`, `message=""` | `mount_id`, `name`, `kind`, `sha`, `object`, `overwritten`, `previous_sha?` | !D,!RO,!I,!OW | member |
| `git.tag_delete` | remove the local `refs/tags/{name}` only (never the commit, other refs or the remote); refuses during a paused operation and an unknown tag (`Tag '{name}' does not exist.`); audited `git.tag_delete` | `name` | `mount_id`, `name`, `sha` (peeled commit) | D,!RO,I,!OW | member |
| `git.log` | commits from a ref | `ref_name=null`, `limit:int=20`, `path=null` | `mount_id`, `commits[]` | RO,I,!OW | member |
| `git.show` | one commit plus its diff | `commit_sha` | `commit{}`, `diff` | RO,I,!OW | member |
| `git.diff` | diff two refs, or a ref and the volume | `from_ref`, `to_ref=null`, `path=null` | `mount_id`, `from`, `to`, `diff` | RO,I,!OW | member |
| `git.commit` | commit the current volume state | `message`, `author_name=null`, `author_email=null` | `commit_sha`, `message`, `author`, `timestamp` | !D,!RO,!I,!OW | member |
| `git.checkout_file` | restore a file from a commit | `commit_sha`, `path` | `path`, `commit`, `size` | D,!RO,I,!OW | member |
| `git.blame` | last change per line | `path`, `ref_name=null` | `path`, `lines[{line, commit, author, email, date}]` | RO,I,!OW | member |
| `git.merge` | merge a ref into the checked-out branch, surfacing conflicts | `source_ref`, `squash:bool=false`, `message=null` | `status` (`merged`\|`already_up_to_date`), `merge_commit`, `fast_forward`, `squashed`, `files_changed`; on a conflict the shape below instead | D,!RO,I,!OW | member |
| `git.merge_resolve` | finish a conflicted merge, per file by strategy or by literal content | `resolutions[{path, strategy?, content?}]` | `status` (`merged`\|`conflict`), `merge_commit`, `remaining_conflicts[]`, `resolved_count`, `files_changed` | D,!RO,!I,!OW | member |
| `git.merge_abort` | abandon a conflicted merge, discarding every recorded resolution | (`mount_id`) | `status` (`aborted`), `operation` | !D,!RO,!I,!OW | member |
| `git.rebase` | validate an interactive rebase plan and replay it onto another commit; the whole todo is checked before any commit is created | `onto`, `todo[{action, sha, message?}]` | `status` (`up_to_date`), `branch`, `new_tip`, `replayed`, `dropped`, `squashed` | D,!RO,!I,!OW | member |
| `git.rebase_continue` | resume the paused rebase once every conflicting path of the paused commit is resolved | `resolutions[{path, strategy?, content?}]?` | the `git.rebase` shape, or the conflict shape when it pauses again | D,!RO,!I,!OW | member |
| `git.rebase_abort` | abandon the paused rebase, restoring the branch exactly | (`mount_id`) | `status` (`aborted`), `operation` | !D,!RO,!I,!OW | member |
| `git.cherry_pick` | apply one commit's change onto the checked-out branch as a NEW commit, keeping the original author | `commit_sha`, `mainline:int=0` | `status` (`committed`\|`already_present`), `new_sha` (null when nothing was created), `source_sha`; on a conflict the shared conflict shape instead | !D,!RO,I,!OW | member |
| `git.cherry_pick_continue` | finish the paused cherry-pick, per file by strategy or by literal content | `resolutions[{path, strategy?, content?}]?` | `status` (`committed`), `new_sha`, `source_sha` | !D,!RO,!I,!OW | member |
| `git.cherry_pick_abort` | abandon the paused cherry-pick, restoring the branch and every byte | (`mount_id`) | `status` (`aborted`), `operation`, `restored_sha` | !D,!RO,!I,!OW | member |
| `git.revert` | undo one commit by adding a NEW commit carrying the exact inverse change, leaving the original in history | `commit_sha`, `mainline:int=0` | `status` (`committed`\|`already_present`), `new_sha` (null when nothing was created), `reverted_sha`; on a conflict the shared conflict shape instead | !D,!RO,!I,!OW | member |
| `git.revert_continue` | finish the paused revert, per file by strategy or by literal content | `resolutions[{path, strategy?, content?}]?` | `status` (`committed`), `new_sha`, `reverted_sha` | !D,!RO,!I,!OW | member |
| `git.revert_abort` | abandon the paused revert, restoring the branch and every byte | (`mount_id`) | `status` (`aborted`), `operation`, `restored_sha` | !D,!RO,!I,!OW | member |
| `git.stash_save` | snapshot the dirty volume as a commit under `refs/stash/*`, then revert the volume to HEAD's tree | `message=null` | `stash_id`, `sha`, `message`, `base_sha`, `branch`, `created_at`, `files_stashed` | !D,!RO,!I,!OW | member |
| `git.stash_list` | every stash entry of the volume, newest first; read only, so it works while an operation is in progress | (`mount_id`) | `mount_id`, `stashes[{stash_id, message, base_sha, branch, created_at}]`, `count` | RO,I,!OW | member |
| `git.stash_drop` | delete one entry by id; the volume and the commit object are untouched | `stash_id` | `stash_id`, `dropped` | D,!RO,I,!OW | member |
| `git.stash_apply` | replay one entry onto whatever is checked out now, keeping the entry | `stash_id` | `stash_id`, `status` (`applied`\|`conflict`), `files_changed`, `dropped` (always `false`); on a conflict the shared conflict shape carries those keys too | D,!RO,!I,!OW | member |
| `git.stash_pop` | the same replay, deleting the entry only once every byte has landed; a conflict keeps it and reports `dropped: false` | `stash_id` | `stash_id`, `status`, `files_changed`, `dropped` | D,!RO,!I,!OW | member |
| `git.remote_add` | record a named remote; https only, no embedded credential, host declared in `git.hosts`; a duplicate name is refused, never upserted | `name`, `url` | `name`, `url`, `host`, `provider` | !D,!RO,!I,!OW | member |
| `git.remote_remove` | delete a remote and every ref under `refs/remotes/{name}/`; an unknown name is `ERR_NOT_FOUND`, never a silent no-op | `name` | `name`, `removed` | D,!RO,!I,!OW | member |
| `git.remote_list` | every remote of the volume, empty list when there is none; read-only, so it stays available while an operation is paused | (`mount_id`) | `mount_id`, `remotes[{name, url, host, provider}]`, `count` | RO,I,!OW | member |
| `git.remote_clone` | clone a remote into the volume | `url`, `branch=null`, `depth:int=0` | `mount_id`, `url`, `branch`, `commit`, `commit_message`, `files_imported`, `commits_imported`, `depth`, `auth`, `skipped?`, `tags_added[]`, `tags_kept_local[]` | !D,!RO,!I,OW | member |
| `git.remote_push` | push a local branch to a named remote, fast-forward unless a lease is supplied | `branch`, `remote` (default `origin`), `remote_branch`, `force`, `expected_remote_sha` | `branch`, `remote`, `created`, `up_to_date`, `remote_sha`, `auth` (credential class, never a value), `forced`; plus `overwritten_sha` when forced and `remote_branch` when it differs from the local name | D,!RO,I,OW | member |
| `git.remote_push_tag` | publish one local tag under the same name, never forced; refuses an unknown tag, no remote, a commit not on the remote and a differing remote tag unless `overwrite` and `expected_remote_commit` (full id or 7+ character prefix of the remote tag's current commit; a mismatch answers `has moved`), the push then leased on the advertised object; `up_to_date` for an identical remote tag (same commit, and for two annotated tags the same message); audited (`git.tag_publish`) only on a real push | `tag`, `remote` (default `origin`), `overwrite:bool=false`, `expected_remote_commit=""` | `mount_id`, `tag`, `remote`, `created`, `up_to_date`, `remote_sha` (the tag's commit), `auth` | D,!RO,I,OW | member |
| `git.remote_push_tags` | publish every due local tag in one push (SPEC-0054): one listing classifies each tag, sorted by name bytes, as `to publish`, `up to date` (same object, or two annotated tags on one commit with one message), `differs (remote at <7>)` or `commit not on remote` (after a send also `published` or `refused by remote: <reason>`); without `confirm_count` a preview (`Tags to publish: <N>. Confirm by stating <N> to send them.`, or `No tag to publish.`); with it equal to N one push of creation refspecs leased on "absent", never a branch, a commit or a replacement; refusals in order: not a member, no remote (`Cannot publish tags: this project tracks no remote.`), remote not tracked (`... does not track a remote named '<r>'.`, an empty name means `origin`), connection, number not a count, wrong number (`Tags to publish: <M>, not <N>. ...` then the list); holds the write lock, preview included; audited (`git.tag_publish`) once per tag published | `remote` (default `origin`), `confirm_count` (absent/null, a string of 1 to 9 digits, or an integer) | `mount_id`, `remote`, `to_publish`, `published`, `sent`, `tags[]` (`name`, `outcome`, `commit`, `remote_commit?`, `reason?`), `lines[]`, `message`, `auth` | !D,!RO,I,OW | member |
| `git.remote_delete_tag` | delete one tag on the remote (SPEC-0053), refspec `:refs/tags/<n>` leased on the advertised tag object; refusals in order: not a member, no such remote (`Cannot delete tag '<n>': this project tracks no remote.`), remote tag absent (`Remote tag '<n>' does not exist.`, also an empty, blank or invalid name), no commit stated (`points at <7>. State that commit to confirm its deletion.`), stated commit differs (`has moved to <7>`, an annotated tag confirmed by its peeled commit); a lease lost between listing and push re lists once, never retried; local tags untouched; audited (`git.tag_remote_delete`) only after the push | `tag`, `remote` (default `origin`), `expected_remote_commit=""` | `mount_id`, `tag`, `remote`, `deleted_commit` (full id), `auth` | D,!RO,!I,OW | member |
| `git.remote_fetch` | fetch objects, update `refs/remotes/origin/*` only, never a working file | (`mount_id`) | updated refs, `refs_stale[]`, objects downloaded, `tags_added[]`, `tags_kept_local[]` | !D,!RO,I,OW | member |
| `git.remote_pull` | fetch, then fast-forward or three-way merge the checked-out branch; a conflict pauses as a `merge` | `branch` | fast-forward, merge outcome, or the shared conflict response; every outcome adds `tags_added[]`, `tags_kept_local[]` | D,!RO,I,OW | member |

Every combine operation reports a conflict with one shape: `{status:
"conflict", operation, operation_id, source_ref, current_step, total_steps,
conflicts[{path, ours, theirs, base, binary, type_change}], continue_with,
abort_with}`, where each of `ours`, `theirs` and `base` is `{exists, content}`
and `content` is null for a deleted or binary side. `current_step` and
`total_steps` are zero-based and null for a single step operation. A conflict
applies nothing: no file, no commit, no ref move, and never a conflict marker
in the volume.

A conflict is finished with `git.merge_resolve` or abandoned with
`git.merge_abort` (the pair `continue_with`/`abort_with` names). Each
resolution carries exactly one of `strategy` (`ours`, the checked-out branch's
side, or `theirs`, the side being merged in, each taken whole) or `content`
(the literal bytes, which is how a caller merges the two sides itself); an
empty `content` is a valid resolution to an empty file, a missing one is an
error. A call is all-or-nothing: a path that is not in conflict, a duplicate
path, an unknown strategy or an empty list rejects the whole call and changes
nothing. Resolving only some paths keeps the operation in progress, records
the decisions in the `git_operations` row (so they survive a restart) and
writes nothing to the volume; resolving the last one charges the write quota,
applies every resulting file atomically and only then advances the ref.

A commit object is `{sha, short_sha, message, author, author_email, timestamp,
date, parents[]}`. `git.remote_clone` uses the OAuth token stored for the
detected provider/host when there is one; an empty remote returns
`{mount_id, url, files_imported: 0, message}`. `git.remote_push`,
`git.remote_fetch` and `git.remote_pull` take no `url`: they resolve the
volume's stored `origin`. See [`git.md`](git.md) for the full remote pipeline,
the `git.hosts` host map and the pull merge semantics.

## git.auth (4, registered only when `git.enabled`)

| Tool | Purpose | Parameters | Returns | Annotations | Auth |
|---|---|---|---|---|---|
| `git.auth` | start the OAuth device flow | `provider` (`github` or `gitlab`), `host=null`, `instance_url=null` | `status` (`pending`), `provider`, `user_code`, `verification_uri`, `expires_in`, `message` | !D,!RO,!I,OW | auth |
| `git.auth_status` | authentication status | `provider=null`, `host=null` | one host: `authenticated`, `provider`, plus `scopes`, `expires_at` when authenticated; omitted: `statuses[]` | RO,I,!OW | auth |
| `git.auth_revoke` | drop the stored token | `provider=null`, `host=null` | `provider`, `revoked` | D,!RO,I,!OW | auth |
| `git.token_set` | seed a PAT already held for a declared host, skipping the device flow | `host`, `token`, `expires_at=null` | confirmation, never echoes the token | !D,!RO,I,!OW | auth |

`git.auth` returns as soon as the provider issues a user code; a detached task
polls the token endpoint, so the client waits by calling `git.auth_status`. A
token belongs to a person plus host (not provider alone, and not a mount): see
[`git.md`](git.md#token-identity-is-per-person-host).

## git.pr (6, registered only when `git.enabled`)

| Tool | Purpose | Parameters | Returns | Annotations | Auth |
|---|---|---|---|---|---|
| `git.pr_create` | open a pull request (GitHub) or merge request (GitLab) on a declared remote | `mount_id`, `base`, `head`, `title`, `body=null`, `draft=false`, `remote=origin` | the normalized pull request (21 keys, see below) | !D,!RO,!I,OW | member |
| `git.pr_list` | list the pull requests of a declared remote, filtered by state | `mount_id`, `state=open` (`open`\|`closed`\|`merged`\|`all`), `remote=origin` | `pull_requests[]` of the normalized shape, plus `count` | RO,I,OW | member |
| `git.pr_get` | read one pull request, enriched with its review state and its check state; an unknown state is a failure, never `none` | `mount_id`, `pr_number`, `remote=origin` | the normalized pull request | RO,I,OW | member |
| `git.pr_diff` | read the unified diff of one pull request, bounded by `git.max_pr_diff_mb` | `mount_id`, `pr_number`, `remote=origin` | `diff`, `truncated`, plus the pull request identity | RO,I,OW | member |
| `git.pr_merge` | merge a pull request on the provider with `merge`, `squash` or `rebase` | `mount_id`, `pr_number`, `strategy`, `commit_title=null`, `commit_message=null`, `remote=origin` | the normalized pull request in state `merged`, plus `note` naming `git.remote_fetch` | D,!RO,!I,OW | member |
| `git.pr_review` | submit a review verdict: `approve`, `request_changes` or `comment`; a provider refusal is surfaced as the provider's own status and message | `mount_id`, `pr_number`, `verdict`, `body=null`, `remote=origin` | the submitted review, plus the pull request identity | !D,!RO,!I,OW | member |

Every `git.pr_*` tool returns ONE shape, defined once in
`git/provider/model.rs` as `PullRequest`, in this key order: `provider`, `host`,
`number`, `title`, `body`, `state` (`open`/`closed`/`merged`), `draft` (a
separate boolean, never folded into `state`), `base`, `head`, `author`, `url`,
`created_at`, `updated_at`, `commits`, `changed_files`, `additions`,
`deletions`, `review_state`, `checks_state`, `mergeable` (tri state: `true`,
`false`, `null` when the provider has not computed it), `raw` (the provider
payload untouched, already scrubbed of the credential). Details and the mapping
rules: [`git.md`](git.md#the-pull-request-surface-gitpr_-us-025).

## search (4, served when search is enabled)

(2026-11-22, SPEC-0101 US-0279) Engines in `tools/search_semantic.rs` (`index`, `query`), shared by
the `#[tool]` handlers and the test registry. Details in [`search.md`](search.md#chunking).

| Tool | Purpose | Key params | Returns |
|---|---|---|---|
| `search.index` | build or refresh the default chunk set of each text file, then index its chunks | `path`, `recursive:bool=false` | `indexed` (chunks), `skipped`, `path` |
| `search.query` | ranked entries | `query`, `mode=null`, `top_k:int=10`, `rerank:bool=true` | `results[{path, score, chunk, rank, chunk_number, heading_path}]`, `mode_used`, `warning` |
| `search.delete` | drop the entries of a path | `path`, `recursive:bool=false` | `deleted` |
| `search.status` | index statistics | | `bm25_docs`, `vector_chunks`, `mode`, `bm25_warm` |

`search.index` refuses `chunk_size` and `chunk_overlap`: `ERR_INVALID_ARGUMENT: chunk_size and
chunk_overlap are no longer accepted` (hidden from the schema); a single path with no text:
`ERR_NOT_SUPPORTED: no text to cut for <path>`. A result of a file with no default set (entries built
before the upgrade) has `chunk_number` 0, `heading_path` `[]` and its text unchanged.

## Served set per configuration (2026-10-14, SPEC-0027)

`McpServer::new` serves exactly what the flags enable; a tool not served is
absent from `tools/list` and its `tools/call` answers `-32602 tool not found`
before any handler runs.

| Configuration | Served |
|---|---|
| default | 92 (53 `fs.*`, 39 `admin.*`) |
| git flag on | 146 |
| git and search flags on | 150 |
| every flag, pandoc on `PATH` | 171 (169 without pandoc: `doc.*` registers nothing) |
| one optional family alone | 92 + `web` 5, `context7` 2, `sqlite` 8, `db` 5, `doc` 2 |

The optional families (`web`, `context7`, `sqlite`, `db`, `doc`) are not
`#[tool]` methods: `add_optional_families` in `mcp/server.rs` runs each enabled
family's production `register()` into one shared `ToolRegistry`, and adds one
`ToolRoute::new_dyn` per tool. The rmcp `Tool` is deserialized from the
family's own `ToolSchema::to_list_entry()`, so the served schema equals
`catalog()` (what `/api/swagger.json` reads); the call goes through
`ToolRegistry::call` then `to_call_result`. The family handlers authorize
`mount_id` themselves, unchanged. They are outside the 140 tool golden
contract. Tests: `tools/tool_surface_e2e.rs`.

(2026-11-12, SPEC-0093) Remote answers of `web.*` and `context7.*` are read by `tools::web::read_answer`
under `ANSWER_LIMIT` (16 777 216 bytes): a declared length past it is refused before any body byte, a
chunked body is abandoned at the limit, nothing is decompressed (counted as received), and at most the
limit plus 1 048 576 bytes are held. Refusal: `ERR_INVALID_ARGUMENT`
`response from host '<host>' exceeds the 16 MiB limit`. `web.download` and `web.fetch` with a
destination use `write_limit` instead: the lower of the project side and the session side, a tie to the
project, refusing with today's storage messages. Order: argument checks (`url must start with http:// or
https://`, `mount_id and save_path must both be provided, or neither`), membership and path, request and
read, then the write checks on the whole answer (read before write, project, session). A refused call
leaves no file, charge, audit line or companion, and logs one line. Tool descriptions are unchanged.

(2026-11-18, SPEC-0098) `doc.to_docx` and `doc.to_pptx` run pandoc with `--sandbox` (`run_pandoc`, `tools/doc.rs`),
so a Markdown or HTML source never makes the server request an address or embed a host file. In a `.docx` an
image that names a URL or a path becomes its alt text and the call succeeds; a `data:` image is still embedded.
A `.pptx` that names such an image fails with `ERR_INTERNAL_ERROR` `pandoc exited with exit status: 99: File
'<name>' not found in resource path`, and no file is written. `--sandbox` needs pandoc 2.15 or later; an older
pandoc fails every conversion with its own message. Tests: `bt_0098_*` in `tools/doc.rs` (a counting stand in
server and a zip reader read the effect, not the tool answer).

## Authorization model

| Gate | Implementation | Applies to |
|---|---|---|
| Verified identity | `IdentityResolver::verify`, RS256, `iss`, `exp`/`nbf`, 30s leeway | every tool call, checked before dispatch |
| Membership | `AdminBackend::require_member` | every `fs.*` and `git.*` tool |
| Platform admin | caseless match against `auth.admins` | `admin.create_project`, `admin.list_all_projects`, `admin.list_users` |
| Owner or platform admin | `AppState::require_owner_or_admin` | `admin.delete_project`, `admin.add_member`, `admin.remove_member`, `admin.set_index_mode`, `admin.set_purge_config` |
| Member or platform admin | inline in `tools/admin.rs` | `admin.list_members`, `admin.get_index_mode` |

**Separation of duties.** A platform admin manages projects and membership and
can list everything, but membership is never implied: `AppState::authorize` is
membership only, with no admin bypass, so a platform admin gets `ERR_FORBIDDEN`
on `fs.read` of a project it is not a member of. An admin who needs the files
adds itself as a member, which leaves a `project_member` row and an `added_by`
trail. The same rule holds on the REST plane and on the git HTTP routes.

`is_owner` and `is_admin` in tool output are computed caselessly, exactly like
the checks that authorize, so a reported flag can never contradict the gate.

## Known limits (2026-11, accepted, not tracked as backlog)
Observed while hardening the extraction paths; none is a reported defect, all are accepted as they are.
* Office extraction holds copies the SPEC-0088 figures do not count: a part repaired from invalid UTF-8, the joined and trimmed content, and an empty grid cell costing a 24 byte `String` for 4 bytes of markup. The real heap of a call is a small multiple of the stated limits.
* The PDF text pass rebuilds the page map on every page (`output_doc_page`), so time grows with the square of the page count, within SPEC-0089's object limit.
* Folder trees and git trees nested deeply are not covered by the SPEC-0092 nesting bounds (2026-11-11); the nesting limits were measured in a debug build.
* PDF parsed structures (content operations, cross reference entries, object tables) can be much larger than the bytes they come from; SPEC-0089 bounds decoded bytes, not parsed structures. This is also why the held figures are logical bytes.
* No total limit across the cross reference streams of a PDF `/Prev` chain: each is held to the stream limit, the work grows with the chain, the peak stays near one stream.
* A converter bundle file that keeps growing between the size check and the read is read at its new size; a converter's own child processes survive the kill on the cap or the timeout. Both need an operator-chosen, misbehaving converter.
* A damaged Office part reads as absent ("main part is missing", a sheet or slide silently left out) instead of being reported as damaged.
* Other tools and REST routes clamp a path climbing above the project root to the root (SPEC-0018); only `extract_archive` refuses it (SPEC-0086).
* The debug test profile does not optimize the RAR crypto crates, so SPEC-0087 E2E-034 runs a reduced set of right-password lengths (`[profile.dev]` in `Cargo.toml` helps).

## Registry tools and the models screen (SPEC-0102, 2026-11-22)

The 12 registry tools (`admin.create_provider` to `admin.list_setting_changes`) are platform admin only, reading
included, and count in the 39 `admin.*` tools, with the ten SPEC-0106 converter tools. They have REST routes under `/api/admin/` (`api.md`) and the browser
screen `/app/models` calls the same functions of `tools/models.rs`. No tool logs an argument; a secret is a `SecretText`.

## Document converters (SPEC-0106)

The ten `admin.*converter*` tools above are platform administrator only, reading included, and nothing in
them gives file access. Resolution at conversion time is project, then platform default, then the
configuration file converter; see `architecture.md` "Document converters registry". The secret is write
only (`secret`, `clear_secret`), answers carry `has_secret`. Tests: `cargo test -p fsa-core --lib -- e2e_0106_`.
