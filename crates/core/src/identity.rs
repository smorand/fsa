//! Identity: verified RS256 bearer JWT. Port of the C# `Identity/IdentityResolver.cs`.
//!
//! The token is read from the configured header (default
//! `X-Forwarded-Authorization`) and, failing that, the standard `Authorization`
//! header, so both a gateway and a direct client work. Signature, issuer and
//! expiry/nbf are verified against the configured RSA public key; the caller
//! identity comes from the `username_claim` claim (default `email`) and is
//! normalized caseless.

use crate::config::AuthConfig;
use crate::errors::{Result, ToolError};
use crate::util::normalize_identity;
use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use serde_json::Value;

pub struct IdentityResolver {
    /// The keys a token may be signed with, in the order they are tried: the stored
    /// list first, then the key file (SPEC-0116 FR-NEW-040).
    keys: Vec<DecodingKey>,
    header: String,
    issuer: Option<String>,
    audience: Option<String>,
    username_claim: String,
    /// Algorithms accepted for verification, from `auth.jwt.algorithms`. The
    /// reference parsed this key and never read it, hardcoding RS256, so a
    /// deployment that configured something else was silently ignored. An entry
    /// we cannot support is dropped at construction and reported by
    /// [`Self::unsupported_algorithms`] so startup can surface it.
    algorithms: Vec<Algorithm>,
    unsupported: Vec<String>,
}

/// Map a configured algorithm name to the verifier's enum. Only asymmetric
/// signatures are accepted: an HMAC family algorithm with a public key file would
/// let anyone holding that public key mint tokens, so `HS*` is refused on purpose.
fn parse_algorithm(name: &str) -> Option<Algorithm> {
    match name.trim().to_ascii_uppercase().as_str() {
        "RS256" => Some(Algorithm::RS256),
        "RS384" => Some(Algorithm::RS384),
        "RS512" => Some(Algorithm::RS512),
        "PS256" => Some(Algorithm::PS256),
        "PS384" => Some(Algorithm::PS384),
        "PS512" => Some(Algorithm::PS512),
        "ES256" => Some(Algorithm::ES256),
        "ES384" => Some(Algorithm::ES384),
        _ => None,
    }
}

impl IdentityResolver {
    /// Build from config: the keys of `auth.jwt.public_keys`, then the key of
    /// `auth.jwt.public_key_path`. No usable key yields a resolver that rejects every
    /// token. A listed key the token library cannot verify with (a modulus outside
    /// 2048 to 8192 bits) is left out: it verifies nothing, as before.
    pub fn new(auth: &AuthConfig) -> Self {
        let mut keys: Vec<DecodingKey> = auth
            .jwt
            .public_keys
            .iter()
            .filter_map(|entry| parse_rsa_public(entry))
            .filter(|(_, bits)| (2048..=8192).contains(bits))
            .map(|(pkcs1, _)| DecodingKey::from_rsa_der(&pkcs1))
            .collect();
        let path = &auth.jwt.public_key_path;
        if !path.is_empty() {
            let file =
                std::fs::read(path).ok().and_then(|pem| DecodingKey::from_rsa_pem(&pem).ok());
            match file {
                Some(k) => keys.push(k),
                // With a list in force the file is a second source that stopped working:
                // say so once, the list keeps serving (FR-NEW-025).
                None if !auth.jwt.public_keys.is_empty() => tracing::warn!(
                    "auth.jwt.public_key_path '{path}' cannot be read as an RSA public key: it is ignored"
                ),
                None => {}
            }
        }
        let mut algorithms = Vec::new();
        let mut unsupported = Vec::new();
        for name in &auth.jwt.algorithms {
            match parse_algorithm(name) {
                Some(a) if !algorithms.contains(&a) => algorithms.push(a),
                Some(_) => {}
                None => unsupported.push(name.clone()),
            }
        }
        // An empty or fully unsupported list would accept nothing, which is a
        // confusing way to fail; fall back to the documented default.
        if algorithms.is_empty() {
            algorithms.push(Algorithm::RS256);
        }
        Self {
            keys,
            header: auth.jwt.header.clone(),
            issuer: auth.jwt.issuer.clone(),
            audience: auth.jwt.audience.clone(),
            username_claim: auth.jwt.username_claim.clone(),
            algorithms,
            unsupported,
        }
    }

    /// Algorithm names from the config that this build cannot verify. Non empty
    /// means the operator wrote something that is being ignored, so the caller
    /// should log it loudly at startup rather than let it pass unnoticed.
    pub fn unsupported_algorithms(&self) -> &[String] {
        &self.unsupported
    }

    /// The algorithms actually accepted for verification.
    pub fn accepted_algorithms(&self) -> &[Algorithm] {
        &self.algorithms
    }

    /// Build directly from a PEM public key, for tests.
    pub fn from_pem(pem: &[u8], issuer: Option<&str>, username_claim: &str) -> Result<Self> {
        let key = DecodingKey::from_rsa_pem(pem)
            .map_err(|e| ToolError::internal(format!("invalid RSA public key: {e}")))?;
        Ok(Self {
            keys: vec![key],
            header: "X-Forwarded-Authorization".to_string(),
            issuer: issuer.map(str::to_string),
            audience: None,
            username_claim: username_claim.to_string(),
            algorithms: vec![Algorithm::RS256],
            unsupported: Vec::new(),
        })
    }

    /// The configured primary header name.
    pub fn header_name(&self) -> &str {
        &self.header
    }

    /// Extract the bearer token from the primary header, then `Authorization`.
    /// Also accepts Basic auth where the password is the token (git CLI compat).
    pub fn bearer_from_headers(&self, get: impl Fn(&str) -> Option<String>) -> Option<String> {
        for name in [self.header.as_str(), "Authorization"] {
            if let Some(raw) = get(name) {
                let t = raw.trim();
                if let Some(rest) = t.strip_prefix("Bearer ").or_else(|| t.strip_prefix("bearer "))
                {
                    return Some(rest.trim().to_string());
                }
                if let Some(b64) = t.strip_prefix("Basic ").or_else(|| t.strip_prefix("basic ")) {
                    use base64::Engine;
                    if let Ok(decoded) =
                        base64::engine::general_purpose::STANDARD.decode(b64.trim())
                        && let Ok(s) = String::from_utf8(decoded)
                    {
                        // "user:token" -> the password is the bearer
                        if let Some((_, pass)) = s.split_once(':')
                            && !pass.is_empty()
                        {
                            return Some(pass.to_string());
                        }
                    }
                }
                // A bare token with no scheme is accepted too.
                if !t.is_empty() && !t.contains(' ') {
                    return Some(t.to_string());
                }
            }
        }
        None
    }

    /// Verify a token and return the normalized caller identity.
    pub fn verify(&self, token: &str) -> Result<String> {
        if self.keys.is_empty() {
            return Err(ToolError::unauthenticated("no JWT public key configured"));
        }

        // Honour every configured algorithm, not just RS256.
        let primary = self.algorithms.first().copied().unwrap_or(Algorithm::RS256);
        let mut v = Validation::new(primary);
        v.algorithms = self.algorithms.clone();
        v.validate_exp = true;
        v.validate_nbf = true;
        // Match the C# TokenValidationParameters.ClockSkew of 30 seconds.
        v.leeway = 30;
        match &self.issuer {
            Some(iss) => v.set_issuer(&[iss]),
            None => v.iss = None,
        }
        match &self.audience {
            Some(aud) => v.set_audience(&[aud]),
            // The C# resolver does not require an audience unless configured.
            None => v.validate_aud = false,
        }

        // The first key that verifies the token serves. When none does, the error is
        // the first one that is not an invalid signature (an expiry says more than a
        // key that was never the signer's), else the first (FR-NEW-040).
        let mut errors = Vec::new();
        let mut decoded = None;
        for key in &self.keys {
            match jsonwebtoken::decode::<Value>(token, key, &v) {
                Ok(d) => {
                    decoded = Some(d);
                    break;
                }
                Err(e) => errors.push(e),
            }
        }
        let data = match decoded {
            Some(d) => d,
            None => {
                let pick = errors
                    .iter()
                    .position(|e| {
                        !matches!(e.kind(), jsonwebtoken::errors::ErrorKind::InvalidSignature)
                    })
                    .unwrap_or(0);
                return Err(ToolError::unauthenticated(format!("invalid token: {}", errors[pick])));
            }
        };

        let person =
            data.claims.get(&self.username_claim).and_then(Value::as_str).ok_or_else(|| {
                ToolError::unauthenticated(format!("token has no '{}' claim", self.username_claim))
            })?;
        if person.trim().is_empty() {
            return Err(ToolError::unauthenticated("token identity claim is empty"));
        }
        Ok(normalize_identity(person))
    }

    /// Header extraction + verification in one step.
    pub fn resolve(&self, get: impl Fn(&str) -> Option<String>) -> Result<String> {
        let token = self
            .bearer_from_headers(get)
            .ok_or_else(|| ToolError::unauthenticated("no bearer token in request headers"))?;
        self.verify(&token)
    }
}

/// The RSA public key an entry of `jwt.public_keys` holds, as the PKCS1 DER the token
/// library takes, with its modulus length in bits. `None` when the entry is not exactly
/// one PEM block of an RSA public key (`PUBLIC KEY` or `RSA PUBLIC KEY`) between blank
/// lines. The key is read from its DER, not through the PEM readers of the `rsa` crate,
/// which refuse a modulus over 4096 bits (SPEC-0116 D15).
pub(crate) fn parse_rsa_public(entry: &str) -> Option<(Vec<u8>, usize)> {
    use base64::Engine;
    use rsa::pkcs1::RsaPublicKey;
    use rsa::pkcs8::spki::SubjectPublicKeyInfoRef;
    let blank = |l: &str| l.trim_matches([' ', '\t', '\r']).is_empty();
    let lines: Vec<&str> =
        entry.lines().map(|l| l.strip_suffix('\r').unwrap_or(l)).filter(|l| !blank(l)).collect();
    let (first, last) = (lines.first()?, lines.last()?);
    let label = match *first {
        "-----BEGIN PUBLIC KEY-----" => "PUBLIC KEY",
        "-----BEGIN RSA PUBLIC KEY-----" => "RSA PUBLIC KEY",
        _ => return None,
    };
    if lines.len() < 3 || *last != format!("-----END {label}-----") {
        return None;
    }
    let body: String = lines[1..lines.len() - 1].concat();
    let der = base64::engine::general_purpose::STANDARD.decode(body.trim()).ok()?;
    let pkcs1: Vec<u8> = if label == "RSA PUBLIC KEY" {
        der
    } else {
        let info = SubjectPublicKeyInfoRef::try_from(der.as_slice()).ok()?;
        if info.algorithm.oid.to_string() != "1.2.840.113549.1.1.1" {
            return None;
        }
        info.subject_public_key.raw_bytes().to_vec()
    };
    let key = RsaPublicKey::try_from(pkcs1.as_slice()).ok()?;
    let modulus = key.modulus.as_bytes();
    let at = modulus.iter().position(|b| *b != 0)?;
    let bits = (modulus.len() - at - 1) * 8 + (8 - modulus[at].leading_zeros() as usize);
    Some((pkcs1, bits))
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{EncodingKey, Header, encode};
    use serde_json::json;

    /// A throwaway RSA keypair as PEM (private, public).
    fn keypair() -> (Vec<u8>, Vec<u8>) {
        use rsa::pkcs1::EncodeRsaPrivateKey;
        use rsa::pkcs8::EncodePublicKey;
        let mut rng = rand::thread_rng();
        let priv_key = rsa::RsaPrivateKey::new(&mut rng, 2048).unwrap();
        let pub_key = priv_key.to_public_key();
        let priv_pem = priv_key.to_pkcs1_pem(rsa::pkcs8::LineEnding::LF).unwrap();
        let pub_pem = pub_key.to_public_key_pem(rsa::pkcs8::LineEnding::LF).unwrap();
        (priv_pem.as_bytes().to_vec(), pub_pem.into_bytes())
    }

    fn mint(priv_pem: &[u8], claims: Value) -> String {
        let key = EncodingKey::from_rsa_pem(priv_pem).unwrap();
        encode(&Header::new(Algorithm::RS256), &claims, &key).unwrap()
    }

    fn now() -> i64 {
        chrono::Utc::now().timestamp()
    }

    #[test]
    fn verifies_a_valid_token_and_normalizes_identity() {
        let (pk, pubk) = keypair();
        let r = IdentityResolver::from_pem(&pubk, Some("web-a2a"), "email").unwrap();
        let t = mint(
            &pk,
            json!({"email": "Alice@Test.COM", "iss": "web-a2a", "exp": now() + 3600, "nbf": now() - 10}),
        );
        assert_eq!(r.verify(&t).unwrap(), "alice@test.com");
    }

    #[test]
    fn rejects_expired_token() {
        let (pk, pubk) = keypair();
        let r = IdentityResolver::from_pem(&pubk, Some("web-a2a"), "email").unwrap();
        let t = mint(
            &pk,
            json!({"email": "a@b.c", "iss": "web-a2a", "exp": now() - 60, "nbf": now() - 120}),
        );
        let e = r.verify(&t).unwrap_err();
        assert_eq!(e.code, crate::errors::code::UNAUTHENTICATED);
    }

    #[test]
    fn rejects_wrong_issuer() {
        let (pk, pubk) = keypair();
        let r = IdentityResolver::from_pem(&pubk, Some("web-a2a"), "email").unwrap();
        let t = mint(&pk, json!({"email": "a@b.c", "iss": "someone-else", "exp": now() + 60}));
        assert!(r.verify(&t).is_err());
    }

    #[test]
    fn rejects_token_signed_by_another_key() {
        let (_pk1, pubk1) = keypair();
        let (pk2, _pubk2) = keypair();
        let r = IdentityResolver::from_pem(&pubk1, Some("web-a2a"), "email").unwrap();
        let t = mint(&pk2, json!({"email": "a@b.c", "iss": "web-a2a", "exp": now() + 60}));
        assert!(r.verify(&t).is_err());
    }

    #[test]
    fn rejects_missing_identity_claim() {
        let (pk, pubk) = keypair();
        let r = IdentityResolver::from_pem(&pubk, Some("web-a2a"), "email").unwrap();
        let t = mint(&pk, json!({"sub": "no-email-here", "iss": "web-a2a", "exp": now() + 60}));
        let e = r.verify(&t).unwrap_err();
        assert!(e.message.contains("no 'email' claim"));
    }

    #[test]
    fn rejects_garbage_token() {
        let (_pk, pubk) = keypair();
        let r = IdentityResolver::from_pem(&pubk, None, "email").unwrap();
        assert!(r.verify("not.a.jwt").is_err());
        assert!(r.verify("").is_err());
    }

    #[test]
    fn extracts_bearer_from_primary_then_standard_header() {
        let (_pk, pubk) = keypair();
        let r = IdentityResolver::from_pem(&pubk, None, "email").unwrap();

        let get_primary =
            |n: &str| (n == "X-Forwarded-Authorization").then(|| "Bearer TOK1".to_string());
        assert_eq!(r.bearer_from_headers(get_primary).as_deref(), Some("TOK1"));

        let get_std = |n: &str| (n == "Authorization").then(|| "Bearer TOK2".to_string());
        assert_eq!(r.bearer_from_headers(get_std).as_deref(), Some("TOK2"));

        // primary wins when both are present
        let both = |n: &str| match n {
            "X-Forwarded-Authorization" => Some("Bearer FIRST".to_string()),
            "Authorization" => Some("Bearer SECOND".to_string()),
            _ => None,
        };
        assert_eq!(r.bearer_from_headers(both).as_deref(), Some("FIRST"));
    }

    #[test]
    fn extracts_token_from_basic_auth_password() {
        use base64::Engine;
        let (_pk, pubk) = keypair();
        let r = IdentityResolver::from_pem(&pubk, None, "email").unwrap();
        let b64 = base64::engine::general_purpose::STANDARD.encode("git:THETOKEN");
        let get = |n: &str| (n == "Authorization").then(|| format!("Basic {b64}"));
        assert_eq!(r.bearer_from_headers(get).as_deref(), Some("THETOKEN"));
    }

    #[test]
    fn no_headers_means_no_token() {
        let (_pk, pubk) = keypair();
        let r = IdentityResolver::from_pem(&pubk, None, "email").unwrap();
        assert!(r.bearer_from_headers(|_| None).is_none());
        let e = r.resolve(|_| None).unwrap_err();
        assert_eq!(e.code, crate::errors::code::UNAUTHENTICATED);
    }

    /// `auth.jwt.algorithms` must actually be honoured: the reference parsed it and
    /// hardcoded RS256, so a configured value was silently ignored.
    #[test]
    fn configured_algorithms_are_honoured() {
        let mut auth = AuthConfig::default();
        auth.jwt.algorithms = vec!["RS512".into(), "PS256".into()];
        let r = IdentityResolver::new(&auth);
        assert_eq!(r.accepted_algorithms(), &[Algorithm::RS512, Algorithm::PS256]);
        assert!(r.unsupported_algorithms().is_empty());
    }

    #[test]
    fn unsupported_algorithm_names_are_reported_not_swallowed() {
        let mut auth = AuthConfig::default();
        auth.jwt.algorithms = vec!["RS256".into(), "MAGIC512".into()];
        let r = IdentityResolver::new(&auth);
        assert_eq!(r.accepted_algorithms(), &[Algorithm::RS256]);
        assert_eq!(r.unsupported_algorithms(), &["MAGIC512".to_string()]);
    }

    /// An HMAC algorithm with a public key file would let anyone holding that public
    /// key mint tokens, so the whole HS family is refused rather than accepted.
    #[test]
    fn hmac_algorithms_are_refused() {
        let mut auth = AuthConfig::default();
        auth.jwt.algorithms = vec!["HS256".into()];
        let r = IdentityResolver::new(&auth);
        assert_eq!(r.unsupported_algorithms(), &["HS256".to_string()]);
        // and it falls back to the documented default rather than accepting nothing
        assert_eq!(r.accepted_algorithms(), &[Algorithm::RS256]);
    }

    #[test]
    fn an_empty_algorithm_list_falls_back_to_rs256() {
        let mut auth = AuthConfig::default();
        auth.jwt.algorithms = vec![];
        let r = IdentityResolver::new(&auth);
        assert_eq!(r.accepted_algorithms(), &[Algorithm::RS256]);
    }

    /// A token signed with RS256 must be rejected when the config only accepts RS512,
    /// which proves the list is enforced and not decorative.
    #[test]
    fn a_token_signed_with_an_unaccepted_algorithm_is_rejected() {
        let (pk, pubk) = keypair();
        let t = mint(&pk, json!({"email": "a@b.c", "iss": "web-a2a", "exp": now() + 3600}));

        let mut r = IdentityResolver::from_pem(&pubk, Some("web-a2a"), "email").unwrap();
        assert_eq!(r.verify(&t).unwrap(), "a@b.c", "RS256 accepted by default");

        // Now accept only RS512: the same RS256 token must fail.
        r.algorithms = vec![Algorithm::RS512];
        assert!(r.verify(&t).is_err(), "RS256 token must not pass an RS512 only policy");
    }

    #[test]
    fn resolver_without_key_rejects_everything() {
        let auth = AuthConfig::default(); // empty public_key_path
        let r = IdentityResolver::new(&auth);
        assert!(r.verify("anything").is_err());
    }

    // ── GROUP H: new identity tests ─────────────────────────────────────────

    /// A token whose exp is exactly at the skew boundary (now - 29) is still valid:
    /// the library adds `leeway` seconds to exp before comparing, so exp+30 > now.
    #[test]
    fn token_exactly_at_30s_skew_is_valid() {
        let (pk, pubk) = keypair();
        let r = IdentityResolver::from_pem(&pubk, Some("web-a2a"), "email").unwrap();
        // exp = now - 29: within the 30s skew window.
        let t = mint(
            &pk,
            json!({"email": "a@b.c", "iss": "web-a2a", "exp": now() - 29, "nbf": now() - 60}),
        );
        assert!(r.verify(&t).is_ok(), "token expiring 29s ago must be accepted within 30s skew");
    }

    /// A token whose exp is just beyond the skew boundary (now - 31) must be rejected.
    #[test]
    fn token_just_beyond_30s_skew_is_rejected() {
        let (pk, pubk) = keypair();
        let r = IdentityResolver::from_pem(&pubk, Some("web-a2a"), "email").unwrap();
        // exp = now - 31: just outside the 30s skew window.
        let t = mint(
            &pk,
            json!({"email": "a@b.c", "iss": "web-a2a", "exp": now() - 31, "nbf": now() - 60}),
        );
        assert!(r.verify(&t).is_err(), "token expiring 31s ago must be rejected (beyond 30s skew)");
    }

    /// A token with no `exp` claim must be rejected; we require expiry.
    #[test]
    fn token_without_exp_claim_is_rejected() {
        let (pk, pubk) = keypair();
        let r = IdentityResolver::from_pem(&pubk, Some("web-a2a"), "email").unwrap();
        // No `exp` field at all.
        let t = mint(&pk, json!({"email": "a@b.c", "iss": "web-a2a"}));
        let e = r.verify(&t).unwrap_err();
        assert_eq!(e.code, crate::errors::code::UNAUTHENTICATED);
    }
}
