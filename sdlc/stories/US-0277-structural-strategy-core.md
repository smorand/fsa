# US-0277: the structural strategy: sections, units, merging

> Spec: SPEC-0101 (sdlc/specs/SPEC-0101-chunk-sets-as-file-metadata/)
> Nature: FEAT
> Priority: P1
> Depends on: US-0276
> min_tier: 1
> Files touched: 7
> Closes spec: no

## Objective
Implement the `structural` strategy of paragraph v3a for everything that does not need cutting oversize elements: the block parser, sections cut at headings, heading paths, pages, thematic breaks, the metadata table, figures, atomic units with the list introduction line, small unit merging, and the fallback to `fixed` for a text with no heading. A member creates a structural set by hand and reads chunks that follow the document structure. Oversize pieces are US-0278: until then a unit above `max_tokens` is kept whole.

## Requirements covered
- SPEC-0101/FR-NEW-001: WHEN a set with the strategy `structural` is built THE system SHALL close the current section at each heading that leaves at most `heading_cut_depth` headings open, and SHALL start the next section with that heading.
- SPEC-0101/FR-NEW-002: The system SHALL keep the heading line in the text of the chunk it opens, and SHALL give each chunk a heading path made of the titles of the headings open when its section started, at most 2 titles.
- SPEC-0101/FR-NEW-003: The system SHALL record the page of a chunk from the page markers of the text, SHALL ignore thematic breaks, and SHALL leave out of every chunk the table placed first in the text whose header cells are all empty.
- SPEC-0101/FR-NEW-004: The system SHALL attach each figure reference of the text to the last chunk that holds text of the last unit starting before the figure, and SHALL add no figure text to any chunk.
- SPEC-0101/FR-NEW-005: The system SHALL NOT cut a table in the middle of a row, a list in the middle of an item, or a code block in the middle of a line, unless the element is above `max_tokens`.
- SPEC-0101/FR-NEW-006: WHEN two adjacent units are under the same parent and neither was produced by cutting a larger element and their text together fits `max_tokens` THE system SHALL merge them into one chunk.
- SPEC-0101/FR-NEW-010 (partial: cleaning of NUL and CRLF; no overlap): The system SHALL produce, for the same text and the same settings, the same chunks character for character, and SHALL produce no chunk above `max_tokens` tokens with the strategy `structural` or above `size_tokens` tokens with the strategy `fixed`.
- SPEC-0101/FR-NEW-012 (partial: the fallback when a set is created): IF a set with the strategy `structural` is built from a text that holds no heading THEN THE system SHALL build it with the strategy `fixed` and the server default fixed settings, and SHALL report the set with the strategy `fixed` and `fallback_from` equal to `structural`.
- SPEC-0101/FR-NEW-017 (partial: the structural setting ranges): IF a request to a chunk tool is not allowed or not valid THEN THE system SHALL refuse it, change nothing, and answer with the exact text of the rule that failed.
- SPEC-0101/FR-NEW-009 (partial: unit numbers, pages and figures of chunks): The system SHALL give each chunk its text, the title of its document, its heading path, its number in the set (1, 2, 3 and so on), its unit number, its first and last page, its token count and its figure references.

## Independent Test
Run `cargo test -p fsa-core --lib -- e2e_0101_043 e2e_0101_05 e2e_0101_06 e2e_0101_07 e2e_0101_08 e2e_0101_09 chunking::structural chunking::blocks`. Tests build texts with headings, pages, figures, tables and lists, create a `structural` set through `fs_create_chunk_set` and compare heading paths, pages, figures and chunk counts; unit tests run the 20 acceptance cases of paragraph v3a section 10 that this story covers (1 to 8, 11, 12, 16, 19).

## Design excerpt
### Decisions
- DEC-002 (SPEC-0101): A `chunking` module of pure functions: `tokens` (ruler singleton, count, encode, lossy decode), `fixed`, `blocks` plus `structural` (paragraph v3a), `model`, `service` (sources, build, replace), `store`. **Obliged by:** SPEC-0101/FR-NEW-001 to FR-NEW-012. **Rationale:** determinism and testability without a server. **Alternatives considered:** put cutting in `search/` (couples it to engines). **Source:** design. **Evidence:** `search/chunker.rs:11` is the code replaced.
- DEC-003 (SPEC-0101): `structural` implements paragraph v3a 5.1 to 5.9 literally, except the merge parent rule of FR-NEW-006 (top level titles equal). The block parser is a single pass with the first matching rule. Packing counts head and tail (header rows, fences). **Obliged by:** SPEC-0101/FR-NEW-001 to FR-NEW-008, FR-NEW-010. **Rationale:** the owner wants chunks close to v3a. **Alternatives considered:** a Markdown crate (does not give v3a block rules; a different parse). **Source:** spec constraint. **Evidence:** `/Users/sebastien/projects/perso/search-doc-corpora/specs/paragraph-v3a.md`.
- DEC-017 (SPEC-0101): A set made by hand keeps its requested strategy and settings and is rebuilt with them; `structural` requested on a text with no heading is stored as `fixed` with `fallback_from` and rebuilds as `structural` once a heading exists. **Obliged by:** SPEC-0101/FR-NEW-012. **Rationale:** the spec says so; requested values are stored beside used values. **Alternatives considered:** store used only (loses the request). **Source:** spec. **Evidence:** n/a.
### Data model
No schema change.
### Contracts
`chunking::structural::cut(text, title, settings: &StructuralSettings) -> Vec<Chunk>` with `StructuralSettings { heading_cut_depth, max_tokens, list_max_items }`; `chunking::blocks::parse(lines) -> Vec<Block>` where `Block` is Heading, Paragraph, Table, List, Code, Figure, PageMarker, ThematicBreak, MetadataTable; `chunking::cut(...)` dispatches: `structural` with no heading line (a `#` line outside a code block) is cut by `fixed` with the server fixed defaults and returns `fallback_from: structural`; an empty text is the same fallback with 0 chunks. Section text is blocks joined by `\n\n`; a merged chunk is units joined by `\n\n`; the merge parent rule is "top level titles equal, or both absent" (deliberate difference from v3a 5.4).
### Paths
| Path | Action |
|---|---|
| `crates/core/src/chunking/blocks.rs`, `chunking/structural.rs` | create |
| `crates/core/src/chunking/mod.rs`, `chunking/service.rs` | modify: dispatch and fallback replace the stub of US-0276 |
| `crates/core/src/chunking/e2e.rs` | modify: these tests and a text builder (headings, tables, lists, figures) |
| `.agent_docs/search.md` | modify: structural strategy paragraph |
### Existing patterns
Follow paragraph v3a 5.1 to 5.4, 5.6, 5.7 (`/Users/sebastien/projects/perso/search-doc-corpora/specs/paragraph-v3a.md`); the heading rule is "1 to 6 `#`, then a space or a tab, then a non empty title"; keep the parser a single pass with the first matching rule; token counts through `chunking::tokens::count` (memoize per unit).

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

### SPEC-0101/E2E-043: structural settings out of range
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_043_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/essay.md`
- **When** Alice creates `structural` sets with `heading_cut_depth` 7, `max_tokens` 63 and `list_max_items` 1
- **Then** the answers are `ERR_INVALID_ARGUMENT: heading_cut_depth must be between 1 and 6`, `ERR_INVALID_ARGUMENT: max_tokens must be between 64 and 8192` and `ERR_INVALID_ARGUMENT: list_max_items must be between 2 and 200`
- **And** no set was created; a set with `max_tokens` 64 is accepted
- **Priority:** High

### SPEC-0101/E2E-050: a converted document is chunked from its metadata text
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_050_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/report.pdf` was converted: body of 3 pages, 2 images with captions `Figure 1: sales` and `Figure 2: costs`, one recognized text line `Total 42`, one table of 4 rows
- **When** Alice creates a `structural` set for `/report.pdf` and lists its chunks and reads chunk 1
- **Then** the set is built; the chunks together hold the body text, the captions, the recognized text and the table rows; document title `report`; no chunk holds image bytes
- **And** the text is the text of the document view with captions and tables in Markdown
- **Priority:** High

### SPEC-0101/E2E-062: a structural set by hand on a text without headings
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_062_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/notes.txt` has no heading
- **When** Alice creates a `structural` set with `max_tokens` 1000
- **Then** the answer is a set with strategy `fixed`, `fallback_from` `structural`, `size_tokens` 2048 (the server default fixed settings, not `max_tokens`)
- **And** creating the same `structural` request again returns the same set; creating a `fixed` set with `size_tokens` 2048 makes another set; after Alice adds a heading and recalculates the set, it is `structural` with `max_tokens` 1000
- **Priority:** High

### SPEC-0101/E2E-063: a heading inside a code block is not a heading
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_063_*`)
- **Given** project `proj` indexing its files (mode `bm25`), settle delay 3 s, Alice (`alice@example.com`) a member; `/snip.md` holds a fenced code block whose first line is `# not a heading` and no other heading
- **When** Alice creates a `structural` set
- **Then** the set is the `fixed` fallback
- **And** adding a real heading outside the code makes the next rebuild `structural`
- **Priority:** Medium

### SPEC-0101/E2E-070: sections follow the headings
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_070_*`)
- **Given** text: `intro` of 100 tokens, `# A`, text a of 1500 tokens, `## B`, text b of 500 tokens, `### C`, text c of 500 tokens
- **When** Alice builds a `structural` set from the text
- **Then** three chunks: the intro with an empty heading path; the section A with path `A`; the section B with path `A > B` holding `## B`, text b, `### C` and text c (C does not cut); the sections A and B do not merge (their total is above 2048 tokens)
- **And** the heading lines stay in the text of the chunks
- **Priority:** High

### SPEC-0101/E2E-071: a document starting at level 3
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_071_*`)
- **Given** text: `### X`, text x, `### Y`, text y, each text of 1500 tokens
- **When** Alice builds a `structural` set
- **Then** the chunks are `X` and `Y` with heading paths `X` and `Y`
- **And** the cut counts open headings, not `#` marks
- **Priority:** Medium

### SPEC-0101/E2E-072: two short sections under one heading merge
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_072_*`)
- **Given** text: `# A`, 100 tokens, `## B`, 100 tokens, `## C`, 100 tokens
- **When** Alice builds a `structural` set
- **Then** one chunk, heading path `A`, holding the text of A and the sections B and C (the units `A`, `A > B` and `A > C` are under the same parent)
- **And** the chunk holds the three heading lines and the common heading path is `A`
- **Priority:** High

### SPEC-0101/E2E-073: two short sections under different headings do not merge
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_073_*`)
- **Given** text: `# A`, 100 tokens, `# B`, 100 tokens
- **When** Alice builds a `structural` set
- **Then** two chunks, heading paths `A` and `B`
- **And** chunk numbers 1 and 2
- **Priority:** High

### SPEC-0101/E2E-074: a page marker records the page and cuts nothing
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_074_*`)
- **Given** text: paragraph one, a line `---`, a line `*[Page 3]*`, paragraph two
- **When** Alice builds a `structural` set
- **Then** one chunk; its first and last page are both 3; no marker line in its text
- **And** a text with no page marker gives chunks with unknown pages
- **Priority:** Medium

### SPEC-0101/E2E-075: a thematic break is ignored
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_075_*`)
- **Given** text: paragraph one, a line `---` alone, paragraph two
- **When** Alice builds a `structural` set
- **Then** one chunk whose text holds both paragraphs and no `---` line
- **And** pages unknown
- **Priority:** Medium

### SPEC-0101/E2E-076: the metadata table is left out
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_076_*`)
- **Given** text: a first table whose header cells are all empty and one row `| Author | Smith |`, then `# A` and text
- **When** Alice builds a `structural` set
- **Then** no chunk holds `Author` or `Smith`
- **And** the chunk holds `# A` and its text
- **Priority:** Medium

### SPEC-0101/E2E-077: a list introduction travels with its list
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_077_*`)
- **Given** text: a paragraph ending with `Causes:` then a list of 3 items
- **When** Alice builds a `structural` set
- **Then** the chunk that holds the list holds the line `Causes:` immediately before the first item, and `Causes:` occurs once in the whole set
- **And** the list items are intact
- **Priority:** Medium

### SPEC-0101/E2E-084: carriage returns and NUL characters
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_084_*`)
- **Given** two texts: the same text once with `\r\n` line ends and NUL characters, once cleaned
- **When** Alice builds a `structural` set from each
- **Then** the chunks are identical character for character
- **And** no NUL character in any chunk
- **Priority:** Medium

### SPEC-0101/E2E-087: a figure follows the unit before it
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_087_*`)
- **Given** text: `# A`, paragraph one, a line `![chart](chart.png)`, `# B`, paragraph two
- **When** Alice builds a `structural` set
- **Then** chunk of A carries the figure reference `chart.png`; the chunk of B carries none
- **And** no figure text appears in any chunk
- **Priority:** Medium

### SPEC-0101/E2E-088: a figure before every unit
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_088_*`)
- **Given** text: a line `[Figure]`, a line `![logo](logo.png)`, then paragraph one
- **When** Alice builds a `structural` set
- **Then** chunk 1 carries the figure reference `logo.png`
- **And** one figure reference, not two
- **Priority:** Medium

### SPEC-0101/E2E-089: what a chunk holds
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_089_*`)
- **Given** file `/guide.md` with `# A`, text, a line `---`, a line `*[Page 2]*`, `# B`, text
- **When** Alice builds a `structural` set and reads chunk 2
- **Then** the chunk shows `document_title` `guide`, `heading_path` [`B`], `number` 2, `unit` 2, `page_start` 2, `page_end` 2, its `token_count`, `figures` empty
- **And** the unit numbers rise with the text
- **Priority:** High

### SPEC-0101/E2E-090: heading_cut_depth 1
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_090_*`)
- **Given** text: `# A`, text, `## B`, text, `# C`, text, each 1500 tokens
- **When** Alice builds a `structural` set with `heading_cut_depth` 1
- **Then** no chunk has the path `A > B`; the chunks holding `# A`, text a, `## B` and text b all have the path `A` and each has at most 2048 tokens; one chunk has the path `C`
- **And** with depth 2 a chunk has the path `A > B`
- **Priority:** Medium

### SPEC-0101/E2E-093: an empty file
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_093_*`)
- **Given** an empty text file `/empty.md`
- **When** Alice creates a `structural` set
- **Then** a set with strategy `fixed`, `fallback_from` `structural`, `chunk_count` 0, `token_count` 0
- **And** listing its chunks answers an empty list and no marker
- **Priority:** Medium

### SPEC-0101/E2E-095: an inline image is not a figure
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_095_*`)
- **Given** text: a paragraph `see ![chart](chart.png) for details`
- **When** Alice builds a `structural` set
- **Then** the chunk text keeps the paragraph as is; the chunk carries no figure reference
- **And** figure references empty
- **Priority:** Medium

## Constraints
- Do not cut oversize units here (US-0278); do not touch the search engines.
- Linear time: no regex that can backtrack across lines.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the tests of the touched modules keep passing unchanged, except those named under Paths as modified.
- [ ] Docs updated: `.agent_docs/search.md`.
- [ ] Self review: every requirement above is met by a named test; every error text equals the design contract; no dash used as punctuation in code, comments or docs; no secret logged.

- [ ] One commit `US-0277: the structural strategy: sections, units, merging` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0101` and the requirement ids.
