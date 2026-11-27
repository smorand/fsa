//! The boot overlay of the settings store (SPEC-0114 FR-NEW-009 to 012, 015):
//! every section with a stored row replaces the section read from the YAML or
//! the defaults, and a configuration read from a YAML file seeds the sections
//! that have no row, once.

use super::registry::now;
use super::server_settings::{
    SECTIONS, decrypt_all, encrypt_all, join_secrets, section_json, split_secrets, with_section,
};
use super::server_settings_store::ServerSettingRow;
use super::store::SettingsStore;
use crate::config::ServerConfig;
use crate::storage::traits::AdminBackend;
use serde_json::Value;

/// The person a seeded row and its record name.
const SEEDER: &str = "system:yaml";

/// What a refused stored section starts with (SPEC-0114, SPEC-0116 FR-NEW-012).
const REFUSED: &str = "the stored settings are refused: ";

/// A key that names a variable holding a secret, as the start reads it.
struct SecretName {
    section: &'static str,
    dotted: &'static str,
    sink: &'static str,
}

const SECRET_NAMES: [SecretName; 5] = [
    SecretName { section: "git", dotted: "git.github_client_secret_env", sink: "GITHUB" },
    SecretName { section: "git", dotted: "git.gitlab_client_secret_env", sink: "GITLAB" },
    SecretName { section: "extract", dotted: "extract.ocr.api_key_env", sink: "OCR" },
    SecretName { section: "search", dotted: "search.embedding.api_key_env", sink: "EMBEDDING" },
    SecretName { section: "search", dotted: "search.reranking.api_key_env", sink: "RERANKING" },
];

impl SecretName {
    /// The name the configuration holds, `None` when the key is unset.
    fn read(&self, c: &ServerConfig) -> Option<String> {
        match self.dotted {
            "git.github_client_secret_env" => c.git.github_client_secret_env.clone(),
            "git.gitlab_client_secret_env" => Some(c.git.gitlab_client_secret_env.clone()),
            "extract.ocr.api_key_env" => c.extract.ocr.api_key_env.clone(),
            "search.embedding.api_key_env" => Some(c.search.embedding.api_key_env.clone()),
            _ => Some(c.search.reranking.api_key_env.clone()),
        }
    }

    /// The key as it reads when it was never set.
    fn unset(&self, c: &mut ServerConfig) {
        match self.dotted {
            "git.github_client_secret_env" => c.git.github_client_secret_env = None,
            "git.gitlab_client_secret_env" => {
                c.git.gitlab_client_secret_env =
                    crate::config::GitConfig::default().gitlab_client_secret_env
            }
            "extract.ocr.api_key_env" => c.extract.ocr.api_key_env = None,
            "search.embedding.api_key_env" => c.search.embedding.api_key_env = String::new(),
            _ => c.search.reranking.api_key_env = String::new(),
        }
    }

    /// A name that is neither empty, a master name, a default of the key nor in its
    /// secret namespace.
    fn outside(&self, name: &str) -> bool {
        !name.is_empty()
            && !crate::config::is_master_name(name)
            && !crate::config::secret_name_defaults(self.dotted).contains(&name)
            && !super::change_rules::in_secret_namespace(self.sink, name)
    }
}

/// Whether the YAML section `section` is imported at the first start (SPEC-0116
/// FR-NEW-020, FR-NEW-028, FR-NEW-031): an MCP path the router cannot mount and a bad
/// key stop the start with the bare message; a `server` section whose host or base URL
/// the rules refuse, and an `auth` section with no key, stay in force without being
/// imported, so a later edit of the file is read.
fn yaml_importable(section: &str, config: &ServerConfig) -> anyhow::Result<bool> {
    let bare = |e: crate::errors::ToolError| anyhow::anyhow!(e.message);
    match section {
        "server" => {
            config.check_mcp_path().map_err(bare)?;
            if let Err(e) = config.check_server_forms() {
                tracing::warn!("the YAML section 'server' is not imported: {}", e.message);
                return Ok(false);
            }
            Ok(true)
        }
        "auth" => {
            config.check_auth_keys().map_err(bare)?;
            if !config.auth.holds_key() {
                tracing::warn!("the YAML section 'auth' is not imported: it holds no key");
                return Ok(false);
            }
            Ok(true)
        }
        _ => Ok(true),
    }
}

/// `config` as the stored settings make it. An unreadable secret, a stored
/// value the sections no longer accept, or a result `validate()` refuses stops
/// the boot naming the section.
pub async fn apply(
    config: ServerConfig,
    admin: &dyn AdminBackend,
    key: Option<[u8; 32]>,
) -> anyhow::Result<ServerConfig> {
    let Some(store) = admin.settings() else { return Ok(config) };
    let rows = store.all_server_settings().await.map_err(|e| anyhow::anyhow!(e.message))?;
    let mut config = config;
    let mut seeds = Vec::new();
    let mut applied = false;
    let mut git_applied = false;
    for section in SECTIONS {
        match rows.iter().find(|r| r.section == section) {
            Some(row) => {
                let mut full: Value = serde_json::from_str(&row.value)
                    .map_err(|_| anyhow::anyhow!("the stored setting '{section}' is unreadable"))?;
                if let Some(blob) = &row.secret_enc {
                    let secrets = decrypt_all(key, blob).ok_or_else(|| {
                        anyhow::anyhow!(
                            "the stored setting '{section}' holds a secret that cannot be read: \
                             set FSA_TOKEN_KEY to the key it was stored with"
                        )
                    })?;
                    join_secrets(&mut full, &secrets);
                }
                let before = section_json(&config, section);
                // The names the configuration held before the stored section replaces it:
                // those of the YAML file in force (FR-NEW-034, FR-NEW-044).
                let yaml_names: Vec<Option<String>> = SECRET_NAMES
                    .iter()
                    .filter(|k| k.section == section)
                    .map(|k| k.read(&config))
                    .collect();
                if let Some(m) = super::change_rules::stored_wrong_type(section, &full) {
                    anyhow::bail!("the stored setting '{section}' is invalid: {m}");
                }
                config = with_section(&config, section, full).map_err(|e| {
                    anyhow::anyhow!("the stored setting '{section}' is invalid: {}", e.message)
                })?;
                // What a start judges of a stored `server` or `auth`, in the order of the
                // section list (FR-NEW-032).
                let judged = match section {
                    "server" => config.check_server_forms(),
                    "auth" => config.check_auth_keys(),
                    _ => Ok(()),
                };
                judged.map_err(|e| anyhow::anyhow!("{REFUSED}{}", e.message))?;
                // A stored name outside the secret namespace is not used, unless the YAML
                // file in force gives that very name (a master name is refused at use).
                for (k, yaml) in
                    SECRET_NAMES.iter().filter(|k| k.section == section).zip(&yaml_names)
                {
                    if let Some(n) = k.read(&config)
                        && k.outside(&n)
                        && yaml.as_ref() != Some(&n)
                    {
                        tracing::warn!(
                            "{} names '{n}' outside the secret namespace: the stored name is not used",
                            k.dotted
                        );
                        k.unset(&mut config);
                    }
                }
                config.note_stored_switch(section);
                applied = true;
                // Only a list that differs from the one the configuration held needs
                // publishing: a file start published its own at load.
                git_applied |=
                    section == "git" && before["hosts"] != section_json(&config, section)["hosts"];
                if config.from_yaml_file && before != section_json(&config, section) {
                    tracing::warn!(
                        section,
                        "the YAML file differs from the stored setting, the database value wins"
                    );
                }
            }
            None if config.from_yaml_file => {
                seeds.extend(yaml_importable(section, &config)?.then_some(section));
            }
            None => {}
        }
    }
    if applied {
        config
            .check()
            .map_err(|e| anyhow::anyhow!("the stored settings are refused: {}", e.message))?;
    }
    // The host map is what every remote operation resolves hosts with. A YAML start
    // published its own at load; a stored `git` section replaces it here, never from
    // `config.rs` (SPEC-0115 DEC-003). Only when the stored list differs from the one the
    // configuration held, so an unchanged list never races the map's other readers.
    if git_applied {
        crate::git::remote::validate_hosts(&config.git)
            .map_err(|e| anyhow::anyhow!("the stored settings are refused: {}", e.message))?;
    }
    if !seeds.is_empty() {
        // From the configuration without the command line flags, so a forced
        // switch is never stored.
        seed(&store, &config.without_flags(), &seeds, key).await;
    }
    // The names that stay in force: a master name is never read, any other name outside
    // the secret namespace is still read and the operator is told (FR-NEW-019, 024).
    for k in &SECRET_NAMES {
        let Some(n) = k.read(&config) else { continue };
        if crate::config::is_master_name(&n) {
            tracing::warn!(
                "{} names the reserved variable '{n}': the server will not read it as a secret",
                k.dotted
            );
        } else if k.outside(&n) {
            tracing::warn!(
                "{} names '{n}' outside the secret namespace: it is still read as a secret, prefer a name starting with FSA_SECRET_",
                k.dotted
            );
        }
    }
    config.apply_forced();
    Ok(config)
}

/// Imports the YAML sections in `seeds`; a failure is logged and never stops
/// the boot (another instance may have seeded first, the next start reads it).
async fn seed(store: &SettingsStore, config: &ServerConfig, seeds: &[&str], key: Option<[u8; 32]>) {
    let result: crate::errors::Result<()> = async {
        let mut tx = store.begin().await?;
        let at = now();
        for section in seeds {
            let mut full = section_json(config, section);
            let secrets = split_secrets(section, &mut full);
            let secret_enc = match (secrets.is_empty(), key) {
                (true, _) => None,
                (false, Some(k)) => Some(encrypt_all(&k, &secrets)?),
                (false, None) => {
                    tracing::warn!(
                        section = *section,
                        "the YAML section holds a secret and no master key is set, it stays in \
                         the file and is not imported"
                    );
                    continue;
                }
            };
            let row = ServerSettingRow {
                section: (*section).into(),
                value: full.to_string(),
                secret_enc,
                updated_at: at.clone(),
                updated_by: SEEDER.into(),
            };
            SettingsStore::put_server_setting(&mut tx, &row, false).await?;
            super::server_settings::record(
                &mut tx,
                SEEDER,
                "seed_setting",
                section,
                vec![],
                at.clone(),
            )
            .await?;
        }
        tx.commit().await
    }
    .await;
    if let Err(e) = result {
        tracing::warn!(error = %e.message, "the YAML settings were not imported");
    }
}
