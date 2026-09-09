//! Exact independently configured public pins for hardware stream-token issuance.

use super::StreamTokenIssuerError;
use ed25519_dalek::VerifyingKey;
use iroha_config::parameters::{actual, validate_production_runtime_handle};
use iroha_crypto::{Algorithm, PublicKey};
use norito::codec::Encode;
use sorafs_manifest::signer::{
    custody::{SignerCustodyAuthorityV1, SignerCustodyBindingV1, SignerCustodyTrustV1},
    protocol::{SignerKeyAlgorithmV1, SignerPurposeBindingV1, SignerRoleV1},
    state_observation::{SIGNER_STATE_OBSERVATION_MAX_AGE_MS_V1, SignerStateObserverTrustV1},
    stream_token::stream_token_binding_digest_v1,
};
use std::fmt;

/// Complete public configuration pinned before contacting either runtime client.
///
/// Construction validates configuration only. No receipt, attestation or provider claim can create
/// these pins, and possession of them establishes neither hardware custody nor current finality.
#[derive(Clone)]
pub struct StreamTokenHardwarePinsV1 {
    binding: SignerCustodyBindingV1,
    custody: SignerCustodyTrustV1,
    observer: SignerStateObserverTrustV1,
    observer_handle: String,
    config_digest: [u8; 32],
}
impl fmt::Debug for StreamTokenHardwarePinsV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("StreamTokenHardwarePinsV1 { public pins redacted }")
    }
}
impl StreamTokenHardwarePinsV1 {
    /// Bind configuration to the independently supplied chain, network and storage provider.
    ///
    /// # Errors
    /// Rejects incomplete, disabled-but-populated, noncanonical or non-independent public pins.
    pub fn from_config(
        storage: &actual::SorafsStorage,
        chain_id: &str,
        network_id: [u8; 32],
    ) -> Result<Option<Self>, StreamTokenIssuerError> {
        let invalid = || StreamTokenIssuerError::InvalidHardwareConfig;
        let token = &storage.stream_tokens;
        if !token.enabled {
            return if token.hardware.is_none() {
                Ok(None)
            } else {
                Err(invalid())
            };
        }
        let hardware = token.hardware.as_ref().ok_or_else(invalid)?;
        // Bound every variable leaf before copying programmatically constructed actual config.
        if chain_id.len() > 128
            || [
                &hardware.runtime_handle,
                &hardware.key_handle,
                &hardware.service_id,
                &hardware.administrator_id,
            ]
            .iter()
            .any(|value| value.len() > 128)
            || hardware.observer.runtime_handle.len() > 256
        {
            return Err(invalid());
        }
        let provider_id = storage.provider_id.as_ref().ok_or_else(invalid)?.0;
        if !storage.enabled
            || provider_id == [0; 32]
            || u32::try_from(hardware.key_revision)
                .ok()
                .filter(|v| *v != 0)
                .is_none()
        {
            return Err(invalid());
        }
        let binding = SignerCustodyBindingV1 {
            chain_id: chain_id.to_owned(),
            network_id,
            runtime_handle: hardware.runtime_handle.clone(),
            key_handle: hardware.key_handle.clone(),
            service_id: hardware.service_id.clone(),
            administrator_id: hardware.administrator_id.clone(),
            role: SignerRoleV1::StreamToken,
            purpose: SignerPurposeBindingV1::StreamToken { provider_id },
            algorithm: SignerKeyAlgorithmV1::Ed25519,
            public_key: strong_key(hardware.public_key)?,
            key_revision: hardware.key_revision,
            policy_revision: hardware.policy_revision,
            policy_digest: hardware.policy_digest,
        };
        // The shared purpose owner is the sole binding grammar/algorithm validator.
        stream_token_binding_digest_v1(&binding).map_err(|_| invalid())?;
        let attester = &hardware.attester;
        let observer_config = &hardware.observer;
        let custody = SignerCustodyTrustV1 {
            authority: authority(&attester.authority)?,
            public_key: strong_key(attester.authority.public_key)?,
            active_from_unix_ms: attester.authority.active_from_unix_ms,
            active_until_unix_ms: attester.authority.active_until_unix_ms,
            max_validity_ms: attester.max_validity_ms,
            max_anchor_age_ms: attester.max_anchor_age_ms,
        };
        let observer = SignerStateObserverTrustV1 {
            authority: authority(&observer_config.authority)?,
            public_key: strong_key(observer_config.authority.public_key)?,
            active_from_unix_ms: observer_config.authority.active_from_unix_ms,
            active_until_unix_ms: observer_config.authority.active_until_unix_ms,
            max_state_age_ms: observer_config.max_state_age_ms,
        };
        let identities = [
            binding.service_id.as_str(),
            binding.administrator_id.as_str(),
            custody.authority.service_id.as_str(),
            custody.authority.administrator_id.as_str(),
            observer.authority.service_id.as_str(),
            observer.authority.administrator_id.as_str(),
        ];
        let keys = [
            &binding.public_key,
            &custody.public_key,
            &observer.public_key,
        ];
        if identities
            .iter()
            .enumerate()
            .any(|(i, id)| identities[..i].contains(id))
            || keys
                .iter()
                .enumerate()
                .any(|(i, key)| keys[..i].contains(key))
            || !(1..=86_400_000).contains(&custody.max_validity_ms)
            || !(1..=86_400_000).contains(&custody.max_anchor_age_ms)
            || !(1..=SIGNER_STATE_OBSERVATION_MAX_AGE_MS_V1).contains(&observer.max_state_age_ms)
            || custody
                .active_from_unix_ms
                .max(observer.active_from_unix_ms)
                >= custody
                    .active_until_unix_ms
                    .min(observer.active_until_unix_ms)
            || validate_production_runtime_handle(&observer_config.runtime_handle).is_err()
            || observer_config
                .runtime_handle
                .to_ascii_lowercase()
                .split(|c: char| !c.is_ascii_alphanumeric())
                .any(|v| v == "software")
            || binding.runtime_handle == observer_config.runtime_handle
        {
            return Err(invalid());
        }
        let preimage = PinsPreimageV1 {
            binding: binding.clone(),
            custody_authority: custody.authority.clone(),
            custody_key: custody.public_key.clone(),
            custody_active_from: custody.active_from_unix_ms,
            custody_active_until: custody.active_until_unix_ms,
            max_validity_ms: custody.max_validity_ms,
            max_anchor_age_ms: custody.max_anchor_age_ms,
            observer_authority: observer.authority.clone(),
            observer_key: observer.public_key.clone(),
            observer_active_from: observer.active_from_unix_ms,
            observer_active_until: observer.active_until_unix_ms,
            max_state_age_ms: observer.max_state_age_ms,
            observer_handle: observer_config.runtime_handle.clone(),
        };
        let bytes = norito::encode_canonical(&preimage).map_err(|_| invalid())?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"iroha.sorafs.stream-token.runtime-pins.v1");
        hasher.update(
            &u64::try_from(bytes.len())
                .map_err(|_| invalid())?
                .to_be_bytes(),
        );
        hasher.update(&bytes);
        Ok(Some(Self {
            binding,
            custody,
            observer,
            observer_handle: observer_config.runtime_handle.clone(),
            config_digest: *hasher.finalize().as_bytes(),
        }))
    }
    /// Exact provider-scoped signing subject; not runtime qualification.
    #[must_use]
    pub fn binding(&self) -> &SignerCustodyBindingV1 {
        &self.binding
    }
    /// Independently pinned attestation authority and policy.
    #[must_use]
    pub fn custody_trust(&self) -> &SignerCustodyTrustV1 {
        &self.custody
    }
    /// Independently pinned finalized-state observation authority and policy.
    #[must_use]
    pub fn observer_trust(&self) -> &SignerStateObserverTrustV1 {
        &self.observer
    }
    /// Exact credential-free observer runtime routing handle.
    #[must_use]
    pub fn observer_handle(&self) -> &str {
        &self.observer_handle
    }
    /// Canonical identity of every immutable public pin, including observer routing and intervals.
    #[must_use]
    pub const fn config_digest(&self) -> [u8; 32] {
        self.config_digest
    }
}
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_torii::sorafs::token::hardware_pins::PinsPreimageV1")]
#[derive(Encode)]
struct PinsPreimageV1 {
    binding: SignerCustodyBindingV1,
    custody_authority: SignerCustodyAuthorityV1,
    custody_key: PublicKey,
    custody_active_from: u64,
    custody_active_until: u64,
    max_validity_ms: u64,
    max_anchor_age_ms: u64,
    observer_authority: SignerCustodyAuthorityV1,
    observer_key: PublicKey,
    observer_active_from: u64,
    observer_active_until: u64,
    max_state_age_ms: u64,
    observer_handle: String,
}
fn strong_key(bytes: [u8; 32]) -> Result<PublicKey, StreamTokenIssuerError> {
    let key = VerifyingKey::from_bytes(&bytes)
        .map_err(|_| StreamTokenIssuerError::InvalidHardwareConfig)?;
    if key.is_weak() {
        return Err(StreamTokenIssuerError::InvalidHardwareConfig);
    }
    PublicKey::from_bytes(Algorithm::Ed25519, &bytes)
        .map_err(|_| StreamTokenIssuerError::InvalidHardwareConfig)
}
fn authority(
    value: &actual::SorafsStreamTokenAuthorityConfig,
) -> Result<SignerCustodyAuthorityV1, StreamTokenIssuerError> {
    let valid_identity = |id: &str| {
        !id.is_empty()
            && id.len() <= 128
            && id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b':'))
            && !id.to_ascii_lowercase().contains("test")
    };
    if !valid_identity(&value.service_id)
        || !valid_identity(&value.administrator_id)
        || value.key_revision == 0
        || value.policy_revision == 0
        || value.policy_digest == [0; 32]
        || value.active_from_unix_ms == 0
        || value.active_until_unix_ms <= value.active_from_unix_ms
    {
        return Err(StreamTokenIssuerError::InvalidHardwareConfig);
    }
    Ok(SignerCustodyAuthorityV1 {
        service_id: value.service_id.clone(),
        administrator_id: value.administrator_id.clone(),
        key_revision: value.key_revision,
        policy_revision: value.policy_revision,
        policy_digest: value.policy_digest,
    })
}
