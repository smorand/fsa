//! Platform settings in the database (SPEC-0102): the providers and models of
//! the model registry with their write only secrets, the server defaults, and
//! the record of changes.
//!
//! `store` owns the SQL on the admin database, `registry` every rule, `secrets`
//! the master key and the secret carrier. Tools call [`Registry`] only.

pub mod model;
pub mod registry;
pub mod secrets;
pub mod store;

#[cfg(test)]
mod e2e;

pub use model::{
    Change, Defaults, Model, ModelInput, ModelUpdate, Page, Provider, ProviderInput, ProviderUpdate,
};
pub use registry::{Registry, RegistryTargets};
pub use secrets::SecretText;
pub use store::SettingsStore;
