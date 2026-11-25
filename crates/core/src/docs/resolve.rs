//! The document service of a registry converter (SPEC-0106 FR-NEW-010, FR-NEW-013):
//! a stored row becomes the same `CliDocService` or `ApiDocService` the
//! configuration file builds, so the answer cap, the sandbox, the failure report
//! and the refusal order stay the ones of the file converter.
//!
//! Built from the row at every conversion, never cached, so a change applies to
//! the next conversion without a restart (FR-NEW-011).

use super::service::{ApiDocService, CliDocService, Conversion, DocService, resolve_extensions};
use crate::config::{DocServiceApiConfig, DocServiceCliConfig, Secret};
use crate::errors::{Result, ToolError};
use crate::settings::SecretText;
use crate::settings::converter::{Converter, FORM_HTTP};
use async_trait::async_trait;
use std::sync::Arc;

/// The service of `converter`. `secret` is the decrypted secret, `None` when the
/// converter has none or when it cannot be read; the second case answers every
/// conversion with the refusal of the spec rather than failing the lookup.
pub(crate) fn build(
    converter: &Converter,
    secret: Option<SecretText>,
) -> Result<Arc<dyn DocService>> {
    let extensions = resolve_extensions(&converter.extensions);
    // The registry keeps both limits positive; a negative value read back from a
    // damaged row refuses every input rather than lifting the cap.
    let max_input_bytes = u64::try_from(converter.max_input_bytes).unwrap_or(0);
    let timeout_secs = u64::try_from(converter.timeout_seconds).unwrap_or(0);
    if converter.has_secret && secret.is_none() {
        return Ok(Arc::new(BrokenSecretService {
            name: converter.name.clone(),
            extensions,
            max_input_bytes,
        }));
    }
    if converter.form == FORM_HTTP {
        let defaults = DocServiceApiConfig::default();
        let config = DocServiceApiConfig {
            url: converter.url.clone().unwrap_or_default(),
            auth_header: converter.auth_header.clone().unwrap_or(defaults.auth_header),
            auth_token: Secret::new(secret.map(|s| s.expose().to_string()).unwrap_or_default()),
            file_field: converter.file_field.clone().unwrap_or(defaults.file_field),
            response_field: converter.response_field.clone().unwrap_or_default(),
            timeout_secs,
        };
        let service = ApiDocService::new(&config, extensions, max_input_bytes)?;
        return Ok(Arc::new(service));
    }
    let config = DocServiceCliConfig {
        command: converter.command.clone().unwrap_or_default(),
        timeout_secs,
    };
    Ok(Arc::new(CliDocService::new(config, extensions, max_input_bytes)))
}

/// A converter whose secret cannot be read: the eligibility and size checks run
/// as for any converter, the conversion itself is refused by the converter name.
struct BrokenSecretService {
    name: String,
    extensions: Vec<String>,
    max_input_bytes: u64,
}

impl BrokenSecretService {
    /// The one text of both entry points; it names the converter, never a secret.
    fn refusal(&self) -> ToolError {
        ToolError::internal(format!("the secret of converter '{}' cannot be read", self.name))
    }
}

#[async_trait]
impl DocService for BrokenSecretService {
    async fn convert(&self, _bytes: &[u8], _file_name: &str) -> Result<Conversion> {
        Err(self.refusal())
    }

    fn preflight(&self) -> Result<()> {
        Err(self.refusal())
    }

    fn accepted_extensions(&self) -> &[String] {
        &self.extensions
    }

    fn max_input_bytes(&self) -> u64 {
        self.max_input_bytes
    }
}
