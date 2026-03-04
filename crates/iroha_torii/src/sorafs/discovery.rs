//! Provider advert ingestion and validation for Torii's SoraFS discovery pipeline.

use std::{collections::HashMap, sync::Arc};

use blake3::hash as blake3_hash;
use ed25519_dalek::{
    PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH, Signature as DalekSignature, Verifier, VerifyingKey,
};
use norito::to_bytes;
use sorafs_manifest::{
    AdvertValidationError, CapabilityType, ProviderAdvertV1, SignatureAlgorithm,
};
use thiserror::Error;

use super::admission::{AdmissionCheckError, AdmissionRegistry, verify_advert_against_envelope};

/// Fingerprint size for stored adverts (BLAKE3-256).
pub const FINGERPRINT_LEN: usize = 32;

/// Outcome of ingesting a provider advert into the cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdvertIngest {
    /// A brand new advert was stored.
    Stored {
        /// BLAKE3 fingerprint of the stored advert.
        fingerprint: [u8; FINGERPRINT_LEN],
    },
    /// An advert replaced a previous version from the same provider.
    Replaced {
        /// BLAKE3 fingerprint of the updated advert.
        fingerprint: [u8; FINGERPRINT_LEN],
    },
    /// The advert matched an existing fingerprint and was ignored.
    Duplicate {
        /// BLAKE3 fingerprint of the existing advert.
        fingerprint: [u8; FINGERPRINT_LEN],
    },
}

/// Metadata downgrade warnings emitted during advert ingestion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdvertWarning {
    /// Provider advert omitted the required `chunk_range_fetch` capability.
    MissingChunkRangeCapability,
    /// Provider advert omitted the required stream budget for range fetch.
    MissingStreamBudget,
    /// Provider advert omitted transport hints for range fetch.
    MissingTransportHints,
    /// Provider advert disabled strict signature enforcement.
    SignatureNotStrict,
    /// Provider advert signature failed verification but enforcement was bypassed.
    SignatureInvalid,
}

impl AdvertWarning {
    /// Returns the canonical telemetry reason label.
    #[must_use]
    pub fn telemetry_reason(self) -> &'static str {
        match self {
            AdvertWarning::MissingChunkRangeCapability => "missing_chunk_range",
            AdvertWarning::MissingStreamBudget => "missing_stream_budget",
            AdvertWarning::MissingTransportHints => "missing_transport_hints",
            AdvertWarning::SignatureNotStrict => "signature_not_strict",
            AdvertWarning::SignatureInvalid => "signature_invalid",
        }
    }

    /// Returns a short identifier suitable for JSON responses.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        self.telemetry_reason()
    }
}

/// Result of an advert ingestion attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdvertIngestResult {
    /// Outcome of the ingestion (stored/replaced/duplicate).
    pub outcome: AdvertIngest,
    /// Downgrade warnings detected during ingestion.
    pub warnings: Vec<AdvertWarning>,
}

/// Errors surfaced while processing provider adverts.
#[derive(Debug, Error)]
pub enum AdvertError {
    /// Provider advert could not be decoded with the Norito codec.
    #[error("decode provider advert: {0}")]
    Decode(#[from] norito::Error),
    /// Structural validation of the advert failed.
    #[error("provider advert validation failed: {0}")]
    Validation(#[from] AdvertValidationError),
    /// The advert declared a signature algorithm Torii does not support.
    #[error("unsupported signature algorithm: {0:?}")]
    UnsupportedSignature(SignatureAlgorithm),
    /// Signature verification failed while strict mode was enforced.
    #[error("signature verification failed: {0}")]
    Signature(String),
    /// The advert referenced capability TLVs not present in the allow-list.
    #[error("unknown capabilities rejected: {capabilities:?}")]
    UnknownCapabilities {
        /// Capability identifiers rejected during validation.
        capabilities: Vec<CapabilityType>,
    },
    /// No admission envelope matched the provider identifier.
    #[error("missing admission envelope for provider {provider_id:02x?}")]
    AdmissionMissing {
        /// Provider identifier lacking an admission envelope.
        provider_id: [u8; 32],
    },
    /// Admission verification failed for the advert.
    #[error("admission verification failed for provider {provider_id:02x?}: {error}")]
    AdmissionFailed {
        /// Provider identifier whose admission failed.
        provider_id: [u8; 32],
        /// Reason the admission check rejected the advert.
        error: AdmissionCheckError,
    },
}

/// Sanitised provider advert stored by the cache.
#[derive(Debug, Clone)]
pub struct AdvertRecord {
    fingerprint: [u8; FINGERPRINT_LEN],
    advert: ProviderAdvertV1,
    known_capabilities: Vec<CapabilityType>,
    warnings: Vec<AdvertWarning>,
}

impl AdvertRecord {
    /// Returns the stored advert.
    #[must_use]
    pub fn advert(&self) -> &ProviderAdvertV1 {
        &self.advert
    }

    /// Returns the filtered capability list recognised by the cache.
    #[must_use]
    pub fn known_capabilities(&self) -> &[CapabilityType] {
        self.known_capabilities.as_slice()
    }

    /// Returns the downgrade warnings observed while ingesting the advert.
    #[must_use]
    pub fn warnings(&self) -> &[AdvertWarning] {
        &self.warnings
    }

    /// Returns the advert fingerprint.
    #[must_use]
    pub fn fingerprint(&self) -> &[u8; FINGERPRINT_LEN] {
        &self.fingerprint
    }
}

/// In-memory cache for provider adverts propagated through Torii.
#[derive(Debug)]
pub struct ProviderAdvertCache {
    known_capabilities: Vec<CapabilityType>,
    records: HashMap<[u8; FINGERPRINT_LEN], AdvertRecord>,
    by_provider: HashMap<[u8; 32], [u8; FINGERPRINT_LEN]>,
    admission: Arc<AdmissionRegistry>,
}

impl ProviderAdvertCache {
    /// Construct a new cache with the provided capability allow-list.
    #[must_use]
    pub fn new<I>(known_capabilities: I, admission: Arc<AdmissionRegistry>) -> Self
    where
        I: IntoIterator<Item = CapabilityType>,
    {
        Self {
            known_capabilities: known_capabilities.into_iter().collect(),
            records: HashMap::new(),
            by_provider: HashMap::new(),
            admission,
        }
    }

    /// Attempt to ingest a provider advert.
    ///
    /// When the advert carries unknown capabilities and `allow_unknown_capabilities` is `false`,
    /// the operation fails with [`AdvertError::UnknownCapabilities`]. When an advert validates
    /// successfully, the cache stores the full advert but exposes only the capabilities recognised
    /// by the allow-list.
    ///
    /// # Errors
    ///
    /// Returns an [`AdvertError`] if signature verification fails, the advert
    /// requests unknown capabilities without opting in, or the associated
    /// admission record rejects the advert contents.
    pub fn ingest(
        &mut self,
        advert: ProviderAdvertV1,
        now: u64,
    ) -> Result<AdvertIngestResult, AdvertError> {
        advert.validate_with_body(now)?;
        let provider_id = advert.body.provider_id;
        let mut warnings = collect_warnings(&advert);
        match verify_signature(&advert) {
            Ok(()) => {}
            Err(_err @ AdvertError::Signature(_)) if !advert.signature_strict => {
                warnings.push(AdvertWarning::SignatureInvalid);
            }
            Err(err) => return Err(err),
        }

        let unknown_caps: Vec<CapabilityType> = advert
            .body
            .capabilities
            .iter()
            .map(|tlv| tlv.cap_type)
            .filter(|cap_type| !self.known_capabilities.contains(cap_type))
            .collect();
        if !unknown_caps.is_empty() && !advert.allow_unknown_capabilities {
            return Err(AdvertError::UnknownCapabilities {
                capabilities: unknown_caps,
            });
        }

        let filtered_caps: Vec<CapabilityType> = advert
            .body
            .capabilities
            .iter()
            .filter_map(|tlv| {
                if self.known_capabilities.contains(&tlv.cap_type) {
                    Some(tlv.cap_type)
                } else {
                    None
                }
            })
            .collect();

        let admission_entry = self
            .admission
            .entry(&provider_id)
            .ok_or(AdvertError::AdmissionMissing { provider_id })?;
        verify_advert_against_envelope(&advert, &admission_entry)
            .map_err(|error| AdvertError::AdmissionFailed { provider_id, error })?;

        let fingerprint = fingerprint(&advert)?;
        let previous = self.by_provider.get(&provider_id).copied();
        if let Some(prev_fp) = previous {
            if prev_fp == fingerprint {
                return Ok(AdvertIngestResult {
                    outcome: AdvertIngest::Duplicate { fingerprint },
                    warnings,
                });
            }
            self.records.remove(&prev_fp);
        }

        let record = AdvertRecord {
            fingerprint,
            advert,
            known_capabilities: filtered_caps,
            warnings: warnings.clone(),
        };
        self.records.insert(fingerprint, record);
        self.by_provider.insert(provider_id, fingerprint);

        let outcome = match previous {
            Some(_) => AdvertIngest::Replaced { fingerprint },
            None => AdvertIngest::Stored { fingerprint },
        };

        Ok(AdvertIngestResult { outcome, warnings })
    }

    /// Return the number of cached adverts.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Returns `true` when the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Look up the advert stored for the given provider id.
    #[must_use]
    pub fn record_by_provider(&self, provider_id: &[u8; 32]) -> Option<&AdvertRecord> {
        self.by_provider
            .get(provider_id)
            .and_then(|fp| self.records.get(fp))
    }

    /// Iterate over all stored adverts.
    pub fn records(&self) -> impl Iterator<Item = &AdvertRecord> {
        self.records.values()
    }

    /// Revalidate cached adverts and drop entries that no longer pass admission checks.
    pub fn prune_stale(&mut self, now: u64) -> usize {
        let mut to_remove = Vec::new();
        for (&fingerprint, record) in &self.records {
            let advert = record.advert();
            let provider_id = advert.body.provider_id;
            let mut drop = advert.validate_with_body(now).is_err();
            if !drop {
                match self.admission.entry(&provider_id) {
                    Some(entry) => {
                        if verify_advert_against_envelope(advert, &entry).is_err() {
                            drop = true;
                        }
                    }
                    None => {
                        drop = true;
                    }
                }
            }
            if drop {
                to_remove.push((fingerprint, provider_id));
            }
        }

        for (fingerprint, provider_id) in &to_remove {
            self.records.remove(fingerprint);
            self.by_provider.remove(provider_id);
        }

        to_remove.len()
    }
}

fn fingerprint(advert: &ProviderAdvertV1) -> Result<[u8; FINGERPRINT_LEN], AdvertError> {
    let bytes = to_bytes(advert)?;
    let digest = blake3_hash(&bytes);
    Ok(digest.into())
}

fn verify_signature(advert: &ProviderAdvertV1) -> Result<(), AdvertError> {
    match advert.signature.algorithm {
        SignatureAlgorithm::Ed25519 => {}
        other => return Err(AdvertError::UnsupportedSignature(other)),
    }

    if advert.signature.public_key.len() != PUBLIC_KEY_LENGTH {
        return Err(AdvertError::Signature(format!(
            "unexpected public key length: {}",
            advert.signature.public_key.len()
        )));
    }
    if advert.signature.signature.len() != SIGNATURE_LENGTH {
        return Err(AdvertError::Signature(format!(
            "unexpected signature length: {}",
            advert.signature.signature.len()
        )));
    }

    let mut pk = [0u8; PUBLIC_KEY_LENGTH];
    pk.copy_from_slice(&advert.signature.public_key);
    let verifying_key =
        VerifyingKey::from_bytes(&pk).map_err(|err| AdvertError::Signature(err.to_string()))?;

    let mut sig_bytes = [0u8; SIGNATURE_LENGTH];
    sig_bytes.copy_from_slice(&advert.signature.signature);
    let signature = DalekSignature::from_bytes(&sig_bytes);

    let body_bytes = to_bytes(&advert.body)?;

    verifying_key
        .verify(&body_bytes, &signature)
        .map_err(|err| AdvertError::Signature(err.to_string()))
}

/// Return the canonical capability name used in configuration and responses.
#[must_use]
pub fn capability_name(capability: CapabilityType) -> &'static str {
    match capability {
        CapabilityType::ToriiGateway => "torii_gateway",
        CapabilityType::QuicNoise => "quic_noise",
        CapabilityType::ChunkRangeFetch => "chunk_range_fetch",
        CapabilityType::SoraNetHybridPq => "soranet_pq",
        CapabilityType::VendorReserved => "vendor_reserved",
    }
}

/// Parse a capability name used in configuration into the corresponding enum value.
#[must_use]
pub fn parse_capability_name(name: &str) -> Option<CapabilityType> {
    match name.trim().to_ascii_lowercase().as_str() {
        "torii" | "torii_gateway" => Some(CapabilityType::ToriiGateway),
        "quic" | "quic_noise" => Some(CapabilityType::QuicNoise),
        "soranet" | "soranet_pq" | "soranet-pq" | "soranet-hybrid-pq" => {
            Some(CapabilityType::SoraNetHybridPq)
        }
        "range" | "chunk_range_fetch" => Some(CapabilityType::ChunkRangeFetch),
        "vendor_reserved" | "vendor" => Some(CapabilityType::VendorReserved),
        _ => None,
    }
}

fn collect_warnings(advert: &ProviderAdvertV1) -> Vec<AdvertWarning> {
    let mut warnings = Vec::new();

    let has_chunk_range = advert
        .body
        .capabilities
        .iter()
        .any(|cap| cap.cap_type == CapabilityType::ChunkRangeFetch);
    if !has_chunk_range {
        warnings.push(AdvertWarning::MissingChunkRangeCapability);
    } else {
        if advert.body.stream_budget.is_none() {
            warnings.push(AdvertWarning::MissingStreamBudget);
        }
        if advert
            .body
            .transport_hints
            .as_ref()
            .map_or(true, Vec::is_empty)
        {
            warnings.push(AdvertWarning::MissingTransportHints);
        }
    }
    if !advert.signature_strict {
        warnings.push(AdvertWarning::SignatureNotStrict);
    }

    warnings
}
