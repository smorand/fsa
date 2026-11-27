# Configuration reference

One YAML file drives the server. Schema and defaults live in `config.rs` and are
identical to the reference implementation, key for key, so the same file drives
both. Every section and every key is optional: an absent section takes its
defaults, and unknown keys are ignored (there is no strict mode).

## `server`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `host` | string | `0.0.0.0` | bind address |
| `port` | int | `5002` | bind port |
| `mcp_path` | string | `/mcp` | path of the MCP endpoint; must start with `/`, otherwise the boot fails |
| `public_base_url` | string | `""` | origin prefixed to `fs.export_zip` URLs (`{public_base_url}/exports/{token}`); empty keeps them relative; `${VAR}` expandable |

## `auth.jwt`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `public_key_path` | string | `""` | RSA public key (PEM) used to verify bearers; empty means every token is rejected |
| `header` | string | `X-Forwarded-Authorization` | header checked first; `Authorization` is the fallback |
| `algorithms` | list | `[RS256]` | parsed for parity and unused: verification is always RS256 |
| `audience` | string | null | when set, `aud` must match; when absent, `aud` is not validated |
| `issuer` | string | `web-a2a` | when set, `iss` must match |
| `username_claim` | string | `email` | claim carrying the caller identity, lowercased on read |

Expiry and `nbf` are always validated with a 30 second leeway, matching the
reference clock skew.

## `auth`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `admins` | list of strings | `[]` | platform admins, matched caselessly. Grants project and membership management, **not** file access (see `.agent_docs/tools.md`) |

## Relational stores: `infra.meta`, `infra.admin`, `infra.git`, `infra.oauth`

Four stores hold the server's relational state, each configured independently and each
accepting the same two backends. Details of the layer are in
[`backends.md`](backends.md).

| Backend | Compiled | Needs |
|---|---|---|
| `sqlite` | always | a path, no dsn |
| `postgres` | `--features postgres` | a dsn |

Keys shared by all four sections:

| Key | Type | Default | Meaning |
|---|---|---|---|
| `backend` | string | `sqlite` | `sqlite` or `postgres`. Anything else is `ERR_INVALID_ARGUMENT` naming the accepted values; a database that was removed is refused first, before any other check (`config.rs::reject_removed_backends`) |
| `dsn` | string | `""` | PostgreSQL only. **Secret**: inject it with `${VAR}` |
| `schema` | string | `public` | PostgreSQL only: the schema holding our tables |
| `pool.max_connections` | int | `10` | server engines only; SQLite keeps one serialized connection by design |
| `pool.acquire_timeout_secs` | int | `30` | server engines only: how long a request waits for a pooled connection |

Plus one SQLite path key per section:

| Section | Key | Default | Holds |
|---|---|---|---|
| `infra.meta` | `dir` | `state/volumes` | one db per volume, `{dir}/{project_id}.db`. Its parent is the state root, from which the git and oauth paths are derived |
| `infra.admin` | `path` | `state/admin.db` | ACL registry (`project`, `project_member`) |
| `infra.git` | (derived) | `{state_root}/git/{project_id}.db` | git index (`git_objects`, `git_refs`, `git_remotes`) |
| `infra.oauth` | (derived) | `{state_root}/oauth.db` | encrypted OAuth tokens |

`infra.git` and `infra.oauth` are new sections. They were previously hardcoded to those
derived paths, which is still exactly what an absent section means, so no existing config
changes behaviour.

### Validation at boot

All four are checked when the config loads, because a silent misconfiguration here is the
real risk. Each failure is `ERR_INVALID_ARGUMENT` naming the offending key:

* `backend` is not one of the two accepted values;
* `backend` is `postgres` and `dsn` is empty;
* `dsn` is set while `backend` is `sqlite`, which never reads a dsn. Rejected rather than
  ignored, so a dsn that does nothing cannot look like it is in use;
* `backend` names an engine this binary was not built with. The message names the cargo
  feature to rebuild with, since the failure is a build choice and not a typo.

Pointing several sections at the same dsn is supported and is the expected deployment:
the table names do not collide, and one pool is shared per distinct backend, dsn and
schema triple.

### Example

```yaml
infra:
  meta:
    backend: postgres
    dsn: "${FSA_META_DSN}"
    schema: mcpfs
    pool: { max_connections: 20, acquire_timeout_secs: 10 }
  admin:
    backend: postgres
    dsn: "${FSA_META_DSN}"
    schema: mcpfs
  git:
    backend: postgres
    dsn: "${FSA_META_DSN}"
  oauth:
    backend: sqlite
```

## `infra.blob`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `backend` | string | `local` | `local`, or `minio` / `s3` |
| `dir` | string | `state/blobs` | local only: layout `{dir}/{bucket}/{sha[..2]}/{sha}` |
| `endpoint` | string | `""` | S3 only: endpoint URL; empty means the AWS default |
| `access_key` | string | `""` | S3 only |
| `secret_key` | string | `""` | S3 only. **Secret**: inject it with `${FSA_MINIO_SECRET_KEY}` |
| `bucket_prefix` | string | `mcpfs-` | bucket per volume: `{bucket_prefix}{project_id}` (also the local subdirectory name) |
| `region` | string | `us-east-1` | S3 only |

Path style addressing is always forced on S3 because MinIO requires it.


## `safety`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `write_quota_bytes` | int | `52428800` (50 MiB) | per session (`person` + project) write budget, charged before each write |
| `trash_dir` | string | `.mcp_trash` | soft delete destination inside the volume |
| `read_guard` | bool | `true` | require a read of a file in this session before editing it |
| `allow_hard_delete` | bool | `false` | when false, `fs.delete(trash=false)` is `ERR_NOT_SUPPORTED` |
| `max_read_lines` | int | `2000` | hard ceiling on `fs.read`; `limit_lines` is clamped to it |
| `max_diff_file_bytes` | int | `5242880` | largest file fs.diff_files accepts, per file; a value that is not a whole number above zero stops the boot |

## `api`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `enabled` | bool | `true` | mounts `/api/fs/**` plus `/api/swagger.json` and `/api/docs`. False means MCP only and all three 404 |

## `extract.ocr`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `provider` | string | `none` | `none` (images yield an empty result plus a note) or `multimodal` |
| `endpoint` | string | `""` | OpenAI compatible `chat/completions` URL |
| `model` | string | `""` | model name sent to that endpoint |
| `api_key_env` | string | `FSA_OCR_KEY` | **name of the environment variable** holding the key, never the key itself |
| `prompt` | string | `Transcribe this document faithfully into Markdown.` | instruction sent with the image |

The prompt and the key are request material only: they are never logged, never
echoed in an error and never returned to the caller, because tool output flows
straight into an LLM context.

## `doc_service`

An external, stateless document to Markdown converter, in two interchangeable
forms. It backs the `trigger_documentation_service` flag on byte uploads and the
`fs.documentize` tool: the companion lands at the exact path `fs.extract_text`
looks at (`toto.pptx` -> `toto.md`), so it is then served as a cache hit by the
built-in extractor. **Off by default**, so no existing deployment changes.

| Key | Type | Default | Meaning |
|---|---|---|---|
| `enabled` | bool | `false` | when false, the upload flag answers `ERR_NOT_SUPPORTED` rather than silently storing a file without its companion |
| `mode` | string | `cli` | `cli` or `api`. Anything else is `ERR_INVALID_ARGUMENT` naming the accepted values |
| `extensions` | list | `[]` | overrides the built-in eligible set when non empty, so a deployment can narrow it to what its converter really handles |
| `max_input_bytes` | int | `536870912` (512 MiB) | refuse to convert beyond this; checked **before** any byte is written. It also caps every channel of the converter's answer (text stdout, web body, bundle files): an answer past it is abandoned as soon as it passes (the child is killed and reaped before its folder is removed) and refused with `document service output is over the doc_service.max_input_bytes limit of <cap>` |
| `cli.command` | list | `[doc-convert, --quiet, -o, {outdir}, {document}]` | argv, never a shell string. Exactly one `{document}` and at most one `{outdir}` placeholder across the whole list; with `{outdir}` the bundle is read from that directory (`document.md`, files, `artifacts.json`), without it stdout is the Markdown (SPEC-0019 DEC-006, 2026-10-10) |
| `cli.timeout_secs` | int | `900` | wall clock ceiling for one conversion; on expiry the child is killed and reaped |
| `api.url` | string | `""` | `POST` target; must start with `http://` or `https://` |
| `api.auth_header` | string | `Authorization` | header **NAME**, not necessarily a bearer scheme |
| `api.auth_token` | string | `""` | **Secret**: inject it with `${VAR}`. Sent **verbatim**, so a bearer is written `auth_token: "Bearer ${TOKEN}"`. Omitted entirely when empty |
| `api.file_field` | string | `file` | name of the `multipart/form-data` part carrying the document, because a third party endpoint names it whatever it likes |
| `api.response_field` | string | `""` | empty means the response body IS the Markdown; otherwise the body is JSON and this names the field carrying it |
| `api.timeout_secs` | int | `900` | request timeout of the shared HTTP client, built once at boot |

The built-in eligible set is PowerPoint (`.pptx .pptm .potx .ppsx .ppt`), Word
(`.docx .doc`), PDF, audio (`.mp3 .m4a .wav .ogg .flac .aac .opus .wma`) and
video (`.mp4 .mov .mkv .webm .avi .m4v .mpeg .mpg`). `.xlsx` and images are
deliberately absent: `fs.extract_text` already covers them.

### Validation at boot

Checked only when `enabled` is true, because an unfinished draft must not stop a
boot that does not use the feature. Each failure is `ERR_INVALID_ARGUMENT` naming
the offending key:

* `mode` is not one of the two accepted values;
* `mode: cli` and `cli.command` is empty, or carries anything other than exactly
  one `{document}` placeholder (none means the converter is never told which file
  to read, two means it is handed a document it was never asked to merge);
* `mode: api` and `api.url` is empty or is not `http://` / `https://`;
* `api.auth_token` is set while `api.auth_header` is empty, which would silently
  drop the token and leave the operator believing the endpoint is authenticated;
* `api.file_field` is empty, which every server rejects on the first upload.

### Example, cli mode

```yaml
doc_service:
  enabled: true
  mode: cli
  max_input_bytes: 536870912
  cli:
    command: ["doc-convert", "--stdout", "--quiet", "{document}"]
    timeout_secs: 900
```

### Example, api mode

```yaml
doc_service:
  enabled: true
  mode: api
  extensions: [".pdf", ".pptx", ".docx"]
  api:
    url: https://convert.internal/v1/markdown
    auth_header: Authorization
    auth_token: "Bearer ${FSA_DOC_SERVICE_TOKEN}"
    file_field: file
    response_field: ""
    timeout_secs: 900
```

The request is `POST url`, `multipart/form-data`, one part named by
`api.file_field` (`file` by default) carrying the original filename and the MIME
guessed from its extension (`application/octet-stream` when unknown), Markdown
back on 2xx. Only the part name and the response shape are negotiable, which is
what `api.file_field` and `api.response_field` are for: a third party endpoint
needs no adapter of ours.

### Sandboxing, and its limit

In `cli` mode the converter is an arbitrary third party binary, so containment is
by construction: every call gets a fresh temporary directory; the input is written
inside it under a sanitized single segment name keeping the original extension;
the child runs with that directory as its working directory and `{document}`
expands to a **relative** `./name.ext`, so a converter that writes beside its
input (as `doc-convert` does, it creates `toto_docling/`) writes inside the
sandbox; `TMPDIR`, `TMP` and `TEMP` point there too; the command is an argv list
so there is no shell, no redirection, no `&&` and no glob; only stdout is read;
stderr is captured, capped at 8 KiB keeping the tail, and surfaced only on
failure; and the directory is removed on every exit path, including a timeout,
where the child is killed and reaped **before** the removal so no process is left
writing into a directory being deleted.

**This is not a hard OS sandbox, and it is not sold as one.** Containment is cwd
plus a relative path plus `TMPDIR` plus argv-without-shell. A converter that
writes to an absolute path or into `$HOME` escapes it and the cleanup will not
catch it. Full containment is the operator's call and is deliberately left there,
because a general purpose jail would break a converter that legitimately needs
network access and a model cache: wrap argv[0] in your own jail, which is exactly
why `command` is a list.

```yaml
doc_service:
  cli:
    command: ["docker", "run", "--rm", "-i", "my/doc-convert", "--stdout", "{document}"]
```

`env_clear` is deliberately NOT used: a converter legitimately needs `HOME`,
`PATH` and its model caches.

### Registry converters take precedence (SPEC-0106)

A converter of the registry (`admin.create_converter`, `/app/converters`) wins over this block: the
project converter first, then the platform default, then this `doc_service` converter, which stays the
fallback exactly as before and is still validated when the platform starts. With an empty registry nothing
changes. Details in `architecture.md` "Document converters registry".

## `git`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `enabled` | bool | `false` | registers the 18 git tools and the git HTTP routes |
| `object_format` | string | `sha1` | `sha256` is accepted and ignored (the bundled libgit2 is sha1 only) |
| `anonymous_read` | bool | `false` | allow unauthenticated clone and fetch |
| `max_pack_size_mb` | int | `512` | push body ceiling, enforced (413) |
| `github_client_id` | string | `""` | GitHub App client id for the `git.auth` device flow |
| `github_client_secret_env` | string | `FSA_GITHUB_CLIENT_SECRET` | name of the env var holding the GitHub secret |
| `gitlab_client_id` | string | `""` | GitLab application id |
| `gitlab_client_secret_env` | string | `GITLAB_CLIENT_SECRET` | name of the env var holding the GitLab secret |
| `gitlab_instance_url` | string | `https://gitlab.com` | base URL for a self hosted GitLab |
| `remote_timeout_secs` | int | `120` | deadline for one clone/push/fetch/pull; expiry releases the per-repository write lock (DEC-032) |
| `max_stash_entries` | int | `100` | how many `refs/stash/*` entries one volume may hold; the save that would exceed it is refused, never the oldest evicted (FR-NEW-130) |
| `github_scope` | string | `repo` | scope set the GitHub device flow requests (FR-NEW-331) |
| `gitlab_scope` | string | `api read_repository write_repository` | scope set the GitLab device flow requests; `api` is mandatory for REST API access, `write_repository` is Git over HTTP only (FR-MOD-109) |
| `max_rebase_todo` | int | `200` | how many entries one `git.rebase` todo may hold, and the ceiling the pre-flight range walk stops at (FR-NEW-216) |
| `max_pr_diff_mb` | int | `12` | MiB of unified diff `git.pr_diff` returns at most; past it the prefix is returned with `truncated: true`, never a refusal (FR-NEW-309) |

Details in [`.agent_docs/git.md`](git.md).

## `web` and `context7` (optional families, off by default)

`web.enabled` and `context7.enabled` (with `context7.api_url`) switch the `web.*` and `context7.*`
families on; while off, the tools are not offered and no remote request is made.

- 2026-11-12 (SPEC-0093, US-0237): `context7.resolve_library_id` and `context7.get_library_docs`
  read the service's answer through the capped reader. An answer that declares a length past
  16 777 216 bytes is refused before its body is read, and one that passes it while being read is
  abandoned (the connection is closed); both are `ERR_INVALID_ARGUMENT`
  `response from host '<host>' exceeds the 16 MiB limit`. A failure status of the documentation
  (404, 5xx) is still answered from the status alone, its body never read. At most the limit plus
  1 048 576 bytes are held during a call. The `web.*` answers follow the same limit (US-0235,
  US-0236).

- 2026-11-12 (SPEC-0093, US-0238 to US-0240): a `web.download` and a `web.fetch` with `mount_id` and
  `save_path` stop at the write limit, the lower of the project side (remaining storage, the replaced
  file counted out) and the session side (`session_remaining`), computed after membership and the path
  check and before the request; a tie goes to the project. The message is today's storage message
  (net growth for the project). The whole answer then goes through `fs_ops::write_bytes`. Both families
  are off by default; `web.test_base` exists only in test builds (`#[cfg(test)]`, `serde(skip)`), never in
  a configuration file. Order of refusals: argument checks, membership and path, request and read, write
  checks. Accepted: redirects and timeouts as before; address filtering (CWE-918) is out of scope.

- 2026-11-16 (SPEC-0097, US-0246): `web.black_list_url` and `web.white_list_url`, two lists of strings,
  empty by default. An entry is a host name (`docs.example`), a `*.host` name (`*.corp.example`), an
  IPv4 or IPv6 address, or an address range (`169.254.0.0/16`, `fd00::/8`). Every command that loads
  the configuration (`serve`, `purge`, `migrate`, source then destination) checks both lists, black
  before white, whether or not `web.enabled` is true, and refuses the first bad entry with
  `ERR_INVALID_ARGUMENT: invalid entry '<entry>' in the web <black|white> list: expected a host name,
  a *.host name, an IP address or an address range`. The two fields are left out of `WebConfig`'s
  `Debug`, so the frozen configuration snapshots do not change. Parsing is in `tools/web_access.rs`.

  Black list decision (US-0248, `web.fetch` returning text only for now; download, saved pages, the
  white list, name lookup and redirects follow in later stories): with at least one list set, the site
  is read as typed with the `url` crate (lower case ASCII, one trailing dot dropped, IPv6 brackets
  removed, an IPv4 mapped IPv6 address judged as its IPv4 address). A name entry matches by equality,
  `*.host` matches any name below it but not `host` itself, an address entry matches that address, a
  range entry matches every address inside it. A match is refused before any request is sent with
  `ERR_FORBIDDEN: host '<site>' is not allowed by the web access lists`; the message never shows a
  resolved address. Both lists empty takes the exact path of before. A configuration that skipped
  `validate` and holds a bad entry makes every `web.fetch` and `web.download` call return the entry
  error instead of reaching the network.

  Normalization (US-0249): entry and request are reduced to one form before comparing. Letter case
  never matters; one trailing dot is dropped; an accented entry is converted to its `xn--` form with
  the `url` crate, so `bücher.example` covers `xn--bcher-kva.example` while `straße.example` stays
  distinct from `strasse.example`; the refusal shows the site only, never the port or the
  credentials (`http://u:p@Blocked.example:8080/` gives `host 'blocked.example'`). An IPv4 mapped
  entry (`::ffff:127.0.0.1`) covers the plain address; an IPv6 range of 96 bits or more starting
  with `::ffff:0:0` is the IPv4 range of the bits left (`::ffff:10.0.0.0/104` is `10.0.0.0/8`); any
  other IPv6 range, `::/0` included, covers native IPv6 addresses only; bits below the prefix are
  ignored (`10.0.0.5/24` is `10.0.0.0/24`). `web.search`, `web.news`, `web.suggestions`, the git
  tools and `context7.*` are not affected by the lists (DEC-014).

  White list (US-0250, `web.fetch` returning text; same scope limits as above): with a white list set,
  only a site matching one of its entries is served. Black is decided first, then the white rule, so a
  site in both lists is refused. An IP literal is judged against the white entries directly; a name
  is served when it matches a white name entry or when every address it resolves to matches a white
  entry (see name lookup below). The four combinations:

  | Black list | White list | A site is served when |
  |---|---|---|
  | empty | empty | always (the original path, untouched) |
  | set | empty | it matches no black entry |
  | empty | set | it matches a white entry |
  | set | set | it matches no black entry and matches a white entry |

  Saves (US-0251): `web.download` and `web.fetch` with `mount_id` and `save_path` obey the lists too.
  Order of refusals: argument, membership and path, the lists, the storage allowance, the request.
  So a refused save never creates or changes a file, never charges the session or the project, and
  wins over a full project. The save mode of `web.fetch` has no `http://` prefix check, so the lists
  judge `ftp://blocked.example/` too; an unlisted `ftp://` address then fails as before with
  `ERR_INTERNAL_ERROR: fetch request failed`.

  Name lookup (US-0252, 2026-11-16): with a list set, a name the black list does not refuse by name
  is looked up once (`tokio::net::lookup_host`), bounded by the call's `timeout_secs`; lookup, hops
  and body read share that one deadline. Black refuses when any resolved address matches (an IPv4
  mapped IPv6 answer is judged as IPv4); white serves when the name matches a white name entry or
  the name has at least one address and every one matches (an empty answer is never white listed by
  address). A lookup error or an empty answer is "no address": the request is not sent and fails
  with `ERR_INTERNAL_ERROR: fetch request failed: no address for host '<site>'` unless the lists
  refused first. A lookup that overruns the deadline fails with `ERR_INTERNAL_ERROR: request timed
  out`. The sender pins the connection to the checked addresses with `resolve_to_addrs`, so a name
  answering differently the second time cannot reach a refused address. Both lists empty: no lookup.
  Known limit (owner decision, BL-0157): the environment proxy is kept (no `no_proxy()`); behind a
  proxy the proxy resolves the name itself, so pinning does not hold there. Test seam: `TestNet`
  name table (`Addrs`, `NoAddress`, `Hang`, last answer repeats, lookup counter); a name absent from
  the table has no address, so tests never reach the real resolver.

  Request reading and the proxy (US-0253, 2026-11-17): the site is read by the `url` crate (WHATWG),
  so `127.1`, `0177.0.0.1`, `0x7f.1` and `2130706433` are the site `127.0.0.1`, and
  `[::ffff:127.0.0.1]` is the site `::ffff:7f00:1` judged as `127.0.0.1`; the refusal names that
  form. An address with no host, no scheme, an IPv6 zone (`[fe80::1%25eth0]`) or a host that cannot
  be converted to ASCII is not judged by the lists: the sender parses with the same crate, so it is
  never sent and fails with today's `ERR_INTERNAL_ERROR: fetch request failed: ...` text. The per
  request client is built by `AccessLists::client` (`tools/web_access.rs`): pin, then the test proxy
  seam (`TestNet::with_proxy`). Tested proxy limit: behind a proxy the lists still refuse on the
  typed site and on the addresses the server resolves, and the proxy receives nothing; an allowed
  request is opened by the proxy, so the pin does not apply to it.

  Redirects (US-0254, 2026-11-17): with a list set the client follows no redirect itself
  (`Policy::none`); `AccessLists::send` follows 301, 302, 303, 307 and 308 by hand, at most 10 hops
  like the default policy. The destination is read against the hop that sent it (`Url::join`, so
  relative and `//host/x` work) and admitted like a first request, on any scheme, before the hop
  limit: a refused eleventh hop is the lists refusal, a loop of allowed hops keeps `fetch request
  failed: error following redirect ... too many redirects`, an allowed `ftp://` hop fails as today.
  A redirect with no readable `Location` is the answer. Each hop has its own lookup and pin and the
  time left of the call. Both lists empty keeps the client's own redirect policy.

  Entry grammar (US-0247), every other shape is refused at boot:

  | Form | Accepted | Refused |
  |---|---|---|
  | name | letters, digits, `-`, `_`; label up to 63, name up to 253; one trailing dot ignored; `xn--` must decode | spaces, empty label, `blocked.example..`, `.`, `xn--`, 64 character label, a port, a scheme |
  | `*.name` | `*` only as the whole first label | `*`, `a*.b` |
  | numeric name | only a four part dotted address | `2130706433`, `0x7f000001`, `10.1`, `010.0.0.1` |
  | address | `10.0.0.5`, `fd12::1` | `[fd00::1]`, `fe80::1%eth0` |
  | range | `address/length`, one to three digits, no sign, no leading zero, within 32 or 128 | `10.0.0.0/08`, `10.0.0.0/+8`, `10.0.0.0/33` |

  `purge` refuses a bad entry like `serve` before opening any store; a purge with valid lists runs
  as before. `migrate` checks the source first, then the destination.

## `search.embedding` and the registry default (SPEC-0102)

`search.embedding` is the fallback: while the platform settings hold a default `text_embedding`
model, indexing and queries call its provider (`endpoint + "/embeddings"`, its secret as bearer)
instead, from the next call on, no restart. Clearing the default returns to this block.
`dimensions` always sets the vector table width. Details in `search.md`, "Embedding source".

## `search.settle_delay_seconds` (SPEC-0100)

Whole seconds, 0 or more, that a file must stay unchanged before its derived data is
rebuilt. Absent or empty means 30; `0` is valid. A YAML integer, float or string is read as
text and parsed in `validate()` before the `search.enabled` early return, so it is checked
even with search off. Anything else (`-5`, `2.5`, `abc`) stops the boot with
`ERR_INVALID_ARGUMENT: settle delay must be a whole number of seconds, 0 or more`.
Read only through `SearchConfig::settle_delay()`. No CLI flag. `fs.recalculate` ignores it:
a rebuild by hand, of kind `search_index` or `chunks`, starts at once (or as soon as the running
one of its kind ends).

## `search.chunking.*` (SPEC-0101)

Server defaults of the chunk sets, five keys, each an integer, float or string scalar
(`${VAR}` expanded first), read as text and parsed in `validate()` before the
`search.enabled` early return, so a bad value stops the boot even with search off.

| Key | Default | Range |
|---|---|---|
| `fixed_size_tokens` | 2048 | 32..8192 |
| `fixed_overlap_tokens` | 0 | 0..8191 |
| `heading_cut_depth` | 2 | 1..6 |
| `max_tokens` | 2048 | 64..8192 |
| `list_max_items` | 40 | 2..200 |

Absent or empty means the default. Out of range or not a whole number:
`ERR_INVALID_ARGUMENT: search.chunking.<key> must be a whole number between <min> and <max>`.
An overlap not below the size: `ERR_INVALID_ARGUMENT: search.chunking.fixed_overlap_tokens must be
smaller than search.chunking.fixed_size_tokens`. Read only through `SearchConfig::chunking()`
(a `chunking::ChunkDefaults`). No CLI flag.

## Derived paths

Under the SQLite backend, only `infra.meta.dir`, `infra.blob.dir` and
`infra.admin.path` are configurable; everything else is derived, so moving
`infra.meta.dir` moves the whole state tree. A store on PostgreSQL uses
its dsn instead and ignores these paths, except `git_repo_dir`, which stays on disk
because libgit2 needs a real working directory.

| Helper | Value |
|---|---|
| `volume_meta_path(p)` | `{infra.meta.dir}/{p}.db` |
| `volume_bucket(p)` | `{infra.blob.bucket_prefix}{p}` |
| `admin_db_path()` | `infra.admin.path` |
| `state_root()` | parent of `infra.meta.dir` (so `state/` by default) |
| `git_db_path(p)` | `{state_root}/git/{p}.db` |
| `git_repo_dir(p)` | `{state_root}/git-repos/{p}/` |
| `oauth_db_path()` | `{state_root}/oauth.db` |

## `${VAR}` expansion

Expansion runs on the **raw text before the YAML is parsed** (`config::expand_env`),
so any value in the file can come from the environment:

| Form | Result |
|---|---|
| `${VAR}` | the value of `VAR`, or the empty string when unset or empty |
| `${VAR:-default}` | the value of `VAR`, or `default` when unset **or empty** |
| `$notavar`, `${}`, `${1BAD}` | left untouched (a name must be alphanumeric plus `_` and must not start with a digit) |

An empty variable counts as unset, which is why `FSA_MINIO_ACCESS_KEY=` in a
`.env` still falls back to the template default.

## Secrets policy

Secrets never live in a committed file. They reach the process through the
environment only: either expanded into the YAML with `${VAR}`, or read at the
composition root by name.

| Variable | Used by |
|---|---|
| `FSA_MINIO_SECRET_KEY` | `infra.blob.secret_key` in `config/minio.yaml.template` |
| `FSA_GITHUB_CLIENT_SECRET` | the `git.auth` GitHub device flow (name configurable) |
| `FSA_TOKEN_KEY` | the master key: when set, OAuth tokens are persisted encrypted (AES-256-GCM, 32 byte base64 key), and model provider secrets can be stored (SPEC-0102); unset means memory only tokens and providers without secrets |
| any name you choose | `doc_service.api.auth_token`, for example `${FSA_DOC_SERVICE_TOKEN}` |
| any name you choose | the `dsn` of a relational store, for example `${FSA_META_DSN}` |

The master key (2026-11-22, SPEC-0102 US-0283). `FSA_TOKEN_KEY` (legacy `MCPFS_TOKEN_KEY`) is read once at
boot, after the admin store connects, by `app::boot_master_key`: unset or blank means no key (providers
without a secret are accepted, a secret is refused with `ERR_NOT_SUPPORTED: no master key is configured;
set FSA_TOKEN_KEY`); a value that is not base64 of 32 bytes stops `serve` with `error: FSA_TOKEN_KEY must be
valid base64.` or `error: FSA_TOKEN_KEY must decode to 32 bytes ...` and exit status 1, whether or not git
is enabled. The valid key goes to the admin store through `AdminBackend::set_master_key`. Generate one
with `openssl rand -base64 32`. The key is never stored; provider secrets are kept as `secret_enc` (nonce,
tag, ciphertext) and travel in memory as `settings::SecretText`, whose `Debug` prints `Secret(set)`.

A **dsn is a secret**, because it usually carries a password. `dsn` is typed as `Dsn`,
not `String`, and that type redacts itself in both `Debug` and `Display`, printing
`Dsn(redacted)` or `redacted`. So a dsn cannot reach a log line, a boot banner or an
error message even by accident, and there is a test asserting it. Never format a dsn
with `{:?}` expecting to see it, and never add a field that prints one.

`doc_service.api.auth_token` is typed `Secret`, the same newtype contract for a
credential that is not a connection string: `Debug` prints `Secret(redacted)` or
`Secret(unset)`, `Display` prints `redacted` or `unset`, and `expose()` is the
single audited read point, used once where the request header is built. A test
asserts the token does not appear in a `{:?}` of the whole config.

* `.env` is **gitignored**; `.env.example` is tracked as the template. `run.sh`
  sources `.env` when present.
* `config/local.yaml` is **gitignored** (personal working copy). Two templates are
  tracked: `config/local.yaml.template` (SQLite plus local blobs, zero external
  service) and `config/minio.yaml.template` (SQLite plus MinIO/S3 blobs, secret
  through `${FSA_MINIO_SECRET_KEY}`). `run.sh` bootstraps the first one on a
  fresh checkout.
* `state/` and `.keys/` are gitignored too.
* No secret is ever logged: tokens redact themselves in `Debug` output.

## Config path resolution

The config file is mandatory. `fsa serve` picks it in this order, highest priority
first, which is the same CLI then environment then file pattern the rest of the CLI
follows:

| # | Source | Behaviour |
|---|---|---|
| 1 | `--config PATH` (also `-c`) | used as given |
| 2 | `$FSA_CONFIG` (a full path) | used as given |
| 3 | `~/.config/fsa/config.yaml` | probed, used only when it exists |
| 3b | `~/.config/mcp-fs/config.yaml` (deprecated) | probed after 3; when it is the one loaded, one warning line is printed |
| 4 | `${FSA_CONFIG_DIR:-config}/${FSA_CONFIG_NAME:-local}.yaml` | probed, used only when it exists |

Cases 1 and 2 are explicit, so a missing file is reported by the name you gave. Cases 3,
3b and 4 are probed, and when none exists the error lists every path tried rather than
naming just one.

### Deprecated names (SPEC-0099, removed at release 1.0.0)

The product was renamed from `mcp-fs` to `fsa`. These old names still work and print one
`warning: environment variable <OLD> is deprecated, use <NEW>` line on standard error per
process (a blank value counts as unset; the new name wins when both are set):

| Old | New |
|---|---|
| `MCP_FS_CONFIG`, `MCP_FS_CONFIG_DIR`, `MCP_FS_CONFIG_NAME` | `FSA_CONFIG`, `FSA_CONFIG_DIR`, `FSA_CONFIG_NAME` |
| `MCPFS_TOKEN_KEY` | `FSA_TOKEN_KEY` |
| `MCP_FS_OCR_KEY` (default of `extract.ocr.api_key_env`) | `FSA_OCR_KEY` |
| `MCPFS_GITHUB_CLIENT_SECRET` (default of `git.github_client_secret_env`) | `FSA_GITHUB_CLIENT_SECRET` |
| `~/.config/mcp-fs/` | `~/.config/fsa/` |

The two defaults fall back only when the setting is absent from the configuration; a value
written there, even a default name, is used as written. `MCPFS_MINIO_*`, `MCPFS_META_DSN`,
`MCPFS_PUBLIC_URL`, `MCPFS_DOC_SERVICE_TOKEN`, `MCPFS_EMBEDDING_KEY` and `MCPFS_RERANK_KEY` were
only used by templates and developer tests and have no fallback: use the `FSA_` names.

Not renamed on purpose, because existing deployments store them: the bucket prefix `mcpfs-`,
the trash directory `.mcp_trash`, the git refs `refs/mcpfs/` and `refs/mcp-fs/rebase-head`,
the stash trailer, the lock key `mcpfs_migrate:`, the cookie `mcpfs_token`, and the test
database `mcpfs`. The developer variables `MCPFS_TEST_*`, `MCPFS_REWRITE_*`, `MCPFS_CHILD_CASE*`
and `MCPFS_PRINT_RECORD` keep their names for now.

Step 3 honours `$XDG_CONFIG_HOME` when it is set and falls back to `$HOME/.config`,
resolved through `etcetera::choose_base_strategy()` rather than a hand-rolled lookup
(SPEC-0012, DR-005). Because it only applies when the file exists it cannot disturb a
checkout that relies on the working directory relative `config/local.yaml`.

The YAML parse itself is still `serde_norway::from_str`, not `figment`: figment would
collapse a duplicate `git.hosts` key before the deliberately duplicate-preserving
`HostMap` parser could reject it (see `.agent_docs/git.md`'s host map section and
`specs/archived/SPEC-0012_2026-09-26_16-04-00-rust-skill-compliance-debt/drift/2026-09-26_17-19-40.md`,
DDRIFT-002).

A missing or unreadable file is `ERR_INVALID_ARGUMENT`; a YAML error reports the parse
failure. The keypair is unrelated to this resolution: `fsa keys --dir .keys` writes
`jwt.key` and `jwt.pub`, and `auth.jwt.public_key_path` must point at the `.pub` file.

(2026-11-22, SPEC-0102 US-0289) No new key. The registry is edited through the `admin.*` tools, `/api/admin/*` and the
`/app/models` screen; the master key `FSA_TOKEN_KEY` is needed to store a provider secret from any of them.

## Server settings in the database (SPEC-0114, 2026-11-26)

Part 1 of BL-0173 (SPEC-0115 added `git`, `web`, `context7`, below). Sections `safety`, `api`, `extract`, `doc`, `sqlite`, `db`, `doc_service`, `search` are rows of the admin table `server_setting(section, value, secret_enc, updated_at, updated_by)`. `value` is the JSON of the section struct (the YAML names), without its secrets; `secret_enc` is one AES 256 GCM blob of a JSON object `{"api.auth_token": "..."}` under `FSA_TOKEN_KEY` (the one secret of these sections is `doc_service.api.auth_token`; the embedding, reranking and OCR keys stay names of environment variables).

- Read and write: `settings::ServerSettings` (`list`, `get`, `set`). A set merges the patch over the section, refuses an unknown key (`unknown key 'api.nope' in setting 'doc_service'`) or a wrong type (`invalid setting '<section>': <serde reason>`), builds a candidate `ServerConfig` and runs `ServerConfig::check()` (every check of `validate()` except publishing the git host map), then writes the canonical JSON and one `setting_change` line (`action` `set_setting`, `target_kind` `setting`, `fields` the changed key paths) in one transaction. An empty text clears a secret, an absent key keeps it.
- Applied at the next start: no consumer reads a changed section live. `restart_required` compares the stored section with the one the running process holds.
- Boot (`settings::server_settings_boot::apply`, called by `app::build_with_env` after the master key is set): a stored row replaces its section; with no row, a configuration read from a YAML file (`ServerConfig::from_yaml_file`, set by `load`) seeds the row once (`updated_by` `system:yaml`, secret encrypted; a secret with no master key is not imported and the YAML value stays in force); a YAML section that differs from a stored one logs a warning and the database wins; a stored secret that cannot be decrypted stops the boot (`the stored setting '<section>' holds a secret that cannot be read: set FSA_TOKEN_KEY to the key it was stored with`); then `check()` and the forced switches (`ServerConfig::force`, flags `--sqlite --db --doc --git --web --context7`) are applied again.
- `fsa serve --database-url URL` / `FSA_DATABASE_URL`: `ServerConfig::set_database_url` (`postgres://`, `postgresql://`, `sqlite:PATH`) replaces `infra.admin`; with it the config file is optional (`cli::load_serve_config_with`). No file and no URL: today's refusal.
- Not yet read from the database: `server`, `auth`, `infra` (BL-0180, BL-0181). `fsa purge`, `fsa migrate` and `fsa keys` read the file only. `fsa migrate` copies the table with the rest of the admin schema.
- Gotcha: `ServerConfig::validate()` publishes the git host map into a process global; a settings change must use `check()`, never `validate()`. Tests that call `ServerConfig::load` or `from_yaml` take `git::remote::tests::lock_for_test()`.
- `ServerConfig` has a hand written `Debug` of the 14 sections only (the two origin fields `from_yaml_file` and `forced` are `serde(skip)`), so the recorded Debug values of the config tests do not move.
- A switch forced by a flag is never stored: `ServerConfig::force` keeps the value it had (`unforced`), the seed and `ServerSettings` use `without_flags()`, and a stored section replacing a forced one becomes the value to go back to (`note_stored_switch`). So `fsa serve --doc` once does not leave `doc.enabled` on in the database (review finding, test `e2e_0114_015b_`).
- A section that holds a secret read from the YAML and no master key cannot be rewritten (it would drop the file's secret): `set` answers `no master key is configured; set FSA_TOKEN_KEY` for any key of that section. `fsa purge` needs `FSA_TOKEN_KEY` when a stored secret exists (a purge with the wrong settings could remove projects too early, so it refuses instead of falling back to the file).

## Git, web and context7 in the database (SPEC-0115, 2026-11-26)

Part 2 of BL-0173. `SECTIONS` has eleven names; `section_json` and `with_section` have three more arms (the `web` arm keeps the `serde(skip)` test seams `test_base` and `test_net` of the running value, `WebConfig::keep_test_seams_of`). No secret path: provider client secrets are environment variable names, host credentials live in the OAuth token store (`git_auth::token_store`), so a host list change never touches a token (`e2e_0115_004`, `029`, `044`).

- `git.hosts` is a free key map (`server_settings::free_paths`): `check_keys` does not descend into it and a change replaces the stored map after `merge` (an object merge could never remove a host). `HostMap` (`git/remote.rs`) serializes as an object sorted by key (the order of the stored value and of every read; `serde_json` here has `preserve_order`, so the sort is explicit) and deserializes by `deserialize_any`: a map as before, or a list of `[host, provider]` pairs, which keeps a repeated host alive until `check_hosts` refuses it (a JSON object cannot hold one). The YAML duplicate test (`e2e_new_006`) is unchanged and passes through the map path.
- Keys are kept as typed; `tools/git.rs` lowercases the host of a remote URL, so a stored key with capitals is never matched (`e2e_0115_049`). Reported, not changed.
- Boot: `server_settings_boot::apply` calls `git::remote::validate_hosts` after the checks when a stored `git` row gives a host list that differs from the one the configuration held (a YAML start already published its own map at load, and its seed row equals it; an in memory configuration keeps what its caller published, which the `token_screen` tests rely on). Publishing on every applied row made each second start of a test republish the map without the lock and broke the `token_screen` tests in every full run (`e2e_0115_056`). `config.rs` must keep exactly one `validate_hosts(` (`e2e_new_224`).
- Every key is restart flagged, `max_pack_size_mb` included (`git/http/mod.rs` builds its body limit once). `--git`, `--web`, `--context7` force the switch on and are never stored (`e2e_0115_013`, `031`, `046`).
- Gotchas of the tests: a test that holds `git::remote::tests::lock_for_test()` must not call `load_yaml` of `server_settings_e2e` (it takes the same lock, a deadlock); a raw `server_setting` write reads before it opens its transaction (one connection); `boot_config` writes a new key pair each call, so a token is minted after the boot it is used on; the web seam fetches over `http://` only.
- Process note: tests that publish the global host map without `git::remote::tests::lock_for_test()` can still break the `token_screen` family in a full run; the web list start tests and `e2e_0114_017b` were fixed here (a lock line each), `config.rs` tests that read a YAML are the remaining unlocked readers (none failed after the fix above).
