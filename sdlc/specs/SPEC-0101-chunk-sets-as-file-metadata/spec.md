# Chunk sets: a file keeps its chunks as metadata, functional specification

> Id: SPEC-0101
> Nature: FEAT
> Depth: L
> Depth evidence: FEAT / depth L: 3 areas (chunking, search indexing, document metadata and tools), a new persisted concept (the chunk set), 29 requirements, 5 new tools or tool changes
> Status: implemented
> Generated on: 2026-11-21
> Project: existing
> From backlog: BL-0168
> Depends on: SPEC-0100
> Security: n/a
> CVSS: n/a
> Affected: n/a
> Fixed in: n/a

## 1. Summary
Search today cuts a file into character windows of 1000 characters, by itself, twice (once per engine), and keeps nothing about where a window comes from. This increment makes chunks part of the file: a file keeps one or several chunk sets (a set is one complete cutting of the file's text by one strategy and one group of settings). The default set is cut by the `structural` strategy, which follows headings, tables, lists and code and counts in tokens; the `fixed` strategy cuts by token count. Members can list a file's sets, list and read chunks with their neighbours, create and delete sets, and recalculate one. Search builds its entries from the default set and no longer cuts files itself. The structural rules follow the paragraph v3a specification (version 2.0, 2026-11-20).

## 2. Current State (MANDATORY, every depth)
Search indexes only text files (UTF-8). It cuts each into windows of 1000 characters with an overlap of 100, looking back up to 50 characters for a space, a full stop or a new line, and it does this separately for each search engine. A chunk keeps no heading, no page and no position: a search result is a path, a score, a text and a rank. Nothing lets a member see, read or navigate the chunks. Converted documents (PDF, Word) are indexed only through their Markdown sibling file. Documents keep images, tables and a document view as metadata, but no chunks. The tool `search.index` takes `chunk_size` and `chunk_overlap` (characters). Every member of a project can read and write every file of it; there is no read only role.

### 2.1 What users can do today
- Index a file with `search.index`, choosing a window size and an overlap in characters; automatic indexing uses 1000 and 100 (14.1 rows 1 to 4).
- Search and get back a chunk text with a score and a rank (14.1 row 5).
- Read the images, tables and document view of a converted document (14.1 row 6).
- Rebuild the search entries of a file by hand or after it settles (SPEC-0100) (14.1 row 7).

### 2.2 Specs governing this area
- SPEC-0008/FR-002, FR-012, FR-020: windows of characters, look back of 50 characters, auto indexing at 1000 and 100. This increment supersedes them (FR-MOD-001); SPEC-0008 is not edited.
- SPEC-0008/E2E-004 and E2E-011 assume character windows and the arguments `chunk_size` and `chunk_overlap`; they are modified (E2E-MOD-001, E2E-MOD-002). SPEC-0100/E2E-069 assumes the old order of refusals; it is modified (E2E-MOD-003).
- SPEC-0100/FR-NEW-011, FR-NEW-012: a rebuild replaces only the search entries of the file; FR-NEW-008 and FR-NEW-009: `fs.recalculate` knows only the kind `search_index`. Modified by FR-MOD-002 and FR-MOD-003.
- SPEC-0019: artifacts of converted documents; their tools are the model for the paging rules used here (100 per page, marker).

### 2.3 What is verified today
Tests exercise the character cutting (5 tests of the cutter), search over indexed text, auto indexing and the rebuild jobs. No test exercises headings, tokens, sets, neighbours or any structure (14.1 row 8).

## 3. Scope
### 3.1 In scope
- Chunk sets as metadata of a file; strategies `structural` and `fixed`; their settings and bounds.
- Tools to list sets, list chunks, read a chunk with neighbours, create a set, delete a set; recalculation of one set through `fs.recalculate`.
- Cutting in tokens, with the `cl100k_base` ruler.
- Text sources: text files, and converted documents through the text kept with their conversion.
- Default set built when a text file settles; search entries built from the default set; indexed text with a header line; `chunk_number` and `heading_path` in search results.
- `search.index` uses the default set and loses `chunk_size` and `chunk_overlap`.
- Server defaults for the settings under `search.chunking`.
- Sets follow moves and deletions; a copy gets its own default set.

### 3.2 Out of scope, and why
- Search fusion, one result per unit, top 10 and search by image (paragraph v3a sections 7 and 8, search by document): BL-0172.
- Vectors in pgvector and their recalculation: BL-0171.
- Per project choice of strategy and settings: BL-0170; this increment has server wide defaults only.
- Settings in the database: BL-0164; defaults live in the configuration file for now.
- Automatic building of sets for a converted document, and rebuilding converted documents when their source changes: BL-0176; their sets are made by hand here.
- Git operations and archive extraction scheduling indexing: BL-0177.
- A web screen for chunks: BL-0175.
- Per sentence embedding splits: the owner chose `fixed` and `structural` only.
- Sets counting in the storage quota: they do not count.

### 3.3 Constraints imposed by the context
- Chunk sizes are counted in tokens of the `cl100k_base` vocabulary, independent of the embedding model; the owner imposes it so cuts do not change with the model.
- The structural rules follow the paragraph v3a specification; the owner wants the chunks to stay close to it.

## 4. Actors
| Actor | Role |
|---|---|
| Member | member of a project: reads, creates, deletes and recalculates chunk sets |
| Non member | a signed in person who is not a member: refused |
| Operator | sets the server defaults of the chunking and restarts the server |
| Platform admin | creates, deletes and restores projects, sets index modes |

## 4.5 Bounded contexts (MANDATORY at M and L)
| Context | What it owns | Key concepts |
|---|---|---|
| Documents | the text of a file and the metadata kept with it | text file, converted document, document text, chunk set, chunk, strategy, settings |
| Search | the searchable entries built from chunks | search entries, default set, indexed text |
| Derived data | rebuild jobs (SPEC-0100) | kind, job, settle delay |

## 5. Usage scenarios
### SC-001: a saved Markdown file gets its default set
**Actor:** Member
**Preconditions:** project `proj` indexes its files; settle delay 3 seconds; Alice (`alice@example.com`) is a member.
**Flow:**
1. Alice saves `/guide.md` → the write succeeds at once.
2. After the settle delay the rebuild cuts the file into chunks, stores them as the default set and builds the search entries from it.
3. Alice lists the sets of `/guide.md` → she sees one default set, `structural`, `current`.
**Postconditions:** search finds the file; its chunks can be read.
**Exceptions:**
- EXC-001a: the project indexes with vectors (mode `rag` or `both`) and the embedding service is down → the rebuild fails; the previous sets stay, marked `out_of_date`.
- EXC-001b: the project does not index its files → no default set is built.
- EXC-001c: the file is not text → no set and no error.

### SC-002: reading chunks and their context
**Actor:** Member
**Flow:**
1. Alice lists the sets of `/big.md` → one default set with 250 chunks.
2. She lists the chunks, a page of 100 at a time → each with number, heading path, pages, size and a preview.
3. She reads chunk 5 with 2 chunks before and 1 after → she gets chunks 3 to 6 with full text.
**Exceptions:** an unknown set, an absent chunk number, a window above 20, a bad marker, a non member, a project that does not exist, an absent path or a folder → each refused with its exact text (FR-NEW-017).

### SC-003: making and deleting sets by hand
**Actor:** Member
**Flow:**
1. Alice creates a `fixed` set of 1000 tokens with an overlap of 200 → the set is built and listed, not the default.
2. She creates the same set again → she gets the same set.
3. She deletes it → it is gone; the default set cannot be deleted.
**Exceptions:** unknown strategy, a setting that does not apply, a value out of range, an overlap not below the size, a sixth set, a file with no text, a person who is not a member.

### SC-004: a converted document is chunked from its metadata text
**Actor:** Member
**Flow:**
1. A PDF was converted (body, image captions, recognized text, tables). Alice creates a `structural` set for it → the set is built from the complete document text.
2. She reads its chunks like those of any file.
**Exceptions:** a document not converted yet has no text to cut.

### SC-005: a text with no heading falls back to fixed
**Actor:** Member
**Flow:**
1. A text file has no heading. Its default set is built as `fixed` and the list says `fallback_from` `structural`.
2. Alice adds a heading and the next rebuild makes the set `structural`.

### SC-006: how the structural strategy cuts
**Actor:** Member (observes the chunks)
**Flow:** a text with headings, pages, tables, lists, code and figures is cut: sections at headings that leave at most 2 open, small units merged under one parent, large units cut into pieces without overlap, lists in batches of 40, table header rows repeated, fences kept, figures attached as references.
**Exceptions:** a text with carriage returns and NUL characters is cleaned first; an empty file gives a set with 0 chunks.

### SC-007: moves, copies, deletions
**Actor:** Member
**Flow:** moving a file carries its sets; copying gives the copy its own default set; deleting a file or a project removes its sets.

### SC-008: recalculating by hand
**Actor:** Member
**Flow:** Alice recalculates the default set by hand with `fs.recalculate`; the set is rebuilt at once and the search entries follow. A set that is not current is marked `out_of_date` until rebuilt.
**Exceptions:** unknown set, no default set, `set_id` with the wrong kind, unknown kind.

### SC-009: search reads the default set
**Actor:** Member
**Flow:** a search result now names the chunk number and its heading path; the heading is part of what search matches. `search.index` indexes the default set.
**Exceptions:** `chunk_size` and `chunk_overlap` are refused.

### SC-010: server defaults and the upgrade
**Actor:** Operator
**Flow:** the operator sets the chunking defaults in the configuration; files indexed before the upgrade keep their old entries until their next rebuild.
**Exceptions:** a default out of range or not a whole number stops the start.

## 6. Functional requirements
### 6.1 New
#### FR-NEW-001 [EARS-E]: structural cuts follow the headings
> WHEN a set with the strategy `structural` is built THE system SHALL close the current section at each heading that leaves at most `heading_cut_depth` headings open, and SHALL start the next section with that heading.

- **Business rules:** A heading opens by its level: a heading closes every open heading of the same or a deeper level, then opens. The cut decision counts the open headings, not the number of `#`: a document that starts with a level 3 heading cuts there.
- **Business rules:** A heading whose opening leaves more than `heading_cut_depth` headings open does not cut; it stays inside the current section.
- **Business rules:** Default `heading_cut_depth` is 2; allowed values are 1 to 6.
- **Business rules:** A heading is a line of 1 to 6 `#`, then a space or a tab, then a title that is not empty. A `#` line inside a code block is not a heading.
- **Business rules:** The text of a section is its blocks joined by a blank line (`\n\n`).
- **Scenario:** SC-006
- **Priority:** Must

#### FR-NEW-002 [EARS-U]: heading path and heading line
> The system SHALL keep the heading line in the text of the chunk it opens, and SHALL give each chunk a heading path made of the titles of the headings open when its section started, at most 2 titles.

- **Business rules:** Text before the first heading has an empty heading path.
- **Business rules:** A chunk made by merging sections takes the common beginning of their heading paths (FR-NEW-006).
- **Business rules:** The title of a heading is its text without the `#` marks, without an optional closing run of `#`, trimmed.
- **Scenario:** SC-006
- **Priority:** Must

#### FR-NEW-003 [EARS-U]: page markers, thematic breaks and the metadata table
> The system SHALL record the page of a chunk from the page markers of the text, SHALL ignore thematic breaks, and SHALL leave out of every chunk the table placed first in the text whose header cells are all empty.

- **Business rules:** A page marker is a line `---` (trailing spaces allowed) whose next non blank line is `*[Page N]*`; it cuts nothing and adds no text. A line `---` alone is a thematic break.
- **Business rules:** The pages of a chunk are the lowest and the highest page recorded on its blocks; they are unknown when the text holds no page marker.
- **Business rules:** A text with no page marker gives chunks with unknown pages.
- **Scenario:** SC-006
- **Priority:** Must

#### FR-NEW-004 [EARS-U]: figures are references, never text
> The system SHALL attach each figure reference of the text to the last chunk that holds text of the last unit starting before the figure, and SHALL add no figure text to any chunk.

- **Business rules:** A figure is a line `[Figure]`, a line `<!-- image -->`, or a line holding only an image link; a `[Figure]` line followed by an image link line is one figure whose reference is the link target.
- **Business rules:** A figure before every unit is attached to the first chunk. A chunk with no figure has an empty list of figure references.
- **Scenario:** SC-006
- **Priority:** Must

#### FR-NEW-005 [EARS-U]: tables, lists and code are cut only when too large
> The system SHALL NOT cut a table in the middle of a row, a list in the middle of an item, or a code block in the middle of a line, unless the element is above `max_tokens`.

- **Business rules:** A paragraph that ends with `:` and sits right before a list travels with the list as its first line; if the paragraph becomes empty it disappears.
- **Business rules:** A code block opens at a line of up to 3 spaces then 3 or more backticks or tildes and closes at a later line made only of that character, at least as long as the opening run, or at the end of the text. A table is a line starting with `|` followed by a separator row (only `|`, `:`, `-` and spaces, starting and ending with `|`), plus the following lines starting with `|`. A list is a run of lines that start, after optional indentation, with `-`, `*`, `+`, or digits followed by `.` or `)`, then whitespace; it continues over the next item lines and lines starting with 2 spaces, and a blank line inside is allowed when followed by such a line. A paragraph is any other run of lines, up to a blank line or a line of the kinds above.
- **Business rules:** A table, a list or a code block is one unit; units merge with their neighbours only by the merge rule (FR-NEW-006).
- **Scenario:** SC-006
- **Priority:** Must

#### FR-NEW-006 [EARS-E]: small adjacent units are merged
> WHEN two adjacent units are under the same parent and neither was produced by cutting a larger element and their text together fits `max_tokens` THE system SHALL merge them into one chunk.

- **Business rules:** The top level title of a unit is the first title of its heading path; a unit with an empty heading path has none. Two units are under the same parent only when their top level titles are equal, or both are absent: a unit with the path `A` and a unit with the path `A > B` are under the same parent; units with the paths `A` and `B` are not. (This is the reading of paragraph v3a section 5.4 that makes its acceptance tests 11 and 12 hold.)
- **Business rules:** A chunk made by merging units takes the unit number of the first merged unit. Unit numbers are given to the units before merging, from 1, in the order of the text.
- **Business rules:** A merged chunk takes the common beginning of the heading paths and the widest page span; its text is its units joined by a blank line, and the fit test is the token count of that joined text.
- **Business rules:** Default `max_tokens` is 2048; allowed values are 64 to 8192; it is the one limit for merging, for a unit, for a table and for any piece.
- **Scenario:** SC-006
- **Priority:** Must

#### FR-NEW-007 [EARS-E]: a unit above max_tokens is cut into pieces
> WHEN a unit is above `max_tokens` THE system SHALL cut it into pieces of at most `max_tokens` tokens, with no overlap and no text repeated, except the table header rows and the code fences.

- **Business rules:** Prose is cut at blank lines and the paragraphs are packed; a paragraph above the limit is cut between lines, then between words, then between tokens, so that the pieces joined give back the text.
- **Business rules:** A table is cut into groups of rows, each group starting with the header row and the separator row.
- **Business rules:** A list is cut into groups of items; the introduction line stays on the first piece only; an item above the limit is cut like prose.
- **Business rules:** A code block is cut at blank lines, then at lines, then between words, then between tokens; it keeps its opening fence line and a closing fence on every piece; a code block never closed runs to the end of the text and its pieces get a closing fence.
- **Business rules:** Pieces are filled in order: elements are added to a piece while the sum of their token counts, plus 1 per join, plus the token counts of the header rows or fences the piece carries, stays at most `max_tokens`; the joined text, header rows or fences included, is then counted and, while it is above the limit and holds more than one element, the last element goes to the next piece. A prose piece is trimmed and empty pieces are dropped, so the pieces joined give back the text up to the whitespace at their ends.
- **Business rules:** A top level item of a list starts at a list marker line indented no more than the first item line; other lines belong to the current item; trailing blank lines are dropped.
- **Business rules:** When the header rows or the fences alone exceed `max_tokens`, a piece holds one row or one line with its header or fences and can exceed `max_tokens`: this is the only case of a chunk above the limit.
- **Business rules:** All pieces of one unit share the same unit number.
- **Scenario:** SC-006
- **Priority:** Must

#### FR-NEW-008 [EARS-E]: long lists are cut in batches of items
> WHEN a list holds more than `list_max_items` top level items THE system SHALL cut it into batches of `list_max_items` items, and SHALL NOT merge a batch with a neighbouring unit.

- **Business rules:** Default `list_max_items` is 40; allowed values are 2 to 200.
- **Business rules:** The introduction line is kept on the first batch only.
- **Scenario:** SC-006
- **Priority:** Must

#### FR-NEW-009 [EARS-U]: what a chunk holds
> The system SHALL give each chunk its text, the title of its document, its heading path, its number in the set (1, 2, 3 and so on), its unit number, its first and last page, its token count and its figure references.

- **Business rules:** The title of the document is the file name without its extension.
- **Business rules:** Fields of a chunk in answers: `number`, `unit`, `heading_path` (a list of 0 to 2 titles), `page_start` and `page_end` (whole numbers, both `null` when unknown), `token_count`, `figures` (a list of strings, empty when none), `document_title`, plus `text` in `fs.get_chunk` and `preview` in `fs.list_chunks`.
- **Business rules:** Chunks split from one unit share the unit number; the unit numbers rise with the order of the text.
- **Business rules:** The token count of a chunk is the count of its text.
- **Scenario:** SC-002
- **Priority:** Must

#### FR-NEW-010 [EARS-U]: chunking is deterministic and bounded
> The system SHALL produce, for the same text and the same settings, the same chunks character for character, and SHALL produce no chunk above `max_tokens` tokens with the strategy `structural` or above `size_tokens` tokens with the strategy `fixed`.

- **Business rules:** Before cutting, NUL characters are removed and `\r\n` becomes `\n`.
- **Business rules:** Tokens are counted with the `cl100k_base` ruler (imposed by the owner, independent of any embedding model).
- **Business rules:** The strategy `structural` leaves no overlap between chunks; the one exception to the size bounds is the case stated in FR-NEW-007.
- **Scenario:** SC-006
- **Priority:** Must

#### FR-NEW-011 [EARS-E]: the fixed strategy
> WHEN a set with the strategy `fixed` is built THE system SHALL cut the text into chunks of `size_tokens` tokens, each starting `size_tokens` minus `overlap_tokens` tokens after the previous one, and SHALL apply no sentence rule.

- **Business rules:** Defaults: `size_tokens` 2048 (allowed 32 to 8192), `overlap_tokens` 0 (allowed 0 up to `size_tokens` minus 1).
- **Business rules:** The text of a `fixed` chunk is the decoding of its tokens, a broken multibyte character becoming U+FFFD; its `token_count` is the number of tokens of the slice. The last chunk holds what remains; no chunk is made when the previous chunk already reaches the end of the text. Every chunk of a fixed set has an empty heading path, unknown pages, unit number equal to its number and no figure reference.
- **Scenario:** SC-003
- **Priority:** Must

#### FR-NEW-012 [EARS-O]: a document with no heading falls back to fixed
> IF a set with the strategy `structural` is built from a text that holds no heading THEN THE system SHALL build it with the strategy `fixed` and the server default fixed settings, and SHALL report the set with the strategy `fixed` and `fallback_from` equal to `structural`.

- **Business rules:** A text is a document with no heading when no line is a heading in the sense of FR-NEW-001.
- **Business rules:** An empty text holds no heading: its `structural` set is the `fixed` fallback with 0 chunks.
- **Business rules:** A set made by hand keeps the settings of its request: every rebuild of it, once the text holds a heading, cuts with those settings (server defaults are read only for the settings omitted at creation); the default set takes the server defaults of the moment of each rebuild.
- **Business rules:** The fallback applies at every later rebuild of the set, so a heading added to the file later makes the next rebuild `structural` again.
- **Scenario:** SC-005
- **Priority:** Must

#### FR-NEW-013 [EARS-E]: creating a chunk set
> WHEN a member of a project asks for a chunk set that the file of that project does not hold, with a strategy and optional settings, THE system SHALL build that set from the text of the file and answer with the set.

- **Business rules:** The tool is `fs.create_chunk_set` with `mount_id`, `path`, `strategy` (`fixed` or `structural`) and `settings` (an object, empty by default); omitted settings take the server defaults and are recorded in the set.
- **Business rules:** A set made by hand is not the default set.
- **Business rules:** Settings are validated before anything is built; the number of sets is checked last (FR-NEW-017).
- **Business rules:** The answer is the set object of FR-NEW-014.
- **Scenario:** SC-003
- **Priority:** Must

#### FR-NEW-014 [EARS-E]: listing the sets of a file
> WHEN a member of a project asks for the chunk sets of a file THE system SHALL list each set with its id, strategy, settings, whether it is the default set, `fallback_from` when it applies, its number of chunks, its token count, its state and the time it was built.

- **Business rules:** The tool is `fs.list_chunk_sets` with `mount_id` and `path`; the answer is `{sets: [...]}`; a file with no set gets an empty list.
- **Business rules:** Fields: `set_id`, `strategy`, `settings`, `default`, `fallback_from` (only when it applies), `chunk_count`, `token_count`, `state` (`current` or `out_of_date`), `built_at` (UTC, RFC 3339 with seconds). Order: the default set first, then by `built_at` oldest first, then by `set_id`.
- **Business rules:** A set is `out_of_date` while its file has changed since the set was built.
- **Business rules:** The `token_count` of a set is the sum of the `token_count` of its chunks, 0 for a set of 0 chunks.
- **Scenario:** SC-002
- **Priority:** Must

#### FR-NEW-015 [EARS-E]: listing the chunks of a set
> WHEN a member of a project asks for the chunks of a set THE system SHALL list them by number with, for each, its number, unit number, heading path, pages, token count, figure references and a preview of its first 200 characters.

- **Business rules:** The tool is `fs.list_chunks` with `mount_id`, `path`, `set_id` and `marker`; at most 100 chunks per answer; the answer is `{chunks: [...], marker?: string}` and the marker is absent when no chunk remains.
- **Business rules:** The preview is the first 200 characters of the text, or the whole text when it is shorter.
- **Business rules:** A marker is valid only for the set it came from.
- **Scenario:** SC-002
- **Priority:** Must

#### FR-NEW-016 [EARS-E]: reading a chunk with its neighbours
> WHEN a member of a project asks for chunk N of a set with `before` and `after` THE system SHALL answer with the full chunk N, the `before` chunks that precede it and the `after` chunks that follow it, in order of number.

- **Business rules:** The tool is `fs.get_chunk` with `mount_id`, `path`, `set_id`, `number`, `before` (default 0) and `after` (default 0); each is 0 to 20. A neighbour beyond the first or the last chunk is absent, not an error.
- **Business rules:** Each chunk of the answer carries its full text and the fields of FR-NEW-009. To widen a window the caller asks again with a chunk number nearer the edge or with larger values.
- **Scenario:** SC-002
- **Priority:** Must

#### FR-NEW-017 [EARS-O]: refusals of the chunk tools
> IF a request to a chunk tool is not allowed or not valid THEN THE system SHALL refuse it, change nothing, and answer with the exact text of the rule that failed.

- **Business rules:** Order of the checks: project absent or deleted, not a member, path absent, path is a folder, then the checks of the tool (strategy, settings, set, number, window, text, number of sets). Settings are checked in the order of the request and, for each, in this order: unknown name, does not apply to the strategy, not a whole number, out of range (`overlap_tokens`: 0 to 8191), then once all are valid the overlap against the size; the first failure is answered.
- **Business rules:** A setting name that no strategy has is refused with `ERR_INVALID_ARGUMENT: unknown setting '<name>'`; a setting value that is not a whole number is refused with `ERR_INVALID_ARGUMENT: <name> must be a whole number between <min> and <max>`; the limit of 5 sets is checked last and only when the requested set does not exist yet.
- **Business rules:** Texts: project absent `ERR_PROJECT_NOT_FOUND` (existing text); non member `ERR_FORBIDDEN: '<person>' is not a member of '<mount_id>'`; path absent `ERR_NOT_FOUND: file not found: <path>`; folder `ERR_INVALID_ARGUMENT: path must be a file`; strategy `ERR_INVALID_ARGUMENT: strategy must be 'fixed' or 'structural'`; a setting that does not apply `ERR_INVALID_ARGUMENT: setting '<name>' does not apply to strategy '<strategy>'`; a setting out of range `ERR_INVALID_ARGUMENT: <name> must be between <min> and <max>`; overlap `ERR_INVALID_ARGUMENT: overlap_tokens must be smaller than size_tokens`; more than 5 sets `ERR_INVALID_ARGUMENT: a file holds at most 5 chunk sets`; no text `ERR_NOT_SUPPORTED: no text to cut for <path>`; set absent `ERR_NOT_FOUND: chunk set not found: <set_id>`; chunk absent `ERR_NOT_FOUND: chunk <number> not found in set <set_id>`; window `ERR_INVALID_ARGUMENT: before and after must be between 0 and 20`; marker `ERR_INVALID_ARGUMENT: marker is not valid`.
- **Business rules:** The setting ranges are those of FR-NEW-001, FR-NEW-006, FR-NEW-008 and FR-NEW-011. `<min>` and `<max>` are those bounds.
- **Scenario:** SC-003
- **Priority:** Must

#### FR-NEW-018 [EARS-E]: deleting a chunk set
> WHEN a member of a project deletes a set that is not the default set THE system SHALL remove the set and its chunks and answer `{deleted: true}`.

- **Business rules:** The tool is `fs.delete_chunk_set` with `mount_id`, `path` and `set_id`.
- **Business rules:** Deleting the default set is refused with `ERR_INVALID_ARGUMENT: the default chunk set cannot be deleted`; deleting an unknown set is refused as in FR-NEW-017.
- **Scenario:** SC-003
- **Priority:** Must

#### FR-NEW-019 [EARS-E]: recalculating one set by hand
> WHEN a member of a project asks `fs.recalculate` for the kind `chunks` of a file THE system SHALL rebuild that set at once from the current text, ignoring the settle delay, and answer with the job as for the kind `search_index`.

- **Business rules:** `fs.recalculate` gains the optional parameter `set_id` for the kind `chunks`; absent, it means the default set. The job record names the kind `chunks`.
- **Business rules:** When the rebuilt set is the default set, the search entries of the file follow the new chunks (FR-NEW-023) only when search is enabled for the server and the project indexes its files; otherwise only the set is rebuilt and the search entries are neither created nor changed.
- **Business rules:** A file with no default set and no `set_id` is refused with `ERR_NOT_FOUND: the file has no default chunk set`; `set_id` with the kind `search_index` is refused with `ERR_INVALID_ARGUMENT: set_id applies to kind 'chunks' only`.
- **Business rules:** A kind that is neither `search_index` nor `chunks` is refused with `ERR_INVALID_ARGUMENT: kind must be 'search_index' or 'chunks'`.
- **Business rules:** Order of the refusals of `fs.recalculate`: project absent or deleted, not a member, kind unknown, path absent, path is a folder; then for the kind `search_index`: `set_id` given, search not enabled, indexing off (SPEC-0100/FR-NEW-009); for the kind `chunks`: set unknown, no default set. The kind `chunks` works when search is not enabled for the server and when the project does not index its files.
- **Business rules:** A `chunks` rebuild of a file with no text to cut is refused with `ERR_NOT_SUPPORTED: no text to cut for <path>`, after the checks set unknown and no default set.
- **Business rules:** A job of the kind `chunks` is per file and kind, whatever the set: a second call supersedes a running `chunks` job of that file (SPEC-0100/FR-NEW-008 applies per kind). It neither cancels nor delays a waiting `search_index` rebuild of the file. A `chunks` job is listed by `fs.list_derived_jobs` like any job.
- **Scenario:** SC-008
- **Priority:** Must

#### FR-NEW-020 [EARS-U]: what text is cut
> The system SHALL cut the text of the file when the file is a text file, and the complete text kept with the conversion of the file, with its image captions, recognized text and tables, when the file is a converted document.

- **Business rules:** A text file is a file whose content is valid UTF-8; Markdown is a text file.
- **Business rules:** A converted document with no conversion yet, and any other file, has no text to cut: the request is refused with `ERR_NOT_SUPPORTED: no text to cut for <path>`.
- **Business rules:** The text of a converted document is the same text `fs.get_document_view` gives for it with image captions and tables in Markdown.
- **Scenario:** SC-004
- **Priority:** Must

#### FR-NEW-021 [EARS-E]: the default set follows the file
> WHEN the rebuild of the search entries of a file runs, or the Markdown companion of a converted document is indexed, THE system SHALL first rebuild every chunk set of that file from its current text, the default set included, and SHALL create the default set when the file has none.

- **Business rules:** The default set uses the strategy `structural` and the server default settings of the moment of the rebuild.
- **Business rules:** A file of a project that does not index its files gets no default set (SPEC-0100/FR-NEW-001); sets made by hand are rebuilt at each rebuild of the file only when the project indexes its files, and otherwise only by hand.
- **Business rules:** Only text files get a default set automatically. At the rebuild of a file that is not valid UTF-8, the sets of a converted document stay `out_of_date` until rebuilt by hand or until the file is deleted, and the sets of any other file are removed; the rebuild ends `done`.
- **Business rules:** A rebuild keeps the `set_id` of each set, replaces its chunks and sets `built_at` to the end of the rebuild.
- **Business rules:** The full rebuild that follows a change of index mode builds the default sets as this requirement says; a mode `none` keeps the existing sets and no longer rebuilds them automatically.
- **Scenario:** SC-001
- **Priority:** Must

#### FR-NEW-022 [EARS-S]: the previous sets are served until the new ones are ready
> WHILE a rebuild of the sets of a file waits or runs THE system SHALL keep serving the previous sets, marked `out_of_date`, and SHALL replace them in one step when the rebuild ends.

- **Business rules:** A failed rebuild keeps the previous sets and the search entries; it adds nothing to the failure rules of SPEC-0100/FR-NEW-012.
- **Business rules:** The sets of a file that became binary are handled as FR-NEW-021 says.
- **Scenario:** SC-001
- **Priority:** Must

#### FR-NEW-023 [EARS-U]: search reads the default set
> The system SHALL build the search entries of a file from the chunks of its default set, one entry per chunk, with the text `<document title> > <heading path titles joined by " > ">` on its own line before the chunk text, and SHALL NOT cut files itself.

- **Business rules:** Without heading path the header line is the title of the document alone. The text of a search result is the chunk text, without the header line.
- **Business rules:** A search result carries the number of the chunk (`chunk_number`) and its heading path (`heading_path`) besides the fields it has today; a result from an entry built before the upgrade carries `chunk_number` 0 and an empty `heading_path`.
- **Business rules:** The ranking, the fusion and the number of results are unchanged (BL-0172 changes them).
- **Scenario:** SC-009
- **Priority:** Must

#### FR-NEW-024 [EARS-E]: indexing by hand uses the default set
> WHEN a member calls `search.index` for a file THE system SHALL index the default set of that file, building it first when the file has none, and answer with the number of chunks indexed.

- **Business rules:** The arguments `chunk_size` and `chunk_overlap` are removed; a call that still passes them is refused with `ERR_INVALID_ARGUMENT: chunk_size and chunk_overlap are no longer accepted`.
- **Business rules:** When the default set is `out_of_date` it is rebuilt first. A single path that is not a text file is refused with `ERR_NOT_SUPPORTED: no text to cut for <path>`; with `recursive`, each text file under a folder is handled as one file and the other files are skipped without error.
- **Scenario:** SC-009
- **Priority:** Must

#### FR-NEW-025 [EARS-E]: sets follow moves, copies and deletions
> WHEN a file is moved, copied or deleted, or its project is deleted, THE system SHALL carry the sets of a moved file to the new path, give the copy only a default set built by its own rebuild, and remove the sets of a deleted file or of a deleted project.

- **Business rules:** A move over an existing file removes the sets of the replaced file first.
- **Business rules:** A folder move, copy or delete applies this to every file under it.
- **Business rules:** A restored project lists no set of its deleted files.
- **Scenario:** SC-007
- **Priority:** Must

#### FR-NEW-026 [EARS-U]: an upgrade converts nothing
> The system SHALL leave the search entries of a file as they were until the first rebuild of that file after the upgrade creates its default set.

- **Business rules:** A file indexed before the upgrade lists no set until then.
- **Business rules:** A rebuild by hand of the kind `search_index` forces the conversion of one file; the kind `chunks` does not create a set.
- **Scenario:** SC-010
- **Priority:** Must

#### FR-NEW-027 [EARS-U]: server defaults for the chunking settings
> The system SHALL read the default settings of the chunk sets from the server configuration keys `search.chunking.fixed_size_tokens`, `search.chunking.fixed_overlap_tokens`, `search.chunking.heading_cut_depth`, `search.chunking.max_tokens` and `search.chunking.list_max_items`.

- **Business rules:** Absent or empty keys take the defaults of FR-NEW-001, FR-NEW-006, FR-NEW-008 and FR-NEW-011.
- **Business rules:** Each value is a whole number in the range of its setting (`fixed_overlap_tokens`: 0 to 8191, and smaller than `fixed_size_tokens`, otherwise the start stops with `ERR_INVALID_ARGUMENT: search.chunking.fixed_overlap_tokens must be smaller than search.chunking.fixed_size_tokens`); a value out of range or not a whole number stops the start with `ERR_INVALID_ARGUMENT: search.chunking.<key> must be a whole number between <min> and <max>`, whether or not search is enabled.
- **Scenario:** SC-010
- **Priority:** Must

#### FR-NEW-028 [EARS-U]: a member of the project only
> The system SHALL allow the chunk tools to the members of the project of the file only.

- **Business rules:** Members of a project can all create, delete and read sets; there is no read only role.
- **Scenario:** SC-003
- **Priority:** Must

#### FR-NEW-029 [EARS-E]: asking for a set the file already holds
> WHEN a member of a project asks for a chunk set that the file already holds with the same requested strategy and the same requested settings THE system SHALL answer with that set and build nothing.

- **Business rules:** The comparison is made on the strategy and settings as requested, after the server defaults filled the omitted ones: a `structural` request that was built as the `fixed` fallback matches the same `structural` request again, not a `fixed` request.
- **Business rules:** A set made by hand with the same requested strategy and settings as the default set is the default set.
- **Scenario:** SC-003
- **Priority:** Must

### 6.2 Modified
#### FR-MOD-001 [EARS-E]: indexing follows chunk sets (supersedes SPEC-0008/FR-002, FR-012, FR-020)
> WHEN the search entries of a file are built THE system SHALL cut them as the default set of the file says, in tokens, and SHALL NOT use character windows.

- **Before:** SPEC-0008/FR-002: `search.index` splits text into overlapping character windows; FR-012: windows of at most `size` characters with a look back of 50 characters; FR-020: auto indexing uses 1000 characters and an overlap of 100.
- **After:** the chunks are those of the default set (structural by default, token sizes, no look back).
- **Reason:** chunks that follow the structure of the document and can be read, listed and navigated.
- **Scenario:** SC-001, SC-009

#### FR-MOD-002 [EARS-E]: the rebuild builds the sets first (modifies SPEC-0100/FR-NEW-011 and FR-NEW-012)
> WHEN a rebuild of the search entries starts THE system SHALL build the chunk sets of the file first (FR-NEW-021).

- **Before:** SPEC-0100/FR-NEW-011 and FR-NEW-012: a rebuild replaces only the search entries of the file and a failed rebuild keeps them.
- **After:** a rebuild builds the sets then the entries; a failed rebuild keeps both the previous sets and the previous entries.
- **Reason:** chunks must exist before they are indexed.
- **Scenario:** SC-001

#### FR-MOD-003 [EARS-E]: `fs.recalculate` accepts the kind `chunks` (modifies SPEC-0100/FR-NEW-008 and FR-NEW-009)
> WHEN a member asks `fs.recalculate` for a kind THE system SHALL accept `search_index` and `chunks`.

- **Before:** SPEC-0100/FR-NEW-009: a kind other than `search_index` is refused with `ERR_INVALID_ARGUMENT: kind must be 'search_index'`.
- **After:** the refusal text for an unknown kind is `ERR_INVALID_ARGUMENT: kind must be 'search_index' or 'chunks'`, and the order of the refusals is that of FR-NEW-019: for the kind `search_index`, `search not enabled` now comes after `path absent` and `path is a folder`.
- **Reason:** one set can be rebuilt by hand.
- **Scenario:** SC-008

### 6.3 Removed
None. (The arguments `chunk_size` and `chunk_overlap` of `search.index` are removed by FR-NEW-024.)

## 7. Non functional requirements (user observable)
### 7.1 Performance
- Listing the chunks of a set of 250 chunks answers in under 2 seconds (E2E-010).
- Cutting is linear in the size of the text: a text of 30 000 tokens is cut in a few seconds (E2E-085).
### 7.2 Security and access
- Every chunk tool needs membership of the project (FR-NEW-017, FR-NEW-028).
### 7.5 Reliability
- A failed rebuild never leaves a mix of old and new chunks (FR-NEW-022).
- The same text and settings always give the same chunks (FR-NEW-010).
### 7.6 Operability
- An operator can see for any file which sets it has, whether they are current and how they were cut (FR-NEW-014).

## 8. Edge cases
| Category | Case | Expected behavior | Test |
|---|---|---|---|
| Empty states | no set; empty file | empty list; set of 0 chunks | E2E-019, E2E-093 |
| Boundary values | window of 0 and 20, 21; 250 chunks in pages; limits 63/64 and 31/32 | exact bounds | E2E-012, E2E-017, E2E-010, E2E-035, E2E-043 |
| Concurrent actions | two members create the same set | one set | E2E-042 |
| Access states | not a member; removed member; absent project | refused | E2E-021, E2E-045, E2E-022 |
| Data variations | CRLF, NUL, no space string, 3000 token paragraph | cut as specified | E2E-082, E2E-083, E2E-084 |
| Partial operations | rebuild fails or is discarded | previous sets kept | E2E-002, E2E-003 |
| Dependency failures | embedding service down | rebuild failed, sets kept | E2E-002, E2E-126 |
| Ordering and timing | save during a rebuild | discarded | E2E-003 |
| Navigation | widen a window by asking again | consistent | E2E-013 |
| Exhaustion | a sixth set | refused | E2E-037 |
| Slow connectivity | not applicable: no interactive flow |  |  |

## 9. Acceptance tests (the contract)
> Behavioral and technology agnostic. /sdlc-design maps each test to a driver. An implementing agent uses this as its definition of done.
> Timing convention: stub services answer at once unless a test says otherwise; a rebuild starts within 2 seconds after the settle delay and ends within 5 seconds after it starts; every stated time is read with a tolerance of 2 seconds. "A text of N tokens" is any text of that size counted with the `cl100k_base` ruler; a text given to a `structural` build that holds no heading line begins with the line `# T`, and that line, its blank line and the chunk that holds only it are not counted in any size, chunk count or `token_count` quoted by a test; "prose" is paragraphs of plain sentences separated by blank lines. Token counts of chunks are read from the chunk's `token_count`.

### 9.1 Summary
| Test | Action | Category | Scenario | Requirements | Priority |
|---|---|---|---|---|---|
| E2E-001 | New | journey | SC-001 | FR-NEW-021, FR-NEW-014, FR-NEW-009, FR-NEW-023 | Critical |
| E2E-002 | New | failure | SC-001 | FR-NEW-022, FR-NEW-021, FR-MOD-002 | High |
| E2E-003 | New | failure | SC-001 | FR-NEW-022, FR-NEW-021 | High |
| E2E-004 | New | failure | SC-001 | FR-NEW-021, FR-NEW-014 | High |
| E2E-005 | New | edge | SC-001 | FR-NEW-020, FR-NEW-021 | High |
| E2E-006 | New | edge | SC-001 | FR-NEW-022, FR-NEW-020 | Medium |
| E2E-007 | New | edge | SC-001 | FR-NEW-021, FR-NEW-027 | Medium |
| E2E-010 | New | journey | SC-002 | FR-NEW-014, FR-NEW-015 | Critical |
| E2E-011 | New | journey | SC-002 | FR-NEW-016, FR-NEW-009 | High |
| E2E-012 | New | edge | SC-002 | FR-NEW-016 | High |
| E2E-013 | New | edge | SC-002 | FR-NEW-016 | Medium |
| E2E-014 | New | edge | SC-002 | FR-NEW-015 | Medium |
| E2E-015 | New | failure | SC-002 | FR-NEW-017, FR-NEW-015 | High |
| E2E-016 | New | failure | SC-002 | FR-NEW-017, FR-NEW-016 | High |
| E2E-017 | New | failure | SC-002 | FR-NEW-017, FR-NEW-016 | High |
| E2E-018 | New | failure | SC-002 | FR-NEW-017, FR-NEW-015 | High |
| E2E-019 | New | edge | SC-002 | FR-NEW-014 | High |
| E2E-020 | New | edge | SC-002 | FR-NEW-014 | Medium |
| E2E-021 | New | failure | SC-002 | FR-NEW-017, FR-NEW-028 | High |
| E2E-022 | New | failure | SC-002 | FR-NEW-017 | High |
| E2E-023 | New | failure | SC-002 | FR-NEW-017 | High |
| E2E-030 | New | journey | SC-003 | FR-NEW-013, FR-NEW-011, FR-NEW-014 | High |
| E2E-031 | New | edge | SC-003 | FR-NEW-013, FR-NEW-029 | High |
| E2E-032 | New | edge | SC-003 | FR-NEW-029 | High |
| E2E-033 | New | failure | SC-003 | FR-NEW-017, FR-NEW-013 | High |
| E2E-034 | New | failure | SC-003 | FR-NEW-017, FR-NEW-013 | High |
| E2E-035 | New | failure | SC-003 | FR-NEW-017, FR-NEW-011 | High |
| E2E-036 | New | failure | SC-003 | FR-NEW-017, FR-NEW-011 | High |
| E2E-037 | New | failure | SC-003 | FR-NEW-017, FR-NEW-013 | High |
| E2E-038 | New | journey | SC-003 | FR-NEW-018 | High |
| E2E-039 | New | failure | SC-003 | FR-NEW-018 | High |
| E2E-040 | New | failure | SC-003 | FR-NEW-018, FR-NEW-017 | High |
| E2E-041 | New | failure | SC-003 | FR-NEW-017, FR-NEW-020 | High |
| E2E-042 | New | edge | SC-003 | FR-NEW-013, FR-NEW-029, FR-NEW-028 | Medium |
| E2E-043 | New | failure | SC-003 | FR-NEW-017, FR-NEW-001, FR-NEW-006, FR-NEW-008 | High |
| E2E-044 | New | edge | SC-003 | FR-NEW-011, FR-NEW-010 | Medium |
| E2E-050 | New | journey | SC-004 | FR-NEW-020, FR-NEW-013, FR-NEW-009 | High |
| E2E-051 | New | failure | SC-004 | FR-NEW-020, FR-NEW-017 | High |
| E2E-052 | New | edge | SC-004 | FR-NEW-014, FR-NEW-022, FR-NEW-020 | Medium |
| E2E-053 | New | edge | SC-004 | FR-NEW-021, FR-NEW-020 | Medium |
| E2E-060 | New | journey | SC-005 | FR-NEW-012, FR-NEW-021 | High |
| E2E-061 | New | edge | SC-005 | FR-NEW-012, FR-NEW-001 | High |
| E2E-062 | New | failure | SC-005 | FR-NEW-012, FR-NEW-013, FR-NEW-029 | High |
| E2E-063 | New | edge | SC-005 | FR-NEW-012, FR-NEW-001 | Medium |
| E2E-070 | New | journey | SC-006 | FR-NEW-001, FR-NEW-002 | High |
| E2E-071 | New | edge | SC-006 | FR-NEW-001, FR-NEW-002 | Medium |
| E2E-072 | New | edge | SC-006 | FR-NEW-006, FR-NEW-002 | High |
| E2E-073 | New | edge | SC-006 | FR-NEW-006 | High |
| E2E-074 | New | edge | SC-006 | FR-NEW-003 | Medium |
| E2E-075 | New | edge | SC-006 | FR-NEW-003 | Medium |
| E2E-076 | New | edge | SC-006 | FR-NEW-003 | Medium |
| E2E-077 | New | edge | SC-006 | FR-NEW-005 | Medium |
| E2E-078 | New | edge | SC-006 | FR-NEW-008 | Medium |
| E2E-079 | New | edge | SC-006 | FR-NEW-007, FR-NEW-005 | Medium |
| E2E-080 | New | failure | SC-006 | FR-NEW-007 | Medium |
| E2E-081 | New | edge | SC-006 | FR-NEW-007 | Medium |
| E2E-082 | New | edge | SC-006 | FR-NEW-007 | Medium |
| E2E-083 | New | failure | SC-006 | FR-NEW-007 | Medium |
| E2E-084 | New | failure | SC-006 | FR-NEW-010 | Medium |
| E2E-085 | New | edge | SC-006 | FR-NEW-010 | Medium |
| E2E-086 | New | edge | SC-006 | FR-NEW-010, FR-NEW-002 | High |
| E2E-087 | New | edge | SC-006 | FR-NEW-004 | Medium |
| E2E-088 | New | edge | SC-006 | FR-NEW-004 | Medium |
| E2E-089 | New | journey | SC-006 | FR-NEW-009 | High |
| E2E-090 | New | edge | SC-006 | FR-NEW-001 | Medium |
| E2E-091 | New | edge | SC-006 | FR-NEW-006, FR-NEW-007 | Medium |
| E2E-092 | New | edge | SC-006 | FR-NEW-008 | Medium |
| E2E-093 | New | edge | SC-006 | FR-NEW-010, FR-NEW-020 | Medium |
| E2E-100 | New | journey | SC-007 | FR-NEW-025 | High |
| E2E-101 | New | journey | SC-007 | FR-NEW-025, FR-NEW-021 | High |
| E2E-102 | New | failure | SC-007 | FR-NEW-025 | High |
| E2E-103 | New | edge | SC-007 | FR-NEW-025 | Medium |
| E2E-104 | New | failure | SC-007 | FR-NEW-025 | High |
| E2E-105 | New | edge | SC-007 | FR-NEW-025 | Medium |
| E2E-110 | New | journey | SC-008 | FR-NEW-019, FR-NEW-023 | Critical |
| E2E-111 | New | journey | SC-008 | FR-NEW-019 | High |
| E2E-112 | New | failure | SC-008 | FR-NEW-019, FR-NEW-017 | High |
| E2E-113 | New | failure | SC-008 | FR-NEW-019 | High |
| E2E-114 | New | failure | SC-008 | FR-NEW-019 | High |
| E2E-115 | New | failure | SC-008 | FR-NEW-019, FR-MOD-003 | High |
| E2E-116 | New | edge | SC-008 | FR-NEW-021, FR-NEW-019, FR-MOD-002 | High |
| E2E-117 | New | edge | SC-008 | FR-NEW-014, FR-NEW-022 | High |
| E2E-120 | New | journey | SC-009 | FR-NEW-023 | Critical |
| E2E-121 | New | edge | SC-009 | FR-NEW-023 | High |
| E2E-122 | New | journey | SC-009 | FR-NEW-024 | High |
| E2E-123 | New | failure | SC-009 | FR-NEW-024 | High |
| E2E-124 | New | edge | SC-009 | FR-NEW-024, FR-NEW-020 | Medium |
| E2E-125 | New | edge | SC-009 | FR-NEW-023 | Medium |
| E2E-126 | New | failure | SC-009 | FR-NEW-024, FR-NEW-022 | High |
| E2E-130 | New | journey | SC-010 | FR-NEW-027, FR-NEW-021 | High |
| E2E-131 | New | failure | SC-010 | FR-NEW-027 | High |
| E2E-132 | New | failure | SC-010 | FR-NEW-027 | High |
| E2E-133 | New | edge | SC-010 | FR-NEW-027 | Medium |
| E2E-134 | New | journey | SC-010 | FR-NEW-026 | High |
| E2E-135 | New | edge | SC-010 | FR-NEW-026, FR-NEW-021 | High |
| E2E-136 | New | edge | SC-010 | FR-NEW-026, FR-NEW-019 | Medium |
| E2E-045 | New | failure | SC-003 | FR-NEW-028, FR-NEW-017 | High |
| E2E-094 | New | edge | SC-006 | FR-NEW-005, FR-NEW-007 | Medium |
| E2E-095 | New | edge | SC-006 | FR-NEW-004 | Medium |
| E2E-138 | New | edge | SC-008 | FR-NEW-019 | Medium |
| E2E-139 | New | failure | SC-003 | FR-NEW-017, FR-NEW-013 | High |
| E2E-140 | New | edge | SC-004 | FR-NEW-021, FR-NEW-022, FR-NEW-020 | Medium |
| E2E-141 | New | edge | SC-010 | FR-NEW-021, FR-NEW-025 | Medium |
| E2E-142 | New | edge | SC-009 | FR-NEW-024, FR-NEW-022 | Medium |
| E2E-143 | New | failure | SC-009 | FR-NEW-024, FR-NEW-020 | High |
| E2E-144 | New | edge | SC-010 | FR-NEW-023, FR-NEW-026 | Medium |
| E2E-145 | New | failure | SC-010 | FR-NEW-027 | High |
| E2E-146 | New | edge | SC-007 | FR-NEW-025 | Medium |
| E2E-MOD-001 | Modified | journey | SC-009 | FR-MOD-001 | High |
| E2E-MOD-002 | Modified | journey | SC-009 | FR-MOD-001 | High |
| E2E-MOD-003 | Modified | failure | SC-008 | FR-MOD-003 | High |

Coverage: happy 17 (journey 17, requirement 0), failure 38, side effect 0, edge 53, state transition 0, security 0 as a category (the access rules are in the failure tests E2E-021 and E2E-045). 108 new tests and 3 modified. Happy to failure ratio 1:2.2.

### 9.2 New tests
#### E2E-001: a saved Markdown file gets its default set
- **Category:** journey. **Scenario:** SC-001. **Requirements:** FR-NEW-021, FR-NEW-014, FR-NEW-009, FR-NEW-023
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/guide.md` is saved with a title, 3 sections under `## ` headings of about 300 tokens each
- **When** Alice saves it and, 15 s later, lists its chunk sets and searches for a word of the second section
- **Then** the list holds one set: strategy `structural`, `default` true, `state` `current`, `chunk_count` 1 (the three sections merge, the total fits 2048 tokens), `token_count` equal to the count of the chunk; the search finds `/guide.md` with `chunk_number` 1
- **And** the settings of the set read `heading_cut_depth` 2, `max_tokens` 2048, `list_max_items` 40
- **Priority:** Critical

#### E2E-002: a failed rebuild keeps the previous sets
- **Category:** failure. **Scenario:** SC-001. **Requirements:** FR-NEW-022, FR-NEW-021, FR-MOD-002
- **Given** project `proj` indexing its files (mode `both`), settle delay 3 s, Alice (`alice@example.com`) a member; `/guide.md` has a current default set of 2 chunks; the embedding service is down
- **When** Alice saves `/guide.md` with new text and waits 15 s
- **Then** the rebuild of the file is `failed` with `embedding service unreachable`; the list of sets shows the same default set, `state` `out_of_date`, still 2 chunks of the old text
- **And** searching a word of the old text still finds `/guide.md`
- **Priority:** High

#### E2E-003: a save during a rebuild discards its sets
- **Category:** failure. **Scenario:** SC-001. **Requirements:** FR-NEW-022, FR-NEW-021
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; a rebuild of `/guide.md` is running (the embedding service answers in 10 s)
- **When** Alice saves `/guide.md` again while it runs
- **Then** when the running rebuild ends its sets are not stored; the list still shows the previous default set `out_of_date` until the next rebuild ends, then `current` with the chunks of the last save
- **And** the chunks of the discarded text never appear in a list
- **Priority:** High

#### E2E-004: a project that does not index has no default set
- **Category:** failure. **Scenario:** SC-001. **Requirements:** FR-NEW-021, FR-NEW-014
- **Given** project `plain` (index mode `none`), Alice a member; `/guide.md` saved
- **When** Alice waits 15 s and lists the sets of `/guide.md`
- **Then** the list is empty: `{sets: []}`
- **And** creating a set by hand (E2E-030) still works in this project
- **Priority:** High

#### E2E-005: a binary file gets no set and no error
- **Category:** edge. **Scenario:** SC-001. **Requirements:** FR-NEW-020, FR-NEW-021
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; a PNG image `/logo.png` of 2 KB
- **When** Alice saves it and waits 15 s, then lists its sets
- **Then** the list is empty and no failed job is listed for it
- **And** the write answered normally
- **Priority:** High

#### E2E-006: a file that stops being text loses its sets
- **Category:** edge. **Scenario:** SC-001. **Requirements:** FR-NEW-022, FR-NEW-020
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/data.md` has a current default set
- **When** Alice overwrites it with the bytes of a PNG image and waits 15 s
- **Then** the list of sets is empty
- **And** the rebuild of the file ended `done`
- **Priority:** Medium

#### E2E-007: the default set is rebuilt with the current server defaults
- **Category:** edge. **Scenario:** SC-001. **Requirements:** FR-NEW-021, FR-NEW-027
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/guide.md` has a default set built with `max_tokens` 2048; the operator restarts the server with `search.chunking.max_tokens` set to 512
- **When** Alice saves `/guide.md` (`# T` then 3000 tokens of prose in 6 paragraphs) and waits 15 s
- **Then** the default set reads `max_tokens` 512 and holds chunks of at most 512 tokens
- **And** an extra set made by hand before the restart keeps its own settings
- **Priority:** Medium

#### E2E-010: listing sets and paging through chunks
- **Category:** journey. **Scenario:** SC-002. **Requirements:** FR-NEW-014, FR-NEW-015
- **Given** project `proj` indexing its files, the server configuration has `search.chunking.max_tokens` 64; `/big.md` has a default set of 250 chunks
- **When** Alice lists the chunks of the set, then asks for the next page with the returned marker, then once more
- **Then** the first answer holds chunks 1 to 100 and a marker, the second 101 to 200 and a marker, the third 201 to 250 and no marker; each chunk shows number, unit, heading path, pages, token count, figures and a preview
- **And** the first answer arrives in under 2 s
- **Priority:** Critical

#### E2E-011: a chunk with its neighbours
- **Category:** journey. **Scenario:** SC-002. **Requirements:** FR-NEW-016, FR-NEW-009
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; the default set of `/big.md` has 20 chunks
- **When** Alice asks chunk 5 of the set with `before` 2 and `after` 1
- **Then** the answer holds chunks 3, 4, 5 and 6 in that order, each with its full text, document title `big`, heading path, unit, pages, token count and figures
- **And** chunk 5 text equals the text of chunk 5 seen alone
- **Priority:** High

#### E2E-012: a window beyond the ends
- **Category:** edge. **Scenario:** SC-002. **Requirements:** FR-NEW-016
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; the default set of `/big.md` has 20 chunks
- **When** Alice asks chunk 1 with `before` 3, then chunk 20 with `after` 2
- **Then** the first answer holds chunk 1 alone, the second chunk 20 alone; no error
- **And** both answers are in order of number
- **Priority:** High

#### E2E-013: widening a window by asking again
- **Category:** edge. **Scenario:** SC-002. **Requirements:** FR-NEW-016
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; the default set of `/big.md` has 20 chunks
- **When** Alice asks chunk 10 with `before` 1 and `after` 1, then chunk 8 with `before` 2 and `after` 0
- **Then** the first answer holds chunks 9 to 11 and the second chunks 6 to 8
- **And** the union of the two answers is chunks 6 to 11 with no chunk changed
- **Priority:** Medium

#### E2E-014: the preview of a short and of a long chunk
- **Category:** edge. **Scenario:** SC-002. **Requirements:** FR-NEW-015
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; a set whose chunk 1 holds 50 characters and chunk 2 holds 3000 characters
- **When** Alice lists the chunks
- **Then** the preview of chunk 1 is its whole text; the preview of chunk 2 is its first 200 characters
- **And** no listed chunk carries a full text field
- **Priority:** Medium

#### E2E-015: an unknown set
- **Category:** failure. **Scenario:** SC-002. **Requirements:** FR-NEW-017, FR-NEW-015
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/big.md` has a default set
- **When** Alice lists the chunks of the set `set-999`
- **Then** the answer is `ERR_NOT_FOUND: chunk set not found: set-999`
- **And** nothing is changed
- **Priority:** High

#### E2E-016: a chunk number that does not exist
- **Category:** failure. **Scenario:** SC-002. **Requirements:** FR-NEW-017, FR-NEW-016
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; the default set of `/big.md` has 20 chunks
- **When** Alice asks chunk 99 of the set, and chunk 0
- **Then** each answer is `ERR_NOT_FOUND: chunk 99 not found in set <id>` and `ERR_NOT_FOUND: chunk 0 not found in set <id>`
- **And** nothing is changed
- **Priority:** High

#### E2E-017: a window above 20
- **Category:** failure. **Scenario:** SC-002. **Requirements:** FR-NEW-017, FR-NEW-016
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; the default set of `/big.md` has 20 chunks
- **When** Alice asks chunk 5 with `before` 21, then with `after` -1
- **Then** each answer is `ERR_INVALID_ARGUMENT: before and after must be between 0 and 20`
- **And** a request with `before` 20 and `after` 20 is accepted
- **Priority:** High

#### E2E-018: an unusable marker
- **Category:** failure. **Scenario:** SC-002. **Requirements:** FR-NEW-017, FR-NEW-015
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; the default set of `/big.md` has 250 chunks
- **When** Alice lists the chunks with the marker `zzz`
- **Then** the answer is `ERR_INVALID_ARGUMENT: marker is not valid`
- **And** a marker from another set is refused the same way
- **Priority:** High

#### E2E-019: a file with no set
- **Category:** edge. **Scenario:** SC-002. **Requirements:** FR-NEW-014
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/new.md` was just created and its rebuild has not run
- **When** Alice lists its sets
- **Then** the answer is `{sets: []}`
- **And** no error and no failed job
- **Priority:** High

#### E2E-020: order of the sets of a file
- **Category:** edge. **Scenario:** SC-002. **Requirements:** FR-NEW-014
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/big.md` has its default set built at 10:00:00 and two extra sets built at 10:05:00 and 10:02:00
- **When** Alice lists the sets
- **Then** the order is the default set, the set of 10:02:00, the set of 10:05:00
- **And** ties of the time are ordered by set id
- **Priority:** Medium

#### E2E-021: a person who is not a member
- **Category:** failure. **Scenario:** SC-002. **Requirements:** FR-NEW-017, FR-NEW-028
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; Dave (`dave@example.com`) is not a member
- **When** Dave lists the sets, lists the chunks, reads a chunk, creates a set and deletes a set of `/big.md`
- **Then** each answer is `ERR_FORBIDDEN: 'dave@example.com' is not a member of 'proj'`
- **And** no set was created or removed
- **Priority:** High

#### E2E-022: a project that does not exist
- **Category:** failure. **Scenario:** SC-002. **Requirements:** FR-NEW-017
- **Given** no project `ghost`
- **When** Alice lists the sets of `/a.md` in `ghost`
- **Then** the answer is `ERR_PROJECT_NOT_FOUND`
- **And** nothing is changed
- **Priority:** High

#### E2E-023: an unknown path and a folder
- **Category:** failure. **Scenario:** SC-002. **Requirements:** FR-NEW-017
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/docs` is a folder, `/nope.md` does not exist
- **When** Alice lists the sets of `/nope.md` and of `/docs`
- **Then** the answers are `ERR_NOT_FOUND: file not found: /nope.md` and `ERR_INVALID_ARGUMENT: path must be a file`
- **And** the order of the checks is path absent, then folder
- **Priority:** High

#### E2E-030: a fixed set with overlap by hand
- **Category:** journey. **Scenario:** SC-003. **Requirements:** FR-NEW-013, FR-NEW-011, FR-NEW-014
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/essay.md` holds 3000 tokens of prose
- **When** Alice creates a set with strategy `fixed` and settings `size_tokens` 1000 and `overlap_tokens` 200
- **Then** the answer is a set with `default` false, strategy `fixed`, `chunk_count` 4 (chunks start at tokens 0, 800, 1600 and 2400), each chunk of at most 1000 tokens, empty heading path, unknown pages
- **And** the list of sets now holds the default set and this set
- **Priority:** High

#### E2E-031: creating the same set twice
- **Category:** edge. **Scenario:** SC-003. **Requirements:** FR-NEW-013, FR-NEW-029
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/essay.md` has no extra set
- **When** Alice creates a `fixed` set with `size_tokens` 500 twice
- **Then** both answers carry the same set id
- **And** the list holds one extra set
- **Priority:** High

#### E2E-032: a set equal to the default set
- **Category:** edge. **Scenario:** SC-003. **Requirements:** FR-NEW-029
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/essay.md` has its default set (`structural`, server defaults)
- **When** Alice creates a `structural` set with empty settings
- **Then** the answer is the default set, `default` true
- **And** no new set exists
- **Priority:** High

#### E2E-033: an unknown strategy
- **Category:** failure. **Scenario:** SC-003. **Requirements:** FR-NEW-017, FR-NEW-013
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/essay.md`
- **When** Alice creates a set with strategy `sentence`
- **Then** the answer is `ERR_INVALID_ARGUMENT: strategy must be 'fixed' or 'structural'`
- **And** no set was created
- **Priority:** High

#### E2E-034: a setting that does not apply
- **Category:** failure. **Scenario:** SC-003. **Requirements:** FR-NEW-017, FR-NEW-013
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/essay.md`
- **When** Alice creates a `structural` set with `overlap_tokens` 50, then a `fixed` set with `heading_cut_depth` 3
- **Then** the answers are `ERR_INVALID_ARGUMENT: setting 'overlap_tokens' does not apply to strategy 'structural'` and `ERR_INVALID_ARGUMENT: setting 'heading_cut_depth' does not apply to strategy 'fixed'`
- **And** no set was created
- **Priority:** High

#### E2E-035: a setting out of range
- **Category:** failure. **Scenario:** SC-003. **Requirements:** FR-NEW-017, FR-NEW-011
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/essay.md`
- **When** Alice creates `fixed` sets with `size_tokens` 31 and 8193
- **Then** each answer is `ERR_INVALID_ARGUMENT: size_tokens must be between 32 and 8192`
- **And** sets with `size_tokens` 32 and 8192 are accepted
- **Priority:** High

#### E2E-036: an overlap not below the size
- **Category:** failure. **Scenario:** SC-003. **Requirements:** FR-NEW-017, FR-NEW-011
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/essay.md`
- **When** Alice creates a `fixed` set with `size_tokens` 100 and `overlap_tokens` 100
- **Then** the answer is `ERR_INVALID_ARGUMENT: overlap_tokens must be smaller than size_tokens`
- **And** a set with `overlap_tokens` 99 is accepted
- **Priority:** High

#### E2E-037: a sixth set
- **Category:** failure. **Scenario:** SC-003. **Requirements:** FR-NEW-017, FR-NEW-013
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/essay.md` holds its default set and 4 extra sets
- **When** Alice creates a fifth extra set, then deletes one extra set and creates a different one
- **Then** the first creation answers `ERR_INVALID_ARGUMENT: a file holds at most 5 chunk sets`; after the deletion the creation succeeds
- **And** the list never holds more than 5 sets
- **Priority:** High

#### E2E-038: deleting an extra set
- **Category:** journey. **Scenario:** SC-003. **Requirements:** FR-NEW-018
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/essay.md` has its default set and an extra `fixed` set
- **When** Alice deletes the extra set
- **Then** the answer is `{deleted: true}`; the list holds the default set only; listing the chunks of the deleted set answers `ERR_NOT_FOUND: chunk set not found: <id>`
- **And** the default set is unchanged
- **Priority:** High

#### E2E-039: deleting the default set
- **Category:** failure. **Scenario:** SC-003. **Requirements:** FR-NEW-018
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/essay.md` has its default set
- **When** Alice deletes the default set
- **Then** the answer is `ERR_INVALID_ARGUMENT: the default chunk set cannot be deleted`
- **And** the set is still listed
- **Priority:** High

#### E2E-040: deleting an unknown set
- **Category:** failure. **Scenario:** SC-003. **Requirements:** FR-NEW-018, FR-NEW-017
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/essay.md`
- **When** Alice deletes the set `set-999`
- **Then** the answer is `ERR_NOT_FOUND: chunk set not found: set-999`
- **And** nothing is changed
- **Priority:** High

#### E2E-041: no text to cut
- **Category:** failure. **Scenario:** SC-003. **Requirements:** FR-NEW-017, FR-NEW-020
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/logo.png` is a PNG image
- **When** Alice creates a `fixed` set for it
- **Then** the answer is `ERR_NOT_SUPPORTED: no text to cut for /logo.png`
- **And** no set was created
- **Priority:** High

#### E2E-042: two members create the same set together
- **Category:** edge. **Scenario:** SC-003. **Requirements:** FR-NEW-013, FR-NEW-029, FR-NEW-028
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; Carol (`carol@example.com`) is a member too
- **When** Alice and Carol create a `fixed` set with `size_tokens` 500 within the same second
- **Then** both answers carry the same set id
- **And** the list holds one extra set
- **Priority:** Medium

#### E2E-043: structural settings out of range
- **Category:** failure. **Scenario:** SC-003. **Requirements:** FR-NEW-017, FR-NEW-001, FR-NEW-006, FR-NEW-008
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/essay.md`
- **When** Alice creates `structural` sets with `heading_cut_depth` 7, `max_tokens` 63 and `list_max_items` 1
- **Then** the answers are `ERR_INVALID_ARGUMENT: heading_cut_depth must be between 1 and 6`, `ERR_INVALID_ARGUMENT: max_tokens must be between 64 and 8192` and `ERR_INVALID_ARGUMENT: list_max_items must be between 2 and 200`
- **And** no set was created; a set with `max_tokens` 64 is accepted
- **Priority:** High

#### E2E-044: fixed chunks with and without overlap
- **Category:** edge. **Scenario:** SC-003. **Requirements:** FR-NEW-011, FR-NEW-010
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/essay.md` holds 10000 tokens of prose
- **When** Alice creates `fixed` sets with `size_tokens` 2048 and `overlap_tokens` 0 (default), then `size_tokens` 1000 and `overlap_tokens` 200
- **Then** the first set has 5 chunks, the last holding 1808 tokens; the second has 13 chunks, chunk 2 starting 800 tokens after chunk 1
- **And** every chunk of both sets is at most its `size_tokens`
- **Priority:** Medium

#### E2E-050: a converted document is chunked from its metadata text
- **Category:** journey. **Scenario:** SC-004. **Requirements:** FR-NEW-020, FR-NEW-013, FR-NEW-009
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/report.pdf` was converted: body of 3 pages, 2 images with captions `Figure 1: sales` and `Figure 2: costs`, one recognized text line `Total 42`, one table of 4 rows
- **When** Alice creates a `structural` set for `/report.pdf` and lists its chunks and reads chunk 1
- **Then** the set is built; the chunks together hold the body text, the captions, the recognized text and the table rows; document title `report`; no chunk holds image bytes
- **And** the text is the text of the document view with captions and tables in Markdown
- **Priority:** High

#### E2E-051: a document not converted yet
- **Category:** failure. **Scenario:** SC-004. **Requirements:** FR-NEW-020, FR-NEW-017
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/raw.pdf` was uploaded and never converted
- **When** Alice creates a set for it
- **Then** the answer is `ERR_NOT_SUPPORTED: no text to cut for /raw.pdf`
- **And** no set was created
- **Priority:** High

#### E2E-052: a converted document that changed
- **Category:** edge. **Scenario:** SC-004. **Requirements:** FR-NEW-014, FR-NEW-022, FR-NEW-020
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/report.pdf` has an extra set; the document is replaced by a new version and converted again
- **When** Alice lists its sets before and after she recalculates the set by hand
- **Then** the set is `out_of_date` before and `current` after, with the chunks of the new text
- **And** the list never shows chunks of the new text under a set marked `current` before the rebuild
- **Priority:** Medium

#### E2E-053: no automatic default set for a converted document
- **Category:** edge. **Scenario:** SC-004. **Requirements:** FR-NEW-021, FR-NEW-020
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/report.pdf` converted
- **When** Alice waits 15 s and lists the sets
- **Then** the list is empty (its sets are made by hand)
- **And** its Markdown companion `/report.md`, a text file, gets a default set
- **Priority:** Medium

#### E2E-060: a text without headings falls back to fixed
- **Category:** journey. **Scenario:** SC-005. **Requirements:** FR-NEW-012, FR-NEW-021
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/notes.txt` holds 3000 tokens of prose with no heading line
- **When** Alice saves it, waits 15 s and lists its sets
- **Then** the default set shows strategy `fixed`, `fallback_from` `structural`, settings `size_tokens` 2048 and `overlap_tokens` 0, 2 chunks
- **And** the second chunk holds the remaining 952 tokens
- **Priority:** High

#### E2E-061: adding a heading ends the fallback
- **Category:** edge. **Scenario:** SC-005. **Requirements:** FR-NEW-012, FR-NEW-001
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; the default set of `/notes.txt` is the fixed fallback
- **When** Alice inserts the line `# Notes` at the top and waits 15 s
- **Then** the default set shows strategy `structural` and no `fallback_from`
- **And** chunk 1 starts with the line `# Notes`
- **Priority:** High

#### E2E-062: a structural set by hand on a text without headings
- **Category:** failure. **Scenario:** SC-005. **Requirements:** FR-NEW-012, FR-NEW-013, FR-NEW-029
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/notes.txt` has no heading
- **When** Alice creates a `structural` set with `max_tokens` 1000
- **Then** the answer is a set with strategy `fixed`, `fallback_from` `structural`, `size_tokens` 2048 (the server default fixed settings, not `max_tokens`)
- **And** creating the same `structural` request again returns the same set; creating a `fixed` set with `size_tokens` 2048 makes another set; after Alice adds a heading and recalculates the set, it is `structural` with `max_tokens` 1000
- **Priority:** High

#### E2E-063: a heading inside a code block is not a heading
- **Category:** edge. **Scenario:** SC-005. **Requirements:** FR-NEW-012, FR-NEW-001
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/snip.md` holds a fenced code block whose first line is `# not a heading` and no other heading
- **When** Alice creates a `structural` set
- **Then** the set is the `fixed` fallback
- **And** adding a real heading outside the code makes the next rebuild `structural`
- **Priority:** Medium

#### E2E-070: sections follow the headings
- **Category:** journey. **Scenario:** SC-006. **Requirements:** FR-NEW-001, FR-NEW-002
- **Given** text: `intro` of 100 tokens, `# A`, text a of 1500 tokens, `## B`, text b of 500 tokens, `### C`, text c of 500 tokens
- **When** Alice builds a `structural` set from the text
- **Then** three chunks: the intro with an empty heading path; the section A with path `A`; the section B with path `A > B` holding `## B`, text b, `### C` and text c (C does not cut); the sections A and B do not merge (their total is above 2048 tokens)
- **And** the heading lines stay in the text of the chunks
- **Priority:** High

#### E2E-071: a document starting at level 3
- **Category:** edge. **Scenario:** SC-006. **Requirements:** FR-NEW-001, FR-NEW-002
- **Given** text: `### X`, text x, `### Y`, text y, each text of 1500 tokens
- **When** Alice builds a `structural` set
- **Then** the chunks are `X` and `Y` with heading paths `X` and `Y`
- **And** the cut counts open headings, not `#` marks
- **Priority:** Medium

#### E2E-072: two short sections under one heading merge
- **Category:** edge. **Scenario:** SC-006. **Requirements:** FR-NEW-006, FR-NEW-002
- **Given** text: `# A`, 100 tokens, `## B`, 100 tokens, `## C`, 100 tokens
- **When** Alice builds a `structural` set
- **Then** one chunk, heading path `A`, holding the text of A and the sections B and C (the units `A`, `A > B` and `A > C` are under the same parent)
- **And** the chunk holds the three heading lines and the common heading path is `A`
- **Priority:** High

#### E2E-073: two short sections under different headings do not merge
- **Category:** edge. **Scenario:** SC-006. **Requirements:** FR-NEW-006
- **Given** text: `# A`, 100 tokens, `# B`, 100 tokens
- **When** Alice builds a `structural` set
- **Then** two chunks, heading paths `A` and `B`
- **And** chunk numbers 1 and 2
- **Priority:** High

#### E2E-074: a page marker records the page and cuts nothing
- **Category:** edge. **Scenario:** SC-006. **Requirements:** FR-NEW-003
- **Given** text: paragraph one, a line `---`, a line `*[Page 3]*`, paragraph two
- **When** Alice builds a `structural` set
- **Then** one chunk; its first and last page are both 3; no marker line in its text
- **And** a text with no page marker gives chunks with unknown pages
- **Priority:** Medium

#### E2E-075: a thematic break is ignored
- **Category:** edge. **Scenario:** SC-006. **Requirements:** FR-NEW-003
- **Given** text: paragraph one, a line `---` alone, paragraph two
- **When** Alice builds a `structural` set
- **Then** one chunk whose text holds both paragraphs and no `---` line
- **And** pages unknown
- **Priority:** Medium

#### E2E-076: the metadata table is left out
- **Category:** edge. **Scenario:** SC-006. **Requirements:** FR-NEW-003
- **Given** text: a first table whose header cells are all empty and one row `| Author | Smith |`, then `# A` and text
- **When** Alice builds a `structural` set
- **Then** no chunk holds `Author` or `Smith`
- **And** the chunk holds `# A` and its text
- **Priority:** Medium

#### E2E-077: a list introduction travels with its list
- **Category:** edge. **Scenario:** SC-006. **Requirements:** FR-NEW-005
- **Given** text: a paragraph ending with `Causes:` then a list of 3 items
- **When** Alice builds a `structural` set
- **Then** the chunk that holds the list holds the line `Causes:` immediately before the first item, and `Causes:` occurs once in the whole set
- **And** the list items are intact
- **Priority:** Medium

#### E2E-078: a list of 95 items
- **Category:** edge. **Scenario:** SC-006. **Requirements:** FR-NEW-008
- **Given** text: `# L` and a list of 95 items of 10 tokens each
- **When** Alice builds a `structural` set
- **Then** four chunks: `# L`, then three list chunks of 40, 40 and 15 items, none merged with a neighbour
- **And** each item appears once
- **Priority:** Medium

#### E2E-079: a table of 5000 tokens
- **Category:** edge. **Scenario:** SC-006. **Requirements:** FR-NEW-007, FR-NEW-005
- **Given** text: a table of 200 rows, 5000 tokens in all
- **When** Alice builds a `structural` set
- **Then** row groups of at most 2048 tokens, each starting with the header row and the separator row; every row appears once
- **And** all pieces share the unit number
- **Priority:** Medium

#### E2E-080: a code block never closed
- **Category:** failure. **Scenario:** SC-006. **Requirements:** FR-NEW-007
- **Given** text: a line of three backticks, then 3000 tokens of code lines, no closing fence
- **When** Alice builds a `structural` set
- **Then** pieces of at most 2048 tokens, each starting with the opening fence line and ending with a closing fence
- **And** the code lines joined give back the original lines
- **Priority:** Medium

#### E2E-081: prose above the limit
- **Category:** edge. **Scenario:** SC-006. **Requirements:** FR-NEW-007
- **Given** text: 5000 tokens of paragraphs separated by blank lines
- **When** Alice builds a `structural` set
- **Then** pieces of at most 2048 tokens cut at blank lines, no repeated text
- **And** the pieces joined give back the text
- **Priority:** Medium

#### E2E-082: one paragraph with no line break
- **Category:** edge. **Scenario:** SC-006. **Requirements:** FR-NEW-007
- **Given** text: one paragraph of 3000 tokens made of words, no line break
- **When** Alice builds a `structural` set
- **Then** two pieces cut between words
- **And** the pieces joined give back the paragraph
- **Priority:** Medium

#### E2E-083: one string with no space
- **Category:** failure. **Scenario:** SC-006. **Requirements:** FR-NEW-007
- **Given** text: one string of 3000 tokens with no space and no line break
- **When** Alice builds a `structural` set
- **Then** two pieces cut by token count
- **And** the pieces joined give back the string
- **Priority:** Medium

#### E2E-084: carriage returns and NUL characters
- **Category:** failure. **Scenario:** SC-006. **Requirements:** FR-NEW-010
- **Given** two texts: the same text once with `\r\n` line ends and NUL characters, once cleaned
- **When** Alice builds a `structural` set from each
- **Then** the chunks are identical character for character
- **And** no NUL character in any chunk
- **Priority:** Medium

#### E2E-085: the same text twice
- **Category:** edge. **Scenario:** SC-006. **Requirements:** FR-NEW-010
- **Given** a text of 30 000 tokens with headings, lists and tables
- **When** Alice builds the set twice
- **Then** the chunks are identical character for character, number for number
- **And** unit numbers and token counts are identical
- **Priority:** Medium

#### E2E-086: no chunk above the limit and at most 2 titles
- **Category:** edge. **Scenario:** SC-006. **Requirements:** FR-NEW-010, FR-NEW-002
- **Given** a text of 30 000 tokens with headings of level 1 to 4
- **When** Alice builds a `structural` set
- **Then** every chunk has at most 2048 tokens and a heading path of at most 2 titles
- **And** no chunk repeats text of another
- **Priority:** High

#### E2E-087: a figure follows the unit before it
- **Category:** edge. **Scenario:** SC-006. **Requirements:** FR-NEW-004
- **Given** text: `# A`, paragraph one, a line `![chart](chart.png)`, `# B`, paragraph two
- **When** Alice builds a `structural` set
- **Then** chunk of A carries the figure reference `chart.png`; the chunk of B carries none
- **And** no figure text appears in any chunk
- **Priority:** Medium

#### E2E-088: a figure before every unit
- **Category:** edge. **Scenario:** SC-006. **Requirements:** FR-NEW-004
- **Given** text: a line `[Figure]`, a line `![logo](logo.png)`, then paragraph one
- **When** Alice builds a `structural` set
- **Then** chunk 1 carries the figure reference `logo.png`
- **And** one figure reference, not two
- **Priority:** Medium

#### E2E-089: what a chunk holds
- **Category:** journey. **Scenario:** SC-006. **Requirements:** FR-NEW-009
- **Given** file `/guide.md` with `# A`, text, a line `---`, a line `*[Page 2]*`, `# B`, text
- **When** Alice builds a `structural` set and reads chunk 2
- **Then** the chunk shows `document_title` `guide`, `heading_path` [`B`], `number` 2, `unit` 2, `page_start` 2, `page_end` 2, its `token_count`, `figures` empty
- **And** the unit numbers rise with the text
- **Priority:** High

#### E2E-090: heading_cut_depth 1
- **Category:** edge. **Scenario:** SC-006. **Requirements:** FR-NEW-001
- **Given** text: `# A`, text, `## B`, text, `# C`, text, each 1500 tokens
- **When** Alice builds a `structural` set with `heading_cut_depth` 1
- **Then** no chunk has the path `A > B`; the chunks holding `# A`, text a, `## B` and text b all have the path `A` and each has at most 2048 tokens; one chunk has the path `C`
- **And** with depth 2 a chunk has the path `A > B`
- **Priority:** Medium

#### E2E-091: max_tokens 64
- **Category:** edge. **Scenario:** SC-006. **Requirements:** FR-NEW-006, FR-NEW-007
- **Given** text: `# A` and 300 tokens of paragraphs of 50 tokens
- **When** Alice builds a `structural` set with `max_tokens` 64
- **Then** chunks of at most 64 tokens, one paragraph each, no paragraph cut
- **And** no chunk above 64 tokens
- **Priority:** Medium

#### E2E-092: list_max_items 2
- **Category:** edge. **Scenario:** SC-006. **Requirements:** FR-NEW-008
- **Given** text: a list of 5 items
- **When** Alice builds a `structural` set with `list_max_items` 2
- **Then** three chunks of 2, 2 and 1 items
- **And** the introduction, if any, only on the first
- **Priority:** Medium

#### E2E-093: an empty file
- **Category:** edge. **Scenario:** SC-006. **Requirements:** FR-NEW-010, FR-NEW-020
- **Given** an empty text file `/empty.md`
- **When** Alice creates a `structural` set
- **Then** a set with strategy `fixed`, `fallback_from` `structural`, `chunk_count` 0, `token_count` 0
- **And** listing its chunks answers an empty list and no marker
- **Priority:** Medium

#### E2E-100: a move carries the sets
- **Category:** journey. **Scenario:** SC-007. **Requirements:** FR-NEW-025
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/a.md` has its default set and an extra `fixed` set
- **When** Alice moves `/a.md` to `/b.md`
- **Then** listing the sets of `/b.md` shows the same two set ids with the same chunks; `/a.md` is refused as unknown
- **And** no rebuild is needed for the sets to be readable
- **Priority:** High

#### E2E-101: a copy gets its own default set only
- **Category:** journey. **Scenario:** SC-007. **Requirements:** FR-NEW-025, FR-NEW-021
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/a.md` has its default set and an extra `fixed` set
- **When** Alice copies `/a.md` to `/c.md` and waits 15 s
- **Then** `/c.md` lists one set, its default set; `/a.md` keeps its two sets
- **And** the extra set was not copied
- **Priority:** High

#### E2E-102: a delete removes the sets
- **Category:** failure. **Scenario:** SC-007. **Requirements:** FR-NEW-025
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/a.md` has two sets
- **When** Alice deletes `/a.md`
- **Then** listing its sets answers `ERR_NOT_FOUND: file not found: /a.md`; writing a new `/a.md` and listing gives an empty list until its rebuild
- **And** the new file gets no set of the old one
- **Priority:** High

#### E2E-103: a move over an existing file
- **Category:** edge. **Scenario:** SC-007. **Requirements:** FR-NEW-025
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/a.md` and `/b.md` each have a default set
- **When** Alice moves `/a.md` over `/b.md`
- **Then** `/b.md` lists the sets of the moved file; the sets of the replaced file are gone
- **And** `/a.md` is unknown
- **Priority:** Medium

#### E2E-104: a project deleted then restored
- **Category:** failure. **Scenario:** SC-007. **Requirements:** FR-NEW-025
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/a.md` has a default set
- **When** the platform admin deletes `proj`, then restores it
- **Then** while deleted, listing sets answers `ERR_PROJECT_NOT_FOUND`; after the restore `/a.md` lists no set until its next rebuild
- **And** no set of the deleted files reappears
- **Priority:** High

#### E2E-105: a folder move
- **Category:** edge. **Scenario:** SC-007. **Requirements:** FR-NEW-025
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/docs/a.md` and `/docs/b.md` each have a default set
- **When** Alice moves `/docs` to `/manual`
- **Then** `/manual/a.md` and `/manual/b.md` list their sets
- **And** nothing under `/docs` lists a set
- **Priority:** Medium

#### E2E-110: recalculating the default set by hand
- **Category:** journey. **Scenario:** SC-008. **Requirements:** FR-NEW-019, FR-NEW-023
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 30 s; `/guide.md` has a default set; Alice saved new text 5 s ago (a rebuild waits)
- **When** Alice calls `fs.recalculate` with kind `chunks` on `/guide.md`, then lists its sets and searches a word of the new text
- **Then** the answer names a job of kind `chunks` (state `running` or `done`); within 7 s the default set is `current` with the new chunks and the search finds the new text
- **And** the waiting `search_index` rebuild of the file is still listed with its time left
- **Priority:** Critical

#### E2E-111: recalculating an extra set
- **Category:** journey. **Scenario:** SC-008. **Requirements:** FR-NEW-019
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/guide.md` has an extra `fixed` set built from older text
- **When** Alice calls `fs.recalculate` with kind `chunks` and that `set_id`
- **Then** within 7 s that set is `current` with the chunks of the current text; the default set is untouched
- **And** the search entries are not rebuilt
- **Priority:** High

#### E2E-112: recalculating an unknown set
- **Category:** failure. **Scenario:** SC-008. **Requirements:** FR-NEW-019, FR-NEW-017
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/guide.md`
- **When** Alice calls `fs.recalculate` with kind `chunks` and `set_id` `set-999`
- **Then** the answer is `ERR_NOT_FOUND: chunk set not found: set-999`
- **And** no job started
- **Priority:** High

#### E2E-113: recalculating when there is no default set
- **Category:** failure. **Scenario:** SC-008. **Requirements:** FR-NEW-019
- **Given** project `plain` (mode `none`); `/guide.md` with no set
- **When** Alice calls `fs.recalculate` with kind `chunks` and no `set_id`
- **Then** the answer is `ERR_NOT_FOUND: the file has no default chunk set`
- **And** no job started
- **Priority:** High

#### E2E-114: set_id with the wrong kind
- **Category:** failure. **Scenario:** SC-008. **Requirements:** FR-NEW-019
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/guide.md` has a default set
- **When** Alice calls `fs.recalculate` with kind `search_index` and a `set_id`
- **Then** the answer is `ERR_INVALID_ARGUMENT: set_id applies to kind 'chunks' only`
- **And** no job started
- **Priority:** High

#### E2E-115: an unknown kind
- **Category:** failure. **Scenario:** SC-008. **Requirements:** FR-NEW-019, FR-MOD-003
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/guide.md`
- **When** Alice calls `fs.recalculate` with kind `concepts`
- **Then** the answer is `ERR_INVALID_ARGUMENT: kind must be 'search_index' or 'chunks'`
- **And** no job started
- **Priority:** High

#### E2E-116: a rebuild by hand of the entries builds the sets first
- **Category:** edge. **Scenario:** SC-008. **Requirements:** FR-NEW-021, FR-NEW-019, FR-MOD-002
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/guide.md` was indexed before the upgrade and has no set
- **When** Alice calls `fs.recalculate` with kind `search_index`
- **Then** within 7 s the file lists a default set and the search entries come from it
- **And** the search results carry `chunk_number`
- **Priority:** High

#### E2E-117: out of date while a rebuild waits
- **Category:** edge. **Scenario:** SC-008. **Requirements:** FR-NEW-014, FR-NEW-022
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/guide.md` has a current default set
- **When** Alice saves new text and lists the sets 1 s later
- **Then** the default set is `out_of_date`; after the rebuild ends it is `current`
- **And** the chunks listed before the end are the old ones
- **Priority:** High

#### E2E-120: search results carry the chunk and its heading path
- **Category:** journey. **Scenario:** SC-009. **Requirements:** FR-NEW-023
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/guide.md` with `# Setup`, text with the word `kubelet`, `# Usage`, text; its default set is current
- **When** Alice searches `kubelet`
- **Then** the first result is `/guide.md` with `chunk_number` 1 and `heading_path` [`Setup`], and the existing fields
- **And** a search for a word only in the title (`guide`) also finds the chunk
- **Priority:** Critical

#### E2E-121: the heading is part of the indexed text
- **Category:** edge. **Scenario:** SC-009. **Requirements:** FR-NEW-023
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/guide.md` with `# Setup` and text that never uses the word `setup` in its body
- **When** Alice searches `setup`
- **Then** the chunk is found
- **And** the text shown in the result is the chunk text without the header line
- **Priority:** High

#### E2E-122: indexing by hand uses the default set
- **Category:** journey. **Scenario:** SC-009. **Requirements:** FR-NEW-024
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/guide.md` has no set yet
- **When** Alice calls `search.index` for `/guide.md`
- **Then** the answer reports the number of chunks of the default set, which now exists
- **And** the search finds the file
- **Priority:** High

#### E2E-123: the removed arguments
- **Category:** failure. **Scenario:** SC-009. **Requirements:** FR-NEW-024
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/guide.md`
- **When** Alice calls `search.index` with `chunk_size` 500
- **Then** the answer is `ERR_INVALID_ARGUMENT: chunk_size and chunk_overlap are no longer accepted`
- **And** nothing was indexed
- **Priority:** High

#### E2E-124: indexing a folder by hand
- **Category:** edge. **Scenario:** SC-009. **Requirements:** FR-NEW-024, FR-NEW-020
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/docs` holds `a.md`, `b.txt` and `c.png`
- **When** Alice calls `search.index` on `/docs` with `recursive` true
- **Then** `a.md` and `b.txt` get their default sets and entries; `c.png` is skipped with no error
- **And** the answer counts the chunks of the two sets
- **Priority:** Medium

#### E2E-125: a chunk without heading path
- **Category:** edge. **Scenario:** SC-009. **Requirements:** FR-NEW-023
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/plain.md` with no heading is a fallback set
- **When** Alice searches a word of the text
- **Then** the result's `heading_path` is empty
- **And** the indexed header line is the document title alone
- **Priority:** Medium

#### E2E-126: embedding down for indexing by hand
- **Category:** failure. **Scenario:** SC-009. **Requirements:** FR-NEW-024, FR-NEW-022
- **Given** project `proj` indexing its files (mode `both`), Alice a member; `/guide.md` is indexed; the embedding service is down
- **When** Alice calls `search.index` for `/guide.md`
- **Then** the answer is an error `embedding service unreachable`
- **And** the previous entries and the previous default set are unchanged
- **Priority:** High

#### E2E-130: server defaults are used
- **Category:** journey. **Scenario:** SC-010. **Requirements:** FR-NEW-027, FR-NEW-021
- **Given** the server configuration has `search.chunking.max_tokens` 1024 and `search.chunking.list_max_items` 20; project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member
- **When** Alice saves `/guide.md` (it begins with `# T`) and lists its sets
- **Then** the default set shows `max_tokens` 1024 and `list_max_items` 20
- **And** a set made by hand with empty settings gets the same values
- **Priority:** High

#### E2E-131: a default out of range
- **Category:** failure. **Scenario:** SC-010. **Requirements:** FR-NEW-027
- **Given** the server configuration has `search.chunking.heading_cut_depth` 9
- **When** the operator starts the server
- **Then** the start stops with `ERR_INVALID_ARGUMENT: search.chunking.heading_cut_depth must be a whole number between 1 and 6`
- **And** with 6 the server starts
- **Priority:** High

#### E2E-132: a default that is not a whole number
- **Category:** failure. **Scenario:** SC-010. **Requirements:** FR-NEW-027
- **Given** the server configuration has `search.chunking.max_tokens` `abc`, then `2.5`
- **When** the operator starts the server each time
- **Then** each start stops with `ERR_INVALID_ARGUMENT: search.chunking.max_tokens must be a whole number between 64 and 8192`
- **And** search is disabled in this configuration and the start still stops
- **Priority:** High

#### E2E-133: absent or empty defaults
- **Category:** edge. **Scenario:** SC-010. **Requirements:** FR-NEW-027
- **Given** a configuration with no `search.chunking` keys, then with empty values
- **When** the operator starts the server and Alice saves a Markdown file that begins with `# T`
- **Then** the server starts and the default set uses 2048, 2, 40
- **And** `fixed_size_tokens` 2048 and `fixed_overlap_tokens` 0 apply to a fallback
- **Priority:** Medium

#### E2E-134: nothing converts at upgrade
- **Category:** journey. **Scenario:** SC-010. **Requirements:** FR-NEW-026
- **Given** a file `/old.md` indexed before the upgrade with character chunks; the upgraded server starts
- **When** Alice lists its sets and searches a word of it
- **Then** the list is empty and the search still finds `/old.md`
- **And** no rebuild job is listed
- **Priority:** High

#### E2E-135: the first rebuild converts the file
- **Category:** edge. **Scenario:** SC-010. **Requirements:** FR-NEW-026, FR-NEW-021
- **Given** same as E2E-134
- **When** Alice saves `/old.md` and waits 15 s
- **Then** the file lists its default set and the search results carry `chunk_number`
- **And** the character chunks are gone from the results
- **Priority:** High

#### E2E-136: a rebuild by hand converts one file only
- **Category:** edge. **Scenario:** SC-010. **Requirements:** FR-NEW-026, FR-NEW-019
- **Given** two files `/old1.md` and `/old2.md` indexed before the upgrade
- **When** Alice calls `fs.recalculate` with kind `search_index` on `/old1.md`
- **Then** `/old1.md` lists a default set; `/old2.md` lists none
- **And** the search still finds both files
- **Priority:** Medium

#### E2E-045: a removed member loses access
- **Category:** failure. **Scenario:** SC-003. **Requirements:** FR-NEW-028, FR-NEW-017
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; Carol was a member and the owner removed her
- **When** Carol lists the sets, creates a set and deletes a set of `/essay.md`
- **Then** each answer is `ERR_FORBIDDEN: 'carol@example.com' is not a member of 'proj'`
- **And** the sets of `/essay.md` are unchanged
- **Priority:** High

#### E2E-094: a table is never cut inside a row
- **Category:** edge. **Scenario:** SC-006. **Requirements:** FR-NEW-005, FR-NEW-007
- **Given** text: `# A`, a paragraph, a table of 10 rows of 300 tokens each, a paragraph
- **When** Alice builds a `structural` set
- **Then** every chunk that holds table rows holds whole rows; the row groups start with the header row and the separator row
- **And** each row appears once
- **Priority:** Medium

#### E2E-095: an inline image is not a figure
- **Category:** edge. **Scenario:** SC-006. **Requirements:** FR-NEW-004
- **Given** text: a paragraph `see ![chart](chart.png) for details`
- **When** Alice builds a `structural` set
- **Then** the chunk text keeps the paragraph as is; the chunk carries no figure reference
- **And** figure references empty
- **Priority:** Medium

#### E2E-138: recalculating with search not enabled
- **Category:** edge. **Scenario:** SC-008. **Requirements:** FR-NEW-019
- **Given** the server has search disabled; project `plain` (mode `none`); `/guide.md` has an extra `fixed` set made by hand
- **When** Alice calls `fs.recalculate` with kind `chunks` and that `set_id`
- **Then** the answer is a job and within 7 s the set is `current` with the chunks of the current text
- **And** the same call with kind `search_index` on an existing file answers `ERR_NOT_SUPPORTED: search is not enabled; set search.enabled: true in config`; a search does not find the new text
- **Priority:** Medium

#### E2E-139: an unknown setting name and a value that is not a whole number
- **Category:** failure. **Scenario:** SC-003. **Requirements:** FR-NEW-017, FR-NEW-013
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/essay.md`
- **When** Alice creates a `fixed` set with the setting `window` 5, then with `size_tokens` `2.5`, then with `size_tokens` `abc`
- **Then** the answers are `ERR_INVALID_ARGUMENT: unknown setting 'window'`, then twice `ERR_INVALID_ARGUMENT: size_tokens must be a whole number between 32 and 8192`
- **And** no set was created
- **Priority:** High

#### E2E-140: a non text file is not rebuilt automatically
- **Category:** edge. **Scenario:** SC-004. **Requirements:** FR-NEW-021, FR-NEW-022, FR-NEW-020
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/report.pdf` converted with an extra set made by hand
- **When** Alice replaces the PDF with a new version, converts it again and waits 15 s
- **Then** the extra set is `out_of_date` and unchanged; the rebuild of the file ended `done`
- **And** recalculating the set by hand makes it `current`
- **Priority:** Medium

#### E2E-141: a mode change keeps the sets
- **Category:** edge. **Scenario:** SC-010. **Requirements:** FR-NEW-021, FR-NEW-025
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/guide.md` has a default set
- **When** the platform admin sets the index mode of `proj` to `none`, then changes the file and waits 15 s
- **Then** the default set is listed `out_of_date`, unchanged, and no rebuild ran
- **And** setting the mode back to `bm25` builds the default set again
- **Priority:** Medium

#### E2E-142: search.index on a stale default set
- **Category:** edge. **Scenario:** SC-009. **Requirements:** FR-NEW-024, FR-NEW-022
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/guide.md` has a default set `out_of_date` (saved 1 s ago, settle delay 30 s)
- **When** Alice calls `search.index` for `/guide.md`
- **Then** the default set is rebuilt first and becomes `current`; the answer counts its chunks
- **And** the search finds the new text
- **Priority:** Medium

#### E2E-143: search.index on a binary file
- **Category:** failure. **Scenario:** SC-009. **Requirements:** FR-NEW-024, FR-NEW-020
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/logo.png` is a PNG image
- **When** Alice calls `search.index` for `/logo.png`
- **Then** the answer is `ERR_NOT_SUPPORTED: no text to cut for /logo.png`
- **And** nothing was indexed
- **Priority:** High

#### E2E-144: a result from an entry built before the upgrade
- **Category:** edge. **Scenario:** SC-010. **Requirements:** FR-NEW-023, FR-NEW-026
- **Given** `/old.md` indexed before the upgrade with character chunks
- **When** Alice searches a word of it before its rebuild
- **Then** the result carries `chunk_number` 0 and an empty `heading_path`
- **And** after its rebuild the result carries the real chunk number
- **Priority:** Medium

#### E2E-145: an overlap default not below the size default
- **Category:** failure. **Scenario:** SC-010. **Requirements:** FR-NEW-027
- **Given** the server configuration has `search.chunking.fixed_size_tokens` 100 and `search.chunking.fixed_overlap_tokens` 100
- **When** the operator starts the server
- **Then** the start stops with `ERR_INVALID_ARGUMENT: search.chunking.fixed_overlap_tokens must be smaller than search.chunking.fixed_size_tokens`
- **And** with 99 the server starts
- **Priority:** High

#### E2E-146: copying and deleting a folder
- **Category:** edge. **Scenario:** SC-007. **Requirements:** FR-NEW-025
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/docs/a.md` and `/docs/b.md` each have a default set
- **When** Alice copies `/docs` to `/copy`, waits 15 s, then deletes `/docs`
- **Then** `/copy/a.md` and `/copy/b.md` each list only a default set; `/docs/a.md` is refused with `ERR_NOT_FOUND: file not found: /docs/a.md`
- **And** no set of `/docs` remains
- **Priority:** Medium

### 9.4 Modified tests
#### E2E-MOD-001: status counts chunks, not files (was SPEC-0008/E2E-004)
- **Category:** journey. **Scenario:** SC-009. **Requirements:** FR-MOD-001
- **Validated before:** `/long.md` (5000 characters) and `/doc1.md` (25 characters) indexed with `chunk_size` 1000 and `chunk_overlap` 100: `search.status` reports more than 2 documents.
- **Validates now:** both files are indexed through their default sets (no arguments): `/long.md` holds 1000 tokens in one section, so it is one chunk and `bm25_docs` is 2, equal to the total of `chunk_count`; with `/long.md` holding 3 sections of 1500 tokens under `# ` headings, `bm25_docs` is 4 (3 plus 1).
- **Given** project in mode `bm25`, two files as above
- **When** Alice indexes them with `search.index` and reads `search.status`
- **Then** `bm25_docs` equals the total of `chunk_count` of the default sets, and `mode` reads `bm25`
- **Priority:** High

#### E2E-MOD-002: re-indexing is idempotent (was SPEC-0008/E2E-011)
- **Category:** journey. **Scenario:** SC-009. **Requirements:** FR-MOD-001
- **Validated before:** indexing twice with the same arguments reports the same inserted count and the same status.
- **Validates now:** indexing twice with no arguments reports the same number of chunks, the default set keeps the same id and chunks, and a query returns each chunk once.
- **Given** `/doc1.md` indexed once
- **When** Alice indexes it again
- **Then** the reported chunk count is equal, `search.status` is unchanged, a query returns each chunk once with no duplicate path and chunk number
- **Priority:** High

#### E2E-MOD-003: a rebuild by hand with search disabled (was SPEC-0100/E2E-069)
- **Category:** failure. **Scenario:** SC-008. **Requirements:** FR-MOD-003
- **Validated before:** with search disabled for the server, `fs.recalculate` of kind `search_index` on `/notes.md` answered `ERR_NOT_SUPPORTED: search is not enabled; set search.enabled: true in config` (the test never created the file).
- **Validates now:** the file `/notes.md` exists and the same answer is given; with an absent path the answer is `ERR_NOT_FOUND: file not found: /notes.md`, because `search not enabled` now comes after `path absent` and `path is a folder`.
- **Given** search disabled for the server, project `proj`, Alice a member, `/notes.md` saved
- **When** Alice calls `fs.recalculate` with kind `search_index` on `/notes.md`, then on `/ghost.md`
- **Then** the answers are `ERR_NOT_SUPPORTED: search is not enabled; set search.enabled: true in config` and `ERR_NOT_FOUND: file not found: /ghost.md`
- **Priority:** High

### 9.5 Removed tests
None.

## 10. Traceability
| Scenario | Requirements | Happy | Failure | Edge |
|---|---|---|---|---|
| SC-001 | FR-MOD-002, FR-NEW-009, FR-NEW-014, FR-NEW-020, FR-NEW-021, FR-NEW-022, FR-NEW-023, FR-NEW-027 | E2E-001 | E2E-002, E2E-003, E2E-004 | E2E-005, E2E-006, E2E-007 |
| SC-002 | FR-NEW-009, FR-NEW-014, FR-NEW-015, FR-NEW-016, FR-NEW-017, FR-NEW-028 | E2E-010, E2E-011 | E2E-015, E2E-016, E2E-017, E2E-018, E2E-021, E2E-022, E2E-023 | E2E-012, E2E-013, E2E-014, E2E-019, E2E-020 |
| SC-003 | FR-NEW-001, FR-NEW-006, FR-NEW-008, FR-NEW-010, FR-NEW-011, FR-NEW-013, FR-NEW-014, FR-NEW-017, FR-NEW-018, FR-NEW-020, FR-NEW-028, FR-NEW-029 | E2E-030, E2E-038 | E2E-033, E2E-034, E2E-035, E2E-036, E2E-037, E2E-039, E2E-040, E2E-041, E2E-043, E2E-045, E2E-139 | E2E-031, E2E-032, E2E-042, E2E-044 |
| SC-004 | FR-NEW-009, FR-NEW-013, FR-NEW-014, FR-NEW-017, FR-NEW-020, FR-NEW-021, FR-NEW-022 | E2E-050 | E2E-051 | E2E-052, E2E-053, E2E-140 |
| SC-005 | FR-NEW-001, FR-NEW-012, FR-NEW-013, FR-NEW-021, FR-NEW-029 | E2E-060 | E2E-062 | E2E-061, E2E-063 |
| SC-006 | FR-NEW-001, FR-NEW-002, FR-NEW-003, FR-NEW-004, FR-NEW-005, FR-NEW-006, FR-NEW-007, FR-NEW-008, FR-NEW-009, FR-NEW-010, FR-NEW-020 | E2E-070, E2E-089 | E2E-080, E2E-083, E2E-084 | E2E-071, E2E-072, E2E-073, E2E-074, E2E-075, E2E-076, E2E-077, E2E-078, E2E-079, E2E-081, E2E-082, E2E-085, E2E-086, E2E-087, E2E-088, E2E-090, E2E-091, E2E-092, E2E-093, E2E-094, E2E-095 |
| SC-007 | FR-NEW-021, FR-NEW-025 | E2E-100, E2E-101 | E2E-102, E2E-104 | E2E-103, E2E-105, E2E-146 |
| SC-008 | FR-MOD-002, FR-MOD-003, FR-NEW-014, FR-NEW-017, FR-NEW-019, FR-NEW-021, FR-NEW-022, FR-NEW-023 | E2E-110, E2E-111 | E2E-112, E2E-113, E2E-114, E2E-115 | E2E-116, E2E-117, E2E-138 |
| SC-009 | FR-NEW-020, FR-NEW-022, FR-NEW-023, FR-NEW-024 | E2E-120, E2E-122 | E2E-123, E2E-126, E2E-143 | E2E-121, E2E-124, E2E-125, E2E-142 |
| SC-010 | FR-NEW-019, FR-NEW-021, FR-NEW-023, FR-NEW-025, FR-NEW-026, FR-NEW-027 | E2E-130, E2E-134 | E2E-131, E2E-132, E2E-145 | E2E-133, E2E-135, E2E-136, E2E-141, E2E-144 |

Tests per requirement (at least 3 each):
- FR-NEW-001: E2E-043, E2E-061, E2E-063, E2E-070, E2E-071, E2E-090 (6 tests)
- FR-NEW-002: E2E-070, E2E-071, E2E-072, E2E-086 (4 tests)
- FR-NEW-003: E2E-074, E2E-075, E2E-076 (3 tests)
- FR-NEW-004: E2E-087, E2E-088, E2E-095 (3 tests)
- FR-NEW-005: E2E-077, E2E-079, E2E-094 (3 tests)
- FR-NEW-006: E2E-043, E2E-072, E2E-073, E2E-091 (4 tests)
- FR-NEW-007: E2E-079, E2E-080, E2E-081, E2E-082, E2E-083, E2E-091, E2E-094 (7 tests)
- FR-NEW-008: E2E-043, E2E-078, E2E-092 (3 tests)
- FR-NEW-009: E2E-001, E2E-011, E2E-050, E2E-089 (4 tests)
- FR-NEW-010: E2E-044, E2E-084, E2E-085, E2E-086, E2E-093 (5 tests)
- FR-NEW-011: E2E-030, E2E-035, E2E-036, E2E-044 (4 tests)
- FR-NEW-012: E2E-060, E2E-061, E2E-062, E2E-063 (4 tests)
- FR-NEW-013: E2E-030, E2E-031, E2E-033, E2E-034, E2E-037, E2E-042, E2E-050, E2E-062, E2E-139 (9 tests)
- FR-NEW-014: E2E-001, E2E-004, E2E-010, E2E-019, E2E-020, E2E-030, E2E-052, E2E-117 (8 tests)
- FR-NEW-015: E2E-010, E2E-014, E2E-015, E2E-018 (4 tests)
- FR-NEW-016: E2E-011, E2E-012, E2E-013, E2E-016, E2E-017 (5 tests)
- FR-NEW-017: E2E-015, E2E-016, E2E-017, E2E-018, E2E-021, E2E-022, E2E-023, E2E-033, E2E-034, E2E-035, E2E-036, E2E-037, E2E-040, E2E-041, E2E-043, E2E-051, E2E-112, E2E-045, E2E-139 (19 tests)
- FR-NEW-018: E2E-038, E2E-039, E2E-040 (3 tests)
- FR-NEW-019: E2E-110, E2E-111, E2E-112, E2E-113, E2E-114, E2E-115, E2E-116, E2E-136, E2E-138 (9 tests)
- FR-NEW-020: E2E-005, E2E-006, E2E-041, E2E-050, E2E-051, E2E-052, E2E-053, E2E-093, E2E-124, E2E-140, E2E-143 (11 tests)
- FR-NEW-021: E2E-001, E2E-002, E2E-003, E2E-004, E2E-005, E2E-007, E2E-053, E2E-060, E2E-101, E2E-116, E2E-130, E2E-135, E2E-140, E2E-141 (14 tests)
- FR-NEW-022: E2E-002, E2E-003, E2E-006, E2E-052, E2E-117, E2E-126, E2E-140, E2E-142 (8 tests)
- FR-NEW-023: E2E-001, E2E-110, E2E-120, E2E-121, E2E-125, E2E-144 (6 tests)
- FR-NEW-024: E2E-122, E2E-123, E2E-124, E2E-126, E2E-142, E2E-143 (6 tests)
- FR-NEW-025: E2E-100, E2E-101, E2E-102, E2E-103, E2E-104, E2E-105, E2E-141, E2E-146 (8 tests)
- FR-NEW-026: E2E-134, E2E-135, E2E-136, E2E-144 (4 tests)
- FR-NEW-027: E2E-007, E2E-130, E2E-131, E2E-132, E2E-133, E2E-145 (6 tests)
- FR-NEW-028: E2E-021, E2E-042, E2E-045 (3 tests)
- FR-NEW-029: E2E-031, E2E-032, E2E-042, E2E-062 (4 tests)

## 11. Open questions
None that change observable behavior.

## 12. Glossary (MANDATORY)
| Term | Definition | Context |
|---|---|---|
| Chunk | a piece of a file's text, kept with its heading path, number, unit, pages, size and figure references | Documents |
| Chunk set (set) | one complete cutting of one file's text by one strategy and one group of settings; a file can hold up to 5 | Documents |
| Default set | the set the search entries are built from; built automatically when a text file of an indexing project settles | Documents, Search |
| Strategy | `structural` (follows the structure of the text) or `fixed` (cuts by token count) | Documents |
| Settings | the numbers a set was cut with (`size_tokens`, `overlap_tokens`, `heading_cut_depth`, `max_tokens`, `list_max_items`) | Documents |
| Heading path | the titles of the open headings when a section starts, at most 2 | Documents |
| Unit | a run of prose, or one table, list or code block, that becomes one or several chunks | Documents |
| Section | the text between two cutting headings | Documents |
| Token | a unit of the `cl100k_base` ruler used for all sizes | Documents |
| Document text | for a text file its content; for a converted document the complete text kept with its conversion, with image captions, recognized text and tables | Documents |
| Fallback | a `structural` set built as `fixed` because the text has no heading | Documents |
| Out of date | a set cut from an earlier content of its file | Documents |
| Indexed text | a header line (title and heading path) followed by the chunk text, what search matches | Search |
| Search entries | what search keeps of a file, replaced as a whole | Search |
| Job, kind, settle delay | as in SPEC-0100 | Derived data |
| Member | a person who belongs to a project | Documents |

## 13. Implementability gate (MANDATORY)
| Round | F | A | Verdict |
|---|---|---|---|
| 1 | 20 | 3 | NOT-IMPLEMENTABLE (merge rule, recalculate order and job scope, chunk field names, requested values match key, packing rules, empty text, fixed tail, non text rebuild, upgrade fields, config bounds, EARS) |
| 2 | 13 | 0 | NOT-IMPLEMENTABLE (Markdown element definitions, the `# T` convention for structural tests, test arithmetic, binary file outcomes, set token count, joiner, check order, folder copy coverage) |
| 3 | 4 | 1 | IMPLEMENTABLE (refusal order modifying SPEC-0100/E2E-069, recalculating when search is off, settings of a fallback set after a heading appears, packing with headers and fences, a test mode) |

**Amendments applied:** every finding of rounds 1 to 3 is merged; E2E-146 and E2E-MOD-003 added; the checks of Phase 10 were run on the final file
**No implementation detail outside Section 14:** PASS

## 14. Evidence appendix (for /sdlc-design and the auditor, not for the product owner)
### 14.1 Current State evidence
| # | Claim (Section 2) | Evidence |
|---|---|---|
| 1 | character windows, size, overlap and the 50 character look back | `crates/core/src/search/chunker.rs:11` |
| 2 | each engine cuts for itself (4 call sites) | `crates/core/src/search/bm25_sqlite.rs:125`, `bm25_pg.rs:80`, `vector_sqlite.rs:120`, `vector_pg.rs:109` |
| 3 | auto indexing at 1000 and 100 | `crates/core/src/search/indexer.rs:45,47`, `:130` |
| 4 | indexing reads only UTF-8 files | `crates/core/src/search/indexer.rs:182` |
| 5 | a search result is path, score, chunk, rank | `crates/core/src/search/mod.rs:39-44` |
| 6 | artifacts, document view tools | `crates/core/src/mcp/server.rs:2580` (`fs.get_document_view`), `crates/core/src/tools/artifacts.rs` |
| 7 | rebuild kinds today: only `search_index`; refusal text | `crates/core/src/search/derived/mod.rs:21-40` (text at :38), `crates/core/src/tools/derived.rs:56,229` |
| 8 | tests of the cutter and of search | `crates/core/src/search/chunker.rs` (5), `crates/core/src/search/e2e.rs` (161), `crates/core/src/search/derived_e2e.rs` (75) |
| 9 | the companion is indexed at once, not scheduled | `crates/core/src/search/indexer.rs:259` |
| 10 | `search.index` arguments `chunk_size` and `chunk_overlap` | `crates/core/src/mcp/server.rs:1524-1528` |
Test command: `make test` (`./test.sh`); search only: `cargo test -p fsa-core --lib search::`.

### 14.2 Class sweep
Not applicable (`Security: n/a`).

### 14.3 Design inputs
- A tokenizer with the `cl100k_base` vocabulary must ship inside the binary or the repository: no network at runtime. No tokenizer crate is in the dependency lock today (survey of 2026-11-21). Signal: the tests of FR-NEW-010 and E2E-079 to E2E-083 compare token counts.
- Chunks and sets are a new persisted concept per file: where it lives, how it follows move and deletion (SPEC-0100 already moves jobs), how a set is replaced in one step. Signal: E2E-002, E2E-003, E2E-100 to E2E-105.
- The document text of a converted document: the text `fs.get_document_view` composes (`captions` true, `table_mode` `markdown`; `crates/core/src/docs/artifacts/render.rs:107-211`); confirm that recognized text is inside the stored conversion text. Signal: E2E-050.
- `fs.recalculate` gains `set_id` and the kind `chunks` (SPEC-0100 `JobKind`, `tools/derived.rs`); the job record needs the set; the tool contract files regenerate. Signal: E2E-110 to E2E-115.
- The cutter replaces the four call sites of the character cutter; the search backend trait must receive chunks, not text (`index_path`), and the indexed header line. Signal: E2E-120, E2E-121, E2E-MOD-001.
- A `search.status` count of documents now counts chunks of the default sets (E2E-MOD-001).
- The tool contract and its count change: five tools added (`fs.list_chunk_sets`, `fs.list_chunks`, `fs.get_chunk`, `fs.create_chunk_set`, `fs.delete_chunk_set`).
- Server keys `search.chunking.*` and their boot validation follow the pattern of `search.settle_delay_seconds` (SPEC-0100/FR-NEW-014, FR-NEW-016).
- The Markdown companion of a converted document is indexed at once, not by a rebuild (`crates/core/src/search/indexer.rs:259,275`); FR-NEW-021 requires its default set to be built before indexing. Signal: E2E-053.
- Set ids need a stable form (design). A structural set made by hand on a headingless file is stored as `fixed` with the server fixed defaults (E2E-062); the design must keep the `fallback_from` field across rebuilds.
