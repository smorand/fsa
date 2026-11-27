//! The rules only a change of the `infra` section obeys (SPEC-0117 FR-NEW-004): what
//! the blob store needs to work. They are not run at the start, so a YAML file that
//! starts today starts the same (AUTO-068).

use crate::config::ServerConfig;
use crate::errors::{Result, ToolError};

fn required(key: &str, backend: &str) -> ToolError {
    ToolError::invalid_argument(format!(
        "infra.blob.{key} is required when infra.blob.backend is '{backend}'"
    ))
}

/// The blob block of `candidate`, in the order of the spec: a known backend, then what
/// the backend needs. Keys are judged trimmed.
pub(crate) fn blob_rules(candidate: &ServerConfig) -> Result<()> {
    let b = &candidate.infra.blob;
    match b.backend.as_str() {
        "local" => {
            if b.dir.trim().is_empty() {
                return Err(required("dir", "local"));
            }
        }
        backend @ ("minio" | "s3") => {
            if b.access_key.trim().is_empty() || b.secret_key.trim().is_empty() {
                return Err(ToolError::invalid_argument(format!(
                    "infra.blob.access_key and infra.blob.secret_key are required when \
                     infra.blob.backend is '{backend}'"
                )));
            }
            if backend == "minio" && b.endpoint.trim().is_empty() {
                return Err(required("endpoint", "minio"));
            }
        }
        other => {
            return Err(ToolError::invalid_argument(format!(
                "unknown infra.blob.backend '{other}', expected one of local, minio, s3"
            )));
        }
    }
    Ok(())
}
