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

pub mod converter;
mod converter_rules;
mod converter_store;
pub(crate) mod converters;
pub mod model;
pub mod profile;
mod profile_store;
pub mod registry;
pub mod secrets;
pub mod store;

#[cfg(test)]
mod converters_e2e;
#[cfg(test)]
mod e2e;
#[cfg(test)]
mod profile_e2e;

pub use converter::{Converter, ConverterInput};
pub use converters::page_json as converter_page_json;
pub use model::{
    Change, Defaults, Model, ModelInput, ModelUpdate, Page, Provider, ProviderInput, ProviderUpdate,
};
pub use profile::{ConfiguredModel, ProfileChange};
pub use profile_store::ProfileRow;
pub use registry::{Registry, RegistryTargets};
pub use secrets::SecretText;
pub use store::SettingsStore;
