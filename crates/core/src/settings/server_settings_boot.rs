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
                config = with_section(&config, section, full).map_err(|e| {
                    anyhow::anyhow!("the stored setting '{section}' is invalid: {}", e.message)
                })?;
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
            None if config.from_yaml_file => seeds.push(section),
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
