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
use soranet_pq::{MlDsaSuite, sign_mldsa, verify_mldsa};
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
                    mode = Some(KemRotationModeV1::from_raw(raw)?);
                }
                x if x == KemField::PreferredSuite as u8 => {
                    let raw = decoder.read_unsigned()?;
                    let suite: u8 =
                        raw.try_into()
                            .map_err(|_| CertificateError::InvalidFieldValue {
                                field: "kem_policy.preferred_suite",
                                reason: format!("value {raw} exceeds u8::MAX"),
                            })?;
                    preferred_suite = Some(suite);
                }
                x if x == KemField::FallbackSuite as u8 => {
                    fallback_suite = if decoder.peek_is_null()? {
                        decoder.read_null()?;
                        Some(None)
                    } else {
                        let raw = decoder.read_unsigned()?;
                        let suite: u8 =
                            raw.try_into()
                                .map_err(|_| CertificateError::InvalidFieldValue {
                                    field: "kem_policy.fallback_suite",
                                    reason: format!("value {raw} exceeds u8::MAX"),
                                })?;
                        Some(Some(suite))
                    };
                }
                x if x == KemField::RotationIntervalHours as u8 => {
                    rotation_interval_hours = Some(decoder.read_u16()?);
                }
                x if x == KemField::GracePeriodHours as u8 => {
                    grace_period_hours = Some(decoder.read_u16()?);
                }
                other => {
                    return Err(CertificateError::UnknownField {
                        field: format!("kem_policy.{other}"),
                    });
                }
            }
        }

        Ok(Self {
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
        })
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
                    url = Some(decoder.read_text()?);
                }
                x if x == EndpointField::Priority as u8 => {
                    let raw = decoder.read_unsigned()?;
                    let value: u8 =
                        raw.try_into()
                            .map_err(|_| CertificateError::InvalidFieldValue {
                                field: "endpoint.priority",
                                reason: format!("value {raw} exceeds u8::MAX"),
                            })?;
                    priority = Some(value);
                }
                x if x == EndpointField::Tags as u8 => {
                    let len = decoder.read_array_len()?;
                    let capacity = capacity_from_len(len, "endpoint.tags")?;
                    let mut values = Vec::with_capacity(capacity);
                    for _ in 0..len {
                        values.push(decoder.read_text()?);
                    }
                    tags = Some(values);
                }
                other => {
                    return Err(CertificateError::UnknownField {
                        field: format!("endpoint.{other}"),
                    });
                }
            }
        }

        Ok(Self {
            url: url.ok_or(CertificateError::MissingField {
                field: "endpoint.url",
            })?,
            priority: priority.ok_or(CertificateError::MissingField {
                field: "endpoint.priority",
            })?,
            tags: tags.unwrap_or_default(),
        })
    }
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
        let digest = compute_signing_digest(SRC_V2_ED25519_DOMAIN, &payload);

        let ed25519_signature: Signature = ed25519_signing_key.sign(&digest);

        let mldsa_digest = compute_signing_digest(SRC_V2_MLDSA_DOMAIN, &payload);
        let mldsa_signature = sign_mldsa(MlDsaSuite::MlDsa65, mldsa_secret_key, &mldsa_digest)
            .map_err(|err| {
                CertificateError::SignatureFailure(format!("ML-DSA signing failed: {err}"))
            })?;

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
                    ed25519 = Some(array);
                }
                x if x == SignatureField::MlDsa65 as u8 => {
                    if decoder.peek_is_null()? {
                        decoder.read_null()?;
                        mldsa = Some(None);
                    } else {
                        mldsa = Some(Some(decoder.read_bytes()?));
                    }
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
                    certificate = Some(parse_certificate_payload(&payload_bytes)?);
                }
                1 => {
                    signatures = Some(RelayCertificateSignaturesV2::decode(&mut decoder)?);
                }
                other => {
                    return Err(CertificateError::UnknownField {
                        field: format!("bundle.{other}"),
                    });
                }
            }
        }

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
        ed25519_public
            .verify_strict(&ed_digest, &Signature::from_bytes(&self.signatures.ed25519))
            .map_err(|err| {
                CertificateError::SignatureFailure(format!("Ed25519 verify failed: {err}"))
            })?;

        match (&self.signatures.mldsa65, phase) {
            (Some(bytes), _) => {
                let digest = compute_signing_digest(SRC_V2_MLDSA_DOMAIN, &payload);
                verify_mldsa(MlDsaSuite::MlDsa65, mldsa_public, &digest, bytes).map_err(|err| {
                    CertificateError::SignatureFailure(format!("ML-DSA verify failed: {err}"))
                })?;
            }
            (None, CertificateValidationPhase::Phase1AllowSingle) => {}
            (None, CertificateValidationPhase::Phase2PreferDual) => {
                return Err(CertificateError::MissingMldsaSignature {
                    phase: "Phase2PreferDual",
                });
            }
            (None, CertificateValidationPhase::Phase3RequireDual) => {
                return Err(CertificateError::MissingMldsaSignature {
                    phase: "Phase3RequireDual",
                });
            }
        }

        Ok(())
    }
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
                    RelayCapabilityFlagsV1::from_bits(bits),
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
        Ok(RelayCertificateV2 {
            relay_id: require_field(self.relay_id, "certificate.relay_id")?,
            identity_ed25519: require_field(self.identity_ed25519, "certificate.identity_ed25519")?,
            identity_mldsa65: require_field(self.identity_mldsa65, "certificate.identity_mldsa65")?,
            descriptor_commit: require_field(
                self.descriptor_commit,
                "certificate.descriptor_commit",
            )?,
            roles: require_field(self.roles, "certificate.roles")?,
            guard_weight: require_field(self.guard_weight, "certificate.guard_weight")?,
            bandwidth_bytes_per_sec: require_field(
                self.bandwidth_bytes_per_sec,
                "certificate.bandwidth_bytes_per_sec",
            )?,
            reputation_weight: require_field(
                self.reputation_weight,
                "certificate.reputation_weight",
            )?,
            endpoints: require_field(self.endpoints, "certificate.endpoints")?,
            capability_flags: self.capability_flags.unwrap_or_default(),
            kem_policy: require_field(self.kem_policy, "certificate.kem_policy")?,
            handshake_suites: require_field(self.handshake_suites, "certificate.handshake_suites")?,
            published_at: require_field(self.published_at, "certificate.published_at")?,
            valid_after: require_field(self.valid_after, "certificate.valid_after")?,
            valid_until: require_field(self.valid_until, "certificate.valid_until")?,
            directory_hash: require_field(self.directory_hash, "certificate.directory_hash")?,
            issuer_fingerprint: require_field(
                self.issuer_fingerprint,
                "certificate.issuer_fingerprint",
            )?,
            pq_kem_public: require_field(self.pq_kem_public, "certificate.pq_kem_public")?,
        })
    }
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
    let capacity = capacity_from_len(len, "certificate.endpoints")?;
    let mut values = Vec::with_capacity(capacity);
    for _ in 0..len {
        values.push(RelayEndpointV2::decode(decoder)?);
    }
    Ok(values)
}

fn decode_handshake_suites(
    decoder: &mut CborDecoder<'_>,
) -> Result<Vec<HandshakeSuite>, CertificateError> {
    let len = decoder.read_array_len()?;
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

fn require_field<T>(value: Option<T>, field: &'static str) -> Result<T, CertificateError> {
    value.ok_or(CertificateError::MissingField { field })
}

fn parse_certificate_payload(bytes: &[u8]) -> Result<RelayCertificateV2, CertificateError> {
    let mut decoder = CborDecoder::new(bytes);
    let map_len = decoder.read_map_len()?;
    let fields = CertificateFieldAccumulator::default().decode(&mut decoder, map_len)?;
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
            24 => u64::from(self.read_u8()?),
            25 => {
                let mut buf = [0u8; 2];
                let slice = self.read_exact(2)?;
                buf.copy_from_slice(slice);
                u64::from(u16::from_be_bytes(buf))
            }
            26 => {
                let mut buf = [0u8; 4];
                let slice = self.read_exact(4)?;
                buf.copy_from_slice(slice);
                u64::from(u32::from_be_bytes(buf))
            }
            27 => {
                let mut buf = [0u8; 8];
                let slice = self.read_exact(8)?;
                buf.copy_from_slice(slice);
                u64::from_be_bytes(buf)
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
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{SECRET_KEY_LENGTH, SigningKey};
    use rand::{RngCore, SeedableRng, rngs::StdRng};
    use soranet_pq::{MlDsaSuite, generate_mldsa_keypair};

    use super::*;

    fn sample_certificate() -> RelayCertificateV2 {
        RelayCertificateV2 {
            relay_id: [0x11; 32],
            identity_ed25519: [0x22; 32],
            identity_mldsa65: vec![0x33; 1952],
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
            pq_kem_public: vec![0x77; 1184],
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

        let mldsa_keys = generate_mldsa_keypair(MlDsaSuite::MlDsa65)
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
    fn verification_fails_without_mldsa_in_phase3() {
        let certificate = sample_certificate();

        let mut rng = StdRng::seed_from_u64(4242);
        let mut seed = [0u8; SECRET_KEY_LENGTH];
        rng.fill_bytes(&mut seed);
        let signing_key = SigningKey::from_bytes(&seed);
        let verifying_key = VerifyingKey::from(&signing_key);

        let mldsa_keys = generate_mldsa_keypair(MlDsaSuite::MlDsa65)
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

        // Phase 1 should allow it.
        bundle
            .verify(
                &verifying_key,
                mldsa_keys.public_key(),
                CertificateValidationPhase::Phase1AllowSingle,
            )
            .expect("phase 1 accepts single signature");
    }
}
