---
Id: BL-0159
Title: The Markdown to Word conversion fetches remote images named in a member's Markdown
Nature: BUG
Source: SPEC-0097 class sweep (CWE-918), validated by the owner on 2026-11-16
Created: 2026-11-16
Decision: deferred out of SPEC-0097 by the owner on 2026-11-16; a different mechanism and a different fix from the web address lists
---

## What
`crates/core/src/tools/doc.rs` (`run_pandoc`, line 289) runs pandoc on a member's Markdown with no sandbox option. Pandoc fetches every remote resource the document names, such as an image `![x](http://127.0.0.1:9/a.png)`. A member can therefore make the server request any address (loopback, private network, cloud metadata) by writing it in a Markdown file and asking for the Word conversion. The web address lists of SPEC-0097 do not cover it, because the request is made by pandoc, not by `web.fetch`.

## Why
Same class as BL-0156 (a member supplied address makes the server open a network connection, CWE-918). The document tools are off by default; the exposure starts when an operator enables them.

## Evidence
- Reproduction on 2026-11-16 with pandoc 3.14: `printf '![x](http://127.0.0.1:9/a.png)\n' > t.md; pandoc t.md -o t.docx` prints `[WARNING] Could not fetch resource http://127.0.0.1:9/a.png`, so the connection was attempted.
- `crates/core/src/tools/doc.rs:289-293`: the command is built with the input, the output and an optional reference document, and no `--sandbox`.
- Class sweep: `sdlc/specs/SPEC-0097-web-access-lists/spec.md` Section 14.2, occurrence 2.

## Notes
Candidate fix to confirm in the spec: run pandoc with `--sandbox` (no resource fetching), then check that a local image reference still behaves as the current tests expect, or apply the same black list and white list as SPEC-0097 to the images pandoc may fetch.
