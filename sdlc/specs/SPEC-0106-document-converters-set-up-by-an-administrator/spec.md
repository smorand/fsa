# Document converters set up by an administrator: functional specification

> Id: SPEC-0106
> Nature: FEAT
> Depth: L
> Depth evidence: FEAT / depth L: 5 areas (converter registry, project and platform selection, conversion, admin tools and web interface, admin screen), 23 requirements, 2 new persisted concepts (converters and project selection). Override with --depth.
> Status: implemented
> Generated on: 2026-11-24
> Project: existing
> From backlog: BL-0165
> Depends on: none
> Security: internal finding
> CVSS: not scored
> Affected: n/a
> Fixed in: n/a

## 1. Summary
Today the document converter of the platform is a block of the configuration file, read once at start, shared by every project and changeable only by editing the file and restarting. This increment lets a platform administrator define named converters (a program to run or an http service, with its secret kept encrypted), choose a platform default and choose a converter for each project, all through the admin tools, the web interface and a screen, with no restart. Setting a converter lets an administrator run a program on the server, so only platform administrators can do it, every change is recorded, arguments are never given to a shell and the secret is never shown.

## 2. Current State (MANDATORY, every depth)
The platform has one document converter, defined by the configuration file: a program with its arguments or an http service with a header and a token read from the environment. It is checked when the platform starts and built once, so a change needs a restart and applies to every project. There is no choice per project and no administrator interface for it. A write, an upload or a conversion of an existing file can ask for conversion; each uses that one converter, and refuses when none is configured or when the extension is not one it accepts. The converter's answer is capped, runs for a bounded time and, for a program, runs in a temporary folder of its own with an argument list and no shell. A recent increment gave administrators a registry of model providers with secrets stored encrypted, a record of changes, admin tools, web routes and a screen; converters are not part of it.

### 2.1 What users can do today
- Convert documents with the single converter of the configuration file (14.1 rows 1 to 5).
- Nothing lets an administrator add, change or choose a converter at run time (14.1 row 6).

### 2.2 Specs governing this area
- SPEC-0006 (documents): FR-007 to FR-016 (optional service, two forms, extensions, size gate, per call sandbox, failure reporting). FR-012 is modified here (FR-MOD-001). That spec is not edited.
- SPEC-0089 (bounds): the answer limit and what a conversion holds; kept for every converter.
- SPEC-0103 (document metadata): the converter answer contract; kept.
- SPEC-0102 (settings registry): the pattern reused (administrators only, encrypted write only secrets, record of changes, web routes, one time tokens); the record of changes is extended with new target kinds, its existing behavior is unchanged.

### 2.3 What is verified today
The configuration file converter, its limits and its failure reports have end to end tests; nothing tests a registry of converters or a choice per project (14.1 row 7).

## 3. Scope
### 3.1 In scope
- Named converters of two forms, a platform default, a converter per project.
- The tools, the web interface and the screen for them; the record of changes.
- Resolution at each conversion; the secret rules; the redaction of the secret in a failure text.

### 3.2 Out of scope, and why
- Testing a converter from the screen: a later convenience.
- Environment variables per converter: a program that needs one wraps itself; keeps the argument list simple.
- Moving the remaining configuration file settings (BL-0173).
- Converter quotas or per project limits: the answer cap and timeout are per converter.
- A project owner choosing a converter: a converter is a program or an address that receives project files, so only platform administrators choose.

### 3.3 Constraints imposed by the context
- Only platform administrators can set a converter (the owner's rule for a feature that runs programs).
- The master key stays the existing one; no new environment variable (the owner wants the database address and the master key only).
- Tool and address names above are public contract read by agents, fixed here because consumers type them.

## 4. Actors
- **Platform administrator**: defines converters, the default and project selections.
- **Project member**: converts documents, sees the result of the choice.
- **Operator**: provides the master key and the configuration file converter.
- **Non administrator**: refused.

## 4.5 Bounded contexts
| Context | What it owns | Key concepts |
|---|---|---|
| Documents | the conversion of a file and its metadata | converter, conversion, companion text |
| Settings | what an administrator configures | converter registry, default, project selection, change record, secret |

## 5. Usage scenarios
### SC-001: Create and read converters
**Actor:** a platform administrator
**Preconditions:** no converter, or some converters
**Flow:**
1. Alice creates a program converter and an http converter → each is answered with its fields and no secret.
2. She lists them → by name, 100 per answer.
**Postconditions:** converters stored
**Exceptions:**
- EXC-001a: a name, form or field breaks a rule → refused with its text.

### SC-002: Change and delete converters
**Actor:** a platform administrator
**Preconditions:** converters exist
**Flow:**
1. Alice updates one → same id, new values.
2. She deletes an unused one → gone.
**Postconditions:** registry updated
**Exceptions:**
- EXC-002a: unknown id, form change, a default or used converter cannot be deleted → refused.

### SC-003: Choose the platform default and the converter of a project
**Actor:** a platform administrator
**Preconditions:** converters and projects exist
**Flow:**
1. Alice sets the default → read back.
2. She sets a project's converter → read back.
3. She clears either.
**Postconditions:** selection stored
**Exceptions:**
- EXC-003a: unknown converter or project → refused.

### SC-004: A conversion uses the right converter
**Actor:** a project member
**Preconditions:** documents to convert
**Flow:**
1. Bob writes a document with the conversion option → the project's converter, else the default, else the configuration file converter converts it.
2. A change by Alice applies to his next conversion.
**Postconditions:** document converted by the resolved converter
**Exceptions:**
- EXC-004a: extension not accepted, no converter, converter failure, cap exceeded → refused or reported, no fallback.

### SC-005: Input rules
**Actor:** a platform administrator
**Preconditions:** a creation or update
**Flow:**
1. Alice sends values at and beyond each limit → accepted or refused with the exact text.
**Postconditions:** only valid converters stored
**Exceptions:**
- EXC-005a: every rule of FR-NEW-001 to FR-NEW-004.

### SC-006: Access and the web interface
**Actor:** a person
**Preconditions:** converters exist
**Flow:**
1. A non administrator tries any operation → refused.
2. An administrator uses the web interface → same answers as the tools.
**Postconditions:** no change by a non administrator
**Exceptions:**
- EXC-006a: refusal text equal on every interface.

### SC-007: The converters screen
**Actor:** a platform administrator
**Preconditions:** a browser session
**Flow:**
1. Alice opens the screen and submits forms with their one time token → the same effect as the tools.
**Postconditions:** screen and tools agree
**Exceptions:**
- EXC-007a: no token, wrong token, non administrator → refused.

### SC-008: Secrets and arguments stay safe
**Actor:** a platform administrator
**Preconditions:** a converter with a secret or a hostile looking argument
**Flow:**
1. Alice stores a secret and a command with shell characters → the secret is never shown, each argument stays one argument.
**Postconditions:** no secret shown, no shell
**Exceptions:**
- EXC-008a: a converter that echoes the secret → `[redacted]`.

### SC-009: The record of changes
**Actor:** a platform administrator
**Preconditions:** changes made
**Flow:**
1. Alice lists the changes → each with who, when, what, field names.
**Postconditions:** every change recorded without values
**Exceptions:**
- EXC-009a: refused or empty changes are not recorded.

### SC-010: An existing platform upgrades
**Actor:** an operator
**Preconditions:** a configuration file converter and an empty registry
**Flow:**
1. The platform starts and converts → as before.
**Postconditions:** nothing changed
**Exceptions:**
- EXC-010a: an invalid configuration file converter stops the start as before.

## 6. Functional requirements
### 6.1 New
#### FR-NEW-001 [EARS-E]: Create a converter
> WHEN a platform administrator creates a converter with a name and a form THE platform SHALL store it and answer with the converter.

- **Business rules:** The tool is `admin.create_converter` with `name`, `form` (`program` or `http`), `command`, `url`, `auth_header` (default `Authorization`), `secret` (default empty), `file_field` (default `file`), `response_field` (default empty), `extensions` (default empty list), `max_input_bytes` (default 536870912) and `timeout_seconds` (default 900). The answer is `{converter_id, name, form, command, url, auth_header, has_secret, file_field, response_field, extensions, max_input_bytes, timeout_seconds, created_at, updated_at}` with `null` for a field that does not belong to the form; `converter_id` starts with `conv-`; times are UTC, RFC 3339 with seconds. The name is trimmed, 1 to 64 characters, unique without regard to letter case. A field is "given" when it is present with a value other than `null`; for tools and the web interface `null` equals omitted, and an empty text or empty list is a value. On the screen a blank text field is not given. Order of checks: name length, name taken, form, the rules of FR-NEW-002 or FR-NEW-003, FR-NEW-004, then the secret rules.
- **User visible text:** `ERR_INVALID_ARGUMENT: name must be 1 to 64 characters`; `ERR_INVALID_ARGUMENT: a converter named '<name>' already exists`; `ERR_INVALID_ARGUMENT: form must be 'program' or 'http'`
- **Scenario:** SC-001
- **Priority:** Must

#### FR-NEW-002 [EARS-O]: The program form
> IF a converter has the form `program` THEN THE platform SHALL accept it only with a command that is a list of text arguments holding the placeholder `{document}` exactly once and the placeholder `{outdir}` at most once, and SHALL accept none of the fields of the http form.

- **Business rules:** At least one argument must be non blank. Order: the command is a list of text arguments, it is not empty, `{document}` count, `{outdir}` count, then a field of the http form (`url`, `auth_header`, `secret`, `file_field`, `response_field`) given (an empty text counts as given). The program is not looked up on the server when the converter is saved: a program that does not exist shows as a failed conversion.
- **User visible text:** `ERR_INVALID_ARGUMENT: command must be a list of text arguments`; `ERR_INVALID_ARGUMENT: command is required for a program converter`; `ERR_INVALID_ARGUMENT: command must contain exactly one '{document}' placeholder across its arguments, found <n>`; `ERR_INVALID_ARGUMENT: command may contain at most one '{outdir}' placeholder across its arguments, found <n>`; `ERR_INVALID_ARGUMENT: <field> is not valid for a program converter`
- **Scenario:** SC-005
- **Priority:** Must

#### FR-NEW-003 [EARS-O]: The http form and its secret
> IF a converter has the form `http` THEN THE platform SHALL accept it only with an http or https address that carries no user name or password and a non empty file field, SHALL accept no command, AND SHALL keep a secret given with it encrypted and write only.

- **Business rules:** The address must carry a host and be at most 2048 characters, otherwise it is refused with the scheme text, which never quotes it. The secret is the text sent as the value of the header named `auth_header` (so `Bearer abc` works); it is stored trimmed, at most 4096 characters after trimming. An empty or blank `url` is refused with the `is required` text. A blank `auth_header` means the default `Authorization` when no secret is set (given or stored) and is refused with `auth_header is required when a secret is set` when one is; a header name is checked only when it is not blank. A blank secret (empty or only spaces) means no secret: on creation none is stored, on update the stored one is kept. `auth_header` must be a valid http header name and the secret a valid header value (no line break); neither text quotes the value. A converter can have no secret. No http converter follows a redirect (FR-MOD-002). Order: url required, url scheme, host and length, url user name, command given, `auth_header` with a secret, `file_field`, header name, header value, secret length, master key. Without a master key a secret is refused and nothing is created.
- **User visible text:** `ERR_INVALID_ARGUMENT: url is required for an http converter`; `ERR_INVALID_ARGUMENT: url must be an http or https URL`; `ERR_INVALID_ARGUMENT: url must not carry a user name or password`; `ERR_INVALID_ARGUMENT: command is not valid for an http converter`; `ERR_INVALID_ARGUMENT: auth_header is required when a secret is set`; `ERR_INVALID_ARGUMENT: file_field must not be empty`; `ERR_INVALID_ARGUMENT: auth_header is not a valid header name`; `ERR_INVALID_ARGUMENT: secret is not a valid header value`; `ERR_INVALID_ARGUMENT: secret must be at most 4096 characters`; `ERR_NOT_SUPPORTED: no master key is configured; set FSA_TOKEN_KEY`
- **Scenario:** SC-001
- **Priority:** Must

#### FR-NEW-004 [EARS-U]: Limits common to both forms
> The platform SHALL hold for every converter a list of accepted extensions, an answer cap in bytes and a timeout in seconds.

- **Business rules:** `extensions` is a list of up to 64 names of 1 to 16 letters or digits, without a dot, stored in lower case, duplicates removed keeping the first, in the given order; an empty list means the built in list of document extensions, as for the configuration file. `max_input_bytes` is a whole number from 1 to 4294967296 and caps both the size of the document sent and the size of the converter's answer, as for the configuration file converter: a document over it is refused before any write with `ERR_INVALID_ARGUMENT: '<path>' is <n> bytes, over the doc_service.max_input_bytes limit of <cap>`. `timeout_seconds` is a whole number from 1 to 86400. A number that is not whole (a fraction, a text, a boolean) is refused with the range text of its field; a text field given as a non-text is refused with `ERR_INVALID_ARGUMENT: <field> must be text`; `clear_secret` given as a non-boolean is refused with `ERR_INVALID_ARGUMENT: clear_secret must be true or false`.
- **User visible text:** `ERR_INVALID_ARGUMENT: extensions must be a list of up to 64 names of 1 to 16 letters or digits`; `ERR_INVALID_ARGUMENT: max_input_bytes must be between 1 and 4294967296`; `ERR_INVALID_ARGUMENT: timeout_seconds must be between 1 and 86400`; `ERR_INVALID_ARGUMENT: <field> must be text`; `ERR_INVALID_ARGUMENT: clear_secret must be true or false`; `ERR_INVALID_ARGUMENT: '<path>' is <n> bytes, over the doc_service.max_input_bytes limit of <cap>`
- **Scenario:** SC-005
- **Priority:** Must

#### FR-NEW-005 [EARS-E]: Update a converter
> WHEN a platform administrator updates a converter THE platform SHALL apply the given fields and answer with the converter, keeping its id and its form.

- **Business rules:** The tool is `admin.update_converter` with `converter_id` and any creation field except `form`; `clear_secret` (default false) removes the secret. An omitted `secret` keeps the stored one. Order: converter exists, `form` given, `secret` with `clear_secret`, then the checks of creation applied to the converter as it is after the update. A blank `secret` keeps the stored one. A field of the other form with the value `null` is not given. An answer can be sent back as an update: `form` with the stored value is accepted and ignored, as are `has_secret`, `created_at`, `updated_at` and a `converter_id` equal to the target; a different `converter_id` is refused with `ERR_INVALID_ARGUMENT: converter_id in the body does not match`; a different `form` is refused with `form cannot be changed`. The uniqueness of the name ignores the converter being updated, so a rename that changes only the letter case is accepted. An update that changes nothing leaves `updated_at` unchanged. An update that gives no new secret, and `clear_secret`, never need the master key. `clear_secret` on a converter without a secret changes nothing; a blank `secret` does not count as given for the rule against giving both; `clear_secret` on a program converter is refused with `ERR_INVALID_ARGUMENT: clear_secret is not valid for a program converter`. A converter named by the platform default or by a project stays so after a rename.
- **User visible text:** `ERR_NOT_FOUND: converter not found: <converter_id>`; `ERR_INVALID_ARGUMENT: form cannot be changed`; `ERR_INVALID_ARGUMENT: secret and clear_secret cannot be given together`; `ERR_INVALID_ARGUMENT: converter_id in the body does not match`; `ERR_INVALID_ARGUMENT: clear_secret is not valid for a program converter`
- **Scenario:** SC-002
- **Priority:** Must

#### FR-NEW-006 [EARS-E]: List converters
> WHEN a platform administrator lists the converters THE platform SHALL return them ordered by name without regard to letter case, 100 per answer, with a marker for the next answer.

- **Business rules:** The tool is `admin.list_converters` with `marker`; the answer is `{converters: [...], marker}`; the marker is `null` on the last answer. Each converter is shown as in FR-NEW-001; the secret is never shown.
- **User visible text:** `ERR_INVALID_ARGUMENT: marker is not valid`
- **Scenario:** SC-001
- **Priority:** Must

#### FR-NEW-007 [EARS-E]: Delete a converter
> WHEN a platform administrator deletes a converter THE platform SHALL remove it unless it is the platform default or the converter of a project.

- **Business rules:** The tool is `admin.delete_converter`; it answers `{converter_id}`. The stored secret is removed with it. Order: the refusal of FR-NEW-017, converter not found, platform default, used by projects. A project removed within its grace period counts as using its converter; a project removed for good does not.
- **User visible text:** `ERR_NOT_FOUND: converter not found: <converter_id>`; `ERR_INVALID_ARGUMENT: converter '<name>' is the platform default`; `ERR_INVALID_ARGUMENT: converter '<name>' is used by <n> project(s)`
- **Scenario:** SC-002
- **Priority:** Must

#### FR-NEW-008 [EARS-E]: The platform default converter
> WHEN a platform administrator sets, clears or reads the platform default converter THE platform SHALL keep one default at most and answer with it.

- **Business rules:** Tools: `admin.set_default_converter` (`converter_id`), `admin.clear_default_converter`, `admin.get_default_converter`; all three answer `{converter: <converter>|null}`; a clear answers `null`. Setting the converter that is already the default changes nothing, and a clear when nothing is set changes nothing.
- **User visible text:** `ERR_NOT_FOUND: converter not found: <converter_id>`
- **Scenario:** SC-003
- **Priority:** Must

#### FR-NEW-009 [EARS-E]: The converter of a project
> WHEN a platform administrator sets, clears or reads the converter of a project THE platform SHALL keep at most one converter for that project and answer with it.

- **Business rules:** Tools: `admin.set_project_converter` (`project_id`, `converter_id`), `admin.clear_project_converter` (`project_id`), `admin.get_project_converter` (`project_id`); the three answer `{project_id, converter}` with `converter` null when none, and a clear answers null. A project in its grace period is found; only a project removed for good gives project not found. Order: the refusal of FR-NEW-017, project not found, converter not found. Platform administrators have no access to the project's files through this.
- **User visible text:** `ERR_PROJECT_NOT_FOUND: project '<id>' not found`; `ERR_NOT_FOUND: converter not found: <converter_id>`
- **Scenario:** SC-003
- **Priority:** Must

#### FR-NEW-010 [EARS-S]: Which converter a conversion uses
> WHILE a document is converted THE platform SHALL use the converter of the project, else the platform default converter, else the converter of the configuration file, else refuse.

- **Business rules:** This holds for every way a conversion starts: a write with the conversion option, an upload with the conversion option and the conversion of an existing file; a multi file upload first checks the whole batch for a converter and for the extension, as today, against the converter resolved for each file at that moment, and one refusal fails the request with nothing written; each file is then converted with the converter resolved again when its own conversion starts, and if that resolution finds no converter or selects one that refuses the file (extension, size, unreadable secret), the upload stops with that refusal, files already written stay and later files are not processed, as for any refusal during a batch today. The converter is resolved before anything is written. Order of refusals on every conversion path: no converter, extension, size, then the unreadable secret. A refusal leaves nothing written; a file is written without a companion only when its conversion fails after the converter accepted it (FR-NEW-013). When the configuration file converter is disabled it is skipped. When the converter chosen accepts the file and has a secret that cannot be read (no master key, another key, a damaged value) the conversion is refused before any write with `ERR_INTERNAL_ERROR: the secret of converter '<name>' cannot be read`, no other converter is tried and the text holds no secret. The refusal with none is the existing one.
- **User visible text:** `ERR_NOT_SUPPORTED: document service is not configured`; `ERR_INTERNAL_ERROR: the secret of converter '<name>' cannot be read`
- **Scenario:** SC-004
- **Priority:** Must

#### FR-NEW-011 [EARS-S]: Changes apply to the next conversion
> WHILE a conversion is running THE platform SHALL finish it with the converter it started with, AND a later conversion SHALL use what the registry holds at its start, with no restart.

- **Business rules:** Deleting a converter that a running conversion uses is allowed once it is unused by the platform default and the projects; the running conversion still completes.
- **Scenario:** SC-004
- **Priority:** Must

#### FR-NEW-012 [EARS-O]: Extensions and no fallback
> IF the extension of the document is not accepted by the converter chosen under FR-NEW-010 THEN THE platform SHALL refuse the conversion and SHALL NOT try another converter.

- **Business rules:** A path is accepted when its name ends with a dot followed by one of the extensions, without regard to letter case. The refusal is the existing one; it lists each accepted extension with a leading dot, joined by `, `, in the stored order, and with an empty list the built in extensions in their built in order.
- **User visible text:** `ERR_NOT_SUPPORTED: '<path>' cannot be documented, the document service accepts: <extensions>`
- **Scenario:** SC-004
- **Priority:** Must

#### FR-NEW-013 [EARS-U]: One set of conversion rules
> The platform SHALL apply to a converter of the registry the rules it applies to the converter of the configuration file: the answer cap, the sandbox of a call, the failure report, the document metadata of the converter answer, the order of refusals and the write that stays when a conversion fails.

- **Business rules:** Nothing about a conversion changes except which converter runs. The converter is resolved before anything is written.
- **Scenario:** SC-004
- **Priority:** Must

#### FR-NEW-014 [EARS-UB]: Arguments are never a shell
> The platform SHALL NOT pass a converter command to a shell: each argument is given to the program as one argument whatever characters it holds.

- **Business rules:** Only `{document}` and `{outdir}` are replaced, inside one argument; the program runs in its own temporary folder, as for the configuration file converter.
- **Scenario:** SC-008
- **Priority:** Must

#### FR-NEW-015 [EARS-UB]: Secrets are never revealed
> The platform SHALL NOT reveal the secret of a converter in any answer, listing, record of changes, screen, log line or failure text.

- **Business rules:** A failure text that quotes the converter's own answer has every occurrence of the secret replaced by `[redacted]`. The secret is stored encrypted with the master key; without the key the converter cannot be created with a secret. Answers carry only `has_secret`.
- **User visible text:** `[redacted]`
- **Scenario:** SC-008
- **Priority:** Must

#### FR-NEW-016 [EARS-U]: Every change is recorded
> The platform SHALL record every creation, update and deletion of a converter and every change of the platform default and of the converter of a project, with who did it, when, what it concerned and which fields changed, AND SHALL NOT record any value.

- **Business rules:** The records join those of the model registry and are read with `admin.list_setting_changes`. `action` is one of `create_converter`, `update_converter`, `delete_converter`, `set_default_converter`, `clear_default_converter`, `set_project_converter`, `clear_project_converter`; `target_kind` is `converter`, `default_converter` or `project_converter`; `fields` lists the names of the changed fields in the order name, command, url, auth_header, secret, file_field, response_field, extensions, max_input_bytes, timeout_seconds (`secret` for the secret, never its value), empty for a creation, a deletion, a default and a project record. `target_id` and `target_name` are the converter id and name for a converter record (the name after the update for an update); for a default record the id and name of the converter that became or ceased to be the default; for a project record the project id and the name of the converter. A clear when nothing is set is not recorded. A refused operation, an update that changes nothing and a repeated set of the same value are not recorded.
- **Scenario:** SC-009
- **Priority:** Must

#### FR-NEW-017 [EARS-U]: Platform administrators only
> The platform SHALL allow every converter operation, reading included, to platform administrators only.

- **Business rules:** A person who is not a platform administrator is refused with the text below on every interface, nothing is changed or revealed, and a project owner has no right to choose a converter. The refusal comes first, before body shape and before any existence check, and names no converter or project; on the screen the order is identity, administrator, one time token, then the operation checks.
- **User visible text:** `ERR_FORBIDDEN: '<person>' is not a platform admin`
- **Scenario:** SC-006
- **Priority:** Must

#### FR-NEW-018 [EARS-U]: The web interface offers the same operations
> The platform SHALL offer every converter operation through its web interface with the same inputs, answers, refusals and access rule as the tools.

- **Business rules:** Public addresses: `POST /api/admin/converters`, `GET /api/admin/converters` (query `marker`), `PATCH /api/admin/converters/{converter_id}`, `DELETE /api/admin/converters/{converter_id}`, `GET /api/admin/converter-default`, `PUT /api/admin/converter-default`, `DELETE /api/admin/converter-default`, `GET /api/admin/projects/{project_id}/converter`, `PUT /api/admin/projects/{project_id}/converter`, `DELETE /api/admin/projects/{project_id}/converter`. Bodies and answers are the tool inputs and answers as JSON; a refusal keeps its error code and text in the existing error envelope.
- **Scenario:** SC-006
- **Priority:** Must

#### FR-NEW-019 [EARS-E]: The converters screen
> WHEN a platform administrator opens the converters screen THE platform SHALL show the converters, the platform default, each project's converter and the latest changes, and SHALL let the administrator create, edit, delete, set the default and set each project's converter, with a one time token on each form.

- **Business rules:** `command` is a text area with one argument per line (blank lines dropped) and `extensions` a text field separated by commas or spaces; an update sent with an empty secret keeps the stored one. Served at `/app/converters`; forms post to `/app/converters/<action>` with the actions `converter/create`, `converter/update`, `converter/delete`, `default/set`, `default/clear`, `project/set`, `project/clear`. The screen shows `Secret stored` or `No secret`, never fills the secret field, keeps the stored secret when the field is left empty on an edit, offers a `Clear the secret` checkbox on the edit form (the same effect as `clear_secret`) and shows the 20 most recent records of converter actions only, newest first. The screen lists every project that has a converter and a field to set one by project id. The posted form fields have the spelling of the tool inputs (`command` and `extensions` as described, `clear_secret` as `clear_secret=on`, `csrf_token`); on an edit form `response_field` and `extensions` are always sent, so left blank they set the empty value, while the other blank text fields keep the stored value. A success redirects to `/app/converters`; a refusal shows the screen again with the refusal text and the status of its error code. A form without a valid one time token issued to the same person is refused with `ERR_FORBIDDEN: missing or invalid csrf_token`; a request carrying the caller's identity in a header needs no token. A refused action shows its text. Non administrators get the refusal of FR-NEW-017.
- **User visible text:** `ERR_FORBIDDEN: missing or invalid csrf_token`
- **Scenario:** SC-007
- **Priority:** Must

#### FR-NEW-020 [EARS-U]: The configuration file converter stays
> The platform SHALL keep reading the converter of the configuration file, checked when the platform starts, and SHALL behave exactly as before when the registry holds no default and no project converter.

- **Business rules:** An existing platform upgrades with an empty registry and nothing changes, except the two modifications listed in FR-MOD-001 and FR-MOD-002.
- **Scenario:** SC-010
- **Priority:** Must

#### FR-NEW-021 [EARS-E]: Removing a project
> WHEN a project is removed for good THE platform SHALL drop its converter selection, AND a project removed and restored within its grace period SHALL keep it.

- **Scenario:** SC-003
- **Priority:** Must

### 6.2 Modified
#### FR-MOD-001 [EARS-O]: A failure text never reveals the secret (modifies SPEC-0006/FR-012)
> IF a conversion fails and the failure text quotes the converter's own answer THEN THE platform SHALL replace every occurrence of the converter's secret in that text by `[redacted]`.

- **Before and after:** Before: the failure text quoted a slice of the converter's answer as received. After: the same, with the secret (trimmed; a blank secret redacts nothing) replaced in the whole answer before it is cut to the quoted length, so a secret straddling the cut leaves no prefix. It applies to failure texts only (the Markdown of a successful answer is kept as the converter gave it), to converters of the registry and to the token of the configuration file converter.
- **Reason:** a converter that echoes or redirects a request header must not turn a failure into a leak.
- **Scenario:** SC-008

#### FR-MOD-002 [EARS-UB]: No http converter follows a redirect (modifies SPEC-0006/FR-008)
> No http converter, whether it comes from the registry or from the configuration file, SHALL follow a redirect.

- **Before and after:** Before: the http form followed redirects, so a custom header holding the secret can reach another host. After: a 3xx answer is a failed conversion reporting the status like any other answer that is not a success.
- **Reason:** a converter that echoes or redirects a request header must not turn a failure into a leak.
- **Scenario:** SC-008

### 6.3 Removed
None.

## 7. Non functional requirements (user observable)
### 7.1 Performance
A change of converter applies to the next conversion at once; listing 100 converters completes within 1 second for the administrator.
### 7.2 Security and access
Platform administrators only (FR-NEW-017); arguments never reach a shell (FR-NEW-014); secrets never revealed (FR-NEW-015). A converter command runs with the rights of the platform: choosing it is as powerful as running a program on the server, which is why only platform administrators can. A converter program inherits the environment of the platform, including the master key variable; an administrator who can set a command can already run any program, so this adds no exposure, and clearing it is left to the owner (proposal P2).
### 7.3 Privacy and compliance
Project files are sent to the converter an administrator chose; the record of changes tells who chose what and when.
### 7.4 Usability and accessibility
The screen shows the refusal text of each refused action and the secret state as `Secret stored` or `No secret`.
### 7.5 Reliability
A change is stored whole or not at all; a conversion in flight is not disturbed by a change (FR-NEW-011).
### 7.6 Operability
An operator can find, for any converter change, who made it, when, and which fields changed.

## 8. Edge cases
| Category | Case | Expected behavior | Test |
|---|---|---|---|
| Empty states | no converter, no default, empty extensions | refusal or built in list | E2E-055, E2E-064, E2E-080 |
| Boundary values | name 64, timeout 1 and 86400, cap 1 and 4294967296, 101 converters | accepted; paged | E2E-061, E2E-062, E2E-063, E2E-073 |
| Concurrent actions | a delete or a change during a conversion | conversion finishes | E2E-060, E2E-012 |
| Slow or lost connectivity | converter timeout | failure reported as before | E2E-056 |
| Access states | non administrator, owner, administrator not a member | refused; allowed without file access | E2E-050, E2E-074 |
| Data variations | accents, emoji, shell characters in names and arguments | stored as written; one argument | E2E-061, E2E-067, E2E-092 |
| Navigation | invalid marker | refused | E2E-049 |
| Partial operations | refused update, copy of secret failure | nothing changes | E2E-042, E2E-038 |
| Dependency failures | converter error, echoed secret | reported, redacted | E2E-056, E2E-059 |
| Ordering and timing | change applies to the next call; restart | applies; kept | E2E-012, E2E-078 |
| Exhaustion | answer over the cap | refused | E2E-057 |

## 9. Acceptance tests (the contract)
> Behavioral and technology agnostic. Convention: Alice (`alice@example.com`) is a platform administrator; Bob (`bob@example.com`) is a member of `proj` and not a platform administrator; `proj` and `other` are projects. "Converts" means Bob writes the file with the conversion option on and reads its extracted text. The stand in converters: **A** (a program) returns the Markdown `# From A`, **B** (an http service) returns `# From B`; `record-args` is a program that records the arguments it receives. A "web interface" call is the web twin of a tool. Names of converters used below: `Pandoc local` (form `program`, command `["pandoc-convert", "{document}"]`) and `Docling API` (form `http`, url `http://localhost:8099/convert`).

### 9.1 Summary
| Test | Action | Category | Scenario | Requirements | Priority |
|---|---|---|---|---|---|
| E2E-001 | New | requirement | SC-001 | FR-NEW-001, FR-NEW-002, FR-NEW-004 | Critical |
| E2E-002 | New | requirement | SC-001 | FR-NEW-001, FR-NEW-003 | Critical |
| E2E-003 | New | requirement | SC-001 | FR-NEW-006 | High |
| E2E-004 | New | requirement | SC-003 | FR-NEW-008 | High |
| E2E-005 | New | requirement | SC-002 | FR-NEW-005, FR-NEW-004 | High |
| E2E-006 | New | requirement | SC-003 | FR-NEW-008 | High |
| E2E-007 | New | requirement | SC-003 | FR-NEW-009 | High |
| E2E-008 | New | requirement | SC-002 | FR-NEW-007 | High |
| E2E-009 | New | requirement | SC-004 | FR-NEW-010 | Critical |
| E2E-010 | New | requirement | SC-004 | FR-NEW-010 | Critical |
| E2E-011 | New | requirement | SC-010 | FR-NEW-010, FR-NEW-020 | High |
| E2E-012 | New | requirement | SC-004 | FR-NEW-011 | High |
| E2E-013 | New | requirement | SC-006 | FR-NEW-018 | High |
| E2E-014 | New | requirement | SC-007 | FR-NEW-019 | High |
| E2E-015 | New | requirement | SC-009 | FR-NEW-016 | High |
| E2E-016 | New | journey | SC-004 | FR-NEW-001, FR-NEW-003, FR-NEW-008, FR-NEW-009, FR-NEW-010, FR-NEW-007 | Critical |
| E2E-017 | New | failure | SC-005 | FR-NEW-001 | High |
| E2E-018 | New | failure | SC-005 | FR-NEW-001 | High |
| E2E-019 | New | failure | SC-001 | FR-NEW-001 | High |
| E2E-020 | New | failure | SC-005 | FR-NEW-001 | High |
| E2E-021 | New | failure | SC-005 | FR-NEW-002 | High |
| E2E-022 | New | failure | SC-005 | FR-NEW-002, FR-NEW-014 | High |
| E2E-023 | New | failure | SC-005 | FR-NEW-002 | High |
| E2E-024 | New | failure | SC-005 | FR-NEW-002 | High |
| E2E-025 | New | failure | SC-005 | FR-NEW-002 | High |
| E2E-026 | New | failure | SC-005 | FR-NEW-002 | High |
| E2E-027 | New | failure | SC-005 | FR-NEW-003 | High |
| E2E-028 | New | failure | SC-005 | FR-NEW-003 | High |
| E2E-029 | New | failure | SC-005 | FR-NEW-003, FR-NEW-015 | High |
| E2E-030 | New | failure | SC-005 | FR-NEW-003 | High |
| E2E-031 | New | failure | SC-005 | FR-NEW-003 | High |
| E2E-032 | New | failure | SC-005 | FR-NEW-003 | High |
| E2E-033 | New | failure | SC-005 | FR-NEW-004 | High |
| E2E-034 | New | failure | SC-005 | FR-NEW-004 | High |
| E2E-035 | New | failure | SC-005 | FR-NEW-004 | High |
| E2E-036 | New | failure | SC-005 | FR-NEW-004 | High |
| E2E-037 | New | failure | SC-005 | FR-NEW-003 | High |
| E2E-038 | New | failure | SC-005 | FR-NEW-003, FR-NEW-015 | High |
| E2E-039 | New | failure | SC-005 | FR-NEW-005, FR-NEW-003 | High |
| E2E-040 | New | failure | SC-005 | FR-NEW-005 | High |
| E2E-041 | New | failure | SC-005 | FR-NEW-005 | High |
| E2E-042 | New | failure | SC-005 | FR-NEW-005, FR-NEW-002 | High |
| E2E-043 | New | failure | SC-002 | FR-NEW-007 | High |
| E2E-044 | New | failure | SC-002 | FR-NEW-007 | High |
| E2E-045 | New | failure | SC-002 | FR-NEW-007 | High |
| E2E-046 | New | failure | SC-003 | FR-NEW-008 | High |
| E2E-047 | New | failure | SC-003 | FR-NEW-009 | High |
| E2E-048 | New | failure | SC-003 | FR-NEW-009 | High |
| E2E-049 | New | failure | SC-001 | FR-NEW-006 | High |
| E2E-050 | New | failure | SC-006 | FR-NEW-017 | Critical |
| E2E-051 | New | failure | SC-006 | FR-NEW-017, FR-NEW-018 | High |
| E2E-052 | New | failure | SC-007 | FR-NEW-017, FR-NEW-019 | High |
| E2E-053 | New | failure | SC-007 | FR-NEW-019 | High |
| E2E-054 | New | failure | SC-004 | FR-NEW-012 | High |
| E2E-055 | New | failure | SC-004 | FR-NEW-012 | High |
| E2E-056 | New | failure | SC-004 | FR-NEW-013 | High |
| E2E-057 | New | failure | SC-004 | FR-NEW-013, FR-NEW-004 | High |
| E2E-058 | New | failure | SC-006 | FR-NEW-018 | Medium |
| E2E-059 | New | failure | SC-008 | FR-NEW-015, FR-MOD-001 | Critical |
| E2E-060 | New | failure | SC-004 | FR-NEW-011, FR-NEW-010 | Medium |
| E2E-061 | New | edge | SC-005 | FR-NEW-001 | Medium |
| E2E-062 | New | edge | SC-005 | FR-NEW-004 | Medium |
| E2E-063 | New | edge | SC-005 | FR-NEW-004 | Medium |
| E2E-064 | New | edge | SC-004 | FR-NEW-012, FR-NEW-004 | Medium |
| E2E-065 | New | edge | SC-004 | FR-NEW-012, FR-NEW-004 | Medium |
| E2E-066 | New | edge | SC-008 | FR-NEW-005, FR-NEW-003 | High |
| E2E-067 | New | edge | SC-008 | FR-NEW-014 | Critical |
| E2E-068 | New | edge | SC-004 | FR-NEW-010 | High |
| E2E-069 | New | edge | SC-003 | FR-NEW-009 | Medium |
| E2E-070 | New | edge | SC-002 | FR-NEW-007, FR-NEW-008 | Medium |
| E2E-071 | New | edge | SC-003 | FR-NEW-021 | Medium |
| E2E-072 | New | edge | SC-003 | FR-NEW-021 | Medium |
| E2E-073 | New | edge | SC-001 | FR-NEW-006 | Medium |
| E2E-074 | New | edge | SC-003 | FR-NEW-009, FR-NEW-017 | Medium |
| E2E-075 | New | edge | SC-004 | FR-NEW-010, FR-NEW-013 | High |
| E2E-076 | New | edge | SC-004 | FR-NEW-013 | Medium |
| E2E-077 | New | edge | SC-008 | FR-NEW-003 | Medium |
| E2E-078 | New | edge | SC-008 | FR-NEW-003, FR-NEW-016 | Medium |
| E2E-079 | New | edge | SC-003 | FR-NEW-009, FR-NEW-010 | Medium |
| E2E-080 | New | edge | SC-010 | FR-NEW-020 | High |
| E2E-081 | New | edge | SC-010 | FR-NEW-020, FR-NEW-010 | Medium |
| E2E-082 | New | failure | SC-010 | FR-NEW-020 | Medium |
| E2E-083 | New | edge | SC-002 | FR-NEW-005 | Medium |
| E2E-084 | New | side effect | SC-009 | FR-NEW-016, FR-NEW-015 | Critical |
| E2E-085 | New | side effect | SC-009 | FR-NEW-016 | High |
| E2E-086 | New | side effect | SC-009 | FR-NEW-016 | Medium |
| E2E-087 | New | side effect | SC-008 | FR-NEW-015 | Critical |
| E2E-088 | New | side effect | SC-007 | FR-NEW-019, FR-NEW-015 | High |
| E2E-089 | New | side effect | SC-007 | FR-NEW-019 | Medium |
| E2E-090 | New | side effect | SC-006 | FR-NEW-018, FR-NEW-019 | High |
| E2E-091 | New | side effect | SC-002 | FR-NEW-007, FR-NEW-009 | Medium |
| E2E-092 | New | security | SC-008 | FR-NEW-014, FR-NEW-002 | Critical |
| E2E-093 | New | security | SC-008 | FR-NEW-015, FR-MOD-001 | Critical |
| E2E-094 | New | security | SC-008 | FR-NEW-015 | Critical |
| E2E-095 | New | edge | SC-004 | FR-NEW-011 | Medium |
| E2E-096 | New | edge | SC-003 | FR-NEW-021, FR-NEW-007 | Medium |
| E2E-097 | New | requirement | SC-005 | FR-NEW-001, FR-NEW-003, FR-NEW-004 | Medium |
| E2E-098 | New | side effect | SC-005 | FR-NEW-001, FR-NEW-010 | Medium |
| E2E-099 | New | edge | SC-006 | FR-NEW-018, FR-NEW-006 | Medium |
| E2E-100 | New | edge | SC-007 | FR-NEW-019, FR-NEW-005 | Medium |
| E2E-101 | New | requirement | SC-008 | FR-NEW-003, FR-NEW-015 | High |
| E2E-102 | New | failure | SC-009 | FR-NEW-016, FR-NEW-017 | Medium |
| E2E-103 | New | edge | SC-009 | FR-NEW-016 | Medium |
| E2E-104 | New | side effect | SC-010 | FR-NEW-020, FR-NEW-010 | Medium |
| E2E-105 | New | side effect | SC-010 | FR-NEW-020, FR-NEW-016 | Medium |
| E2E-106 | New | side effect | SC-001 | FR-NEW-001, FR-NEW-010 | Medium |
| E2E-107 | New | side effect | SC-004 | FR-NEW-013 | Medium |
| E2E-108 | New | side effect | SC-003 | FR-NEW-009 | Low |
| E2E-109 | New | failure | SC-009 | FR-NEW-016, FR-NEW-017 | Medium |
| E2E-110 | New | failure | SC-010 | FR-NEW-020, FR-NEW-010 | Medium |
| E2E-111 | New | edge | SC-004 | FR-NEW-012, FR-NEW-004 | Medium |
| E2E-112 | New | edge | SC-005 | FR-NEW-004 | Medium |
| E2E-113 | New | failure | SC-004 | FR-NEW-004, FR-NEW-013 | High |
| E2E-114 | New | failure | SC-004 | FR-NEW-010, FR-NEW-015 | High |
| E2E-115 | New | requirement | SC-007 | FR-NEW-019, FR-NEW-002 | High |
| E2E-116 | New | edge | SC-002 | FR-NEW-005, FR-NEW-001 | Medium |
| E2E-117 | New | failure | SC-008 | FR-MOD-001, FR-NEW-015 | High |
| E2E-118 | New | failure | SC-008 | FR-NEW-003, FR-MOD-002, FR-NEW-015 | High |
| E2E-119 | New | side effect | SC-009 | FR-NEW-016, FR-NEW-008, FR-NEW-009 | High |
| E2E-120 | New | edge | SC-003 | FR-NEW-008, FR-NEW-009, FR-NEW-016 | Medium |
| E2E-121 | New | failure | SC-006 | FR-NEW-017 | High |
| E2E-122 | New | failure | SC-002 | FR-NEW-007 | Medium |
| E2E-123 | New | failure | SC-003 | FR-NEW-009 | Medium |
| E2E-124 | New | edge | SC-002 | FR-NEW-007, FR-NEW-021 | Medium |
| E2E-125 | New | edge | SC-005 | FR-NEW-003, FR-NEW-005 | Medium |
| E2E-126 | New | failure | SC-005 | FR-NEW-003 | Medium |
| E2E-127 | New | failure | SC-005 | FR-NEW-003, FR-NEW-015 | Medium |
| E2E-128 | New | failure | SC-005 | FR-NEW-003 | Medium |
| E2E-129 | New | edge | SC-004 | FR-NEW-010, FR-NEW-011 | Low |
| E2E-130 | New | edge | SC-004 | FR-NEW-010, FR-NEW-012 | Medium |
| E2E-131 | New | failure | SC-004 | FR-NEW-010, FR-NEW-012 | Medium |
| E2E-132 | New | failure | SC-004 | FR-NEW-010, FR-NEW-004 | Medium |
| E2E-133 | New | failure | SC-002 | FR-NEW-005 | Medium |
| E2E-134 | New | edge | SC-002 | FR-NEW-005, FR-NEW-001 | Medium |
| E2E-135 | New | failure | SC-008 | FR-MOD-002, FR-NEW-015 | High |
| E2E-136 | New | edge | SC-003 | FR-NEW-009, FR-NEW-021 | Medium |
| E2E-137 | New | edge | SC-002 | FR-NEW-005 | Low |
| E2E-138 | New | failure | SC-005 | FR-NEW-005, FR-NEW-002 | Low |
| E2E-139 | New | edge | SC-007 | FR-NEW-019, FR-NEW-005 | Medium |
| E2E-140 | New | side effect | SC-007 | FR-NEW-019, FR-NEW-016 | Low |
| E2E-141 | New | failure | SC-008 | FR-NEW-003, FR-MOD-002 | Medium |
| E2E-142 | New | edge | SC-007 | FR-NEW-019, FR-NEW-005 | Medium |
| E2E-143 | New | failure | SC-005 | FR-NEW-003 | Medium |
| E2E-144 | New | edge | SC-005 | FR-NEW-003 | Medium |
| E2E-145 | New | failure | SC-005 | FR-NEW-004 | Low |
| E2E-146 | New | failure | SC-005 | FR-NEW-001, FR-NEW-004 | Low |
| E2E-147 | New | failure | SC-005 | FR-NEW-005 | Low |
| E2E-148 | New | failure | SC-004 | FR-NEW-010, FR-NEW-012 | Low |
| E2E-149 | New | side effect | SC-009 | FR-NEW-016, FR-NEW-005 | Medium |
| E2E-150 | New | edge | SC-008 | FR-NEW-005, FR-NEW-003 | Low |
| E2E-MOD-001 | Modified | failure | SC-008 | FR-MOD-001 | High |

Coverage by category: requirement 18, journey 1, failure 69, edge 42, side effect 17, security 3, modified 1. Happy 20 (requirement and journey tests plus the modified one), failure 69: happy to failure ratio 1:3.45 (beats 1:1).

### 9.2 New tests
#### E2E-001: create a program converter
- **Category:** requirement
- **Scenario:** SC-001
- **Requirements:** FR-NEW-001, FR-NEW-002, FR-NEW-004
- **Given** no converter exists
- **When** Alice creates the converter `Pandoc local`, form `program`, command `["pandoc-convert", "--out", "{outdir}", "{document}"]`
- **Then** the answer holds a converter id starting with `conv-`, the name, the form `program`, the command as given, `timeout_seconds` 900, `max_input_bytes` 536870912, an empty extension list, `has_secret` false and equal creation and update times
- **And** listing the converters shows exactly this one
- **Priority:** Critical

#### E2E-002: create an http converter with a secret
- **Category:** requirement
- **Scenario:** SC-001
- **Requirements:** FR-NEW-001, FR-NEW-003
- **Given** no converter exists
- **When** Alice creates `Docling API`, form `http`, url `http://localhost:8099/convert`, secret `Bearer tok-123`
- **Then** the answer shows `auth_header` `Authorization`, `file_field` `file`, `response_field` empty, `has_secret` true and no secret value anywhere in it
- **Priority:** Critical

#### E2E-003: converters are listed by name without regard to case
- **Category:** requirement
- **Scenario:** SC-001
- **Requirements:** FR-NEW-006
- **Given** the converters `docling`, `Pandoc local` and `alpha`
- **When** Alice lists the converters
- **Then** the order is `alpha`, `docling`, `Pandoc local`, with no marker
- **Priority:** High

#### E2E-004: the platform default is read
- **Category:** requirement
- **Scenario:** SC-003
- **Requirements:** FR-NEW-008
- **Given** no default set
- **When** Alice reads the default, sets `Pandoc local` as default, reads it again
- **Then** the first answer holds no converter (`null`), the second holds `Pandoc local` with its id
- **Priority:** High

#### E2E-005: a converter is updated
- **Category:** requirement
- **Scenario:** SC-002
- **Requirements:** FR-NEW-005, FR-NEW-004
- **Given** the converter named `Pandoc local` (form `program`, command `["pandoc-convert", "{document}"]`) exists
- **When** Alice renames it `Pandoc 3`, sets `timeout_seconds` 120 and `extensions` `["PDF", "docx"]`
- **Then** the answer keeps the same converter id and shows the name `Pandoc 3`, the timeout 120 and the extensions `["pdf", "docx"]` (lower case); the update time is not before the creation time
- **Priority:** High

#### E2E-006: the platform default is set and replaced
- **Category:** requirement
- **Scenario:** SC-003
- **Requirements:** FR-NEW-008
- **Given** `Pandoc local` and `Docling API` exist
- **When** Alice sets `Pandoc local` as default, then `Docling API`
- **Then** reading the default answers `Docling API`
- **Priority:** High

#### E2E-007: a project converter is set and read
- **Category:** requirement
- **Scenario:** SC-003
- **Requirements:** FR-NEW-009
- **Given** `Docling API` exists and project `proj` exists
- **When** Alice sets `Docling API` as the converter of `proj` and reads it
- **Then** the answer holds `Docling API` and the project id `proj`
- **Priority:** High

#### E2E-008: an unused converter is deleted
- **Category:** requirement
- **Scenario:** SC-002
- **Requirements:** FR-NEW-007
- **Given** the converter named `Pandoc local` (form `program`, command `["pandoc-convert", "{document}"]`) exists, used by nothing
- **When** Alice deletes it
- **Then** the answer confirms the deletion and listing shows no converter
- **Priority:** High

#### E2E-009: a conversion uses the project converter
- **Category:** requirement
- **Scenario:** SC-004
- **Requirements:** FR-NEW-010
- **Given** converter A (`Pandoc local`) returns `# From A` and converter B (`Docling API`) returns `# From B`; the platform default is A; the converter of `proj` is B
- **When** Bob writes `/report.pdf` in `proj` with the conversion option on and reads its extracted text
- **Then** the text is `# From B`
- **Priority:** Critical

#### E2E-010: a conversion uses the platform default when the project has none
- **Category:** requirement
- **Scenario:** SC-004
- **Requirements:** FR-NEW-010
- **Given** the platform default is converter A; `proj` has no converter
- **When** Bob converts `/report.pdf`
- **Then** the text is `# From A`
- **Priority:** Critical

#### E2E-011: a conversion uses the configuration file converter when nothing is selected
- **Category:** requirement
- **Scenario:** SC-010
- **Requirements:** FR-NEW-010, FR-NEW-020
- **Given** no default, no project converter, and the configuration file defines a converter returning `# From file`
- **When** Bob converts `/report.pdf`
- **Then** the text is `# From file`
- **Priority:** High

#### E2E-012: a change applies to the next conversion without a restart
- **Category:** requirement
- **Scenario:** SC-004
- **Requirements:** FR-NEW-011
- **Given** the platform default is A and `/a.pdf` was converted with it
- **When** Alice sets B as the default, Bob converts `/b.pdf`
- **Then** `/b.pdf` gives `# From B` and `/a.pdf` still gives `# From A`
- **Priority:** High

#### E2E-013: REST creates and lists like the tools
- **Category:** requirement
- **Scenario:** SC-006
- **Requirements:** FR-NEW-018
- **Given** no converter exists
- **When** Alice creates `Pandoc local` through the web interface and lists through the tool and through the web interface
- **Then** both listings are identical and the creation answer equals the tool's answer for the same input
- **Priority:** High

#### E2E-014: the screen creates a converter
- **Category:** requirement
- **Scenario:** SC-007
- **Requirements:** FR-NEW-019
- **Given** Alice opens the converters screen in a browser session
- **When** she fills the form for `Docling API` (form `http`, the url, the secret `Bearer tok-123`) with the one time token and submits
- **Then** the screen lists `Docling API` with `Secret stored` and no secret text
- **Priority:** High

#### E2E-015: the record of changes lists the changes
- **Category:** requirement
- **Scenario:** SC-009
- **Requirements:** FR-NEW-016
- **Given** Alice created `Pandoc local`, renamed it `Pandoc 3`, set it as default and deleted nothing
- **When** she lists the setting changes
- **Then** the three latest entries are, newest first, the default set, the update with the field `name`, the creation, each with `person` `alice@example.com`
- **Priority:** High

#### E2E-016: a converter set up end to end
- **Category:** journey
- **Scenario:** SC-004
- **Requirements:** FR-NEW-001, FR-NEW-003, FR-NEW-008, FR-NEW-009, FR-NEW-010, FR-NEW-007
- **Given** no converter; the stand in HTTP converter B and the stand in program A are running
- **When** Alice creates A and B, makes A the default, gives `proj` the converter B, Bob converts `/report.pdf`, Alice clears the project converter, Bob converts `/other.pdf`, Alice clears the default and deletes B
- **Then** the first text is `# From B`, the second `# From A`, the deletion of B succeeds only after the project converter was cleared
- **Priority:** Critical

#### E2E-017: an empty name
- **Category:** failure
- **Scenario:** SC-005
- **Requirements:** FR-NEW-001
- **Given** no converter exists
- **When** Alice creates a converter with the name `   `
- **Then** she is refused with `ERR_INVALID_ARGUMENT: name must be 1 to 64 characters`
- **Priority:** High

#### E2E-018: a name of 65 characters
- **Category:** failure
- **Scenario:** SC-005
- **Requirements:** FR-NEW-001
- **Given** no converter exists
- **When** Alice creates a converter whose name has 65 characters
- **Then** she is refused with `ERR_INVALID_ARGUMENT: name must be 1 to 64 characters`
- **Priority:** High

#### E2E-019: a name taken in another letter case
- **Category:** failure
- **Scenario:** SC-001
- **Requirements:** FR-NEW-001
- **Given** the converter named `Pandoc local` (form `program`, command `["pandoc-convert", "{document}"]`) exists
- **When** Alice creates `PANDOC LOCAL`
- **Then** she is refused with `ERR_INVALID_ARGUMENT: a converter named 'PANDOC LOCAL' already exists`
- **Priority:** High

#### E2E-020: an unknown form
- **Category:** failure
- **Scenario:** SC-005
- **Requirements:** FR-NEW-001
- **Given** no converter exists
- **When** Alice creates a converter with the form `ftp`
- **Then** she is refused with `ERR_INVALID_ARGUMENT: form must be 'program' or 'http'`
- **Priority:** High

#### E2E-021: a program converter without a command
- **Category:** failure
- **Scenario:** SC-005
- **Requirements:** FR-NEW-002
- **Given** no converter exists
- **When** Alice creates a `program` converter with the command `["", "  "]`
- **Then** she is refused with `ERR_INVALID_ARGUMENT: command is required for a program converter`
- **Priority:** High

#### E2E-022: a command that is not a list
- **Category:** failure
- **Scenario:** SC-005
- **Requirements:** FR-NEW-002, FR-NEW-014
- **Given** no converter exists
- **When** Alice creates a `program` converter whose command is the text `pandoc-convert {document}`
- **Then** she is refused with `ERR_INVALID_ARGUMENT: command must be a list of text arguments`
- **Priority:** High

#### E2E-023: a command without the document placeholder
- **Category:** failure
- **Scenario:** SC-005
- **Requirements:** FR-NEW-002
- **Given** no converter exists
- **When** Alice creates a `program` converter with the command `["pandoc-convert"]`
- **Then** she is refused with `ERR_INVALID_ARGUMENT: command must contain exactly one '{document}' placeholder across its arguments, found 0`
- **Priority:** High

#### E2E-024: a command with the document placeholder twice
- **Category:** failure
- **Scenario:** SC-005
- **Requirements:** FR-NEW-002
- **Given** no converter exists
- **When** Alice creates a `program` converter with the command `["c", "{document}", "{document}"]`
- **Then** she is refused with `ERR_INVALID_ARGUMENT: command must contain exactly one '{document}' placeholder across its arguments, found 2`
- **Priority:** High

#### E2E-025: a command with the output folder placeholder twice
- **Category:** failure
- **Scenario:** SC-005
- **Requirements:** FR-NEW-002
- **Given** no converter exists
- **When** Alice creates a `program` converter with the command `["c", "{outdir}", "{outdir}", "{document}"]`
- **Then** she is refused with `ERR_INVALID_ARGUMENT: command may contain at most one '{outdir}' placeholder across its arguments, found 2`
- **Priority:** High

#### E2E-026: an http field given to a program converter
- **Category:** failure
- **Scenario:** SC-005
- **Requirements:** FR-NEW-002
- **Given** no converter exists
- **When** Alice creates a `program` converter with a valid command and the url `http://localhost:8099/convert`
- **Then** she is refused with `ERR_INVALID_ARGUMENT: url is not valid for a program converter`
- **Priority:** High

#### E2E-027: an http converter without a url
- **Category:** failure
- **Scenario:** SC-005
- **Requirements:** FR-NEW-003
- **Given** no converter exists
- **When** Alice creates an `http` converter with no url
- **Then** she is refused with `ERR_INVALID_ARGUMENT: url is required for an http converter`
- **Priority:** High

#### E2E-028: an http converter with a url that is not http or https
- **Category:** failure
- **Scenario:** SC-005
- **Requirements:** FR-NEW-003
- **Given** no converter exists
- **When** Alice creates an `http` converter with the url `ftp://host/convert`
- **Then** she is refused with `ERR_INVALID_ARGUMENT: url must be an http or https URL`
- **Priority:** High

#### E2E-029: an http converter with a user name in the url
- **Category:** failure
- **Scenario:** SC-005
- **Requirements:** FR-NEW-003, FR-NEW-015
- **Given** no converter exists
- **When** Alice creates an `http` converter with the url `http://bob:pw@localhost:8099/convert`
- **Then** she is refused with `ERR_INVALID_ARGUMENT: url must not carry a user name or password`, and the answer does not quote the url
- **Priority:** High

#### E2E-030: a secret without a header name
- **Category:** failure
- **Scenario:** SC-005
- **Requirements:** FR-NEW-003
- **Given** no converter exists
- **When** Alice creates an `http` converter with the secret `tok` and the header name `   `
- **Then** she is refused with `ERR_INVALID_ARGUMENT: auth_header is required when a secret is set`
- **Priority:** High

#### E2E-031: an empty file field
- **Category:** failure
- **Scenario:** SC-005
- **Requirements:** FR-NEW-003
- **Given** no converter exists
- **When** Alice creates an `http` converter with the file field `  `
- **Then** she is refused with `ERR_INVALID_ARGUMENT: file_field must not be empty`
- **Priority:** High

#### E2E-032: a command given to an http converter
- **Category:** failure
- **Scenario:** SC-005
- **Requirements:** FR-NEW-003
- **Given** no converter exists
- **When** Alice creates an `http` converter with a valid url and the command `["c", "{document}"]`
- **Then** she is refused with `ERR_INVALID_ARGUMENT: command is not valid for an http converter`
- **Priority:** High

#### E2E-033: a timeout of 0
- **Category:** failure
- **Scenario:** SC-005
- **Requirements:** FR-NEW-004
- **Given** no converter exists
- **When** Alice creates a valid program converter with `timeout_seconds` 0
- **Then** she is refused with `ERR_INVALID_ARGUMENT: timeout_seconds must be between 1 and 86400`
- **Priority:** High

#### E2E-034: a timeout of 86401
- **Category:** failure
- **Scenario:** SC-005
- **Requirements:** FR-NEW-004
- **Given** no converter exists
- **When** Alice creates a valid program converter with `timeout_seconds` 86401
- **Then** she is refused with `ERR_INVALID_ARGUMENT: timeout_seconds must be between 1 and 86400`
- **Priority:** High

#### E2E-035: an answer cap of 0
- **Category:** failure
- **Scenario:** SC-005
- **Requirements:** FR-NEW-004
- **Given** no converter exists
- **When** Alice creates a valid program converter with `max_input_bytes` 0
- **Then** she is refused with `ERR_INVALID_ARGUMENT: max_input_bytes must be between 1 and 4294967296`
- **Priority:** High

#### E2E-036: extensions that are not names
- **Category:** failure
- **Scenario:** SC-005
- **Requirements:** FR-NEW-004
- **Given** no converter exists
- **When** Alice creates a valid program converter with `extensions` `[".pdf"]`, then with an extension of 17 letters, then with 65 extensions
- **Then** each is refused with `ERR_INVALID_ARGUMENT: extensions must be a list of up to 64 names of 1 to 16 letters or digits`
- **Priority:** High

#### E2E-037: a secret of 4097 characters
- **Category:** failure
- **Scenario:** SC-005
- **Requirements:** FR-NEW-003
- **Given** no converter exists
- **When** Alice creates a valid `http` converter with a secret of 4097 characters
- **Then** she is refused with `ERR_INVALID_ARGUMENT: secret must be at most 4096 characters`
- **Priority:** High

#### E2E-038: a secret when no master key is configured
- **Category:** failure
- **Scenario:** SC-005
- **Requirements:** FR-NEW-003, FR-NEW-015
- **Given** the platform has no master key
- **When** Alice creates a valid `http` converter with the secret `tok` on a platform without a master key
- **Then** she is refused with `ERR_NOT_SUPPORTED: no master key is configured; set FSA_TOKEN_KEY` and no converter is created
- **Priority:** High

#### E2E-039: a secret kept and cleared together
- **Category:** failure
- **Scenario:** SC-005
- **Requirements:** FR-NEW-005, FR-NEW-003
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) exists with a secret
- **When** Alice updates `Docling API` giving `secret` `new` and `clear_secret` true
- **Then** she is refused with `ERR_INVALID_ARGUMENT: secret and clear_secret cannot be given together` and the stored secret is unchanged
- **Priority:** High

#### E2E-040: an update of an unknown converter
- **Category:** failure
- **Scenario:** SC-005
- **Requirements:** FR-NEW-005
- **Given** no converter exists
- **When** Alice updates the converter `conv-000000000000`
- **Then** she is refused with `ERR_NOT_FOUND: converter not found: conv-000000000000`
- **Priority:** High

#### E2E-041: a change of form
- **Category:** failure
- **Scenario:** SC-005
- **Requirements:** FR-NEW-005
- **Given** the converter named `Pandoc local` (form `program`, command `["pandoc-convert", "{document}"]`) exists
- **When** Alice updates `Pandoc local` giving the form `http`
- **Then** she is refused with `ERR_INVALID_ARGUMENT: form cannot be changed`
- **Priority:** High

#### E2E-042: an update that breaks a rule
- **Category:** failure
- **Scenario:** SC-005
- **Requirements:** FR-NEW-005, FR-NEW-002
- **Given** the converter named `Pandoc local` (form `program`, command `["pandoc-convert", "{document}"]`) exists
- **When** Alice updates `Pandoc local` with the command `["c"]`
- **Then** she is refused with `ERR_INVALID_ARGUMENT: command must contain exactly one '{document}' placeholder across its arguments, found 0` and the converter is unchanged
- **Priority:** High

#### E2E-043: the default converter cannot be deleted
- **Category:** failure
- **Scenario:** SC-002
- **Requirements:** FR-NEW-007
- **Given** the converter named `Pandoc local` (form `program`, command `["pandoc-convert", "{document}"]`) is the platform default
- **When** Alice deletes the converter that is the platform default
- **Then** she is refused with `ERR_INVALID_ARGUMENT: converter 'Pandoc local' is the platform default`
- **Priority:** High

#### E2E-044: a converter used by projects cannot be deleted
- **Category:** failure
- **Scenario:** SC-002
- **Requirements:** FR-NEW-007
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) is the converter of the projects `proj` and `other`
- **When** Alice deletes the converter chosen by `proj` and by `other`
- **Then** she is refused with `ERR_INVALID_ARGUMENT: converter 'Docling API' is used by 2 project(s)`
- **Priority:** High

#### E2E-045: a deletion of an unknown converter
- **Category:** failure
- **Scenario:** SC-002
- **Requirements:** FR-NEW-007
- **Given** no converter exists
- **When** Alice deletes `conv-000000000000`
- **Then** she is refused with `ERR_NOT_FOUND: converter not found: conv-000000000000`
- **Priority:** High

#### E2E-046: a default that does not exist
- **Category:** failure
- **Scenario:** SC-003
- **Requirements:** FR-NEW-008
- **Given** no converter exists
- **When** Alice sets `conv-000000000000` as the default
- **Then** she is refused with `ERR_NOT_FOUND: converter not found: conv-000000000000`
- **Priority:** High

#### E2E-047: a project that does not exist
- **Category:** failure
- **Scenario:** SC-003
- **Requirements:** FR-NEW-009
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) exists
- **When** Alice sets `Docling API` as the converter of `ghost`
- **Then** she is refused with `ERR_PROJECT_NOT_FOUND: project 'ghost' not found`
- **Priority:** High

#### E2E-048: a project converter that does not exist
- **Category:** failure
- **Scenario:** SC-003
- **Requirements:** FR-NEW-009
- **Given** no converter exists
- **When** Alice sets `conv-000000000000` as the converter of `proj`
- **Then** she is refused with `ERR_NOT_FOUND: converter not found: conv-000000000000`
- **Priority:** High

#### E2E-049: a page marker that is not valid
- **Category:** failure
- **Scenario:** SC-001
- **Requirements:** FR-NEW-006
- **Given** no converter exists
- **When** Alice lists the converters with the marker `garbage`
- **Then** she is refused with `ERR_INVALID_ARGUMENT: marker is not valid`
- **Priority:** High

#### E2E-050: only platform administrators use the converter tools
- **Category:** failure
- **Scenario:** SC-006
- **Requirements:** FR-NEW-017
- **Given** Bob is a member and owner of `proj` and not a platform administrator; a converter exists
- **When** Bob creates, updates, deletes, lists, sets the default, reads the default, sets and reads the converter of `proj`
- **Then** each is refused with `ERR_FORBIDDEN: 'bob@example.com' is not a platform admin`
- **And** nothing is changed and no converter name appears in any refusal
- **Priority:** Critical

#### E2E-051: the web interface refuses a non administrator the same way
- **Category:** failure
- **Scenario:** SC-006
- **Requirements:** FR-NEW-017, FR-NEW-018
- **Given** Bob is not a platform administrator
- **When** Bob sends a creation and a listing through the web interface
- **Then** each is refused with `ERR_FORBIDDEN` and the detail `ERR_FORBIDDEN: 'bob@example.com' is not a platform admin`
- **Priority:** High

#### E2E-052: the screen refuses a non administrator
- **Category:** failure
- **Scenario:** SC-007
- **Requirements:** FR-NEW-017, FR-NEW-019
- **Given** Bob opens the converters screen and posts a creation
- **When** the screen answers
- **Then** both are refused with the platform administrator refusal and nothing is created
- **Priority:** High

#### E2E-053: a form without its one time token
- **Category:** failure
- **Scenario:** SC-007
- **Requirements:** FR-NEW-019
- **Given** Alice is in a browser session
- **When** she posts the creation form without a token, then with a token already used, then with a token issued to Bob
- **Then** each is refused with `ERR_FORBIDDEN: missing or invalid csrf_token` and nothing is created
- **Priority:** High

#### E2E-054: an extension the resolved converter does not accept
- **Category:** failure
- **Scenario:** SC-004
- **Requirements:** FR-NEW-012
- **Given** the converter of `proj` accepts only `txt`; the platform default accepts `pdf`
- **When** Bob converts `/report.pdf` in `proj`
- **Then** the conversion is refused with the existing refusal `'/report.pdf' cannot be documented, the document service accepts: .txt`, the platform default is not tried and nothing is written
- **Priority:** High

#### E2E-055: no converter anywhere
- **Category:** failure
- **Scenario:** SC-004
- **Requirements:** FR-NEW-012
- **Given** no project converter, no default and no converter in the configuration file
- **When** Bob converts `/report.pdf`
- **Then** the conversion is refused with `ERR_NOT_SUPPORTED: document service is not configured` and nothing is written
- **Priority:** High

#### E2E-056: a converter that fails
- **Category:** failure
- **Scenario:** SC-004
- **Requirements:** FR-NEW-013
- **Given** the converter of `proj` returns an error
- **When** Bob converts `/report.pdf`
- **Then** the file is written, the conversion failure is reported in the answer as for a converter of the configuration file and no companion or artifact exists
- **Priority:** High

#### E2E-057: an answer over the converter cap
- **Category:** failure
- **Scenario:** SC-004
- **Requirements:** FR-NEW-013, FR-NEW-004
- **Given** the converter of `proj` has `max_input_bytes` 1000, a 900 byte document `/report.pdf` and the converter returns 1001 bytes
- **When** Bob converts `/report.pdf`
- **Then** the failure is `document service output is over the doc_service.max_input_bytes limit of 1000` and nothing is kept
- **Priority:** High

#### E2E-058: a web refusal equals the tool refusal
- **Category:** failure
- **Scenario:** SC-006
- **Requirements:** FR-NEW-018
- **Given** no converter exists
- **When** Alice updates `conv-000000000000` through the web interface and through the tool
- **Then** the error code and the text are the same
- **Priority:** Medium

#### E2E-059: a failed converter answer that echoes the secret
- **Category:** failure
- **Scenario:** SC-008
- **Requirements:** FR-NEW-015, FR-MOD-001
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) has the secret `Bearer tok-SECRET-123`; the stub answers an error whose body repeats the header it received
- **When** Bob converts `/report.pdf` through it
- **Then** the failure shown to Bob holds `[redacted]` where the secret was and never the text `tok-SECRET-123`
- **And** the same holds for the converter of the configuration file with its own token
- **Priority:** Critical

#### E2E-060: a converter removed between the check and the use
- **Category:** failure
- **Scenario:** SC-004
- **Requirements:** FR-NEW-011, FR-NEW-010
- **Given** Bob's conversion has started with converter B
- **When** Alice deletes B after clearing the project selection while the conversion runs
- **Then** the running conversion ends with `# From B`; the next conversion uses the default
- **Priority:** Medium

#### E2E-061: a name of 64 characters and accents
- **Category:** edge
- **Scenario:** SC-005
- **Requirements:** FR-NEW-001
- **Given** no converter
- **When** Alice creates a converter named with 64 characters ending in `é🌐`
- **Then** it is created and listed with its exact name
- **Priority:** Medium

#### E2E-062: timeout bounds
- **Category:** edge
- **Scenario:** SC-005
- **Requirements:** FR-NEW-004
- **Given** no converter
- **When** Alice creates two converters, with `timeout_seconds` 1 and 86400
- **Then** both are created
- **Priority:** Medium

#### E2E-063: an answer cap bound
- **Category:** edge
- **Scenario:** SC-005
- **Requirements:** FR-NEW-004
- **Given** no converter
- **When** Alice creates two converters with `max_input_bytes` 1 and 4294967296
- **Then** both are created
- **Priority:** Medium

#### E2E-064: empty extensions means the built in list
- **Category:** edge
- **Scenario:** SC-004
- **Requirements:** FR-NEW-012, FR-NEW-004
- **Given** the converter of `proj` has no extensions
- **When** Bob converts `/a.pdf`, `/b.docx` and `/c.xyz`
- **Then** `/a.pdf` and `/b.docx` are converted, `/c.xyz` is refused with the list of the built in extensions, each with its leading dot, in their built in order
- **Priority:** Medium

#### E2E-065: custom extensions
- **Category:** edge
- **Scenario:** SC-004
- **Requirements:** FR-NEW-012, FR-NEW-004
- **Given** the converter of `proj` has `extensions` `["txt"]`
- **When** Bob converts `/a.txt` then `/a.pdf`
- **Then** `/a.txt` is converted; `/a.pdf` is refused with `'/a.pdf' cannot be documented, the document service accepts: .txt`
- **Priority:** Medium

#### E2E-066: a secret kept, replaced and cleared
- **Category:** edge
- **Scenario:** SC-008
- **Requirements:** FR-NEW-005, FR-NEW-003
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) with the secret `s1`
- **When** Alice updates it without a secret, then with `secret` `s2`, then with `clear_secret` true
- **Then** `has_secret` stays true after the first update, stays true after the second (the converter now sends `s2`), and is false after the third
- **Priority:** High

#### E2E-067: an argument that looks like a shell command stays one argument
- **Category:** edge
- **Scenario:** SC-008
- **Requirements:** FR-NEW-014
- **Given** no converter and no file `/tmp/fsa-pwned`; `record-args` writes the arguments it receives into the Markdown it returns
- **When** Alice creates a `program` converter with the command `["record-args", "x; touch /tmp/fsa-pwned #", "{document}"]` and Bob converts `/report.pdf` with it
- **Then** the extracted text, in which `record-args` writes the arguments it received, lists exactly 3 arguments, the second being `x; touch /tmp/fsa-pwned #`
- **And** no file `/tmp/fsa-pwned` exists (it did not exist before)
- **Priority:** Critical

#### E2E-068: the project converter wins over the default and the file
- **Category:** edge
- **Scenario:** SC-004
- **Requirements:** FR-NEW-010
- **Given** the file converter, a platform default A and a project converter B all exist
- **When** Bob converts
- **Then** the text is `# From B`; after Alice clears the project converter `# From A`; after she clears the default `# From file`
- **Priority:** High

#### E2E-069: clearing the project converter
- **Category:** edge
- **Scenario:** SC-003
- **Requirements:** FR-NEW-009
- **Given** `proj` has a converter
- **When** Alice clears it and reads it
- **Then** the answer holds no converter and listing shows the converter unused (deletable)
- **Priority:** Medium

#### E2E-070: deleting after unsetting
- **Category:** edge
- **Scenario:** SC-002
- **Requirements:** FR-NEW-007, FR-NEW-008
- **Given** the converter named `Pandoc local` (form `program`, command `["pandoc-convert", "{document}"]`) is the default
- **When** Alice clears the default then deletes it
- **Then** the deletion succeeds
- **Priority:** Medium

#### E2E-071: a project removed for good loses its selection
- **Category:** edge
- **Scenario:** SC-003
- **Requirements:** FR-NEW-021
- **Given** `proj` has converter B; the platform removes `proj` for good after its grace period
- **When** Alice deletes B
- **Then** B is deleted (it is used by no project); a recreated `proj` has no converter
- **Priority:** Medium

#### E2E-072: a project soft deleted and restored keeps its converter
- **Category:** edge
- **Scenario:** SC-003
- **Requirements:** FR-NEW-021
- **Given** `proj` has converter B and is removed then restored within its grace period
- **When** Alice reads the converter of `proj`
- **Then** it is B
- **Priority:** Medium

#### E2E-073: 101 converters are paged
- **Category:** edge
- **Scenario:** SC-001
- **Requirements:** FR-NEW-006
- **Given** 101 converters named `c001` to `c101`
- **When** Alice lists, then follows the marker
- **Then** 100 converters and a marker, then one converter and no marker
- **Priority:** Medium

#### E2E-074: an administrator who is not a member can select but not read files
- **Category:** edge
- **Scenario:** SC-003
- **Requirements:** FR-NEW-009, FR-NEW-017
- **Given** Alice is a platform administrator and not a member of `proj`
- **When** Alice sets a converter for `proj` and lists the files of `proj`
- **Then** the selection succeeds and the file listing is refused with `ERR_FORBIDDEN: 'alice@example.com' is not a member of 'proj'`
- **Priority:** Medium

#### E2E-075: all three conversion paths use the resolved converter
- **Category:** edge
- **Scenario:** SC-004
- **Requirements:** FR-NEW-010, FR-NEW-013
- **Given** the converter of `proj` is B
- **When** Bob converts through a write with the option, through an upload with the option and through the conversion of an existing file
- **Then** all three give `# From B`
- **Priority:** High

#### E2E-076: the answer carries the full document metadata through a database converter
- **Category:** edge
- **Scenario:** SC-004
- **Requirements:** FR-NEW-013
- **Given** an http converter returns a version 2 manifest with one picture and OCR text `Revenue`
- **When** Bob converts `/report.pdf`
- **Then** the image list shows the picture with OCR text `Revenue`, as for a converter of the configuration file
- **Priority:** Medium

#### E2E-077: the secret is sent as the header value
- **Category:** edge
- **Scenario:** SC-008
- **Requirements:** FR-NEW-003
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) has `auth_header` `X-Api-Key` and the secret `abc`
- **When** Bob converts `/report.pdf`
- **Then** the conversion gives `# From B`, because the stub answers `# From B` only when it receives the header `X-Api-Key: abc` and no `Authorization` header (otherwise it answers 401)
- **Priority:** Medium

#### E2E-078: a converter with a secret is kept after a restart
- **Category:** edge
- **Scenario:** SC-008
- **Requirements:** FR-NEW-003, FR-NEW-016
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) with the secret `tok`; the platform restarts with the same master key
- **When** Bob converts
- **Then** the conversion gives `# From B` (the stub answers it only for the header value `tok`); the list and the record of changes are unchanged
- **Priority:** Medium

#### E2E-079: the other project is not affected
- **Category:** edge
- **Scenario:** SC-003
- **Requirements:** FR-NEW-009, FR-NEW-010
- **Given** `proj` has converter B, `other` has none, the default is A
- **When** a member of each converts
- **Then** `proj` gives `# From B`, `other` gives `# From A`
- **Priority:** Medium

#### E2E-080: the configuration file converter is untouched while the registry is empty
- **Category:** edge
- **Scenario:** SC-010
- **Requirements:** FR-NEW-020
- **Given** the platform upgrades with an empty registry and a configuration file converter
- **When** Bob converts and Alice lists converters
- **Then** the conversion behaves as before the upgrade and the list is empty
- **Priority:** High

#### E2E-081: a registry default works with the configuration file converter disabled
- **Category:** edge
- **Scenario:** SC-010
- **Requirements:** FR-NEW-020, FR-NEW-010
- **Given** the configuration file converter is disabled and the platform default is A
- **When** Bob converts
- **Then** the conversion succeeds with `# From A`
- **Priority:** Medium

#### E2E-082: the configuration file is still checked at start
- **Category:** failure
- **Scenario:** SC-010
- **Requirements:** FR-NEW-020
- **Given** the configuration file converter has two `{document}` placeholders
- **When** the platform starts
- **Then** it refuses to start with the existing configuration error text
- **Priority:** Medium

#### E2E-083: an update renames without changing the id
- **Category:** edge
- **Scenario:** SC-002
- **Requirements:** FR-NEW-005
- **Given** the converter named `Pandoc local` (form `program`, command `["pandoc-convert", "{document}"]`) is the default and the converter of `proj`
- **When** Alice renames it `Pandoc 4`
- **Then** the default and the project converter are still it, and a conversion still uses it
- **Priority:** Medium

#### E2E-084: the record of changes holds field names only
- **Category:** side effect
- **Scenario:** SC-009
- **Requirements:** FR-NEW-016, FR-NEW-015
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) exists
- **When** Alice updates its url, its secret and its command timeout
- **Then** the record shows the action `update_converter`, `target_kind` `converter` and `fields` `["url", "secret", "timeout_seconds"]`, and holds no url, no secret and no number
- **Priority:** Critical

#### E2E-085: refused and no-op changes are not recorded
- **Category:** side effect
- **Scenario:** SC-009
- **Requirements:** FR-NEW-016
- **Given** the converter named `Pandoc local` (form `program`, command `["pandoc-convert", "{document}"]`) exists
- **When** Alice sends a refused creation and an update that changes nothing
- **Then** the record gained no entry
- **Priority:** High

#### E2E-086: the record keeps the order of changes
- **Category:** side effect
- **Scenario:** SC-009
- **Requirements:** FR-NEW-016
- **Given** Alice makes 3 changes in the same second
- **When** she lists the changes
- **Then** they appear newest first with increasing change ids
- **Priority:** Medium

#### E2E-087: the secret is never revealed
- **Category:** side effect
- **Scenario:** SC-008
- **Requirements:** FR-NEW-015
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) has the secret `tok-SECRET-123`
- **When** Alice lists, reads the default, reads the project converter, lists the changes, opens the screen and a conversion fails
- **Then** none of these answers, the screen page or the server log holds `tok-SECRET-123`
- **Priority:** Critical

#### E2E-088: the screen shows the secret state only
- **Category:** side effect
- **Scenario:** SC-007
- **Requirements:** FR-NEW-019, FR-NEW-015
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) has a secret
- **When** Alice opens the screen and its edit form
- **Then** the screen shows `Secret stored`, the secret field is empty, and leaving it empty on save keeps the secret
- **Priority:** High

#### E2E-089: the screen shows the refusal text
- **Category:** side effect
- **Scenario:** SC-007
- **Requirements:** FR-NEW-019
- **Given** Alice is in a browser session
- **When** she creates a converter with the name `   ` on the screen
- **Then** the screen is shown again with the status of a refused request and the text `name must be 1 to 64 characters`, and creates nothing; a creation that succeeds redirects to the converters screen
- **Priority:** Medium

#### E2E-090: a change through one interface is visible through the others
- **Category:** side effect
- **Scenario:** SC-006
- **Requirements:** FR-NEW-018, FR-NEW-019
- **Given** no converter
- **When** Alice creates through the screen, then lists through the tool and the web interface
- **Then** the converter is listed by all three
- **Priority:** High

#### E2E-091: project selection appears in the usage count
- **Category:** side effect
- **Scenario:** SC-002
- **Requirements:** FR-NEW-007, FR-NEW-009
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) is the converter of `proj`
- **When** Alice clears the selection and deletes the converter
- **Then** the deletion succeeds (it was refused with `converter 'Docling API' is used by 1 project(s)` while the selection existed)
- **Priority:** Medium

#### E2E-092: an argument cannot start a program of its own (exploit)
- **Category:** security
- **Scenario:** SC-008
- **Requirements:** FR-NEW-014, FR-NEW-002
- **Given** no converter; the file `/tmp/fsa-pwned` does not exist; the stand in program `record-args` writes the arguments it receives into the Markdown it returns
- **When** Alice creates a `program` converter with the command `["record-args", "$(touch /tmp/fsa-pwned)", "`touch /tmp/fsa-pwned`", "x | touch /tmp/fsa-pwned", "{document}"]`, Bob converts `/report.pdf`
- **Then** the extracted text, in which `record-args` writes the arguments it received, lists 5 arguments, the three payloads each as one literal argument
- **And** no file `/tmp/fsa-pwned` exists (it did not exist before the test)
- **Priority:** Critical

#### E2E-093: a converter answer cannot reveal the secret (exploit)
- **Category:** security
- **Scenario:** SC-008
- **Requirements:** FR-NEW-015, FR-MOD-001
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) with the secret `Bearer tok-SECRET-123`; the stub answers status 500 with a body that repeats the header it received
- **When** Bob converts `/report.pdf`
- **Then** the failure text holds `[redacted]` and never `tok-SECRET-123`
- **And** the log holds no `tok-SECRET-123`
- **And** the converter of the configuration file with the token `cfg-TOKEN-456` behaves the same
- **Priority:** Critical

#### E2E-094: the stored secret cannot be read back by any read (exploit)
- **Category:** security
- **Scenario:** SC-008
- **Requirements:** FR-NEW-015
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) with the secret `tok-SECRET-123`
- **When** Alice reads every answer and screen of the converter registry, and an operator reads the stored form of the setting
- **Then** the text `tok-SECRET-123` appears in none (the stored form is encrypted)
- **And** the same refusals with no master key leave nothing stored
- **Priority:** Critical

#### E2E-095: a conversion in flight keeps the project converter it started with
- **Category:** edge
- **Scenario:** SC-004
- **Requirements:** FR-NEW-011
- **Given** Bob's conversion in `proj` has started with converter B
- **When** Alice sets converter A as the converter of `proj` while it runs, then Bob converts another file
- **Then** the running conversion ends with `# From B`, the next one gives `# From A`
- **Priority:** Medium

#### E2E-096: a project removed for good no longer counts as using a converter
- **Category:** edge
- **Scenario:** SC-003
- **Requirements:** FR-NEW-021, FR-NEW-007
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) is the converter of `proj` only; the platform removes `proj` for good
- **When** Alice deletes `Docling API`
- **Then** it is deleted without the usage refusal
- **Priority:** Medium

#### E2E-097: a converter created with every field given
- **Category:** requirement
- **Scenario:** SC-005
- **Requirements:** FR-NEW-001, FR-NEW-003, FR-NEW-004
- **Given** no converter
- **When** Alice creates `Docling API`, form `http`, url `https://converter.example.com/v1/convert`, auth_header `X-Api-Key`, secret `k`, file_field `document`, response_field `markdown`, extensions `["pdf"]`, max_input_bytes 1000000, timeout_seconds 60
- **Then** the answer shows every value as given, `has_secret` true
- **Priority:** Medium

#### E2E-098: a refused creation changes nothing
- **Category:** side effect
- **Scenario:** SC-005
- **Requirements:** FR-NEW-001, FR-NEW-010
- **Given** one converter exists and `proj` converts with the configuration file converter
- **When** Alice sends 3 refused creations (empty name, bad url, bad timeout)
- **Then** the list still holds one converter, the record of changes gained nothing and `proj` still converts as before
- **Priority:** Medium

#### E2E-099: the web interface pages like the tool
- **Category:** edge
- **Scenario:** SC-006
- **Requirements:** FR-NEW-018, FR-NEW-006
- **Given** 101 converters
- **When** Alice lists through the web interface and follows its marker
- **Then** she gets 100 then 1, identical to the tool's pages
- **Priority:** Medium

#### E2E-100: the screen renames a converter without changing its id
- **Category:** edge
- **Scenario:** SC-007
- **Requirements:** FR-NEW-019, FR-NEW-005
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) is the converter of `proj`; Alice is in a browser session
- **When** she renames it `Docling v2` on the screen
- **Then** the screen shows the new name and `proj` still shows it as its converter
- **Priority:** Medium

#### E2E-101: a stored secret is used by the conversion
- **Category:** requirement
- **Scenario:** SC-008
- **Requirements:** FR-NEW-003, FR-NEW-015
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) has the secret `Bearer tok-123`
- **When** Bob converts `/report.pdf`
- **Then** the conversion gives `# From B`, because the stub answers `# From B` only for the header `Authorization: Bearer tok-123` (otherwise it answers 401)
- **Priority:** High

#### E2E-102: only administrators read the record of changes
- **Category:** failure
- **Scenario:** SC-009
- **Requirements:** FR-NEW-016, FR-NEW-017
- **Given** changes exist; Bob is not a platform administrator
- **When** Bob lists the setting changes
- **Then** he is refused with `ERR_FORBIDDEN: 'bob@example.com' is not a platform admin`
- **Priority:** Medium

#### E2E-103: the record of changes survives a restart
- **Category:** edge
- **Scenario:** SC-009
- **Requirements:** FR-NEW-016
- **Given** 3 converter changes recorded
- **When** the platform restarts and Alice lists the changes
- **Then** the same 3 entries with the same ids appear
- **Priority:** Medium

#### E2E-104: a platform with only a registry default converts
- **Category:** side effect
- **Scenario:** SC-010
- **Requirements:** FR-NEW-020, FR-NEW-010
- **Given** an upgraded platform whose configuration file converter is disabled; Alice creates a converter and sets it as default
- **When** Bob converts
- **Then** the conversion works and the configuration file stays as it was
- **Priority:** Medium

#### E2E-105: an upgrade leaves the registry and the record empty
- **Category:** side effect
- **Scenario:** SC-010
- **Requirements:** FR-NEW-020, FR-NEW-016
- **Given** an existing platform upgrades
- **When** Alice lists converters and changes
- **Then** both are empty and nothing was written by the upgrade
- **Priority:** Medium

#### E2E-106: creating converters changes no conversion
- **Category:** side effect
- **Scenario:** SC-001
- **Requirements:** FR-NEW-001, FR-NEW-010
- **Given** `proj` converts with the configuration file converter
- **When** Alice creates two converters without choosing any
- **Then** Bob's next conversion still gives `# From file`
- **Priority:** Medium

#### E2E-107: the companion text and the report do not depend on the converter source
- **Category:** side effect
- **Scenario:** SC-004
- **Requirements:** FR-NEW-013
- **Given** the same stand in answer is given by a registry converter and by the configuration file converter
- **When** Bob converts one file with each
- **Then** the companion text and the conversion report are identical
- **Priority:** Medium

#### E2E-108: selecting a converter changes no file
- **Category:** side effect
- **Scenario:** SC-003
- **Requirements:** FR-NEW-009
- **Given** `proj` holds 3 files
- **When** Alice sets and clears a converter for `proj`
- **Then** the 3 files and their access times are unchanged
- **Priority:** Low

#### E2E-109: a refused change by a non administrator leaves no record
- **Category:** failure
- **Scenario:** SC-009
- **Requirements:** FR-NEW-016, FR-NEW-017
- **Given** Bob is not a platform administrator
- **When** Bob tries to create a converter, then Alice lists the changes
- **Then** the list holds no entry for Bob's attempt
- **Priority:** Medium

#### E2E-110: no converter anywhere after an upgrade
- **Category:** failure
- **Scenario:** SC-010
- **Requirements:** FR-NEW-020, FR-NEW-010
- **Given** an upgraded platform whose configuration file converter is disabled and an empty registry
- **When** Bob writes `/report.pdf` with the conversion option
- **Then** he gets `ERR_NOT_SUPPORTED: document service is not configured` and nothing is written
- **Priority:** Medium

#### E2E-111: an extension matches the end of the name after a dot
- **Category:** edge
- **Scenario:** SC-004
- **Requirements:** FR-NEW-012, FR-NEW-004
- **Given** the converter of `proj` has `extensions` `["txt"]`
- **When** Bob converts `/notxt` and `/Notes.TXT`
- **Then** `/notxt` is refused with `'/notxt' cannot be documented, the document service accepts: .txt`; `/Notes.TXT` is converted
- **Priority:** Medium

#### E2E-112: extensions are stored lower case without duplicates
- **Category:** edge
- **Scenario:** SC-005
- **Requirements:** FR-NEW-004
- **Given** no converter
- **When** Alice creates a converter with `extensions` `["pdf", "PDF", "Docx"]`
- **Then** the answer shows `["pdf", "docx"]`
- **Priority:** Medium

#### E2E-113: a document over the cap is refused before any write
- **Category:** failure
- **Scenario:** SC-004
- **Requirements:** FR-NEW-004, FR-NEW-013
- **Given** the converter of `proj` has `max_input_bytes` 1000
- **When** Bob writes a 1001 byte `/report.pdf` with the conversion option
- **Then** he is refused with `ERR_INVALID_ARGUMENT: '/report.pdf' is 1001 bytes, over the doc_service.max_input_bytes limit of 1000` and nothing is written
- **Priority:** High

#### E2E-114: a stored secret that can no longer be read
- **Category:** failure
- **Scenario:** SC-004
- **Requirements:** FR-NEW-010, FR-NEW-015
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) has a secret and is the converter of `proj`; the platform restarts with another master key
- **When** Bob writes `/report.pdf` with the conversion option
- **Then** he is refused with `ERR_INTERNAL_ERROR: the secret of converter 'Docling API' cannot be read` (his file is accepted by the extension and size checks), nothing is written, no other converter is tried and the text holds no secret
- **Priority:** High

#### E2E-115: the screen creates a program converter from text fields
- **Category:** requirement
- **Scenario:** SC-007
- **Requirements:** FR-NEW-019, FR-NEW-002
- **Given** Alice is in a browser session
- **When** she enters the name `Pandoc local`, form `program`, a command text area holding the lines `pandoc-convert`, a blank line and `{document}`, and submits with the token
- **Then** the converter is created with the command `["pandoc-convert", "{document}"]`
- **Priority:** High

#### E2E-116: an answer sent back as an update is accepted
- **Category:** edge
- **Scenario:** SC-002
- **Requirements:** FR-NEW-005, FR-NEW-001
- **Given** the converter named `Pandoc local` (form `program`, command `["pandoc-convert", "{document}"]`) exists
- **When** Alice updates it sending the whole answer she received (with `converter_id`, `form`, `has_secret`, `created_at`, `updated_at` and `url`, `auth_header`, `file_field`, `response_field` null)
- **Then** the update succeeds, changes nothing and leaves `updated_at` unchanged; a field of the other form given with a value (an empty text included) is refused with `ERR_INVALID_ARGUMENT: url is not valid for a program converter`
- **Priority:** Medium

#### E2E-117: a secret that straddles the quoted slice is still redacted
- **Category:** failure
- **Scenario:** SC-008
- **Requirements:** FR-MOD-001, FR-NEW-015
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) has the secret `tok-SECRET-123`; the stub answers 500 with a body that repeats the received header so that the secret starts before byte 2048 of the body and ends after it
- **When** Bob converts `/report.pdf`
- **Then** the failure text holds `[redacted]` and neither the secret nor any prefix of it longer than 3 characters
- **Priority:** High

#### E2E-118: an http converter never follows a redirect
- **Category:** failure
- **Scenario:** SC-008
- **Requirements:** FR-NEW-003, FR-MOD-002, FR-NEW-015
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) has `auth_header` `X-Api-Key` and the secret `abc`; the stub answers 302 to a second stub on another port
- **When** Bob converts `/report.pdf`
- **Then** the conversion fails with the converter's HTTP status reported; the second stub received no request and no secret
- **Priority:** High

#### E2E-119: each kind of change is recorded with its target
- **Category:** side effect
- **Scenario:** SC-009
- **Requirements:** FR-NEW-016, FR-NEW-008, FR-NEW-009
- **Given** `Pandoc local` exists and `proj` exists
- **When** Alice sets it as the default, clears the default, sets it for `proj` and clears it
- **Then** the four records show `set_default_converter`, `clear_default_converter`, `set_project_converter`, `clear_project_converter`, with `target_kind` `default_converter`, `default_converter`, `project_converter`, `project_converter`, `target_id` the converter id or the project id, `target_name` the converter name, and empty `fields`
- **Priority:** High

#### E2E-120: a clear of nothing is answered null and not recorded
- **Category:** edge
- **Scenario:** SC-003
- **Requirements:** FR-NEW-008, FR-NEW-009, FR-NEW-016
- **Given** no default and no converter for `proj`
- **When** Alice clears the default and clears the converter of `proj`
- **Then** each answers `converter` null and the record gained no entry
- **Priority:** Medium

#### E2E-121: a non administrator is refused before anything else
- **Category:** failure
- **Scenario:** SC-006
- **Requirements:** FR-NEW-017
- **Given** Bob is not a platform administrator
- **When** Bob deletes `conv-000000000000`, sets `conv-000000000000` for `ghost` and lists with the marker `garbage`
- **Then** each is refused with `ERR_FORBIDDEN: 'bob@example.com' is not a platform admin`, never with a not found or a marker text
- **Priority:** High

#### E2E-122: a default that is also used is refused as the default
- **Category:** failure
- **Scenario:** SC-002
- **Requirements:** FR-NEW-007
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) is the platform default and the converter of `proj`
- **When** Alice deletes it
- **Then** she is refused with `ERR_INVALID_ARGUMENT: converter 'Docling API' is the platform default`
- **Priority:** Medium

#### E2E-123: an unknown project is refused before an unknown converter
- **Category:** failure
- **Scenario:** SC-003
- **Requirements:** FR-NEW-009
- **Given** no project `ghost`
- **When** Alice sets `conv-000000000000` as the converter of `ghost`
- **Then** she is refused with `ERR_PROJECT_NOT_FOUND: project 'ghost' not found`
- **Priority:** Medium

#### E2E-124: a soft deleted project still uses its converter
- **Category:** edge
- **Scenario:** SC-002
- **Requirements:** FR-NEW-007, FR-NEW-021
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) is the converter of `proj`; `proj` is removed within its grace period
- **When** Alice deletes `Docling API`
- **Then** she is refused with `ERR_INVALID_ARGUMENT: converter 'Docling API' is used by 1 project(s)`
- **Priority:** Medium

#### E2E-125: a blank secret means no secret
- **Category:** edge
- **Scenario:** SC-005
- **Requirements:** FR-NEW-003, FR-NEW-005
- **Given** no converter
- **When** Alice creates an `http` converter with the secret `   `, then updates `Docling API` (which has a secret) with the secret `   `
- **Then** the creation stores no secret (`has_secret` false) and the update keeps the stored secret
- **Priority:** Medium

#### E2E-126: a header name that is not valid
- **Category:** failure
- **Scenario:** SC-005
- **Requirements:** FR-NEW-003
- **Given** no converter
- **When** Alice creates an `http` converter with `auth_header` `X Api Key` and the secret `abc`
- **Then** she is refused with `ERR_INVALID_ARGUMENT: auth_header is not a valid header name` and the text does not quote the name
- **Priority:** Medium

#### E2E-127: a secret that is not a valid header value
- **Category:** failure
- **Scenario:** SC-005
- **Requirements:** FR-NEW-003, FR-NEW-015
- **Given** no converter
- **When** Alice creates an `http` converter with a secret holding a line break
- **Then** she is refused with `ERR_INVALID_ARGUMENT: secret is not a valid header value` and the text does not quote the secret
- **Priority:** Medium

#### E2E-128: an address without a host or longer than 2048 characters
- **Category:** failure
- **Scenario:** SC-005
- **Requirements:** FR-NEW-003
- **Given** no converter
- **When** Alice creates an `http` converter with the url `http://`, then with a url of 2049 characters
- **Then** each is refused with `ERR_INVALID_ARGUMENT: url must be an http or https URL` and the text does not quote the url
- **Priority:** Medium

#### E2E-129: batch upload resolves the converter per file
- **Category:** edge
- **Scenario:** SC-004
- **Requirements:** FR-NEW-010, FR-NEW-011
- **Given** Bob uploads 2 files with the conversion option and Alice changes the default between the two conversions
- **When** the upload runs
- **Then** each file is converted by the converter chosen when its own conversion starts
- **Priority:** Low

#### E2E-130: a change between two files of a batch is applied per file
- **Category:** edge
- **Scenario:** SC-004
- **Requirements:** FR-NEW-010, FR-NEW-012
- **Given** Bob uploads `/a.pdf` and `/b.pdf` with the conversion option; the batch check passes, then Alice sets as default a converter that accepts only `txt` before the second conversion starts
- **When** the upload runs
- **Then** `/a.pdf` is converted and stays; the upload stops with `'/b.pdf' cannot be documented, the document service accepts: .txt` and `/b.pdf` is not written, as for any refusal during a batch today
- **Priority:** Medium

#### E2E-131: an extension refusal comes before an unreadable secret
- **Category:** failure
- **Scenario:** SC-004
- **Requirements:** FR-NEW-010, FR-NEW-012
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) has a secret that can no longer be read and accepts `pdf` only; it is the converter of `proj`
- **When** Bob writes `/c.xyz` with the conversion option
- **Then** he is refused with `'/c.xyz' cannot be documented, the document service accepts: .pdf`, not with the secret text
- **Priority:** Medium

#### E2E-132: a size refusal comes before an unreadable secret
- **Category:** failure
- **Scenario:** SC-004
- **Requirements:** FR-NEW-010, FR-NEW-004
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) has `max_input_bytes` 1000 and a secret that can no longer be read; it is the converter of `proj`
- **When** Bob writes a 1001 byte `/report.pdf` with the conversion option
- **Then** he is refused with `ERR_INVALID_ARGUMENT: '/report.pdf' is 1001 bytes, over the doc_service.max_input_bytes limit of 1000`
- **Priority:** Medium

#### E2E-133: an update that names another converter in its body
- **Category:** failure
- **Scenario:** SC-002
- **Requirements:** FR-NEW-005
- **Given** the converter named `Pandoc local` (form `program`, command `["pandoc-convert", "{document}"]`) exists and so does `Docling API`
- **When** Alice updates `Pandoc local` with a body whose `converter_id` is the id of `Docling API`
- **Then** she is refused with `ERR_INVALID_ARGUMENT: converter_id in the body does not match` and nothing changes
- **Priority:** Medium

#### E2E-134: a rename that changes only the letter case
- **Category:** edge
- **Scenario:** SC-002
- **Requirements:** FR-NEW-005, FR-NEW-001
- **Given** the converter named `Pandoc local` (form `program`, command `["pandoc-convert", "{document}"]`) exists
- **When** Alice renames it `PANDOC LOCAL`, then `PANDOC LOCAL` again
- **Then** the first is accepted, the second changes nothing; renaming it to the name of another converter is refused with `a converter named '<name>' already exists`
- **Priority:** Medium

#### E2E-135: the configuration file converter does not follow a redirect either
- **Category:** failure
- **Scenario:** SC-008
- **Requirements:** FR-MOD-002, FR-NEW-015
- **Given** the configuration file converter is an http converter with the header `X-Api-Key` and the token `cfg-TOKEN-456`; its stub answers 302 to a second stub
- **When** Bob converts `/report.pdf`
- **Then** the conversion fails with the HTTP status reported; the second stub received no request and no token
- **Priority:** High

#### E2E-136: a project in its grace period can still be given a converter
- **Category:** edge
- **Scenario:** SC-003
- **Requirements:** FR-NEW-009, FR-NEW-021
- **Given** `proj` is removed within its grace period
- **When** Alice sets, reads and clears a converter for `proj`
- **Then** each succeeds; for a project removed for good each is refused with `ERR_PROJECT_NOT_FOUND: project 'proj' not found`
- **Priority:** Medium

#### E2E-137: clearing the secret of a converter that has none
- **Category:** edge
- **Scenario:** SC-002
- **Requirements:** FR-NEW-005
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) exists without a secret
- **When** Alice updates it with `clear_secret` true, then with `secret` `   ` and `clear_secret` true
- **Then** both succeed and change nothing
- **Priority:** Low

#### E2E-138: clearing the secret of a program converter
- **Category:** failure
- **Scenario:** SC-005
- **Requirements:** FR-NEW-005, FR-NEW-002
- **Given** the converter named `Pandoc local` (form `program`, command `["pandoc-convert", "{document}"]`) exists
- **When** Alice updates it with `clear_secret` true
- **Then** she is refused with `ERR_INVALID_ARGUMENT: clear_secret is not valid for a program converter`
- **Priority:** Low

#### E2E-139: the screen clears a secret
- **Category:** edge
- **Scenario:** SC-007
- **Requirements:** FR-NEW-019, FR-NEW-005
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) has a secret; Alice is in a browser session
- **When** she ticks `Clear the secret` on its edit form and saves
- **Then** the screen shows `No secret` and the listing shows `has_secret` false
- **Priority:** Medium

#### E2E-140: the screen shows the 20 latest converter changes
- **Category:** side effect
- **Scenario:** SC-007
- **Requirements:** FR-NEW-019, FR-NEW-016
- **Given** 21 converter changes and 3 model registry changes were made, the model changes being the newest
- **When** Alice opens the screen
- **Then** it shows the 20 most recent converter changes, newest first, the oldest one is missing, and no model registry change appears
- **Priority:** Low

#### E2E-141: a redirect to the same host is not followed
- **Category:** failure
- **Scenario:** SC-008
- **Requirements:** FR-NEW-003, FR-MOD-002
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) exists and its stub answers 307 with a relative `Location`
- **When** Bob converts `/report.pdf`
- **Then** the conversion fails with the HTTP status reported and the stub received exactly one request
- **Priority:** Medium

#### E2E-142: the screen empties the response field and the extensions
- **Category:** edge
- **Scenario:** SC-007
- **Requirements:** FR-NEW-019, FR-NEW-005
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) has `response_field` `markdown` and `extensions` `["pdf"]`; Alice is in a browser session
- **When** she blanks both fields on the edit form and saves
- **Then** the converter now has an empty `response_field` and an empty `extensions` list, and its url is unchanged
- **Priority:** Medium

#### E2E-143: an empty url
- **Category:** failure
- **Scenario:** SC-005
- **Requirements:** FR-NEW-003
- **Given** no converter
- **When** Alice creates an `http` converter with the url `` (empty text)
- **Then** she is refused with `ERR_INVALID_ARGUMENT: url is required for an http converter`
- **Priority:** Medium

#### E2E-144: a blank header name without a secret
- **Category:** edge
- **Scenario:** SC-005
- **Requirements:** FR-NEW-003
- **Given** no converter
- **When** Alice creates an `http` converter with a valid url, no secret and `auth_header` `  `
- **Then** it is created with `auth_header` `Authorization`; with a secret and the same blank header she is refused with `ERR_INVALID_ARGUMENT: auth_header is required when a secret is set`
- **Priority:** Medium

#### E2E-145: numbers that are not whole
- **Category:** failure
- **Scenario:** SC-005
- **Requirements:** FR-NEW-004
- **Given** no converter
- **When** Alice creates a valid program converter with `timeout_seconds` 60.5, then "60", then true
- **Then** each is refused with `ERR_INVALID_ARGUMENT: timeout_seconds must be between 1 and 86400`
- **Priority:** Low

#### E2E-146: a text field given as a number
- **Category:** failure
- **Scenario:** SC-005
- **Requirements:** FR-NEW-001, FR-NEW-004
- **Given** no converter
- **When** Alice creates a converter with the name 42, and an `http` converter with the url 42
- **Then** each is refused with `ERR_INVALID_ARGUMENT: name must be text` and `ERR_INVALID_ARGUMENT: url must be text`
- **Priority:** Low

#### E2E-147: clear_secret given as a text
- **Category:** failure
- **Scenario:** SC-005
- **Requirements:** FR-NEW-005
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) exists
- **When** Alice updates it with `clear_secret` "yes"
- **Then** she is refused with `ERR_INVALID_ARGUMENT: clear_secret must be true or false`
- **Priority:** Low

#### E2E-148: the converter disappears between two files of a batch
- **Category:** failure
- **Scenario:** SC-004
- **Requirements:** FR-NEW-010, FR-NEW-012
- **Given** Bob uploads `/a.pdf` and `/b.pdf` with the conversion option; after the batch check Alice clears the only converter before the second conversion starts
- **When** the upload runs
- **Then** `/a.pdf` is converted and stays; the upload stops with `ERR_NOT_SUPPORTED: document service is not configured` and `/b.pdf` is not written
- **Priority:** Low

#### E2E-149: a rename is recorded with the new name
- **Category:** side effect
- **Scenario:** SC-009
- **Requirements:** FR-NEW-016, FR-NEW-005
- **Given** the converter named `Pandoc local` (form `program`, command `["pandoc-convert", "{document}"]`) exists
- **When** Alice renames it `Pandoc 3`
- **Then** the record shows `update_converter`, `target_kind` `converter`, the converter id, `target_name` `Pandoc 3` and `fields` `["name"]`
- **Priority:** Medium

#### E2E-150: an update that touches no secret does not need the master key
- **Category:** edge
- **Scenario:** SC-008
- **Requirements:** FR-NEW-005, FR-NEW-003
- **Given** the converter named `Docling API` (form `http`, url `http://localhost:8099/convert`) has a secret; the platform restarts without a master key
- **When** Alice renames it, then updates it with `clear_secret` true
- **Then** both succeed (the secret is removed by the second); giving a new secret is refused with `ERR_NOT_SUPPORTED: no master key is configured; set FSA_TOKEN_KEY`
- **Priority:** Low

### 9.3 Exploit tests
E2E-092 (class A, CWE-78), E2E-093 and E2E-094 (class B, CWE-522 and CWE-532) are the exploit tests of the class sweep of 14.2; they carry their literal payloads above, assert the effect through another channel (a recorded argument list and the absence of a file; the server log, the stored form and every answer) and are red for named reasons: E2E-093 is red on the current code for the configuration file token, whose raw echo appears in the failure text (`service.rs:611-615`); E2E-092 and E2E-067 are red against an implementation that builds `sh -c` with the joined arguments (the recorded argument list differs and the file is created); E2E-094 is red against an implementation that stores the secret in clear. Each starts with the file `/tmp/fsa-pwned` absent.

### 9.4 Modified tests
#### E2E-MOD-001: a failed conversion quotes the converter answer without the secret (was an existing test of a converter failure reported in the answer; /sdlc-design names it)
- **Validated before:** a failed conversion shows the converter's own error text is reported with the text the converter returned.
- **Validates now:** the same, with every occurrence of the secret replaced by `[redacted]`.
- **Scenario:** SC-008
- **Requirements:** FR-MOD-001
- **Given** a converter holding the secret `tok-SECRET-123` whose error text repeats it
- **When** Bob converts a file and the converter fails
- **Then** the failure shown to Bob holds `[redacted]` and not `tok-SECRET-123`

### 9.5 Removed tests
None.

## 10. Traceability
| Scenario | Requirements | Happy | Failure | Edge |
|---|---|---|---|---|
| SC-001 | FR-NEW-001, FR-NEW-002, FR-NEW-003, FR-NEW-004, FR-NEW-006, FR-NEW-010 | E2E-001, E2E-002, E2E-003 | E2E-019, E2E-049 | E2E-073, E2E-106 |
| SC-002 | FR-NEW-001, FR-NEW-004, FR-NEW-005, FR-NEW-007, FR-NEW-008, FR-NEW-009, FR-NEW-021 | E2E-005, E2E-008 | E2E-043, E2E-044, E2E-045, E2E-122, E2E-133 | E2E-070, E2E-083, E2E-091, E2E-116, E2E-124, E2E-134, E2E-137 |
| SC-003 | FR-NEW-007, FR-NEW-008, FR-NEW-009, FR-NEW-010, FR-NEW-016, FR-NEW-017, FR-NEW-021 | E2E-004, E2E-006, E2E-007 | E2E-046, E2E-047, E2E-048, E2E-123 | E2E-069, E2E-071, E2E-072, E2E-074, E2E-079, E2E-096, E2E-108, E2E-120, E2E-136 |
| SC-004 | FR-NEW-001, FR-NEW-003, FR-NEW-004, FR-NEW-007, FR-NEW-008, FR-NEW-009, FR-NEW-010, FR-NEW-011, FR-NEW-012, FR-NEW-013, FR-NEW-015 | E2E-009, E2E-010, E2E-012, E2E-016 | E2E-054, E2E-055, E2E-056, E2E-057, E2E-060, E2E-113, E2E-114, E2E-131, E2E-132, E2E-148 | E2E-064, E2E-065, E2E-068, E2E-075, E2E-076, E2E-095, E2E-107, E2E-111, E2E-129, E2E-130 |
| SC-005 | FR-NEW-001, FR-NEW-002, FR-NEW-003, FR-NEW-004, FR-NEW-005, FR-NEW-010, FR-NEW-014, FR-NEW-015 | E2E-097 | E2E-017, E2E-018, E2E-020, E2E-021, E2E-022, E2E-023, E2E-024, E2E-025, E2E-026, E2E-027, E2E-028, E2E-029, E2E-030, E2E-031, E2E-032, E2E-033, E2E-034, E2E-035, E2E-036, E2E-037, E2E-038, E2E-039, E2E-040, E2E-041, E2E-042, E2E-126, E2E-127, E2E-128, E2E-138, E2E-143, E2E-145, E2E-146, E2E-147 | E2E-061, E2E-062, E2E-063, E2E-098, E2E-112, E2E-125, E2E-144 |
| SC-006 | FR-NEW-006, FR-NEW-017, FR-NEW-018, FR-NEW-019 | E2E-013 | E2E-050, E2E-051, E2E-058, E2E-121 | E2E-090, E2E-099 |
| SC-007 | FR-NEW-002, FR-NEW-005, FR-NEW-015, FR-NEW-016, FR-NEW-017, FR-NEW-019 | E2E-014, E2E-115 | E2E-052, E2E-053 | E2E-088, E2E-089, E2E-100, E2E-139, E2E-140, E2E-142 |
| SC-008 | FR-MOD-001, FR-MOD-002, FR-NEW-002, FR-NEW-003, FR-NEW-005, FR-NEW-014, FR-NEW-015, FR-NEW-016 | E2E-101 | E2E-059, E2E-092, E2E-093, E2E-094, E2E-117, E2E-118, E2E-135, E2E-141, E2E-MOD-001 | E2E-066, E2E-067, E2E-077, E2E-078, E2E-087, E2E-150 |
| SC-009 | FR-NEW-005, FR-NEW-008, FR-NEW-009, FR-NEW-015, FR-NEW-016, FR-NEW-017 | E2E-015 | E2E-102, E2E-109 | E2E-084, E2E-085, E2E-086, E2E-103, E2E-119, E2E-149 |
| SC-010 | FR-NEW-010, FR-NEW-016, FR-NEW-020 | E2E-011 | E2E-082, E2E-110 | E2E-080, E2E-081, E2E-104, E2E-105 |

## 11. Open questions
None.

Decision recorded: the latent hits B4 and B5 of the class sweep (the command line agent) are outside this lot and deferred to proposal P1, not written to the backlog; the owner's default is accepted unless said otherwise.

## 12. Glossary (MANDATORY)
| Term | Definition | Context |
|---|---|---|
| Converter | a named way to turn a document into Markdown: a program or an http service | Settings |
| Form | `program` (a command that takes the file) or `http` (an address that receives it) | Settings |
| Platform default | the converter used by projects that have none of their own | Settings |
| Converter of a project | the converter an administrator chose for one project | Settings |
| Configuration file converter | the converter defined in the platform's configuration file, used last | Documents |
| Secret | the text sent as the value of a header to an http converter, stored encrypted and never shown | Settings |
| Master key | the existing key that encrypts stored secrets | Settings |
| Conversion option | the option of a write or an upload that asks for conversion | Documents |
| Record of changes | the list of who changed what and when, without values | Settings |
| One time token | the token a screen form needs to be accepted | Settings |
| Web interface | the second way in to the same operations, beside the tools | Settings |

## 13. Implementability gate (MANDATORY)
| Round | F | A | Verdict |
|---|---|---|---|
| 1 | 12 | 7 | NOT-IMPLEMENTABLE |
| 2 | 6 | 4 | NOT-IMPLEMENTABLE |
| 3 | 8 | 5 | NOT-IMPLEMENTABLE |
| 4 | 0 | 0 | IMPLEMENTABLE |

**Amendments applied:** extension format and matching; the cap on input and answer; unreadable secret; screen input, field names, redirects and listing; redaction before the cut; no redirect for any http converter (FR-MOD-002); record fields; refusal order on every interface; update semantics; blank and wrong typed values; batch behavior as today; nothing written on a refusal; sweep rows A4 to A7 and B6 to B10; 14.3 inputs. The fourth round was authorized by the owner after the three round cap. **No implementation detail outside Section 14 (except the fixed public names and verbatim texts):** PASS

## 14. Evidence appendix (for /sdlc-design and the auditor, not for the product owner)
### 14.1 Current State evidence
| # | Claim (Section 2) | Evidence |
|---|---|---|
| 1 | the converter is a block of the configuration file with two forms and defaults | `crates/core/src/config.rs:1087-1098,1037-1082,1012-1025` |
| 2 | it is checked at start only (placeholders, url, header, field) | `crates/core/src/config.rs:1181,1317-1380` |
| 3 | it is built once and held in the state | `crates/core/src/app.rs:126,173`; `crates/core/src/state.rs:25` |
| 4 | the conversion entry points (write, upload, documentize, each as tool and as web route) and the existing refusals | `crates/core/src/tools/write.rs:136-148`; `crates/core/src/api/dataplane.rs:669-708,1631,1663`; `crates/core/src/tools/document.rs:121`; `crates/core/src/mcp/server.rs:2050,2968`; `crates/core/src/core/fs_ops.rs:638-699,746-760` |
| 5 | the program form uses an argument list in a temporary folder with no shell; the http form sends a header; both capped | `crates/core/src/docs/service.rs:361-440,538-650`; `config.rs:1037` (comment: argv list, never a shell string) |
| 6 | no per project converter and no admin interface for it | `crates/core/src/storage/traits.rs:155-174` (project fields), `crates/core/src/tools/admin.rs:991-1002` (the registered admin tools) |
| 7 | existing tests of the converter | `crates/core/src/docs/service.rs` tests, `docs/manifest_e2e.rs`, `tools/converter_bound_e2e.rs` |
| 8 | the pattern: registry in the admin store, encrypted secrets, platform admin only, change record, web routes, screen | `crates/core/src/settings/{registry,store,secrets,model}.rs`, `tools/models.rs`, `api/admin_settings.rs`, `models_screen.rs` |
| 9 | the failure text quotes a slice of the converter's answer | `crates/core/src/docs/service.rs:611-615` |
Test command: `cargo test -p fsa-core --lib -- docs:: e2e_0089_ e2e_0102_ tools::write tools::document`

### 14.2 Class sweep
**Class A:** an administrator supplied list of arguments or program text reaching a program launch or a shell (CWE-78).
**Searches run:** (1) `rg -n 'Command::new|tokio::process|std::process|"sh"|"bash"|-c"|\{document\}|\{outdir\}' --glob '*.rs' --glob '*.toml' .` (spawn sites, shell strings, placeholders); (2) `rg -n 'Command::new|process::Command|\.spawn\(|pandoc_bin|Command\b' crates/*/src` (spawn sites and configuration fed binaries); (3) a read of the argument expansion and the validator. Not searched: native `exec` calls (`unsafe` is forbidden workspace wide).

| # | Occurrence | Verdict | Disposition |
|---|---|---|---|
| A1 | `crates/core/src/docs/service.rs:404` (spawn of the converter with an argument list) | safe today: argument list, no shell wrapper, `{document}` becomes `./<name>`, plain replacement inside one argument (`service.rs:683`), validator `config.rs:1331-1343`; the new source (the registry) reaches this same sink | FR-NEW-002, FR-NEW-014; E2E-092 |
| A2 | `crates/core/src/tools/doc.rs:296` (pandoc with fixed arguments and generated paths) | safe: no administrator text reaches it | none |
| A3 | `crates/core/src/git/test_remote.rs:237`, `tools/converter_bound_e2e.rs:131`, `tools/nesting_testkit.rs:47`, `settings/e2e.rs:672`, `storage/blob/s3.rs:300-417` | safe: test code or fixed arguments | none |
| A6 | `crates/core/src/docs/service.rs:656-683` (`sanitize_file_name` restricts a document name to letters, digits, dot, dash and underscore before `expand_argv` replaces `{document}` then `{outdir}`; no placeholder or brace can be smuggled in) | safe | none |
| A7 | `crates/core/src/docs/service.rs:406-412` (the child keeps the platform environment, the master key variable included, and its stderr is quoted back, `service.rs:497`) | accepted: an administrator who can set a command can already run any program | proposal P2, not written; owner decision (default: accepted) |
| A4 | `crates/core/src/tools/editor.rs:595,597` (`open` and `xdg-open` with a local url) | safe: fixed program name, argument list, no administrator text | none |
| A5 | `crates/core/tests/git_negotiation_e2e.rs:227`, `crates/core/tests/full_stack_e2e.rs:300` (`git -c http.extraHeader=Authorization: Bearer` in tests) | safe: test code | none |

**Class B:** a credential (token, key, secret, header value) reaching a place where it can be read back: a log line, an error text, an answer, a listing, a record, a screen (CWE-522, CWE-532).
**Searches run:** (0) `rg -n -i 'redirect' crates/core/src` (redirect handling); (1) `rg -n 'auth_token|secret_enc|expose\(|Secret|derive\(.*Debug' -g'*.rs' .`; (2) `rg -n -i 'tracing::\w+!.*(token|secret|key|auth|password|dsn|url|header)' crates/*/src`; (3) `rg -n 'Authorization|Bearer|bearer_auth|\.header\(' --glob '*.rs' crates/core/src crates/agent/src`; (4) `format!` with `url` in `search` and `git`. Not searched: request logging layers (none present in the dependencies).

| # | Occurrence | Verdict | Disposition |
|---|---|---|---|
| B1 | `crates/core/src/docs/service.rs:599` (failure text quotes a slice of the converter's answer, which can repeat a request header) | vulnerable when a converter echoes the header it received | fixed in this lot: FR-MOD-001, FR-NEW-015; E2E-093 |
| B2 | `crates/core/src/config.rs:71-100` (`Secret`), `docs/service.rs:576` (single read point), `settings/secrets.rs:13,35`, `settings/model.rs:15`, `settings/store.rs:19` | safe: redacting `Debug`, single read point, answers carry only `has_secret` | new converter rows follow the same rules: FR-NEW-015; E2E-087, E2E-094 |
| B3 | `crates/core/src/search/embedding.rs:75,84`, `search/rerank.rs:67,80` (errors use `without_url`), `git/provider/mod.rs:221,389,537` | safe | none |
| B4 | `crates/agent/src/config.rs:11-24,36-47` (`token` and `api_key` as plain text in a `Debug` derive; no call site prints them) | latent, not exploitable today, in the command line agent | outside this lot: proposal P1, not written; deferral needs the owner's decision (default: deferred) |
| B5 | `crates/agent/src/mcp.rs:125,171` (error quotes the configured url), `agent/src/llm.rs:146` (quotes an endpoint answer body like B1 did) | latent, same agent | proposal P1 with B4 |
| B9 | `crates/core/src/tools/web_access.rs:183`, `git/provider/mod.rs:391` (redirects disabled) | safe | none |
| B10 | `crates/core/src/search/embedding.rs:66`, `search/rerank.rs:55`, `docs/ocr.rs:99` (an `Authorization: Bearer` sent by clients that follow redirects; reqwest drops `Authorization` across hosts) | safe: only `Authorization`, dropped on a cross host redirect; the custom header case exists only for the document converter (B6) | none |
| B7 | `crates/core/src/docs/service.rs:362,553` (`tracing::instrument` with `skip(self, bytes)`) | safe: secrets are not traced | none |
| B8 | an administrator chosen `url` that points at an internal address (server side request) | accepted: part of the power to run a program on the server (7.2) | none |
| B6 | `crates/core/src/docs/service.rs:542-546` (the http client follows redirects by default; a custom header is forwarded to another host) | vulnerable (registry converter with a custom header name, and the configuration file converter) | fixed in this lot: FR-MOD-002; E2E-118, E2E-135, E2E-141 |

### 14.3 Design inputs
- The converter is built once at boot and held in the state (`app.rs:126,173`, `state.rs:25`); the three callers read it from there (`tools/write.rs:136`, `api/dataplane.rs:690-708`, `tools/document.rs:121`, `core/fs_ops.rs:638-760`). A call time resolver is needed (project, default, configuration file), as the search embedding targets do (`settings/registry.rs:716-741`). Signal: E2E-009, E2E-012, E2E-075.
- The http converter builds its client with the timeout at construction (`docs/service.rs:542-546`) and the program form reads the timeout from the configuration; per converter values need a service built per call or a cache by converter version. Signal: E2E-062, E2E-056.
- Converters, the platform default and the project selection are three new persisted concepts in the global (admin) store; the project selection needs a column or a table next to `Project` (`storage/traits.rs:155-163`). Signal: E2E-007, E2E-071, E2E-072.
- The secret needs the cipher and the master key of the settings registry (`settings/secrets.rs`, `git/oauth/cipher`). Signal: E2E-038, E2E-094.
- The record of changes needs the new `action` and `target_kind` values (`settings/model.rs:72-81`, `settings/store.rs:14-15`) and `admin.list_setting_changes` must list them. Signal: E2E-015, E2E-084.
- The validation texts for the program and http forms exist for the configuration file (`config.rs:1322-1380`) with other wording; save time validation uses the texts of this spec. Signal: E2E-021 to E2E-032.
- The program form does not clear the environment of the child (`docs/service.rs:406-412`), so a converter program inherits the server's environment, master key variable included. An administrator who can set a command can already run anything, so this is not a new exposure; clearing it would change the configuration file converter too (not decided here). Signal: none; reported as proposal P2.
- The secret redaction applies where the failure text is built (`docs/service.rs:611-615` and the stderr and stdout quoting at `service.rs:58,497`). Signal: E2E-093, E2E-059.
- The conversion callers also read the service from the state in the tool handlers (`mcp/server.rs:2050,2968`); the nine new `admin.*` tools need `#[tool]` wrappers like `admin_create_provider` (`mcp/server.rs:3409-3417`); the tool list and count (`tools/admin.rs:991-1002`, `mcp/server.rs:4544`, `tools/contract_golden.rs:324,465`) change. Signal: E2E-075 (repeated through the web routes `dataplane.rs:1631,1663`) and the tool count tests.
- The mount id equals the project id (`tools/mod.rs:175-180`, `state.rs:49-55`), so the resolver takes the mount id.
- Hard removal of a project (`storage/admin.rs:399,451`) must drop the selection in the same transaction; the admin schema changes in both stores with a column migration (`storage/admin.rs:1130`). Signal: E2E-071, E2E-096, E2E-105.
- The http client follows redirects by default (`docs/service.rs:542-546`); a registry converter must not. Signal: E2E-118.
- The extension list is stored with a leading dot today (`docs/service.rs:177-190`, built in list at `44-50`, matching `143-151`); the registry stores names without a dot. Signal: E2E-054, E2E-064, E2E-111.
- The input size gate is `fs_ops.rs:763-770`; the order of checks (extension, size, before the write) is `fs_ops.rs:641-645`. Signal: E2E-113.
- The unreadable secret text follows the registry (`settings/registry.rs:704-708`). Signal: E2E-114.
- The multipart upload validates the batch with one service (converter and extension only; the size gate is per file, `core/fs_ops.rs:763`, as today) before any write (`api/dataplane.rs:690-698`) and the engine calls take one `Option<&dyn DocService>` (`core/fs_ops.rs:638,746-760`); both change to a per file resolver result. Signal: E2E-129, E2E-130.
- The success statuses of the new web routes follow the existing admin routes (`api/admin_settings.rs:64-70`); asserted in E2E-013. The total tool count test is `mcp/server.rs:4562` with `tools/contract_golden.rs:324`; the tool count tests are technical signals, not acceptance tests.
- Soft deleted projects are found by `deleted_at` (`storage/admin.rs:321-329,671`); the selection must be kept and counted for them. Signal: E2E-124, E2E-136.
- New tools and routes raise the tool count and change the route tables and contract files (`tools/mod.rs`, `api/dataplane.rs:45`, `api/openapi.rs`, `TOOL_CONTRACT.txt`, `tool-contract-golden.json`). Signal: the tool count and golden tests.
- A screen follows `models_screen.rs`/`token_screen.rs`; the one time token store is private to each screen router. Signal: E2E-053.
