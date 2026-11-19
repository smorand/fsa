---
Id: BL-0164
Title: Platform settings stored in the database, managed by an admin interface, starting with the model registry
Nature: FEAT
Source: owner request of 2026-11-19: no configuration file and no environment variables, only the database URL, everything else set through an admin interface
Created: 2026-11-19
Decision: the first increment is the model registry (providers and models); moving every other setting is BL-0173
---

## What
A platform administrator creates and edits, through admin tools, the REST plane and a web screen, the settings that today live in the YAML file and in environment variables. This entry builds the store and the first two kinds:
- **providers**: a name, an endpoint, an authentication scheme and its secret (stored encrypted, never returned);
- **models**: a name, the provider, a kind (text embedding, image embedding, reranker, OCR, chat), the vector dimension for the embedding kinds, and limits.
Projects (BL-0170) and the converters (BL-0165) refer to a model by its id.

## Why
Settings in a file and in the environment are hard to deploy and to maintain, and a per project choice of model needs a place to pick a model from. The owner wants setup through an interface, with the database URL as the only environment variable.

## Evidence
- Today the embedding and rerank models are one server wide block, `search.embedding` and `search.reranking` (`crates/core/src/config.rs:754-800`), keys read from environment variables named by `api_key_env`.
- The existing admin surface manages projects, members, quota, purge and index mode only: 17 `admin.*` tools (`TOOL_CONTRACT.txt`, `admin.add_member` to `admin.undelete_project`) and three browser screens (`token_screen.rs`, `deleted_projects_screen.rs`, `trash_screen.rs`); none manages server settings.
- A secret cipher already exists for the OAuth tokens kept in the database (`crates/core/src/git/oauth/cipher.rs`, AES 256 GCM), its key coming from `MCPFS_TOKEN_KEY` (`cipher.rs:32`).

## Notes
- Open question for the spec, owner to decide: a secret stored in the database needs a key that cannot live in the same database. Recommendation: one environment variable beside the database URL, the master key (reusing the current token key). Alternative: a key file path.
- Who may change a setting: platform administrators only. Every change is audited without the secret.
- Whether a change applies at once or at the next start: the model registry is read at call time, so at once.
- Both PostgreSQL and SQLite must hold the settings.
