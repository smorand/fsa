//! Platform settings in the database (SPEC-0102): the providers and models of
//! the model registry with their write only secrets, the server defaults, and
//! the record of changes.
//!
//! `store` owns the SQL on the admin database, `registry` every rule, `secrets`
//! the master key and the secret carrier. Tools call [`Registry`] only.
//!
//! SPEC-0107 adds the indexing profile of a project (`profile`, `profile_store`).
//!
//! SPEC-0106 adds the document converters to the same registry: `converter`
//! holds the shapes, `converter_rules` the input checks, `converter_store` the
//! SQL and `converters` the operations.

pub(crate) mod change_rules;
pub mod converter;
mod converter_rules;
mod converter_store;
pub(crate) mod converters;
mod infra_check;
mod infra_rules;
pub mod model;
pub mod profile;
mod profile_store;
pub mod registry;
pub mod secrets;
pub mod server_settings;
pub mod server_settings_boot;
mod server_settings_store;
pub mod store;

#[cfg(test)]
mod converters_e2e;
#[cfg(test)]
mod e2e;
#[cfg(test)]
pub(crate) mod infra_testkit;
#[cfg(test)]
mod key_fixtures;
#[cfg(test)]
mod profile_e2e;
#[cfg(all(test, feature = "postgres"))]
mod profile_pg_e2e;
#[cfg(all(test, feature = "rag"))]
mod search_e2e;
#[cfg(test)]
mod server_settings10_boot_e2e;
#[cfg(test)]
mod server_settings10_check_e2e;
#[cfg(test)]
mod server_settings10_e2e;
#[cfg(all(test, feature = "postgres"))]
mod server_settings10_pg_e2e;
#[cfg(test)]
mod server_settings3_boot_e2e;
#[cfg(test)]
mod server_settings3_e2e;
#[cfg(all(test, feature = "postgres"))]
mod server_settings3_pg_e2e;
#[cfg(test)]
mod server_settings4_e2e;
#[cfg(test)]
mod server_settings5_e2e;
#[cfg(test)]
mod server_settings6_e2e;
#[cfg(test)]
mod server_settings7_e2e;
#[cfg(test)]
mod server_settings8_e2e;
#[cfg(all(test, feature = "postgres"))]
mod server_settings9_pg_e2e;
#[cfg(test)]
mod server_settings_e2e;
#[cfg(all(test, feature = "postgres"))]
mod server_settings_pg_e2e;
#[cfg(all(test, feature = "rag"))]
pub(crate) mod vectors_e2e;

pub use converter::{Converter, ConverterInput};
pub use converters::page_json as converter_page_json;
pub use model::{
    Change, Defaults, Model, ModelInput, ModelUpdate, Page, Provider, ProviderInput, ProviderUpdate,
};
pub use profile::{ConfiguredModel, ProfileChange};
pub use profile_store::ProfileRow;
pub use registry::{Registry, RegistryTargets};
pub use secrets::SecretText;
pub use server_settings::ServerSettings;
pub use store::SettingsStore;
