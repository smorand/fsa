---
Id: BL-0163
Title: Rename the project from mcp-fs to FSA (File System Agentic)
Nature: DEBT
Source: owner request of 2026-11-19
Created: 2026-11-19
Decision: owner approved the plan on 2026-11-19; the GitHub rename is an external, owner approved action
---

## What
Rename the product, the binary, the packages, the repository and the local folder to `fsa`. Tool names (`fs.*`, `admin.*`, `git.*`, `search.*`) do not change. The environment variables are not renamed one for one: the settings roadmap (BL-0164 and BL-0173) removes most of them, so this entry only renames what stays.

## Why
The name `mcp-fs` ties the product to one transport. FSA is the product: a file system for agents, reachable by MCP, REST and a web app.

## Evidence
- Packages `mcp-fs` (`crates/mcp-fs/Cargo.toml:2`) and `mcp-fs-core` (`crates/core/Cargo.toml:2`), library crate `mcp_fs_core` (`crates/core/Cargo.toml:12`).
- `rg -l 'mcp[-_]fs'` outside `sdlc/`, `plan/`, `target/` and `Cargo.lock` finds 55 files; `rg -l 'MCPFS_'` finds 34 files; the most used names are `MCPFS_TEST_PG_DSN` (56), `MCPFS_MINIO_SECRET_KEY` (13), `MCPFS_TOKEN_KEY` (12).
- Remote `git@github.com:smorand/mcp-fs-rust.git`; local folder `/Users/sebastien/projects/perso/mcp-fs-rust`.

## Notes
- Steps outside the code, to run only on the owner's yes at that moment: `gh repo rename`, `git remote set-url origin`, then the local folder move done last by the owner (the working directory of the session changes).
- Public contracts to plan: the binary name, the MCP server name clients see, the environment variables that stay, the Docker image and compose names, the config file names. Dual maintenance of a renamed environment variable is the default (new name first, old name fallback with one warning).
- Do this first: every later spec then carries the right name.
