---
Id: BL-0170
Title: Per project indexing profile: chunking, text model and image model
Nature: FEAT
Source: owner roadmap of 2026-11-19; supersedes BL-0035
Created: 2026-11-19
---

## What
Each project has an indexing profile: the chunk strategy and its parameters (BL-0168), the text embedding model and the image embedding model, each chosen from the model registry (BL-0164). Admin tools and the web screen read and change it. Changing a model marks the project's vectors as out of date.

## Why
The embedding model is one server wide setting today, and a vector from one model cannot be compared with one from another.

## Evidence
- `search.embedding` is a single block (`crates/core/src/config.rs:754-800`); the dimension is frozen after the first index (`.agent_docs/search.md`, "Config keys").
- The project already carries an `index_mode` (`admin.set_index_mode`, `admin.get_index_mode`), the natural neighbour of the profile.

## Notes
- Depends on BL-0164 and BL-0168.
- A different dimension per project means the vector storage cannot assume one column width; the design decides.
