# Black list and white list for the addresses web.fetch and web.download can reach: functional specification

> Id: SPEC-0097
> Nature: FEAT
> Depth: M
> Depth evidence: 2 modules touched (the web tools, the server configuration), 12 requirements, no persisted data, no change to any tool schema. M by exclusion from S (two modules) and L.
> Status: implemented
> Generated on: 2026-11-16
> Project: existing
> From backlog: BL-0156
> Depends on: none
> Security: internal finding
> CVSS: not scored
> Affected: every release with the web tool family enabled (it is off by default)
> Fixed in: unreleased

## 1. Summary

An operator who enables the web tools can today let any project member make the server request any web address, including the server's own machine, the private network and the cloud metadata address, then read the answer or save it into the project. This increment gives the operator two lists, a black list and a white list, that decide which sites `web.fetch` and `web.download` can reach. With both lists empty nothing changes. The lists are checked on the site as typed, on the addresses it resolves to and on every redirect. The trigger is the SSRF finding of the SPEC-0093 class sweep (CWE-918), validated by the owner on 2026-11-12.

## 2. Current State (MANDATORY, every depth)

### 2.1 What users can do today

- A project member can ask `web.fetch` for the text of any address starting with `http://` or `https://`. With a project and a destination path the same tool saves the raw answer into the project instead.
- A project member can ask `web.download` to save any such address into the project.
- Nothing limits which site is reached. A request to the server's own machine, to a private network address or to the cloud metadata address is sent like any other.
- When the site answers with a redirect, the server follows it, up to a fixed number of hops, to any other address, including a private one.
- For `web.download` and for `web.fetch` returning text, the only refusal on the address is a wrong start: `url must start with http:// or https://`. `web.fetch` with a destination does not check the start of the address.
- The operator has no setting that restricts the sites. The web family has one switch (on or off, off by default) and settings for the number of search results, the timeout and the safe search level.
- The other web tools (search, news, suggestions) always contact fixed search sites; the member supplies only the words searched.
- Answer size limits exist since SPEC-0093 (16 MiB) and are unchanged by this increment.

### 2.2 Specs governing this area

- SPEC-0093 bounds the size of what `web.fetch` and `web.download` read and fixes the order of refusals (argument, membership and path, request and read, write checks). It records address filtering as out of scope. This increment adds it and deviates from that exclusion. SPEC-0093 is not modified.
- SPEC-0009 introduced the optional web family (off by default). Not modified.

### 2.3 What is verified today

Both tools are exercised by tests that call real tool requests against stand in sites on the test machine's own loopback address. No test refuses an address, because no refusal exists. With both lists empty this behavior is preserved, so these tests stay valid. The command and files are in Section 14.1.

## 3. Scope

### 3.1 In scope

- Two operator settings, a black list and a white list of sites, both empty by default.
- The decision rule over the four combinations of empty and non empty lists.
- The format of an entry, and the refusal of a malformed entry when the server starts.
- Checking the site as typed, every address it resolves to, and every redirect hop.
- The refusal message, and the guarantee that a refused request sends nothing, writes nothing and counts nothing.
- The two tools `web.fetch` (text mode and save mode) and `web.download`.
- Documentation of the two settings, with a recommended black list entry for the cloud metadata addresses.

### 3.2 Out of scope, and why

- The search, news and suggestions tools: they contact fixed sites chosen by the product, never an address given by a member (Section 14.1, claim 8).
- The documentation tools (`context7.*`): the address is set by the operator, not by a member.
- The `git.*` tools: a member can only name hosts the operator declared in the host map, a separate existing control.
- Ports and paths in an entry: an entry names a site, not a page. Limiting by port or path is a different feature.
- A document conversion (Markdown to Word) that fetches remote images named in a member's Markdown (Section 14.2, occurrence 2): different mechanism and different fix. Deferred by owner decision of 2026-11-16 to its own entry, BL-0159 (`Nature: BUG`). Until it ships, that conversion stays exploitable on installations where the document tools are enabled.
- Proxy behavior: how the server uses a proxy is BL-0157. This increment only states the limit it creates (FR-NEW-006).
- Per project or per member lists: one list pair for the whole server.
- Changing a list without restarting the server.

### 3.3 Constraints imposed by the context

- The two settings have names fixed by the owner (Section 14.3).
- Both lists empty must behave exactly as today (owner, 2026-11-16).
- A site in both lists is blocked (owner, 2026-11-16).

## 4. Actors

| Actor | Description |
|---|---|
| Operator | Configures the server, including the two lists, and reads the documentation. |
| Member | Project member who calls `web.fetch` and `web.download`. |
| Non member | A signed in person who is not a member of the project named in the request. |
| Remote site | The site a member names, or a site a redirect leads to. |

## 4.5 Bounded contexts (MANDATORY at M and L)

| Context | What it owns | Key concepts |
|---|---|---|
| Web access | which sites the web tools can reach, the two lists and their entries | black list, white list, entry, match, refusal, site as typed, resolved address, hop |
| Project files | the files a download or a saved fetch writes, and the storage the project uses | destination path, storage use |

Terms marked `Tests` in the glossary belong to the test vocabulary of Section 9, not to a bounded context.

## 5. Usage scenarios

### SC-001: Both lists empty, behavior unchanged
**Actor:** member
**Preconditions:** the web tools are on; both lists empty; Alice (`alice@example.com`) is a member of project `proj`.
**Flow:**
1. Alice asks `web.fetch` for `http://news.example/` → she receives the page text.
2. Alice asks `web.download` for `http://docs.example/manual.pdf` into `/manual.pdf` of `proj` → the file is saved.
3. Alice asks for a site on the server's own machine, `http://127.0.0.1:PORT/` → it answers like any other site.
**Postconditions:** nothing is refused because of an address.
**Exceptions:**
- EXC-001a: the site cannot be reached → the usual failure message of today.
- EXC-001b: the address starts with `ftp://` → `url must start with http:// or https://`.

### SC-002: Black list only
**Actor:** member
**Preconditions:** the black list has at least one entry; the white list is empty.
**Flow:**
1. Alice asks for a site that matches no entry → the request is served.
2. Alice asks for a site that matches an entry → she receives the refusal and nothing reaches that site.
**Postconditions:** blocked sites are never contacted; every other site is reachable.
**Exceptions:**
- EXC-002a: the site is allowed by name but resolves to a black listed address → refused (SC-006).

### SC-003: White list only
**Actor:** member
**Preconditions:** the white list has at least one entry; the black list is empty.
**Flow:**
1. Alice asks for a site that matches an entry → served.
2. Alice asks for any other site → refused.
**Postconditions:** only white listed sites are contacted.
**Exceptions:**
- EXC-003a: the site name cannot be resolved and the name matches no entry → refused.

### SC-004: Both lists
**Actor:** member
**Preconditions:** both lists have at least one entry.
**Flow:**
1. Alice asks for a site that matches the white list and not the black list → served.
2. Alice asks for a site that matches the black list, whether or not the white list also names it → refused.
3. Alice asks for a site that matches neither list or only the black list → refused.
**Postconditions:** reachable sites are the white list minus the black list.

### SC-005: Redirects
**Actor:** member
**Preconditions:** a list setup that refuses some site.
**Flow:**
1. Alice asks for a site that is allowed and that answers with a redirect to a refused site → the refusal is given for the redirected site and nothing reaches it.
**Postconditions:** a redirect cannot reach a site the lists refuse.
**Exceptions:**
- EXC-005a: the redirect leads to an allowed site → served.
- EXC-005b: the second hop of a chain is refused → refused at that hop.

### SC-006: A site name that points to a refused address
**Actor:** member
**Preconditions:** the black list holds a range such as `10.0.0.0/8`.
**Flow:**
1. Alice asks for `http://friendly.example/` whose name points to `10.1.2.3` → refused; the message names `friendly.example` and never shows `10.1.2.3`.
2. Alice asks for the same address written differently (`http://2130706433/` for `127.0.0.1`, or `http://[::ffff:127.0.0.1]/`) → treated as the plain address and refused.
**Postconditions:** an address cannot be reached by disguising it.

### SC-007: Operator makes a malformed entry
**Actor:** operator
**Preconditions:** the server starts with a list containing an invalid entry.
**Flow:**
1. The operator starts the server → it refuses to start and says which entry is invalid and why.
**Postconditions:** the server is not running. Fixing the entry and starting again works.

### SC-008: A refused save leaves nothing behind
**Actor:** member
**Preconditions:** a destination path in `proj`; storage use known.
**Flow:**
1. Alice asks `web.download` or `web.fetch` with a destination for a refused site → refused; no file is created or changed and storage use does not change.
**Postconditions:** the project is exactly as before the request.

## 6. Functional requirements

### 6.1 New

#### FR-NEW-001 [EARS-S]: Empty lists change nothing
> WHILE both lists are empty THE web access context SHALL let `web.fetch` and `web.download` reach any site, exactly as before this change.

- **Business rules:** no address is looked up or compared when both lists are empty; the server's own machine, private network addresses and the cloud metadata address stay reachable.
- **User visible text:** unchanged: `url must start with http:// or https://` for a wrong start.
- **Scenario:** SC-001
- **Priority:** Must

#### FR-NEW-002 [EARS-E]: Black list only
> WHEN the black list has at least one entry and the white list is empty THE web access context SHALL refuse a request that matches the black list and SHALL serve every other request.

- **Scenario:** SC-002, SC-008
- **Priority:** Must

#### FR-NEW-003 [EARS-E]: White list only
> WHEN the white list has at least one entry and the black list is empty THE web access context SHALL serve a request that matches the white list and SHALL refuse every other request.

- **Scenario:** SC-003
- **Priority:** Must

#### FR-NEW-004 [EARS-E]: Both lists
> WHEN both lists have at least one entry THE web access context SHALL refuse a request that matches the black list, and among the others SHALL serve only a request that matches the white list.

- **Business rules:** a site named in both lists is refused. The reachable set is the white list minus the black list.
- **Scenario:** SC-004
- **Priority:** Must

#### FR-NEW-005 [EARS-U]: Entry forms
> The web access context SHALL accept as a list entry exactly one of: a host name (`docs.example`), a name with a leading wildcard (`*.corp.example`), an IPv4 or IPv6 address (`10.0.0.5`, `fd12::1`), or an address range written with a slash (`169.254.0.0/16`, `fd00::/8`).

- **Business rules:**
  - Letter case never matters (`Docs.Example` equals `docs.example`), and a trailing dot is ignored (`blocked.example.` is `blocked.example`), in an entry and in a request.
  - A host name matches only that site.
  - A wildcard name matches any site below the name (`wiki.corp.example`, `a.b.corp.example`) and never the bare name (`corp.example`).
  - An address matches only that address; a range matches every address inside it, first and last included.
  - An entry has no scheme, path or port.
  - A range length is one to three decimal digits with no sign and no leading zero; at most 32 for an IPv4 range and 128 for an IPv6 range; `/0` is valid and covers every address of its family. `10.0.0.0/08`, `10.0.0.0/+8` and `10.0.0.0/` are invalid.
  - An IPv6 range in IPv4 mapped form (`::ffff:10.0.0.0/104`) is valid and covers the plain IPv4 addresses `10.0.0.0` to `10.255.255.255`. An IPv6 entry in brackets (`[fd00::1]`) or with a zone (`fe80::1%eth0`) is invalid.
  - An IPv6 range matches only true IPv6 addresses, except a range of 96 to 128 bits whose first 96 bits are `0:0:0:0:0:ffff`, which is read as the IPv4 range with the same trailing bits (length minus 96). So `::/0` and `::ffff:0:0/80` never cover a plain IPv4 address, and `::ffff:0:0/96` covers every IPv4 address.
  - A label with a leading or trailing hyphen is valid (`-a.example`, `a-.example`). A label whose first four characters are `xn--` is valid only when what follows is valid punycode that decodes; `xn--a` is invalid. No other rule rejects a name. The conversion to ASCII form is the same for an entry and for a request.
  - The site of a request is read exactly as the WHATWG URL standard reads the host of an `http` or `https` address: IPv4 numbers in decimal, octal (leading zero) or hexadecimal, with one to four parts, become the dotted four part form (`127.1` is `127.0.0.1`, `0177.0.0.1` is `127.0.0.1`, `010.0.0.1` is `8.0.0.1`). A name is converted with the UTS 46 non transitional processing, so `straße.example` becomes `xn--strae-oqa.example` and stays distinct from `strasse.example`. A host name entry is converted the same way, under the stricter entry rules of this list. A label `xn--` with nothing after it, in any letter case, is invalid in an entry.
  - Exactly one trailing dot is ignored, in an entry and in a request. `blocked.example..` is therefore an invalid entry, and so is `.`.
  - An entry is used exactly as written: a leading or trailing space makes it invalid.
  - A range whose address has bits set below its prefix (`10.0.0.5/24`) is valid and covers `10.0.0.0` to `10.0.0.255`.
  - An IPv4 part with a leading zero (`010.0.0.1`) is invalid. A label longer than 63 characters, a name longer than 253 characters, or a name that cannot be converted to its ASCII form (`xn--a`) is invalid.
  - An IPv6 entry in IPv4 mapped form (`::ffff:127.0.0.1`) is valid and is judged as the plain IPv4 address `127.0.0.1`; `::ffff:0:0/96` is valid and covers every mapped address.
  - A host name is compared in its ASCII form: an entry or a request written with accents (`bücher.example`) is converted to its ASCII form (`xn--bcher-kva.example`) before comparing.
  - A host name is made of labels separated by dots; a label is made of letters, digits, hyphens and underscores and is never empty. A `*` is valid only as the whole first label (`*.corp.example`).
  - An entry whose last label is only digits, or starts with `0x`, must be an IPv4 address of four parts (`10.0.0.5`); otherwise it is invalid (`2130706433`, `0x7f000001`, `10.1` are invalid entries).
- **Scenario:** SC-002, SC-003, SC-004, SC-006, SC-007
- **Priority:** Must

#### FR-NEW-006 [EARS-E]: What is matched
> WHEN a request names a site THE web access context SHALL decide the black list on the site as typed and on every address the site resolves to, and the white list on the site as typed or on all of its resolved addresses.

- **Business rules:**
  - Black list: refused if the site as typed matches an entry, or if any one resolved address matches an entry.
  - White list: served if the site as typed matches an entry, or if the site resolves to at least one address and every resolved address matches an entry. A site that resolves to no address is never white listed by address.
  - An IPv4 address written in another form (`2130706433`, `0x7f000001`, `[::ffff:127.0.0.1]`) is judged as the plain IPv4 address `127.0.0.1`.
  - The address that was checked is the address the request is sent to.
  - An address a site name resolves to, when it is an IPv4 mapped IPv6 address (`::ffff:a.b.c.d`), is judged as the plain IPv4 address `a.b.c.d`, for the black list and for the white list, exactly as a typed address is.
  - WHILE a list is non empty, a request whose address has no host, or whose host cannot be converted to its ASCII form, or that cannot be read as an address at all, is never sent and fails with `ERR_INTERNAL_ERROR: fetch request failed` followed by the reason, never with the lists refusal. With both lists empty nothing changes.
  - Limit, owner decision of 2026-11-16: when the server reaches the web through a proxy set in its environment, the proxy makes the connection, so the line above does not hold. The lists are still decided on the site as typed and on the addresses the server itself resolves, before the request goes to the proxy. How the server uses a proxy is the subject of BL-0157, not of this increment.
  - A lookup that ends in an error of any kind (name not found, no address, server failure, timeout) is treated as "resolves to no address". The timeout of the request also bounds the lookup, and a lookup that exceeds it fails with today's `ERR_INTERNAL_ERROR: request timed out`, whatever the lists hold.
  - A site that resolves to no address is not refused by the black list on an address; if the site as typed matches no entry the request goes ahead and fails with today's failure message (`ERR_INTERNAL_ERROR: fetch request failed` followed by the reason).
  - A request whose address cannot be read as a web address, such as an IPv6 address with a zone (`http://[fe80::1%25eth0]/`), is never sent while a list is non empty: it fails with the same failure message as today. With both lists empty nothing changes.
- **Scenario:** SC-002, SC-003, SC-006, SC-008
- **Priority:** Must

#### FR-NEW-007 [EARS-E]: Redirects
> WHEN a site answers with a redirect THE web access context SHALL apply the same decision to the redirected site before following it, on every hop of a chain.

- **Business rules:** a refusal on any hop ends the request. Apart from the lists decision everything about redirects is unchanged: the maximum is 10 hops, the status codes followed, the handling of a relative address, and the failure messages (too many hops, a redirect to an address not starting with `http://` or `https://`) are those of today, each beginning `ERR_INTERNAL_ERROR: fetch request failed`. A redirect destination is resolved against the address of the hop that sent it, as today, and the lists decide the host of the resolved address; a protocol relative destination (`//blocked.example/x`) is therefore refused for `blocked.example` when that site is listed. On each hop the lists decision comes first: if the hop's site is refused the answer is the refusal, whatever the hop's scheme and even beyond the maximum of 10 hops; only a hop that the lists allow can end with the failures above. A redirect answer with no usable destination address is not followed and is returned as the answer, exactly as today: `web.fetch` shows the answer's text, and `web.download` and the save mode of `web.fetch` write its body.
- **Scenario:** SC-005
- **Priority:** Must

#### FR-NEW-008 [EARS-E]: Refusal
> WHEN a request is refused by the lists THE web access context SHALL answer `ERR_FORBIDDEN: host '<site>' is not allowed by the web access lists`, SHALL send nothing to that site, and SHALL NOT show any resolved address.

- **Business rules:**
  - `<site>` is the site as typed in the request, or as given by the redirect that was refused, in lower case, in its ASCII form, with a trailing dot removed. A numeric IPv4 form is written in dotted form (`http://2130706433/` gives `127.0.0.1`); an IPv6 address is written without brackets, in its shortest lower case form (`[::ffff:127.0.0.1]` gives `::ffff:7f00:1`). The refusal applies to the site named by the address whatever its scheme: in the save mode of `web.fetch`, `ftp://blocked.example/` is refused with the lists refusal when `blocked.example` matches, and `ftp://news.example/` (no match) gets today's failure message. It never contains a port, a user name or a password: `http://u:p@Blocked.example:8080/` gives `blocked.example`.
  - The refusal comes after the argument checks, the membership check and the path check, and before the storage allowance check and any request. A non member therefore sees the membership refusal, not this one.
- **User visible text:** `ERR_FORBIDDEN: host 'blocked.example' is not allowed by the web access lists`
- **Scenario:** SC-002, SC-003, SC-008
- **Priority:** Must

#### FR-NEW-009 [EARS-E]: A refused save leaves nothing behind
> WHEN a request that saves into a project is refused by the lists THE web access context SHALL NOT create or change any file, and the project's storage use SHALL be unchanged.

- **Scenario:** SC-002, SC-005, SC-006, SC-008
- **Priority:** Must

#### FR-NEW-010 [EARS-O]: Malformed entry at startup
> IF a list contains an entry that is not one of the forms of FR-NEW-005 THEN THE server SHALL refuse to start and SHALL say which list and which entry are invalid.

- **Business rules:** an empty entry, `*` alone, an entry with a scheme (`https://blocked.example/x`), a path, a port (`example.com:8080`), or a range with an impossible length (`10.0.0.0/33`) is invalid. A list that is empty is valid.
- **User visible text:** `ERR_INVALID_ARGUMENT: invalid entry '<entry>' in the web black list: expected a host name, a *.host name, an IP address or an address range` (and `web white list` for the other list). The server prints the line `error: ` followed by it to its error output and exits with a non zero status. When several entries are invalid, only the first is named, black list before white list, in list order.
- **Scenario:** SC-007
- **Priority:** Must

#### FR-NEW-011 [EARS-UB]: Other tools unaffected
> The web access context SHALL NOT apply the lists to web search, web news and web suggestions, nor to any tool other than `web.fetch` and `web.download`.

- **Scenario:** SC-002, SC-003, SC-004
- **Priority:** Must

#### FR-NEW-012 [EARS-U]: The check does not depend on the web switch
> The server program SHALL check every entry of both lists at the start of every command that reads the configuration (`serve`, `purge`, `migrate`), whether or not the web tools are on, and SHALL refuse with the message of FR-NEW-010 and exit with a non zero status before doing any work on an invalid entry. For `migrate` the source configuration is checked first, then the destination, and the first invalid entry found is the one named. The commands `keys`, `token` and `version` do not read the configuration and are unchanged.

- **Scenario:** SC-007
- **Priority:** Must

### 6.2 Modified

None. No existing requirement of SPEC-0093 changes; its exclusion of address filtering (Section 2.2) is the deviation.

### 6.3 Removed

None.

## 7. Non functional requirements (user observable)

### 7.1 Performance
- WHILE both lists are empty THE web access context SHALL add no wait to a request (no address lookup beyond today's).

### 7.2 Security and access
- The refusal reveals only the site the member typed or a redirect named, never an address it resolves to (FR-NEW-008).
- An entry in both lists is refused (FR-NEW-004).

### 7.3 Privacy and compliance
Not applicable: no personal data is stored by this increment.

### 7.4 Usability and accessibility
- An operator reading the documentation finds the four list combinations in one table, and a recommended black list entry for the cloud metadata addresses (`169.254.0.0/16`).

### 7.5 Reliability
- A refused request never leaves a partial file (FR-NEW-009).

### 7.6 Operability
- A malformed entry stops the server at startup with the entry named (FR-NEW-010), so a typo never silently leaves a site open.

## 8. Edge cases

| Category | Case | Expected behavior | Test |
|---|---|---|---|
| Empty states | both lists empty | everything reachable | E2E-001 to E2E-003 |
| Boundary values | first and last address of a range, neighbour outside | inside refused, outside served | E2E-015, E2E-016 |
| Concurrent actions | not applicable: a decision depends only on the request and the lists | n/a | n/a |
| Slow or lost connectivity | site unreachable with both lists empty | today's failure message | E2E-004 |
| Access states | non member asks for a refused site | membership refusal | E2E-049 |
| Data variations | upper case entry, upper case site, accents, trailing dot, address with a zone, IPv6, decimal and mapped address forms | matched regardless of form | E2E-014, E2E-053, E2E-057, E2E-058, E2E-046, E2E-SEC-004, E2E-SEC-005 |
| Navigation | redirect chains, redirect to a refused site | refused at the hop | E2E-035 to E2E-037, E2E-SEC-002 |
| Partial operations | refused save with an existing file at the path | file untouched | E2E-050 |
| Dependency failures | site name that cannot be resolved | refused when the white list is set; goes ahead and fails as today under a black list only | E2E-023, E2E-054, E2E-055 |
| Ordering and timing | entry in both lists | refused | E2E-030 |
| Exhaustion | storage allowance exhausted and site refused | the lists refusal comes first | E2E-051 |

## 9. Acceptance tests (the contract)

> Behavioral and technology agnostic. `/sdlc-design` maps each test to a driver.

**Cast used by every test.** Alice (`alice@example.com`) is a member of project `proj`. Bob (`bob@example.com`) is not. Sites, each answering with the stated page: `news.example` at `93.184.216.34`, page `Daily headlines`; `docs.example` at `93.184.216.35`, page `Manual v1`; `friendly.example` at `10.1.2.3`, page `Internal payroll`; `mixed.example` at `93.184.216.36` and `10.1.2.4`, page `Mixed`; `localhost` at `127.0.0.1`. A "stand in" is a site on the test machine that counts the requests it receives. The web tools are on. Startup refusals appear as the line `error: <message>`; the startup tests quote the `<message>` part. The refusal text is `ERR_FORBIDDEN: host '<site>' is not allowed by the web access lists`. Lists not mentioned are empty.

### 9.1 Summary

| Test | Action | Category | Scenario | Requirements | Priority |
|---|---|---|---|---|---|
| E2E-001 | New | happy | SC-001 | FR-NEW-001 | Critical |
| E2E-002 | New | happy | SC-001 | FR-NEW-001 | Critical |
| E2E-003 | New | edge | SC-001 | FR-NEW-001 | High |
| E2E-004 | New | failure | SC-001 | FR-NEW-001 | High |
| E2E-005 | New | failure | SC-001 | FR-NEW-001 | Medium |
| E2E-006 | New | side effect | SC-001 | FR-NEW-001 | High |
| E2E-007 | New | happy | SC-002 | FR-NEW-002 | Critical |
| E2E-008 | New | failure | SC-002 | FR-NEW-002, FR-NEW-008 | Critical |
| E2E-009 | New | failure | SC-002 | FR-NEW-002, FR-NEW-005 | High |
| E2E-010 | New | failure | SC-002 | FR-NEW-002, FR-NEW-005 | High |
| E2E-011 | New | failure | SC-002 | FR-NEW-002, FR-NEW-006 | High |
| E2E-012 | New | failure | SC-002 | FR-NEW-002, FR-NEW-005 | Medium |
| E2E-013 | New | edge | SC-002 | FR-NEW-005 | High |
| E2E-014 | New | edge | SC-002 | FR-NEW-005 | Medium |
| E2E-015 | New | edge | SC-002 | FR-NEW-005 | High |
| E2E-016 | New | edge | SC-002 | FR-NEW-005 | High |
| E2E-017 | New | side effect | SC-002 | FR-NEW-008 | Critical |
| E2E-018 | New | edge | SC-002 | FR-NEW-011 | High |
| E2E-019 | New | happy | SC-003 | FR-NEW-003 | Critical |
| E2E-020 | New | happy | SC-003 | FR-NEW-003, FR-NEW-005 | High |
| E2E-021 | New | failure | SC-003 | FR-NEW-003, FR-NEW-008 | Critical |
| E2E-022 | New | failure | SC-003 | FR-NEW-003 | Critical |
| E2E-023 | New | failure | SC-003 | FR-NEW-003, FR-NEW-006 | High |
| E2E-024 | New | edge | SC-003 | FR-NEW-003, FR-NEW-005 | High |
| E2E-025 | New | edge | SC-003 | FR-NEW-006 | High |
| E2E-026 | New | edge | SC-003 | FR-NEW-006 | High |
| E2E-027 | New | side effect | SC-003 | FR-NEW-003 | High |
| E2E-028 | New | edge | SC-003 | FR-NEW-011 | High |
| E2E-029 | New | happy | SC-004 | FR-NEW-004 | Critical |
| E2E-030 | New | failure | SC-004 | FR-NEW-004 | Critical |
| E2E-031 | New | failure | SC-004 | FR-NEW-004 | High |
| E2E-032 | New | edge | SC-004 | FR-NEW-004, FR-NEW-005 | High |
| E2E-033 | New | side effect | SC-004 | FR-NEW-004 | High |
| E2E-034 | New | edge | SC-004 | FR-NEW-011 | High |
| E2E-035 | New | happy | SC-005 | FR-NEW-007 | Critical |
| E2E-036 | New | failure | SC-005 | FR-NEW-007 | Critical |
| E2E-037 | New | edge | SC-005 | FR-NEW-007 | High |
| E2E-038 | New | side effect | SC-005 | FR-NEW-007, FR-NEW-009 | High |
| E2E-039 | New | happy | SC-006 | FR-NEW-006 | High |
| E2E-040 | New | edge | SC-006 | FR-NEW-006 | High |
| E2E-041 | New | happy | SC-007 | FR-NEW-010 | High |
| E2E-042 | New | failure | SC-007 | FR-NEW-010 | Critical |
| E2E-043 | New | failure | SC-007 | FR-NEW-010 | High |
| E2E-044 | New | failure | SC-007 | FR-NEW-010 | High |
| E2E-045 | New | failure | SC-007 | FR-NEW-010 | High |
| E2E-046 | New | edge | SC-007 | FR-NEW-005, FR-NEW-010 | Medium |
| E2E-047 | New | side effect | SC-007 | FR-NEW-010 | High |
| E2E-048 | New | happy | SC-008 | FR-NEW-009 | High |
| E2E-049 | New | failure | SC-008 | FR-NEW-008 | High |
| E2E-050 | New | edge | SC-008 | FR-NEW-009 | High |
| E2E-051 | New | side effect | SC-008 | FR-NEW-009, FR-NEW-008 | High |
| E2E-053 | New | edge | SC-002 | FR-NEW-005 | High |
| E2E-054 | New | failure | SC-002 | FR-NEW-006 | High |
| E2E-055 | New | edge | SC-002 | FR-NEW-006, FR-NEW-009 | High |
| E2E-056 | New | failure | SC-007 | FR-NEW-005, FR-NEW-010 | High |
| E2E-057 | New | edge | SC-002 | FR-NEW-005 | Medium |
| E2E-058 | New | failure | SC-006 | FR-NEW-006 | High |
| E2E-059 | New | failure | SC-007 | FR-NEW-005, FR-NEW-010 | Medium |
| E2E-060 | New | failure | SC-007 | FR-NEW-005, FR-NEW-010 | High |
| E2E-061 | New | edge | SC-002 | FR-NEW-005 | High |
| E2E-062 | New | edge | SC-006 | FR-NEW-005, FR-NEW-006 | High |
| E2E-063 | New | failure | SC-007 | FR-NEW-010, FR-NEW-012 | High |
| E2E-064 | New | failure | SC-005 | FR-NEW-007 | High |
| E2E-065 | New | edge | SC-002 | FR-NEW-008 | High |
| E2E-066 | New | happy | SC-007 | FR-NEW-012 | Medium |
| E2E-067 | New | failure | SC-007 | FR-NEW-010, FR-NEW-012 | Medium |
| E2E-068 | New | failure | SC-006 | FR-NEW-006 | High |
| E2E-069 | New | happy | SC-006 | FR-NEW-006 | High |
| E2E-070 | New | edge | SC-006 | FR-NEW-006 | High |
| E2E-071 | New | edge | SC-001 | FR-NEW-001 | High |
| E2E-072 | New | failure | SC-007 | FR-NEW-005, FR-NEW-010 | High |
| E2E-073 | New | edge | SC-006 | FR-NEW-005, FR-NEW-006 | High |
| E2E-074 | New | edge | SC-005 | FR-NEW-007 | High |
| E2E-075 | New | edge | SC-006 | FR-NEW-005, FR-NEW-006 | High |
| E2E-076 | New | edge | SC-008 | FR-NEW-008 | High |
| E2E-077 | New | edge | SC-005 | FR-NEW-007 | High |
| E2E-078 | New | failure | SC-005 | FR-NEW-007 | High |
| E2E-079 | New | edge | SC-005 | FR-NEW-007 | High |
| E2E-080 | New | failure | SC-006 | FR-NEW-006 | High |
| E2E-081 | New | failure | SC-008 | FR-NEW-006 | High |
| E2E-082 | New | edge | SC-007 | FR-NEW-005, FR-NEW-010 | Medium |
| E2E-083 | New | edge | SC-006 | FR-NEW-006 | High |
| E2E-084 | New | edge | SC-002 | FR-NEW-005 | High |
| E2E-085 | New | failure | SC-007 | FR-NEW-005, FR-NEW-010 | Medium |
| E2E-086 | New | failure | SC-007 | FR-NEW-005, FR-NEW-010 | Medium |
| E2E-087 | New | failure | SC-003 | FR-NEW-006 | High |
| E2E-088 | New | edge | SC-005 | FR-NEW-007 | High |
| E2E-089 | New | failure | SC-007 | FR-NEW-012 | High |
| E2E-090 | New | failure | SC-007 | FR-NEW-012 | High |
| E2E-091 | New | happy | SC-007 | FR-NEW-012 | Medium |
| E2E-052 | New | side effect | SC-006 | FR-NEW-006, FR-NEW-009 | High |
| E2E-SEC-001 | New | security | SC-002 | FR-NEW-002, FR-NEW-008 | Critical |
| E2E-SEC-002 | New | security | SC-005 | FR-NEW-007 | Critical |
| E2E-SEC-003 | New | security | SC-006 | FR-NEW-006 | Critical |
| E2E-SEC-004 | New | security | SC-006 | FR-NEW-006 | Critical |
| E2E-SEC-005 | New | security | SC-006 | FR-NEW-006 | Critical |
| E2E-SEC-006 | New | security | SC-008 | FR-NEW-002, FR-NEW-009 | Critical |
| E2E-SEC-007 | New | security | SC-008 | FR-NEW-002, FR-NEW-009 | Critical |
| E2E-SEC-008 | New | security | SC-006 | FR-NEW-006 | Critical |
| E2E-SEC-009 | New | security | SC-006 | FR-NEW-006 | Critical |

Coverage: happy 13, failure 36, side effect 8, edge 34, state transition 0 (no state machine), security 9. Total 100. Happy to failure ratio 1:3.5 counting security tests as failures (13 to 45).

### 9.2 New tests

**SC-001**

#### E2E-001: fetch with empty lists
- **Category:** happy. **Scenario:** SC-001. **Requirements:** FR-NEW-001
- **Given** both lists empty. **When** Alice asks `web.fetch` for `http://news.example/`. **Then** she receives `Daily headlines`.

#### E2E-002: download with empty lists
- **Category:** happy. **Scenario:** SC-001. **Requirements:** FR-NEW-001
- **Given** both lists empty. **When** Alice asks `web.download` for `http://docs.example/manual.pdf` into `/manual.pdf` of `proj`. **Then** the call succeeds. **And** `/manual.pdf` holds `Manual v1`.

#### E2E-003: own machine reachable with empty lists
- **Category:** edge. **Scenario:** SC-001. **Requirements:** FR-NEW-001
- **Given** both lists empty and a stand in on `127.0.0.1` answering `Local`. **When** Alice fetches it. **Then** she receives `Local`.

#### E2E-004: unreachable site, empty lists
- **Category:** failure. **Scenario:** SC-001. **Requirements:** FR-NEW-001
- **Given** both lists empty and `down.example` that refuses connections. **When** Alice fetches `http://down.example/`. **Then** she gets the same failure message as before this change (`fetch request failed`, never the lists refusal).

#### E2E-071: a redirect without a destination, empty lists
- **Category:** edge. **Scenario:** SC-001. **Requirements:** FR-NEW-001
- **Given** both lists empty and `news.example` answering status 302 with no destination and the body `Moved`. **When** Alice fetches `http://news.example/`. **Then** she receives `Moved`, not `ERR_INTERNAL_ERROR` and not the lists refusal.

#### E2E-005: wrong start of address, empty lists
- **Category:** failure. **Scenario:** SC-001. **Requirements:** FR-NEW-001
- **Given** both lists empty. **When** Alice fetches `ftp://news.example/`. **Then** she gets `url must start with http:// or https://`.

#### E2E-006: download counts storage, empty lists
- **Category:** side effect. **Scenario:** SC-001. **Requirements:** FR-NEW-001
- **Given** both lists empty and `proj` using 100 bytes. **When** Alice downloads `http://docs.example/manual.pdf` (page `Manual v1`, 9 bytes) into `/manual.pdf`. **Then** `proj` uses 109 bytes.

**SC-002**

#### E2E-007: unlisted site served
- **Category:** happy. **Scenario:** SC-002. **Requirements:** FR-NEW-002
- **Given** black list `blocked.example`. **When** Alice fetches `http://news.example/`. **Then** she receives `Daily headlines`.

#### E2E-008: listed name refused
- **Category:** failure. **Scenario:** SC-002. **Requirements:** FR-NEW-002, FR-NEW-008
- **Given** black list `blocked.example`. **When** Alice fetches `http://blocked.example/`. **Then** she gets exactly `ERR_FORBIDDEN: host 'blocked.example' is not allowed by the web access lists`.

#### E2E-009: wildcard refuses a subdomain
- **Category:** failure. **Scenario:** SC-002. **Requirements:** FR-NEW-002, FR-NEW-005
- **Given** black list `*.corp.example`. **When** Alice fetches `http://wiki.corp.example/` and then `http://a.b.corp.example/`. **Then** both get the refusal naming the site typed.

#### E2E-010: exact address refused
- **Category:** failure. **Scenario:** SC-002. **Requirements:** FR-NEW-002, FR-NEW-005
- **Given** black list `10.0.0.5`. **When** Alice fetches `http://10.0.0.5/`. **Then** she gets `ERR_FORBIDDEN: host '10.0.0.5' is not allowed by the web access lists`.

#### E2E-011: a name for the own machine is refused
- **Category:** failure. **Scenario:** SC-002. **Requirements:** FR-NEW-002, FR-NEW-006
- **Given** black list `127.0.0.0/8`. **When** Alice fetches `http://localhost/`. **Then** she gets `ERR_FORBIDDEN: host 'localhost' is not allowed by the web access lists`, and the message contains no `127.0.0.1`.

#### E2E-012: IPv6 range refused
- **Category:** failure. **Scenario:** SC-002. **Requirements:** FR-NEW-002, FR-NEW-005
- **Given** black list `fd00::/8`. **When** Alice fetches `http://[fd12::1]/`. **Then** she gets `ERR_FORBIDDEN: host 'fd12::1' is not allowed by the web access lists`.

#### E2E-013: wildcard does not match the bare name
- **Category:** edge. **Scenario:** SC-002. **Requirements:** FR-NEW-005
- **Given** black list `*.corp.example` and a site `corp.example` answering `Corp home`. **When** Alice fetches `http://corp.example/`. **Then** she receives `Corp home`.

#### E2E-014: letter case never matters
- **Category:** edge. **Scenario:** SC-002. **Requirements:** FR-NEW-005
- **Given** black list `BLOCKED.Example`. **When** Alice fetches `http://Blocked.EXAMPLE/`. **Then** she gets `ERR_FORBIDDEN: host 'blocked.example' is not allowed by the web access lists`.

#### E2E-015: neighbour address outside an entry
- **Category:** edge. **Scenario:** SC-002. **Requirements:** FR-NEW-005
- **Given** black list `10.0.0.5` and a site at `10.0.0.6` answering `Six`. **When** Alice fetches `http://10.0.0.6/`. **Then** she receives `Six`.

#### E2E-016: range boundaries
- **Category:** edge. **Scenario:** SC-002. **Requirements:** FR-NEW-005
- **Given** black list `10.0.0.0/24` and a site at `10.0.1.0` answering `Next`. **When** Alice fetches `http://10.0.0.0/`, then `http://10.0.0.255/`, then `http://10.0.1.0/`. **Then** the first two are refused and the third returns `Next`.

#### E2E-053: trailing dot does not bypass the list
- **Category:** edge. **Scenario:** SC-002. **Requirements:** FR-NEW-005
- **Given** black list `blocked.example`. **When** Alice fetches `http://blocked.example./`. **Then** she gets `ERR_FORBIDDEN: host 'blocked.example' is not allowed by the web access lists`.

#### E2E-054: unresolvable name under a black list
- **Category:** failure. **Scenario:** SC-002. **Requirements:** FR-NEW-006
- **Given** black list `10.0.0.0/8` and a name `ghost.example` that resolves to no address. **When** Alice fetches `http://ghost.example/`. **Then** her answer starts with `ERR_INTERNAL_ERROR: fetch request failed` and is not the lists refusal.

#### E2E-055: unresolvable name, download
- **Category:** edge. **Scenario:** SC-002. **Requirements:** FR-NEW-006, FR-NEW-009
- **Given** black list `10.0.0.0/8` and `ghost.example` resolving to no address. **When** Alice downloads `http://ghost.example/g.pdf` into `/g.pdf`. **Then** her answer starts with `ERR_INTERNAL_ERROR: fetch request failed`. **And** `/g.pdf` does not exist.

#### E2E-057: a name written with accents
- **Category:** edge. **Scenario:** SC-002. **Requirements:** FR-NEW-005
- **Given** black list `bücher.example`. **When** Alice fetches `http://xn--bcher-kva.example/`. **Then** she gets `ERR_FORBIDDEN: host 'xn--bcher-kva.example' is not allowed by the web access lists`.

#### E2E-058: an address with a zone is never sent
- **Category:** failure. **Scenario:** SC-006. **Requirements:** FR-NEW-006
- **Given** black list `fe80::/10` and a stand in at `fe80::1`. **When** Alice fetches `http://[fe80::1%25eth0]/`. **Then** her answer starts with `ERR_INTERNAL_ERROR: fetch request failed`. **And** the stand in counts 0 requests.

#### E2E-061: a range written with bits below its prefix
- **Category:** edge. **Scenario:** SC-002. **Requirements:** FR-NEW-005
- **Given** black list `10.0.0.5/24` and a site at `10.0.0.200` answering `Edge`. **When** Alice fetches `http://10.0.0.200/`. **Then** she gets `ERR_FORBIDDEN: host '10.0.0.200' is not allowed by the web access lists`.

#### E2E-064: a redirect loop keeps today's failure
- **Category:** failure. **Scenario:** SC-005. **Requirements:** FR-NEW-007
- **Given** black list `blocked.example` and `news.example` redirecting to itself without end. **When** Alice fetches `http://news.example/`. **Then** her answer starts with `ERR_INTERNAL_ERROR: fetch request failed` and is not the lists refusal.

#### E2E-065: port and credentials are not in the refusal
- **Category:** edge. **Scenario:** SC-002. **Requirements:** FR-NEW-008
- **Given** black list `blocked.example`. **When** Alice fetches `http://u:p@Blocked.example:8080/`. **Then** she gets `ERR_FORBIDDEN: host 'blocked.example' is not allowed by the web access lists`, with no `u:p` and no `8080`.

#### E2E-074: a redirect without a destination, black list set
- **Category:** edge. **Scenario:** SC-005. **Requirements:** FR-NEW-007
- **Given** black list `blocked.example` and `news.example` answering status 302 with no destination and the body `Moved`. **When** Alice fetches `http://news.example/`. **Then** she receives `Moved`, not `ERR_INTERNAL_ERROR` and not the lists refusal.

#### E2E-077: a hop to a refused site on a non web scheme
- **Category:** edge. **Scenario:** SC-005. **Requirements:** FR-NEW-007
- **Given** black list `blocked.example` and `news.example` redirecting to `ftp://blocked.example/`. **When** Alice fetches `http://news.example/`. **Then** she gets `ERR_FORBIDDEN: host 'blocked.example' is not allowed by the web access lists`.

#### E2E-078: a hop to an allowed site on a non web scheme
- **Category:** failure. **Scenario:** SC-005. **Requirements:** FR-NEW-007
- **Given** black list `blocked.example` and `news.example` redirecting to `ftp://docs.example/`. **When** Alice fetches `http://news.example/`. **Then** her answer starts with `ERR_INTERNAL_ERROR: fetch request failed`.

#### E2E-079: the refused site is the eleventh hop
- **Category:** edge. **Scenario:** SC-005. **Requirements:** FR-NEW-007
- **Given** black list `blocked.example` and a chain of 10 allowed hops whose next hop leads to `http://blocked.example/`. **When** Alice fetches the first. **Then** she gets `ERR_FORBIDDEN: host 'blocked.example' is not allowed by the web access lists`, not the too many hops failure.

#### E2E-017: a refused request never reaches the site
- **Category:** side effect. **Scenario:** SC-002. **Requirements:** FR-NEW-008
- **Given** black list `blocked.example` and a stand in for it. **When** Alice fetches it. **Then** she gets the refusal. **And** the stand in counts 0 requests.

#### E2E-018: web search unaffected
- **Category:** edge. **Scenario:** SC-002. **Requirements:** FR-NEW-011
- **Given** the search site of the product is `search.example` (a stand in answering `Result A`) and black list `search.example`. **When** Alice asks `web.search` for `weather`. **Then** she receives `Result A`.

**SC-003**

#### E2E-019: white listed name served
- **Category:** happy. **Scenario:** SC-003. **Requirements:** FR-NEW-003
- **Given** white list `docs.example`. **When** Alice fetches `http://docs.example/`. **Then** she receives `Manual v1`.

#### E2E-020: white listed range served
- **Category:** happy. **Scenario:** SC-003. **Requirements:** FR-NEW-003, FR-NEW-005
- **Given** white list `10.0.0.0/24` and a site at `10.0.0.7` answering `Seven`. **When** Alice fetches `http://10.0.0.7/`. **Then** she receives `Seven`.

#### E2E-021: other site refused
- **Category:** failure. **Scenario:** SC-003. **Requirements:** FR-NEW-003, FR-NEW-008
- **Given** white list `docs.example`. **When** Alice fetches `http://news.example/`. **Then** she gets `ERR_FORBIDDEN: host 'news.example' is not allowed by the web access lists`.

#### E2E-022: metadata address not white listed
- **Category:** failure. **Scenario:** SC-003. **Requirements:** FR-NEW-003
- **Given** white list `docs.example`. **When** Alice fetches `http://169.254.169.254/latest/meta-data/`. **Then** she gets `ERR_FORBIDDEN: host '169.254.169.254' is not allowed by the web access lists`.

#### E2E-023: unresolvable name is not white listed by address
- **Category:** failure. **Scenario:** SC-003. **Requirements:** FR-NEW-003, FR-NEW-006
- **Given** white list `10.0.0.0/24` and a name `ghost.example` that resolves to no address. **When** Alice fetches `http://ghost.example/`. **Then** she gets `ERR_FORBIDDEN: host 'ghost.example' is not allowed by the web access lists`.

#### E2E-024: wildcard in the white list excludes the bare name
- **Category:** edge. **Scenario:** SC-003. **Requirements:** FR-NEW-003, FR-NEW-005
- **Given** white list `*.docs.example` and a site `a.docs.example` answering `Sub`. **When** Alice fetches `http://a.docs.example/` then `http://docs.example/`. **Then** the first returns `Sub` and the second is refused.

#### E2E-025: a name resolving inside a white listed range
- **Category:** edge. **Scenario:** SC-003. **Requirements:** FR-NEW-006
- **Given** white list `10.1.2.0/24`. **When** Alice fetches `http://friendly.example/`. **Then** she receives `Internal payroll`.

#### E2E-026: a name resolving partly outside the white list
- **Category:** edge. **Scenario:** SC-003. **Requirements:** FR-NEW-006
- **Given** white list `10.1.2.0/24`. **When** Alice fetches `http://mixed.example/`. **Then** she gets `ERR_FORBIDDEN: host 'mixed.example' is not allowed by the web access lists`.

#### E2E-027: allowed fetch with a destination saves
- **Category:** side effect. **Scenario:** SC-003. **Requirements:** FR-NEW-003
- **Given** white list `docs.example`. **When** Alice fetches `http://docs.example/` with destination `/page.html` in `proj`. **Then** `/page.html` holds `Manual v1`.

#### E2E-028: web news unaffected
- **Category:** edge. **Scenario:** SC-003. **Requirements:** FR-NEW-011
- **Given** the search site of the product is `search.example` (a stand in answering `Headline B`) and white list `docs.example`. **When** Alice asks `web.news` for `economy`. **Then** she receives `Headline B`.

**SC-004**

#### E2E-029: white minus black, served
- **Category:** happy. **Scenario:** SC-004. **Requirements:** FR-NEW-004
- **Given** white list `*.docs.example`, black list `secret.docs.example`. **When** Alice fetches `http://www.docs.example/` (page `Public manual`). **Then** she receives `Public manual`.

#### E2E-030: a site in both lists is refused
- **Category:** failure. **Scenario:** SC-004. **Requirements:** FR-NEW-004
- **Given** white list `*.docs.example`, black list `secret.docs.example`. **When** Alice fetches `http://secret.docs.example/`. **Then** she gets `ERR_FORBIDDEN: host 'secret.docs.example' is not allowed by the web access lists`.

#### E2E-031: outside the white list, refused
- **Category:** failure. **Scenario:** SC-004. **Requirements:** FR-NEW-004
- **Given** white list `*.docs.example`, black list `secret.docs.example`. **When** Alice fetches `http://news.example/`. **Then** she gets the refusal naming `news.example`.

#### E2E-032: white range minus one black address
- **Category:** edge. **Scenario:** SC-004. **Requirements:** FR-NEW-004, FR-NEW-005
- **Given** white list `10.0.0.0/24`, black list `10.0.0.5`. **When** Alice fetches `http://10.0.0.5/` and then `http://10.0.0.6/` (page `Six`). **Then** the first is refused and the second returns `Six`.

#### E2E-033: allowed download under both lists saves
- **Category:** side effect. **Scenario:** SC-004. **Requirements:** FR-NEW-004
- **Given** white list `*.docs.example`, black list `secret.docs.example`. **When** Alice downloads `http://www.docs.example/m.pdf` (`Public manual`) into `/m.pdf`. **Then** `/m.pdf` holds `Public manual`.

#### E2E-034: web suggestions unaffected
- **Category:** edge. **Scenario:** SC-004. **Requirements:** FR-NEW-011
- **Given** the search site of the product is `search.example` (a stand in answering `weather today`), white list `docs.example` and black list `secret.docs.example`. **When** Alice asks `web.suggestions` for `weat`. **Then** she receives `weather today`.

**SC-005**

#### E2E-035: redirect to an allowed site
- **Category:** happy. **Scenario:** SC-005. **Requirements:** FR-NEW-007
- **Given** black list `blocked.example`; `news.example` redirects to `docs.example`. **When** Alice fetches `http://news.example/`. **Then** she receives `Manual v1`.

#### E2E-036: redirect to a refused site
- **Category:** failure. **Scenario:** SC-005. **Requirements:** FR-NEW-007
- **Given** white list `news.example`; `news.example` redirects to `http://docs.example/`. **When** Alice fetches `http://news.example/`. **Then** she gets `ERR_FORBIDDEN: host 'docs.example' is not allowed by the web access lists`. **And** a stand in for `docs.example` counts 0 requests.

#### E2E-037: second hop of a chain refused
- **Category:** edge. **Scenario:** SC-005. **Requirements:** FR-NEW-007
- **Given** black list `blocked.example`; `news.example` redirects to `docs.example`, which redirects to `http://blocked.example/`. **When** Alice fetches `http://news.example/`. **Then** she gets the refusal naming `blocked.example`.

#### E2E-038: download refused at a hop saves nothing
- **Category:** side effect. **Scenario:** SC-005. **Requirements:** FR-NEW-007, FR-NEW-009
- **Given** white list `news.example`; `news.example` redirects to `docs.example`. **When** Alice downloads `http://news.example/f.pdf` into `/f.pdf`. **Then** she gets the refusal naming `docs.example`. **And** `/f.pdf` does not exist.

**SC-006**

#### E2E-039: a name pointing to a public address
- **Category:** happy. **Scenario:** SC-006. **Requirements:** FR-NEW-006
- **Given** black list `10.0.0.0/8`. **When** Alice fetches `http://news.example/` (resolves to `93.184.216.34`). **Then** she receives `Daily headlines`.

#### E2E-062: an IPv4 mapped entry covers the plain address
- **Category:** edge. **Scenario:** SC-006. **Requirements:** FR-NEW-005, FR-NEW-006
- **Given** black list `::ffff:127.0.0.1` and a stand in at `127.0.0.1`. **When** Alice fetches `http://127.0.0.1/`. **Then** she gets `ERR_FORBIDDEN: host '127.0.0.1' is not allowed by the web access lists`. **And** the stand in counts 0 requests.

#### E2E-068: blocked name with a proxy in the environment
- **Category:** failure. **Scenario:** SC-006. **Requirements:** FR-NEW-006
- **Given** the server started with a web proxy set in its environment (a stand in proxy), black list `blocked.example`. **When** Alice fetches `http://blocked.example/`. **Then** she gets `ERR_FORBIDDEN: host 'blocked.example' is not allowed by the web access lists`. **And** the stand in proxy counts 0 requests.

#### E2E-069: allowed site with a proxy in the environment
- **Category:** happy. **Scenario:** SC-006. **Requirements:** FR-NEW-006
- **Given** the same proxy and black list `blocked.example`. **When** Alice fetches `http://news.example/`. **Then** she receives the proxy's answer `Daily headlines`. **And** the stand in proxy counts 1 request (the proxy makes the connection).

#### E2E-070: name resolved by the server to a black listed address, with a proxy
- **Category:** edge. **Scenario:** SC-006. **Requirements:** FR-NEW-006
- **Given** the same proxy, black list `10.0.0.0/8` and `friendly.example` resolving (on the server) to `10.1.2.3`. **When** Alice fetches `http://friendly.example/`. **Then** she gets `ERR_FORBIDDEN: host 'friendly.example' is not allowed by the web access lists`. **And** the stand in proxy counts 0 requests.

#### E2E-073: a mapped range and a range of length zero
- **Category:** edge. **Scenario:** SC-006. **Requirements:** FR-NEW-005, FR-NEW-006
- **Given** a stand in at `10.1.2.3` answering `Ten`, and black list `["::ffff:10.0.0.0/104"]`, then separately `["0.0.0.0/0"]`. **When** Alice fetches `http://10.1.2.3/` in each run. **Then** both times she gets `ERR_FORBIDDEN: host '10.1.2.3' is not allowed by the web access lists`. **And** the stand in counts 0 requests.

#### E2E-075: an IPv6 range of length zero does not cover plain IPv4
- **Category:** edge. **Scenario:** SC-006. **Requirements:** FR-NEW-005, FR-NEW-006
- **Given** black list `["::/0"]`, a stand in at `127.0.0.1` answering `Local` and a stand in at `fd12::1` answering `V6`. **When** Alice fetches `http://127.0.0.1/`, then `http://[fd12::1]/`. **Then** the first returns `Local` and the second gives `ERR_FORBIDDEN: host 'fd12::1' is not allowed by the web access lists`.

#### E2E-080: an address with no host is never sent
- **Category:** failure. **Scenario:** SC-006. **Requirements:** FR-NEW-006
- **Given** black list `blocked.example`. **When** Alice fetches `http://`. **Then** her answer starts with `ERR_INTERNAL_ERROR: fetch request failed` and is not the lists refusal.

#### E2E-040: two addresses, one black listed
- **Category:** edge. **Scenario:** SC-006. **Requirements:** FR-NEW-006
- **Given** black list `10.0.0.0/8`. **When** Alice fetches `http://mixed.example/`. **Then** she gets `ERR_FORBIDDEN: host 'mixed.example' is not allowed by the web access lists`.

#### E2E-052: refused name leaves no file
- **Category:** side effect. **Scenario:** SC-006. **Requirements:** FR-NEW-006, FR-NEW-009
- **Given** black list `10.0.0.0/8`. **When** Alice fetches `http://friendly.example/` with destination `/p.html`. **Then** she gets the refusal naming `friendly.example`. **And** `/p.html` does not exist.

**SC-007**

#### E2E-041: valid lists start the server
- **Category:** happy. **Scenario:** SC-007. **Requirements:** FR-NEW-010
- **Given** black list `["10.0.0.0/8", "*.corp.example", "fd00::/8", "blocked.example"]` and white list `["docs.example"]`. **When** the operator starts the server. **Then** it starts.

#### E2E-042: an address with a scheme and path
- **Category:** failure. **Scenario:** SC-007. **Requirements:** FR-NEW-010
- **Given** black list `["https://blocked.example/x"]`. **When** the server starts. **Then** it prints `ERR_INVALID_ARGUMENT: invalid entry 'https://blocked.example/x' in the web black list: expected a host name, a *.host name, an IP address or an address range` and exits non zero.

#### E2E-043: impossible range length
- **Category:** failure. **Scenario:** SC-007. **Requirements:** FR-NEW-010
- **Given** white list `["10.0.0.0/33"]`. **When** the server starts. **Then** it prints `ERR_INVALID_ARGUMENT: invalid entry '10.0.0.0/33' in the web white list: expected a host name, a *.host name, an IP address or an address range`.

#### E2E-044: wildcard alone
- **Category:** failure. **Scenario:** SC-007. **Requirements:** FR-NEW-010
- **Given** black list `["*"]`. **When** the server starts. **Then** it prints `ERR_INVALID_ARGUMENT: invalid entry '*' in the web black list: expected a host name, a *.host name, an IP address or an address range` and exits non zero.

#### E2E-045: an entry with a port
- **Category:** failure. **Scenario:** SC-007. **Requirements:** FR-NEW-010
- **Given** black list `["example.com:8080"]`. **When** the server starts. **Then** it prints `ERR_INVALID_ARGUMENT: invalid entry 'example.com:8080' in the web black list: expected a host name, a *.host name, an IP address or an address range` and exits non zero.

#### E2E-046: upper case wildcard and IPv6 accepted
- **Category:** edge. **Scenario:** SC-007. **Requirements:** FR-NEW-005, FR-NEW-010
- **Given** black list `["*.Corp.EXAMPLE", "FD12::1"]`. **When** the server starts and Alice fetches `http://wiki.corp.example/`. **Then** the server starts and Alice gets the refusal naming `wiki.corp.example`.

#### E2E-056: malformed name entry
- **Category:** failure. **Scenario:** SC-007. **Requirements:** FR-NEW-005, FR-NEW-010
- **Given** black list `["a..b"]`. **When** the server starts. **Then** it prints `ERR_INVALID_ARGUMENT: invalid entry 'a..b' in the web black list: expected a host name, a *.host name, an IP address or an address range`.

#### E2E-059: numeric entry that is not a four part address
- **Category:** failure. **Scenario:** SC-007. **Requirements:** FR-NEW-005, FR-NEW-010
- **Given** white list `["2130706433"]`, and then `["0x7f000001"]`, and then `["10.1"]`. **When** the server starts with each. **Then** each time it prints `ERR_INVALID_ARGUMENT: invalid entry '<entry>' in the web white list: expected a host name, a *.host name, an IP address or an address range` with that entry, and exits non zero.

#### E2E-060: more malformed entries
- **Category:** failure. **Scenario:** SC-007. **Requirements:** FR-NEW-005, FR-NEW-010
- **Given** black list `["010.0.0.1"]`, then `[" blocked.example"]` (leading space), then `["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.example"]` (the label is 64 times `a`), then `["xn--a"]`. **When** the server starts with each. **Then** each time it prints `ERR_INVALID_ARGUMENT: invalid entry '<entry>' in the web black list: expected a host name, a *.host name, an IP address or an address range` with that entry, and exits non zero.

#### E2E-063: the web switch off does not skip the check
- **Category:** failure. **Scenario:** SC-007. **Requirements:** FR-NEW-010, FR-NEW-012
- **Given** the web tools off and black list `["*"]`. **When** the server starts. **Then** it prints `ERR_INVALID_ARGUMENT: invalid entry '*' in the web black list: expected a host name, a *.host name, an IP address or an address range` and exits non zero.

#### E2E-066: valid lists with the web tools off
- **Category:** happy. **Scenario:** SC-007. **Requirements:** FR-NEW-012
- **Given** the web tools off and black list `["10.0.0.0/8"]`. **When** the server starts. **Then** it starts.

#### E2E-067: bad white list entry with the web tools off
- **Category:** failure. **Scenario:** SC-007. **Requirements:** FR-NEW-010, FR-NEW-012
- **Given** the web tools off and white list `["10.0.0.0/33"]`. **When** the server starts. **Then** it prints `ERR_INVALID_ARGUMENT: invalid entry '10.0.0.0/33' in the web white list: expected a host name, a *.host name, an IP address or an address range` and exits non zero.

#### E2E-072: malformed range lengths and bracketed or zoned IPv6 entries
- **Category:** failure. **Scenario:** SC-007. **Requirements:** FR-NEW-005, FR-NEW-010
- **Given** black list `["10.0.0.0/08"]`, then `["10.0.0.0/+8"]`, then `["[fd00::1]"]`, then `["fe80::1%eth0"]`. **When** the server starts with each. **Then** each time it prints `ERR_INVALID_ARGUMENT: invalid entry '<entry>' in the web black list: expected a host name, a *.host name, an IP address or an address range` with that entry, and exits non zero.

#### E2E-047: a refused start leaves nothing running
- **Category:** side effect. **Scenario:** SC-007. **Requirements:** FR-NEW-010
- **Given** black list `["*"]`. **When** the server starts and refuses. **Then** nothing listens on the server's port.

**SC-008**

#### E2E-048: allowed download under a black list
- **Category:** happy. **Scenario:** SC-008. **Requirements:** FR-NEW-009
- **Given** black list `blocked.example`. **When** Alice downloads `http://docs.example/m.pdf` into `/m.pdf`. **Then** `/m.pdf` holds `Manual v1`.

#### E2E-049: a non member sees the membership refusal
- **Category:** failure. **Scenario:** SC-008. **Requirements:** FR-NEW-008
- **Given** black list `blocked.example`. **When** Bob asks `web.download` for `http://blocked.example/x` into `proj`. **Then** he gets `ERR_FORBIDDEN: 'bob@example.com' is not a member of 'proj'`, not the lists refusal.

#### E2E-076: refusal whatever the scheme, in save mode
- **Category:** edge. **Scenario:** SC-008. **Requirements:** FR-NEW-008
- **Given** black list `blocked.example`. **When** Alice fetches `ftp://blocked.example/` with destination `/f.txt`, and then `ftp://news.example/` with destination `/g.txt`. **Then** the first gives `ERR_FORBIDDEN: host 'blocked.example' is not allowed by the web access lists`; the second gives an answer starting with `ERR_INTERNAL_ERROR: fetch request failed`. **And** neither `/f.txt` nor `/g.txt` exists.

#### E2E-081: an address with no scheme, save mode
- **Category:** failure. **Scenario:** SC-008. **Requirements:** FR-NEW-006
- **Given** white list `docs.example`. **When** Alice fetches `docs.example/x` with destination `/p.html`. **Then** her answer starts with `ERR_INTERNAL_ERROR: fetch request failed`. **And** `/p.html` does not exist.

#### E2E-082: hyphens at the edge of a label, and a valid punycode label
- **Category:** edge. **Scenario:** SC-007. **Requirements:** FR-NEW-005, FR-NEW-010
- **Given** black list `["-a.example"]`, then `["a-.example"]`, then `["xn--bcher-kva.example"]`. **When** the server starts with each. **Then** it starts each time. **And** with black list `["-a.example"]`, Alice fetching `http://-a.example/` gets `ERR_FORBIDDEN: host '-a.example' is not allowed by the web access lists`.

#### E2E-083: other written forms of the own machine
- **Category:** edge. **Scenario:** SC-006. **Requirements:** FR-NEW-006
- **Given** black list `127.0.0.0/8` and a stand in at `127.0.0.1`. **When** Alice fetches `http://127.1/`, then `http://0177.0.0.1/`, then `http://0x7f.1/`. **Then** each time she gets `ERR_FORBIDDEN: host '127.0.0.1' is not allowed by the web access lists`. **And** the stand in counts 0 requests.

#### E2E-084: a name with a sharp s stays distinct
- **Category:** edge. **Scenario:** SC-002. **Requirements:** FR-NEW-005
- **Given** black list `straße.example` and a site `strasse.example` answering `Plain`. **When** Alice fetches `http://strasse.example/`, then `http://straße.example/`. **Then** the first returns `Plain` and the second gives `ERR_FORBIDDEN: host 'xn--strae-oqa.example' is not allowed by the web access lists`.

#### E2E-085: an empty punycode label
- **Category:** failure. **Scenario:** SC-007. **Requirements:** FR-NEW-005, FR-NEW-010
- **Given** black list `["xn--"]`, then `["XN--"]`. **When** the server starts with each. **Then** it prints `ERR_INVALID_ARGUMENT: invalid entry '<entry>' in the web black list: expected a host name, a *.host name, an IP address or an address range` with that entry, and exits non zero.

#### E2E-086: more than one trailing dot
- **Category:** failure. **Scenario:** SC-007. **Requirements:** FR-NEW-005, FR-NEW-010
- **Given** black list `["blocked.example.."]`, then `["."]`. **When** the server starts with each. **Then** it prints `ERR_INVALID_ARGUMENT: invalid entry '<entry>' in the web black list: expected a host name, a *.host name, an IP address or an address range` with that entry, and exits non zero.

#### E2E-087: a lookup that times out
- **Category:** failure. **Scenario:** SC-003. **Requirements:** FR-NEW-006
- **Given** white list `docs.example` and a name `slow.example` whose lookup does not answer within the request timeout. **When** Alice fetches `http://slow.example/` with `timeout_secs` 1. **Then** she gets `ERR_INTERNAL_ERROR: request timed out`.

#### E2E-088: a protocol relative redirect to a listed site
- **Category:** edge. **Scenario:** SC-005. **Requirements:** FR-NEW-007
- **Given** black list `blocked.example` and `news.example` redirecting to `//blocked.example/x`. **When** Alice fetches `http://news.example/`. **Then** she gets `ERR_FORBIDDEN: host 'blocked.example' is not allowed by the web access lists`. **And** a stand in for `blocked.example` counts 0 requests.

#### E2E-089: the purge command refuses a bad entry
- **Category:** failure. **Scenario:** SC-007. **Requirements:** FR-NEW-012
- **Given** black list `["*"]` and a project with expired files. **When** the operator runs `mcp-fs purge`. **Then** it prints `error: ERR_INVALID_ARGUMENT: invalid entry '*' in the web black list: expected a host name, a *.host name, an IP address or an address range`, exits non zero, and no file is purged.

#### E2E-090: the migrate command refuses a bad entry
- **Category:** failure. **Scenario:** SC-007. **Requirements:** FR-NEW-012
- **Given** a source configuration with white list `["10.0.0.0/33"]`. **When** the operator runs `mcp-fs migrate` with it. **Then** it prints `error: ERR_INVALID_ARGUMENT: invalid entry '10.0.0.0/33' in the web white list: expected a host name, a *.host name, an IP address or an address range`, exits non zero, and nothing is copied.

#### E2E-091: the purge command accepts valid lists
- **Category:** happy. **Scenario:** SC-007. **Requirements:** FR-NEW-012
- **Given** black list `["10.0.0.0/8"]`. **When** the operator runs `mcp-fs purge`. **Then** it runs and exits zero.

#### E2E-050: an existing file is untouched by a refused save
- **Category:** edge. **Scenario:** SC-008. **Requirements:** FR-NEW-009
- **Given** black list `blocked.example` and `/page.html` holding `Old`. **When** Alice fetches `http://blocked.example/` with destination `/page.html`. **Then** she gets the refusal. **And** `/page.html` still holds `Old`.

#### E2E-051: storage use unchanged, and the lists refusal comes first
- **Category:** side effect. **Scenario:** SC-008. **Requirements:** FR-NEW-009, FR-NEW-008
- **Given** black list `blocked.example`, `proj` using 100 bytes and its storage allowance fully used. **When** Alice downloads `http://blocked.example/m.pdf` into `/m.pdf`. **Then** she gets the lists refusal, not a storage refusal. **And** `proj` still uses 100 bytes.

### 9.3 Exploit tests (`--security`)

Each test carries the literal payload. Each asserts the refusal AND, through a second channel, that the stand in site received 0 requests. Red before the fix because: with both lists empty at that time, the call returns the stand in's body instead of a refusal (`expected the refusal, got the page text`).

#### E2E-SEC-001: cloud metadata credentials
- **Covers:** class sweep occurrence #1 (Section 14.2). **Requirements:** FR-NEW-002, FR-NEW-008
- **Given** black list `169.254.0.0/16` and a stand in at `169.254.169.254` answering `AccessKeyId=AKIAEXAMPLE`. **When** Alice fetches `http://169.254.169.254/latest/meta-data/iam/security-credentials/`. **Then** she gets `ERR_FORBIDDEN: host '169.254.169.254' is not allowed by the web access lists`, and her answer does not contain `AKIAEXAMPLE`. **And** the stand in counts 0 requests.

#### E2E-SEC-002: redirect from a public page to the metadata address
- **Covers:** occurrence #1. **Requirements:** FR-NEW-007
- **Given** black list `169.254.0.0/16`; `news.example` answers with a redirect to `http://169.254.169.254/latest/meta-data/`. **When** Alice fetches `http://news.example/`. **Then** she gets the refusal naming `169.254.169.254`. **And** the stand in at that address counts 0 requests.

#### E2E-SEC-003: a name that points to the own machine
- **Covers:** occurrence #1. **Requirements:** FR-NEW-006
- **Given** black list `127.0.0.0/8`; `evil.example` resolves to `127.0.0.1` where a stand in answers `ADMIN PANEL`. **When** Alice fetches `http://evil.example/admin`. **Then** she gets the refusal naming `evil.example` without `127.0.0.1`. **And** the stand in counts 0 requests.

#### E2E-SEC-008: a name resolving to an IPv4 mapped IPv6 address
- **Covers:** occurrence #1. **Requirements:** FR-NEW-006
- **Given** black list `127.0.0.0/8`; `evil6.example` resolves to `::ffff:127.0.0.1` where a stand in answers `ADMIN PANEL`. **When** Alice fetches `http://evil6.example/admin`. **Then** she gets `ERR_FORBIDDEN: host 'evil6.example' is not allowed by the web access lists`, without `127.0.0.1`. **And** the stand in counts 0 requests.

#### E2E-SEC-009: a name whose second lookup differs
- **Covers:** occurrence #1. **Requirements:** FR-NEW-006
- **Given** black list `127.0.0.0/8`; `rebind.example` answers `93.184.216.34` on its first lookup and `127.0.0.1` on every later one; a stand in at `127.0.0.1` answers `ADMIN PANEL`. **When** Alice fetches `http://rebind.example/admin`. **Then** her answer does not contain `ADMIN PANEL`. **And** the stand in at `127.0.0.1` counts 0 requests.

#### E2E-SEC-004: decimal form of the own machine
- **Covers:** occurrence #1. **Requirements:** FR-NEW-006
- **Given** black list `127.0.0.0/8` and a stand in at `127.0.0.1`. **When** Alice fetches `http://2130706433/`. **Then** she gets `ERR_FORBIDDEN: host '127.0.0.1' is not allowed by the web access lists`. **And** the stand in counts 0 requests.

#### E2E-SEC-005: IPv4 mapped IPv6 form
- **Covers:** occurrence #1. **Requirements:** FR-NEW-006
- **Given** black list `127.0.0.0/8` and a stand in at `127.0.0.1`. **When** Alice fetches `http://[::ffff:127.0.0.1]/`. **Then** she gets `ERR_FORBIDDEN: host '::ffff:7f00:1' is not allowed by the web access lists`. **And** the stand in counts 0 requests.

#### E2E-SEC-006: download of the metadata address
- **Covers:** occurrence #1 reached through `web.download`. **Requirements:** FR-NEW-002, FR-NEW-009
- **Given** black list `169.254.0.0/16` and the same stand in. **When** Alice asks `web.download` for `http://169.254.169.254/latest/meta-data/iam/security-credentials/` into `/creds.txt`. **Then** she gets the refusal. **And** `/creds.txt` does not exist and the stand in counts 0 requests.

#### E2E-SEC-007: fetch with a destination of the metadata address
- **Covers:** occurrence #1 reached through the save mode of `web.fetch`. **Requirements:** FR-NEW-002, FR-NEW-009
- **Given** the same setup. **When** Alice fetches that address with destination `/creds.txt`. **Then** she gets the refusal. **And** `/creds.txt` does not exist and the stand in counts 0 requests.

### 9.4 Modified tests
None.

### 9.5 Removed tests
None.

## 10. Traceability

| Scenario | Requirements | Happy | Failure | Edge | Side effect |
|---|---|---|---|---|---|
| SC-001 | FR-NEW-001 | E2E-001, 002 | E2E-004, 005 | E2E-003, 071 | E2E-006 |
| SC-002 | FR-NEW-002, 005, 006, 008, 009, 011 | E2E-007 | E2E-008 to 012, 054, E2E-SEC-001 | E2E-013 to 016, 018, 053, 055, 057, 061, 065, 084 | E2E-017 |
| SC-003 | FR-NEW-003, 005, 006, 008, 011 | E2E-019, 020 | E2E-021 to 023, 087 | E2E-024 to 026, 028 | E2E-027 |
| SC-004 | FR-NEW-004, 005, 011 | E2E-029 | E2E-030, 031 | E2E-032, 034 | E2E-033 |
| SC-005 | FR-NEW-007, 009 | E2E-035 | E2E-036, 064, 078, E2E-SEC-002 | E2E-037, 074, 077, 079, 088 | E2E-038 |
| SC-006 | FR-NEW-005, 006, 009 | E2E-039, 069 | E2E-058, 068, 080, E2E-SEC-003 to E2E-SEC-005, E2E-SEC-008, E2E-SEC-009 | E2E-040, 062, 070, 073, 075, 083 | E2E-052 |
| SC-007 | FR-NEW-005, 010, 012 | E2E-041, 066, 091 | E2E-042 to 045, 056, 059, 060, 063, 067, 072, 085, 086, 089, 090 | E2E-046, 082 | E2E-047 |
| SC-008 | FR-NEW-002, 006, 008, 009 | E2E-048 | E2E-049, 081, E2E-SEC-006, E2E-SEC-007 | E2E-050, 076 | E2E-051 |

| Requirement | Tests |
|---|---|
| FR-NEW-001 | E2E-001 to 006, 071 |
| FR-NEW-002 | E2E-007 to 012, E2E-SEC-001, E2E-SEC-006, E2E-SEC-007 |
| FR-NEW-003 | E2E-019 to 024, 027 |
| FR-NEW-004 | E2E-029 to 033 |
| FR-NEW-005 | E2E-009, 010, 012 to 016, 020, 024, 032, 046, 053, 056, 057, 059, 060, 061, 062, 072, 073, 075, 082, 084, 085, 086 |
| FR-NEW-006 | E2E-011, 023, 025, 026, 039, 040, 052, 054, 055, 058, 062, 068, 069, 070, 073, 075, 080, 081, 083, 087, E2E-SEC-003 to E2E-SEC-005, E2E-SEC-008, E2E-SEC-009 |
| FR-NEW-007 | E2E-035 to 038, 064, 074, 077, 078, 079, 088, E2E-SEC-002 |
| FR-NEW-008 | E2E-008, 017, 021, 049, 051, 065, 076, E2E-SEC-001 |
| FR-NEW-009 | E2E-038, 048, 050, 051, 052, 055, E2E-SEC-006, E2E-SEC-007 |
| FR-NEW-010 | E2E-041 to 047, 056, 059, 060, 063, 067, 072, 082, 085, 086 |
| FR-NEW-011 | E2E-018, 028, 034 |
| FR-NEW-012 | E2E-063, 066, 067, 089, 090, 091 |

## 11. Open questions
None.

## 12. Glossary (MANDATORY)

| Term | Definition | Context |
|---|---|---|
| Black list | The operator's list of sites the web tools must not reach. | Web access |
| White list | The operator's list of sites the web tools can reach, when it is not empty. | Web access |
| Entry | One item of a list: a host name, a wildcard name, an address or an address range. | Web access |
| Match | An entry covers the site as typed or an address that site resolves to, by FR-NEW-005 and FR-NEW-006. | Web access |
| Site as typed | The host written in the request or given by a redirect, in lower case, in its ASCII form, without trailing dot, port, user name or password. | Web access |
| Destination path | The place in a project where a download or a saved fetch writes its file. | Project files |
| Resolved address | An address a site name points to. | Web access |
| Refusal | The answer `ERR_FORBIDDEN: host '<site>' is not allowed by the web access lists`, given before any request is sent. | Web access |
| Hop | One redirect in a chain of redirects. | Web access |
| Stand in | A test site that counts the requests it receives. | Tests |
| Storage use | The bytes a project's files occupy against its allowance. | Project files |

## 13. Implementability gate (MANDATORY)

Auditor: a fresh general purpose subagent per round, given only the document path and the audit contract (no transcript). Consistency check (Phase 10): a fresh reviewer subagent per round, last run on the final text. The test designer was the main run (Section 9 preface of the plan approved by the owner on 2026-11-16; the plan grew from 46 to 100 tests through the gate amendments below).

| Round | F | A | Verdict |
|---|---|---|---|
| 1 | 6 | 3 | NOT-IMPLEMENTABLE |
| 2 | 5 | 3 | NOT-IMPLEMENTABLE |
| 3 | 2 | 2 | NOT-IMPLEMENTABLE (cap reached, escalated to the owner: proxy, device flow row) |
| 4 | 2 | 2 | NOT-IMPLEMENTABLE |
| 5 | 3 | 4 | NOT-IMPLEMENTABLE |
| 6 | 3 | 0 | NOT-IMPLEMENTABLE |
| 7 | 1 | 1 | NOT-IMPLEMENTABLE |
| 8 | 4 | 0 | NOT-IMPLEMENTABLE |
| 9 | 1 | 0 | NOT-IMPLEMENTABLE |
| 10 | 1 | 2 | NOT-IMPLEMENTABLE |
| 11 | 0 | 4 | IMPLEMENTABLE |

**Amendments applied:** FR-NEW-005 entry and request reading rules (WHATWG host reading, UTS 46, trailing dot, range lengths, mapped ranges, hyphens, punycode); FR-NEW-006 lookup errors, mapped resolved addresses, unreadable addresses, proxy limit (owner option A); FR-NEW-007 hop order, redirect behavior unchanged, protocol relative destinations; FR-NEW-008 normalized `<site>`, scheme independence; FR-NEW-010 message prefix and order; FR-NEW-012 boot check for `serve`, `purge`, `migrate`; E2E-054 to E2E-091 and E2E-SEC-008, E2E-SEC-009; class sweep rows 9 to 13 and searches 4 to 8; BL-0159 written with the owner's validation; one line added to BL-0157. After round 11 the four A findings were applied (design inputs in 14.3, E2E-SEC-009, spelled E2E-060 entry, citation) and the consistency check passed on the final text.
**Rounds beyond the three round cap:** the cap was reached at round 3 and escalated to the owner, who decided the proxy and device flow points; rounds 4 to 11 followed that scope decision. The F findings of rounds 4 to 10 were precision gaps in entry and request reading, redirects, save mode and boot commands, each answered by an amendment.
**No implementation detail outside Section 14:** PASS (consistency check, final run)

## 14. Evidence appendix (for /sdlc-design and the auditor, not for the product owner)

### 14.1 Current State evidence

| # | Claim (Section 2) | Evidence |
|---|---|---|
| 1 | `web.fetch` is a registered tool, with text mode and save mode | `crates/core/src/tools/web.rs:78`, `:105-133` |
| 2 | `web.download` is a registered tool | `crates/core/src/tools/web.rs:140` |
| 3 | The only refusal on the address is the prefix check, in `web.download` and in the text mode of `web.fetch`; the save mode of `web.fetch` has none | `crates/core/src/tools/web.rs:156` (download), `:597` (fetch text), `:105-133` (fetch save, no check) |
| 4 | One function sends every member supplied request, called from three places, with no address check | `crates/core/src/tools/web.rs:449-455` (`fetch_raw`), callers `:120`, `:164`, `:602` |
| 5 | The client sets a timeout and a user agent only, so the default redirect policy applies (up to 10 hops, any host) | `crates/core/src/tools/web.rs:260-266` (`build_client`) |
| 6 | The web family has one switch, off by default, plus results, timeout and safe search settings | `crates/core/src/config.rs:623-632`, `:647-658` |
| 7 | Size limits exist (16 MiB), unchanged here | `crates/core/src/tools/web.rs:443, :555-600`, `.agent_docs/config.md:285-306` |
| 8 | Search, news and suggestions use fixed constants; only the query comes from the member | `crates/core/src/tools/web.rs:331`, `:408`, `:615` (`build_client(10)` calls) |
| 9 | SPEC-0093 excluded address filtering and fixed the order of refusals | `.agent_docs/config.md:299-306` ("address filtering (CWE-918) is out of scope" at line 306) |
| 10 | Tests call real tool requests against stand ins bound on the loopback address | `crates/core/src/tools/web_testkit.rs:79-81`; `web_bound_e2e.rs` (51 tests), `tool_surface_e2e.rs` (16 tests), counted with `rg -c '#\[(tokio::)?test'` on 2026-11-16 |
| 11 | `ERR_FORBIDDEN` exists, answers 403 and is used for membership | `crates/core/src/errors.rs:10`, `:59-60`, `:124` |

Test command: `cargo test -p mcp-fs-core --lib -- web_bound_e2e tool_surface_e2e` (`--list` shows 67 tests for these two filters on 2026-11-16; the full gate is `cargo test --workspace`).

### 14.2 Class sweep (`--security`)

**Class:** a web address or content supplied by a project member that makes the server, or a program it launches, open a network connection (server side request forgery, CWE-918).

**Searches run (read only subagent, 2026-11-16):**
1. `rg -n 'reqwest::|Client::builder|ClientBuilder|\.send\(\)|ureq|hyper::|"curl"|redirect' crates scripts config --glob '!**/tests/**'` plus `rg -n 'http\.(get|post)|\.post\(|\.get\(|\.head\(' crates/core/src`: covers HTTP client idioms.
2. `rg -n 'git2|RemoteCallbacks|Remote::|ls_remote|"clone"|"fetch"|git\.hosts|hosts' crates` plus `rg -n 'validate_remote_url|remote_anonymous|connect_auth|resolve_host' crates/core/src`: covers git network idioms.
3. `rg -n 'TcpStream|Url::parse|httpfs|read_parquet|endpoint|Command::new' crates` plus the same under `crates/core/src` without tests: covers sockets, converters, remote file reads.
4. `rg -n 'Client::new\(\)|reqwest::get' crates --glob '!**/tests/**'`: covers shared and one shot clients (derived by the auditor).
5. `rg -n 'src=|href=|!\[' crates/core/src/docs crates/core/src/tools/doc.rs`: covers member content carrying links to a converter (derived by the auditor); no production code follows a link, the only reader is the external converter.
6. `rg -n 'instance_url' crates/core/src` (derived by the auditor): covers member supplied base URLs of the OAuth device flow.
7. Direct check by me, then the auditor's `db.*` probe: `rg -n 'ctx\.sql|\.sql\(|SQLOptions|SessionContext|LOCATION' crates/core/src/tools/db.rs` and a read of the DataFusion registry at the pinned revision.
8. Direct check by me: `pandoc` run on `![x](http://127.0.0.1:9/a.png)` (version 3.14) printed `Could not fetch resource http://127.0.0.1:9/a.png`.

Idioms not searched: what the operator's external document converter (`doc_service` command or endpoint) does with the links inside a member's file, since that program is outside this repository.

| # | Occurrence | Verdict | Disposition |
|---|---|---|---|
| 1 | `crates/core/src/tools/web.rs:449-455` (`fetch_raw`, via `web.fetch` and `web.download`) | vulnerable: prefix check only (`:156`, `:597`) | FR-NEW-001 to FR-NEW-012 (the originating occurrence, in this lot) |
| 2 | `crates/core/src/tools/doc.rs:289-293` (pandoc converts a member's Markdown; remote images are fetched) | vulnerable: no sandbox flag, reproduced with pandoc 3.14 | deferred by owner decision of 2026-11-16: `sdlc/backlog/BL-0159-pandoc-fetches-remote-images-of-a-members-markdown.md` (`Nature: BUG`, `Evidence` with the reproduction and `doc.rs:289-293`, `Decision:` naming the owner). Stays exploitable where the document tools are enabled |
| 3 | `crates/core/src/git/remote.rs:246-270` (`validate_remote_url`: https only, no credentials), `:182-187` (`resolve_host`: host declared in the map), `tools/git.rs:3170-3177`, `:4415-4417` | safe: only https and hosts declared in the host map are accepted, the map has no wildcard and no tool can change it. Residual: a declared host that resolves to a private address is not blocked, and libgit2's own redirects are not constrained by this code (operator chooses the declared hosts) | out of scope, reported |
| 4 | `crates/core/src/git/provider/mod.rs:111-119`, `:391`, `:519-547` | safe: host must be declared; redirects `Policy::none()` with a manual same host loop capped at `MAX_REDIRECTS` (`:47`) | out of scope |
| 5 | `crates/core/src/tools/context7.rs:87`, `:102`, `:144` | safe: address from operator setting; the member supplies a query only | out of scope |
| 6 | `crates/core/src/docs/service.rs:532-560`, `:394` | safe: address and command from operator setting; no member URL | out of scope |
| 7 | `crates/core/src/docs/ocr.rs:97`, `search/embedding.rs:24`, `search/rerank.rs:38`, `storage/blob/s3.rs:41-44` | safe: operator settings | out of scope |
| 13 | `crates/core/src/tools/db.rs:106-109` (`session()`), with the member's SQL run at `:331-334`, `:371-375`, `:424-437`, `:532-536`, `:584-588` | safe for a network connection: `session()` builds a plain query context and registers no remote store; the pinned DataFusion revision `6f1436f` (`Cargo.toml:137`) registers only the local file store in a new context (`datafusion/execution/src/object_store.rs:208-212`) and answers any other scheme `No suitable object store found for <url>` (`:255-262`), so `CREATE EXTERNAL TABLE ... LOCATION 'http://...'` cannot connect. Residual, outside this class and not fixed here: member SQL may name a local file location of the server (`file://` or a plain path), ASSUMED, not run | out of scope, reported (a different class: local file disclosure) |
| 12 | `crates/core/src/git/oauth/device_flow.rs:193-250` (`gitlab_base`, `post_form`), reached from `tools/git_auth.rs:354-355` with the member's `instance_url` (`git_auth.rs:90`) | safe: `resolve_auth_host` (`git_auth.rs:293-340`) requires the hostname of `instance_url` to be declared in `git.hosts` and mapped to the same provider, and the host map is operator owned. Residual: the port and path of `instance_url` are taken as typed, and a declared host that resolves to a private address is not blocked (same residual as row 3) | out of scope, reported (owner, 2026-11-16) |
| 9 | `crates/core/src/app.rs:111`, `cli.rs:210` (`reqwest::Client::new()`) | safe: shared client for the search backend, whose endpoints are operator settings (row 7); no member address reaches it | out of scope |
| 10 | `crates/core/src/tools/editor.rs:812` (`reqwest::get`) | safe: inside test code (`#[cfg(test)]` at `:20`) | out of scope |
| 11 | `crates/core/src/docs/service.rs:385-396` (the operator's converter command receives the member's file) | residual: the converter program is the operator's and nothing here blocks its network access; the server only passes a relative file name | out of scope, reported |
| 8 | `crates/core/src/tools/web.rs:331`, `:408`, `:615` | safe: fixed search site constants | FR-NEW-011 |

### 14.3 Design inputs

- Setting names fixed by the owner: `web.black_list_url` and `web.white_list_url`, both lists of strings, default empty. Both go in the web section of the configuration, with the existing `web.*` settings (`crates/core/src/config.rs:623-632`).
- `WebConfig` has a hand written debug text and two frozen snapshots of the whole configuration: `crates/core/src/config.rs:1710-1711`. Adding fields means either the snapshots change (an existing test modified, to be justified) or the new fields stay out of the debug text, as `test_base` does (field `config.rs:631`, hand written `Debug` `config.rs:636-645`). Signal: the snapshot tests fail on the first build.
- The single choke point is `fetch_raw` (`web.rs:449`) with `build_client` (`:260`). Redirects use the default client policy today; every hop needs the check, so a redirect policy that runs the decision is needed (the git provider already does a manual redirect loop, `git/provider/mod.rs:519-547`, a precedent).
- "The address checked is the address the request is sent to" (FR-NEW-006) means resolution and connection must agree. A name resolved once for the check and again for the connection allows a rebinding bypass. Signal: no test can catch a time of check gap with a static name table; it needs a design level guarantee.
- The refusal precedes the storage allowance check (`write_limit`, `web.rs:496`) and follows membership and path; SPEC-0093 fixed the order argument, membership and path, request and read, write checks. This increment adds the lists decision between path and allowance.
- Entry parsing and validation belong with the other boot checks (`config.rs:990` `validate`, `:1136` `validate_doc_service` as a pattern). A refusal at boot must not leak into the `Dsn`/secret redaction; entries are not secrets.
- The test sites bind the loopback address (`web_testkit.rs:81`), which the empty default keeps working; tests of non empty lists on loopback need their own entries (for example `127.0.0.0/8` in the white list) and a name resolution seam for `friendly.example`, `mixed.example`, `ghost.example`, `evil.example` (test only).
- Documentation to update: `README.md`, `AGENTS.md` web section, `.agent_docs/config.md` section "web and context7", `TOOL_CONTRACT.txt` unchanged (no schema change; tool descriptions are frozen, `tool-contract-golden.json` must stay identical).
- The redirect policy of the HTTP client is synchronous and cannot resolve names, and the client built at `web.rs:260-266` uses the default policy. Design must either set no automatic redirects and follow hops by hand (precedent `git/provider/mod.rs:519-547`) or install a name resolver that filters. Signal for the time of check gap (FR-NEW-006, "the address checked is the address sent to"): a test whose name table answers a public address on the first lookup and `127.0.0.1` on the second, which needs the test only resolution seam above.
- Tests that type a literal non loopback address (`169.254.169.254`, `10.0.0.6`, `10.0.1.0`, `fd12::1`) need a test only connect seam that maps an (address, port 80) pair to a stand in on `127.0.0.1:<port>` after the lists decision, because the stand ins bind only the loopback address (`web_testkit.rs:79-81`). Signal: E2E-015, E2E-016 and E2E-SEC-001 cannot count requests without it.
- E2E-018, E2E-028 and E2E-034 use a stand in as the product's search site (`search.example`), reached through the test only base address (`config.rs:631`). Signal: those tests fail by returning a refusal instead of the stand in's page if the lists were applied to search.
- E2E-SEC-009 needs a name table that answers a public address on the first lookup and the loopback address on every later one, to prove one resolution is used for the check and the connection; a design that resolves twice connects to the stand in and fails the test.
- Proxy (owner decision of 2026-11-16, option A): `build_client` (`web.rs:260-266`) sets no proxy option, so whether the standard proxy environment variables apply is the HTTP client's default and is not stated in this repository (verified: reqwest 0.12.28 sets `auto_sys_proxy` true by default, `reqwest-0.12.28/src/async_impl/client.rs:309` and `:419`, and `web.rs:260-266` never calls `no_proxy()`, so the environment proxy is honoured). Design documents the limit of FR-NEW-006 and BL-0157 owns the proxy design. Signal: E2E-068 to E2E-070 need a stand in proxy.
- E2E-080 needs the URL parser used by the HTTP client to reject `http://` (an empty host); `http:///x` is not hostless, it parses with the host `x`. Signal: the test fails if a design reads `http://` as a site.
- Boot refusals go through the existing `error: <message>` rendering (`crates/core/src/cli.rs:134`). Signal: the E2E-042 assertion fails on an exact match if the prefix differs.
- Entry validation runs at boot for both lists whether or not `web.enabled` is true (FR-NEW-012); the existing `validate_doc_service` returns early when its switch is off (`config.rs:1137-1139`), so the web check must not copy that shape. Boot refusals are `ToolError::invalid_argument` values (`config.rs:1143` as precedent), which render with the `ERR_INVALID_ARGUMENT:` prefix.
- Redirects: keep today's limit and failure messages (FR-NEW-007); a manual hop loop must reproduce them, including the over limit, non web address and missing address cases. Signal: E2E-064 and a test per case.
- Entry and request normalization (FR-NEW-005, FR-NEW-006, FR-NEW-008) must go through one URL parser, so a numeric or mapped form and an accented name are reduced the same way for the entry and the request. Signal: E2E-SEC-004, E2E-SEC-005, E2E-057.
