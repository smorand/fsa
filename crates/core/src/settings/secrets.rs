//! The master key and the secret carrier (SPEC-0102 DEC-003, DEC-005).
//!
//! The key is the one the git token store uses (`FSA_TOKEN_KEY`, legacy name
//! through `env_compat`), encrypted with the same AES 256 GCM cipher, so an
//! operator manages one key for every secret of the platform.

use crate::errors::Result;
use crate::git::oauth::cipher;

/// A secret in memory. No `Display`, and a `Debug` that never prints the
/// value, so no log line, error text or answer can format it by accident.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct SecretText(String);

impl SecretText {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Counted in characters, the unit of the spec's limit.
    pub fn chars(&self) -> usize {
        self.0.chars().count()
    }

    /// The value, for the cipher only.
    pub(crate) fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for SecretText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.0.is_empty() { "Secret(empty)" } else { "Secret(set)" })
    }
}

/// The master key from `env`: `None` when the variable is unset or blank, the
/// cipher's refusal when it is not base64 of 32 bytes.
pub fn master_key_with(env: &dyn Fn(&str) -> Option<String>) -> Result<Option<[u8; 32]>> {
    match crate::git::oauth::store::token_key_with(env) {
        None => Ok(None),
        Some(raw) => cipher::decode_key(&raw).map(Some),
    }
}

/// The master key of this process (DEC-003).
pub fn master_key_from_env() -> Result<Option<[u8; 32]>> {
    master_key_with(&|name| std::env::var(name).ok())
}

pub(crate) fn encrypt(key: &[u8; 32], secret: &SecretText) -> Result<Vec<u8>> {
    cipher::encrypt(key, secret.expose())
}

/// The secret of `blob`; `None` when it cannot be read (no key, another key,
/// a damaged blob), so the caller words the refusal by the provider name alone.
pub(crate) fn decrypt(key: Option<[u8; 32]>, blob: &[u8]) -> Option<SecretText> {
    cipher::decrypt(&key?, blob).ok().map(SecretText::new)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    fn env_of(value: &'static str) -> impl Fn(&str) -> Option<String> {
        move |name| (name == "FSA_TOKEN_KEY").then(|| value.to_string())
    }

    #[test]
    fn the_debug_of_a_secret_never_shows_it() {
        let s = SecretText::new("sk-test-123");
        assert_eq!(format!("{s:?}"), "Secret(set)");
        assert_eq!(format!("{:?}", Some(s)), "Some(Secret(set))");
    }

    #[test]
    fn an_unset_or_blank_key_is_no_key() {
        assert_eq!(master_key_with(&|_| None).unwrap(), None);
        assert_eq!(master_key_with(&env_of("   ")).unwrap(), None);
    }

    #[test]
    fn a_valid_key_is_read_and_a_malformed_one_is_refused_with_the_cipher_message() {
        let good = base64::engine::general_purpose::STANDARD.encode([7u8; 32]);
        let good: &'static str = Box::leak(good.into_boxed_str());
        assert_eq!(master_key_with(&env_of(good)).unwrap(), Some([7u8; 32]));
        let e = master_key_with(&env_of("not-a-key")).unwrap_err();
        assert!(e.message.starts_with("FSA_TOKEN_KEY must be valid base64."), "{}", e.message);
        let short: &'static str =
            Box::leak(base64::engine::general_purpose::STANDARD.encode([1u8; 16]).into_boxed_str());
        let e = master_key_with(&env_of(short)).unwrap_err();
        assert!(e.message.starts_with("FSA_TOKEN_KEY must decode to 32 bytes"), "{}", e.message);
    }
}
