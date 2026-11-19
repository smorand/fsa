//! `doc.*` tools: document conversion via pandoc.
//!
//! The family is optional: `register` is a no-op when pandoc is not found in PATH (or at
//! the configured bin path). A warning is logged so the operator knows why the tool is
//! absent.

use crate::config::DocConfig;
use crate::errors::{Result, ToolError};
use crate::safety::SafetyManager;
use crate::storage::VolumeClient;
use crate::tools::registry_support::ToolSchema;
use crate::tools::registry_support::{ToolRegistry, handler};
use crate::tools::{norm, volume};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;

const DOCX_DESC: &str = "Convert a Markdown or HTML file in the volume to a .docx Word \
    document using pandoc. Supply a .docx template via template_path to apply custom \
    styles, headers, and footers.";

const PPTX_DESC: &str = "Convert a Markdown or HTML file in the volume to a .pptx \
    PowerPoint presentation using pandoc. In Markdown, --- (horizontal rule) separates \
    slides and # headings set the slide title. Supply a .pptx template via template_path \
    to apply custom themes and layouts.";

/// Pandoc binary path + timeout bundled to stay within clippy's argument limit.
struct PandocCtx {
    bin: String,
    timeout_secs: u64,
}

/// Attempt to resolve the pandoc binary path from the config or from PATH.
fn resolve_pandoc(config: &DocConfig) -> Option<String> {
    if !config.pandoc_bin.is_empty() {
        return Some(config.pandoc_bin.clone());
    }
    which::which("pandoc").ok().map(|p| p.display().to_string())
}

/// Catalog-only, `ToolRegistry`-free view of the `doc.*` family. Mirrors
/// `register`'s own conditional: empty when pandoc cannot be resolved.
pub fn catalog(config: &DocConfig) -> Vec<crate::tools::catalog::ToolCatalogEntry> {
    let mut reg = ToolRegistry::new();
    register(&mut reg, config);
    reg.names()
        .iter()
        .map(|n| crate::tools::catalog::ToolCatalogEntry::from(&reg.resolve(n).unwrap().schema))
        .collect()
}

/// Register the `doc.*` family. Returns immediately without adding any tool if
/// pandoc cannot be found, logging a warning for the operator.
pub fn register(reg: &mut ToolRegistry, config: &DocConfig) {
    let Some(bin) = resolve_pandoc(config) else {
        tracing::warn!(
            "doc.enabled=true but pandoc not found in PATH \
             (set doc.pandoc_bin in config to point at it explicitly): \
             doc.to_docx and doc.to_pptx will not be registered"
        );
        return;
    };
    let pandoc = Arc::new(PandocCtx { bin, timeout_secs: config.pandoc_timeout_secs });

    // ── doc.to_docx ──────────────────────────────────────────────────────────
    reg.add(
        ToolSchema::new("doc.to_docx", DOCX_DESC)
            .req_str("mount_id", "Project/volume id the operation targets.")
            .req_str("src_path", "Absolute POSIX path of the Markdown or HTML source file.")
            .req_str("dst_path", "Absolute POSIX path of the .docx file to write.")
            .opt_str_null(
                "template_path",
                "Absolute POSIX path of a .docx template in the volume (optional).",
            )
            .opt_bool(
                "overwrite",
                false,
                "Allow overwriting an existing file (default no-clobber).",
            )
            .read_only(false)
            .destructive(true)
            .idempotent(true)
            .open_world(true),
        handler({
            let pandoc = pandoc.clone();
            move |ctx, a| {
                let pandoc = pandoc.clone();
                async move {
                    let (mount, client) = volume(&ctx, &a).await?;
                    let src_path = norm(&ctx, &a, "src_path")?;
                    let dst_path = norm(&ctx, &a, "dst_path")?;
                    let template_path = opt_norm(&ctx, &a, "template_path")?;
                    let overwrite = a.bool_or("overwrite", false);
                    convert(
                        &WriteCtx { person: &ctx.person, mount: &mount, safety: &ctx.state.safety },
                        &client,
                        &pandoc,
                        &ConvertPaths {
                            src_path: &src_path,
                            dst_path: &dst_path,
                            out_ext: "docx",
                            template_path: template_path.as_deref(),
                            overwrite,
                        },
                    )
                    .await
                }
            }
        }),
    );

    // ── doc.to_pptx ──────────────────────────────────────────────────────────
    reg.add(
        ToolSchema::new("doc.to_pptx", PPTX_DESC)
            .req_str("mount_id", "Project/volume id the operation targets.")
            .req_str("src_path", "Absolute POSIX path of the Markdown or HTML source file.")
            .req_str("dst_path", "Absolute POSIX path of the .pptx file to write.")
            .opt_str_null(
                "template_path",
                "Absolute POSIX path of a .pptx template in the volume (optional).",
            )
            .opt_bool(
                "overwrite",
                false,
                "Allow overwriting an existing file (default no-clobber).",
            )
            .read_only(false)
            .destructive(true)
            .idempotent(true)
            .open_world(true),
        handler({
            let pandoc = pandoc.clone();
            move |ctx, a| {
                let pandoc = pandoc.clone();
                async move {
                    let (mount, client) = volume(&ctx, &a).await?;
                    let src_path = norm(&ctx, &a, "src_path")?;
                    let dst_path = norm(&ctx, &a, "dst_path")?;
                    let template_path = opt_norm(&ctx, &a, "template_path")?;
                    let overwrite = a.bool_or("overwrite", false);
                    convert(
                        &WriteCtx { person: &ctx.person, mount: &mount, safety: &ctx.state.safety },
                        &client,
                        &pandoc,
                        &ConvertPaths {
                            src_path: &src_path,
                            dst_path: &dst_path,
                            out_ext: "pptx",
                            template_path: template_path.as_deref(),
                            overwrite,
                        },
                    )
                    .await
                }
            }
        }),
    );
}

/// Normalize an optional path parameter (returns `None` when the arg is absent/null).
fn opt_norm(
    ctx: &crate::tools::registry_support::ToolCtx,
    a: &crate::tools::registry_support::Args,
    key: &str,
) -> Result<Option<String>> {
    match a.opt_str(key) {
        Some(p) => Ok(Some(ctx.state.safety.normalize_path(&p)?)),
        None => Ok(None),
    }
}

/// Caller identity and safety context bundled to stay within clippy's argument limit.
struct WriteCtx<'a> {
    person: &'a str,
    mount: &'a str,
    safety: &'a Arc<SafetyManager>,
}

/// Source/destination paths and output format, bundled to stay within clippy's argument limit.
struct ConvertPaths<'a> {
    src_path: &'a str,
    dst_path: &'a str,
    /// Expected output extension: `"docx"` or `"pptx"`.
    out_ext: &'a str,
    template_path: Option<&'a str>,
    overwrite: bool,
}

/// Core conversion shared by `doc.to_docx` and `doc.to_pptx`.
///
/// Pandoc infers the writer from the output file extension, so no `--to` flag is needed.
async fn convert(
    ctx: &WriteCtx<'_>,
    client: &Arc<VolumeClient>,
    pandoc: &PandocCtx,
    paths: &ConvertPaths<'_>,
) -> Result<serde_json::Value> {
    let (person, mount, safety) = (ctx.person, ctx.mount, ctx.safety);
    let ConvertPaths { src_path, dst_path, out_ext, template_path, overwrite } = paths;

    // 1. Extension checks.
    validate_dst_ext(dst_path, out_ext)?;
    validate_src_ext(src_path)?;

    // 2. Read source from the volume.
    let src_bytes = client
        .read_bytes(src_path)
        .await
        .map_err(|_| ToolError::not_found(format!("source file not found: {src_path}")))?;

    // 3. Optionally read the template from the volume.
    let template_bytes = match template_path {
        Some(tp) => {
            let b = client
                .read_bytes(tp)
                .await
                .map_err(|_| ToolError::not_found(format!("template file not found: {tp}")))?;
            Some(b)
        }
        None => None,
    };

    // 4. No-clobber check.
    let already_exists = client.exists(dst_path).await.unwrap_or(false);
    if already_exists && !overwrite {
        return Err(ToolError::no_clobber(format!(
            "file already exists (set overwrite=true to replace): {dst_path}"
        )));
    }
    if already_exists {
        safety.record_read(person, mount, dst_path);
        safety.ensure_read_before_write(person, mount, dst_path)?;
    }

    // 5. Build a temp directory and write input files.
    let src_ext =
        std::path::Path::new(src_path).extension().and_then(|e| e.to_str()).unwrap_or("md");
    let tmpdir = tempfile::tempdir()
        .map_err(|e| ToolError::internal(format!("cannot create temp directory: {e}")))?;
    let input_path = tmpdir.path().join(format!("input.{src_ext}"));
    let output_path = tmpdir.path().join(format!("output.{out_ext}"));

    std::fs::write(&input_path, &src_bytes)
        .map_err(|e| ToolError::internal(format!("cannot write temp input: {e}")))?;

    let template_arg: Option<std::path::PathBuf> = match template_bytes.as_ref() {
        Some(b) => {
            let tp = tmpdir.path().join(format!("template.{out_ext}"));
            std::fs::write(&tp, b)
                .map_err(|e| ToolError::internal(format!("cannot write temp template: {e}")))?;
            Some(tp)
        }
        None => None,
    };

    // 6. Run pandoc.
    let out_bytes = run_pandoc(pandoc, &input_path, &output_path, template_arg.as_deref()).await?;

    // 7. Write into the volume.
    client
        .write_bytes_atomic(dst_path, &out_bytes)
        .await
        .map_err(|e| ToolError::internal(format!("failed to write output to volume: {e}")))?;

    // 8. Accounting.
    safety.record_read(person, mount, src_path);
    let n = out_bytes.len();
    safety.charge_write(person, mount, n as i64)?;
    let op = format!("doc.to_{out_ext}");
    safety.record_audit(person, mount, &op, dst_path, &format!("{n} bytes"));

    Ok(json!({
        "path": dst_path,
        "bytes_written": n,
        "overwritten": already_exists,
    }))
}

/// Invoke pandoc and return the produced bytes.
///
/// Pandoc runs with `--sandbox` (pandoc 2.15 or later): its readers and writers then
/// touch only the files named on the command line. The source is member authored, so
/// without it an image or frame it names would be fetched from any address (loopback,
/// private network, cloud metadata) and a named host file would be embedded in the
/// output the member owns. An older pandoc rejects the option, so the conversion
/// fails closed.
///
/// The output format is inferred from `output`'s extension. `tmpdir` is kept alive
/// by the caller; this function only needs the concrete file paths.
async fn run_pandoc(
    pandoc: &PandocCtx,
    input: &std::path::Path,
    output: &std::path::Path,
    template: Option<&std::path::Path>,
) -> Result<Vec<u8>> {
    let mut cmd = tokio::process::Command::new(&pandoc.bin);
    cmd.arg("--sandbox").arg(input).arg("-o").arg(output);
    if let Some(tp) = template {
        cmd.arg(format!("--reference-doc={}", tp.display()));
    }
    cmd.stdout(std::process::Stdio::null()).stderr(std::process::Stdio::piped());

    let child =
        cmd.spawn().map_err(|e| ToolError::internal(format!("cannot start pandoc: {e}")))?;

    let out =
        tokio::time::timeout(Duration::from_secs(pandoc.timeout_secs), child.wait_with_output())
            .await
            .map_err(|_| {
                ToolError::internal(format!("pandoc timed out after {}s", pandoc.timeout_secs))
            })?
            .map_err(|e| ToolError::internal(format!("pandoc I/O error: {e}")))?;

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(ToolError::internal(format!("pandoc exited with {}: {stderr}", out.status)));
    }

    let bytes = std::fs::read(output)
        .map_err(|e| ToolError::internal(format!("cannot read pandoc output: {e}")))?;

    if bytes.len() < 2 || &bytes[..2] != b"PK" {
        return Err(ToolError::internal(format!(
            "pandoc output is not a valid .{} (missing ZIP magic PK)",
            output.extension().and_then(|e| e.to_str()).unwrap_or("?")
        )));
    }
    Ok(bytes)
}

fn validate_dst_ext(path: &str, expected: &str) -> Result<()> {
    if std::path::Path::new(path).extension().and_then(|e| e.to_str()) != Some(expected) {
        return Err(ToolError::invalid_argument(format!("path must end with .{expected}")));
    }
    Ok(())
}

fn validate_src_ext(path: &str) -> Result<()> {
    let ext = std::path::Path::new(path).extension().and_then(|e| e.to_str()).unwrap_or("");
    match ext {
        "md" | "markdown" | "html" | "htm" => Ok(()),
        _ => Err(ToolError::invalid_argument(
            "src_path must be a .md, .markdown, .html or .htm file".to_string(),
        )),
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::code;
    use crate::tools::testkit::{
        MOUNT, assert_description, assert_family, assert_schema, harness_with_extra,
    };
    use serde_json::json;

    fn register_with_pandoc(reg: &mut ToolRegistry) {
        let Some(bin) = which::which("pandoc").ok().map(|p| p.display().to_string()) else {
            eprintln!("skipped: pandoc not found in PATH");
            return;
        };
        register(reg, &DocConfig { enabled: true, pandoc_bin: bin, ..DocConfig::default() });
    }

    fn register_with_pandoc_extra(reg: &mut ToolRegistry, _cfg: &crate::config::ServerConfig) {
        register_with_pandoc(reg);
    }

    fn pandoc_available() -> bool {
        which::which("pandoc").is_ok()
    }

    // ── family / schema / description ─────────────────────────────────────────

    #[test]
    fn family_registers_every_tool() {
        if !pandoc_available() {
            eprintln!("skipped: pandoc not found");
            return;
        }
        assert_family(register_with_pandoc, &["doc.to_docx", "doc.to_pptx"]);
    }

    #[test]
    fn doc_to_docx_schema_matches_contract() {
        if !pandoc_available() {
            eprintln!("skipped: pandoc not found");
            return;
        }
        assert_schema(
            register_with_pandoc,
            "doc.to_docx",
            r#"{"type":"object","properties":{
                 "mount_id":{"description":"Project/volume id the operation targets.","type":"string"},
                 "src_path":{"description":"Absolute POSIX path of the Markdown or HTML source file.","type":"string"},
                 "dst_path":{"description":"Absolute POSIX path of the .docx file to write.","type":"string"},
                 "template_path":{"description":"Absolute POSIX path of a .docx template in the volume (optional).","type":"string","default":null},
                 "overwrite":{"description":"Allow overwriting an existing file (default no-clobber).","type":"boolean","default":false}},
               "required":["mount_id","src_path","dst_path"]}"#,
        );
    }

    #[test]
    fn doc_to_docx_description_matches_contract() {
        if !pandoc_available() {
            eprintln!("skipped: pandoc not found");
            return;
        }
        assert_description(register_with_pandoc, "doc.to_docx", DOCX_DESC);
    }

    #[test]
    fn doc_to_pptx_schema_matches_contract() {
        if !pandoc_available() {
            eprintln!("skipped: pandoc not found");
            return;
        }
        assert_schema(
            register_with_pandoc,
            "doc.to_pptx",
            r#"{"type":"object","properties":{
                 "mount_id":{"description":"Project/volume id the operation targets.","type":"string"},
                 "src_path":{"description":"Absolute POSIX path of the Markdown or HTML source file.","type":"string"},
                 "dst_path":{"description":"Absolute POSIX path of the .pptx file to write.","type":"string"},
                 "template_path":{"description":"Absolute POSIX path of a .pptx template in the volume (optional).","type":"string","default":null},
                 "overwrite":{"description":"Allow overwriting an existing file (default no-clobber).","type":"boolean","default":false}},
               "required":["mount_id","src_path","dst_path"]}"#,
        );
    }

    #[test]
    fn doc_to_pptx_description_matches_contract() {
        if !pandoc_available() {
            eprintln!("skipped: pandoc not found");
            return;
        }
        assert_description(register_with_pandoc, "doc.to_pptx", PPTX_DESC);
    }

    // ── doc.to_docx integration ───────────────────────────────────────────────

    #[tokio::test]
    async fn doc_to_docx_converts_markdown() {
        if !pandoc_available() {
            eprintln!("skipped: pandoc not found");
            return;
        }
        let h = harness_with_extra(register_with_pandoc_extra).await;
        h.seed("/doc.md", "# Title\n\nBody text.\n").await;
        let r = h
            .call(
                "doc.to_docx",
                json!({"mount_id": MOUNT, "src_path": "/doc.md", "dst_path": "/out.docx"}),
            )
            .await
            .unwrap();
        assert_eq!(r["path"], "/out.docx");
        assert_eq!(r["overwritten"], false);
        assert!(r["bytes_written"].as_i64().unwrap() > 0);
        assert_eq!(&h.client().await.read_bytes("/out.docx").await.unwrap()[..2], b"PK");
    }

    #[tokio::test]
    async fn doc_to_docx_converts_html() {
        if !pandoc_available() {
            eprintln!("skipped: pandoc not found");
            return;
        }
        let h = harness_with_extra(register_with_pandoc_extra).await;
        h.seed("/page.html", "<h1>Title</h1><p>Body.</p>").await;
        let r = h
            .call(
                "doc.to_docx",
                json!({"mount_id": MOUNT, "src_path": "/page.html", "dst_path": "/out.docx"}),
            )
            .await
            .unwrap();
        assert_eq!(r["path"], "/out.docx");
        assert_eq!(&h.client().await.read_bytes("/out.docx").await.unwrap()[..2], b"PK");
    }

    #[tokio::test]
    async fn doc_to_docx_no_clobber() {
        if !pandoc_available() {
            eprintln!("skipped: pandoc not found");
            return;
        }
        let h = harness_with_extra(register_with_pandoc_extra).await;
        h.seed("/doc.md", "# A\n").await;
        h.call(
            "doc.to_docx",
            json!({"mount_id": MOUNT, "src_path": "/doc.md", "dst_path": "/out.docx"}),
        )
        .await
        .unwrap();
        let err = h
            .call(
                "doc.to_docx",
                json!({"mount_id": MOUNT, "src_path": "/doc.md", "dst_path": "/out.docx"}),
            )
            .await
            .unwrap_err();
        assert_eq!(err.code, code::NO_CLOBBER);
    }

    #[tokio::test]
    async fn doc_to_docx_overwrite_allowed() {
        if !pandoc_available() {
            eprintln!("skipped: pandoc not found");
            return;
        }
        let h = harness_with_extra(register_with_pandoc_extra).await;
        h.seed("/doc.md", "# A\n").await;
        h.call(
            "doc.to_docx",
            json!({"mount_id": MOUNT, "src_path": "/doc.md", "dst_path": "/out.docx"}),
        )
        .await
        .unwrap();
        let r = h.call("doc.to_docx", json!({"mount_id": MOUNT, "src_path": "/doc.md", "dst_path": "/out.docx", "overwrite": true})).await.unwrap();
        assert_eq!(r["overwritten"], true);
    }

    #[tokio::test]
    async fn doc_to_docx_rejects_wrong_dst_ext() {
        if !pandoc_available() {
            eprintln!("skipped: pandoc not found");
            return;
        }
        let h = harness_with_extra(register_with_pandoc_extra).await;
        h.seed("/doc.md", "# A\n").await;
        let err = h
            .call(
                "doc.to_docx",
                json!({"mount_id": MOUNT, "src_path": "/doc.md", "dst_path": "/out.txt"}),
            )
            .await
            .unwrap_err();
        assert_eq!(err.code, code::INVALID_ARGUMENT);
        assert_eq!(err.message, "path must end with .docx");
    }

    #[tokio::test]
    async fn doc_to_docx_rejects_wrong_src_ext() {
        if !pandoc_available() {
            eprintln!("skipped: pandoc not found");
            return;
        }
        let h = harness_with_extra(register_with_pandoc_extra).await;
        h.seed("/data.csv", "a,b\n1,2\n").await;
        let err = h
            .call(
                "doc.to_docx",
                json!({"mount_id": MOUNT, "src_path": "/data.csv", "dst_path": "/out.docx"}),
            )
            .await
            .unwrap_err();
        assert_eq!(err.code, code::INVALID_ARGUMENT);
        assert!(err.message.contains("src_path must be a .md"));
    }

    #[tokio::test]
    async fn doc_to_docx_missing_src() {
        if !pandoc_available() {
            eprintln!("skipped: pandoc not found");
            return;
        }
        let h = harness_with_extra(register_with_pandoc_extra).await;
        let err = h
            .call(
                "doc.to_docx",
                json!({"mount_id": MOUNT, "src_path": "/nope.md", "dst_path": "/out.docx"}),
            )
            .await
            .unwrap_err();
        assert_eq!(err.code, code::NOT_FOUND);
    }

    #[tokio::test]
    async fn doc_to_docx_missing_template() {
        if !pandoc_available() {
            eprintln!("skipped: pandoc not found");
            return;
        }
        let h = harness_with_extra(register_with_pandoc_extra).await;
        h.seed("/doc.md", "# A\n").await;
        let err = h.call("doc.to_docx", json!({"mount_id": MOUNT, "src_path": "/doc.md", "dst_path": "/out.docx", "template_path": "/nope.docx"})).await.unwrap_err();
        assert_eq!(err.code, code::NOT_FOUND);
    }

    // ── doc.to_pptx integration ───────────────────────────────────────────────

    #[tokio::test]
    async fn doc_to_pptx_converts_markdown() {
        if !pandoc_available() {
            eprintln!("skipped: pandoc not found");
            return;
        }
        let h = harness_with_extra(register_with_pandoc_extra).await;
        h.seed("/deck.md", "# Slide 1\n\nContent.\n\n---\n\n# Slide 2\n\nMore content.\n").await;
        let r = h
            .call(
                "doc.to_pptx",
                json!({"mount_id": MOUNT, "src_path": "/deck.md", "dst_path": "/out.pptx"}),
            )
            .await
            .unwrap();
        assert_eq!(r["path"], "/out.pptx");
        assert_eq!(r["overwritten"], false);
        assert!(r["bytes_written"].as_i64().unwrap() > 0);
        assert_eq!(&h.client().await.read_bytes("/out.pptx").await.unwrap()[..2], b"PK");
    }

    #[tokio::test]
    async fn doc_to_pptx_converts_html() {
        if !pandoc_available() {
            eprintln!("skipped: pandoc not found");
            return;
        }
        let h = harness_with_extra(register_with_pandoc_extra).await;
        h.seed("/deck.html", "<h1>Slide 1</h1><p>Content.</p><h1>Slide 2</h1><p>More.</p>").await;
        let r = h
            .call(
                "doc.to_pptx",
                json!({"mount_id": MOUNT, "src_path": "/deck.html", "dst_path": "/out.pptx"}),
            )
            .await
            .unwrap();
        assert_eq!(r["path"], "/out.pptx");
        assert_eq!(&h.client().await.read_bytes("/out.pptx").await.unwrap()[..2], b"PK");
    }

    #[tokio::test]
    async fn doc_to_pptx_no_clobber() {
        if !pandoc_available() {
            eprintln!("skipped: pandoc not found");
            return;
        }
        let h = harness_with_extra(register_with_pandoc_extra).await;
        h.seed("/deck.md", "# A\n").await;
        h.call(
            "doc.to_pptx",
            json!({"mount_id": MOUNT, "src_path": "/deck.md", "dst_path": "/out.pptx"}),
        )
        .await
        .unwrap();
        let err = h
            .call(
                "doc.to_pptx",
                json!({"mount_id": MOUNT, "src_path": "/deck.md", "dst_path": "/out.pptx"}),
            )
            .await
            .unwrap_err();
        assert_eq!(err.code, code::NO_CLOBBER);
    }

    #[tokio::test]
    async fn doc_to_pptx_overwrite_allowed() {
        if !pandoc_available() {
            eprintln!("skipped: pandoc not found");
            return;
        }
        let h = harness_with_extra(register_with_pandoc_extra).await;
        h.seed("/deck.md", "# A\n").await;
        h.call(
            "doc.to_pptx",
            json!({"mount_id": MOUNT, "src_path": "/deck.md", "dst_path": "/out.pptx"}),
        )
        .await
        .unwrap();
        let r = h.call("doc.to_pptx", json!({"mount_id": MOUNT, "src_path": "/deck.md", "dst_path": "/out.pptx", "overwrite": true})).await.unwrap();
        assert_eq!(r["overwritten"], true);
    }

    #[tokio::test]
    async fn doc_to_pptx_rejects_wrong_dst_ext() {
        if !pandoc_available() {
            eprintln!("skipped: pandoc not found");
            return;
        }
        let h = harness_with_extra(register_with_pandoc_extra).await;
        h.seed("/deck.md", "# A\n").await;
        let err = h
            .call(
                "doc.to_pptx",
                json!({"mount_id": MOUNT, "src_path": "/deck.md", "dst_path": "/out.pdf"}),
            )
            .await
            .unwrap_err();
        assert_eq!(err.code, code::INVALID_ARGUMENT);
        assert_eq!(err.message, "path must end with .pptx");
    }

    #[tokio::test]
    async fn doc_to_pptx_rejects_wrong_src_ext() {
        if !pandoc_available() {
            eprintln!("skipped: pandoc not found");
            return;
        }
        let h = harness_with_extra(register_with_pandoc_extra).await;
        h.seed("/data.csv", "a,b\n1,2\n").await;
        let err = h
            .call(
                "doc.to_pptx",
                json!({"mount_id": MOUNT, "src_path": "/data.csv", "dst_path": "/out.pptx"}),
            )
            .await
            .unwrap_err();
        assert_eq!(err.code, code::INVALID_ARGUMENT);
        assert!(err.message.contains("src_path must be a .md"));
    }

    #[tokio::test]
    async fn doc_to_pptx_missing_src() {
        if !pandoc_available() {
            eprintln!("skipped: pandoc not found");
            return;
        }
        let h = harness_with_extra(register_with_pandoc_extra).await;
        let err = h
            .call(
                "doc.to_pptx",
                json!({"mount_id": MOUNT, "src_path": "/nope.md", "dst_path": "/out.pptx"}),
            )
            .await
            .unwrap_err();
        assert_eq!(err.code, code::NOT_FOUND);
    }

    #[tokio::test]
    async fn doc_to_pptx_missing_template() {
        if !pandoc_available() {
            eprintln!("skipped: pandoc not found");
            return;
        }
        let h = harness_with_extra(register_with_pandoc_extra).await;
        h.seed("/deck.md", "# A\n").await;
        let err = h.call("doc.to_pptx", json!({"mount_id": MOUNT, "src_path": "/deck.md", "dst_path": "/out.pptx", "template_path": "/nope.pptx"})).await.unwrap_err();
        assert_eq!(err.code, code::NOT_FOUND);
    }

    // ── SPEC-0098: the conversion reads no resource the source names ───────────

    /// A stand in that answers every request `404` and counts the connections, so a
    /// request made by pandoc shows up as a count read on a channel other than the
    /// tool answer. The serving thread blocks on `accept` and ends with the process.
    struct StandIn {
        port: u16,
        seen: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    }

    impl StandIn {
        fn start() -> Self {
            use std::io::Write;
            use std::sync::atomic::{AtomicUsize, Ordering};
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            let seen = std::sync::Arc::new(AtomicUsize::new(0));
            let counter = seen.clone();
            std::thread::spawn(move || {
                for mut conn in listener.incoming().flatten() {
                    counter.fetch_add(1, Ordering::SeqCst);
                    let _ = conn.write_all(b"HTTP/1.0 404 Not Found\r\nContent-Length: 0\r\n\r\n");
                }
            });
            Self { port, seen }
        }

        /// Connections accepted so far.
        fn accepted(&self) -> usize {
            self.seen.load(std::sync::atomic::Ordering::SeqCst)
        }
    }

    /// Entry names of a zip held in memory.
    fn zip_names(bytes: &[u8]) -> Vec<String> {
        let z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        z.file_names().filter_map(|n| n.ok().map(|n| n.into_owned())).collect()
    }

    /// Whether any entry of the zip holds `needle`.
    fn zip_contains(bytes: &[u8], needle: &[u8]) -> bool {
        use std::io::Read;
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        (0..z.len()).any(|i| {
            let mut data = Vec::new();
            z.by_index(i).unwrap().read_to_end(&mut data).unwrap();
            data.windows(needle.len()).any(|w| w == needle)
        })
    }

    async fn convert_ok(h: &crate::tools::testkit::Harness, tool: &str, src: &str, dst: &str) {
        h.call(tool, json!({"mount_id": MOUNT, "src_path": src, "dst_path": dst})).await.unwrap();
    }

    // BT-001: a remote image in the Markdown causes no request.
    #[tokio::test]
    async fn bt_0098_001_remote_image_in_markdown_makes_no_request() {
        if !pandoc_available() {
            eprintln!("skipped: pandoc not found");
            return;
        }
        let stand_in = StandIn::start();
        let h = harness_with_extra(register_with_pandoc_extra).await;
        h.seed("/a.md", &format!("![x](http://127.0.0.1:{}/a.png)\n", stand_in.port)).await;
        let r = h
            .call(
                "doc.to_docx",
                json!({"mount_id": MOUNT, "src_path": "/a.md", "dst_path": "/a.docx"}),
            )
            .await
            .unwrap();
        assert_eq!(r["path"], "/a.docx");
        assert_eq!(r["overwritten"], false);
        assert_eq!(stand_in.accepted(), 0, "pandoc connected to the stand in");
    }

    // BT-002: addresses named by HTML elements.
    #[tokio::test]
    async fn bt_0098_002_html_img_and_iframe_make_no_request() {
        if !pandoc_available() {
            eprintln!("skipped: pandoc not found");
            return;
        }
        let stand_in = StandIn::start();
        let h = harness_with_extra(register_with_pandoc_extra).await;
        let p = stand_in.port;
        h.seed(
            "/p.html",
            &format!(
                "<p>x</p><img src=\"http://127.0.0.1:{p}/h.png\"><iframe src=\"http://127.0.0.1:{p}/i\"></iframe>"
            ),
        )
        .await;
        convert_ok(&h, "doc.to_docx", "/p.html", "/p.docx").await;
        assert_eq!(stand_in.accepted(), 0, "pandoc connected to the stand in");
    }

    // BT-003: host files named by absolute path and by file: URL.
    #[tokio::test]
    async fn bt_0098_003_host_file_is_not_embedded() {
        if !pandoc_available() {
            eprintln!("skipped: pandoc not found");
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let secret = dir.path().join("SECRET.txt");
        std::fs::write(&secret, "SECRET-MARKER-0098").unwrap();
        let abs = secret.display().to_string();
        let h = harness_with_extra(register_with_pandoc_extra).await;
        h.seed("/f.md", &format!("![a]({abs})\n\n![f](file://{abs})\n")).await;
        convert_ok(&h, "doc.to_docx", "/f.md", "/f.docx").await;
        let out = h.client().await.read_bytes("/f.docx").await.unwrap();
        let media: Vec<_> =
            zip_names(&out).into_iter().filter(|n| n.starts_with("word/media/")).collect();
        assert!(media.is_empty(), "host file embedded as {media:?}");
        assert!(!zip_contains(&out, b"SECRET-MARKER-0098"));
    }

    // BT-004: a remote image makes a pptx fail, with no request and no file.
    #[tokio::test]
    async fn bt_0098_004_pptx_remote_image_fails_without_request() {
        if !pandoc_available() {
            eprintln!("skipped: pandoc not found");
            return;
        }
        let stand_in = StandIn::start();
        let url = format!("http://127.0.0.1:{}/a.png", stand_in.port);
        let h = harness_with_extra(register_with_pandoc_extra).await;
        h.seed("/p.md", &format!("# T\n\n![x]({url})\n")).await;
        let err = h
            .call(
                "doc.to_pptx",
                json!({"mount_id": MOUNT, "src_path": "/p.md", "dst_path": "/p.pptx"}),
            )
            .await
            .unwrap_err();
        assert_eq!(err.code, code::INTERNAL_ERROR);
        let want =
            format!("pandoc exited with exit status: 99: File '{url}' not found in resource path");
        assert!(err.message.starts_with(&want), "got: {}", err.message);
        assert!(!h.client().await.exists("/p.pptx").await.unwrap());
        assert_eq!(stand_in.accepted(), 0, "pandoc connected to the stand in");
    }

    // BT-010: the docx keeps the alt text of an image it may not load.
    #[tokio::test]
    async fn bt_0098_010_docx_keeps_alt_text() {
        if !pandoc_available() {
            eprintln!("skipped: pandoc not found");
            return;
        }
        let h = harness_with_extra(register_with_pandoc_extra).await;
        h.seed("/a.md", "![ALT-0098](http://127.0.0.1:1/a.png)\n").await;
        let r = h
            .call(
                "doc.to_docx",
                json!({"mount_id": MOUNT, "src_path": "/a.md", "dst_path": "/a.docx"}),
            )
            .await
            .unwrap();
        assert!(r["bytes_written"].as_i64().unwrap() > 0);
        let out = h.client().await.read_bytes("/a.docx").await.unwrap();
        assert!(zip_contains(&out, b"ALT-0098"));
    }

    // BT-011: a failed pptx leaves the file it would have replaced.
    #[tokio::test]
    async fn bt_0098_011_failed_pptx_keeps_existing_file() {
        if !pandoc_available() {
            eprintln!("skipped: pandoc not found");
            return;
        }
        let h = harness_with_extra(register_with_pandoc_extra).await;
        h.seed("/p.md", "# T\n\n![x](http://127.0.0.1:1/a.png)\n").await;
        h.client().await.write_bytes_atomic("/p.pptx", b"PK-OLD-BYTES").await.unwrap();
        h.state.safety.record_read(crate::tools::testkit::PERSON, MOUNT, "/p.pptx");
        let err = h
            .call(
                "doc.to_pptx",
                json!({"mount_id": MOUNT, "src_path": "/p.md", "dst_path": "/p.pptx", "overwrite": true}),
            )
            .await
            .unwrap_err();
        assert_eq!(err.code, code::INTERNAL_ERROR);
        assert_eq!(h.client().await.read_bytes("/p.pptx").await.unwrap(), b"PK-OLD-BYTES");
    }

    // BT-020: a data: image is still embedded.
    #[tokio::test]
    async fn bt_0098_020_data_image_is_still_embedded() {
        if !pandoc_available() {
            eprintln!("skipped: pandoc not found");
            return;
        }
        let h = harness_with_extra(register_with_pandoc_extra).await;
        h.seed(
            "/d.md",
            "![d](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==)\n",
        )
        .await;
        convert_ok(&h, "doc.to_docx", "/d.md", "/d.docx").await;
        let out = h.client().await.read_bytes("/d.docx").await.unwrap();
        let media: Vec<_> = zip_names(&out)
            .into_iter()
            .filter(|n| n.starts_with("word/media/") && n.ends_with(".png"))
            .collect();
        assert_eq!(media.len(), 1, "got {media:?}");
    }

    // BT-021: plain sources and templates behave as before.
    #[tokio::test]
    async fn bt_0098_021_plain_and_templated_conversions_still_work() {
        if !pandoc_available() {
            eprintln!("skipped: pandoc not found");
            return;
        }
        let h = harness_with_extra(register_with_pandoc_extra).await;
        h.seed("/n.md", "# T\n\nhello\n").await;
        convert_ok(&h, "doc.to_docx", "/n.md", "/t.docx").await;
        convert_ok(&h, "doc.to_pptx", "/n.md", "/t.pptx").await;
        for (tool, ext) in [("doc.to_docx", "docx"), ("doc.to_pptx", "pptx")] {
            for template in [None, Some(format!("/t.{ext}"))] {
                let dst = format!("/o-{}.{ext}", template.is_some());
                let mut args = json!({"mount_id": MOUNT, "src_path": "/n.md", "dst_path": dst});
                if let Some(t) = &template {
                    args["template_path"] = json!(t);
                }
                let r = h.call(tool, args).await.unwrap();
                assert!(r["bytes_written"].as_i64().unwrap() > 0, "{tool} {template:?}");
                assert_eq!(&h.client().await.read_bytes(&dst).await.unwrap()[..2], b"PK");
            }
        }
    }
}
