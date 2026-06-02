//! `SoraNet` relay certificate (`SRC`) version 2 helpers.
//!
//! `SRCv2` documents the identity, transport capabilities, and validity window
//! for a relay. Certificates are CBOR-encoded, dual-signed (Ed25519 +
//! ML-DSA-65), and referenced by the directory consensus artefacts.
//!
//! This module provides a minimal CBOR encoder/decoder tailored to the `SRCv2`
//! schema so we can avoid pulling an additional dependency while keeping the
//! encoding canonical and deterministic.

use std::{
    convert::{TryFrom, TryInto},
    time::Duration,
};

use blake3::Hasher as Blake3;
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use soranet_pq::{MlDsaSuite, MlKemSuite, sign_mldsa_from_os, verify_mldsa};
use thiserror::Error;

use crate::soranet::handshake::HandshakeSuite;

/// Canonical Blake3 domain separator for `SRCv2` digests.
const SRC_V2_DOMAIN: &[u8] = b"soranet.src.v2.digest";
/// Canonical Blake3 domain separator for Ed25519 signing.
const SRC_V2_ED25519_DOMAIN: &[u8] = b"soranet.src.v2.ed25519";
/// Canonical Blake3 domain separator for ML-DSA signing.
const SRC_V2_MLDSA_DOMAIN: &[u8] = b"soranet.src.v2.mldsa65";

/// `SRCv2` map field identifiers.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Field {
    Version = 0,
    RelayId = 1,
    IdentityEd25519 = 2,
    IdentityMlDsa65 = 3,
    DescriptorCommit = 4,
    Roles = 5,
    GuardWeight = 6,
    BandwidthBytesPerSec = 7,
    ReputationWeight = 8,
    Endpoints = 9,
    CapabilityFlags = 10,
    KemPolicy = 11,
    HandshakeSuites = 12,
    PublishedAt = 13,
    ValidAfter = 14,
    ValidUntil = 15,
    DirectoryHash = 16,
    IssuerFingerprint = 17,
    PqKemPublic = 18,
}

fn field_label(field: Field) -> &'static str {
    match field {
        Field::Version => "certificate.version",
        Field::RelayId => "certificate.relay_id",
        Field::IdentityEd25519 => "certificate.identity_ed25519",
        Field::IdentityMlDsa65 => "certificate.identity_mldsa65",
        Field::DescriptorCommit => "certificate.descriptor_commit",
        Field::Roles => "certificate.roles",
        Field::GuardWeight => "certificate.guard_weight",
        Field::BandwidthBytesPerSec => "certificate.bandwidth_bytes_per_sec",
        Field::ReputationWeight => "certificate.reputation_weight",
        Field::Endpoints => "certificate.endpoints",
        Field::CapabilityFlags => "certificate.capability_flags",
        Field::KemPolicy => "certificate.kem_policy",
        Field::HandshakeSuites => "certificate.handshake_suites",
        Field::PublishedAt => "certificate.published_at",
        Field::ValidAfter => "certificate.valid_after",
        Field::ValidUntil => "certificate.valid_until",
        Field::DirectoryHash => "certificate.directory_hash",
        Field::IssuerFingerprint => "certificate.issuer_fingerprint",
        Field::PqKemPublic => "certificate.pq_kem_public",
    }
}

/// `SRCv2` signature map field identifiers.
#[repr(u8)]
enum SignatureField {
    Ed25519 = 0,
    MlDsa65 = 1,
}

/// Endpoint map field identifiers.
#[repr(u8)]
enum EndpointField {
    Url = 0,
    Priority = 1,
    Tags = 2,
}

/// Capability flag bit positions.
mod capability_bits {
    pub const BLINDED_CID: u16 = 1 << 0;
    pub const POW_TICKET: u16 = 1 << 1;
    pub const NORITO_STREAM: u16 = 1 << 2;
    pub const KAIGI_BRIDGE: u16 = 1 << 3;
}

/// KEM rotation policy field identifiers.
#[repr(u8)]
enum KemField {
    Mode = 0,
    PreferredSuite = 1,
    FallbackSuite = 2,
    RotationIntervalHours = 3,
    GracePeriodHours = 4,
}

/// Identifier for SRC schema version.
pub const SRC_CERTIFICATE_VERSION: u8 = 2;

/// Validation phases for SRC rollout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CertificateValidationPhase {
    /// Phase 1 — accept certificates that only contain an Ed25519 signature.
    Phase1AllowSingle,
    /// Phase 2 — prefer dual signatures but accept single signatures with a warning.
    Phase2PreferDual,
    /// Phase 3 — require both Ed25519 and ML-DSA-65 signatures.
    Phase3RequireDual,
}

/// Rotation policy for ML-KEM keys advertised by the relay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KemRotationModeV1 {
    /// Relay pins a single ML-KEM suite until the certificate is reissued.
    Static = 0,
    /// Relay stages the fallback suite and switches once directory consensus coordinates it.
    Staged = 1,
    /// Relay rotates ML-KEM material on a rolling schedule published in the directory.
    Rolling = 2,
}

impl KemRotationModeV1 {
    fn from_raw(raw: u64) -> Result<Self, CertificateError> {
        match raw {
            0 => Ok(Self::Static),
            1 => Ok(Self::Staged),
            2 => Ok(Self::Rolling),
            other => Err(CertificateError::InvalidFieldValue {
                field: "kem_policy.mode",
                reason: format!("unsupported mode {other}"),
            }),
        }
    }
}

/// Rotation policy structure embedded in the SRC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KemRotationPolicyV1 {
    /// Rotation strategy for ML-KEM suites.
    pub mode: KemRotationModeV1,
    /// Preferred ML-KEM suite (`snnet.pqkem` identifier).
    pub preferred_suite: u8,
    /// Optional fallback suite advertised for staged upgrades.
    pub fallback_suite: Option<u8>,
    /// Rotation cadence in hours (0 for static policies).
    pub rotation_interval_hours: u16,
    /// Grace period in hours when switching suites.
    pub grace_period_hours: u16,
}

impl KemRotationPolicyV1 {
    fn encode(self, encoder: &mut CborEncoder) {
        encoder.write_map_header(5);
        encoder.write_unsigned(KemField::Mode as u64);
        encoder.write_unsigned(self.mode as u64);
        encoder.write_unsigned(KemField::PreferredSuite as u64);
        encoder.write_unsigned(self.preferred_suite.into());
        encoder.write_unsigned(KemField::FallbackSuite as u64);
        match self.fallback_suite {
            Some(value) => encoder.write_unsigned(value.into()),
            None => encoder.write_null(),
        }
        encoder.write_unsigned(KemField::RotationIntervalHours as u64);
        encoder.write_unsigned(self.rotation_interval_hours.into());
        encoder.write_unsigned(KemField::GracePeriodHours as u64);
        encoder.write_unsigned(self.grace_period_hours.into());
    }

    fn decode(decoder: &mut CborDecoder) -> Result<Self, CertificateError> {
        let map_len = decoder.read_map_len()?;
        let mut mode = None;
        let mut preferred_suite = None;
        let mut fallback_suite = None;
        let mut rotation_interval_hours = None;
        let mut grace_period_hours = None;

        for _ in 0..map_len {
            let raw_key = decoder.read_unsigned()?;
            let key: u8 = raw_key
                .try_into()
                .map_err(|_| CertificateError::InvalidFieldValue {
                    field: "kem_policy",
                    reason: format!("field key {raw_key} exceeds u8::MAX"),
                })?;
            match key {
                x if x == KemField::Mode as u8 => {
                    let raw = decoder.read_unsigned()?;
                    set_decoded_once(
                        &mut mode,
                        KemRotationModeV1::from_raw(raw)?,
                        "kem_policy.mode",
                    )?;
                }
                x if x == KemField::PreferredSuite as u8 => {
                    let raw = decoder.read_unsigned()?;
                    let suite: u8 =
                        raw.try_into()
                            .map_err(|_| CertificateError::InvalidFieldValue {
                                field: "kem_policy.preferred_suite",
                                reason: format!("value {raw} exceeds u8::MAX"),
                            })?;
                    validate_kem_suite_id(suite, "kem_policy.preferred_suite")?;
                    set_decoded_once(&mut preferred_suite, suite, "kem_policy.preferred_suite")?;
                }
                x if x == KemField::FallbackSuite as u8 => {
                    let value = if decoder.peek_is_null()? {
                        decoder.read_null()?;
                        None
                    } else {
                        let raw = decoder.read_unsigned()?;
                        let suite: u8 =
                            raw.try_into()
                                .map_err(|_| CertificateError::InvalidFieldValue {
                                    field: "kem_policy.fallback_suite",
                                    reason: format!("value {raw} exceeds u8::MAX"),
                                })?;
                        validate_kem_suite_id(suite, "kem_policy.fallback_suite")?;
                        Some(suite)
                    };
                    set_decoded_once(&mut fallback_suite, value, "kem_policy.fallback_suite")?;
                }
                x if x == KemField::RotationIntervalHours as u8 => {
                    set_decoded_once(
                        &mut rotation_interval_hours,
                        decoder.read_u16()?,
                        "kem_policy.rotation_interval_hours",
                    )?;
                }
                x if x == KemField::GracePeriodHours as u8 => {
                    set_decoded_once(
                        &mut grace_period_hours,
                        decoder.read_u16()?,
                        "kem_policy.grace_period_hours",
                    )?;
                }
                other => {
                    return Err(CertificateError::UnknownField {
                        field: format!("kem_policy.{other}"),
                    });
                }
            }
        }

        let policy = Self {
            mode: mode.ok_or(CertificateError::MissingField {
                field: "kem_policy.mode",
            })?,
            preferred_suite: preferred_suite.ok_or(CertificateError::MissingField {
                field: "kem_policy.preferred_suite",
            })?,
            fallback_suite: fallback_suite.unwrap_or(None),
            rotation_interval_hours: rotation_interval_hours.ok_or(
                CertificateError::MissingField {
                    field: "kem_policy.rotation_interval_hours",
                },
            )?,
            grace_period_hours: grace_period_hours.ok_or(CertificateError::MissingField {
                field: "kem_policy.grace_period_hours",
            })?,
        };
        policy.validate_semantics()
    }

    fn validate_semantics(self) -> Result<Self, CertificateError> {
        if self.fallback_suite == Some(self.preferred_suite) {
            return Err(CertificateError::InvalidFieldValue {
                field: "kem_policy.fallback_suite",
                reason: "must differ from preferred suite".to_owned(),
            });
        }
        match self.mode {
            KemRotationModeV1::Static => {
                if self.fallback_suite.is_some() {
                    return Err(CertificateError::InvalidFieldValue {
                        field: "kem_policy.fallback_suite",
                        reason: "static policies must not advertise a fallback suite".to_owned(),
                    });
                }
                if self.rotation_interval_hours != 0 {
                    return Err(CertificateError::InvalidFieldValue {
                        field: "kem_policy.rotation_interval_hours",
                        reason: "static policies must use a zero rotation interval".to_owned(),
                    });
                }
                if self.grace_period_hours != 0 {
                    return Err(CertificateError::InvalidFieldValue {
                        field: "kem_policy.grace_period_hours",
                        reason: "static policies must use a zero grace period".to_owned(),
                    });
                }
            }
            KemRotationModeV1::Staged => {
                if self.fallback_suite.is_none() {
                    return Err(CertificateError::InvalidFieldValue {
                        field: "kem_policy.fallback_suite",
                        reason: "staged policies must advertise a fallback suite".to_owned(),
                    });
                }
            }
            KemRotationModeV1::Rolling => {
                if self.rotation_interval_hours == 0 {
                    return Err(CertificateError::InvalidFieldValue {
                        field: "kem_policy.rotation_interval_hours",
                        reason: "rolling policies must use a nonzero rotation interval".to_owned(),
                    });
                }
            }
        }
        Ok(self)
    }
}

/// Relay endpoint entry embedded in the SRC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelayEndpointV2 {
    /// Scheme-qualified endpoint URL.
    pub url: String,
    /// Priority (lower numbers are preferred).
    pub priority: u8,
    /// Stream tags advertised by the endpoint.
    pub tags: Vec<String>,
}

impl RelayEndpointV2 {
    fn encode(&self, encoder: &mut CborEncoder) {
        encoder.write_map_header(3);
        encoder.write_unsigned(EndpointField::Url as u64);
        encoder.write_text(&self.url);
        encoder.write_unsigned(EndpointField::Priority as u64);
        encoder.write_unsigned(self.priority.into());
        encoder.write_unsigned(EndpointField::Tags as u64);
        encoder.write_array_header(self.tags.len() as u64);
        for tag in &self.tags {
            encoder.write_text(tag);
        }
    }

    fn decode(decoder: &mut CborDecoder) -> Result<Self, CertificateError> {
        let map_len = decoder.read_map_len()?;
        let mut url = None;
        let mut priority = None;
        let mut tags: Option<Vec<String>> = None;

        for _ in 0..map_len {
            let raw_key = decoder.read_unsigned()?;
            let key: u8 = raw_key
                .try_into()
                .map_err(|_| CertificateError::InvalidFieldValue {
                    field: "endpoint",
                    reason: format!("field key {raw_key} exceeds u8::MAX"),
                })?;
            match key {
                x if x == EndpointField::Url as u8 => {
                    set_decoded_once(&mut url, decoder.read_text()?, "endpoint.url")?;
                }
                x if x == EndpointField::Priority as u8 => {
                    let raw = decoder.read_unsigned()?;
                    let value: u8 =
                        raw.try_into()
                            .map_err(|_| CertificateError::InvalidFieldValue {
                                field: "endpoint.priority",
                                reason: format!("value {raw} exceeds u8::MAX"),
                            })?;
                    set_decoded_once(&mut priority, value, "endpoint.priority")?;
                }
                x if x == EndpointField::Tags as u8 => {
                    let len = decoder.read_array_len()?;
                    let capacity = capacity_from_len(len, "endpoint.tags")?;
                    let mut values = Vec::with_capacity(capacity);
                    for _ in 0..len {
                        values.push(decoder.read_text()?);
                    }
                    set_decoded_once(&mut tags, values, "endpoint.tags")?;
                }
                other => {
                    return Err(CertificateError::UnknownField {
                        field: format!("endpoint.{other}"),
                    });
                }
            }
        }

        let url = url.ok_or(CertificateError::MissingField {
            field: "endpoint.url",
        })?;
        validate_endpoint_url(&url)?;
        let priority = priority.ok_or(CertificateError::MissingField {
            field: "endpoint.priority",
        })?;
        let tags = tags.unwrap_or_default();
        validate_endpoint_tags(&tags)?;

        Ok(Self {
            url,
            priority,
            tags,
        })
    }
}

fn validate_endpoint_url(url: &str) -> Result<(), CertificateError> {
    if url.is_empty() {
        return Err(CertificateError::InvalidFieldValue {
            field: "endpoint.url",
            reason: "must not be empty".to_owned(),
        });
    }
    if url.chars().any(char::is_control) {
        return Err(CertificateError::InvalidFieldValue {
            field: "endpoint.url",
            reason: "must not contain control characters".to_owned(),
        });
    }
    if url.chars().any(char::is_whitespace) {
        return Err(CertificateError::InvalidFieldValue {
            field: "endpoint.url",
            reason: "must not contain whitespace characters".to_owned(),
        });
    }
    Ok(())
}

fn validate_endpoint_tags(tags: &[String]) -> Result<(), CertificateError> {
    let mut seen = Vec::with_capacity(tags.len());
    for tag in tags {
        if tag.is_empty() {
            return Err(CertificateError::InvalidFieldValue {
                field: "endpoint.tags",
                reason: "tag must not be empty".to_owned(),
            });
        }
        if tag.chars().any(char::is_control) {
            return Err(CertificateError::InvalidFieldValue {
                field: "endpoint.tags",
                reason: "tag must not contain control characters".to_owned(),
            });
        }
        if tag.chars().any(char::is_whitespace) {
            return Err(CertificateError::InvalidFieldValue {
                field: "endpoint.tags",
                reason: "tag must not contain whitespace characters".to_owned(),
            });
        }
        if seen.contains(&tag.as_str()) {
            return Err(CertificateError::InvalidFieldValue {
                field: "endpoint.tags",
                reason: format!("duplicate endpoint tag `{tag}`"),
            });
        }
        seen.push(tag.as_str());
    }
    Ok(())
}

/// Toggle used when constructing capability flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityToggle {
    /// Capability is disabled.
    Disabled,
    /// Capability is enabled.
    Enabled,
}

impl CapabilityToggle {
    /// Returns `true` when the toggle represents an enabled capability.
    #[must_use]
    pub const fn is_enabled(self) -> bool {
        matches!(self, Self::Enabled)
    }
}

/// Capability flags advertised in the SRC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RelayCapabilityFlagsV1 {
    bits: u16,
}

impl RelayCapabilityFlagsV1 {
    const ALL_FLAGS: u16 = capability_bits::BLINDED_CID
        | capability_bits::POW_TICKET
        | capability_bits::NORITO_STREAM
        | capability_bits::KAIGI_BRIDGE;

    /// Create capability flags from boolean toggles.
    #[must_use]
    pub const fn new(
        supports_blinded_cid: CapabilityToggle,
        requires_pow_ticket: CapabilityToggle,
        supports_norito_stream: CapabilityToggle,
        supports_kaigi_bridge: CapabilityToggle,
    ) -> Self {
        let mut bits = 0;
        if supports_blinded_cid.is_enabled() {
            bits |= capability_bits::BLINDED_CID;
        }
        if requires_pow_ticket.is_enabled() {
            bits |= capability_bits::POW_TICKET;
        }
        if supports_norito_stream.is_enabled() {
            bits |= capability_bits::NORITO_STREAM;
        }
        if supports_kaigi_bridge.is_enabled() {
            bits |= capability_bits::KAIGI_BRIDGE;
        }
        Self { bits }
    }

    /// Returns `true` when the relay publishes blinded `CID` cache keys.
    #[must_use]
    pub const fn supports_blinded_cid(self) -> bool {
        self.bits & capability_bits::BLINDED_CID != 0
    }

    /// Returns `true` when the relay enforces `PoW` tickets for circuit establishment.
    #[must_use]
    pub const fn requires_pow_ticket(self) -> bool {
        self.bits & capability_bits::POW_TICKET != 0
    }

    /// Returns `true` when the relay exposes Norito streaming endpoints.
    #[must_use]
    pub const fn supports_norito_stream(self) -> bool {
        self.bits & capability_bits::NORITO_STREAM != 0
    }

    /// Returns `true` when the relay bridges Kaigi rooms over `SoraNet`.
    #[must_use]
    pub const fn supports_kaigi_bridge(self) -> bool {
        self.bits & capability_bits::KAIGI_BRIDGE != 0
    }

    /// Convert to the packed bit representation used on-wire.
    #[must_use]
    pub const fn to_bits(self) -> u16 {
        self.bits
    }

    /// Construct flags from the packed bit representation.
    #[must_use]
    pub const fn from_bits(bits: u16) -> Self {
        Self {
            bits: bits & Self::ALL_FLAGS,
        }
    }

    fn try_from_certificate_bits(bits: u16) -> Result<Self, CertificateError> {
        let unknown_bits = bits & !Self::ALL_FLAGS;
        if unknown_bits != 0 {
            return Err(CertificateError::InvalidFieldValue {
                field: "certificate.capability_flags",
                reason: format!("unsupported capability bits {unknown_bits:#06x}"),
            });
        }
        Ok(Self { bits })
    }
}

/// Relay roles advertised in the SRC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RelayRolesV2 {
    /// Relay is eligible as an entry guard.
    pub entry: bool,
    /// Relay can act as a middle hop.
    pub middle: bool,
    /// Relay can terminate circuits (exit).
    pub exit: bool,
}

impl RelayRolesV2 {
    fn to_bits(self) -> u8 {
        u8::from(self.entry) | (u8::from(self.middle) << 1) | (u8::from(self.exit) << 2)
    }

    fn from_bits(bits: u8) -> Self {
        Self {
            entry: bits & 0x01 != 0,
            middle: bits & 0x02 != 0,
            exit: bits & 0x04 != 0,
        }
    }
}

/// Core `SRCv2` payload describing a relay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelayCertificateV2 {
    /// Stable relay identifier (BLAKE3 digest).
    pub relay_id: [u8; 32],
    /// Ed25519 identity key.
    pub identity_ed25519: [u8; 32],
    /// ML-DSA-65 identity key.
    pub identity_mldsa65: Vec<u8>,
    /// Directory descriptor commitment echoed during the handshake.
    pub descriptor_commit: [u8; 32],
    /// Relay role flags.
    pub roles: RelayRolesV2,
    /// Weight applied when selecting entry guards.
    pub guard_weight: u32,
    /// Sustained bandwidth in bytes per second.
    pub bandwidth_bytes_per_sec: u64,
    /// Reputation weight applied by the directory.
    pub reputation_weight: u32,
    /// Advertised circuit endpoints.
    pub endpoints: Vec<RelayEndpointV2>,
    /// Feature flags surfaced by the relay.
    pub capability_flags: RelayCapabilityFlagsV1,
    /// ML-KEM rotation policy surfaced by the relay.
    pub kem_policy: KemRotationPolicyV1,
    /// Supported handshake suites (preference order).
    pub handshake_suites: Vec<HandshakeSuite>,
    /// Publication timestamp (Unix seconds).
    pub published_at: i64,
    /// Valid after timestamp (Unix seconds).
    pub valid_after: i64,
    /// Valid until timestamp (Unix seconds).
    pub valid_until: i64,
    /// Directory consensus hash the certificate binds to.
    pub directory_hash: [u8; 32],
    /// Issuer fingerprint (e.g. governance signer).
    pub issuer_fingerprint: [u8; 32],
    /// ML-KEM public key advertised by the relay (Kyber-768).
    pub pq_kem_public: Vec<u8>,
}

impl RelayCertificateV2 {
    /// Serialize the certificate payload to canonical CBOR bytes.
    pub fn to_cbor(&self) -> Vec<u8> {
        let mut encoder = CborEncoder::new();
        encoder.write_map_header(19);

        encoder.write_unsigned(Field::Version as u64);
        encoder.write_unsigned(SRC_CERTIFICATE_VERSION.into());

        encoder.write_unsigned(Field::RelayId as u64);
        encoder.write_bytes(&self.relay_id);

        encoder.write_unsigned(Field::IdentityEd25519 as u64);
        encoder.write_bytes(&self.identity_ed25519);

        encoder.write_unsigned(Field::IdentityMlDsa65 as u64);
        encoder.write_bytes(&self.identity_mldsa65);

        encoder.write_unsigned(Field::DescriptorCommit as u64);
        encoder.write_bytes(&self.descriptor_commit);

        encoder.write_unsigned(Field::Roles as u64);
        encoder.write_unsigned(self.roles.to_bits().into());

        encoder.write_unsigned(Field::GuardWeight as u64);
        encoder.write_unsigned(self.guard_weight.into());

        encoder.write_unsigned(Field::BandwidthBytesPerSec as u64);
        encoder.write_unsigned(self.bandwidth_bytes_per_sec);

        encoder.write_unsigned(Field::ReputationWeight as u64);
        encoder.write_unsigned(self.reputation_weight.into());

        encoder.write_unsigned(Field::Endpoints as u64);
        encoder.write_array_header(self.endpoints.len() as u64);
        for endpoint in &self.endpoints {
            endpoint.encode(&mut encoder);
        }

        encoder.write_unsigned(Field::CapabilityFlags as u64);
        encoder.write_unsigned(self.capability_flags.to_bits().into());

        encoder.write_unsigned(Field::KemPolicy as u64);
        self.kem_policy.encode(&mut encoder);

        encoder.write_unsigned(Field::HandshakeSuites as u64);
        encoder.write_array_header(self.handshake_suites.len() as u64);
        for suite in &self.handshake_suites {
            encoder.write_unsigned((*suite as u8).into());
        }

        encoder.write_unsigned(Field::PublishedAt as u64);
        encoder.write_i64(self.published_at);

        encoder.write_unsigned(Field::ValidAfter as u64);
        encoder.write_i64(self.valid_after);

        encoder.write_unsigned(Field::ValidUntil as u64);
        encoder.write_i64(self.valid_until);

        encoder.write_unsigned(Field::DirectoryHash as u64);
        encoder.write_bytes(&self.directory_hash);

        encoder.write_unsigned(Field::IssuerFingerprint as u64);
        encoder.write_bytes(&self.issuer_fingerprint);

        encoder.write_unsigned(Field::PqKemPublic as u64);
        encoder.write_bytes(&self.pq_kem_public);

        encoder.finish()
    }

    /// Compute the BLAKE3 digest of the certificate payload.
    pub fn digest(&self) -> [u8; 32] {
        let mut hasher = Blake3::new();
        hasher.update(SRC_V2_DOMAIN);
        hasher.update(&self.to_cbor());
        hasher.finalize().into()
    }

    /// Issue a signed certificate bundle using the provided identity keys.
    ///
    /// # Errors
    /// Returns an error when either signature cannot be produced with the supplied keys.
    pub fn issue(
        self,
        ed25519_signing_key: &SigningKey,
        mldsa_secret_key: &[u8],
    ) -> Result<RelayCertificateBundleV2, CertificateError> {
        let payload = self.to_cbor();
        parse_certificate_payload(&payload)?;
        MlDsaSuite::MlDsa65
            .validate_secret_key(mldsa_secret_key)
            .map_err(|err| {
                CertificateError::SignatureFailure(format!("ML-DSA secret key is invalid: {err}"))
            })?;
        let digest = compute_signing_digest(SRC_V2_ED25519_DOMAIN, &payload);

        let ed25519_signature: Signature = ed25519_signing_key.sign(&digest);

        let mldsa_digest = compute_signing_digest(SRC_V2_MLDSA_DOMAIN, &payload);
        let mldsa_signature =
            sign_mldsa_from_os(MlDsaSuite::MlDsa65, mldsa_secret_key, &[], &mldsa_digest).map_err(
                |err| CertificateError::SignatureFailure(format!("ML-DSA signing failed: {err}")),
            )?;

        Ok(RelayCertificateBundleV2 {
            certificate: self,
            signatures: RelayCertificateSignaturesV2 {
                ed25519: ed25519_signature.to_bytes(),
                mldsa65: Some(mldsa_signature.as_bytes().to_vec()),
            },
        })
    }

    /// Returns the length of the validity window.
    pub fn validity_duration(&self) -> Duration {
        let seconds_i64 = (self.valid_until - self.valid_after).max(0);
        let seconds =
            u64::try_from(seconds_i64).expect("difference is clamped to a non-negative range");
        Duration::from_secs(seconds)
    }
}

/// Signatures attached to an `SRCv2` payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelayCertificateSignaturesV2 {
    /// Ed25519 signature over the canonical certificate payload.
    pub ed25519: [u8; 64],
    /// Optional ML-DSA-65 signature over the canonical payload.
    pub mldsa65: Option<Vec<u8>>,
}

impl RelayCertificateSignaturesV2 {
    fn encode(&self, encoder: &mut CborEncoder) {
        encoder.write_map_header(2);
        encoder.write_unsigned(SignatureField::Ed25519 as u64);
        encoder.write_bytes(&self.ed25519);
        encoder.write_unsigned(SignatureField::MlDsa65 as u64);
        match &self.mldsa65 {
            Some(bytes) => encoder.write_bytes(bytes),
            None => encoder.write_null(),
        }
    }

    fn decode(decoder: &mut CborDecoder) -> Result<Self, CertificateError> {
        let map_len = decoder.read_map_len()?;
        let mut ed25519: Option<[u8; 64]> = None;
        let mut mldsa: Option<Option<Vec<u8>>> = None;

        for _ in 0..map_len {
            let raw_key = decoder.read_unsigned()?;
            let key: u8 = raw_key
                .try_into()
                .map_err(|_| CertificateError::InvalidFieldValue {
                    field: "signatures",
                    reason: format!("field key {raw_key} exceeds u8::MAX"),
                })?;
            match key {
                x if x == SignatureField::Ed25519 as u8 => {
                    let bytes = decoder.read_bytes()?;
                    let len = bytes.len();
                    let array: [u8; 64] =
                        bytes
                            .try_into()
                            .map_err(|_| CertificateError::InvalidFieldValue {
                                field: "signatures.ed25519",
                                reason: format!("expected 64 bytes, got {len}"),
                            })?;
                    set_decoded_once(&mut ed25519, array, "signatures.ed25519")?;
                }
                x if x == SignatureField::MlDsa65 as u8 => {
                    let value = if decoder.peek_is_null()? {
                        decoder.read_null()?;
                        None
                    } else {
                        let bytes = decoder.read_bytes()?;
                        validate_exact_len(
                            "signatures.mldsa65",
                            bytes.len(),
                            MlDsaSuite::MlDsa65.signature_len(),
                        )?;
                        Some(bytes)
                    };
                    set_decoded_once(&mut mldsa, value, "signatures.mldsa65")?;
                }
                other => {
                    return Err(CertificateError::UnknownField {
                        field: format!("signatures.{other}"),
                    });
                }
            }
        }

        Ok(Self {
            ed25519: ed25519.ok_or(CertificateError::MissingField {
                field: "signatures.ed25519",
            })?,
            mldsa65: mldsa.unwrap_or(None),
        })
    }
}

/// `SRCv2` bundle (payload + signatures).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelayCertificateBundleV2 {
    /// Certificate payload.
    pub certificate: RelayCertificateV2,
    /// Detached signatures.
    pub signatures: RelayCertificateSignaturesV2,
}

impl RelayCertificateBundleV2 {
    /// Serialize the bundle to CBOR.
    pub fn to_cbor(&self) -> Vec<u8> {
        let mut encoder = CborEncoder::new();
        encoder.write_map_header(2);
        encoder.write_unsigned(0);
        encoder.write_bytes(&self.certificate.to_cbor());
        encoder.write_unsigned(1);
        self.signatures.encode(&mut encoder);
        encoder.finish()
    }

    /// Deserialize a bundle from CBOR.
    ///
    /// # Errors
    /// Returns an error when the CBOR payload is structurally invalid or references
    /// unsupported certificate fields.
    pub fn from_cbor(bytes: &[u8]) -> Result<Self, CertificateError> {
        let mut decoder = CborDecoder::new(bytes);
        let map_len = decoder.read_map_len()?;
        let mut certificate = None;
        let mut signatures = None;

        for _ in 0..map_len {
            let key = decoder.read_unsigned()?;
            match key {
                0 => {
                    let payload_bytes = decoder.read_bytes()?;
                    set_decoded_once(
                        &mut certificate,
                        parse_certificate_payload(&payload_bytes)?,
                        "bundle.certificate",
                    )?;
                }
                1 => {
                    set_decoded_once(
                        &mut signatures,
                        RelayCertificateSignaturesV2::decode(&mut decoder)?,
                        "bundle.signatures",
                    )?;
                }
                other => {
                    return Err(CertificateError::UnknownField {
                        field: format!("bundle.{other}"),
                    });
                }
            }
        }
        decoder.ensure_finished()?;

        Ok(Self {
            certificate: certificate.ok_or(CertificateError::MissingField {
                field: "bundle.certificate",
            })?,
            signatures: signatures.ok_or(CertificateError::MissingField {
                field: "bundle.signatures",
            })?,
        })
    }

    /// Verify the bundle signatures according to the supplied validation phase.
    ///
    /// # Errors
    /// Returns an error when the signatures are invalid, the bundle metadata does not
    /// match the provided context, or the local system clock fails.
    pub fn verify(
        &self,
        ed25519_public: &VerifyingKey,
        mldsa_public: &[u8],
        phase: CertificateValidationPhase,
    ) -> Result<(), CertificateError> {
        let payload = self.certificate.to_cbor();
        let ed_digest = compute_signing_digest(SRC_V2_ED25519_DOMAIN, &payload);
        if ed25519_public.is_weak() {
            return Err(CertificateError::SignatureFailure(
                "Ed25519 public key is small-order (weak); rejected".to_owned(),
            ));
        }
        ed25519_public
            .verify_strict(&ed_digest, &Signature::from_bytes(&self.signatures.ed25519))
            .map_err(|err| {
                CertificateError::SignatureFailure(format!("Ed25519 verify failed: {err}"))
            })?;

        match (&self.signatures.mldsa65, phase) {
            (Some(bytes), _) => {
                validate_mldsa_verify_material(mldsa_public, bytes)?;
                let digest = compute_signing_digest(SRC_V2_MLDSA_DOMAIN, &payload);
                verify_mldsa(MlDsaSuite::MlDsa65, mldsa_public, &[], &digest, bytes).map_err(
                    |err| {
                        CertificateError::SignatureFailure(format!("ML-DSA verify failed: {err}"))
                    },
                )?;
            }
            (
                None,
                CertificateValidationPhase::Phase1AllowSingle
                | CertificateValidationPhase::Phase2PreferDual,
            ) => {}
            (None, CertificateValidationPhase::Phase3RequireDual) => {
                return Err(CertificateError::MissingMldsaSignature {
                    phase: "Phase3RequireDual",
                });
            }
        }

        Ok(())
    }
}

fn validate_mldsa_verify_material(
    public_key: &[u8],
    signature: &[u8],
) -> Result<(), CertificateError> {
    MlDsaSuite::MlDsa65
        .validate_public_key(public_key)
        .map_err(|err| {
            CertificateError::SignatureFailure(format!("ML-DSA public key is invalid: {err}"))
        })?;
    MlDsaSuite::MlDsa65
        .validate_signature(signature)
        .map_err(|err| {
            CertificateError::SignatureFailure(format!("ML-DSA signature is invalid: {err}"))
        })?;
    Ok(())
}

#[derive(Default)]
struct CertificateFieldAccumulator {
    version_seen: bool,
    relay_id: Option<[u8; 32]>,
    identity_ed25519: Option<[u8; 32]>,
    identity_mldsa65: Option<Vec<u8>>,
    descriptor_commit: Option<[u8; 32]>,
    roles: Option<RelayRolesV2>,
    guard_weight: Option<u32>,
    bandwidth_bytes_per_sec: Option<u64>,
    reputation_weight: Option<u32>,
    endpoints: Option<Vec<RelayEndpointV2>>,
    capability_flags: Option<RelayCapabilityFlagsV1>,
    kem_policy: Option<KemRotationPolicyV1>,
    handshake_suites: Option<Vec<HandshakeSuite>>,
    published_at: Option<i64>,
    valid_after: Option<i64>,
    valid_until: Option<i64>,
    directory_hash: Option<[u8; 32]>,
    issuer_fingerprint: Option<[u8; 32]>,
    pq_kem_public: Option<Vec<u8>>,
}

impl CertificateFieldAccumulator {
    fn set_once<T>(slot: &mut Option<T>, value: T, field: Field) -> Result<(), CertificateError> {
        if slot.is_some() {
            return Err(CertificateError::DuplicateField {
                field: field_label(field),
            });
        }
        *slot = Some(value);
        Ok(())
    }

    fn decode(
        mut self,
        decoder: &mut CborDecoder<'_>,
        map_len: u64,
    ) -> Result<Self, CertificateError> {
        for _ in 0..map_len {
            let field = read_certificate_field_key(decoder)?;
            self.decode_field(decoder, field)?;
        }
        Ok(self)
    }

    fn decode_field(
        &mut self,
        decoder: &mut CborDecoder<'_>,
        field: Field,
    ) -> Result<(), CertificateError> {
        match field {
            Field::Version => {
                if self.version_seen {
                    return Err(CertificateError::DuplicateField {
                        field: field_label(field),
                    });
                }
                self.version_seen = true;
                verify_certificate_version(decoder)?;
            }
            Field::RelayId => {
                Self::set_once(&mut self.relay_id, decoder.read_array32()?, field)?;
            }
            Field::IdentityEd25519 => {
                Self::set_once(&mut self.identity_ed25519, decoder.read_array32()?, field)?;
            }
            Field::IdentityMlDsa65 => {
                Self::set_once(&mut self.identity_mldsa65, decoder.read_bytes()?, field)?;
            }
            Field::DescriptorCommit => {
                Self::set_once(&mut self.descriptor_commit, decoder.read_array32()?, field)?;
            }
            Field::Roles => {
                Self::set_once(&mut self.roles, decode_roles(decoder)?, field)?;
            }
            Field::GuardWeight => {
                Self::set_once(&mut self.guard_weight, decoder.read_u32()?, field)?;
            }
            Field::BandwidthBytesPerSec => {
                Self::set_once(
                    &mut self.bandwidth_bytes_per_sec,
                    decoder.read_unsigned()?,
                    field,
                )?;
            }
            Field::ReputationWeight => {
                Self::set_once(&mut self.reputation_weight, decoder.read_u32()?, field)?;
            }
            Field::Endpoints => {
                Self::set_once(&mut self.endpoints, decode_endpoints(decoder)?, field)?;
            }
            Field::CapabilityFlags => {
                let bits = decoder.read_u16()?;
                Self::set_once(
                    &mut self.capability_flags,
                    RelayCapabilityFlagsV1::try_from_certificate_bits(bits)?,
                    field,
                )?;
            }
            Field::KemPolicy => {
                Self::set_once(
                    &mut self.kem_policy,
                    KemRotationPolicyV1::decode(decoder)?,
                    field,
                )?;
            }
            Field::HandshakeSuites => {
                Self::set_once(
                    &mut self.handshake_suites,
                    decode_handshake_suites(decoder)?,
                    field,
                )?;
            }
            Field::PublishedAt => {
                Self::set_once(&mut self.published_at, decoder.read_i64()?, field)?;
            }
            Field::ValidAfter => {
                Self::set_once(&mut self.valid_after, decoder.read_i64()?, field)?;
            }
            Field::ValidUntil => {
                Self::set_once(&mut self.valid_until, decoder.read_i64()?, field)?;
            }
            Field::DirectoryHash => {
                Self::set_once(&mut self.directory_hash, decoder.read_array32()?, field)?;
            }
            Field::IssuerFingerprint => {
                Self::set_once(&mut self.issuer_fingerprint, decoder.read_array32()?, field)?;
            }
            Field::PqKemPublic => {
                Self::set_once(&mut self.pq_kem_public, decoder.read_bytes()?, field)?;
            }
        }
        Ok(())
    }

    fn into_certificate(self) -> Result<RelayCertificateV2, CertificateError> {
        let relay_id = require_field(self.relay_id, "certificate.relay_id")?;
        let identity_ed25519 =
            require_field(self.identity_ed25519, "certificate.identity_ed25519")?;
        let identity_mldsa65 =
            require_field(self.identity_mldsa65, "certificate.identity_mldsa65")?;
        let descriptor_commit =
            require_field(self.descriptor_commit, "certificate.descriptor_commit")?;
        let roles = require_field(self.roles, "certificate.roles")?;
        let guard_weight = require_field(self.guard_weight, "certificate.guard_weight")?;
        let bandwidth_bytes_per_sec = require_field(
            self.bandwidth_bytes_per_sec,
            "certificate.bandwidth_bytes_per_sec",
        )?;
        let reputation_weight =
            require_field(self.reputation_weight, "certificate.reputation_weight")?;
        let endpoints = require_field(self.endpoints, "certificate.endpoints")?;
        let capability_flags = self.capability_flags.unwrap_or_default();
        let kem_policy = require_field(self.kem_policy, "certificate.kem_policy")?;
        let handshake_suites =
            require_field(self.handshake_suites, "certificate.handshake_suites")?;
        let published_at = require_field(self.published_at, "certificate.published_at")?;
        let valid_after = require_field(self.valid_after, "certificate.valid_after")?;
        let valid_until = require_field(self.valid_until, "certificate.valid_until")?;
        let directory_hash = require_field(self.directory_hash, "certificate.directory_hash")?;
        let issuer_fingerprint =
            require_field(self.issuer_fingerprint, "certificate.issuer_fingerprint")?;
        let pq_kem_public = require_field(self.pq_kem_public, "certificate.pq_kem_public")?;

        validate_exact_len(
            "certificate.identity_mldsa65",
            identity_mldsa65.len(),
            MlDsaSuite::MlDsa65.public_key_len(),
        )?;
        validate_ed25519_identity_key(&identity_ed25519)?;
        let preferred_suite =
            MlKemSuite::from_kem_id(kem_policy.preferred_suite).ok_or_else(|| {
                CertificateError::InvalidFieldValue {
                    field: "certificate.kem_policy.preferred_suite",
                    reason: format!("unsupported suite {}", kem_policy.preferred_suite),
                }
            })?;
        validate_exact_len(
            "certificate.pq_kem_public",
            pq_kem_public.len(),
            preferred_suite.public_key_len(),
        )?;
        validate_certificate_time_bounds(published_at, valid_after, valid_until)?;

        Ok(RelayCertificateV2 {
            relay_id,
            identity_ed25519,
            identity_mldsa65,
            descriptor_commit,
            roles,
            guard_weight,
            bandwidth_bytes_per_sec,
            reputation_weight,
            endpoints,
            capability_flags,
            kem_policy,
            handshake_suites,
            published_at,
            valid_after,
            valid_until,
            directory_hash,
            issuer_fingerprint,
            pq_kem_public,
        })
    }
}

fn validate_kem_suite_id(suite: u8, field: &'static str) -> Result<(), CertificateError> {
    if MlKemSuite::from_kem_id(suite).is_none() {
        return Err(CertificateError::InvalidFieldValue {
            field,
            reason: format!("unsupported suite {suite}"),
        });
    }
    Ok(())
}

fn validate_exact_len(
    field: &'static str,
    actual: usize,
    expected: usize,
) -> Result<(), CertificateError> {
    if actual != expected {
        return Err(CertificateError::InvalidFieldValue {
            field,
            reason: format!("expected {expected} bytes, got {actual}"),
        });
    }
    Ok(())
}

fn validate_ed25519_identity_key(bytes: &[u8; 32]) -> Result<(), CertificateError> {
    crate::signature::ed25519::Ed25519Sha512::parse_public_key(bytes).map_err(|err| {
        CertificateError::InvalidFieldValue {
            field: "certificate.identity_ed25519",
            reason: format!("invalid Ed25519 public key: {err}"),
        }
    })?;
    Ok(())
}

fn validate_certificate_time_bounds(
    published_at: i64,
    valid_after: i64,
    valid_until: i64,
) -> Result<(), CertificateError> {
    if valid_after >= valid_until {
        return Err(CertificateError::InvalidFieldValue {
            field: "certificate.valid_until",
            reason: "valid_until must be greater than valid_after".to_owned(),
        });
    }
    if published_at > valid_until {
        return Err(CertificateError::InvalidFieldValue {
            field: "certificate.published_at",
            reason: "published_at must not be after valid_until".to_owned(),
        });
    }
    Ok(())
}

fn read_certificate_field_key(decoder: &mut CborDecoder<'_>) -> Result<Field, CertificateError> {
    let raw_key = decoder.read_unsigned()?;
    let key: u8 = raw_key
        .try_into()
        .map_err(|_| CertificateError::InvalidFieldValue {
            field: "certificate",
            reason: format!("field key {raw_key} exceeds u8::MAX"),
        })?;
    match key {
        x if x == Field::Version as u8 => Ok(Field::Version),
        x if x == Field::RelayId as u8 => Ok(Field::RelayId),
        x if x == Field::IdentityEd25519 as u8 => Ok(Field::IdentityEd25519),
        x if x == Field::IdentityMlDsa65 as u8 => Ok(Field::IdentityMlDsa65),
        x if x == Field::DescriptorCommit as u8 => Ok(Field::DescriptorCommit),
        x if x == Field::Roles as u8 => Ok(Field::Roles),
        x if x == Field::GuardWeight as u8 => Ok(Field::GuardWeight),
        x if x == Field::BandwidthBytesPerSec as u8 => Ok(Field::BandwidthBytesPerSec),
        x if x == Field::ReputationWeight as u8 => Ok(Field::ReputationWeight),
        x if x == Field::Endpoints as u8 => Ok(Field::Endpoints),
        x if x == Field::CapabilityFlags as u8 => Ok(Field::CapabilityFlags),
        x if x == Field::KemPolicy as u8 => Ok(Field::KemPolicy),
        x if x == Field::HandshakeSuites as u8 => Ok(Field::HandshakeSuites),
        x if x == Field::PublishedAt as u8 => Ok(Field::PublishedAt),
        x if x == Field::ValidAfter as u8 => Ok(Field::ValidAfter),
        x if x == Field::ValidUntil as u8 => Ok(Field::ValidUntil),
        x if x == Field::DirectoryHash as u8 => Ok(Field::DirectoryHash),
        x if x == Field::IssuerFingerprint as u8 => Ok(Field::IssuerFingerprint),
        x if x == Field::PqKemPublic as u8 => Ok(Field::PqKemPublic),
        other => Err(CertificateError::UnknownField {
            field: format!("certificate.{other}"),
        }),
    }
}

fn decode_endpoints(
    decoder: &mut CborDecoder<'_>,
) -> Result<Vec<RelayEndpointV2>, CertificateError> {
    let len = decoder.read_array_len()?;
    if len == 0 {
        return Err(CertificateError::InvalidFieldValue {
            field: "certificate.endpoints",
            reason: "must advertise at least one endpoint".to_owned(),
        });
    }
    let capacity = capacity_from_len(len, "certificate.endpoints")?;
    let mut values = Vec::with_capacity(capacity);
    for _ in 0..len {
        let endpoint = RelayEndpointV2::decode(decoder)?;
        if values
            .iter()
            .any(|prior: &RelayEndpointV2| prior.url == endpoint.url)
        {
            return Err(CertificateError::InvalidFieldValue {
                field: "certificate.endpoints",
                reason: format!("duplicate endpoint URL `{}`", endpoint.url),
            });
        }
        values.push(endpoint);
    }
    Ok(values)
}

fn decode_handshake_suites(
    decoder: &mut CborDecoder<'_>,
) -> Result<Vec<HandshakeSuite>, CertificateError> {
    let len = decoder.read_array_len()?;
    if len == 0 {
        return Err(CertificateError::InvalidFieldValue {
            field: "certificate.handshake_suites",
            reason: "must advertise at least one handshake suite".to_owned(),
        });
    }
    let capacity = capacity_from_len(len, "certificate.handshake_suites")?;
    let mut suites = Vec::with_capacity(capacity);
    for _ in 0..len {
        let raw = decoder.read_unsigned()?;
        let value: u8 = raw
            .try_into()
            .map_err(|_| CertificateError::InvalidFieldValue {
                field: "certificate.handshake_suites",
                reason: format!("value {raw} exceeds u8::MAX"),
            })?;
        let suite =
            HandshakeSuite::try_from(value).map_err(|_| CertificateError::InvalidFieldValue {
                field: "certificate.handshake_suites",
                reason: format!("unknown suite identifier {value:#04x}"),
            })?;
        if suites.contains(&suite) {
            return Err(CertificateError::InvalidFieldValue {
                field: "certificate.handshake_suites",
                reason: format!("duplicate suite identifier {value:#04x}"),
            });
        }
        suites.push(suite);
    }
    Ok(suites)
}

fn decode_roles(decoder: &mut CborDecoder<'_>) -> Result<RelayRolesV2, CertificateError> {
    let raw = decoder.read_unsigned()?;
    let bits: u8 = raw
        .try_into()
        .map_err(|_| CertificateError::InvalidFieldValue {
            field: "certificate.roles",
            reason: format!("value {raw} exceeds u8::MAX"),
        })?;
    let unknown_bits = bits & !0x07;
    if unknown_bits != 0 {
        return Err(CertificateError::InvalidFieldValue {
            field: "certificate.roles",
            reason: format!("unsupported role bits {unknown_bits:#04x}"),
        });
    }
    Ok(RelayRolesV2::from_bits(bits))
}

fn verify_certificate_version(decoder: &mut CborDecoder<'_>) -> Result<(), CertificateError> {
    let version = decoder.read_unsigned()?;
    if version != u64::from(SRC_CERTIFICATE_VERSION) {
        return Err(CertificateError::InvalidFieldValue {
            field: "certificate.version",
            reason: format!("expected {SRC_CERTIFICATE_VERSION}, got {version}"),
        });
    }
    Ok(())
}

fn set_decoded_once<T>(
    slot: &mut Option<T>,
    value: T,
    field: &'static str,
) -> Result<(), CertificateError> {
    if slot.is_some() {
        return Err(CertificateError::DuplicateField { field });
    }
    *slot = Some(value);
    Ok(())
}

fn require_field<T>(value: Option<T>, field: &'static str) -> Result<T, CertificateError> {
    value.ok_or(CertificateError::MissingField { field })
}

fn parse_certificate_payload(bytes: &[u8]) -> Result<RelayCertificateV2, CertificateError> {
    let mut decoder = CborDecoder::new(bytes);
    let map_len = decoder.read_map_len()?;
    let fields = CertificateFieldAccumulator::default().decode(&mut decoder, map_len)?;
    decoder.ensure_finished()?;
    fields.into_certificate()
}

fn compute_signing_digest(domain: &[u8], payload: &[u8]) -> [u8; 32] {
    let mut hasher = Blake3::new();
    hasher.update(domain);
    hasher.update(payload);
    hasher.finalize().into()
}

/// Errors surfaced while encoding, decoding, or verifying `SRCv2` bundles.
#[derive(Debug, Error)]
pub enum CertificateError {
    /// The CBOR payload was malformed or truncated.
    #[error("invalid CBOR payload: {0}")]
    InvalidCbor(&'static str),
    /// A required field was missing during decoding.
    #[error("missing certificate field `{field}`")]
    MissingField {
        /// Name of the missing field.
        field: &'static str,
    },
    /// An unexpected or unknown field was encountered.
    #[error("unknown certificate field `{field}`")]
    UnknownField {
        /// Name of the field that was not recognised by the decoder.
        field: String,
    },
    /// A field contained an unsupported value.
    #[error("invalid value for `{field}`: {reason}")]
    InvalidFieldValue {
        /// Field whose value could not be parsed or validated.
        field: &'static str,
        /// Human-readable explanation of the validation failure.
        reason: String,
    },
    /// A field appeared more than once in the CBOR map.
    #[error("duplicate certificate field `{field}`")]
    DuplicateField {
        /// Name of the duplicated field.
        field: &'static str,
    },
    /// Signature verification or creation failed.
    #[error("{0}")]
    SignatureFailure(String),
    /// The validation phase required a ML-DSA signature that was absent.
    #[error("ML-DSA signature missing ({phase})")]
    MissingMldsaSignature {
        /// Handshake phase for which the ML-DSA signature was expected.
        phase: &'static str,
    },
}

fn capacity_from_len(len: u64, field: &'static str) -> Result<usize, CertificateError> {
    usize::try_from(len).map_err(|_| CertificateError::InvalidFieldValue {
        field,
        reason: format!("array length {len} exceeds usize::MAX"),
    })
}

/// Minimal CBOR encoder specialised for `SRCv2` structures.
struct CborEncoder {
    /// Accumulates the encoded CBOR bytes.
    buffer: Vec<u8>,
}

impl CborEncoder {
    fn new() -> Self {
        Self { buffer: Vec::new() }
    }

    fn finish(self) -> Vec<u8> {
        self.buffer
    }

    fn write_unsigned(&mut self, value: u64) {
        encode_unsigned(&mut self.buffer, value);
    }

    fn write_i64(&mut self, value: i64) {
        encode_i64(&mut self.buffer, value);
    }

    fn write_bytes(&mut self, bytes: &[u8]) {
        encode_bytes(&mut self.buffer, bytes);
    }

    fn write_text(&mut self, text: &str) {
        encode_text(&mut self.buffer, text);
    }

    fn write_array_header(&mut self, len: u64) {
        encode_major(&mut self.buffer, 4, len);
    }

    fn write_map_header(&mut self, len: u64) {
        encode_major(&mut self.buffer, 5, len);
    }

    fn write_null(&mut self) {
        self.buffer.push(0xf6);
    }
}

fn encode_major(buf: &mut Vec<u8>, major: u8, value: u64) {
    debug_assert!(major <= 7);
    if value < 24 {
        buf.push((major << 5) | u8::try_from(value).expect("values < 24 always fit in u8"));
    } else if value <= 0xFF {
        buf.push((major << 5) | 24);
        buf.push(u8::try_from(value).expect("values <= 0xFF fit in u8"));
    } else if value <= 0xFFFF {
        buf.push((major << 5) | 25);
        buf.extend_from_slice(
            &u16::try_from(value)
                .expect("values <= 0xFFFF fit in u16")
                .to_be_bytes(),
        );
    } else if value <= 0xFFFF_FFFF {
        buf.push((major << 5) | 26);
        buf.extend_from_slice(
            &u32::try_from(value)
                .expect("values <= 0xFFFF_FFFF fit in u32")
                .to_be_bytes(),
        );
    } else {
        buf.push((major << 5) | 27);
        buf.extend_from_slice(&value.to_be_bytes());
    }
}

fn encode_unsigned(buf: &mut Vec<u8>, value: u64) {
    encode_major(buf, 0, value);
}

fn encode_i64(buf: &mut Vec<u8>, value: i64) {
    if value >= 0 {
        let magnitude = u64::try_from(value).expect("non-negative i64 fits in u64");
        encode_major(buf, 0, magnitude);
    } else {
        let magnitude = u64::try_from(-1 - value).expect("conversion preserves magnitude");
        encode_major(buf, 1, magnitude);
    }
}

fn encode_bytes(buf: &mut Vec<u8>, bytes: &[u8]) {
    let len = u64::try_from(bytes.len()).expect("slice length fits in u64");
    encode_major(buf, 2, len);
    buf.extend_from_slice(bytes);
}

fn encode_text(buf: &mut Vec<u8>, text: &str) {
    let len = u64::try_from(text.len()).expect("text length fits in u64");
    encode_major(buf, 3, len);
    buf.extend_from_slice(text.as_bytes());
}

/// Minimal CBOR decoder specialised for `SRCv2` structures.
struct CborDecoder<'a> {
    /// Remaining CBOR bytes to decode.
    data: &'a [u8],
    /// Current cursor offset within `data`.
    pos: usize,
}

impl<'a> CborDecoder<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn read_u8(&mut self) -> Result<u8, CertificateError> {
        if self.pos >= self.data.len() {
            return Err(CertificateError::InvalidCbor("unexpected end of input"));
        }
        let value = self.data[self.pos];
        self.pos += 1;
        Ok(value)
    }

    fn read_unsigned(&mut self) -> Result<u64, CertificateError> {
        let (major, value) = self.read_major()?;
        if major != 0 {
            return Err(CertificateError::InvalidCbor(
                "expected unsigned integer major type",
            ));
        }
        Ok(value)
    }

    fn read_u16(&mut self) -> Result<u16, CertificateError> {
        let value = self.read_unsigned()?;
        value
            .try_into()
            .map_err(|_| CertificateError::InvalidCbor("u16 out of range"))
    }

    fn read_u32(&mut self) -> Result<u32, CertificateError> {
        let value = self.read_unsigned()?;
        value
            .try_into()
            .map_err(|_| CertificateError::InvalidCbor("u32 out of range"))
    }

    fn read_i64(&mut self) -> Result<i64, CertificateError> {
        let (major, value) = self.read_major()?;
        match major {
            0 => {
                i64::try_from(value).map_err(|_| CertificateError::InvalidCbor("i64 out of range"))
            }
            1 => {
                let magnitude = i64::try_from(value)
                    .map_err(|_| CertificateError::InvalidCbor("i64 out of range"))?;
                Ok(-1 - magnitude)
            }
            _ => Err(CertificateError::InvalidCbor("expected integer major type")),
        }
    }

    fn read_bytes(&mut self) -> Result<Vec<u8>, CertificateError> {
        let (major, len) = self.read_major()?;
        if major != 2 {
            return Err(CertificateError::InvalidCbor(
                "expected byte string major type",
            ));
        }
        let len = usize::try_from(len)
            .map_err(|_| CertificateError::InvalidCbor("byte string length exceeds usize"))?;
        if self.pos + len > self.data.len() {
            return Err(CertificateError::InvalidCbor("byte string truncated"));
        }
        let slice = &self.data[self.pos..self.pos + len];
        self.pos += len;
        Ok(slice.to_vec())
    }

    fn read_text(&mut self) -> Result<String, CertificateError> {
        let (major, len) = self.read_major()?;
        if major != 3 {
            return Err(CertificateError::InvalidCbor(
                "expected text string major type",
            ));
        }
        let len = usize::try_from(len)
            .map_err(|_| CertificateError::InvalidCbor("text string length exceeds usize"))?;
        if self.pos + len > self.data.len() {
            return Err(CertificateError::InvalidCbor("text string truncated"));
        }
        let slice = &self.data[self.pos..self.pos + len];
        self.pos += len;
        let text = core::str::from_utf8(slice)
            .map_err(|_| CertificateError::InvalidCbor("invalid UTF-8 in text string"))?;
        Ok(text.to_string())
    }

    fn read_array_len(&mut self) -> Result<u64, CertificateError> {
        let (major, len) = self.read_major()?;
        if major != 4 {
            return Err(CertificateError::InvalidCbor("expected array major type"));
        }
        Ok(len)
    }

    fn read_map_len(&mut self) -> Result<u64, CertificateError> {
        let (major, len) = self.read_major()?;
        if major != 5 {
            return Err(CertificateError::InvalidCbor("expected map major type"));
        }
        Ok(len)
    }

    fn read_array32(&mut self) -> Result<[u8; 32], CertificateError> {
        let bytes = self.read_bytes()?;
        let len = bytes.len();
        bytes
            .try_into()
            .map_err(|_| CertificateError::InvalidFieldValue {
                field: "array32",
                reason: format!("expected 32 bytes, got {len}"),
            })
    }

    fn peek_is_null(&self) -> Result<bool, CertificateError> {
        let byte = self
            .data
            .get(self.pos)
            .ok_or(CertificateError::InvalidCbor("unexpected end of input"))?;
        Ok(*byte == 0xf6)
    }

    fn read_null(&mut self) -> Result<(), CertificateError> {
        if !self.peek_is_null()? {
            return Err(CertificateError::InvalidCbor("expected null"));
        }
        self.pos += 1;
        Ok(())
    }

    fn read_major(&mut self) -> Result<(u8, u64), CertificateError> {
        let byte = self.read_u8()?;
        let major = byte >> 5;
        let additional = byte & 0x1f;
        let value = match additional {
            v @ 0..=23 => u64::from(v),
            24 => {
                let value = self.read_u8()?;
                if value < 24 {
                    return Err(CertificateError::InvalidCbor(
                        "non-shortest CBOR integer or length encoding",
                    ));
                }
                u64::from(value)
            }
            25 => {
                let mut buf = [0u8; 2];
                let slice = self.read_exact(2)?;
                buf.copy_from_slice(slice);
                let value = u64::from(u16::from_be_bytes(buf));
                if value <= 0xFF {
                    return Err(CertificateError::InvalidCbor(
                        "non-shortest CBOR integer or length encoding",
                    ));
                }
                value
            }
            26 => {
                let mut buf = [0u8; 4];
                let slice = self.read_exact(4)?;
                buf.copy_from_slice(slice);
                let value = u64::from(u32::from_be_bytes(buf));
                if value <= 0xFFFF {
                    return Err(CertificateError::InvalidCbor(
                        "non-shortest CBOR integer or length encoding",
                    ));
                }
                value
            }
            27 => {
                let mut buf = [0u8; 8];
                let slice = self.read_exact(8)?;
                buf.copy_from_slice(slice);
                let value = u64::from_be_bytes(buf);
                if value <= 0xFFFF_FFFF {
                    return Err(CertificateError::InvalidCbor(
                        "non-shortest CBOR integer or length encoding",
                    ));
                }
                value
            }
            _ => return Err(CertificateError::InvalidCbor("unsupported additional info")),
        };
        Ok((major, value))
    }

    fn read_exact(&mut self, len: usize) -> Result<&[u8], CertificateError> {
        if self.pos + len > self.data.len() {
            return Err(CertificateError::InvalidCbor("truncated CBOR payload"));
        }
        let slice = &self.data[self.pos..self.pos + len];
        self.pos += len;
        Ok(slice)
    }

    fn ensure_finished(&self) -> Result<(), CertificateError> {
        if self.pos != self.data.len() {
            return Err(CertificateError::InvalidCbor("trailing CBOR data"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{SECRET_KEY_LENGTH, SigningKey};
    use rand::{RngCore, SeedableRng, rngs::StdRng};
    use soranet_pq::{MlDsaSuite, MlKemSuite, generate_mldsa_keypair_from_os};

    use super::*;

    const ED25519_SMALL_ORDER_POINT: [u8; 32] = [
        1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ];

    fn sample_certificate() -> RelayCertificateV2 {
        RelayCertificateV2 {
            relay_id: [0x11; 32],
            identity_ed25519: SigningKey::from_bytes(&[0x22; SECRET_KEY_LENGTH])
                .verifying_key()
                .to_bytes(),
            identity_mldsa65: vec![0x33; MlDsaSuite::MlDsa65.public_key_len()],
            descriptor_commit: [0x44; 32],
            roles: RelayRolesV2 {
                entry: true,
                middle: true,
                exit: false,
            },
            guard_weight: 250,
            bandwidth_bytes_per_sec: 1_000_000,
            reputation_weight: 80,
            endpoints: vec![RelayEndpointV2 {
                url: "soranet://relay.example:443".to_string(),
                priority: 1,
                tags: vec!["norito-stream".into()],
            }],
            capability_flags: RelayCapabilityFlagsV1::new(
                CapabilityToggle::Enabled,
                CapabilityToggle::Enabled,
                CapabilityToggle::Enabled,
                CapabilityToggle::Disabled,
            ),
            kem_policy: KemRotationPolicyV1 {
                mode: KemRotationModeV1::Static,
                preferred_suite: 1,
                fallback_suite: None,
                rotation_interval_hours: 0,
                grace_period_hours: 0,
            },
            handshake_suites: vec![
                HandshakeSuite::Nk3PqForwardSecure,
                HandshakeSuite::Nk2Hybrid,
            ],
            published_at: 1_734_000_000,
            valid_after: 1_734_000_000,
            valid_until: 1_734_086_400,
            directory_hash: [0x55; 32],
            issuer_fingerprint: [0x66; 32],
            pq_kem_public: vec![0x77; MlKemSuite::MlKem768.public_key_len()],
        }
    }

    #[test]
    fn relay_capability_flags_roundtrip() {
        let flags = RelayCapabilityFlagsV1::new(
            CapabilityToggle::Enabled,
            CapabilityToggle::Disabled,
            CapabilityToggle::Enabled,
            CapabilityToggle::Disabled,
        );
        assert!(flags.supports_blinded_cid());
        assert!(!flags.requires_pow_ticket());
        assert!(flags.supports_norito_stream());
        assert!(!flags.supports_kaigi_bridge());
        assert_eq!(flags, RelayCapabilityFlagsV1::from_bits(flags.to_bits()));
    }

    #[test]
    fn capacity_from_len_handles_max_value() {
        let len = usize::MAX as u64;
        assert_eq!(capacity_from_len(len, "test").unwrap(), usize::MAX);
    }

    #[test]
    fn capacity_from_len_rejects_overflow_on_32_bit() {
        if usize::BITS < 64 {
            assert!(capacity_from_len(u64::MAX, "test").is_err());
        }
    }

    #[test]
    fn encode_decode_roundtrip() {
        let certificate = sample_certificate();
        let bytes = certificate.to_cbor();
        let decoded = parse_certificate_payload(&bytes).expect("decode");
        assert_eq!(certificate, decoded);
    }

    #[test]
    fn parse_certificate_payload_rejects_wrong_version() {
        let mut encoder = CborEncoder::new();
        encoder.write_map_header(1);
        encoder.write_unsigned(u64::from(Field::Version as u8));
        encoder.write_unsigned(u64::from(SRC_CERTIFICATE_VERSION) + 1);
        let bytes = encoder.finish();

        let err = parse_certificate_payload(&bytes).expect_err("version mismatch should fail");
        match err {
            CertificateError::InvalidFieldValue { field, .. } => {
                assert_eq!(field, "certificate.version");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn parse_certificate_payload_reports_missing_fields() {
        let mut encoder = CborEncoder::new();
        encoder.write_map_header(1);
        encoder.write_unsigned(u64::from(Field::Version as u8));
        encoder.write_unsigned(u64::from(SRC_CERTIFICATE_VERSION));
        let bytes = encoder.finish();

        let err = parse_certificate_payload(&bytes).expect_err("missing fields must be reported");
        match err {
            CertificateError::MissingField { field } => {
                assert_eq!(field, "certificate.relay_id");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn parse_certificate_payload_rejects_unknown_field() {
        let mut encoder = CborEncoder::new();
        encoder.write_map_header(2);
        encoder.write_unsigned(u64::from(Field::Version as u8));
        encoder.write_unsigned(u64::from(SRC_CERTIFICATE_VERSION));
        encoder.write_unsigned(255);
        encoder.write_unsigned(0);
        let bytes = encoder.finish();

        let err = parse_certificate_payload(&bytes).expect_err("unknown field should fail");
        match err {
            CertificateError::UnknownField { field } => {
                assert_eq!(field, "certificate.255");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn parse_certificate_payload_rejects_unknown_role_and_capability_bits() {
        let mut encoder = CborEncoder::new();
        encoder.write_map_header(2);
        encoder.write_unsigned(u64::from(Field::Version as u8));
        encoder.write_unsigned(u64::from(SRC_CERTIFICATE_VERSION));
        encoder.write_unsigned(u64::from(Field::Roles as u8));
        encoder.write_unsigned(0x80);
        let bytes = encoder.finish();
        let err = parse_certificate_payload(&bytes).expect_err("unknown role bits must fail");
        match err {
            CertificateError::InvalidFieldValue { field, .. } => {
                assert_eq!(field, "certificate.roles");
            }
            other => panic!("unexpected error: {other:?}"),
        }

        let mut encoder = CborEncoder::new();
        encoder.write_map_header(2);
        encoder.write_unsigned(u64::from(Field::Version as u8));
        encoder.write_unsigned(u64::from(SRC_CERTIFICATE_VERSION));
        encoder.write_unsigned(u64::from(Field::CapabilityFlags as u8));
        encoder.write_unsigned(0x8000);
        let bytes = encoder.finish();
        let err = parse_certificate_payload(&bytes).expect_err("unknown capability bits must fail");
        match err {
            CertificateError::InvalidFieldValue { field, .. } => {
                assert_eq!(field, "certificate.capability_flags");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn nested_decoders_reject_duplicate_fields() {
        let mut encoder = CborEncoder::new();
        encoder.write_map_header(2);
        encoder.write_unsigned(KemField::Mode as u64);
        encoder.write_unsigned(KemRotationModeV1::Static as u64);
        encoder.write_unsigned(KemField::Mode as u64);
        encoder.write_unsigned(KemRotationModeV1::Rolling as u64);
        let bytes = encoder.finish();
        let mut decoder = CborDecoder::new(&bytes);
        let err = KemRotationPolicyV1::decode(&mut decoder)
            .expect_err("duplicate KEM policy fields must fail");
        match err {
            CertificateError::DuplicateField { field } => {
                assert_eq!(field, "kem_policy.mode");
            }
            other => panic!("unexpected error: {other:?}"),
        }

        let mut encoder = CborEncoder::new();
        encoder.write_map_header(2);
        encoder.write_unsigned(EndpointField::Url as u64);
        encoder.write_text("soranet://first.example");
        encoder.write_unsigned(EndpointField::Url as u64);
        encoder.write_text("soranet://second.example");
        let bytes = encoder.finish();
        let mut decoder = CborDecoder::new(&bytes);
        let err =
            RelayEndpointV2::decode(&mut decoder).expect_err("duplicate endpoint fields must fail");
        match err {
            CertificateError::DuplicateField { field } => {
                assert_eq!(field, "endpoint.url");
            }
            other => panic!("unexpected error: {other:?}"),
        }

        let mut encoder = CborEncoder::new();
        encoder.write_map_header(2);
        encoder.write_unsigned(SignatureField::Ed25519 as u64);
        encoder.write_bytes(&[0; 64]);
        encoder.write_unsigned(SignatureField::Ed25519 as u64);
        encoder.write_bytes(&[1; 64]);
        let bytes = encoder.finish();
        let mut decoder = CborDecoder::new(&bytes);
        let err = RelayCertificateSignaturesV2::decode(&mut decoder)
            .expect_err("duplicate signature fields must fail");
        match err {
            CertificateError::DuplicateField { field } => {
                assert_eq!(field, "signatures.ed25519");
            }
            other => panic!("unexpected error: {other:?}"),
        }

        let certificate = sample_certificate().to_cbor();
        let mut encoder = CborEncoder::new();
        encoder.write_map_header(2);
        encoder.write_unsigned(0);
        encoder.write_bytes(&certificate);
        encoder.write_unsigned(0);
        encoder.write_bytes(&certificate);
        let bytes = encoder.finish();
        let err = RelayCertificateBundleV2::from_cbor(&bytes)
            .expect_err("duplicate bundle fields must fail");
        match err {
            CertificateError::DuplicateField { field } => {
                assert_eq!(field, "bundle.certificate");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn parse_certificate_payload_rejects_trailing_bytes() {
        let mut bytes = sample_certificate().to_cbor();
        bytes.push(0x00);

        let err = parse_certificate_payload(&bytes).expect_err("trailing payload bytes must fail");
        match err {
            CertificateError::InvalidCbor(reason) => {
                assert_eq!(reason, "trailing CBOR data");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn bundle_from_cbor_rejects_trailing_bytes() {
        let bundle = RelayCertificateBundleV2 {
            certificate: sample_certificate(),
            signatures: RelayCertificateSignaturesV2 {
                ed25519: [0; 64],
                mldsa65: None,
            },
        };
        let mut bytes = bundle.to_cbor();
        bytes.push(0x00);

        let err = RelayCertificateBundleV2::from_cbor(&bytes)
            .expect_err("trailing bundle bytes must fail");
        match err {
            CertificateError::InvalidCbor(reason) => {
                assert_eq!(reason, "trailing CBOR data");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn parse_certificate_payload_rejects_non_shortest_cbor_encodings() {
        let canonical = sample_certificate().to_cbor();
        assert_eq!(canonical[0], 0xB3, "sample certificate uses a 19-field map");
        let mut non_shortest_map_len = vec![0xB8, 0x13];
        non_shortest_map_len.extend_from_slice(&canonical[1..]);
        let err = parse_certificate_payload(&non_shortest_map_len)
            .expect_err("non-shortest map length must fail");
        match err {
            CertificateError::InvalidCbor(reason) => {
                assert_eq!(reason, "non-shortest CBOR integer or length encoding");
            }
            other => panic!("unexpected error: {other:?}"),
        }

        assert_eq!(canonical[1], 0x00, "first field key is version");
        let mut non_shortest_field_key = Vec::with_capacity(canonical.len() + 1);
        non_shortest_field_key.push(canonical[0]);
        non_shortest_field_key.extend_from_slice(&[0x18, 0x00]);
        non_shortest_field_key.extend_from_slice(&canonical[2..]);
        let err = parse_certificate_payload(&non_shortest_field_key)
            .expect_err("non-shortest field key must fail");
        match err {
            CertificateError::InvalidCbor(reason) => {
                assert_eq!(reason, "non-shortest CBOR integer or length encoding");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn parse_certificate_payload_rejects_unknown_handshake_suite() {
        let mut encoder = CborEncoder::new();
        encoder.write_map_header(2);
        encoder.write_unsigned(u64::from(Field::Version as u8));
        encoder.write_unsigned(u64::from(SRC_CERTIFICATE_VERSION));
        encoder.write_unsigned(u64::from(Field::HandshakeSuites as u8));
        encoder.write_array_header(1);
        encoder.write_unsigned(0xFF);
        let bytes = encoder.finish();

        let err =
            parse_certificate_payload(&bytes).expect_err("invalid handshake suite should fail");
        match err {
            CertificateError::InvalidFieldValue { field, .. } => {
                assert_eq!(field, "certificate.handshake_suites");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn parse_certificate_payload_rejects_empty_or_duplicate_handshake_suites() {
        let mut encoder = CborEncoder::new();
        encoder.write_map_header(2);
        encoder.write_unsigned(u64::from(Field::Version as u8));
        encoder.write_unsigned(u64::from(SRC_CERTIFICATE_VERSION));
        encoder.write_unsigned(u64::from(Field::HandshakeSuites as u8));
        encoder.write_array_header(0);
        let bytes = encoder.finish();
        let err = parse_certificate_payload(&bytes).expect_err("empty suite list must fail");
        match err {
            CertificateError::InvalidFieldValue { field, .. } => {
                assert_eq!(field, "certificate.handshake_suites");
            }
            other => panic!("unexpected error: {other:?}"),
        }

        let mut encoder = CborEncoder::new();
        encoder.write_map_header(2);
        encoder.write_unsigned(u64::from(Field::Version as u8));
        encoder.write_unsigned(u64::from(SRC_CERTIFICATE_VERSION));
        encoder.write_unsigned(u64::from(Field::HandshakeSuites as u8));
        encoder.write_array_header(2);
        encoder.write_unsigned(HandshakeSuite::Nk2Hybrid as u64);
        encoder.write_unsigned(HandshakeSuite::Nk2Hybrid as u64);
        let bytes = encoder.finish();
        let err = parse_certificate_payload(&bytes).expect_err("duplicate suite list must fail");
        match err {
            CertificateError::InvalidFieldValue { field, .. } => {
                assert_eq!(field, "certificate.handshake_suites");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn parse_certificate_payload_rejects_empty_or_duplicate_endpoints() {
        let mut certificate = sample_certificate();
        certificate.endpoints.clear();
        let err = parse_certificate_payload(&certificate.to_cbor())
            .expect_err("empty endpoint lists must fail");
        match err {
            CertificateError::InvalidFieldValue { field, .. } => {
                assert_eq!(field, "certificate.endpoints");
            }
            other => panic!("unexpected error: {other:?}"),
        }

        let mut certificate = sample_certificate();
        certificate.endpoints.push(certificate.endpoints[0].clone());
        let err = parse_certificate_payload(&certificate.to_cbor())
            .expect_err("duplicate endpoint URLs must fail");
        match err {
            CertificateError::InvalidFieldValue { field, .. } => {
                assert_eq!(field, "certificate.endpoints");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn parse_certificate_payload_rejects_invalid_endpoint_urls() {
        for url in [
            "",
            " soranet://relay.example:443",
            "soranet://relay.example:443 ",
            "soranet://relay.example:\n443",
        ] {
            let mut certificate = sample_certificate();
            certificate.endpoints[0].url = url.to_owned();
            let err = parse_certificate_payload(&certificate.to_cbor())
                .expect_err("ambiguous endpoint URLs must fail");
            match err {
                CertificateError::InvalidFieldValue { field, .. } => {
                    assert_eq!(field, "endpoint.url");
                }
                other => panic!("unexpected error: {other:?}"),
            }
        }
    }

    #[test]
    fn parse_certificate_payload_rejects_invalid_endpoint_tags() {
        for tags in [
            vec!["".to_string()],
            vec!["nk 3".to_string()],
            vec!["nk\n3".to_string()],
            vec!["nk3".to_string(), "nk3".to_string()],
        ] {
            let mut certificate = sample_certificate();
            certificate.endpoints[0].tags = tags;
            let err = parse_certificate_payload(&certificate.to_cbor())
                .expect_err("ambiguous endpoint tags must fail");
            match err {
                CertificateError::InvalidFieldValue { field, .. } => {
                    assert_eq!(field, "endpoint.tags");
                }
                other => panic!("unexpected error: {other:?}"),
            }
        }
    }

    #[test]
    fn parse_certificate_payload_rejects_unknown_kem_suites() {
        let mut certificate = sample_certificate();
        certificate.kem_policy.preferred_suite = 0xFF;
        let err = parse_certificate_payload(&certificate.to_cbor()).expect_err("unknown KEM suite");
        match err {
            CertificateError::InvalidFieldValue { field, .. } => {
                assert_eq!(field, "kem_policy.preferred_suite");
            }
            other => panic!("unexpected error: {other:?}"),
        }

        let mut certificate = sample_certificate();
        certificate.kem_policy.fallback_suite = Some(0xFE);
        let err =
            parse_certificate_payload(&certificate.to_cbor()).expect_err("unknown fallback suite");
        match err {
            CertificateError::InvalidFieldValue { field, .. } => {
                assert_eq!(field, "kem_policy.fallback_suite");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn parse_certificate_payload_rejects_inconsistent_kem_policies() {
        let mut certificate = sample_certificate();
        certificate.kem_policy.fallback_suite = Some(MlKemSuite::MlKem512.kem_id());
        assert_kem_policy_error(certificate, "kem_policy.fallback_suite");

        let mut certificate = sample_certificate();
        certificate.kem_policy.rotation_interval_hours = 1;
        assert_kem_policy_error(certificate, "kem_policy.rotation_interval_hours");

        let mut certificate = sample_certificate();
        certificate.kem_policy.grace_period_hours = 1;
        assert_kem_policy_error(certificate, "kem_policy.grace_period_hours");

        let mut certificate = sample_certificate();
        certificate.kem_policy.mode = KemRotationModeV1::Staged;
        assert_kem_policy_error(certificate, "kem_policy.fallback_suite");

        let mut certificate = sample_certificate();
        certificate.kem_policy.mode = KemRotationModeV1::Staged;
        certificate.kem_policy.fallback_suite = Some(certificate.kem_policy.preferred_suite);
        assert_kem_policy_error(certificate, "kem_policy.fallback_suite");

        let mut certificate = sample_certificate();
        certificate.kem_policy.mode = KemRotationModeV1::Rolling;
        assert_kem_policy_error(certificate, "kem_policy.rotation_interval_hours");
    }

    fn assert_kem_policy_error(certificate: RelayCertificateV2, expected_field: &'static str) {
        let err = parse_certificate_payload(&certificate.to_cbor())
            .expect_err("inconsistent KEM policy must fail");
        match err {
            CertificateError::InvalidFieldValue { field, .. } => {
                assert_eq!(field, expected_field);
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn parse_certificate_payload_rejects_invalid_key_lengths() {
        let mut certificate = sample_certificate();
        certificate.identity_mldsa65.pop();
        let err = parse_certificate_payload(&certificate.to_cbor())
            .expect_err("invalid ML-DSA key length");
        match err {
            CertificateError::InvalidFieldValue { field, .. } => {
                assert_eq!(field, "certificate.identity_mldsa65");
            }
            other => panic!("unexpected error: {other:?}"),
        }

        let mut certificate = sample_certificate();
        certificate.pq_kem_public.push(0x88);
        let err = parse_certificate_payload(&certificate.to_cbor())
            .expect_err("invalid ML-KEM key length");
        match err {
            CertificateError::InvalidFieldValue { field, .. } => {
                assert_eq!(field, "certificate.pq_kem_public");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn bundle_decode_rejects_invalid_mldsa65_signature_length() {
        let bundle = RelayCertificateBundleV2 {
            certificate: sample_certificate(),
            signatures: RelayCertificateSignaturesV2 {
                ed25519: [0xAA; 64],
                mldsa65: Some(vec![0xBB; MlDsaSuite::MlDsa65.signature_len() + 1]),
            },
        };

        let err = RelayCertificateBundleV2::from_cbor(&bundle.to_cbor())
            .expect_err("invalid ML-DSA signature length should fail");
        match err {
            CertificateError::InvalidFieldValue { field, .. } => {
                assert_eq!(field, "signatures.mldsa65");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn parse_certificate_payload_rejects_weak_ed25519_identity_key() {
        let mut certificate = sample_certificate();
        certificate.identity_ed25519 = ED25519_SMALL_ORDER_POINT;
        let err = parse_certificate_payload(&certificate.to_cbor())
            .expect_err("weak Ed25519 identity keys must fail");
        match err {
            CertificateError::InvalidFieldValue { field, .. } => {
                assert_eq!(field, "certificate.identity_ed25519");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn parse_certificate_payload_rejects_invalid_validity_windows() {
        let mut certificate = sample_certificate();
        certificate.valid_until = certificate.valid_after;
        let err = parse_certificate_payload(&certificate.to_cbor())
            .expect_err("zero-length validity windows must fail");
        match err {
            CertificateError::InvalidFieldValue { field, .. } => {
                assert_eq!(field, "certificate.valid_until");
            }
            other => panic!("unexpected error: {other:?}"),
        }

        let mut certificate = sample_certificate();
        certificate.valid_until = certificate.valid_after.saturating_sub(1);
        let err = parse_certificate_payload(&certificate.to_cbor())
            .expect_err("inverted validity windows must fail");
        match err {
            CertificateError::InvalidFieldValue { field, .. } => {
                assert_eq!(field, "certificate.valid_until");
            }
            other => panic!("unexpected error: {other:?}"),
        }

        let mut certificate = sample_certificate();
        certificate.published_at = certificate.valid_until.saturating_add(1);
        let err = parse_certificate_payload(&certificate.to_cbor())
            .expect_err("certificates published after expiry must fail");
        match err {
            CertificateError::InvalidFieldValue { field, .. } => {
                assert_eq!(field, "certificate.published_at");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn parse_certificate_payload_rejects_duplicate_field() {
        let mut encoder = CborEncoder::new();
        encoder.write_map_header(2);
        encoder.write_unsigned(u64::from(Field::Version as u8));
        encoder.write_unsigned(u64::from(SRC_CERTIFICATE_VERSION));
        encoder.write_unsigned(u64::from(Field::Version as u8));
        encoder.write_unsigned(u64::from(SRC_CERTIFICATE_VERSION));
        let bytes = encoder.finish();

        let err = parse_certificate_payload(&bytes).expect_err("duplicate field should fail");
        match err {
            CertificateError::DuplicateField { field } => {
                assert_eq!(field, "certificate.version");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn bundle_sign_verify() {
        let certificate = sample_certificate();

        let mut rng = StdRng::seed_from_u64(42);
        let mut seed = [0u8; SECRET_KEY_LENGTH];
        rng.fill_bytes(&mut seed);
        let signing_key = SigningKey::from_bytes(&seed);
        let verifying_key = VerifyingKey::from(&signing_key);

        let mldsa_keys = generate_mldsa_keypair_from_os(MlDsaSuite::MlDsa65)
            .expect("ML-DSA keypair generation should succeed");

        let bundle = certificate
            .issue(&signing_key, mldsa_keys.secret_key())
            .expect("issue certificate");

        bundle
            .verify(
                &verifying_key,
                mldsa_keys.public_key(),
                CertificateValidationPhase::Phase3RequireDual,
            )
            .expect("verify");

        // Ensure digest is stable.
        let digest_a = bundle.certificate.digest();
        let digest_b = bundle.certificate.digest();
        assert_eq!(digest_a, digest_b);
    }

    #[test]
    fn issue_rejects_invalid_certificate_payload_before_signing() {
        let mut certificate = sample_certificate();
        certificate.endpoints.clear();

        let signing_key = SigningKey::from_bytes(&[0x88; SECRET_KEY_LENGTH]);
        let mldsa_keys = generate_mldsa_keypair_from_os(MlDsaSuite::MlDsa65)
            .expect("ML-DSA keypair generation should succeed");

        let err = certificate
            .issue(&signing_key, mldsa_keys.secret_key())
            .expect_err("invalid certificates must not be signed into bundles");
        match err {
            CertificateError::InvalidFieldValue { field, .. } => {
                assert_eq!(field, "certificate.endpoints");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn issue_rejects_invalid_mldsa_secret_key_length() {
        let certificate = sample_certificate();
        let signing_key = SigningKey::from_bytes(&[0x89; SECRET_KEY_LENGTH]);
        let mldsa_keys = generate_mldsa_keypair_from_os(MlDsaSuite::MlDsa65)
            .expect("ML-DSA keypair generation should succeed");
        let mut short_secret_key = mldsa_keys.secret_key().to_vec();
        short_secret_key.pop();

        let err = certificate
            .issue(&signing_key, &short_secret_key)
            .expect_err("invalid ML-DSA secret keys must fail before backend signing");
        match err {
            CertificateError::SignatureFailure(message) => {
                assert!(
                    message.contains("secret key"),
                    "unexpected error: {message}"
                );
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn verification_fails_without_mldsa_in_phase3() {
        let certificate = sample_certificate();

        let mut rng = StdRng::seed_from_u64(4242);
        let mut seed = [0u8; SECRET_KEY_LENGTH];
        rng.fill_bytes(&mut seed);
        let signing_key = SigningKey::from_bytes(&seed);
        let verifying_key = VerifyingKey::from(&signing_key);

        let mldsa_keys = generate_mldsa_keypair_from_os(MlDsaSuite::MlDsa65)
            .expect("ML-DSA keypair generation should succeed");

        let mut bundle = certificate
            .issue(&signing_key, mldsa_keys.secret_key())
            .expect("issue");

        bundle.signatures.mldsa65 = None;

        let err = bundle
            .verify(
                &verifying_key,
                mldsa_keys.public_key(),
                CertificateValidationPhase::Phase3RequireDual,
            )
            .expect_err("phase 3 requires ML-DSA signature");

        matches!(err, CertificateError::MissingMldsaSignature { .. });

        // Phase 1 and Phase 2 should allow it during the staged rollout.
        bundle
            .verify(
                &verifying_key,
                mldsa_keys.public_key(),
                CertificateValidationPhase::Phase1AllowSingle,
            )
            .expect("phase 1 accepts single signature");
        bundle
            .verify(
                &verifying_key,
                mldsa_keys.public_key(),
                CertificateValidationPhase::Phase2PreferDual,
            )
            .expect("phase 2 accepts single signature");
    }

    #[test]
    fn verification_rejects_weak_ed25519_public_key() {
        let certificate = sample_certificate();

        let mut rng = StdRng::seed_from_u64(99);
        let mut seed = [0u8; SECRET_KEY_LENGTH];
        rng.fill_bytes(&mut seed);
        let signing_key = SigningKey::from_bytes(&seed);

        let mldsa_keys = generate_mldsa_keypair_from_os(MlDsaSuite::MlDsa65)
            .expect("ML-DSA keypair generation should succeed");

        let bundle = certificate
            .issue(&signing_key, mldsa_keys.secret_key())
            .expect("issue");
        let weak_key = VerifyingKey::from_bytes(&ED25519_SMALL_ORDER_POINT)
            .expect("small-order point should parse as a dalek verifying key");

        let err = bundle
            .verify(
                &weak_key,
                mldsa_keys.public_key(),
                CertificateValidationPhase::Phase3RequireDual,
            )
            .expect_err("weak Ed25519 verifier key must fail before signature math");
        match err {
            CertificateError::SignatureFailure(message) => {
                assert!(
                    message.contains("small-order"),
                    "unexpected error: {message}"
                );
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn verification_rejects_invalid_mldsa_material_lengths() {
        let certificate = sample_certificate();

        let mut rng = StdRng::seed_from_u64(7);
        let mut seed = [0u8; SECRET_KEY_LENGTH];
        rng.fill_bytes(&mut seed);
        let signing_key = SigningKey::from_bytes(&seed);
        let verifying_key = VerifyingKey::from(&signing_key);

        let mldsa_keys = generate_mldsa_keypair_from_os(MlDsaSuite::MlDsa65)
            .expect("ML-DSA keypair generation should succeed");

        let bundle = certificate
            .issue(&signing_key, mldsa_keys.secret_key())
            .expect("issue");

        let mut short_public_key = mldsa_keys.public_key().to_vec();
        short_public_key.pop();
        let err = bundle
            .verify(
                &verifying_key,
                &short_public_key,
                CertificateValidationPhase::Phase3RequireDual,
            )
            .expect_err("invalid ML-DSA public key length must fail before backend verify");
        match err {
            CertificateError::SignatureFailure(message) => {
                assert!(
                    message.contains("public key"),
                    "unexpected error: {message}"
                );
            }
            other => panic!("unexpected error: {other:?}"),
        }

        let mut bundle = bundle;
        bundle.signatures.mldsa65 = Some(vec![0x99; MlDsaSuite::MlDsa65.signature_len() - 1]);
        let err = bundle
            .verify(
                &verifying_key,
                mldsa_keys.public_key(),
                CertificateValidationPhase::Phase3RequireDual,
            )
            .expect_err("invalid ML-DSA signature length must fail before backend verify");
        match err {
            CertificateError::SignatureFailure(message) => {
                assert!(message.contains("signature"), "unexpected error: {message}");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
