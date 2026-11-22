# US-0278: the structural strategy: oversize pieces and list batches

> Spec: SPEC-0101 (sdlc/specs/SPEC-0101-chunk-sets-as-file-metadata/)
> Nature: FEAT
> Priority: P1
> Depends on: US-0277
> min_tier: 1
> Files touched: 5
> Closes spec: no

## Objective
Complete the `structural` strategy with the cutting of units above `max_tokens`: prose at blank lines and packed, tables into row groups with the header repeated, lists into item groups, code blocks with fences on every piece, hard split between lines, words and tokens, and lists above `list_max_items` cut into batches. After this story a structural chunk never exceeds `max_tokens` (except the header or fence case).

## Requirements covered
- SPEC-0101/FR-NEW-007: WHEN a unit is above `max_tokens` THE system SHALL cut it into pieces of at most `max_tokens` tokens, with no overlap and no text repeated, except the table header rows and the code fences.
- SPEC-0101/FR-NEW-008: WHEN a list holds more than `list_max_items` top level items THE system SHALL cut it into batches of `list_max_items` items, and SHALL NOT merge a batch with a neighbouring unit.
- SPEC-0101/FR-NEW-010 (partial: size bounds of `structural`, determinism): The system SHALL produce, for the same text and the same settings, the same chunks character for character, and SHALL produce no chunk above `max_tokens` tokens with the strategy `structural` or above `size_tokens` tokens with the strategy `fixed`.

## Independent Test
Run `cargo test -p fsa-core --lib -- e2e_0101_078 e2e_0101_079 e2e_0101_08 e2e_0101_09 chunking::structural`. Tests build a table of 5000 tokens, a list of 95 items, an unclosed code block of 3000 tokens, prose of 5000 tokens, a paragraph of 3000 tokens with no line break and a string of 3000 tokens with no space, and assert piece sizes, header and fences, and that the pieces joined give back the text; v3a section 10 cases 8, 9, 10, 13, 14, 15, 17 to 20 run as unit tests.

## Design excerpt
### Decisions
- DEC-003 (SPEC-0101): `structural` implements paragraph v3a 5.1 to 5.9 literally, except the merge parent rule of FR-NEW-006 (top level titles equal). The block parser is a single pass with the first matching rule. Packing counts head and tail (header rows, fences). **Obliged by:** SPEC-0101/FR-NEW-001 to FR-NEW-008, FR-NEW-010. **Rationale:** the owner wants chunks close to v3a. **Alternatives considered:** a Markdown crate (does not give v3a block rules; a different parse). **Source:** spec constraint. **Evidence:** `/Users/sebastien/projects/perso/search-doc-corpora/specs/paragraph-v3a.md`.
- DEC-016 (SPEC-0101): Cuts run in `spawn_blocking`; the ruler is a `OnceLock` singleton. **Obliged by:** SPEC-0101/FR-NEW-010 (bounded linear cost), NFR 7.1. **Rationale:** a 30 000 token text must not block the runtime. **Alternatives considered:** inline cut. **Source:** design. **Evidence:** n/a.
### Data model
No schema change.
### Contracts
Inside `chunking::structural`: `split_unit(unit, max_tokens, list_max_items) -> Vec<Piece>`, `pack(elements, limit, head, tail, join)` (v3a 5.8: estimate sum plus 1 per join plus head and tail, then recount and drop the last element while above the limit and more than one element), `hard_split(text, limit)` (v3a 5.9: lines, then words, then token prefixes). A top level list item starts at a marker line indented no more than the first item line. The only chunk allowed above `max_tokens` is one row (or line) with its header rows (or fences) when those alone exceed it.
### Paths
| Path | Action |
|---|---|
| `crates/core/src/chunking/structural.rs` | modify: splitting, packing, batches |
| `crates/core/src/chunking/tokens.rs` | modify: token prefix helper for the hard split |
| `crates/core/src/chunking/e2e.rs` | modify: these tests and builders |
### Existing patterns
v3a sections 5.3 (lists), 5.5, 5.8, 5.9; the unit struct of US-0277; token counting helper `chunking::tokens::count`.

## Acceptance tests
> 100% must pass, run through `make check` (or the project target). Never run test files ad hoc.

### SPEC-0101/E2E-078: a list of 95 items
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_078_*`)
- **Given** text: `# L` and a list of 95 items of 10 tokens each
- **When** Alice builds a `structural` set
- **Then** four chunks: `# L`, then three list chunks of 40, 40 and 15 items, none merged with a neighbour
- **And** each item appears once
- **Priority:** Medium

### SPEC-0101/E2E-079: a table of 5000 tokens
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_079_*`)
- **Given** text: a table of 200 rows, 5000 tokens in all
- **When** Alice builds a `structural` set
- **Then** row groups of at most 2048 tokens, each starting with the header row and the separator row; every row appears once
- **And** all pieces share the unit number
- **Priority:** Medium

### SPEC-0101/E2E-080: a code block never closed
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_080_*`)
- **Given** text: a line of three backticks, then 3000 tokens of code lines, no closing fence
- **When** Alice builds a `structural` set
- **Then** pieces of at most 2048 tokens, each starting with the opening fence line and ending with a closing fence
- **And** the code lines joined give back the original lines
- **Priority:** Medium

### SPEC-0101/E2E-081: prose above the limit
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_081_*`)
- **Given** text: 5000 tokens of paragraphs separated by blank lines
- **When** Alice builds a `structural` set
- **Then** pieces of at most 2048 tokens cut at blank lines, no repeated text
- **And** the pieces joined give back the text
- **Priority:** Medium

### SPEC-0101/E2E-082: one paragraph with no line break
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_082_*`)
- **Given** text: one paragraph of 3000 tokens made of words, no line break
- **When** Alice builds a `structural` set
- **Then** two pieces cut between words
- **And** the pieces joined give back the paragraph
- **Priority:** Medium

### SPEC-0101/E2E-083: one string with no space
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_083_*`)
- **Given** text: one string of 3000 tokens with no space and no line break
- **When** Alice builds a `structural` set
- **Then** two pieces cut by token count
- **And** the pieces joined give back the string
- **Priority:** Medium

### SPEC-0101/E2E-085: the same text twice
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_085_*`)
- **Given** a text of 30 000 tokens with headings, lists and tables
- **When** Alice builds the set twice
- **Then** the chunks are identical character for character, number for number
- **And** unit numbers and token counts are identical
- **Priority:** Medium

### SPEC-0101/E2E-086: no chunk above the limit and at most 2 titles
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_086_*`)
- **Given** a text of 30 000 tokens with headings of level 1 to 4
- **When** Alice builds a `structural` set
- **Then** every chunk has at most 2048 tokens and a heading path of at most 2 titles
- **And** no chunk repeats text of another
- **Priority:** High

### SPEC-0101/E2E-091: max_tokens 64
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_091_*`)
- **Given** text: `# A` and 300 tokens of paragraphs of 50 tokens
- **When** Alice builds a `structural` set with `max_tokens` 64
- **Then** chunks of at most 64 tokens, one paragraph each, no paragraph cut
- **And** no chunk above 64 tokens
- **Priority:** Medium

### SPEC-0101/E2E-092: list_max_items 2
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_092_*`)
- **Given** text: a list of 5 items
- **When** Alice builds a `structural` set with `list_max_items` 2
- **Then** three chunks of 2, 2 and 1 items
- **And** the introduction, if any, only on the first
- **Priority:** Medium

### SPEC-0101/E2E-094: a table is never cut inside a row
- Level / Driver / Location: functional e2e / `McpServer` handler on the shared fixture, clock paused / `crates/core/src/chunking/e2e.rs` (`e2e_0101_094_*`)
- **Given** text: `# A`, a paragraph, a table of 10 rows of 300 tokens each, a paragraph
- **When** Alice builds a `structural` set
- **Then** every chunk that holds table rows holds whole rows; the row groups start with the header row and the separator row
- **And** each row appears once
- **Priority:** Medium

## Constraints
- No change to merging or sections (US-0277).
- No overlap in structural pieces.
- Linear in the size of the text.

## Definition of done
- [ ] RED evidence recorded for every new test before the production code.
- [ ] Every acceptance test above green; `make check` green on the work branch after this commit; main only receives the whole squash at landing.
- [ ] Non regression: the tests of the touched modules keep passing unchanged, except those named under Paths as modified.
- [ ] Docs updated: None beyond US-0282.
- [ ] Self review: every requirement above is met by a named test; every error text equals the design contract; no dash used as punctuation in code, comments or docs; no secret logged.

- [ ] One commit `US-0278: the structural strategy: oversize pieces and list batches` on the work branch, body with `Summary:` (2 to 4 plain lines), `Spec: SPEC-0101` and the requirement ids.
