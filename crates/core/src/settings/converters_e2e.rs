//! SPEC-0106 functional tests of the converter registry, driven through the real
//! `McpServer` admin handlers on the SPEC-0102 fixture [`Fx`]: Alice is the
//! platform administrator, Bob an ordinary member who owns `proj`.

use super::e2e::Fx;
use crate::mcp::server::{
    CreateConverterArgs, ListConvertersArgs, ListDirArgs, ProjectIdArgs, SetDefaultConverterArgs,
    SetProjectConverterArgs, UpdateConverterArgs,
};
use crate::search::derived_e2e::{args, tool_json};
use serde_json::{Value, json};

const ALICE: &str = "admin@example.com";
const BOB: &str = "bob@example.com";
const FORBIDDEN_BOB: &str = "ERR_FORBIDDEN: 'bob@example.com' is not a platform admin";

impl Fx {
    pub async fn create_converter_as(&self, who: &str, a: Value) -> Result<Value, String> {
        let p: rmcp::handler::server::wrapper::Parameters<CreateConverterArgs> = args(a);
        let out = tool_json(self.server(who).admin_create_converter(p).await);
        self.keep(out)
    }

    pub async fn create_converter(&self, a: Value) -> Result<Value, String> {
        self.create_converter_as(ALICE, a).await
    }

    /// A program converter of that name with a valid command.
    pub async fn program(&self, name: &str) -> Value {
        self.create_converter(
            json!({"name": name, "form": "program", "command": ["pandoc-convert", "{document}"]}),
        )
        .await
        .unwrap()
    }

    pub async fn update_converter_as(&self, who: &str, a: Value) -> Result<Value, String> {
        let p: rmcp::handler::server::wrapper::Parameters<UpdateConverterArgs> = args(a);
        let out = tool_json(self.server(who).admin_update_converter(p).await);
        self.keep(out)
    }

    /// `fields` given to the converter `id`, as Alice.
    pub async fn update_converter(&self, id: &str, fields: Value) -> Result<Value, String> {
        let mut a = fields;
        a["converter_id"] = json!(id);
        self.update_converter_as(ALICE, a).await
    }

    pub async fn list_converters_as(&self, who: &str, marker: Value) -> Result<Value, String> {
        let p: rmcp::handler::server::wrapper::Parameters<ListConvertersArgs> =
            args(json!({"marker": marker}));
        let out = tool_json(self.server(who).admin_list_converters(p).await);
        self.keep(out)
    }

    pub async fn set_default_converter_as(&self, who: &str, id: &str) -> Result<Value, String> {
        let p: rmcp::handler::server::wrapper::Parameters<SetDefaultConverterArgs> =
            args(json!({"converter_id": id}));
        let out = tool_json(self.server(who).admin_set_default_converter(p).await);
        self.keep(out)
    }

    pub async fn clear_default_converter_as(&self, who: &str) -> Result<Value, String> {
        let out = tool_json(self.server(who).admin_clear_default_converter().await);
        self.keep(out)
    }

    pub async fn get_default_converter_as(&self, who: &str) -> Result<Value, String> {
        let out = tool_json(self.server(who).admin_get_default_converter().await);
        self.keep(out)
    }

    pub async fn set_project_converter_as(
        &self,
        who: &str,
        project: &str,
        id: &str,
    ) -> Result<Value, String> {
        let p: rmcp::handler::server::wrapper::Parameters<SetProjectConverterArgs> =
            args(json!({"project_id": project, "converter_id": id}));
        let out = tool_json(self.server(who).admin_set_project_converter(p).await);
        self.keep(out)
    }

    pub async fn clear_project_converter_as(
        &self,
        who: &str,
        project: &str,
    ) -> Result<Value, String> {
        let p: rmcp::handler::server::wrapper::Parameters<ProjectIdArgs> =
            args(json!({"project_id": project}));
        let out = tool_json(self.server(who).admin_clear_project_converter(p).await);
        self.keep(out)
    }

    pub async fn get_project_converter_as(
        &self,
        who: &str,
        project: &str,
    ) -> Result<Value, String> {
        let p: rmcp::handler::server::wrapper::Parameters<ProjectIdArgs> =
            args(json!({"project_id": project}));
        let out = tool_json(self.server(who).admin_get_project_converter(p).await);
        self.keep(out)
    }

    pub async fn delete_converter_as(&self, who: &str, id: &str) -> Result<Value, String> {
        let p: rmcp::handler::server::wrapper::Parameters<SetDefaultConverterArgs> =
            args(json!({"converter_id": id}));
        let out = tool_json(self.server(who).admin_delete_converter(p).await);
        self.keep(out)
    }

    /// Every converter, all pages walked.
    pub async fn converters(&self) -> Vec<Value> {
        let mut out = Vec::new();
        let mut marker = Value::Null;
        loop {
            let page = self.list_converters_as(ALICE, marker).await.unwrap();
            out.extend(page["converters"].as_array().unwrap().iter().cloned());
            match page["marker"].as_str() {
                Some(m) => marker = json!(m),
                None => return out,
            }
        }
    }
}

fn names(list: &[Value]) -> Vec<&str> {
    list.iter().map(|c| c["name"].as_str().unwrap()).collect()
}

/// A program creation with `command` and the extra fields of `extra`.
fn program_with(name: &str, command: Value, extra: Value) -> Value {
    let mut a = json!({"name": name, "form": "program", "command": command});
    for (k, v) in extra.as_object().unwrap() {
        a[k] = v.clone();
    }
    a
}

async fn refused(fx: &Fx, a: Value) -> String {
    fx.create_converter(a).await.unwrap_err()
}

#[tokio::test]
async fn e2e_0106_001_create_a_program_converter() {
    let fx = Fx::new().await;
    let cmd = json!(["pandoc-convert", "--out", "{outdir}", "{document}"]);
    let c =
        fx.create_converter(program_with("Pandoc local", cmd.clone(), json!({}))).await.unwrap();
    let id = c["converter_id"].as_str().unwrap();
    assert!(id.starts_with("conv-") && id.len() == 17, "{c}");
    assert!(id[5..].chars().all(|ch| ch.is_ascii_hexdigit()), "{c}");
    assert_eq!(c["name"], "Pandoc local");
    assert_eq!(c["form"], "program");
    assert_eq!(c["command"], cmd);
    assert_eq!(c["timeout_seconds"], 900);
    assert_eq!(c["max_input_bytes"], 536870912);
    assert_eq!(c["extensions"], json!([]));
    assert_eq!(c["has_secret"], false);
    for field in ["url", "auth_header", "file_field", "response_field"] {
        assert_eq!(c[field], Value::Null, "{field} does not belong to a program converter");
    }
    assert_eq!(c["created_at"], c["updated_at"]);
    assert!(chrono::DateTime::parse_from_rfc3339(c["created_at"].as_str().unwrap()).is_ok());
    let keys: Vec<&str> = c.as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        [
            "converter_id",
            "name",
            "form",
            "command",
            "url",
            "auth_header",
            "has_secret",
            "file_field",
            "response_field",
            "extensions",
            "max_input_bytes",
            "timeout_seconds",
            "created_at",
            "updated_at"
        ]
    );
    let all = fx.converters().await;
    assert_eq!(all, vec![c.clone()]);
    // FR-NEW-016: the creation is recorded with no field and no value.
    let changes = fx.changes().await;
    assert_eq!(changes.len(), 1, "{changes:?}");
    assert_eq!(changes[0]["action"], "create_converter");
    assert_eq!(changes[0]["target_kind"], "converter");
    assert_eq!(changes[0]["target_id"], c["converter_id"]);
    assert_eq!(changes[0]["target_name"], "Pandoc local");
    assert_eq!(changes[0]["fields"], json!([]));
    assert_eq!(changes[0]["person"], ALICE);
}

#[tokio::test]
async fn e2e_0106_003_converters_are_listed_by_name_without_regard_to_case() {
    let fx = Fx::new().await;
    for name in ["docling", "Pandoc local", "alpha"] {
        fx.program(name).await;
    }
    let page = fx.list_converters_as(ALICE, Value::Null).await.unwrap();
    let list = page["converters"].as_array().unwrap();
    assert_eq!(names(list), ["alpha", "docling", "Pandoc local"]);
    assert_eq!(page["marker"], Value::Null, "{page}");
    assert!(page.as_object().unwrap().contains_key("marker"), "the marker is null, not absent");
}

#[tokio::test]
async fn e2e_0106_017_an_empty_name() {
    let fx = Fx::new().await;
    let e = refused(&fx, program_with("   ", json!(["c", "{document}"]), json!({}))).await;
    assert_eq!(e, "ERR_INVALID_ARGUMENT: name must be 1 to 64 characters");
    assert!(fx.converters().await.is_empty());
}

#[tokio::test]
async fn e2e_0106_018_a_name_of_65_characters() {
    let fx = Fx::new().await;
    let e =
        refused(&fx, program_with(&"n".repeat(65), json!(["c", "{document}"]), json!({}))).await;
    assert_eq!(e, "ERR_INVALID_ARGUMENT: name must be 1 to 64 characters");
    assert!(fx.converters().await.is_empty());
}

#[tokio::test]
async fn e2e_0106_019_a_name_taken_in_another_letter_case() {
    let fx = Fx::new().await;
    fx.program("Pandoc local").await;
    let e = refused(&fx, program_with("PANDOC LOCAL", json!(["c", "{document}"]), json!({}))).await;
    assert_eq!(e, "ERR_INVALID_ARGUMENT: a converter named 'PANDOC LOCAL' already exists");
    assert_eq!(names(&fx.converters().await), ["Pandoc local"]);
}

#[tokio::test]
async fn e2e_0106_020_an_unknown_form() {
    let fx = Fx::new().await;
    let a = json!({"name": "x", "form": "ftp", "command": ["c", "{document}"]});
    let e = refused(&fx, a).await;
    assert_eq!(e, "ERR_INVALID_ARGUMENT: form must be 'program' or 'http'");
    assert!(fx.converters().await.is_empty());
}

#[tokio::test]
async fn e2e_0106_021_a_program_converter_without_a_command() {
    let fx = Fx::new().await;
    let e = refused(&fx, program_with("x", json!(["", "  "]), json!({}))).await;
    assert_eq!(e, "ERR_INVALID_ARGUMENT: command is required for a program converter");
    let e = refused(&fx, json!({"name": "x", "form": "program"})).await;
    assert_eq!(e, "ERR_INVALID_ARGUMENT: command is required for a program converter");
    assert!(fx.converters().await.is_empty());
}

#[tokio::test]
async fn e2e_0106_022_a_command_that_is_not_a_list() {
    let fx = Fx::new().await;
    let e = refused(&fx, program_with("x", json!("pandoc-convert {document}"), json!({}))).await;
    assert_eq!(e, "ERR_INVALID_ARGUMENT: command must be a list of text arguments");
    let e = refused(&fx, program_with("x", json!(["c", 1, "{document}"]), json!({}))).await;
    assert_eq!(e, "ERR_INVALID_ARGUMENT: command must be a list of text arguments");
    assert!(fx.converters().await.is_empty());
}

#[tokio::test]
async fn e2e_0106_023_a_command_without_the_document_placeholder() {
    let fx = Fx::new().await;
    let e = refused(&fx, program_with("x", json!(["pandoc-convert"]), json!({}))).await;
    assert_eq!(
        e,
        "ERR_INVALID_ARGUMENT: command must contain exactly one '{document}' placeholder across its arguments, found 0"
    );
}

#[tokio::test]
async fn e2e_0106_024_a_command_with_the_document_placeholder_twice() {
    let fx = Fx::new().await;
    let e =
        refused(&fx, program_with("x", json!(["c", "{document}", "{document}"]), json!({}))).await;
    assert_eq!(
        e,
        "ERR_INVALID_ARGUMENT: command must contain exactly one '{document}' placeholder across its arguments, found 2"
    );
}

#[tokio::test]
async fn e2e_0106_025_a_command_with_the_output_folder_placeholder_twice() {
    let fx = Fx::new().await;
    let cmd = json!(["c", "{outdir}", "{outdir}", "{document}"]);
    let e = refused(&fx, program_with("x", cmd, json!({}))).await;
    assert_eq!(
        e,
        "ERR_INVALID_ARGUMENT: command may contain at most one '{outdir}' placeholder across its arguments, found 2"
    );
}

#[tokio::test]
async fn e2e_0106_026_an_http_field_given_to_a_program_converter() {
    let fx = Fx::new().await;
    let cmd = json!(["c", "{document}"]);
    let e = refused(
        &fx,
        program_with("x", cmd.clone(), json!({"url": "http://localhost:8099/convert"})),
    )
    .await;
    assert_eq!(e, "ERR_INVALID_ARGUMENT: url is not valid for a program converter");
    // An empty text counts as given; the fields are checked in the order of FR-NEW-002.
    for field in ["auth_header", "secret", "file_field", "response_field"] {
        let e = refused(&fx, program_with("x", cmd.clone(), json!({field: ""}))).await;
        assert_eq!(
            e,
            format!("ERR_INVALID_ARGUMENT: {field} is not valid for a program converter")
        );
    }
    let e = refused(&fx, program_with("x", cmd.clone(), json!({"secret": "s", "url": "u"}))).await;
    assert_eq!(e, "ERR_INVALID_ARGUMENT: url is not valid for a program converter");
    // `null` equals omitted.
    let ok = program_with("x", cmd, json!({"url": null, "secret": null}));
    assert!(fx.create_converter(ok).await.is_ok());
}

#[tokio::test]
async fn e2e_0106_049_a_page_marker_that_is_not_valid() {
    let fx = Fx::new().await;
    let e = fx.list_converters_as(ALICE, json!("garbage")).await.unwrap_err();
    assert_eq!(e, "ERR_INVALID_ARGUMENT: marker is not valid");
}

/// Of the operations E2E-050 names, this story ships create and list; the
/// others are added to this test by the stories that add their tools.
#[tokio::test]
async fn e2e_0106_050_only_platform_administrators_use_the_converter_tools() {
    let fx = Fx::new().await;
    let c = fx.program("Pandoc local").await;
    let before = fx.changes().await;
    let e = fx
        .create_converter_as(BOB, program_with("Bob conv", json!(["c", "{document}"]), json!({})))
        .await
        .unwrap_err();
    assert_eq!(e, FORBIDDEN_BOB);
    let e = fx.list_converters_as(BOB, Value::Null).await.unwrap_err();
    assert_eq!(e, FORBIDDEN_BOB);
    assert!(!e.contains("Pandoc"));
    let id = id_of(&c);
    for e in [
        fx.set_default_converter_as(BOB, id).await,
        fx.clear_default_converter_as(BOB).await,
        fx.get_default_converter_as(BOB).await,
        fx.set_project_converter_as(BOB, "proj", id).await,
        fx.clear_project_converter_as(BOB, "proj").await,
        fx.get_project_converter_as(BOB, "proj").await,
    ] {
        assert_eq!(e.unwrap_err(), FORBIDDEN_BOB);
    }
    assert_eq!(fx.converters().await, vec![c]);
    assert_eq!(fx.changes().await, before);
}

#[tokio::test]
async fn e2e_0106_061_a_name_of_64_characters_and_accents() {
    let fx = Fx::new().await;
    let name = format!("{}é🌐", "a".repeat(62));
    assert_eq!(name.chars().count(), 64);
    let c = fx.program(&name).await;
    assert_eq!(c["name"], name.as_str());
    assert_eq!(names(&fx.converters().await), [name.as_str()]);
}

#[tokio::test]
async fn e2e_0106_073_101_converters_are_paged() {
    let fx = Fx::new().await;
    for i in 1..=101 {
        fx.program(&format!("c{i:03}")).await;
    }
    let first = fx.list_converters_as(ALICE, Value::Null).await.unwrap();
    let list = first["converters"].as_array().unwrap();
    assert_eq!(list.len(), 100);
    assert_eq!(list[0]["name"], "c001");
    assert_eq!(list[99]["name"], "c100");
    let marker = first["marker"].clone();
    assert!(marker.is_string(), "{first}");
    let second = fx.list_converters_as(ALICE, marker).await.unwrap();
    assert_eq!(names(second["converters"].as_array().unwrap()), ["c101"]);
    assert_eq!(second["marker"], Value::Null);
}

#[tokio::test]
async fn e2e_0106_085_refused_and_no_op_changes_are_not_recorded() {
    let fx = Fx::new().await;
    fx.program("Pandoc local").await;
    let before = fx.changes().await;
    let e = refused(&fx, program_with("other", json!(["pandoc-convert"]), json!({}))).await;
    assert!(e.starts_with("ERR_INVALID_ARGUMENT"), "{e}");
    let e = refused(&fx, program_with("pandoc LOCAL", json!(["c", "{document}"]), json!({}))).await;
    assert!(e.contains("already exists"), "{e}");
    assert_eq!(fx.changes().await, before);
}

/// Of the operations E2E-121 names, this story ships the listing; the delete
/// and the project selection join it in their stories.
#[tokio::test]
async fn e2e_0106_121_a_non_administrator_is_refused_before_anything_else() {
    let fx = Fx::new().await;
    let e = fx.list_converters_as(BOB, json!("garbage")).await.unwrap_err();
    assert_eq!(e, FORBIDDEN_BOB);
    // Body shape comes after the refusal too.
    let e = fx.create_converter_as(BOB, json!({"name": 5, "form": "ftp", "command": "x"})).await;
    assert_eq!(e.unwrap_err(), FORBIDDEN_BOB);
    // An unknown project or converter is not revealed either.
    let e = fx.set_project_converter_as(BOB, "ghost", "conv-000000000000").await;
    assert_eq!(e.unwrap_err(), FORBIDDEN_BOB);
    let e = fx.set_default_converter_as(BOB, "conv-000000000000").await;
    assert_eq!(e.unwrap_err(), FORBIDDEN_BOB);
}

/// An http creation with a valid url and the extra fields of `extra`.
fn http_with(name: &str, extra: Value) -> Value {
    let mut a = json!({"name": name, "form": "http", "url": "http://localhost:8099/convert"});
    for (k, v) in extra.as_object().unwrap() {
        a[k] = v.clone();
    }
    a
}

/// Refused, nothing created, and the refusal does not quote `needle`.
async fn refused_http(fx: &Fx, a: Value, text: &str, needle: Option<&str>) {
    let e = refused(fx, a).await;
    assert_eq!(e, text);
    if let Some(n) = needle {
        assert!(!e.contains(n), "the refusal quotes the value: {e}");
    }
    assert!(fx.converters().await.is_empty());
}

#[tokio::test]
async fn e2e_0106_002_create_an_http_converter_with_a_secret() {
    let fx = Fx::new().await;
    let c = fx.create_converter(http_with("Docling API", json!({"secret": "Bearer tok-123"})));
    let c = c.await.unwrap();
    assert_eq!(c["form"], "http");
    assert_eq!(c["url"], "http://localhost:8099/convert");
    assert_eq!(c["auth_header"], "Authorization");
    assert_eq!(c["file_field"], "file");
    assert_eq!(c["response_field"], "");
    assert_eq!(c["has_secret"], true);
    assert_eq!(c["command"], Value::Null);
    assert!(!c.to_string().contains("tok-123"), "{c}");
    assert_eq!(fx.converters().await, vec![c]);
    assert_eq!(fx.converter_secrets().await, vec![Some("Bearer tok-123".to_string())]);
    fx.assert_nowhere("tok-123").await;
}

#[tokio::test]
async fn e2e_0106_027_an_http_converter_without_a_url() {
    let fx = Fx::new().await;
    let a = json!({"name": "x", "form": "http"});
    refused_http(&fx, a, "ERR_INVALID_ARGUMENT: url is required for an http converter", None).await;
    let a = json!({"name": "x", "form": "http", "url": "   "});
    refused_http(&fx, a, "ERR_INVALID_ARGUMENT: url is required for an http converter", None).await;
}

#[tokio::test]
async fn e2e_0106_028_an_http_converter_with_a_url_that_is_not_http_or_https() {
    let fx = Fx::new().await;
    let a = http_with("x", json!({"url": "ftp://host/convert"}));
    let text = "ERR_INVALID_ARGUMENT: url must be an http or https URL";
    refused_http(&fx, a, text, Some("ftp://")).await;
}

#[tokio::test]
async fn e2e_0106_029_an_http_converter_with_a_user_name_in_the_url() {
    let fx = Fx::new().await;
    let a = http_with("x", json!({"url": "http://bob:pw@localhost:8099/convert"}));
    let text = "ERR_INVALID_ARGUMENT: url must not carry a user name or password";
    refused_http(&fx, a, text, Some("bob:pw")).await;
    let a = http_with("x", json!({"url": "http://bob@localhost:8099/convert"}));
    refused_http(&fx, a, text, Some("bob@")).await;
}

#[tokio::test]
async fn e2e_0106_030_a_secret_without_a_header_name() {
    let fx = Fx::new().await;
    let a = http_with("x", json!({"secret": "tok", "auth_header": "   "}));
    let text = "ERR_INVALID_ARGUMENT: auth_header is required when a secret is set";
    refused_http(&fx, a, text, None).await;
}

#[tokio::test]
async fn e2e_0106_031_an_empty_file_field() {
    let fx = Fx::new().await;
    let a = http_with("x", json!({"file_field": "  "}));
    refused_http(&fx, a, "ERR_INVALID_ARGUMENT: file_field must not be empty", None).await;
}

#[tokio::test]
async fn e2e_0106_032_a_command_given_to_an_http_converter() {
    let fx = Fx::new().await;
    let a = http_with("x", json!({"command": ["c", "{document}"]}));
    let text = "ERR_INVALID_ARGUMENT: command is not valid for an http converter";
    refused_http(&fx, a, text, None).await;
    // The command comes after the url user name and before the header rules.
    let a = http_with(
        "x",
        json!({"url": "http://u:p@h/c", "command": ["c"], "secret": "t", "auth_header": ""}),
    );
    let text = "ERR_INVALID_ARGUMENT: url must not carry a user name or password";
    refused_http(&fx, a, text, None).await;
    let a = http_with("x", json!({"command": [], "secret": "t", "auth_header": ""}));
    let text = "ERR_INVALID_ARGUMENT: command is not valid for an http converter";
    refused_http(&fx, a, text, None).await;
}

#[tokio::test]
async fn e2e_0106_037_a_secret_of_4097_characters() {
    let fx = Fx::new().await;
    let a = http_with("x", json!({"secret": "s".repeat(4097)}));
    let text = "ERR_INVALID_ARGUMENT: secret must be at most 4096 characters";
    refused_http(&fx, a, text, None).await;
    // Counted after trimming.
    let a = http_with("x", json!({"secret": format!("  {}  ", "s".repeat(4096))}));
    assert_eq!(fx.create_converter(a).await.unwrap()["has_secret"], true);
}

#[tokio::test]
async fn e2e_0106_038_a_secret_when_no_master_key_is_configured() {
    let fx = Fx::with_key(None).await;
    let a = http_with("x", json!({"secret": "tok"}));
    let text = "ERR_NOT_SUPPORTED: no master key is configured; set FSA_TOKEN_KEY";
    refused_http(&fx, a, text, None).await;
    assert!(fx.changes().await.is_empty());
    // Without a secret the key is not needed.
    let c = fx.create_converter(http_with("x", json!({}))).await.unwrap();
    assert_eq!(c["has_secret"], false);
}

#[tokio::test]
async fn e2e_0106_097_a_converter_created_with_every_field_given() {
    let fx = Fx::new().await;
    let c = fx
        .create_converter(json!({
            "name": "Docling API", "form": "http",
            "url": "https://converter.example.com/v1/convert", "auth_header": "X-Api-Key",
            "secret": "k", "file_field": "document", "response_field": "markdown",
            "extensions": ["pdf"], "max_input_bytes": 1000000, "timeout_seconds": 60
        }))
        .await
        .unwrap();
    assert_eq!(c["name"], "Docling API");
    assert_eq!(c["url"], "https://converter.example.com/v1/convert");
    assert_eq!(c["auth_header"], "X-Api-Key");
    assert_eq!(c["file_field"], "document");
    assert_eq!(c["response_field"], "markdown");
    assert_eq!(c["extensions"], json!(["pdf"]));
    assert_eq!(c["max_input_bytes"], 1000000);
    assert_eq!(c["timeout_seconds"], 60);
    assert_eq!(c["has_secret"], true);
    assert_eq!(fx.converters().await, vec![c]);
}

/// The update half of E2E-125 (a blank secret keeps the stored one) comes with
/// `admin.update_converter`.
#[tokio::test]
async fn e2e_0106_125_a_blank_secret_means_no_secret() {
    let fx = Fx::new().await;
    let c = fx.create_converter(http_with("Docling API", json!({"secret": "   "}))).await.unwrap();
    assert_eq!(c["has_secret"], false);
    assert_eq!(fx.converter_secrets().await, vec![None]);
    // A blank secret needs no master key either.
    let fx = Fx::with_key(None).await;
    let c = fx.create_converter(http_with("x", json!({"secret": ""}))).await.unwrap();
    assert_eq!(c["has_secret"], false);
}

#[tokio::test]
async fn e2e_0106_126_a_header_name_that_is_not_valid() {
    let fx = Fx::new().await;
    let a = http_with("x", json!({"auth_header": "X Api Key", "secret": "abc"}));
    let text = "ERR_INVALID_ARGUMENT: auth_header is not a valid header name";
    refused_http(&fx, a, text, Some("X Api Key")).await;
}

#[tokio::test]
async fn e2e_0106_127_a_secret_that_is_not_a_valid_header_value() {
    let fx = Fx::new().await;
    let a = http_with("x", json!({"secret": "tok\nleak-me"}));
    let text = "ERR_INVALID_ARGUMENT: secret is not a valid header value";
    refused_http(&fx, a, text, Some("leak-me")).await;
    fx.assert_nowhere("leak-me").await;
}

#[tokio::test]
async fn e2e_0106_128_an_address_without_a_host_or_longer_than_2048_characters() {
    let fx = Fx::new().await;
    let text = "ERR_INVALID_ARGUMENT: url must be an http or https URL";
    refused_http(&fx, http_with("x", json!({"url": "http://"})), text, None).await;
    let long = format!("http://h/{}", "a".repeat(2049 - 9));
    assert_eq!(long.chars().count(), 2049);
    refused_http(&fx, http_with("x", json!({"url": long})), text, Some("aaaaaaaaaa")).await;
    let ok = format!("http://h/{}", "a".repeat(2048 - 9));
    assert!(fx.create_converter(http_with("x", json!({"url": ok}))).await.is_ok());
}

#[tokio::test]
async fn e2e_0106_143_an_empty_url() {
    let fx = Fx::new().await;
    let a = http_with("x", json!({"url": ""}));
    refused_http(&fx, a, "ERR_INVALID_ARGUMENT: url is required for an http converter", None).await;
}

#[tokio::test]
async fn e2e_0106_144_a_blank_header_name_without_a_secret() {
    let fx = Fx::new().await;
    let a = http_with("x", json!({"auth_header": "  ", "secret": "tok"}));
    let text = "ERR_INVALID_ARGUMENT: auth_header is required when a secret is set";
    refused_http(&fx, a, text, None).await;
    let c = fx.create_converter(http_with("x", json!({"auth_header": "  "}))).await.unwrap();
    assert_eq!(c["auth_header"], "Authorization");
    assert_eq!(c["has_secret"], false);
}

// US-0311: the limits common to both forms and the refusal of wrong typed values.

const TIMEOUT_TEXT: &str = "ERR_INVALID_ARGUMENT: timeout_seconds must be between 1 and 86400";
const CAP_TEXT: &str = "ERR_INVALID_ARGUMENT: max_input_bytes must be between 1 and 4294967296";
const EXT_TEXT: &str = "ERR_INVALID_ARGUMENT: extensions must be a list of up to 64 names of 1 to 16 letters or digits";

#[tokio::test]
async fn e2e_0106_033_a_timeout_of_0() {
    let fx = Fx::new().await;
    let a = program_with("x", json!(["c", "{document}"]), json!({"timeout_seconds": 0}));
    assert_eq!(refused(&fx, a).await, TIMEOUT_TEXT);
    assert!(fx.converters().await.is_empty());
}

#[tokio::test]
async fn e2e_0106_034_a_timeout_of_86401() {
    let fx = Fx::new().await;
    let a = program_with("x", json!(["c", "{document}"]), json!({"timeout_seconds": 86401}));
    assert_eq!(refused(&fx, a).await, TIMEOUT_TEXT);
}

#[tokio::test]
async fn e2e_0106_035_an_answer_cap_of_0() {
    let fx = Fx::new().await;
    let a = program_with("x", json!(["c", "{document}"]), json!({"max_input_bytes": 0}));
    assert_eq!(refused(&fx, a).await, CAP_TEXT);
    let a =
        program_with("x", json!(["c", "{document}"]), json!({"max_input_bytes": 4294967297u64}));
    assert_eq!(refused(&fx, a).await, CAP_TEXT);
}

#[tokio::test]
async fn e2e_0106_036_extensions_that_are_not_names() {
    let fx = Fx::new().await;
    let long = "a".repeat(17);
    let many: Vec<String> = (0..65).map(|i| format!("e{i}")).collect();
    for bad in [json!([".pdf"]), json!([long]), json!(many), json!("pdf"), json!([1])] {
        let a = program_with("x", json!(["c", "{document}"]), json!({"extensions": bad}));
        assert_eq!(refused(&fx, a).await, EXT_TEXT);
    }
}

#[tokio::test]
async fn e2e_0106_062_timeout_bounds() {
    let fx = Fx::new().await;
    for (n, t) in [("a", 1), ("b", 86400)] {
        let a = program_with(n, json!(["c", "{document}"]), json!({"timeout_seconds": t}));
        assert_eq!(fx.create_converter(a).await.unwrap()["timeout_seconds"], t);
    }
}

#[tokio::test]
async fn e2e_0106_063_an_answer_cap_bound() {
    let fx = Fx::new().await;
    for (n, t) in [("a", 1u64), ("b", 4294967296)] {
        let a = program_with(n, json!(["c", "{document}"]), json!({"max_input_bytes": t}));
        assert_eq!(fx.create_converter(a).await.unwrap()["max_input_bytes"], t);
    }
}

#[tokio::test]
async fn e2e_0106_112_extensions_are_stored_lower_case_without_duplicates() {
    let fx = Fx::new().await;
    let a = program_with(
        "x",
        json!(["c", "{document}"]),
        json!({"extensions": ["pdf", "PDF", "Docx"]}),
    );
    assert_eq!(fx.create_converter(a).await.unwrap()["extensions"], json!(["pdf", "docx"]));
    assert_eq!(fx.converters().await[0]["extensions"], json!(["pdf", "docx"]));
}

#[tokio::test]
async fn e2e_0106_145_numbers_that_are_not_whole() {
    let fx = Fx::new().await;
    for bad in [json!(60.5), json!("60"), json!(true)] {
        let a = program_with("x", json!(["c", "{document}"]), json!({"timeout_seconds": bad}));
        assert_eq!(refused(&fx, a).await, TIMEOUT_TEXT);
    }
}

#[tokio::test]
async fn e2e_0106_146_a_text_field_given_as_a_number() {
    let fx = Fx::new().await;
    let a = json!({"name": 42, "form": "program", "command": ["c", "{document}"]});
    assert_eq!(refused(&fx, a).await, "ERR_INVALID_ARGUMENT: name must be text");
    let a = http_with("x", json!({"url": 42}));
    assert_eq!(refused(&fx, a).await, "ERR_INVALID_ARGUMENT: url must be text");
}

const PANDOC: &str = "Pandoc local";

/// An http converter `Docling API`, with the secret `s1` when `secret`.
async fn docling(fx: &Fx, secret: bool) -> Value {
    let extra = if secret { json!({"secret": "s1"}) } else { json!({}) };
    fx.create_converter(http_with("Docling API", extra)).await.unwrap()
}

fn id_of(c: &Value) -> &str {
    c["converter_id"].as_str().unwrap()
}

#[tokio::test]
async fn e2e_0106_005_a_converter_is_updated() {
    let fx = Fx::new().await;
    let c = fx.program(PANDOC).await;
    let u = fx
        .update_converter(
            id_of(&c),
            json!({"name": "Pandoc 3", "timeout_seconds": 120, "extensions": ["PDF", "docx"]}),
        )
        .await
        .unwrap();
    assert_eq!(u["converter_id"], c["converter_id"]);
    assert_eq!(u["form"], "program");
    assert_eq!(u["name"], "Pandoc 3");
    assert_eq!(u["timeout_seconds"], 120);
    assert_eq!(u["extensions"], json!(["pdf", "docx"]));
    assert_eq!(u["command"], c["command"]);
    assert!(u["updated_at"].as_str() >= u["created_at"].as_str(), "{u}");
    assert_eq!(fx.converters().await, vec![u]);
}

#[tokio::test]
async fn e2e_0106_039_a_secret_kept_and_cleared_together() {
    let fx = Fx::new().await;
    let c = docling(&fx, true).await;
    let e = fx.update_converter(id_of(&c), json!({"secret": "new", "clear_secret": true})).await;
    assert_eq!(
        e.unwrap_err(),
        "ERR_INVALID_ARGUMENT: secret and clear_secret cannot be given together"
    );
    assert_eq!(fx.converter_secrets().await, vec![Some("s1".to_string())]);
    assert_eq!(fx.converters().await, vec![c]);
}

#[tokio::test]
async fn e2e_0106_040_an_update_of_an_unknown_converter() {
    let fx = Fx::new().await;
    let e = fx.update_converter("conv-000000000000", json!({"name": "x"})).await;
    assert_eq!(e.unwrap_err(), "ERR_NOT_FOUND: converter not found: conv-000000000000");
}

#[tokio::test]
async fn e2e_0106_041_a_change_of_form() {
    let fx = Fx::new().await;
    let c = fx.program(PANDOC).await;
    let e = fx.update_converter(id_of(&c), json!({"form": "http"})).await;
    assert_eq!(e.unwrap_err(), "ERR_INVALID_ARGUMENT: form cannot be changed");
    assert_eq!(fx.converters().await, vec![c]);
}

#[tokio::test]
async fn e2e_0106_042_an_update_that_breaks_a_rule() {
    let fx = Fx::new().await;
    let c = fx.program(PANDOC).await;
    let e = fx.update_converter(id_of(&c), json!({"command": ["c"]})).await;
    assert_eq!(
        e.unwrap_err(),
        "ERR_INVALID_ARGUMENT: command must contain exactly one '{document}' placeholder across its arguments, found 0"
    );
    assert_eq!(fx.converters().await, vec![c]);
}

/// The default and project halves of E2E-083 come with the selection stories
/// (US-0313 on); here the converter keeps its id, which is what they hold.
#[tokio::test]
async fn e2e_0106_083_an_update_renames_without_changing_the_id() {
    let fx = Fx::new().await;
    let c = fx.program(PANDOC).await;
    let u = fx.update_converter(id_of(&c), json!({"name": "Pandoc 4"})).await.unwrap();
    assert_eq!(u["converter_id"], c["converter_id"]);
    assert_eq!(fx.converters().await.len(), 1);
}

#[tokio::test]
async fn e2e_0106_084_the_record_of_changes_holds_field_names_only() {
    let fx = Fx::new().await;
    let c = docling(&fx, false).await;
    fx.update_converter(
        id_of(&c),
        json!({"url": "http://other.example.com/x", "secret": "tok-84", "timeout_seconds": 7731}),
    )
    .await
    .unwrap();
    let changes = fx.changes().await;
    assert_eq!(changes[0]["action"], "update_converter");
    assert_eq!(changes[0]["target_kind"], "converter");
    assert_eq!(changes[0]["fields"], json!(["url", "secret", "timeout_seconds"]));
    let all = json!(changes).to_string();
    for needle in ["other.example.com", "tok-84", "7731"] {
        assert!(!all.contains(needle), "the record holds {needle}: {all}");
    }
    fx.assert_nowhere("tok-84").await;
}

#[tokio::test]
async fn e2e_0106_116_an_answer_sent_back_as_an_update_is_accepted() {
    let fx = Fx::new().await;
    let c = fx.program(PANDOC).await;
    let before = fx.changes().await;
    let u = fx.update_converter(id_of(&c), c.clone()).await.unwrap();
    assert_eq!(u, c);
    assert_eq!(fx.changes().await, before);
    for field in ["url", "auth_header", "file_field", "response_field"] {
        let e = fx.update_converter(id_of(&c), json!({ field: "" })).await.unwrap_err();
        assert_eq!(
            e,
            format!("ERR_INVALID_ARGUMENT: {field} is not valid for a program converter")
        );
    }
    // An http converter's answer goes back too, the secret flag ignored.
    let h = docling(&fx, true).await;
    assert_eq!(fx.update_converter(id_of(&h), h.clone()).await.unwrap(), h);
    assert_eq!(fx.converter_secrets().await.last().unwrap().as_deref(), Some("s1"));
}

/// The tool has one `converter_id`, the target; a body naming another one comes
/// from the REST plane, so the registry is called as that plane will.
#[tokio::test]
async fn e2e_0106_133_an_update_that_names_another_converter_in_its_body() {
    use crate::settings::{ConverterInput, Registry};
    let fx = Fx::new().await;
    let p = fx.program(PANDOC).await;
    let d = docling(&fx, false).await;
    let reg = Registry::of(fx.state.admin.as_ref(), 0).unwrap();
    let input = ConverterInput {
        converter_id: Some(d["converter_id"].clone()),
        name: Some(json!("renamed")),
        ..Default::default()
    };
    let e = reg.update_converter(ALICE, id_of(&p), input).await.unwrap_err();
    assert_eq!(e.to_string(), "ERR_INVALID_ARGUMENT: converter_id in the body does not match");
    let same =
        ConverterInput { converter_id: Some(p["converter_id"].clone()), ..Default::default() };
    reg.update_converter(ALICE, id_of(&p), same).await.unwrap();
    assert_eq!(fx.converters().await.len(), 2);
    assert_eq!(fx.converters().await[1]["name"], "Pandoc local");
}

#[tokio::test]
async fn e2e_0106_134_a_rename_that_changes_only_the_letter_case() {
    let fx = Fx::new().await;
    let c = fx.program(PANDOC).await;
    let u = fx.update_converter(id_of(&c), json!({"name": "PANDOC LOCAL"})).await.unwrap();
    assert_eq!(u["name"], "PANDOC LOCAL");
    let n = fx.changes().await.len();
    let again = fx.update_converter(id_of(&c), json!({"name": "PANDOC LOCAL"})).await.unwrap();
    assert_eq!(again, u);
    assert_eq!(fx.changes().await.len(), n);
    let d = docling(&fx, false).await;
    let e = fx.update_converter(id_of(&d), json!({"name": "pandoc local"})).await.unwrap_err();
    assert_eq!(e, "ERR_INVALID_ARGUMENT: a converter named 'pandoc local' already exists");
}

#[tokio::test]
async fn e2e_0106_137_clearing_the_secret_of_a_converter_that_has_none() {
    let fx = Fx::new().await;
    let c = docling(&fx, false).await;
    let n = fx.changes().await.len();
    for a in [json!({"clear_secret": true}), json!({"secret": "   ", "clear_secret": true})] {
        assert_eq!(fx.update_converter(id_of(&c), a).await.unwrap(), c);
    }
    assert_eq!(fx.changes().await.len(), n);
}

#[tokio::test]
async fn e2e_0106_138_clearing_the_secret_of_a_program_converter() {
    let fx = Fx::new().await;
    let c = fx.program(PANDOC).await;
    let e = fx.update_converter(id_of(&c), json!({"clear_secret": true})).await;
    assert_eq!(
        e.unwrap_err(),
        "ERR_INVALID_ARGUMENT: clear_secret is not valid for a program converter"
    );
}

#[tokio::test]
async fn e2e_0106_147_clear_secret_given_as_a_text() {
    let fx = Fx::new().await;
    let c = docling(&fx, true).await;
    let e = fx.update_converter(id_of(&c), json!({"clear_secret": "yes"})).await;
    assert_eq!(e.unwrap_err(), "ERR_INVALID_ARGUMENT: clear_secret must be true or false");
}

#[tokio::test]
async fn e2e_0106_149_a_rename_is_recorded_with_the_new_name() {
    let fx = Fx::new().await;
    let c = fx.program(PANDOC).await;
    fx.update_converter(id_of(&c), json!({"name": "Pandoc 3"})).await.unwrap();
    let r = &fx.changes().await[0];
    assert_eq!(r["action"], "update_converter");
    assert_eq!(r["target_kind"], "converter");
    assert_eq!(r["target_id"], c["converter_id"]);
    assert_eq!(r["target_name"], "Pandoc 3");
    assert_eq!(r["fields"], json!(["name"]));
}

#[tokio::test]
async fn e2e_0106_150_an_update_that_touches_no_secret_does_not_need_the_master_key() {
    let mut fx = Fx::new().await;
    let c = docling(&fx, true).await;
    fx.restart(None).await;
    let u = fx.update_converter(id_of(&c), json!({"name": "Docling 2"})).await.unwrap();
    assert_eq!(u["has_secret"], true);
    let u = fx.update_converter(id_of(&c), json!({"clear_secret": true})).await.unwrap();
    assert_eq!(u["has_secret"], false);
    let e = fx.update_converter(id_of(&c), json!({"secret": "new"})).await.unwrap_err();
    assert_eq!(e, "ERR_NOT_SUPPORTED: no master key is configured; set FSA_TOKEN_KEY");
}

#[tokio::test]
async fn e2e_0106_066_a_secret_kept_replaced_and_cleared() {
    let fx = Fx::new().await;
    let c = docling(&fx, true).await;
    let id = id_of(&c);
    let u = fx.update_converter(id, json!({"timeout_seconds": 5})).await.unwrap();
    assert_eq!(u["has_secret"], true);
    assert_eq!(fx.converter_secrets().await, vec![Some("s1".to_string())]);
    let u = fx.update_converter(id, json!({"secret": "   "})).await.unwrap();
    assert_eq!(u["has_secret"], true, "a blank secret keeps the stored one");
    let u = fx.update_converter(id, json!({"secret": "s2"})).await.unwrap();
    assert_eq!(u["has_secret"], true);
    assert_eq!(fx.converter_secrets().await, vec![Some("s2".to_string())]);
    let u = fx.update_converter(id, json!({"clear_secret": true})).await.unwrap();
    assert_eq!(u["has_secret"], false);
    assert_eq!(fx.converter_secrets().await, vec![None]);
    fx.assert_nowhere("s2").await;
}

// US-0313: the platform default and the converter of a project.

const PROJ: &str = "proj";
const UNKNOWN_CONV: &str = "conv-000000000000";

#[tokio::test]
async fn e2e_0106_004_the_platform_default_is_set_and_read() {
    let fx = Fx::new().await;
    let c = fx.program(PANDOC).await;
    assert_eq!(fx.get_default_converter_as(ALICE).await.unwrap(), json!({"converter": null}));
    let set = fx.set_default_converter_as(ALICE, id_of(&c)).await.unwrap();
    assert_eq!(set, json!({"converter": c}));
    let got = fx.get_default_converter_as(ALICE).await.unwrap();
    assert_eq!(got["converter"]["name"], PANDOC);
    assert_eq!(got["converter"]["converter_id"], c["converter_id"]);
}

#[tokio::test]
async fn e2e_0106_006_a_new_default_replaces_the_previous_one() {
    let fx = Fx::new().await;
    let p = fx.program(PANDOC).await;
    let d = docling(&fx, false).await;
    fx.set_default_converter_as(ALICE, id_of(&p)).await.unwrap();
    fx.set_default_converter_as(ALICE, id_of(&d)).await.unwrap();
    let got = fx.get_default_converter_as(ALICE).await.unwrap();
    assert_eq!(got["converter"]["name"], "Docling API");
    assert_eq!(got["converter"]["converter_id"], d["converter_id"]);
}

#[tokio::test]
async fn e2e_0106_007_a_project_converter_is_set_and_read() {
    let fx = Fx::new().await;
    let d = docling(&fx, false).await;
    assert_eq!(
        fx.get_project_converter_as(ALICE, PROJ).await.unwrap(),
        json!({"project_id": PROJ, "converter": null})
    );
    let set = fx.set_project_converter_as(ALICE, PROJ, id_of(&d)).await.unwrap();
    assert_eq!(set, json!({"project_id": PROJ, "converter": d}));
    let got = fx.get_project_converter_as(ALICE, PROJ).await.unwrap();
    assert_eq!(got["project_id"], PROJ);
    assert_eq!(got["converter"]["name"], "Docling API");
    assert_eq!(got["converter"]["converter_id"], d["converter_id"]);
}

#[tokio::test]
async fn e2e_0106_046_an_unknown_converter_as_the_default() {
    let fx = Fx::new().await;
    let before = fx.changes().await;
    let e = fx.set_default_converter_as(ALICE, UNKNOWN_CONV).await.unwrap_err();
    assert_eq!(e, format!("ERR_NOT_FOUND: converter not found: {UNKNOWN_CONV}"));
    assert_eq!(fx.get_default_converter_as(ALICE).await.unwrap(), json!({"converter": null}));
    assert_eq!(fx.changes().await, before);
}

#[tokio::test]
async fn e2e_0106_047_an_unknown_project() {
    let fx = Fx::new().await;
    let c = fx.program(PANDOC).await;
    let before = fx.changes().await;
    let e = fx.set_project_converter_as(ALICE, "ghost", id_of(&c)).await.unwrap_err();
    assert_eq!(e, "ERR_PROJECT_NOT_FOUND: project 'ghost' not found");
    let e = fx.get_project_converter_as(ALICE, "ghost").await.unwrap_err();
    assert_eq!(e, "ERR_PROJECT_NOT_FOUND: project 'ghost' not found");
    let e = fx.clear_project_converter_as(ALICE, "ghost").await.unwrap_err();
    assert_eq!(e, "ERR_PROJECT_NOT_FOUND: project 'ghost' not found");
    assert_eq!(fx.changes().await, before);
}

#[tokio::test]
async fn e2e_0106_048_an_unknown_converter_for_a_project() {
    let fx = Fx::new().await;
    let before = fx.changes().await;
    let e = fx.set_project_converter_as(ALICE, PROJ, UNKNOWN_CONV).await.unwrap_err();
    assert_eq!(e, format!("ERR_NOT_FOUND: converter not found: {UNKNOWN_CONV}"));
    assert_eq!(
        fx.get_project_converter_as(ALICE, PROJ).await.unwrap(),
        json!({"project_id": PROJ, "converter": null})
    );
    assert_eq!(fx.changes().await, before);
}

#[tokio::test]
async fn e2e_0106_069_clearing_the_default_and_the_project_converter() {
    let fx = Fx::new().await;
    let c = fx.program(PANDOC).await;
    fx.set_default_converter_as(ALICE, id_of(&c)).await.unwrap();
    fx.set_project_converter_as(ALICE, PROJ, id_of(&c)).await.unwrap();
    let out = fx.clear_default_converter_as(ALICE).await.unwrap();
    assert_eq!(out, json!({"converter": null}));
    let out = fx.clear_project_converter_as(ALICE, PROJ).await.unwrap();
    assert_eq!(out, json!({"project_id": PROJ, "converter": null}));
    assert_eq!(fx.get_default_converter_as(ALICE).await.unwrap(), json!({"converter": null}));
    assert_eq!(
        fx.get_project_converter_as(ALICE, PROJ).await.unwrap(),
        json!({"project_id": PROJ, "converter": null})
    );
    // The converter itself stays.
    assert_eq!(fx.converters().await, vec![c]);
}

#[tokio::test]
async fn e2e_0106_071_a_project_removed_for_good_and_recreated_starts_without_converter() {
    let fx = Fx::new().await;
    let c = fx.program(PANDOC).await;
    fx.set_project_converter_as(ALICE, PROJ, id_of(&c)).await.unwrap();
    fx.state.admin.delete_project(PROJ).await.unwrap();
    let e = fx.get_project_converter_as(ALICE, PROJ).await.unwrap_err();
    assert_eq!(e, "ERR_PROJECT_NOT_FOUND: project 'proj' not found");
    fx.state.admin.create_project(PROJ, BOB).await.unwrap();
    assert_eq!(
        fx.get_project_converter_as(ALICE, PROJ).await.unwrap(),
        json!({"project_id": PROJ, "converter": null})
    );
}

#[tokio::test]
async fn e2e_0106_072_a_restored_project_keeps_its_converter() {
    let fx = Fx::new().await;
    let c = fx.program(PANDOC).await;
    fx.set_project_converter_as(ALICE, PROJ, id_of(&c)).await.unwrap();
    assert!(fx.state.admin.soft_delete_project(PROJ).await.unwrap());
    fx.state.admin.undelete_project(PROJ).await.unwrap();
    let got = fx.get_project_converter_as(ALICE, PROJ).await.unwrap();
    assert_eq!(got, json!({"project_id": PROJ, "converter": c}));
}

#[tokio::test]
async fn e2e_0106_074_choosing_a_converter_gives_no_access_to_the_files() {
    let fx = Fx::new().await;
    let c = fx.program(PANDOC).await;
    fx.set_project_converter_as(ALICE, PROJ, id_of(&c)).await.unwrap();
    let p: rmcp::handler::server::wrapper::Parameters<ListDirArgs> =
        args(json!({"mount_id": PROJ, "path": "/"}));
    let e = tool_json(fx.server(ALICE).fs_list_dir(p).await).unwrap_err();
    assert!(e.ends_with("ERR_FORBIDDEN: 'admin@example.com' is not a member of 'proj'"), "{e}");
}

#[tokio::test]
async fn e2e_0106_108_choosing_a_converter_leaves_the_files_alone() {
    let fx = Fx::new().await;
    let client = fx.state.stores.client(PROJ).await.unwrap();
    client.write_text_atomic("/b.txt", "b").await.unwrap();
    client.write_text_atomic("/c.txt", "c").await.unwrap();
    let paths = ["/a.txt", "/b.txt", "/c.txt"];
    let mut before = Vec::new();
    for p in paths {
        before.push(client.stat(p).await.unwrap());
    }
    let c = fx.program(PANDOC).await;
    fx.set_project_converter_as(ALICE, PROJ, id_of(&c)).await.unwrap();
    fx.get_project_converter_as(ALICE, PROJ).await.unwrap();
    fx.clear_project_converter_as(ALICE, PROJ).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    for (p, b) in paths.iter().zip(&before) {
        let now = client.stat(p).await.unwrap();
        assert_eq!(&now, b, "{p}");
    }
}

#[tokio::test]
async fn e2e_0106_119_each_choice_is_recorded() {
    let fx = Fx::new().await;
    let c = fx.program(PANDOC).await;
    let id = id_of(&c);
    fx.set_default_converter_as(ALICE, id).await.unwrap();
    fx.clear_default_converter_as(ALICE).await.unwrap();
    fx.set_project_converter_as(ALICE, PROJ, id).await.unwrap();
    fx.clear_project_converter_as(ALICE, PROJ).await.unwrap();
    let all = fx.changes().await;
    // Newest first, after the creation.
    let expected = [
        ("clear_project_converter", "project_converter", PROJ),
        ("set_project_converter", "project_converter", PROJ),
        ("clear_default_converter", "default_converter", id),
        ("set_default_converter", "default_converter", id),
    ];
    assert_eq!(all.len(), 5, "{all:?}");
    for (r, (action, kind, target)) in all.iter().zip(expected) {
        assert_eq!(r["action"], action);
        assert_eq!(r["target_kind"], kind);
        assert_eq!(r["target_id"], target);
        assert_eq!(r["target_name"], PANDOC);
        assert_eq!(r["fields"], json!([]));
        assert_eq!(r["person"], ALICE);
    }
    // Setting the same choice again records nothing.
    fx.set_default_converter_as(ALICE, id).await.unwrap();
    fx.set_project_converter_as(ALICE, PROJ, id).await.unwrap();
    let n = fx.changes().await.len();
    fx.set_default_converter_as(ALICE, id).await.unwrap();
    fx.set_project_converter_as(ALICE, PROJ, id).await.unwrap();
    assert_eq!(fx.changes().await.len(), n);
}

#[tokio::test]
async fn e2e_0106_120_clearing_nothing_answers_null_and_records_nothing() {
    let fx = Fx::new().await;
    let before = fx.changes().await;
    assert_eq!(fx.clear_default_converter_as(ALICE).await.unwrap(), json!({"converter": null}));
    assert_eq!(
        fx.clear_project_converter_as(ALICE, PROJ).await.unwrap(),
        json!({"project_id": PROJ, "converter": null})
    );
    assert_eq!(fx.changes().await, before);
}

#[tokio::test]
async fn e2e_0106_123_an_unknown_project_is_refused_before_an_unknown_converter() {
    let fx = Fx::new().await;
    let e = fx.set_project_converter_as(ALICE, "ghost", UNKNOWN_CONV).await.unwrap_err();
    assert_eq!(e, "ERR_PROJECT_NOT_FOUND: project 'ghost' not found");
}

#[tokio::test]
async fn e2e_0106_136_a_project_in_its_grace_period() {
    let fx = Fx::new().await;
    let c = fx.program(PANDOC).await;
    assert!(fx.state.admin.soft_delete_project(PROJ).await.unwrap());
    let set = fx.set_project_converter_as(ALICE, PROJ, id_of(&c)).await.unwrap();
    assert_eq!(set, json!({"project_id": PROJ, "converter": c}));
    let got = fx.get_project_converter_as(ALICE, PROJ).await.unwrap();
    assert_eq!(got, json!({"project_id": PROJ, "converter": c}));
    let out = fx.clear_project_converter_as(ALICE, PROJ).await.unwrap();
    assert_eq!(out, json!({"project_id": PROJ, "converter": null}));
    fx.state.admin.delete_project(PROJ).await.unwrap();
    let gone = "ERR_PROJECT_NOT_FOUND: project 'proj' not found";
    assert_eq!(fx.set_project_converter_as(ALICE, PROJ, id_of(&c)).await.unwrap_err(), gone);
    assert_eq!(fx.get_project_converter_as(ALICE, PROJ).await.unwrap_err(), gone);
    assert_eq!(fx.clear_project_converter_as(ALICE, PROJ).await.unwrap_err(), gone);
}

// US-0314: delete a converter.

#[tokio::test]
async fn e2e_0106_008_an_unused_converter_is_deleted() {
    let fx = Fx::new().await;
    let c = fx.program(PANDOC).await;
    let out = fx.delete_converter_as(ALICE, id_of(&c)).await.unwrap();
    assert_eq!(out, json!({"converter_id": id_of(&c)}));
    assert!(fx.converters().await.is_empty());
    let last = &fx.changes().await[0];
    assert_eq!(last["action"], "delete_converter");
    assert_eq!(last["target_kind"], "converter");
    assert_eq!(last["target_name"], PANDOC);
}

#[tokio::test]
async fn e2e_0106_043_the_default_converter_cannot_be_deleted() {
    let fx = Fx::new().await;
    let c = fx.program(PANDOC).await;
    fx.set_default_converter_as(ALICE, id_of(&c)).await.unwrap();
    let e = fx.delete_converter_as(ALICE, id_of(&c)).await.unwrap_err();
    assert_eq!(e, "ERR_INVALID_ARGUMENT: converter 'Pandoc local' is the platform default");
    assert_eq!(fx.converters().await.len(), 1);
}

#[tokio::test]
async fn e2e_0106_044_a_converter_used_by_projects_cannot_be_deleted() {
    let fx = Fx::new().await;
    fx.state.admin.create_project("other", BOB).await.unwrap();
    let c = docling(&fx, false).await;
    fx.set_project_converter_as(ALICE, PROJ, id_of(&c)).await.unwrap();
    fx.set_project_converter_as(ALICE, "other", id_of(&c)).await.unwrap();
    let e = fx.delete_converter_as(ALICE, id_of(&c)).await.unwrap_err();
    assert_eq!(e, "ERR_INVALID_ARGUMENT: converter 'Docling API' is used by 2 project(s)");
    assert_eq!(fx.converters().await.len(), 1);
}

#[tokio::test]
async fn e2e_0106_045_a_deletion_of_an_unknown_converter() {
    let fx = Fx::new().await;
    let e = fx.delete_converter_as(ALICE, UNKNOWN_CONV).await.unwrap_err();
    assert_eq!(e, "ERR_NOT_FOUND: converter not found: conv-000000000000");
}

#[tokio::test]
async fn e2e_0106_070_deleting_after_unsetting() {
    let fx = Fx::new().await;
    let c = fx.program(PANDOC).await;
    fx.set_default_converter_as(ALICE, id_of(&c)).await.unwrap();
    fx.clear_default_converter_as(ALICE).await.unwrap();
    fx.delete_converter_as(ALICE, id_of(&c)).await.unwrap();
    assert!(fx.converters().await.is_empty());
}

#[tokio::test]
async fn e2e_0106_091_project_selection_appears_in_the_usage_count() {
    let fx = Fx::new().await;
    let c = docling(&fx, true).await;
    fx.set_project_converter_as(ALICE, PROJ, id_of(&c)).await.unwrap();
    let e = fx.delete_converter_as(ALICE, id_of(&c)).await.unwrap_err();
    assert_eq!(e, "ERR_INVALID_ARGUMENT: converter 'Docling API' is used by 1 project(s)");
    fx.clear_project_converter_as(ALICE, PROJ).await.unwrap();
    fx.delete_converter_as(ALICE, id_of(&c)).await.unwrap();
    assert!(fx.converters().await.is_empty());
    assert_eq!(fx.converter_secrets().await, Vec::<Option<String>>::new());
}

#[tokio::test]
async fn e2e_0106_096_a_project_removed_for_good_no_longer_counts() {
    let fx = Fx::new().await;
    let c = docling(&fx, false).await;
    fx.set_project_converter_as(ALICE, PROJ, id_of(&c)).await.unwrap();
    fx.state.admin.delete_project(PROJ).await.unwrap();
    fx.delete_converter_as(ALICE, id_of(&c)).await.unwrap();
    assert!(fx.converters().await.is_empty());
}

#[tokio::test]
async fn e2e_0106_122_a_default_that_is_also_used_is_refused_as_the_default() {
    let fx = Fx::new().await;
    let c = docling(&fx, false).await;
    fx.set_default_converter_as(ALICE, id_of(&c)).await.unwrap();
    fx.set_project_converter_as(ALICE, PROJ, id_of(&c)).await.unwrap();
    let e = fx.delete_converter_as(ALICE, id_of(&c)).await.unwrap_err();
    assert_eq!(e, "ERR_INVALID_ARGUMENT: converter 'Docling API' is the platform default");
}

#[tokio::test]
async fn e2e_0106_124_a_soft_deleted_project_still_uses_its_converter() {
    let fx = Fx::new().await;
    let c = docling(&fx, false).await;
    fx.set_project_converter_as(ALICE, PROJ, id_of(&c)).await.unwrap();
    assert!(fx.state.admin.soft_delete_project(PROJ).await.unwrap());
    let e = fx.delete_converter_as(ALICE, id_of(&c)).await.unwrap_err();
    assert_eq!(e, "ERR_INVALID_ARGUMENT: converter 'Docling API' is used by 1 project(s)");
}

#[tokio::test]
async fn e2e_0106_mod001_a_non_administrator_cannot_delete_a_converter() {
    let fx = Fx::new().await;
    let c = fx.program(PANDOC).await;
    let e = fx.delete_converter_as(BOB, id_of(&c)).await.unwrap_err();
    assert_eq!(e, FORBIDDEN_BOB);
    assert_eq!(fx.converters().await.len(), 1);
}

// US-0315: a conversion uses the resolved converter (SPEC-0106 FR-NEW-010,
// FR-NEW-011, FR-NEW-013, FR-NEW-020).

use crate::mcp::server::WriteBytesArgs;

impl Fx {
    /// A stand in converter program that answers `# From <label>` on stdout.
    fn stand_in(&self, label: &str) -> Vec<String> {
        use std::os::unix::fs::PermissionsExt as _;
        let path = self.dir.path().join(format!("conv-{}.sh", label.replace(' ', "-")));
        std::fs::write(&path, format!("#!/bin/sh\nprintf '# From {label}\\n'\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        vec![path.display().to_string(), "{document}".into()]
    }

    /// A registry converter named `name` that answers `# From <label>`.
    async fn stand_in_converter(&self, name: &str, label: &str) -> String {
        let c = self
            .create_converter(
                json!({"name": name, "form": "program", "command": self.stand_in(label)}),
            )
            .await
            .unwrap();
        id_of(&c).to_string()
    }

    /// Bob writes `/report.pdf` in `proj` with the conversion option.
    async fn bob_writes_report(&self, overwrite: bool) -> Result<Value, String> {
        self.bob_writes_at("proj", "/report.pdf", overwrite).await
    }

    /// Bob writes `path` in `project` with the conversion option.
    async fn bob_writes_at(
        &self,
        project: &str,
        path: &str,
        overwrite: bool,
    ) -> Result<Value, String> {
        let p: rmcp::handler::server::wrapper::Parameters<WriteBytesArgs> = args(json!({
            "mount_id": project,
            "path": path,
            "base64": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"%PDF-1.4"),
            "overwrite": overwrite,
            "create_parents": true,
            "trigger_documentation_service": true,
        }));
        self.keep(tool_json(self.server(BOB).fs_write_bytes(p).await))
    }

    async fn read_proj(&self, path: &str) -> Option<String> {
        self.read_in("proj", path).await
    }

    async fn read_in(&self, project: &str, path: &str) -> Option<String> {
        let client = self.state.stores.client(project).await.unwrap();
        client.read_text(path).await.ok()
    }

    /// The same platform with `doc` as its configuration file converter.
    fn with_file_converter(&mut self, doc: Option<std::sync::Arc<dyn crate::docs::DocService>>) {
        let s = &self.state;
        self.state = std::sync::Arc::new(crate::state::AppState {
            config: s.config.clone(),
            admin: s.admin.clone(),
            stores: s.stores.clone(),
            safety: s.safety.clone(),
            identity: s.identity.clone(),
            editors: s.editors.clone(),
            doc_service: doc,
            search: s.search.clone(),
        });
    }

    fn file_converter(&self) -> std::sync::Arc<dyn crate::docs::DocService> {
        std::sync::Arc::new(crate::docs::CliDocService::new(
            crate::config::DocServiceCliConfig { command: self.stand_in("file"), timeout_secs: 60 },
            vec![".pdf".into()],
            16 * 1024 * 1024,
        ))
    }
}

/// SPEC-0106/E2E-009: Given default A and `proj` converter B, When Bob writes
/// `/report.pdf` with the conversion option, Then the companion says `# From B`.
#[tokio::test]
async fn e2e_0106_009_the_project_converter_wins_over_the_default() {
    let fx = Fx::new().await;
    let a = fx.stand_in_converter("Pandoc local", "A").await;
    let b = fx.stand_in_converter("Docling API", "B").await;
    fx.set_default_converter_as(ALICE, &a).await.unwrap();
    fx.set_project_converter_as(ALICE, "proj", &b).await.unwrap();
    let out = fx.bob_writes_report(false).await.unwrap();
    assert_eq!(out["documentation"]["md_path"], "/report.md", "{out}");
    assert_eq!(fx.read_proj("/report.md").await.unwrap().trim(), "# From B");
}

/// SPEC-0106/E2E-010: Given default A and no project converter, When Bob writes
/// `/report.pdf` with the conversion option, Then the companion says `# From A`.
#[tokio::test]
async fn e2e_0106_010_the_default_converter_applies_without_a_project_converter() {
    let fx = Fx::new().await;
    let a = fx.stand_in_converter("Pandoc local", "A").await;
    fx.set_default_converter_as(ALICE, &a).await.unwrap();
    let out = fx.bob_writes_report(false).await.unwrap();
    assert_eq!(out["documentation"]["md_path"], "/report.md", "{out}");
    assert_eq!(fx.read_proj("/report.md").await.unwrap().trim(), "# From A");
}

/// SPEC-0106/E2E-011: Given nothing selected and a configuration file converter,
/// When Bob writes `/report.pdf` with the conversion option, Then the file
/// converter answers `# From file`. Pins unchanged behaviour.
#[tokio::test]
async fn e2e_0106_011_the_file_converter_applies_when_nothing_is_selected() {
    let mut fx = Fx::new().await;
    let file = fx.file_converter();
    fx.with_file_converter(Some(file));
    fx.stand_in_converter("Pandoc local", "A").await;
    fx.bob_writes_report(false).await.unwrap();
    assert_eq!(fx.read_proj("/report.md").await.unwrap().trim(), "# From file");
}

/// SPEC-0106/E2E-012: Given default A, When Alice makes B the default, Then the
/// next conversion uses B without a restart.
#[tokio::test]
async fn e2e_0106_012_a_change_applies_to_the_next_conversion_without_a_restart() {
    let fx = Fx::new().await;
    let a = fx.stand_in_converter("Pandoc local", "A").await;
    let b = fx.stand_in_converter("Docling API", "B").await;
    fx.set_default_converter_as(ALICE, &a).await.unwrap();
    fx.bob_writes_report(false).await.unwrap();
    assert_eq!(fx.read_proj("/report.md").await.unwrap().trim(), "# From A");
    fx.set_default_converter_as(ALICE, &b).await.unwrap();
    fx.bob_writes_report(true).await.unwrap();
    assert_eq!(fx.read_proj("/report.md").await.unwrap().trim(), "# From B");
}

/// SPEC-0106/E2E-055: Given no converter anywhere, When Bob writes
/// `/report.pdf` with the conversion option, Then `ERR_NOT_SUPPORTED: document
/// service is not configured` and nothing is written. Pins unchanged behaviour.
#[tokio::test]
async fn e2e_0106_055_no_converter_anywhere_refuses_and_writes_nothing() {
    let fx = Fx::new().await;
    let e = fx.bob_writes_report(false).await.unwrap_err();
    assert_eq!(e, "ERR_NOT_SUPPORTED: document service is not configured");
    assert!(fx.read_proj("/report.pdf").await.is_none());
}

/// SPEC-0106/E2E-110: an upgraded platform (configuration file converter disabled,
/// registry empty after the migration) refuses a conversion the same way and
/// leaves nothing written. Pins unchanged behaviour.
#[tokio::test]
async fn e2e_0106_110_no_converter_anywhere_after_an_upgrade() {
    let fx = Fx::new().await;
    // The migration of an upgraded platform leaves the registry empty.
    assert!(
        fx.list_converters_as(ALICE, Value::Null).await.unwrap()["converters"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let e = fx.bob_writes_report(false).await.unwrap_err();
    assert_eq!(e, "ERR_NOT_SUPPORTED: document service is not configured");
    assert!(fx.read_proj("/report.pdf").await.is_none());
}

/// Stored bytes of a PDF, written without the conversion option.
const PDF: &[u8] = b"%PDF-1.4";

impl Fx {
    /// A converter that waits for the file `gate` before answering `# From <label>`,
    /// and touches `started` once it runs: a conversion Alice can change under.
    fn gated_stand_in(&self, label: &str) -> (Vec<String>, std::path::PathBuf, std::path::PathBuf) {
        use std::os::unix::fs::PermissionsExt as _;
        let started = self.dir.path().join(format!("started-{label}"));
        let gate = self.dir.path().join(format!("gate-{label}"));
        let path = self.dir.path().join(format!("gated-{label}.sh"));
        let script = format!(
            "#!/bin/sh\ntouch '{}'\nwhile [ ! -e '{}' ]; do sleep 0.05; done\nprintf '# From {label}\\n'\n",
            started.display(),
            gate.display()
        );
        std::fs::write(&path, script).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        (vec![path.display().to_string(), "{document}".into()], started, gate)
    }

    /// Bob stores `path` in `project` without converting it.
    async fn bob_stores(&self, project: &str, path: &str) {
        let client = self.state.stores.client(project).await.unwrap();
        client.write_bytes_atomic(path, PDF).await.unwrap();
    }

    /// A second project of Bob's, `other`.
    async fn with_other_project(&self) {
        self.state.admin.create_project("other", BOB).await.unwrap();
        self.state.stores.provision_volume("other").await.unwrap();
    }

    /// The multipart upload of `PDF` as `name` with the conversion option, as Bob.
    async fn bob_uploads(&self, name: &str) -> (axum::http::StatusCode, Value) {
        self.bob_uploads_many(&[name]).await
    }

    /// The multipart upload of `PDF` under every name of `names`, as Bob.
    async fn bob_uploads_many(&self, names: &[&str]) -> (axum::http::StatusCode, Value) {
        use tower::ServiceExt;
        let token = crate::keys::mint_token_from_file(
            self.dir.path().join("keys").join(crate::keys::PRIVATE_KEY_FILE),
            BOB,
            crate::keys::DEFAULT_ISSUER,
            crate::keys::DEFAULT_CLAIM,
            3600,
        )
        .unwrap();
        let b = "S-BOUNDARY";
        let mut body = format!(
            "--{b}\r\nContent-Disposition: form-data; name=\"directory\"\r\n\r\n/\r\n\
             --{b}\r\nContent-Disposition: form-data; name=\"trigger_documentation_service\"\r\n\r\ntrue\r\n"
        );
        for name in names {
            body.push_str(&format!(
                "--{b}\r\nContent-Disposition: form-data; name=\"files\"; filename=\"{name}\"\r\n\
                 Content-Type: application/pdf\r\n\r\n%PDF-1.4\r\n"
            ));
        }
        body.push_str(&format!("--{b}--\r\n"));
        let req = axum::http::Request::builder()
            .method("POST")
            .uri("/api/fs/proj/upload")
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", format!("multipart/form-data; boundary={b}"))
            .body(axum::body::Body::from(body))
            .unwrap();
        let res = crate::api::router(self.state.clone()).oneshot(req).await.unwrap();
        let status = res.status();
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
    }

    /// Bob's REST write of `path` in `proj` with the conversion option.
    async fn bob_writes_on_the_web(&self, path: &str) -> (axum::http::StatusCode, Value) {
        let body = json!({
            "path": path,
            "base64": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, PDF),
            "trigger_documentation_service": true,
        });
        self.rest(BOB, "POST", "/api/fs/proj/write-bytes", Some(body)).await
    }
}

/// SPEC-0106/E2E-060: Given Bob's conversion has started with converter B, When
/// Alice clears the project selection then deletes B while it runs, Then the
/// running conversion ends with `# From B` and the next one uses the default.
#[tokio::test]
async fn e2e_0106_060_a_converter_deleted_during_a_conversion_still_finishes() {
    let fx = Fx::new().await;
    let a = fx.stand_in_converter("Pandoc local", "A").await;
    let (command, started, gate) = fx.gated_stand_in("B");
    let b = fx
        .create_converter(json!({"name": "Docling API", "form": "program", "command": command}))
        .await
        .unwrap();
    let b = id_of(&b).to_string();
    fx.set_default_converter_as(ALICE, &a).await.unwrap();
    fx.set_project_converter_as(ALICE, "proj", &b).await.unwrap();
    let write = fx.bob_writes_report(false);
    let drive = async {
        while !started.exists() {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        fx.clear_project_converter_as(ALICE, "proj").await.unwrap();
        fx.delete_converter_as(ALICE, &b).await.unwrap();
        std::fs::write(&gate, "").unwrap();
    };
    let (out, ()) = tokio::join!(write, drive);
    out.unwrap();
    assert_eq!(fx.read_proj("/report.md").await.unwrap().trim(), "# From B");
    fx.bob_writes_at("proj", "/next.pdf", false).await.unwrap();
    assert_eq!(fx.read_proj("/next.md").await.unwrap().trim(), "# From A");
}

/// SPEC-0106/E2E-095: Given Bob's conversion in `proj` started with B, When
/// Alice makes A the converter of `proj` while it runs and Bob converts another
/// file, Then the running one ends with `# From B`, the next one gives `# From A`.
#[tokio::test]
async fn e2e_0106_095_a_conversion_in_flight_keeps_the_converter_it_started_with() {
    let fx = Fx::new().await;
    let a = fx.stand_in_converter("Pandoc local", "A").await;
    let (command, started, gate) = fx.gated_stand_in("B");
    let b = fx
        .create_converter(json!({"name": "Docling API", "form": "program", "command": command}))
        .await
        .unwrap();
    fx.set_project_converter_as(ALICE, "proj", id_of(&b)).await.unwrap();
    let write = fx.bob_writes_report(false);
    let drive = async {
        while !started.exists() {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        fx.set_project_converter_as(ALICE, "proj", &a).await.unwrap();
        fx.bob_writes_at("proj", "/next.pdf", false).await.unwrap();
        std::fs::write(&gate, "").unwrap();
    };
    let (out, ()) = tokio::join!(write, drive);
    out.unwrap();
    assert_eq!(fx.read_proj("/report.md").await.unwrap().trim(), "# From B");
    assert_eq!(fx.read_proj("/next.md").await.unwrap().trim(), "# From A");
}

/// SPEC-0106/E2E-068: the project converter wins over the default, which wins
/// over the configuration file converter, as each is cleared.
#[tokio::test]
async fn e2e_0106_068_the_project_converter_wins_over_the_default_and_the_file() {
    let mut fx = Fx::new().await;
    let file = fx.file_converter();
    fx.with_file_converter(Some(file));
    let a = fx.stand_in_converter("Pandoc local", "A").await;
    let b = fx.stand_in_converter("Docling API", "B").await;
    fx.set_default_converter_as(ALICE, &a).await.unwrap();
    fx.set_project_converter_as(ALICE, "proj", &b).await.unwrap();
    fx.bob_writes_report(false).await.unwrap();
    assert_eq!(fx.read_proj("/report.md").await.unwrap().trim(), "# From B");
    fx.clear_project_converter_as(ALICE, "proj").await.unwrap();
    fx.bob_writes_report(true).await.unwrap();
    assert_eq!(fx.read_proj("/report.md").await.unwrap().trim(), "# From A");
    fx.clear_default_converter_as(ALICE).await.unwrap();
    fx.bob_writes_report(true).await.unwrap();
    assert_eq!(fx.read_proj("/report.md").await.unwrap().trim(), "# From file");
}

/// SPEC-0106/E2E-075: the converter of `proj` is B; a write, an upload and the
/// conversion of an existing file, each through the tool and through the web
/// route where there is one (the upload has no tool), all give `# From B`.
#[tokio::test]
async fn e2e_0106_075_all_three_paths_use_the_resolved_converter() {
    let fx = Fx::new().await;
    let a = fx.stand_in_converter("Pandoc local", "A").await;
    let b = fx.stand_in_converter("Docling API", "B").await;
    fx.set_default_converter_as(ALICE, &a).await.unwrap();
    fx.set_project_converter_as(ALICE, "proj", &b).await.unwrap();
    let fxr = &fx;
    let read = |p: &'static str| async move { fxr.read_proj(p).await.unwrap().trim().to_string() };

    fx.bob_writes_at("proj", "/tool-write.pdf", false).await.unwrap();
    assert_eq!(read("/tool-write.md").await, "# From B");

    let (s, v) = fx.bob_writes_on_the_web("/web-write.pdf").await;
    assert!(s.is_success(), "{v}");
    assert_eq!(read("/web-write.md").await, "# From B");

    let (s, v) = fx.bob_uploads("up.pdf").await;
    assert!(s.is_success(), "{v}");
    assert_eq!(read("/up.md").await, "# From B");

    fx.bob_stores("proj", "/tool-doc.pdf").await;
    let p: rmcp::handler::server::wrapper::Parameters<crate::mcp::server::DocumentizeArgs> =
        args(json!({"mount_id": "proj", "path": "/tool-doc.pdf"}));
    fx.keep(tool_json(fx.server(BOB).fs_documentize(p).await)).unwrap();
    assert_eq!(read("/tool-doc.md").await, "# From B");

    fx.bob_stores("proj", "/web-doc.pdf").await;
    let (s, v) = fx
        .rest(BOB, "POST", "/api/fs/proj/documentize", Some(json!({"path": "/web-doc.pdf"})))
        .await;
    assert!(s.is_success(), "{v}");
    assert_eq!(read("/web-doc.md").await, "# From B");
}

/// SPEC-0106/E2E-079: `proj` has B, `other` has none, the default is A.
#[tokio::test]
async fn e2e_0106_079_the_other_project_is_not_affected() {
    let fx = Fx::new().await;
    fx.with_other_project().await;
    let a = fx.stand_in_converter("Pandoc local", "A").await;
    let b = fx.stand_in_converter("Docling API", "B").await;
    fx.set_default_converter_as(ALICE, &a).await.unwrap();
    fx.set_project_converter_as(ALICE, "proj", &b).await.unwrap();
    fx.bob_writes_at("proj", "/report.pdf", false).await.unwrap();
    fx.bob_writes_at("other", "/report.pdf", false).await.unwrap();
    assert_eq!(fx.read_in("proj", "/report.md").await.unwrap().trim(), "# From B");
    assert_eq!(fx.read_in("other", "/report.md").await.unwrap().trim(), "# From A");
}

/// SPEC-0106/E2E-080: an empty registry leaves the configuration file converter
/// in charge and the list empty. Pins unchanged behaviour.
#[tokio::test]
async fn e2e_0106_080_the_file_converter_is_untouched_while_the_registry_is_empty() {
    let mut fx = Fx::new().await;
    let file = fx.file_converter();
    fx.with_file_converter(Some(file));
    fx.bob_writes_report(false).await.unwrap();
    assert_eq!(fx.read_proj("/report.md").await.unwrap().trim(), "# From file");
    assert!(fx.converters().await.is_empty());
}

/// SPEC-0106/E2E-081: with the configuration file converter disabled, a registry
/// default converts. Passes at once: the platform of `Fx` has no file converter.
#[tokio::test]
async fn e2e_0106_081_a_registry_default_works_with_the_file_converter_disabled() {
    let fx = Fx::new().await;
    assert!(fx.state.doc_service.is_none());
    let a = fx.stand_in_converter("Pandoc local", "A").await;
    fx.set_default_converter_as(ALICE, &a).await.unwrap();
    fx.bob_writes_report(false).await.unwrap();
    assert_eq!(fx.read_proj("/report.md").await.unwrap().trim(), "# From A");
}

/// SPEC-0106/E2E-082: two `{document}` placeholders still stop the start, with
/// the text the configuration check has always used.
#[test]
fn e2e_0106_082_the_configuration_file_is_still_checked_at_start() {
    let e = crate::config::ServerConfig::from_yaml(
        "doc_service:\n  enabled: true\n  cli:\n    command: [c, \"{document}\", \"{document}\"]\n",
    )
    .expect_err("two documents is not the contract");
    assert_eq!(e.code, crate::errors::code::INVALID_ARGUMENT);
    assert_eq!(
        e.message,
        "doc_service.cli.command must contain exactly one '{document}' placeholder across its arguments, found 2"
    );
}

/// SPEC-0106/E2E-098: three refused creations change nothing, and `proj` keeps
/// converting with the configuration file converter.
#[tokio::test]
async fn e2e_0106_098_a_refused_creation_changes_nothing() {
    let mut fx = Fx::new().await;
    let file = fx.file_converter();
    fx.with_file_converter(Some(file));
    fx.program("Pandoc local").await;
    let before = fx.changes().await;
    refused(&fx, program_with("", json!(["c", "{document}"]), json!({}))).await;
    refused(&fx, http_with("bad url", json!({"url": "ftp://x"}))).await;
    refused(&fx, program_with("slow", json!(["c", "{document}"]), json!({"timeout_seconds": 0})))
        .await;
    assert_eq!(fx.converters().await.len(), 1);
    assert_eq!(fx.changes().await, before);
    fx.bob_writes_report(false).await.unwrap();
    assert_eq!(fx.read_proj("/report.md").await.unwrap().trim(), "# From file");
}

/// SPEC-0106/E2E-104: only a registry default, the file converter disabled; the
/// conversion works and the configuration stays as it was.
#[tokio::test]
async fn e2e_0106_104_a_platform_with_only_a_registry_default_converts() {
    let fx = Fx::new().await;
    let before = serde_json::to_string(&fx.state.config.doc_service).unwrap();
    let a = fx.stand_in_converter("Pandoc local", "A").await;
    fx.set_default_converter_as(ALICE, &a).await.unwrap();
    fx.bob_writes_report(false).await.unwrap();
    assert_eq!(fx.read_proj("/report.md").await.unwrap().trim(), "# From A");
    assert_eq!(serde_json::to_string(&fx.state.config.doc_service).unwrap(), before);
    assert!(fx.state.doc_service.is_none());
}

/// SPEC-0106/E2E-106: creating converters without choosing any changes no
/// conversion.
#[tokio::test]
async fn e2e_0106_106_creating_converters_changes_no_conversion() {
    let mut fx = Fx::new().await;
    let file = fx.file_converter();
    fx.with_file_converter(Some(file));
    fx.stand_in_converter("Pandoc local", "A").await;
    fx.stand_in_converter("Docling API", "B").await;
    fx.bob_writes_report(false).await.unwrap();
    assert_eq!(fx.read_proj("/report.md").await.unwrap().trim(), "# From file");
}

/// SPEC-0106/E2E-107: the same answer from a registry converter and from the
/// configuration file converter gives the same companion text and report.
#[tokio::test]
async fn e2e_0106_107_the_companion_and_the_report_do_not_depend_on_the_source() {
    let registry = Fx::new().await;
    let r = registry.stand_in_converter("Pandoc local", "same").await;
    registry.set_default_converter_as(ALICE, &r).await.unwrap();
    let from_registry = registry.bob_writes_report(false).await.unwrap();

    let mut file = Fx::new().await;
    let command = file.stand_in("same");
    file.with_file_converter(Some(std::sync::Arc::new(crate::docs::CliDocService::new(
        crate::config::DocServiceCliConfig { command, timeout_secs: 60 },
        vec![".pdf".into()],
        16 * 1024 * 1024,
    ))));
    let from_file = file.bob_writes_report(false).await.unwrap();

    assert_eq!(from_registry["documentation"], from_file["documentation"]);
    assert_eq!(registry.read_proj("/report.md").await, file.read_proj("/report.md").await);
}

/// An http registry converter answered by the stub of `docs::manifest_e2e`: the
/// http build path, the stored extensions and the stored answer cap all apply.
#[tokio::test]
async fn e2e_0106_http_converter_converts_and_keeps_its_extensions_and_cap() {
    let fx = Fx::new().await;
    let url = crate::docs::manifest_e2e::stub_url(
        json!({"markdown": "# From http", "files": {}}).to_string(),
    )
    .await;
    let c = fx
        .create_converter(json!({
            "name": "Docling API", "form": "http", "url": url, "extensions": ["pdf"],
            "max_input_bytes": 200
        }))
        .await
        .unwrap();
    fx.set_default_converter_as(ALICE, id_of(&c)).await.unwrap();
    // 8 bytes of PDF and the answer fit the cap of 200 and the extension list.
    fx.bob_writes_report(false).await.unwrap();
    assert_eq!(fx.read_proj("/report.md").await.unwrap().trim(), "# From http");
    // A name outside the stored extensions is refused with the converter's list.
    let e = fx.bob_writes_at("proj", "/notes.docx", false).await.unwrap_err();
    assert!(e.starts_with("ERR_NOT_SUPPORTED") && e.contains("accepts: .pdf"), "{e}");
    // The cap also bounds the answer: 8 bytes in, the stub's answer is longer.
    fx.update_converter(id_of(&c), json!({"max_input_bytes": 8})).await.unwrap();
    let out = fx.bob_writes_at("proj", "/long.pdf", false).await.unwrap();
    let m = out["documentation"]["error"]["message"].as_str().unwrap_or_default();
    assert!(m.contains("output is over the doc_service.max_input_bytes limit of 8"), "{out}");
    // Over the stored cap on the input.
    let big = fx.update_converter(id_of(&c), json!({"max_input_bytes": 4})).await.unwrap();
    assert_eq!(big["max_input_bytes"], 4);
    let e = fx.bob_writes_at("proj", "/again.pdf", false).await.unwrap_err();
    assert!(e.contains("max_input_bytes limit of 4"), "{e}");
}

// SPEC-0106 US-0316: extensions, size cap, unreadable secret and batch behavior.

/// A non default master key, for the restart of E2E-114.
const OTHER_KEY: [u8; 32] = [7u8; 32];

impl Fx {
    /// Bob writes `len` bytes of PDF at `path` in `proj` with the conversion option.
    async fn bob_writes_sized(&self, path: &str, len: usize) -> Result<Value, String> {
        let mut data = b"%PDF-1.4".to_vec();
        data.resize(len.max(data.len()), b'x');
        let p: rmcp::handler::server::wrapper::Parameters<WriteBytesArgs> = args(json!({
            "mount_id": "proj",
            "path": path,
            "base64": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, data),
            "create_parents": true,
            "trigger_documentation_service": true,
        }));
        self.keep(tool_json(self.server(BOB).fs_write_bytes(p).await))
    }

    /// A converter of `proj` named `name` answering `markdown` over http, with `extra`.
    async fn project_http(&self, name: &str, markdown: &str, extra: Value) -> String {
        let url = crate::docs::manifest_e2e::stub_url(
            json!({"markdown": markdown, "files": {}}).to_string(),
        )
        .await;
        let mut a = json!({"name": name, "form": "http", "url": url});
        for (k, v) in extra.as_object().unwrap() {
            a[k] = v.clone();
        }
        let id = id_of(&self.create_converter(a).await.unwrap()).to_string();
        self.set_project_converter_as(ALICE, "proj", &id).await.unwrap();
        id
    }

    /// A program converter running `script` (a shell body), as the converter of `proj`.
    async fn project_script(&self, name: &str, script: &str, extra: Value) -> String {
        use std::os::unix::fs::PermissionsExt as _;
        let path = self.dir.path().join(format!("script-{}.sh", name.replace(' ', "-")));
        std::fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut a = json!({"name": name, "form": "program",
            "command": [path.display().to_string(), "{document}"]});
        for (k, v) in extra.as_object().unwrap() {
            a[k] = v.clone();
        }
        let id = id_of(&self.create_converter(a).await.unwrap()).to_string();
        self.set_project_converter_as(ALICE, "proj", &id).await.unwrap();
        id
    }

    /// Nothing of `path` exists in `proj`, neither the source nor its companion.
    async fn assert_absent(&self, path: &str) {
        let client = self.state.stores.client("proj").await.unwrap();
        assert!(client.read_bytes(path).await.is_err(), "{path} was written");
    }
}

fn not_accepted(path: &str, list: &str) -> String {
    format!(
        "ERR_NOT_SUPPORTED: '{path}' cannot be documented, the document service accepts: {list}"
    )
}

/// SPEC-0106/E2E-054: `proj` accepts only `txt`, the default accepts `pdf`.
#[tokio::test]
async fn e2e_0106_054_an_extension_the_resolved_converter_does_not_accept() {
    let fx = Fx::new().await;
    let d = fx.stand_in_converter("Pandoc local", "A").await;
    fx.set_default_converter_as(ALICE, &d).await.unwrap();
    fx.project_script("Text only", "printf '# From T\\n'", json!({"extensions": ["txt"]})).await;
    let e = fx.bob_writes_report(false).await.unwrap_err();
    assert_eq!(e, not_accepted("/report.pdf", ".txt"));
    fx.assert_absent("/report.pdf").await;
    fx.assert_absent("/report.md").await;
}

/// SPEC-0106/E2E-056: the converter of `proj` fails; the file stays, the failure is
/// reported as for a converter of the configuration file.
#[tokio::test]
async fn e2e_0106_056_a_converter_that_fails() {
    let fx = Fx::new().await;
    fx.project_script("Broken", "exit 3", json!({})).await;
    let out = fx.bob_writes_report(false).await.unwrap();
    assert_eq!(fx.read_proj("/report.pdf").await.as_deref(), Some("%PDF-1.4"));
    assert!(out["documentation"]["error"]["code"].is_string(), "{out}");

    let mut file = Fx::new().await;
    let path = file.dir.path().join("fail.sh");
    std::fs::write(&path, "#!/bin/sh\nexit 3\n").unwrap();
    std::fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    file.with_file_converter(Some(std::sync::Arc::new(crate::docs::CliDocService::new(
        crate::config::DocServiceCliConfig {
            command: vec![path.display().to_string(), "{document}".into()],
            timeout_secs: 60,
        },
        vec![".pdf".into()],
        16 * 1024 * 1024,
    ))));
    let from_file = file.bob_writes_report(false).await.unwrap();
    assert_eq!(out["documentation"]["error"]["code"], from_file["documentation"]["error"]["code"]);
    assert!(fx.read_proj("/report.md").await.is_none());
    assert!(fx.read_proj("/report.pdf").await.is_some());
}

/// SPEC-0106/E2E-057: an answer over the cap of 1000 bytes keeps nothing.
#[tokio::test]
async fn e2e_0106_057_an_answer_over_the_converter_cap() {
    let fx = Fx::new().await;
    fx.project_http("Docling API", &"y".repeat(1001), json!({"max_input_bytes": 1000})).await;
    let out = fx.bob_writes_sized("/report.pdf", 900).await.unwrap();
    assert_eq!(
        out["documentation"]["error"]["message"],
        "document service output is over the doc_service.max_input_bytes limit of 1000"
    );
    assert!(fx.read_proj("/report.md").await.is_none());
}

/// SPEC-0106/E2E-064: no extensions means the built in list.
#[tokio::test]
async fn e2e_0106_064_empty_extensions_means_the_built_in_list() {
    let fx = Fx::new().await;
    fx.project_script("Any", "printf '# From any\\n'", json!({"extensions": []})).await;
    fx.bob_writes_at("proj", "/a.pdf", false).await.unwrap();
    fx.bob_writes_at("proj", "/b.docx", false).await.unwrap();
    assert!(fx.read_proj("/a.md").await.is_some() && fx.read_proj("/b.md").await.is_some());
    let e = fx.bob_writes_at("proj", "/c.xyz", false).await.unwrap_err();
    let built_in = crate::docs::service::DOC_SERVICE_EXTS.join(", ");
    assert_eq!(e, not_accepted("/c.xyz", &built_in));
}

/// SPEC-0106/E2E-065: `extensions` `["txt"]`.
#[tokio::test]
async fn e2e_0106_065_custom_extensions() {
    let fx = Fx::new().await;
    fx.project_script("Text only", "printf '# From T\\n'", json!({"extensions": ["txt"]})).await;
    // The fixture already holds /a.txt, so a fresh name stands for it.
    fx.bob_writes_at("proj", "/n.txt", false).await.unwrap();
    assert!(fx.read_proj("/n.md").await.is_some());
    let e = fx.bob_writes_at("proj", "/a.pdf", false).await.unwrap_err();
    assert_eq!(e, not_accepted("/a.pdf", ".txt"));
}

/// SPEC-0106/E2E-111: an extension matches the end of the name after a dot.
#[tokio::test]
async fn e2e_0106_111_an_extension_matches_the_end_of_the_name_after_a_dot() {
    let fx = Fx::new().await;
    fx.project_script("Text only", "printf '# From T\\n'", json!({"extensions": ["txt"]})).await;
    let e = fx.bob_writes_at("proj", "/notxt", false).await.unwrap_err();
    assert_eq!(e, not_accepted("/notxt", ".txt"));
    fx.bob_writes_at("proj", "/Notes.TXT", false).await.unwrap();
    assert!(
        fx.read_proj("/Notes.md").await.is_some() || fx.read_proj("/Notes.TXT.md").await.is_some()
    );
}

/// SPEC-0106/E2E-113: a document over the cap is refused before any write.
#[tokio::test]
async fn e2e_0106_113_a_document_over_the_cap_is_refused_before_any_write() {
    let fx = Fx::new().await;
    fx.project_http("Docling API", "# x", json!({"max_input_bytes": 1000})).await;
    let e = fx.bob_writes_sized("/report.pdf", 1001).await.unwrap_err();
    assert_eq!(
        e,
        "ERR_INVALID_ARGUMENT: '/report.pdf' is 1001 bytes, over the doc_service.max_input_bytes limit of 1000"
    );
    fx.assert_absent("/report.pdf").await;
}

const SECRET_TEXT: &str =
    "ERR_INTERNAL_ERROR: the secret of converter 'Docling API' cannot be read";

/// A `Docling API` converter with a secret, the converter of `proj`, then a
/// restart with another master key so the secret cannot be read.
async fn unreadable(fx: &mut Fx, extra: Value) {
    let mut a = http_with("Docling API", extra);
    a["secret"] = json!("s3cret-value");
    let id = id_of(&fx.create_converter(a).await.unwrap()).to_string();
    fx.set_project_converter_as(ALICE, "proj", &id).await.unwrap();
    fx.restart(Some(OTHER_KEY)).await;
}

/// SPEC-0106/E2E-114: a stored secret that can no longer be read.
#[tokio::test]
async fn e2e_0106_114_a_stored_secret_that_can_no_longer_be_read() {
    let mut fx = Fx::new().await;
    let d = fx.stand_in_converter("Pandoc local", "A").await;
    fx.set_default_converter_as(ALICE, &d).await.unwrap();
    unreadable(&mut fx, json!({})).await;
    let e = fx.bob_writes_report(false).await.unwrap_err();
    assert_eq!(e, SECRET_TEXT);
    assert!(!e.contains("s3cret-value"));
    fx.assert_absent("/report.pdf").await;
    fx.assert_absent("/report.md").await;
}

/// SPEC-0106/E2E-131: an extension refusal comes before an unreadable secret.
#[tokio::test]
async fn e2e_0106_131_an_extension_refusal_comes_before_an_unreadable_secret() {
    let mut fx = Fx::new().await;
    unreadable(&mut fx, json!({"extensions": ["pdf"]})).await;
    let e = fx.bob_writes_at("proj", "/c.xyz", false).await.unwrap_err();
    assert_eq!(e, not_accepted("/c.xyz", ".pdf"));
}

/// SPEC-0106/E2E-132: a size refusal comes before an unreadable secret.
#[tokio::test]
async fn e2e_0106_132_a_size_refusal_comes_before_an_unreadable_secret() {
    let mut fx = Fx::new().await;
    unreadable(&mut fx, json!({"max_input_bytes": 1000})).await;
    let e = fx.bob_writes_sized("/report.pdf", 1001).await.unwrap_err();
    assert_eq!(
        e,
        "ERR_INVALID_ARGUMENT: '/report.pdf' is 1001 bytes, over the doc_service.max_input_bytes limit of 1000"
    );
}

/// Two gated batches: `a.pdf` starts under the default A, `change` runs while it
/// waits, then the gate opens and `b.pdf` resolves again. The upload answer.
async fn batch_with_change<F, Fut>(fx: &Fx, change: F) -> (axum::http::StatusCode, Value)
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    let upload = fx.bob_uploads_many(&["a.pdf", "b.pdf"]);
    let (command, started, gate) = fx.gated_stand_in("A");
    let a = fx
        .create_converter(json!({"name": "Gated", "form": "program", "command": command}))
        .await
        .unwrap();
    fx.set_default_converter_as(ALICE, id_of(&a)).await.unwrap();
    let drive = async {
        while !started.exists() {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        change().await;
        std::fs::write(&gate, "").unwrap();
    };
    let (out, ()) = tokio::join!(upload, drive);
    out
}

/// SPEC-0106/E2E-129: the converter is resolved per file of a batch.
#[tokio::test]
async fn e2e_0106_129_batch_upload_resolves_the_converter_per_file() {
    let fx = Fx::new().await;
    let b = fx.stand_in_converter("Other", "B").await;
    let (s, v) = batch_with_change(&fx, || async {
        fx.set_default_converter_as(ALICE, &b).await.unwrap();
    })
    .await;
    assert!(s.is_success(), "{v}");
    assert_eq!(fx.read_proj("/a.md").await.unwrap().trim(), "# From A");
    assert_eq!(fx.read_proj("/b.md").await.unwrap().trim(), "# From B");
}

/// SPEC-0106/E2E-130: a default that refuses the second file stops the upload.
#[tokio::test]
async fn e2e_0106_130_a_change_between_two_files_of_a_batch_is_applied_per_file() {
    let fx = Fx::new().await;
    let t = fx
        .create_converter(json!({"name": "Text only", "form": "program",
            "command": fx.stand_in("T"), "extensions": ["txt"]}))
        .await
        .unwrap();
    let (s, v) = batch_with_change(&fx, || async {
        fx.set_default_converter_as(ALICE, id_of(&t)).await.unwrap();
    })
    .await;
    assert!(!s.is_success(), "{v}");
    assert!(
        v.to_string().contains("'/b.pdf' cannot be documented, the document service accepts: .txt"),
        "{v}"
    );
    assert_eq!(fx.read_proj("/a.md").await.unwrap().trim(), "# From A");
    assert!(fx.read_proj("/a.pdf").await.is_some());
    fx.assert_absent("/b.pdf").await;
}

/// SPEC-0106/E2E-148: the only converter disappears between two files.
#[tokio::test]
async fn e2e_0106_148_the_converter_disappears_between_two_files_of_a_batch() {
    let fx = Fx::new().await;
    let (s, v) = batch_with_change(&fx, || async {
        fx.clear_default_converter_as(ALICE).await.unwrap();
    })
    .await;
    assert!(!s.is_success(), "{v}");
    assert!(v.to_string().contains("document service is not configured"), "{v}");
    assert!(fx.read_proj("/a.pdf").await.is_some());
    fx.assert_absent("/b.pdf").await;
}

/// SPEC-0106/E2E-076: the answer carries the full document metadata through an
/// http registry converter.
#[tokio::test]
async fn e2e_0106_076_the_answer_carries_the_full_document_metadata() {
    let fx = Fx::new().await;
    let b64 = |t: &str| base64::Engine::encode(&base64::engine::general_purpose::STANDARD, t);
    let url = crate::docs::manifest_e2e::stub_url(
        json!({
            "markdown": "![](p1.png)\n",
            "files": {"p1.png": b64("PNG")},
            "manifest": {"version": 2, "pictures": {"p1.png": {"ocr_text": "Revenue"}}},
        })
        .to_string(),
    )
    .await;
    let c = fx
        .create_converter(json!({"name": "Docling API", "form": "http", "url": url}))
        .await
        .unwrap();
    fx.set_project_converter_as(ALICE, "proj", id_of(&c)).await.unwrap();
    fx.bob_writes_report(false).await.unwrap();
    let p: rmcp::handler::server::wrapper::Parameters<crate::mcp::server::ListImagesArgs> =
        args(json!({"mount_id": "proj", "path": "/report.pdf"}));
    let out = fx.keep(tool_json(fx.server(BOB).fs_list_images(p).await)).unwrap();
    assert_eq!(out["images"][0]["ocr_text"], "Revenue", "{out}");
}

// ---------------------------------------------------------------------------
// US-0317: arguments stay literal, secrets stay hidden, no redirect
// ---------------------------------------------------------------------------

use axum::http::{HeaderMap, StatusCode, header};
use std::sync::{Arc, Mutex};

const PWNED: &str = "/tmp/fsa-pwned";
const SECRET_V: &str = "tok-SECRET-123";
const FILE_TOKEN: &str = "cfg-TOKEN-456";

/// The credentials every request of a stub carried, one entry per request.
type Hits = Arc<Mutex<Vec<String>>>;

/// The two credential headers of a request, as `<authorization>|<x-api-key>`.
fn credentials(h: &HeaderMap) -> String {
    let get = |n: &str| h.get(n).and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    format!("{}|{}", get("authorization"), get("x-api-key"))
}

/// Serves `app` on a free port and returns its base address.
async fn spawn_stub(app: axum::Router) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    format!("http://{addr}")
}

/// A stub answering `status` with `pad` letters then the credentials it received.
async fn echo_stub(status: u16, pad: usize) -> (String, Hits) {
    let hits: Hits = Hits::default();
    let seen = hits.clone();
    let app = axum::Router::new().route(
        "/convert",
        axum::routing::post(move |h: HeaderMap| {
            let seen = seen.clone();
            async move {
                let c = credentials(&h);
                seen.lock().unwrap().push(c.clone());
                (StatusCode::from_u16(status).unwrap(), format!("{}{c}", "a".repeat(pad)))
            }
        }),
    );
    (format!("{}/convert", spawn_stub(app).await), hits)
}

/// A stub answering `# From B` only for exactly the header `name: value`
/// (otherwise 401), and recording what it received.
async fn exact_header_stub(name: &'static str, value: &'static str) -> (String, Hits) {
    let hits: Hits = Hits::default();
    let seen = hits.clone();
    let app = axum::Router::new().route(
        "/convert",
        axum::routing::post(move |h: HeaderMap| {
            let seen = seen.clone();
            async move {
                seen.lock().unwrap().push(credentials(&h));
                let other = if name == "authorization" { "x-api-key" } else { "authorization" };
                let ok = h.get(name).and_then(|v| v.to_str().ok()) == Some(value)
                    && h.get(other).is_none();
                if ok { (StatusCode::OK, "# From B") } else { (StatusCode::UNAUTHORIZED, "no") }
            }
        }),
    );
    (format!("{}/convert", spawn_stub(app).await), hits)
}

/// A stub answering `status` with a `Location` of `to` (a path or a full address).
async fn redirect_stub(status: u16, to: String) -> (String, Hits) {
    let hits: Hits = Hits::default();
    let seen = hits.clone();
    let app = axum::Router::new().route(
        "/convert",
        axum::routing::post(move |h: HeaderMap| {
            let (seen, to) = (seen.clone(), to.clone());
            async move {
                seen.lock().unwrap().push(credentials(&h));
                (StatusCode::from_u16(status).unwrap(), [(header::LOCATION, to)])
            }
        }),
    );
    (format!("{}/convert", spawn_stub(app).await), hits)
}

/// The recorded credentials of a stub, joined for a message.
fn seen_by(hits: &Hits) -> Vec<String> {
    hits.lock().unwrap().clone()
}

/// A converter program printing every argument it receives, itself included, one
/// per line as `ARG[<text>]`: the converter's own output is the record.
fn record_args(fx: &Fx, command_tail: &[&str]) -> Vec<String> {
    use std::os::unix::fs::PermissionsExt as _;
    let path = fx.dir.path().join("record-args");
    std::fs::write(
        &path,
        "#!/bin/sh\nprintf 'ARG[record-args]\\n'\nfor a in \"$@\"; do printf 'ARG[%s]\\n' \"$a\"; done\n",
    )
    .unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut command = vec![path.display().to_string()];
    command.extend(command_tail.iter().map(|s| (*s).to_string()));
    command
}

/// The `ARG[...]` lines of the companion Bob got, the document path blanked.
async fn recorded_args(fx: &Fx) -> Vec<String> {
    let md = fx.read_proj("/report.md").await.expect("the companion exists");
    md.lines()
        .filter_map(|l| l.strip_prefix("ARG[").and_then(|l| l.strip_suffix(']')))
        .map(str::to_string)
        .collect()
}

/// The exploit file must not exist before, and is cleared if a stale run left it.
fn clear_pwned() {
    let _ = std::fs::remove_file(PWNED);
    assert!(std::fs::metadata(PWNED).is_err(), "{PWNED} must be absent at the start");
}

fn assert_not_pwned() {
    assert!(std::fs::metadata(PWNED).is_err(), "{PWNED} exists: an argument ran as a command");
}

/// A registry http converter at `url` selected for `proj`.
async fn select_http(fx: &Fx, url: &str, extra: Value) {
    let mut a = json!({"url": url});
    for (k, v) in extra.as_object().unwrap() {
        a[k] = v.clone();
    }
    let c = fx.create_converter(http_with("Docling API", a)).await.unwrap();
    fx.set_project_converter_as(ALICE, "proj", id_of(&c)).await.unwrap();
}

/// The configuration file http converter, as the platform's file converter.
fn file_http(fx: &mut Fx, url: &str) {
    let cfg = crate::config::DocServiceApiConfig {
        url: url.to_string(),
        auth_header: "X-Api-Key".into(),
        auth_token: crate::config::Secret::new(FILE_TOKEN),
        ..Default::default()
    };
    let svc = crate::docs::ApiDocService::new(&cfg, vec![".pdf".into()], 16 * 1024 * 1024).unwrap();
    fx.with_file_converter(Some(Arc::new(svc)));
}

/// What Bob sees when the conversion fails: the whole answer of the write, whose
/// `documentation.error` carries the failure text.
async fn failure(fx: &Fx) -> String {
    let out = fx.bob_writes_report(false).await.expect("the write itself succeeds");
    let message = out["documentation"]["error"]["message"].as_str();
    assert!(message.is_some(), "the conversion must fail: {out}");
    out.to_string()
}

/// SPEC-0106/E2E-059: a failed answer that echoes the secret is shown redacted,
/// for a registry converter and for the configuration file converter.
#[tokio::test]
async fn e2e_0106_059_a_failed_converter_answer_that_echoes_the_secret() {
    let mut fx = Fx::new().await;
    let (url, _) = echo_stub(500, 0).await;
    select_http(&fx, &url, json!({"secret": format!("Bearer {SECRET_V}")})).await;
    let e = failure(&fx).await;
    assert!(e.contains("[redacted]"), "{e}");
    assert!(!e.contains(SECRET_V), "{e}");

    let mut fx2 = Fx::new().await;
    let (url, _) = echo_stub(500, 0).await;
    file_http(&mut fx2, &url);
    let e = failure(&fx2).await;
    assert!(e.contains("[redacted]"), "{e}");
    assert!(!e.contains(FILE_TOKEN), "{e}");
    let _ = &mut fx;
}

/// SPEC-0106/E2E-067: an argument that looks like a shell command stays one argument.
#[tokio::test]
async fn e2e_0106_067_an_argument_that_looks_like_a_shell_command_stays_one_argument() {
    clear_pwned();
    let fx = Fx::new().await;
    let command = record_args(&fx, &["x; touch /tmp/fsa-pwned #", "{document}"]);
    let c = fx
        .create_converter(json!({"name": "Rec", "form": "program", "command": command}))
        .await
        .unwrap();
    fx.set_project_converter_as(ALICE, "proj", id_of(&c)).await.unwrap();
    fx.bob_writes_report(false).await.unwrap();
    let got = recorded_args(&fx).await;
    assert_eq!(got.len(), 3, "{got:?}");
    assert_eq!(got[1], "x; touch /tmp/fsa-pwned #");
    assert_not_pwned();
}

/// SPEC-0106/E2E-077: the secret is sent as the value of the named header.
#[tokio::test]
async fn e2e_0106_077_the_secret_is_sent_as_the_header_value() {
    let fx = Fx::new().await;
    let (url, hits) = exact_header_stub("x-api-key", "abc").await;
    select_http(&fx, &url, json!({"auth_header": "X-Api-Key", "secret": "abc"})).await;
    fx.bob_writes_report(false).await.unwrap();
    assert_eq!(fx.read_proj("/report.md").await.unwrap().trim(), "# From B");
    assert_eq!(seen_by(&hits), vec!["|abc".to_string()]);
}

/// SPEC-0106/E2E-078: a converter with a secret is kept after a restart.
#[tokio::test]
async fn e2e_0106_078_a_converter_with_a_secret_is_kept_after_a_restart() {
    let mut fx = Fx::new().await;
    let (url, _) = exact_header_stub("authorization", "tok").await;
    select_http(&fx, &url, json!({"secret": "tok"})).await;
    let (list, changes) = (fx.converters().await, fx.changes().await);
    fx.restart(Some(crate::settings::e2e::KEY)).await;
    fx.bob_writes_report(false).await.unwrap();
    assert_eq!(fx.read_proj("/report.md").await.unwrap().trim(), "# From B");
    assert_eq!(fx.converters().await, list);
    assert_eq!(fx.changes().await, changes);
}

/// The raw stored bytes of every converter secret.
async fn raw_secret_blobs(fx: &Fx) -> Vec<Vec<u8>> {
    use crate::storage::rel::{Query, RelationalDb, SqlValue, SqliteRelationalDb};
    let db = SqliteRelationalDb::open(fx.dir.path().join("admin.db")).unwrap();
    let rows = db
        .query(&Query::new("SELECT secret_enc FROM converter WHERE secret_enc IS NOT NULL"))
        .await
        .unwrap();
    rows.iter()
        .filter(|r| !matches!(r.value(0).unwrap(), SqlValue::Null))
        .map(|r| r.blob(0).unwrap())
        .collect()
}

/// SPEC-0106/E2E-087: the secret is never revealed by an answer, a listing, a
/// record, or the log, including when a conversion fails.
#[tokio::test]
async fn e2e_0106_087_the_secret_is_never_revealed() {
    let fx = Fx::new().await;
    let (url, _) = echo_stub(500, 0).await;
    select_http(&fx, &url, json!({"secret": SECRET_V})).await;
    fx.set_default_converter_as(ALICE, id_of(&fx.converters().await[0].clone())).await.unwrap();
    fx.list_converters_as(ALICE, Value::Null).await.unwrap();
    fx.get_default_converter_as(ALICE).await.unwrap();
    fx.get_project_converter_as(ALICE, "proj").await.unwrap();
    let e = failure(&fx).await;
    assert!(!e.contains(SECRET_V), "{e}");
    fx.assert_nowhere(SECRET_V).await;
}

/// SPEC-0106/E2E-092: three shell forms stay literal arguments.
#[tokio::test]
async fn e2e_0106_092_an_argument_cannot_start_a_program_of_its_own() {
    clear_pwned();
    let fx = Fx::new().await;
    let payloads =
        ["$(touch /tmp/fsa-pwned)", "`touch /tmp/fsa-pwned`", "x | touch /tmp/fsa-pwned"];
    let mut tail = payloads.to_vec();
    tail.push("{document}");
    let c = fx
        .create_converter(
            json!({"name": "Rec", "form": "program", "command": record_args(&fx, &tail)}),
        )
        .await
        .unwrap();
    fx.set_project_converter_as(ALICE, "proj", id_of(&c)).await.unwrap();
    fx.bob_writes_report(false).await.unwrap();
    let got = recorded_args(&fx).await;
    assert_eq!(got.len(), 5, "{got:?}");
    assert_eq!(&got[1..4], &payloads.map(String::from), "{got:?}");
    assert_not_pwned();
}

/// SPEC-0106/E2E-093: a converter answer cannot reveal the secret, for the
/// registry converter and for the configuration file token.
#[tokio::test]
async fn e2e_0106_093_a_converter_answer_cannot_reveal_the_secret() {
    let fx = Fx::new().await;
    let (url, _) = echo_stub(500, 0).await;
    select_http(&fx, &url, json!({"secret": format!("Bearer {SECRET_V}")})).await;
    let e = failure(&fx).await;
    assert!(e.contains("[redacted]") && !e.contains(SECRET_V), "{e}");
    fx.assert_nowhere(SECRET_V).await;

    let mut fx = Fx::new().await;
    let (url, _) = echo_stub(500, 0).await;
    file_http(&mut fx, &url);
    let e = failure(&fx).await;
    assert!(e.contains("[redacted]") && !e.contains(FILE_TOKEN), "{e}");
    fx.assert_nowhere(FILE_TOKEN).await;
}

/// SPEC-0106/E2E-094: the stored secret is encrypted, never read back by a read,
/// and a refused creation without a master key stores nothing.
#[tokio::test]
async fn e2e_0106_094_the_stored_secret_cannot_be_read_back_by_any_read() {
    let fx = Fx::new().await;
    select_http(&fx, "http://localhost:8099/convert", json!({"secret": SECRET_V})).await;
    let id = id_of(&fx.converters().await[0].clone()).to_string();
    fx.set_default_converter_as(ALICE, &id).await.unwrap();
    fx.list_converters_as(ALICE, Value::Null).await.unwrap();
    fx.get_default_converter_as(ALICE).await.unwrap();
    fx.get_project_converter_as(ALICE, "proj").await.unwrap();
    fx.assert_nowhere(SECRET_V).await;
    let blobs = raw_secret_blobs(&fx).await;
    assert_eq!(blobs.len(), 1);
    assert!(
        !blobs[0].windows(SECRET_V.len()).any(|w| w == SECRET_V.as_bytes()),
        "the stored form holds the secret in clear"
    );
    assert_eq!(
        crate::git::oauth::cipher::decrypt(&crate::settings::e2e::KEY, &blobs[0]).unwrap(),
        SECRET_V
    );

    let bare = Fx::with_key(None).await;
    let e = bare
        .create_converter(http_with("Docling API", json!({"secret": SECRET_V})))
        .await
        .unwrap_err();
    assert!(e.starts_with("ERR_NOT_SUPPORTED"), "{e}");
    assert!(bare.converters().await.is_empty());
    assert!(raw_secret_blobs(&bare).await.is_empty());
    bare.assert_nowhere(SECRET_V).await;
}

/// SPEC-0106/E2E-101: a stored secret is used by the conversion.
#[tokio::test]
async fn e2e_0106_101_a_stored_secret_is_used_by_the_conversion() {
    let fx = Fx::new().await;
    let (url, _) = exact_header_stub("authorization", "Bearer tok-123").await;
    select_http(&fx, &url, json!({"secret": "Bearer tok-123"})).await;
    fx.bob_writes_report(false).await.unwrap();
    assert_eq!(fx.read_proj("/report.md").await.unwrap().trim(), "# From B");
}

/// SPEC-0106/E2E-117: a secret straddling byte 2048 of the body leaves no prefix.
#[tokio::test]
async fn e2e_0106_117_a_secret_that_straddles_the_quoted_slice_is_still_redacted() {
    let fx = Fx::new().await;
    // The secret (14 bytes) starts 12 bytes before the cut and ends 2 after it; the
    // 10 byte marker that replaces it still ends before the cut.
    let (url, _) = echo_stub(500, 2048 - 12).await;
    select_http(&fx, &url, json!({"secret": SECRET_V})).await;
    let e = failure(&fx).await;
    assert!(e.contains("[redacted]"), "{e}");
    for k in 4..=SECRET_V.len() {
        assert!(!e.contains(&SECRET_V[..k]), "prefix {:?} leaked: {e}", &SECRET_V[..k]);
    }
}

/// SPEC-0106/E2E-118: an http converter never follows a redirect.
#[tokio::test]
async fn e2e_0106_118_an_http_converter_never_follows_a_redirect() {
    let fx = Fx::new().await;
    let (second, second_hits) = echo_stub(200, 0).await;
    let (url, first) = redirect_stub(302, second).await;
    select_http(&fx, &url, json!({"auth_header": "X-Api-Key", "secret": "abc"})).await;
    let e = failure(&fx).await;
    assert!(e.contains("document service returned HTTP 302"), "{e}");
    assert_eq!(seen_by(&first).len(), 1);
    assert!(
        seen_by(&second_hits).is_empty(),
        "the redirect was followed: {:?}",
        seen_by(&second_hits)
    );
}

/// SPEC-0106/E2E-135: the configuration file converter follows no redirect.
#[tokio::test]
async fn e2e_0106_135_the_configuration_file_converter_does_not_follow_a_redirect() {
    let mut fx = Fx::new().await;
    let (second, second_hits) = echo_stub(200, 0).await;
    let (url, _) = redirect_stub(302, second).await;
    file_http(&mut fx, &url);
    let e = failure(&fx).await;
    assert!(e.contains("document service returned HTTP 302"), "{e}");
    assert!(
        seen_by(&second_hits).is_empty(),
        "the redirect was followed: {:?}",
        seen_by(&second_hits)
    );
}

/// SPEC-0106/E2E-141: a redirect to the same host is not followed.
#[tokio::test]
async fn e2e_0106_141_a_redirect_to_the_same_host_is_not_followed() {
    let fx = Fx::new().await;
    let (url, hits) = redirect_stub(307, "/elsewhere".into()).await;
    select_http(&fx, &url, json!({})).await;
    let e = failure(&fx).await;
    assert!(e.contains("document service returned HTTP 307"), "{e}");
    assert_eq!(seen_by(&hits).len(), 1);
}

/// SPEC-0106/E2E-MOD-001: a failed conversion quotes the converter answer with
/// every occurrence of the secret replaced by `[redacted]`.
#[tokio::test]
async fn e2e_0106_mod001_a_failed_conversion_quotes_the_answer_without_the_secret() {
    let fx = Fx::new().await;
    let (url, _) = echo_stub(500, 0).await;
    select_http(&fx, &url, json!({"secret": SECRET_V})).await;
    let e = failure(&fx).await;
    assert!(e.contains("document service returned HTTP 500: "), "{e}");
    assert!(e.contains("[redacted]") && !e.contains(SECRET_V), "{e}");
}

/// SPEC-0106/E2E-015: the latest entries, newest first, are the default set,
/// the update naming only `name`, and the creation.
#[tokio::test]
async fn e2e_0106_015_the_record_of_changes_lists_the_changes() {
    let fx = Fx::new().await;
    let c = fx.program(PANDOC).await;
    fx.update_converter(id_of(&c), json!({"name": "Pandoc 3"})).await.unwrap();
    fx.set_default_converter_as(ALICE, id_of(&c)).await.unwrap();
    let changes = fx.changes().await;
    let top: Vec<&str> = changes[..3].iter().map(|c| c["action"].as_str().unwrap()).collect();
    assert_eq!(top, ["set_default_converter", "update_converter", "create_converter"]);
    assert_eq!(changes[1]["fields"], json!(["name"]));
    assert!(changes[..3].iter().all(|c| c["person"] == ALICE), "{changes:?}");
}

/// SPEC-0106/E2E-086: three changes made at once come back newest first with
/// increasing ids when read oldest first.
#[tokio::test]
async fn e2e_0106_086_the_record_keeps_the_order_of_changes() {
    let fx = Fx::new().await;
    let a = fx.program("A conv").await;
    let b = fx.program("B conv").await;
    fx.set_default_converter_as(ALICE, id_of(&a)).await.unwrap();
    fx.set_default_converter_as(ALICE, id_of(&b)).await.unwrap();
    let changes = fx.changes().await;
    assert_eq!(changes.len(), 4);
    let ids: Vec<&str> = changes.iter().map(|c| c["change_id"].as_str().unwrap()).collect();
    assert!(ids.windows(2).all(|w| w[0] > w[1]), "newest first, {ids:?}");
}

/// SPEC-0106/E2E-102.
#[tokio::test]
async fn e2e_0106_102_only_administrators_read_the_record_of_changes() {
    let fx = Fx::new().await;
    fx.program(PANDOC).await;
    assert_eq!(fx.changes_as(BOB, "").await.unwrap_err(), FORBIDDEN_BOB);
}

/// SPEC-0106/E2E-103.
#[tokio::test]
async fn e2e_0106_103_the_record_of_changes_survives_a_restart() {
    let mut fx = Fx::new().await;
    let c = fx.program(PANDOC).await;
    fx.update_converter(id_of(&c), json!({"name": "Pandoc 3"})).await.unwrap();
    fx.set_default_converter_as(ALICE, id_of(&c)).await.unwrap();
    let before = fx.changes().await;
    assert_eq!(before.len(), 3);
    fx.restart(Some([7u8; 32])).await;
    assert_eq!(fx.changes().await, before);
}

/// SPEC-0106/E2E-109.
#[tokio::test]
async fn e2e_0106_109_a_refused_change_by_a_non_administrator_leaves_no_record() {
    let fx = Fx::new().await;
    fx.program(PANDOC).await;
    let e = fx
        .create_converter_as(BOB, program_with("Bob conv", json!(["c", "{document}"]), json!({})))
        .await
        .unwrap_err();
    assert_eq!(e, FORBIDDEN_BOB);
    let changes = fx.changes().await;
    assert_eq!(changes.len(), 1);
    assert!(!json!(changes).to_string().contains("Bob conv"));
}

// ── US-0319: the web routes ─────────────────────────────────────────────────

fn web_refusal(status: axum::http::StatusCode, body: &Value, tool: &str) {
    assert_eq!(body["detail"], json!(tool), "same text as the tool");
    assert_eq!(body["error"], json!(tool.split(':').next().unwrap()));
    let want = match body["error"].as_str().unwrap() {
        "ERR_FORBIDDEN" => 403,
        "ERR_NOT_FOUND" => 404,
        "ERR_INVALID_ARGUMENT" => 400,
        "ERR_ALREADY_EXISTS" | "ERR_CONFLICT" => 409,
        other => panic!("unexpected code {other}"),
    };
    assert_eq!(status.as_u16(), want);
}

/// SPEC-0106/E2E-013: the web interface creates and lists like the tools.
#[tokio::test]
async fn e2e_0106_013_rest_creates_and_lists_like_the_tools() {
    let fx = Fx::new().await;
    let body = json!({"name": "Pandoc local", "form": "program", "command": ["pandoc-convert", "{document}"]});
    let (s, made) = fx.rest(ALICE, "POST", "/api/admin/converters", Some(body.clone())).await;
    assert_eq!(s, axum::http::StatusCode::OK, "{made}");
    // The same input through the tool is a duplicate: compare on a second name.
    let twin = json!({"name": "Pandoc twin", "form": "program", "command": ["pandoc-convert", "{document}"]});
    let tool = fx.create_converter(twin).await.unwrap();
    for k in ["form", "command", "extensions", "max_input_bytes", "timeout_seconds", "has_secret"] {
        assert_eq!(made[k], tool[k], "{k}");
    }
    assert!(made["converter_id"].as_str().unwrap().starts_with("conv-"));
    let (s, web) = fx.rest(ALICE, "GET", "/api/admin/converters", None).await;
    assert_eq!(s, axum::http::StatusCode::OK);
    let via_tool = fx.list_converters_as(ALICE, Value::Null).await.unwrap();
    assert_eq!(web, via_tool);
    assert_eq!(names(web["converters"].as_array().unwrap()), ["Pandoc local", "Pandoc twin"]);
}

/// The default and project routes, update and delete, answer as the tools do.
#[tokio::test]
async fn e2e_0106_013_rest_default_project_update_delete() {
    let fx = Fx::new().await;
    let id = fx.program("one").await["converter_id"].as_str().unwrap().to_string();
    let ok = axum::http::StatusCode::OK;
    let (s, b) = fx
        .rest(ALICE, "PATCH", &format!("/api/admin/converters/{id}"), Some(json!({"name": "uno"})))
        .await;
    assert_eq!((s, b["name"].clone()), (ok, json!("uno")));
    let (s, b) = fx
        .rest(ALICE, "PUT", "/api/admin/converter-default", Some(json!({"converter_id": id})))
        .await;
    assert_eq!((s, b), (ok, fx.get_default_converter_as(ALICE).await.unwrap()));
    let (s, b) = fx.rest(ALICE, "GET", "/api/admin/converter-default", None).await;
    assert_eq!((s, b["converter"]["converter_id"].clone()), (ok, json!(id)));
    let (s, b) = fx.rest(ALICE, "DELETE", "/api/admin/converter-default", None).await;
    assert_eq!((s, b), (ok, json!({"converter": null})));
    let p = "/api/admin/projects/proj/converter";
    let (s, b) = fx.rest(ALICE, "PUT", p, Some(json!({"converter_id": id}))).await;
    assert_eq!((s, b), (ok, fx.get_project_converter_as(ALICE, "proj").await.unwrap()));
    let (s, b) = fx.rest(ALICE, "GET", p, None).await;
    assert_eq!((s, b["project_id"].clone()), (ok, json!("proj")));
    let (s, b) = fx.rest(ALICE, "DELETE", p, None).await;
    assert_eq!((s, b), (ok, json!({"project_id": "proj", "converter": null})));
    let (s, b) = fx.rest(ALICE, "DELETE", &format!("/api/admin/converters/{id}"), None).await;
    assert_eq!((s, b), (ok, json!({"converter_id": id})));
}

/// SPEC-0106/E2E-051: the web interface refuses a non administrator the same way.
#[tokio::test]
async fn e2e_0106_051_rest_refuses_a_non_administrator() {
    let fx = Fx::new().await;
    let body = json!({"name": "Bob conv", "form": "program", "command": ["c", "{document}"]});
    let (s, b) = fx.rest(BOB, "POST", "/api/admin/converters", Some(body)).await;
    web_refusal(s, &b, FORBIDDEN_BOB);
    let (s, b) = fx.rest(BOB, "GET", "/api/admin/converters", None).await;
    web_refusal(s, &b, FORBIDDEN_BOB);
    assert!(fx.converters().await.is_empty());
}

/// SPEC-0106/E2E-058: a web refusal equals the tool refusal.
#[tokio::test]
async fn e2e_0106_058_rest_refusal_equals_the_tool_refusal() {
    let fx = Fx::new().await;
    let tool = fx.update_converter("conv-000000000000", json!({"name": "x"})).await.unwrap_err();
    assert!(tool.starts_with("ERR_NOT_FOUND"), "{tool}");
    let (s, b) = fx
        .rest(ALICE, "PATCH", "/api/admin/converters/conv-000000000000", Some(json!({"name": "x"})))
        .await;
    web_refusal(s, &b, &tool);
}

/// SPEC-0106/E2E-099: the web interface pages like the tool.
#[tokio::test]
async fn e2e_0106_099_rest_pages_like_the_tool() {
    let fx = Fx::new().await;
    for i in 1..=101 {
        fx.program(&format!("c{i:03}")).await;
    }
    let (s, first) = fx.rest(ALICE, "GET", "/api/admin/converters", None).await;
    assert_eq!(s, axum::http::StatusCode::OK);
    assert_eq!(first["converters"].as_array().unwrap().len(), 100);
    assert_eq!(first, fx.list_converters_as(ALICE, Value::Null).await.unwrap());
    let m = first["marker"].as_str().unwrap().to_string();
    let (_, second) =
        fx.rest(ALICE, "GET", &format!("/api/admin/converters?marker={m}"), None).await;
    assert_eq!(names(second["converters"].as_array().unwrap()), ["c101"]);
    assert_eq!(second["marker"], Value::Null);
    assert_eq!(second, fx.list_converters_as(ALICE, json!(m)).await.unwrap());
}

// ---------------------------------------------------------------------------
// US-0320: the converters screen (E2E-014, 052, 053, 088, 089, 090, 100, 115, 139, 140, 142).
// ---------------------------------------------------------------------------

use super::e2e::{Via, token_of};
use axum::Router;

const DOCLING_URL: &str = "http://localhost:8099/convert";

impl Fx {
    fn conv_screen(&self) -> Router {
        crate::converters_screen::router(self.state.clone())
    }

    /// The page as `who` sees it in a browser, with the token its forms carry.
    async fn open_conv(&self, router: &Router, who: &'static str) -> (StatusCode, String) {
        self.page(router, Via::Cookie(who), "GET", "/app/converters", None).await
    }

    /// A form post as Alice in a browser: a fresh token from the page, then the post.
    async fn submit_conv(
        &self,
        router: &Router,
        action: &str,
        fields: &[(&str, &str)],
    ) -> (StatusCode, String) {
        let (s, html) = self.open_conv(router, ALICE).await;
        assert_eq!(s, StatusCode::OK, "{html}");
        let token = token_of(&html);
        let mut all: Vec<(&str, &str)> = fields.to_vec();
        all.push(("csrf_token", &token));
        self.page(
            router,
            Via::Cookie(ALICE),
            "POST",
            &format!("/app/converters/{action}"),
            Some(&all),
        )
        .await
    }

    async fn docling(&self) -> Value {
        self.create_converter(json!({
            "name": "Docling API", "form": "http", "url": DOCLING_URL,
            "secret": "Bearer tok-123", "response_field": "markdown", "extensions": ["pdf"]
        }))
        .await
        .unwrap()
    }
}

const SEE_OTHER: StatusCode = StatusCode::SEE_OTHER;

#[tokio::test]
async fn e2e_0106_014_the_screen_creates_a_converter() {
    let fx = Fx::new().await;
    let r = fx.conv_screen();
    let (s, _) = fx
        .submit_conv(
            &r,
            "converter/create",
            &[
                ("name", "Docling API"),
                ("form", "http"),
                ("url", DOCLING_URL),
                ("secret", "Bearer tok-123"),
            ],
        )
        .await;
    assert_eq!(s, SEE_OTHER);
    let (_, html) = fx.open_conv(&r, ALICE_STR).await;
    assert!(html.contains("Docling API") && html.contains("Secret stored"), "{html}");
    assert!(!html.contains("tok-123"));
    fx.assert_nowhere("tok-123").await;
}

const ALICE_STR: &str = ALICE;

#[tokio::test]
async fn e2e_0106_052_the_screen_refuses_a_non_administrator() {
    let fx = Fx::new().await;
    let r = fx.conv_screen();
    let (s, body) = fx.open_conv(&r, "bob@example.com").await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    assert!(body.contains("is not a platform admin"), "{body}");
    let (s, body) = fx
        .page(
            &r,
            Via::Cookie("bob@example.com"),
            "POST",
            "/app/converters/converter/create",
            Some(&[
                ("name", "X"),
                ("form", "program"),
                ("command", "a\n{document}"),
                ("csrf_token", "x"),
            ]),
        )
        .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    assert!(body.contains("is not a platform admin"), "{body}");
    assert!(fx.converters().await.is_empty());
}

#[tokio::test]
async fn e2e_0106_053_a_form_without_its_one_time_token() {
    let fx = Fx::new().await;
    let r = fx.conv_screen();
    let form = [("name", "P"), ("form", "program"), ("command", "a\n{document}")];
    let post = |extra: Option<&'static str>, who: &'static str| {
        let mut f: Vec<(&str, &str)> = form.to_vec();
        if let Some(t) = extra {
            f.push(("csrf_token", t));
        }
        let (fx, r) = (&fx, &r);
        async move {
            fx.page(r, Via::Cookie(who), "POST", "/app/converters/converter/create", Some(&f)).await
        }
    };
    let (s, body) = post(None, ALICE).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    assert!(
        body.contains("ERR_FORBIDDEN") && body.contains("missing or invalid csrf_token"),
        "{body}"
    );
    // A token already used.
    let (_, html) = fx.open_conv(&r, ALICE).await;
    let used = Box::leak(token_of(&html).into_boxed_str());
    let (s, _) = post(Some(used), ALICE).await;
    assert_eq!(s, SEE_OTHER);
    let (s, body) = post(Some(used), ALICE).await;
    assert_eq!((s, body.contains("missing or invalid csrf_token")), (StatusCode::FORBIDDEN, true));
    // A token issued to another person.
    let other = fx.open_conv(&r, ALICE).await.1;
    let other = Box::leak(token_of(&other).into_boxed_str());
    let (s, body) = post(Some(other), "bob@example.com").await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    // The admin refusal wins over the token one, as on the models screen.
    assert!(body.contains("'bob@example.com' is not a platform admin"), "{body}");
    assert_eq!(fx.converters().await.len(), 1, "only the one valid post created");
}

#[tokio::test]
async fn e2e_0106_088_the_screen_shows_the_secret_state_only() {
    let fx = Fx::new().await;
    let c = fx.docling().await;
    let id = c["converter_id"].as_str().unwrap();
    let r = fx.conv_screen();
    let (_, html) = fx.open_conv(&r, ALICE).await;
    assert!(html.contains("Secret stored") && !html.contains("tok-123"), "{html}");
    assert!(html.contains("name=\"secret\""));
    assert!(!html.contains("name=\"secret\" value="), "the secret field is never filled");
    let (s, _) = fx
        .submit_conv(
            &r,
            "converter/update",
            &[("converter_id", id), ("name", "Docling API"), ("secret", "")],
        )
        .await;
    assert_eq!(s, SEE_OTHER);
    assert_eq!(fx.converter_secrets().await, vec![Some("Bearer tok-123".to_string())]);
    fx.assert_nowhere("tok-123").await;
}

#[tokio::test]
async fn e2e_0106_089_the_screen_shows_the_refusal_text() {
    let fx = Fx::new().await;
    let r = fx.conv_screen();
    let (s, body) = fx
        .submit_conv(
            &r,
            "converter/create",
            &[("name", "   "), ("form", "program"), ("command", "a\n{document}")],
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert!(body.contains("name must be 1 to 64 characters"), "{body}");
    assert!(fx.converters().await.is_empty());
    let (s, _) = fx
        .submit_conv(
            &r,
            "converter/create",
            &[("name", "Ok"), ("form", "program"), ("command", "a\n{document}")],
        )
        .await;
    assert_eq!(s, SEE_OTHER);
}

#[tokio::test]
async fn e2e_0106_090_a_change_through_the_screen_is_visible_elsewhere() {
    let fx = Fx::new().await;
    let r = fx.conv_screen();
    let (s, _) = fx
        .submit_conv(
            &r,
            "converter/create",
            &[("name", "Via screen"), ("form", "program"), ("command", "a\n{document}")],
        )
        .await;
    assert_eq!(s, SEE_OTHER);
    assert_eq!(names(&fx.converters().await), ["Via screen"]);
    let (_, rest) = fx.rest(ALICE, "GET", "/api/admin/converters", None).await;
    assert_eq!(names(rest["converters"].as_array().unwrap()), ["Via screen"]);
    let (_, html) = fx.open_conv(&r, ALICE).await;
    assert!(html.contains("Via screen"));
}

#[tokio::test]
async fn e2e_0106_100_the_screen_renames_without_changing_the_id() {
    let fx = Fx::new().await;
    let c = fx.docling().await;
    let id = c["converter_id"].as_str().unwrap();
    fx.set_project_converter_as(ALICE, "proj", id).await.unwrap();
    let r = fx.conv_screen();
    let (s, _) = fx
        .submit_conv(&r, "converter/update", &[("converter_id", id), ("name", "Docling v2")])
        .await;
    assert_eq!(s, SEE_OTHER);
    let (_, html) = fx.open_conv(&r, ALICE).await;
    // The record of the creation keeps the old name, so look above the log.
    let rows = &html[..html.find("Latest changes").unwrap()];
    assert!(rows.contains("Docling v2") && !rows.contains("Docling API"), "{html}");
    let p = fx.get_project_converter_as(ALICE, "proj").await.unwrap();
    assert_eq!(p["converter"]["converter_id"], id);
    assert_eq!(p["converter"]["name"], "Docling v2");
    assert!(html.contains("data-project=\"proj\""));
}

#[tokio::test]
async fn e2e_0106_115_the_screen_creates_a_program_converter_from_text_fields() {
    let fx = Fx::new().await;
    let r = fx.conv_screen();
    let (s, _) = fx
        .submit_conv(
            &r,
            "converter/create",
            &[
                ("name", "Pandoc local"),
                ("form", "program"),
                ("command", "pandoc-convert\r\n\r\n{document}"),
                ("extensions", "pdf, docx md"),
            ],
        )
        .await;
    assert_eq!(s, SEE_OTHER);
    let all = fx.converters().await;
    assert_eq!(all[0]["command"], json!(["pandoc-convert", "{document}"]));
    assert_eq!(all[0]["extensions"], json!(["pdf", "docx", "md"]));
}

#[tokio::test]
async fn e2e_0106_139_the_screen_clears_a_secret() {
    let fx = Fx::new().await;
    let id = fx.docling().await["converter_id"].as_str().unwrap().to_string();
    let r = fx.conv_screen();
    let (s, _) = fx
        .submit_conv(&r, "converter/update", &[("converter_id", &id), ("clear_secret", "on")])
        .await;
    assert_eq!(s, SEE_OTHER);
    let (_, html) = fx.open_conv(&r, ALICE).await;
    assert!(html.contains("No secret") && !html.contains("Secret stored"), "{html}");
    assert_eq!(fx.converters().await[0]["has_secret"], false);
}

#[tokio::test]
async fn e2e_0106_140_the_screen_shows_the_20_latest_converter_changes() {
    let fx = Fx::new().await;
    for i in 1..=21 {
        fx.program(&format!("Conv number {i:02}")).await;
    }
    for i in 1..=3 {
        fx.local(&format!("Provider {i}")).await;
    }
    let r = fx.conv_screen();
    let (_, html) = fx.open_conv(&r, ALICE).await;
    let log = &html[html.find("Latest changes").expect("a changes section")..];
    assert!(!log.contains("Conv number 01"), "the oldest is missing");
    assert!(log.contains("Conv number 02") && log.contains("Conv number 21"));
    assert!(!log.contains("provider") && !log.contains("create_provider"), "{log}");
    assert_eq!(log.matches("create_converter").count(), 20);
    assert!(log.find("Conv number 21").unwrap() < log.find("Conv number 20").unwrap());
}

#[tokio::test]
async fn e2e_0106_142_the_screen_empties_the_response_field_and_the_extensions() {
    let fx = Fx::new().await;
    let id = fx.docling().await["converter_id"].as_str().unwrap().to_string();
    let r = fx.conv_screen();
    let (s, _) = fx
        .submit_conv(
            &r,
            "converter/update",
            &[("converter_id", &id), ("response_field", ""), ("extensions", "")],
        )
        .await;
    assert_eq!(s, SEE_OTHER);
    let c = &fx.converters().await[0];
    assert_eq!(c["response_field"], "");
    assert_eq!(c["extensions"], json!([]));
    assert_eq!(c["url"], DOCLING_URL);
}

/// SPEC-0106/E2E-016: Given no converter, When Alice creates A and B, makes A the
/// default, gives `proj` the converter B, Bob converts `/report.pdf`, Alice clears
/// the project converter, Bob converts `/other.pdf`, Alice clears the default and
/// deletes B, Then the texts are `# From B` then `# From A`, and B is deleted only
/// once the project converter was cleared.
#[tokio::test]
async fn e2e_0106_016_a_converter_set_up_end_to_end() {
    let fx = Fx::new().await;
    assert!(fx.converters().await.is_empty());
    let a = fx.stand_in_converter("Pandoc local", "A").await;
    let b = fx.stand_in_converter("Docling API", "B").await;
    fx.set_default_converter_as(ALICE, &a).await.unwrap();
    fx.set_project_converter_as(ALICE, "proj", &b).await.unwrap();
    fx.bob_writes_at("proj", "/report.pdf", false).await.unwrap();
    assert_eq!(fx.read_proj("/report.md").await.unwrap().trim(), "# From B");
    let e = fx.delete_converter_as(ALICE, &b).await.unwrap_err();
    assert_eq!(e, "ERR_INVALID_ARGUMENT: converter 'Docling API' is used by 1 project(s)");
    fx.clear_project_converter_as(ALICE, "proj").await.unwrap();
    fx.bob_writes_at("proj", "/other.pdf", false).await.unwrap();
    assert_eq!(fx.read_proj("/other.md").await.unwrap().trim(), "# From A");
    fx.clear_default_converter_as(ALICE).await.unwrap();
    fx.delete_converter_as(ALICE, &b).await.unwrap();
    assert_eq!(names(&fx.converters().await), vec!["Pandoc local"]);
}

/// SPEC-0106/E2E-105: Given a platform whose admin database predates the
/// converter tables and the project column, When it starts on the new version
/// and Alice lists converters and changes, Then both are empty and the upgrade
/// wrote nothing.
#[tokio::test]
async fn e2e_0106_105_an_upgrade_leaves_the_registry_and_the_record_empty() {
    use crate::storage::rel::{Query, RelationalDb as _, SqliteRelationalDb};
    let fx = Fx::new().await;
    let db = SqliteRelationalDb::open(fx.dir.path().join("admin.db")).unwrap();
    for sql in [
        "DROP TABLE converter",
        "DROP TABLE converter_default",
        "ALTER TABLE project DROP COLUMN converter_id",
    ] {
        db.execute(&Query::new(sql)).await.unwrap();
    }
    fx.state.admin.connect().await.unwrap();
    assert!(fx.converters().await.is_empty());
    assert!(fx.changes().await.is_empty());
    assert_eq!(fx.get_default_converter_as(ALICE).await.unwrap(), json!({"converter": null}));
    assert_eq!(
        fx.get_project_converter_as(ALICE, "proj").await.unwrap(),
        json!({"project_id": "proj", "converter": null})
    );
}
