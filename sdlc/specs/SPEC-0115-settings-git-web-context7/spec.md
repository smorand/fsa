# Git, web and context7 settings in the database: functional specification

> Id: SPEC-0115
> Nature: FEAT
> Depth: M
> Depth evidence: FEAT / depth M: 3 areas (the three sections and their rules, the start of the server, the screen and contract), 17 requirements, no new table, no new tool, no new dependency. Auto run 2, part 4 of 6 (BL-0173 part 2/4; BL-0179).
> Status: implemented
> Generated on: 2026-11-26
> Project: existing
> From backlog: BL-0179
> Depends on: SPEC-0114, SPEC-0097, SPEC-0052
> Security: no secret value lives in these three sections; credentials of git hosts stay in the encrypted token store; the write only rule of SPEC-0114 still guards any future secret key
> CVSS: not scored
> Affected: n/a
> Fixed in: n/a

## 1. Summary
Three more sections of the YAML configuration, `git` (hosts and limits), `web` (with the two access lists of SPEC-0097) and `context7`, become settings a platform administrator reads and changes with the three tools, the routes and the screen of SPEC-0114, and the server reads at the next start. The YAML stays an optional seed and the fallback, a stored value wins, and a server started with nothing but the admin database URL and the master key serves the stored hosts, limits and lists. Nothing new is invented: the same store, the same rules, the same change record.

## 2. Current State (MANDATORY)
Eight sections are settings since SPEC-0114. `git`, `web` and `context7` are still read from the YAML file only. A server started from a database URL alone runs with the defaults for them: git, web and context7 off, no declared git host, no access list. The git host map is checked at the start and published once; it is what every remote operation resolves hosts with.

### 2.1 What users can do today
- Declare a git host, change a git limit, set a web list or switch context7: edit the YAML and restart (14.1 items 1 and 2).
- Start with a database URL alone and get git hosts or web lists: impossible (14.1 item 3).
- Learn which hosts the server trusts: read the file.

### 2.2 Specs governing this area
SPEC-0114 (store, rules, screen, boot), SPEC-0097 (web lists, checked at the start), SPEC-0052 and the git remote specs (hosts), SPEC-0102 (secrets under the master key).

### 2.3 What is verified today
The host map by the tests of the git remote module, among them the refusal of a repeated host at the start; the lists by the SPEC-0097 suite; the three sections by the configuration tests; the store by the SPEC-0114 suite (identifiers in 14.1).

## 3. Scope
### 3.1 In scope
- The sections `git`, `web`, `context7` in the list, get and set tools, the routes, the screen.
- Their start time overlay, publication of the host map, the YAML seed, the flags that force a switch.
### 3.2 Out of scope, and why
- Applying a change without a restart: same rule as SPEC-0114, every consumer reads its section once at the start (AUTO-038). This covers the pack size limit of git too.
- The OAuth store, the token screen, the environment variable names of provider client secrets: unchanged. Part 6 moves the stores.
- Sections `server`, `auth`, `infra`: parts 5 and 6.
### 3.3 Constraints imposed by the context
YAML alone behaves as today. The existing configuration tests and the repeated host test are not modified. The tool contract keeps its tool count; only descriptions that list the sections change.

## 4. Actors
| Actor | Role |
|---|---|
| Platform administrator | reads and changes the three sections |
| Other member | refused |
| Operator | starts the server with a database URL and the master key |

## 4.5 Bounded contexts
Platform settings (SPEC-0114), git remote hosts, web access lists, server start.

## 5. Usage scenarios
### 5.1 Happy
- **S1** The administrator declares the host `github.example.com` as a `github` host: the answer says a restart is required; after the restart (git switched on) a clone from that host is accepted.
- **S2** The administrator sets the web black list to `["*.ads.example"]` and the white list to `[]`: after the restart the web tools refuse `x.ads.example`.
- **S3** An operator starts with only the database URL and the master key: the stored host, limits and lists are in force and the git remote tools see the host.
- **S4** The administrator replaces the host list with one host: the other hosts are gone after the restart and the tokens people stored for them are still stored.
- **S5** The first start from a YAML file imports the three sections once; a later YAML edit does not change them.
### 5.2 Failures
- **F1** A host list naming `github.com` twice is refused with `git.hosts contains duplicate host 'github.com'`, nothing is stored.
- **F2** A white list entry that is not an accepted form is refused with the message the start gives, nothing is stored.
- **F3** A host written with a scheme, a path, a port or a wildcard, or a provider that is not one of the four, is refused with the start's message.
- **F4** A key that does not exist, such as `git.github_client_secret`, or a value of the wrong type, is refused naming the section.
- **F5** A member who is not a platform administrator is refused on the three tools, the routes and the screen.
- **F6** An empty database with no YAML starts with git, web and context7 off.
### 5.3 Edge
- **E1** `git.hosts` given as an empty object removes every host; given again with the same hosts it changes nothing.
- **E2** The flags `--git`, `--web`, `--context7` switch the section on over a stored `enabled: false`, and the switch is never stored.

## 6. Functional requirements
### 6.1 New
- **FR-NEW-001** The sections `git`, `web` and `context7` are setting sections: the list names eleven sections in this order: safety, api, extract, doc, sqlite, db, doc_service, search, git, web, context7; the refusal of an unknown section names the same eleven.
- **FR-NEW-002** A section reads and writes under the names of its YAML keys; the value of a section never set is the one the server runs on; `git.hosts` reads as an object of host to provider.
- **FR-NEW-003** When a change gives `git.hosts`, the given hosts replace the whole host list (they are not merged into the stored list); the value is accepted as an object of host to provider or as a list of `[host, provider]` pairs.
- **FR-NEW-004** A change is accepted only when the section as it will be passes the checks of the start: the host key, the provider, a repeated host (`git.hosts contains duplicate host '<host>'`), each access list entry, and every other check of the configuration, with the same message.
- **FR-NEW-005** A change to a git, web or context7 key that holds a secret is refused as an unknown key; these sections hold no secret value, and the provider client secret keys hold the name of an environment variable.
- **FR-NEW-006** A change to the three sections applies at the next start and the answer says `restart_required` while the stored section differs from the running one.
- **FR-NEW-007** At the start, a stored `git`, `web` or `context7` section replaces the one from the YAML or the defaults, and the git host list in force is the stored one.
- **FR-NEW-008** A server started with a database and no row for these sections, and no YAML, has git, web and context7 off, no host and no access list.
- **FR-NEW-009** A change to the host list does not change, delete or read any stored git token.
- **FR-NEW-010** A start from a YAML file imports each of the three sections that has no row, once; a later YAML edit is ignored while a row exists, with the warning of SPEC-0114.
- **FR-NEW-011** The flags `--git`, `--web`, `--context7` still force the switch on over a stored value, and a forced switch is never persisted.
- **FR-NEW-012** The change record holds the names of the changed keys only, never a value. A key of the host list is named `hosts.<host as typed>`; a host added, removed, or whose provider changed counts as one changed key; a host kept with the same provider is not named. Other keys are named by their YAML key (`black_list_url`, `github_client_id`). An accepted change that changes no key adds one record line with no key; a refused change adds none.
- **FR-NEW-013** The settings screen lists and edits the three sections like the eight others.
- **FR-NEW-014** The behavior is the same on SQLite and on PostgreSQL.
- **FR-NEW-015** A host key is stored and read back exactly as typed, with no change of letter case; two keys that differ only by letter case are two hosts and both are accepted; a host is matched by exact comparison of the key as typed against the host of a remote URL in lower case, as today (so a stored key with capitals is never matched by a remote operation).
- **FR-NEW-016** `git.hosts` reads as an object whose keys are in ascending byte order of the key as typed (`GitHub.com` before `github.com`); a list of pairs whose element is not exactly two texts, and a `hosts` that is neither an object nor a list of pairs (a text, a number, `null`), are refused with a message that begins `invalid setting 'git':`, nothing is stored.
- **FR-NEW-017** A change that sets both web lists is accepted when each list passes the checks of the start, and the decision on a site is the one of SPEC-0097, whichever way the lists were set (YAML or database).

## 7. Non functional requirements
Security and access: platform administrators only, reading included (SPEC-0114). Reliability: a stored value the start refuses stops the start naming the section. Operability: a repeated YAML difference logs one warning, as in SPEC-0114.

## 8. Edge cases
Empty host list; a host key in capitals; the same host in two spellings; a white list and a black list set together; a stored section that no longer passes the checks at the start; two administrators saving at once (last write wins, both recorded, SPEC-0114); a section never set read after a YAML start.

## 9. Acceptance tests (the contract)
### 9.1 Summary
54 tests: 12 happy (001 to 005, 014, 019, 020, 021, 049, 051, 054), 22 failure (006 to 011, 017, 023 to 025, 027, 028, 029, 035, 036, 041, 043, 045, 046, 048, 052, 053), 20 edge or side effect (012, 013, 015, 016, 018, 022, 026, 030 to 034, 037, 038, 039, 040, 042, 044, 047, 050). 17 requirements, each with at least 3 tests (section 10 is generated from the Covers column). Failure tests outnumber happy ones, 22 to 12. Test names are `e2e_0115_NNN`. Names such as `restart_required`, the error codes and the flags are the public contract fixed by SPEC-0114 and the command line, not design choices. A message quoted below is compared whole; where only a start is quoted, the test compares that start.

### 9.2 New tests
| Id | Covers | Given / When / Then |
|---|---|---|
| E2E-001 | S1 FR-NEW-002 FR-NEW-006 | Given an administrator and a server with no host. When she sets `github.example.com` to `github`. Then the answer holds `{"github.example.com": "github"}`, `restart_required` is true, source is `database`, and a read gives the same answer. |
| E2E-002 | S2 FR-NEW-002 FR-NEW-006 FR-NEW-017 | Given web on (stored `web.enabled` true). When she sets the black list `["*.ads.example"]`. Then the answer holds the list, the key not given (`max_results` 10) keeps its value, and before the restart a `web.fetch` of `https://x.ads.example/` is not refused by the lists; after the restart it answers `host 'x.ads.example' is not allowed by the web access lists`. |
| E2E-003 | S3 FR-NEW-007 | Given a stored `git.enabled` true, a stored host, a stored pack size limit of 64, a stored black list, a stored `web.enabled` true and a stored `context7.enabled` true, and the server stopped. When it starts with only the database and the master key. Then the host resolves as `github`, the pack limit is 64, a `web.fetch` of `https://x.ads.example/` answers `host 'x.ads.example' is not allowed by the web access lists`, context7 is on. |
| E2E-004 | S4 FR-NEW-003 FR-NEW-009 | Given git on, hosts `github.com` and `gitlab.example.com` and a stored token for `gitlab.example.com`. When she sets the hosts to `{"github.com": "github"}`. Then the stored list is that one host (not merged), and the token of `gitlab.example.com` is still listed with the same last 4 characters; the record line of that set holds exactly the key `hosts.gitlab.example.com`. |
| E2E-005 | S5 FR-NEW-010 FR-NEW-012 | Given a YAML with host `a.example.com`. When the first start runs, then the YAML is edited to host `b.example.com` and the server starts again. Then the host in force is `a.example.com`, and the record holds, among the eleven imports of the first start, exactly one import line for each of `git`, `web` and `context7`, each made by `system:yaml`. |
| E2E-006 | F1 FR-NEW-004 | When she sets hosts as the pairs `["github.com","github"]` and `["github.com","gitlab"]`. Then she is refused with `git.hosts contains duplicate host 'github.com'` and nothing is stored; the repeated host in a YAML file is still refused at the start by the unchanged test of the git module. |
| E2E-007 | F2 FR-NEW-004 FR-NEW-017 | When she sets the white list `["not a site"]`. Then she is refused with `invalid entry 'not a site' in the web white list: expected a host name, a *.host name, an IP address or an address range`, nothing is stored; a YAML file holding the same entry still stops the start with an error whose text contains that message. |
| E2E-008 | F3 FR-NEW-004 | When she sets one at a time the hosts `https://github.com`, `github.com/org`, `github.com:443`, `*.github.com`, and the provider `githib`. Then the refusals are: for the first three `git.hosts key '<host>' is not a bare hostname (no scheme, path, or port allowed)` followed by the parser's reason; for the fourth `git.hosts key '*.github.com' contains a wildcard character: host matching is exact only, one key per hostname`; for the fifth `git.hosts entry 'github.com' has unknown provider 'githib', expected one of github, gitlab, generic, anonymous`; nothing is stored. |
| E2E-009 | F4 FR-NEW-005 | When she sets `github_client_secret`, `context7.nope`, `max_pack_size_mb` as `"big"`, `web.enabled` as `"yes"`. Then the first is refused with `unknown key 'github_client_secret' in setting 'git'`, the second with `unknown key 'nope' in setting 'context7'`, the others with a message that begins `invalid setting 'git':` and `invalid setting 'web':`; nothing is stored; a read of each of the three sections shows no secret entry. |
| E2E-010 | F5 FR-NEW-013 | Given Bob, a member who is not an administrator. When he lists, reads or sets one of the three sections by tool and by route, and opens the screen. Then each is refused with `ERR_FORBIDDEN: 'bob@example.com' is not a platform admin` (the screen at `/app/settings` answers with the same text and status as for a non administrator on the eight existing sections, the routes with the status they use today for `ERR_FORBIDDEN`). |
| E2E-011 | F6 FR-NEW-008 | Given an empty database, no YAML, a master key. When the server starts. Then git, web and context7 are off, there is no host, no list, and no tool of the three families is offered. |
| E2E-012 | E1 FR-NEW-003 | Given two hosts. When she sets `hosts` to `{}`. Then no host is stored and the record line of that set holds the two keys `hosts.<host>` of the removed hosts. When she then sets the same two hosts and sets them a second time, the second answer changes no key and the record gains one line for that set whose list of changed keys is empty. |
| E2E-013 | E2 FR-NEW-011 | Given a stored `git.enabled` false. When the server starts with the git flag. Then git is on; reading the section shows `enabled` false and `restart_required` false; setting the pack size limit to 64 stores `enabled` false. |
| E2E-014 | FR-NEW-006 | Given git on, a project the administrator can push to, and the pack size limit 512 running. When she sets 64. Then the running limit is still 512 and the section says a restart is required; after the restart a push of a 70 MiB pack is refused with the status `413` and one of 60 MiB is accepted. |
| E2E-015 | FR-NEW-012 | When she sets hosts `corp.example.com` as `github` and the provider client id `abc`. Then the record line holds the key names `hosts.corp.example.com` and `github_client_id` and none of the values `github` or `abc`. |
| E2E-016 | FR-NEW-002 FR-NEW-003 | Each of the three sections read, then set to the value read, changes nothing and records no key. |
| E2E-017 | FR-NEW-004 FR-NEW-007 | Given a stored host key edited in the database to `*.x`. Then the start fails with `the stored settings are refused: git.hosts key '*.x' contains a wildcard character: host matching is exact only, one key per hostname`. A stored white list entry `not a site` fails with `the stored settings are refused: invalid entry 'not a site' in the web white list: expected a host name, a *.host name, an IP address or an address range`. A stored `git` value that is not an object fails with a message that begins `the stored setting 'git' is invalid:`. |
| E2E-018 | FR-NEW-001 | The list names the eleven sections in order with their source; an unknown section is refused with `unknown setting section 'nope', expected one of safety, api, extract, doc, sqlite, db, doc_service, search, git, web, context7`. |
| E2E-019 | FR-NEW-013 FR-NEW-001 | Given an administrator on the screen. When she submits for the section `git` the hosts value `{"github.example.com":"github"}`. Then the screen lists eleven sections, shows the key `github.example.com` and shows that a restart is required. |
| E2E-020 | FR-NEW-007 | Start from a YAML alone (no row): the three sections equal the YAML ones, behavior as today. |
| E2E-021 | FR-NEW-014 FR-NEW-003 | On PostgreSQL she sets hosts to `{"github.com":"github"}`: a read gives that object. She sets the pairs `["a.example","github"]` and `["a.example","gitlab"]`: refused with `git.hosts contains duplicate host 'a.example'`. She sets `{"b.example":"generic"}`: it replaces the list. After a restart the host in force is `b.example`. |
| E2E-022 | FR-NEW-004 | Admin A sets hosts `{"a.example":"github"}`, then admin B sets `{"b.example":"gitlab"}`. Then the stored list is `{"b.example":"gitlab"}` and the record holds two lines, one per administrator. Set truly at once, the stored list equals one of the two values and the record holds two lines. |
| E2E-023 | F4 FR-NEW-004 | When she sets the section `git` to the text `github.com`. Then she is refused with `the value of setting 'git' must be an object`, nothing stored. |
| E2E-024 | F3 FR-NEW-004 FR-NEW-016 | When she sets `hosts` to the text `github.com`. Then she is refused with a message that begins `invalid setting 'git':`, nothing stored. |
| E2E-025 | F2 FR-NEW-004 FR-NEW-017 | When she sets the black list to the text `*.ads.example` instead of a list. Then she is refused with a message that begins `invalid setting 'web':`, nothing stored. |
| E2E-026 | FR-NEW-015 | When she sets hosts `{"GitHub.com": "github", "github.com": "gitlab"}`. Then the answer holds both keys as typed, no duplicate refusal occurs and `restart_required` is true; when she then sets `{"GitHub.com": "github"}` the stored list holds only `GitHub.com`. |
| E2E-027 | FR-NEW-016 | When she sets hosts as the list `[["github.com","github","x"]]`. Then she is refused with a message that begins `invalid setting 'git':`, nothing stored. |
| E2E-028 | FR-NEW-016 | When she sets hosts to `null`, then to the number `3`. Then each is refused with a message that begins `invalid setting 'git':`, nothing stored. |
| E2E-029 | FR-NEW-009 | Given git on and a stored token for `gitlab.example.com`. When her change of hosts is refused as a duplicate. Then the token is still listed with the same last 4 characters. |
| E2E-030 | FR-NEW-010 | Given a stored row for `git` only. When a server starts from a YAML holding other values for the three sections. Then only `web` and `context7` are imported, the `git` of the YAML is ignored with the warning of SPEC-0114. |
| E2E-031 | FR-NEW-011 | Given no stored row and a YAML file whose `git`, `web` and `context7` sections each hold `enabled: false`. When the server starts from that YAML with the three flags. Then the three are on, and the rows imported by the seed hold `enabled` false. |
| E2E-032 | FR-NEW-008 FR-NEW-007 | Given a database holding only a `git` row and no YAML. When the server starts. Then the git host of the row is in force and web and context7 are off. |
| E2E-033 | FR-NEW-012 | When a change is refused. Then the record holds no new line. |
| E2E-034 | FR-NEW-005 | When she sets `gitlab_client_secret_env` to `MY_GITLAB_SECRET`. Then it is stored as that name, the answer shows no secret entry and never the content of that variable. |
| E2E-035 | FR-NEW-014 | On PostgreSQL the duplicate host refusal stores nothing and records nothing. |
| E2E-036 | FR-NEW-013 | On the screen she submits for `git` the pairs `[["github.com","github"],["github.com","gitlab"]]`. Then the screen shows `git.hosts contains duplicate host 'github.com'` and nothing is stored. |
| E2E-037 | FR-NEW-001 | A section named `GIT` in capitals is refused with the same message as E2E-018. |
| E2E-038 | FR-NEW-017 | Given the black list `["*.example"]` and the white list `["ok.example"]`, set by the YAML on one server and by the database on another, web on in both. Then on both servers a `web.fetch` of each of `https://ok.example/`, `https://x.example/` and `https://other.test/` answers `host '<site>' is not allowed by the web access lists`, `<site>` being the typed host. |
| E2E-039 | FR-NEW-016 | Hosts set as `{"b.example": "generic", "a.example": "gitlab", "c.example": "github"}` read back with keys in the order a, b, c; hosts `{"b.example":"generic","B.example":"generic"}` read back as `B.example`, `b.example`. |
| E2E-040 | FR-NEW-003 | Given the stored hosts `a.example` and `b.example`. When she sets hosts as the pairs `[["c.example","github"]]`. Then the stored list is `{"c.example":"github"}` only. |
| E2E-041 | FR-NEW-005 | When she sets `github_client_secret_env` to the number `5`. Then she is refused with a message that begins `invalid setting 'git':`, nothing is stored. |
| E2E-042 | FR-NEW-007 | Given a stored `web` row edited in the database to hold only `enabled` true, and a YAML with `web.max_results` 20. When the server starts. Then `max_results` in force is 10 (the stored row wins whole) and one warning says the YAML differs. |
| E2E-043 | FR-NEW-008 | Given an empty database, no YAML, no master key, and the server started with the git flag. When it starts. Then it starts, a `git.clone` of `https://github.com/o/r.git` is refused with `ERR_INVALID_ARGUMENT` and the message `host 'github.com' is not declared in git.hosts; declare it under git.hosts before cloning from this host`, and no web tool is offered; started without the flag, no `git.` tool is offered. |
| E2E-044 | FR-NEW-009 | Given git on and a stored token for `gitlab.example.com`. When she sets hosts to `{}`. Then the token is still listed with the same last 4 characters. |
| E2E-045 | FR-NEW-010 | Given a YAML whose white list holds `not a site` and an empty database. When the server starts. Then the start fails with an error whose text contains the message of E2E-007 and no row is imported. |
| E2E-046 | FR-NEW-011 | Given a stored `web.enabled` false and a server started with the web flag. When she sets `max_results` to 20. Then the stored section still holds `enabled` false. |
| E2E-047 | FR-NEW-012 | When she sets the black list to `["a.example","b.example"]`. Then the record line holds the key `black_list_url` and neither entry. |
| E2E-048 | FR-NEW-014 | On PostgreSQL she sets hosts `{"*.x":"github"}`. Then she is refused with the wildcard message of E2E-008 for `*.x`, nothing is stored and nothing is recorded. |
| E2E-049 | FR-NEW-015 | Given she set `{"Corp.Example":"github"}` with `enabled` true and the server restarted. Then a read gives the key `Corp.Example` and a `git.clone` of `https://Corp.Example/o/r.git` is refused with `ERR_INVALID_ARGUMENT` and the message `host 'corp.example' is not declared in git.hosts; declare it under git.hosts before cloning from this host`. |
| E2E-050 | FR-NEW-015 | The stored hosts `{"GitHub.com":"github"}` survive a restart with the key unchanged. |
| E2E-051 | FR-NEW-017 | Given the black list `["*.ads.example"]` and the white list `["ok.example"]` set by the database, web on. Then a `web.fetch` of `https://ok.example/` is not refused by the lists, one of `https://x.ads.example/` answers `host 'x.ads.example' is not allowed by the web access lists` and one of `https://other.test/` answers `host 'other.test' is not allowed by the web access lists` because the white list is not empty. |
| E2E-052 | FR-NEW-017 | When she sets the black list to `["ok.example","not a site"]`. Then she is refused with `invalid entry 'not a site' in the web black list: expected a host name, a *.host name, an IP address or an address range`, nothing is stored. |
| E2E-053 | FR-NEW-006 | Given a running section equal to the stored one. When she sets a value that is refused. Then `restart_required` stays false. |
| E2E-054 | FR-NEW-002 | She reads `context7` on a server never set. Then `api_url` is `https://context7.com/api`, `request_timeout_secs` is 30 and the source is `default`. |

### 9.3 Regression tests
None beyond 9.4.

### 9.4 Modified tests [existing only]
Declared exceptions, target as small as possible: the test of the refusal text of an unknown section (it lists the section names), the loops over the section list (they follow the list), the tool descriptions and the contract lines that list the sections. Identifiers in 14.2.

## 10. Traceability
Scenarios: S1 E2E-001; S2 E2E-002; S3 E2E-003; S4 E2E-004; S5 E2E-005; F1 E2E-006; F2 E2E-007 025; F3 E2E-008 024; F4 E2E-009 023; F5 E2E-010; F6 E2E-011; E1 E2E-012; E2 E2E-013.

| Requirement | Tests |
|---|---|
| FR-NEW-001 | E2E-018, E2E-019, E2E-037 |
| FR-NEW-002 | E2E-001, E2E-002, E2E-016, E2E-054 |
| FR-NEW-003 | E2E-004, E2E-012, E2E-016, E2E-021, E2E-040 |
| FR-NEW-004 | E2E-006, E2E-007, E2E-008, E2E-017, E2E-022, E2E-023, E2E-024, E2E-025 |
| FR-NEW-005 | E2E-009, E2E-034, E2E-041 |
| FR-NEW-006 | E2E-001, E2E-002, E2E-014, E2E-053 |
| FR-NEW-007 | E2E-003, E2E-017, E2E-020, E2E-032, E2E-042 |
| FR-NEW-008 | E2E-011, E2E-032, E2E-043 |
| FR-NEW-009 | E2E-004, E2E-029, E2E-044 |
| FR-NEW-010 | E2E-005, E2E-030, E2E-045 |
| FR-NEW-011 | E2E-013, E2E-031, E2E-046 |
| FR-NEW-012 | E2E-005, E2E-015, E2E-033, E2E-047 |
| FR-NEW-013 | E2E-010, E2E-019, E2E-036 |
| FR-NEW-014 | E2E-021, E2E-035, E2E-048 |
| FR-NEW-015 | E2E-026, E2E-049, E2E-050 |
| FR-NEW-016 | E2E-024, E2E-027, E2E-028, E2E-039 |
| FR-NEW-017 | E2E-002, E2E-007, E2E-025, E2E-038, E2E-051, E2E-052 |

## 11. Open questions
None that changes behavior.

## 12. Glossary
Section: one top level block of the configuration. Host list: the map of git host to provider. Access lists: the web black and white lists of SPEC-0097. Running section: the one the process was started with. Seed: the one time import of the YAML.

## 13. Implementability gate
Gate record: round 1 (independent auditor): NOT-IMPLEMENTABLE, 1 blocking and 4 minor F, merged. Round 2 (new auditor): NOT-IMPLEMENTABLE, coverage of 11 requirements and exact messages, merged with 15 added tests. Round 3 (new auditor): NOT-IMPLEMENTABLE, one blocking F (the clone refusal text of E2E-043 and 049) and two minor F (sort order, screen refusal), all precision, merged here. Rounds 4 to 6 (new auditors each time) found one blocking precision finding each (git must be on to observe a clone, record line of an empty change, the record key of a host), merged. The verdict of the last round is in the run log.

## 14. Evidence appendix
### 14.1 Current State evidence
Suites named in 2.3: the git remote tests of `git/remote.rs` (`e2e_new_006`), `e2e_0097_`, `e2e_0114_`.
1. `config.rs` `GitConfig`, `WebConfig`, `Context7Config` are read only by `ServerConfig::from_yaml`. 2. `git/remote.rs` `validate_hosts` publishes the map; `check_hosts` refuses a repeated host. 3. `settings/server_settings.rs` `SECTIONS` holds eight names; `app.rs` `build_with_env` calls `server_settings_boot::apply`, which does not publish the host map.
### 14.2 Existing tests to change
Also change the parameter description `Section name: ...` of `get_setting` and `set_setting` in `tools/settings.rs:28` and `mcp/server.rs:1748,1754` to name the eleven sections, regenerate `tool-contract-golden.json` (lines 543 and 1045), update `.agent_docs/config.md:592` and the SPEC-0114 entry of `AGENTS.md:79` including its "Parts 2 to 4" sentence. Detection signal: `tool_contract_golden_is_current`.
`AGENTS.md:79` (eight sections becomes eleven). The refusal status of the routes for a non administrator is fixed by `e2e_0114_007` (`settings/server_settings_e2e.rs:488`).
`settings/server_settings_e2e.rs:94` (the refusal text names eleven sections), the loops at `:172`, `:362`, `:371`, `:570` (they follow `SECTIONS`), `tools/settings.rs:20` and `mcp/server.rs:4121` (the description lists the sections), `TOOL_CONTRACT.txt` line 147 (description) and line 958 (return shape lists the sections),  `tool-contract-golden.json` (regenerated).
### 14.3 Design inputs
- D1 `HostMap` serializes as a list of pairs today and deserializes only from a map: it must serialize as an object and accept a list of pairs (duplicates must survive to the check).
- D1b Until `HostMap` serializes as an object, the round trip loop `settings/server_settings_e2e.rs:172` (`e2e_0114_013`) fails on `git`. Detection signal: that test.
- D2 `check_keys` refuses any host name as an unknown key and `merge` merges objects: `hosts` is a free key map replaced whole.
- D3 The boot overlay must publish the host map after a stored `git` section is applied, else a database only start has no host. NOTHING DETECTS THIS without E2E-003.
- D4 `with_section` for `web` drops the test only fields; keep them.
- D5 Consumers read their section once at the start (`git/http/mod.rs:114`, `mcp/server.rs:71`), so all three are restart flagged.
- D6 `git/remote.rs:1990` (`e2e_new_224`) requires exactly one `validate_hosts(` in `config.rs`; the overlay publishes the map from `settings/server_settings_boot.rs`, never by a second call in `config.rs`. Detection signal: that test.
- D7 The order of the hosts read back is ascending, whatever the order they were typed in.
- D8 The object key order of a JSON value depends on a `serde_json` feature (`Cargo.toml`); the read of `git.hosts` sorts explicitly. Detection signal: E2E-039.
- D9 `with_section` fills the keys missing from a stored row with the defaults, never with the YAML values. Detection signal: E2E-042.
