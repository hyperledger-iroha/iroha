//! Strict configuration admission for independently administered hardware stream-token trust.

use super::*;

const PREFIX: &str = "sorafs.storage.stream_tokens.hardware";
const MAX_CUSTODY_TIME_MS: u64 = 24 * 60 * 60 * 1_000;
const MAX_OBSERVER_AGE_MS: u64 = 300_000;

/// Complete public hardware signer inputs; every leaf is required when issuance is enabled.
#[derive(Debug, Default, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct SorafsStreamTokenHardwareConfig {
    /// Opaque `hsm`, `kms` or `pkcs11` runtime handle, without credentials.
    pub runtime_handle: Option<String>,
    /// Opaque `hsm`, `kms` or `pkcs11` key-generation handle, without credentials.
    pub key_handle: Option<String>,
    /// Public signer service identity.
    pub service_id: Option<String>,
    /// Independent signer administrator identity.
    pub administrator_id: Option<String>,
    /// Canonical lowercase strong Ed25519 public-key hex.
    pub public_key_hex: Option<String>,
    /// Sole key generation, within `1..=u32::MAX`.
    pub key_revision: Option<u64>,
    /// Nonzero signer policy generation.
    pub policy_revision: Option<u64>,
    /// Canonical lowercase nonzero signer policy digest.
    pub policy_digest_hex: Option<String>,
    /// Independently administered hardware attester trust.
    #[config(nested)]
    pub attester: SorafsStreamTokenAttesterConfig,
    /// Independently administered finalized-state observer trust.
    #[config(nested)]
    pub observer: SorafsStreamTokenObserverConfig,
}

/// Required public hardware-attestation authority and eligibility policy.
#[derive(Debug, Default, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct SorafsStreamTokenAttesterConfig {
    /// Public attester service identity.
    pub service_id: Option<String>,
    /// Independent attester administrator identity.
    pub administrator_id: Option<String>,
    /// Canonical lowercase strong Ed25519 attester public-key hex.
    pub public_key_hex: Option<String>,
    /// Nonzero attester key generation.
    pub key_revision: Option<u64>,
    /// Nonzero attester policy generation.
    pub policy_revision: Option<u64>,
    /// Canonical lowercase nonzero attester policy digest.
    pub policy_digest_hex: Option<String>,
    /// Inclusive eligibility start in Unix milliseconds.
    pub active_from_unix_ms: Option<u64>,
    /// Exclusive eligibility end in Unix milliseconds.
    pub active_until_unix_ms: Option<u64>,
    /// Maximum attested custody lifetime, at most one day in milliseconds.
    pub max_validity_ms: Option<u64>,
    /// Maximum current custody-anchor age, at most one day in milliseconds.
    pub max_anchor_age_ms: Option<u64>,
}

/// Required public finalized-state observer routing, authority and eligibility policy.
#[derive(Debug, Default, ReadConfig, Clone, norito::JsonDeserialize)]
pub struct SorafsStreamTokenObserverConfig {
    /// Credential-free production observer handle, never a software signer handle.
    pub runtime_handle: Option<String>,
    /// Public observer service identity.
    pub service_id: Option<String>,
    /// Independent observer administrator identity.
    pub administrator_id: Option<String>,
    /// Canonical lowercase strong Ed25519 observer public-key hex.
    pub public_key_hex: Option<String>,
    /// Nonzero observer key generation.
    pub key_revision: Option<u64>,
    /// Nonzero observer policy generation.
    pub policy_revision: Option<u64>,
    /// Canonical lowercase nonzero observer policy digest.
    pub policy_digest_hex: Option<String>,
    /// Inclusive eligibility start in Unix milliseconds.
    pub active_from_unix_ms: Option<u64>,
    /// Exclusive eligibility end in Unix milliseconds.
    pub active_until_unix_ms: Option<u64>,
    /// Maximum observation age and lifetime, at most 300,000 milliseconds.
    pub max_state_age_ms: Option<u64>,
}

impl SorafsStreamTokenHardwareConfig {
    /// Whether any hardware or trust leaf was explicitly configured.
    pub(super) fn is_configured(&self) -> bool {
        self.runtime_handle.is_some()
            || self.key_handle.is_some()
            || self.service_id.is_some()
            || self.administrator_id.is_some()
            || self.public_key_hex.is_some()
            || self.key_revision.is_some()
            || self.policy_revision.is_some()
            || self.policy_digest_hex.is_some()
            || self.attester.is_configured()
            || self.observer.is_configured()
    }

    pub(super) fn parse(
        &self,
        enabled: bool,
        emitter: &mut Emitter<ParseError>,
    ) -> Option<actual::SorafsStreamTokenHardwareConfig> {
        let mut parser = Parser { emitter };
        if !enabled {
            if self.is_configured() {
                parser.error(
                    PREFIX,
                    "runtime bindings are forbidden while issuance is disabled",
                );
            }
            return None;
        }
        if !self.is_configured() {
            parser.error(PREFIX, "is required when issuance is enabled");
            return None;
        }
        let runtime_handle = parser.text(
            self.runtime_handle.as_deref(),
            &path("runtime_handle"),
            hardware_handle,
            "must be a canonical credential-free production hardware handle",
        );
        let key_handle = parser.text(
            self.key_handle.as_deref(),
            &path("key_handle"),
            hardware_handle,
            "must be a canonical credential-free production hardware handle",
        );
        let service_id = parser.identity(self.service_id.as_deref(), &path("service_id"));
        let administrator_id =
            parser.identity(self.administrator_id.as_deref(), &path("administrator_id"));
        let public_key = parser.public_key(self.public_key_hex.as_deref(), &path("public_key_hex"));
        let key_revision = parser.number(
            self.key_revision,
            &path("key_revision"),
            u64::from(u32::MAX),
        );
        let policy_revision =
            parser.number(self.policy_revision, &path("policy_revision"), u64::MAX);
        let policy_digest = parser.digest(
            self.policy_digest_hex.as_deref(),
            &path("policy_digest_hex"),
        );
        let attester = self.attester.parse(&mut parser);
        let observer = self.observer.parse(&mut parser);
        let candidate = actual::SorafsStreamTokenHardwareConfig {
            runtime_handle: runtime_handle?,
            key_handle: key_handle?,
            service_id: service_id?,
            administrator_id: administrator_id?,
            public_key: public_key?,
            key_revision: key_revision?,
            policy_revision: policy_revision?,
            policy_digest: policy_digest?,
            attester: attester?,
            observer: observer?,
        };
        let ids = [
            candidate.service_id.as_str(),
            candidate.administrator_id.as_str(),
            candidate.attester.authority.service_id.as_str(),
            candidate.attester.authority.administrator_id.as_str(),
            candidate.observer.authority.service_id.as_str(),
            candidate.observer.authority.administrator_id.as_str(),
        ];
        if ids
            .iter()
            .enumerate()
            .any(|(index, identity)| ids[..index].contains(identity))
        {
            parser.error(PREFIX, "all six signer, attester and observer service/administrator identities must be distinct");
            return None;
        }
        let keys = [
            candidate.public_key,
            candidate.attester.authority.public_key,
            candidate.observer.authority.public_key,
        ];
        if keys
            .iter()
            .enumerate()
            .any(|(index, key)| keys[..index].contains(key))
        {
            parser.error(
                PREFIX,
                "signer, attester and observer public keys must be distinct",
            );
            return None;
        }
        let attester = &candidate.attester.authority;
        let observer = &candidate.observer.authority;
        if attester
            .active_from_unix_ms
            .max(observer.active_from_unix_ms)
            >= attester
                .active_until_unix_ms
                .min(observer.active_until_unix_ms)
        {
            parser.error(
                PREFIX,
                "attester and observer eligibility intervals must overlap",
            );
            return None;
        }
        Some(candidate)
    }
}

impl SorafsStreamTokenAttesterConfig {
    fn fields(&self) -> AuthorityFields<'_> {
        AuthorityFields {
            service_id: self.service_id.as_deref(),
            administrator_id: self.administrator_id.as_deref(),
            public_key_hex: self.public_key_hex.as_deref(),
            key_revision: self.key_revision,
            policy_revision: self.policy_revision,
            policy_digest_hex: self.policy_digest_hex.as_deref(),
            active_from_unix_ms: self.active_from_unix_ms,
            active_until_unix_ms: self.active_until_unix_ms,
        }
    }
    fn is_configured(&self) -> bool {
        self.fields().is_configured()
            || self.max_validity_ms.is_some()
            || self.max_anchor_age_ms.is_some()
    }
    fn parse(&self, parser: &mut Parser<'_>) -> Option<actual::SorafsStreamTokenAttesterConfig> {
        let authority = parser.authority(self.fields(), "attester");
        let max_validity_ms = parser.number(
            self.max_validity_ms,
            &path("attester.max_validity_ms"),
            MAX_CUSTODY_TIME_MS,
        );
        let max_anchor_age_ms = parser.number(
            self.max_anchor_age_ms,
            &path("attester.max_anchor_age_ms"),
            MAX_CUSTODY_TIME_MS,
        );
        Some(actual::SorafsStreamTokenAttesterConfig {
            authority: authority?,
            max_validity_ms: max_validity_ms?,
            max_anchor_age_ms: max_anchor_age_ms?,
        })
    }
}

impl SorafsStreamTokenObserverConfig {
    fn fields(&self) -> AuthorityFields<'_> {
        AuthorityFields {
            service_id: self.service_id.as_deref(),
            administrator_id: self.administrator_id.as_deref(),
            public_key_hex: self.public_key_hex.as_deref(),
            key_revision: self.key_revision,
            policy_revision: self.policy_revision,
            policy_digest_hex: self.policy_digest_hex.as_deref(),
            active_from_unix_ms: self.active_from_unix_ms,
            active_until_unix_ms: self.active_until_unix_ms,
        }
    }
    fn is_configured(&self) -> bool {
        self.fields().is_configured()
            || self.runtime_handle.is_some()
            || self.max_state_age_ms.is_some()
    }
    fn parse(&self, parser: &mut Parser<'_>) -> Option<actual::SorafsStreamTokenObserverConfig> {
        let runtime_handle = parser.text(self.runtime_handle.as_deref(), &path("observer.runtime_handle"),
            observer_handle, "must be a canonical credential-free production observer handle without software markers");
        let authority = parser.authority(self.fields(), "observer");
        let max_state_age_ms = parser.number(
            self.max_state_age_ms,
            &path("observer.max_state_age_ms"),
            MAX_OBSERVER_AGE_MS,
        );
        Some(actual::SorafsStreamTokenObserverConfig {
            runtime_handle: runtime_handle?,
            authority: authority?,
            max_state_age_ms: max_state_age_ms?,
        })
    }
}

struct AuthorityFields<'a> {
    service_id: Option<&'a str>,
    administrator_id: Option<&'a str>,
    public_key_hex: Option<&'a str>,
    key_revision: Option<u64>,
    policy_revision: Option<u64>,
    policy_digest_hex: Option<&'a str>,
    active_from_unix_ms: Option<u64>,
    active_until_unix_ms: Option<u64>,
}
impl AuthorityFields<'_> {
    fn is_configured(&self) -> bool {
        self.service_id.is_some()
            || self.administrator_id.is_some()
            || self.public_key_hex.is_some()
            || self.key_revision.is_some()
            || self.policy_revision.is_some()
            || self.policy_digest_hex.is_some()
            || self.active_from_unix_ms.is_some()
            || self.active_until_unix_ms.is_some()
    }
}

struct Parser<'a> {
    emitter: &'a mut Emitter<ParseError>,
}
impl Parser<'_> {
    fn error(&mut self, field: &str, message: &str) {
        self.emitter.emit(
            Report::new(ParseError::InvalidSorafsConfig).attach(format!("{field} {message}")),
        );
    }
    fn text(
        &mut self,
        value: Option<&str>,
        field: &str,
        valid: fn(&str) -> bool,
        message: &str,
    ) -> Option<String> {
        match value {
            None => {
                self.error(field, "is required when issuance is enabled");
                None
            }
            Some(value) if !valid(value) => {
                self.error(field, message);
                None
            }
            Some(value) => Some(value.to_owned()),
        }
    }
    fn identity(&mut self, value: Option<&str>, field: &str) -> Option<String> {
        self.text(
            value,
            field,
            identity,
            "must be a canonical nonempty production identity of at most 128 ASCII bytes",
        )
    }
    fn number(&mut self, value: Option<u64>, field: &str, maximum: u64) -> Option<u64> {
        match value {
            None => {
                self.error(field, "is required when issuance is enabled");
                None
            }
            Some(value) if value == 0 || value > maximum => {
                self.error(field, &format!("must be within 1..={maximum}"));
                None
            }
            Some(value) => Some(value),
        }
    }
    fn digest(&mut self, value: Option<&str>, field: &str) -> Option<[u8; 32]> {
        let value = match value {
            None => {
                self.error(field, "is required when issuance is enabled");
                return None;
            }
            Some(value) => value,
        };
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
        {
            self.error(field, "must be exactly 64 lowercase hexadecimal characters");
            return None;
        }
        let bytes: [u8; 32] = hex::decode(value)
            .expect("validated hex")
            .try_into()
            .expect("fixed width");
        if bytes == [0; 32] {
            self.error(field, "must be non-zero");
            return None;
        }
        Some(bytes)
    }
    fn public_key(&mut self, value: Option<&str>, field: &str) -> Option<[u8; 32]> {
        let value = match value {
            None => {
                self.error(field, "is required when issuance is enabled");
                return None;
            }
            Some(value) => value,
        };
        if !is_canonical_nonzero_ed25519_public_key_hex(value) {
            self.error(field, "must be canonical lowercase non-zero 32-byte hex");
            return None;
        }
        if !is_canonical_strong_ed25519_public_key_hex(value) {
            self.error(field, "is not a valid Ed25519 public key");
            return None;
        }
        Some(
            hex::decode(value)
                .expect("validated key hex")
                .try_into()
                .expect("fixed key width"),
        )
    }
    fn authority(
        &mut self,
        fields: AuthorityFields<'_>,
        prefix: &str,
    ) -> Option<actual::SorafsStreamTokenAuthorityConfig> {
        let field = |name: &str| path(&format!("{prefix}.{name}"));
        let service_id = self.identity(fields.service_id, &field("service_id"));
        let administrator_id = self.identity(fields.administrator_id, &field("administrator_id"));
        let public_key = self.public_key(fields.public_key_hex, &field("public_key_hex"));
        let key_revision = self.number(fields.key_revision, &field("key_revision"), u64::MAX);
        let policy_revision =
            self.number(fields.policy_revision, &field("policy_revision"), u64::MAX);
        let policy_digest = self.digest(fields.policy_digest_hex, &field("policy_digest_hex"));
        let active_from = self.number(
            fields.active_from_unix_ms,
            &field("active_from_unix_ms"),
            u64::MAX,
        );
        let active_until = self.number(
            fields.active_until_unix_ms,
            &field("active_until_unix_ms"),
            u64::MAX,
        );
        let (active_from_unix_ms, active_until_unix_ms) = (active_from?, active_until?);
        if active_until_unix_ms <= active_from_unix_ms {
            self.error(
                &field("active_until_unix_ms"),
                "must be after active_from_unix_ms",
            );
            return None;
        }
        Some(actual::SorafsStreamTokenAuthorityConfig {
            service_id: service_id?,
            administrator_id: administrator_id?,
            public_key: public_key?,
            key_revision: key_revision?,
            policy_revision: policy_revision?,
            policy_digest: policy_digest?,
            active_from_unix_ms,
            active_until_unix_ms,
        })
    }
}

fn path(field: &str) -> String {
    format!("{PREFIX}.{field}")
}

fn identity(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':'))
        && !value.to_ascii_lowercase().contains("test")
}

fn hardware_handle(value: &str) -> bool {
    if value.len() > 128 || !is_production_runtime_handle(value) {
        return false;
    }
    let Some((scheme, opaque)) = value.split_once(':') else {
        return false;
    };
    let opaque = opaque.strip_prefix("//").unwrap_or(opaque);
    matches!(scheme, "hsm" | "kms" | "pkcs11")
        && opaque.split('/').all(|component| {
            component.bytes().any(|byte| byte.is_ascii_alphanumeric())
                && component
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        })
}

fn observer_handle(value: &str) -> bool {
    is_production_runtime_handle(value)
        && !value
            .to_ascii_lowercase()
            .split(|character: char| !character.is_ascii_alphanumeric())
            .any(|component| component == "software")
}
