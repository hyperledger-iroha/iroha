//! Canonical composite-proof transport for the replacement RNS-native MKHE profile.
//!
//! This envelope binds the replacement profile, topology, non-authorizing
//! release candidate, statement, operational context, and structural source
//! receipt before any proof section is allocated. It authenticates transport
//! structure only: a successful decode is not proof verification and carries
//! no release or readiness authority.

use super::{
    ZkAmsMkheErrorV1,
    rns_native_profile::{
        ZK_AMS_MKHE_RNS_NATIVE_PROOF_MAX_BYTES_V1, ZK_AMS_MKHE_RNS_NATIVE_QPCS_MAX_BYTES_V1,
        zk_ams_mkhe_rns_native_profile_manifest_v1, zk_ams_mkhe_rns_native_profile_v1,
        zk_ams_mkhe_rns_native_release_candidate_digest_v1,
    },
    rns_native_source::{ZkAmsMkheRnsNativeSourceLayoutV1, ZkAmsMkheRnsNativeSourceReceiptV1},
};
use crate::vega::sponge::Keccak256;

const RNS_NATIVE_PROOF_ENVELOPE_TAG_V1: [u8; 4] = *b"ZANP";
const IDENTITY_DIGEST_COUNT_V1: usize = 6;
const SECTION_DESCRIPTOR_BYTES_V1: usize = 1 + 4 + 4 + 32;
const SECTION_DESCRIPTORS_OFFSET_V1: usize = 4 + 1 + IDENTITY_DIGEST_COUNT_V1 * 32 + 1 + 4;
const WHOLE_PROOF_DIGEST_BYTES_V1: usize = 32;

/// Exact number of ordered sections in one replacement composite proof.
pub const ZK_AMS_MKHE_RNS_NATIVE_PROOF_SECTION_COUNT_V1: usize = 4;
/// Exact composite-proof envelope schema version.
pub const ZK_AMS_MKHE_RNS_NATIVE_PROOF_ENVELOPE_VERSION_V1: u8 = 1;
/// Maximum terminal Hyrax/Bulletproof bridge section bytes.
pub const ZK_AMS_MKHE_RNS_NATIVE_TERMINAL_BRIDGE_SECTION_MAX_BYTES_V1: u32 = 2 * 1024 * 1024;
/// Maximum two-equation RNS-relation/qPCS section bytes.
pub const ZK_AMS_MKHE_RNS_NATIVE_RNS_RELATION_QPCS_SECTION_MAX_BYTES_V1: u32 =
    ZK_AMS_MKHE_RNS_NATIVE_QPCS_MAX_BYTES_V1 as u32;
/// Maximum cross-field/global-lookup section bytes.
pub const ZK_AMS_MKHE_RNS_NATIVE_CROSS_FIELD_LOOKUP_SECTION_MAX_BYTES_V1: u32 = 8 * 1024 * 1024;
/// Maximum zero-padding proof section bytes.
pub const ZK_AMS_MKHE_RNS_NATIVE_ZERO_PADDING_SECTION_MAX_BYTES_V1: u32 = 512 * 1024;
/// Exact bytes before the four ordered section payloads.
pub const ZK_AMS_MKHE_RNS_NATIVE_PROOF_ENVELOPE_HEADER_BYTES_V1: usize =
    SECTION_DESCRIPTORS_OFFSET_V1
        + ZK_AMS_MKHE_RNS_NATIVE_PROOF_SECTION_COUNT_V1 * SECTION_DESCRIPTOR_BYTES_V1
        + WHOLE_PROOF_DIGEST_BYTES_V1;
/// Hard complete-envelope ceiling, including metadata and section payloads.
pub const ZK_AMS_MKHE_RNS_NATIVE_PROOF_ENVELOPE_MAX_BYTES_V1: u32 =
    ZK_AMS_MKHE_RNS_NATIVE_PROOF_MAX_BYTES_V1 as u32;

const _: () = {
    assert!(ZK_AMS_MKHE_RNS_NATIVE_PROOF_ENVELOPE_HEADER_BYTES_V1 == 398);
    assert!(
        ZK_AMS_MKHE_RNS_NATIVE_RNS_RELATION_QPCS_SECTION_MAX_BYTES_V1 as u64
            == ZK_AMS_MKHE_RNS_NATIVE_QPCS_MAX_BYTES_V1
    );
    assert!(
        ZK_AMS_MKHE_RNS_NATIVE_PROOF_ENVELOPE_MAX_BYTES_V1 as u64
            == ZK_AMS_MKHE_RNS_NATIVE_PROOF_MAX_BYTES_V1
    );
    assert!(
        ZK_AMS_MKHE_RNS_NATIVE_PROOF_ENVELOPE_HEADER_BYTES_V1 as u64
            + ZK_AMS_MKHE_RNS_NATIVE_TERMINAL_BRIDGE_SECTION_MAX_BYTES_V1 as u64
            + ZK_AMS_MKHE_RNS_NATIVE_RNS_RELATION_QPCS_SECTION_MAX_BYTES_V1 as u64
            + ZK_AMS_MKHE_RNS_NATIVE_CROSS_FIELD_LOOKUP_SECTION_MAX_BYTES_V1 as u64
            + ZK_AMS_MKHE_RNS_NATIVE_ZERO_PADDING_SECTION_MAX_BYTES_V1 as u64
            <= ZK_AMS_MKHE_RNS_NATIVE_PROOF_MAX_BYTES_V1
    );
};

/// Fixed section discriminator and ordering for the replacement composite proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ZkAmsMkheRnsNativeProofSectionKindV1 {
    /// Terminal Hyrax proof and cross-basis Bulletproof bridge.
    TerminalHyraxBpBridge = 1,
    /// Two aggregated RNS equations and their qPCS proof.
    RnsRelationQpcs = 2,
    /// Cross-field checks and the committed global lookup.
    CrossFieldGlobalLookup = 3,
    /// Proof that every governed padding lane is zero.
    ZeroPadding = 4,
}

impl ZkAmsMkheRnsNativeProofSectionKindV1 {
    /// Return the exact governed cap for this section.
    #[must_use]
    pub const fn max_bytes(self) -> u32 {
        match self {
            Self::TerminalHyraxBpBridge => {
                ZK_AMS_MKHE_RNS_NATIVE_TERMINAL_BRIDGE_SECTION_MAX_BYTES_V1
            }
            Self::RnsRelationQpcs => ZK_AMS_MKHE_RNS_NATIVE_RNS_RELATION_QPCS_SECTION_MAX_BYTES_V1,
            Self::CrossFieldGlobalLookup => {
                ZK_AMS_MKHE_RNS_NATIVE_CROSS_FIELD_LOOKUP_SECTION_MAX_BYTES_V1
            }
            Self::ZeroPadding => ZK_AMS_MKHE_RNS_NATIVE_ZERO_PADDING_SECTION_MAX_BYTES_V1,
        }
    }

    const fn index(self) -> usize {
        self as usize - 1
    }
}

impl TryFrom<u8> for ZkAmsMkheRnsNativeProofSectionKindV1 {
    type Error = ZkAmsMkheErrorV1;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::TerminalHyraxBpBridge),
            2 => Ok(Self::RnsRelationQpcs),
            3 => Ok(Self::CrossFieldGlobalLookup),
            4 => Ok(Self::ZeroPadding),
            _ => Err(ZkAmsMkheErrorV1::InvalidWireEncoding),
        }
    }
}

/// Sole canonical section order.
pub const ZK_AMS_MKHE_RNS_NATIVE_PROOF_SECTION_ORDER_V1: [ZkAmsMkheRnsNativeProofSectionKindV1;
    ZK_AMS_MKHE_RNS_NATIVE_PROOF_SECTION_COUNT_V1] = [
    ZkAmsMkheRnsNativeProofSectionKindV1::TerminalHyraxBpBridge,
    ZkAmsMkheRnsNativeProofSectionKindV1::RnsRelationQpcs,
    ZkAmsMkheRnsNativeProofSectionKindV1::CrossFieldGlobalLookup,
    ZkAmsMkheRnsNativeProofSectionKindV1::ZeroPadding,
];

/// Fixed-width descriptor of one ordered proof section.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkAmsMkheRnsNativeProofSectionDescriptorV1 {
    kind: ZkAmsMkheRnsNativeProofSectionKindV1,
    max_bytes: u32,
    encoded_bytes: u32,
    section_digest: [u8; 32],
}

impl ZkAmsMkheRnsNativeProofSectionDescriptorV1 {
    /// Section kind at this canonical position.
    #[must_use]
    pub const fn kind(self) -> ZkAmsMkheRnsNativeProofSectionKindV1 {
        self.kind
    }

    /// Exact governed section cap serialized into the descriptor.
    #[must_use]
    pub const fn max_bytes(self) -> u32 {
        self.max_bytes
    }

    /// Exact payload bytes following the fixed envelope header.
    #[must_use]
    pub const fn encoded_bytes(self) -> u32 {
        self.encoded_bytes
    }

    /// Digest binding the kind, cap, length, and exact section bytes.
    #[must_use]
    pub const fn section_digest(self) -> [u8; 32] {
        self.section_digest
    }
}

#[derive(Clone, Copy)]
struct ReplacementBindingsV1 {
    profile_manifest_digest: [u8; 32],
    topology_digest: [u8; 32],
    release_candidate_digest: [u8; 32],
}

#[derive(Clone, Copy)]
struct ValidatedSourceContextV1 {
    bindings: ReplacementBindingsV1,
    statement_digest: [u8; 32],
    operational_context_digest: [u8; 32],
    source_receipt_digest: [u8; 32],
}

/// Move-only owner of one canonical replacement composite-proof envelope.
///
/// Section bytes are intentionally not `Clone`; decoding creates one bounded
/// owner after all lengths, digests, and contextual bindings pass preflight.
#[derive(PartialEq, Eq)]
pub struct ZkAmsMkheRnsNativeProofEnvelopeV1 {
    version: u8,
    profile_manifest_digest: [u8; 32],
    topology_digest: [u8; 32],
    release_candidate_digest: [u8; 32],
    statement_digest: [u8; 32],
    operational_context_digest: [u8; 32],
    source_receipt_digest: [u8; 32],
    total_wire_bytes: u32,
    descriptors:
        [ZkAmsMkheRnsNativeProofSectionDescriptorV1; ZK_AMS_MKHE_RNS_NATIVE_PROOF_SECTION_COUNT_V1],
    proof_digest: [u8; 32],
    sections: [Vec<u8>; ZK_AMS_MKHE_RNS_NATIVE_PROOF_SECTION_COUNT_V1],
}

impl core::fmt::Debug for ZkAmsMkheRnsNativeProofEnvelopeV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheRnsNativeProofEnvelopeV1")
            .field("version", &self.version)
            .field(
                "profile_manifest_digest",
                &hex::encode(self.profile_manifest_digest),
            )
            .field("topology_digest", &hex::encode(self.topology_digest))
            .field(
                "release_candidate_digest",
                &hex::encode(self.release_candidate_digest),
            )
            .field("statement_digest", &hex::encode(self.statement_digest))
            .field(
                "operational_context_digest",
                &hex::encode(self.operational_context_digest),
            )
            .field(
                "source_receipt_digest",
                &hex::encode(self.source_receipt_digest),
            )
            .field("total_wire_bytes", &self.total_wire_bytes)
            .field("descriptors", &self.descriptors)
            .field("proof_digest", &hex::encode(self.proof_digest))
            .field("section_bytes", &"<redacted>")
            .finish()
    }
}

impl ZkAmsMkheRnsNativeProofEnvelopeV1 {
    /// Construct the sole ordered envelope without copying any section owner.
    ///
    /// # Errors
    ///
    /// Rejects zero or colliding contexts, empty/oversized sections, resource
    /// overflow, or a mismatch in the fixed replacement identities.
    pub fn new(
        source_layout: ZkAmsMkheRnsNativeSourceLayoutV1,
        source_receipt: ZkAmsMkheRnsNativeSourceReceiptV1,
        terminal_hyrax_bp_bridge: Vec<u8>,
        rns_relation_qpcs: Vec<u8>,
        cross_field_global_lookup: Vec<u8>,
        zero_padding: Vec<u8>,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let source = validated_source_context_v1(source_layout, source_receipt)?;
        let sections = [
            terminal_hyrax_bp_bridge,
            rns_relation_qpcs,
            cross_field_global_lookup,
            zero_padding,
        ];
        for (kind, section) in ZK_AMS_MKHE_RNS_NATIVE_PROOF_SECTION_ORDER_V1
            .into_iter()
            .zip(sections.iter())
        {
            validate_section_length_v1(kind, section.len())?;
        }
        let mut descriptors = [ZkAmsMkheRnsNativeProofSectionDescriptorV1 {
            kind: ZkAmsMkheRnsNativeProofSectionKindV1::TerminalHyraxBpBridge,
            max_bytes: 0,
            encoded_bytes: 0,
            section_digest: [0; 32],
        }; ZK_AMS_MKHE_RNS_NATIVE_PROOF_SECTION_COUNT_V1];
        for (index, kind) in ZK_AMS_MKHE_RNS_NATIVE_PROOF_SECTION_ORDER_V1
            .into_iter()
            .enumerate()
        {
            descriptors[index] = ZkAmsMkheRnsNativeProofSectionDescriptorV1 {
                kind,
                max_bytes: kind.max_bytes(),
                encoded_bytes: u32::try_from(sections[index].len())
                    .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
                section_digest: section_digest_v1(kind, &sections[index]),
            };
        }
        let total_wire_bytes = checked_total_wire_bytes_v1(&descriptors)?;
        let mut envelope = Self {
            version: ZK_AMS_MKHE_RNS_NATIVE_PROOF_ENVELOPE_VERSION_V1,
            profile_manifest_digest: source.bindings.profile_manifest_digest,
            topology_digest: source.bindings.topology_digest,
            release_candidate_digest: source.bindings.release_candidate_digest,
            statement_digest: source.statement_digest,
            operational_context_digest: source.operational_context_digest,
            source_receipt_digest: source.source_receipt_digest,
            total_wire_bytes,
            descriptors,
            proof_digest: [0; 32],
            sections,
        };
        envelope.proof_digest = whole_proof_digest_v1(&envelope);
        envelope.validate_transport_v1()?;
        Ok(envelope)
    }

    /// Encode the sole canonical representation.
    ///
    /// # Errors
    ///
    /// Returns an error if any binding, descriptor, section, digest, or total
    /// length is no longer canonical, or if the bounded output allocation fails.
    pub fn to_canonical_bytes_v1(&self) -> Result<Vec<u8>, ZkAmsMkheErrorV1> {
        self.validate_transport_v1()?;
        let total = usize::try_from(self.total_wire_bytes)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(total)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        bytes.extend_from_slice(&RNS_NATIVE_PROOF_ENVELOPE_TAG_V1);
        bytes.push(self.version);
        for digest in [
            self.profile_manifest_digest,
            self.topology_digest,
            self.release_candidate_digest,
            self.statement_digest,
            self.operational_context_digest,
            self.source_receipt_digest,
        ] {
            bytes.extend_from_slice(&digest);
        }
        bytes.push(ZK_AMS_MKHE_RNS_NATIVE_PROOF_SECTION_COUNT_V1 as u8);
        bytes.extend_from_slice(&self.total_wire_bytes.to_be_bytes());
        for descriptor in self.descriptors {
            bytes.push(descriptor.kind as u8);
            bytes.extend_from_slice(&descriptor.max_bytes.to_be_bytes());
            bytes.extend_from_slice(&descriptor.encoded_bytes.to_be_bytes());
            bytes.extend_from_slice(&descriptor.section_digest);
        }
        bytes.extend_from_slice(&self.proof_digest);
        for section in &self.sections {
            bytes.extend_from_slice(section);
        }
        if bytes.len() != total || bytes.capacity() < total {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(bytes)
    }

    /// Preflight, decode, and own exactly one canonical envelope.
    ///
    /// The independently retained source layout and receipt supply the exact
    /// statement, operational context, and structural provenance.  The receipt
    /// is revalidated against that layout before its digest is trusted.  All
    /// metadata and hashes are checked against borrowed input before any
    /// section allocation occurs.  This is transport validation only, not
    /// proof verification.
    ///
    /// # Errors
    ///
    /// Rejects every wrong length, tag, order, cap, context, section digest,
    /// whole-proof digest, trailing byte, or failed bounded allocation.
    pub fn from_canonical_bytes_exact_v1(
        bytes: &[u8],
        source_layout: ZkAmsMkheRnsNativeSourceLayoutV1,
        source_receipt: ZkAmsMkheRnsNativeSourceReceiptV1,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        preflight_outer_length_v1(bytes)?;
        let source = validated_source_context_v1(source_layout, source_receipt)?;
        let preflight = preflight_v1(bytes, source)?;
        let mut sections: [Vec<u8>; ZK_AMS_MKHE_RNS_NATIVE_PROOF_SECTION_COUNT_V1] =
            core::array::from_fn(|_| Vec::new());
        for (index, section) in sections.iter_mut().enumerate() {
            let start = preflight.section_offsets[index];
            let length = usize::try_from(preflight.descriptors[index].encoded_bytes)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
            let end = start
                .checked_add(length)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
            let source = bytes
                .get(start..end)
                .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
            section
                .try_reserve_exact(length)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
            section.extend_from_slice(source);
        }
        Ok(Self {
            version: ZK_AMS_MKHE_RNS_NATIVE_PROOF_ENVELOPE_VERSION_V1,
            profile_manifest_digest: preflight.bindings.profile_manifest_digest,
            topology_digest: preflight.bindings.topology_digest,
            release_candidate_digest: preflight.bindings.release_candidate_digest,
            statement_digest: source.statement_digest,
            operational_context_digest: source.operational_context_digest,
            source_receipt_digest: source.source_receipt_digest,
            total_wire_bytes: preflight.total_wire_bytes,
            descriptors: preflight.descriptors,
            proof_digest: preflight.proof_digest,
            sections,
        })
    }

    /// Digest of the canonical non-authorizing RNS profile manifest.
    #[must_use]
    pub const fn profile_manifest_digest(&self) -> [u8; 32] {
        self.profile_manifest_digest
    }

    /// Digest of the fixed replacement proof topology.
    #[must_use]
    pub const fn topology_digest(&self) -> [u8; 32] {
        self.topology_digest
    }

    /// Digest of the deliberately non-authorizing replacement release candidate.
    #[must_use]
    pub const fn release_candidate_digest(&self) -> [u8; 32] {
        self.release_candidate_digest
    }

    /// Exact proved-statement digest.
    #[must_use]
    pub const fn statement_digest(&self) -> [u8; 32] {
        self.statement_digest
    }

    /// Exact operational/replay-context digest.
    #[must_use]
    pub const fn operational_context_digest(&self) -> [u8; 32] {
        self.operational_context_digest
    }

    /// Structural receipt digest for the authenticated source snapshot.
    #[must_use]
    pub const fn source_receipt_digest(&self) -> [u8; 32] {
        self.source_receipt_digest
    }

    /// Complete canonical envelope bytes.
    #[must_use]
    pub const fn total_wire_bytes(&self) -> u32 {
        self.total_wire_bytes
    }

    /// Four fixed ordered descriptors.
    #[must_use]
    pub const fn descriptors(
        &self,
    ) -> &[ZkAmsMkheRnsNativeProofSectionDescriptorV1; ZK_AMS_MKHE_RNS_NATIVE_PROOF_SECTION_COUNT_V1]
    {
        &self.descriptors
    }

    /// Borrow one proof-system-specific section without copying it.
    #[must_use]
    pub fn section(&self, kind: ZkAmsMkheRnsNativeProofSectionKindV1) -> &[u8] {
        &self.sections[kind.index()]
    }

    /// Digest of the complete canonical envelope content.
    ///
    /// This digest proves byte integrity only; it is not proof-system validity.
    #[must_use]
    pub const fn proof_digest(&self) -> [u8; 32] {
        self.proof_digest
    }

    fn validate_transport_v1(&self) -> Result<(), ZkAmsMkheErrorV1> {
        let bindings = replacement_bindings_v1()?;
        validate_identity_digests_v1(
            bindings,
            self.statement_digest,
            self.operational_context_digest,
            self.source_receipt_digest,
        )?;
        if self.version != ZK_AMS_MKHE_RNS_NATIVE_PROOF_ENVELOPE_VERSION_V1
            || self.profile_manifest_digest != bindings.profile_manifest_digest
            || self.topology_digest != bindings.topology_digest
            || self.release_candidate_digest != bindings.release_candidate_digest
        {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        for (index, kind) in ZK_AMS_MKHE_RNS_NATIVE_PROOF_SECTION_ORDER_V1
            .into_iter()
            .enumerate()
        {
            let descriptor = self.descriptors[index];
            let section = &self.sections[index];
            validate_section_length_v1(kind, section.len())?;
            if descriptor.kind != kind
                || descriptor.max_bytes != kind.max_bytes()
                || usize::try_from(descriptor.encoded_bytes).ok() != Some(section.len())
                || descriptor.section_digest == [0; 32]
                || descriptor.section_digest != section_digest_v1(kind, section)
            {
                return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
            }
        }
        if self.total_wire_bytes != checked_total_wire_bytes_v1(&self.descriptors)?
            || self.proof_digest == [0; 32]
            || self.proof_digest != whole_proof_digest_v1(self)
        {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(())
    }
}

struct PreflightV1 {
    bindings: ReplacementBindingsV1,
    total_wire_bytes: u32,
    descriptors:
        [ZkAmsMkheRnsNativeProofSectionDescriptorV1; ZK_AMS_MKHE_RNS_NATIVE_PROOF_SECTION_COUNT_V1],
    proof_digest: [u8; 32],
    section_offsets: [usize; ZK_AMS_MKHE_RNS_NATIVE_PROOF_SECTION_COUNT_V1],
}

fn preflight_v1(
    bytes: &[u8],
    source: ValidatedSourceContextV1,
) -> Result<PreflightV1, ZkAmsMkheErrorV1> {
    preflight_outer_length_v1(bytes)?;
    let bindings = source.bindings;
    let expected_statement_digest = source.statement_digest;
    let expected_operational_context_digest = source.operational_context_digest;
    let expected_source_receipt_digest = source.source_receipt_digest;
    let mut decoder = RnsNativeProofDecoderV1::new(bytes);
    decoder.expect_bytes(&RNS_NATIVE_PROOF_ENVELOPE_TAG_V1)?;
    decoder.expect_u8(ZK_AMS_MKHE_RNS_NATIVE_PROOF_ENVELOPE_VERSION_V1)?;
    for expected in [
        bindings.profile_manifest_digest,
        bindings.topology_digest,
        bindings.release_candidate_digest,
        expected_statement_digest,
        expected_operational_context_digest,
        expected_source_receipt_digest,
    ] {
        if decoder.array::<32>()? != expected {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
    }
    decoder.expect_u8(
        u8::try_from(ZK_AMS_MKHE_RNS_NATIVE_PROOF_SECTION_COUNT_V1)
            .map_err(|_| ZkAmsMkheErrorV1::InvalidProfile)?,
    )?;
    let total_wire_bytes = decoder.u32()?;
    if total_wire_bytes > ZK_AMS_MKHE_RNS_NATIVE_PROOF_ENVELOPE_MAX_BYTES_V1 {
        return Err(ZkAmsMkheErrorV1::WireTooLarge);
    }
    if usize::try_from(total_wire_bytes).ok() != Some(bytes.len()) {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let mut descriptors = [ZkAmsMkheRnsNativeProofSectionDescriptorV1 {
        kind: ZkAmsMkheRnsNativeProofSectionKindV1::TerminalHyraxBpBridge,
        max_bytes: 0,
        encoded_bytes: 0,
        section_digest: [0; 32],
    }; ZK_AMS_MKHE_RNS_NATIVE_PROOF_SECTION_COUNT_V1];
    for (index, expected_kind) in ZK_AMS_MKHE_RNS_NATIVE_PROOF_SECTION_ORDER_V1
        .into_iter()
        .enumerate()
    {
        let kind = ZkAmsMkheRnsNativeProofSectionKindV1::try_from(decoder.u8()?)?;
        let max_bytes = decoder.u32()?;
        let encoded_bytes = decoder.u32()?;
        let section_digest = decoder.array()?;
        if kind != expected_kind || max_bytes != expected_kind.max_bytes() {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        validate_section_length_v1(
            kind,
            usize::try_from(encoded_bytes)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )?;
        if section_digest == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        descriptors[index] = ZkAmsMkheRnsNativeProofSectionDescriptorV1 {
            kind,
            max_bytes,
            encoded_bytes,
            section_digest,
        };
    }
    let proof_digest = decoder.array()?;
    if decoder.position() != ZK_AMS_MKHE_RNS_NATIVE_PROOF_ENVELOPE_HEADER_BYTES_V1
        || proof_digest == [0; 32]
        || checked_total_wire_bytes_v1(&descriptors)? != total_wire_bytes
    {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }

    let mut section_offsets = [0_usize; ZK_AMS_MKHE_RNS_NATIVE_PROOF_SECTION_COUNT_V1];
    let mut cursor = ZK_AMS_MKHE_RNS_NATIVE_PROOF_ENVELOPE_HEADER_BYTES_V1;
    for (index, descriptor) in descriptors.iter().enumerate() {
        section_offsets[index] = cursor;
        let length = usize::try_from(descriptor.encoded_bytes)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let end = cursor
            .checked_add(length)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let section = bytes
            .get(cursor..end)
            .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
        if section_digest_v1(descriptor.kind, section) != descriptor.section_digest {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        cursor = end;
    }
    let section_slices = [
        section_slice_v1(bytes, section_offsets[0], descriptors[0])?,
        section_slice_v1(bytes, section_offsets[1], descriptors[1])?,
        section_slice_v1(bytes, section_offsets[2], descriptors[2])?,
        section_slice_v1(bytes, section_offsets[3], descriptors[3])?,
    ];
    if cursor != bytes.len()
        || proof_digest
            != whole_proof_digest_from_parts_v1(
                bindings,
                expected_statement_digest,
                expected_operational_context_digest,
                expected_source_receipt_digest,
                total_wire_bytes,
                &descriptors,
                section_slices,
            )
    {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    Ok(PreflightV1 {
        bindings,
        total_wire_bytes,
        descriptors,
        proof_digest,
        section_offsets,
    })
}

fn preflight_outer_length_v1(bytes: &[u8]) -> Result<(), ZkAmsMkheErrorV1> {
    let envelope_cap = usize::try_from(ZK_AMS_MKHE_RNS_NATIVE_PROOF_ENVELOPE_MAX_BYTES_V1)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if bytes.len() > envelope_cap {
        return Err(ZkAmsMkheErrorV1::WireTooLarge);
    }
    if bytes.len() < ZK_AMS_MKHE_RNS_NATIVE_PROOF_ENVELOPE_HEADER_BYTES_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    Ok(())
}

fn replacement_bindings_v1() -> Result<ReplacementBindingsV1, ZkAmsMkheErrorV1> {
    let profile_manifest = zk_ams_mkhe_rns_native_profile_manifest_v1()?;
    profile_manifest.validate()?;
    let bindings = ReplacementBindingsV1 {
        profile_manifest_digest: profile_manifest.manifest_digest,
        topology_digest: profile_manifest.proof_topology_digest,
        release_candidate_digest: zk_ams_mkhe_rns_native_release_candidate_digest_v1()?,
    };
    if [
        bindings.profile_manifest_digest,
        bindings.topology_digest,
        bindings.release_candidate_digest,
    ]
    .into_iter()
    .any(|digest| digest == [0; 32])
    {
        return Err(ZkAmsMkheErrorV1::InvalidProfile);
    }
    Ok(bindings)
}

fn validated_source_context_v1(
    layout: ZkAmsMkheRnsNativeSourceLayoutV1,
    receipt: ZkAmsMkheRnsNativeSourceReceiptV1,
) -> Result<ValidatedSourceContextV1, ZkAmsMkheErrorV1> {
    layout
        .validate()
        .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    receipt
        .validate(layout)
        .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    let bindings = replacement_bindings_v1()?;
    let profile = zk_ams_mkhe_rns_native_profile_v1()?;
    if layout.profile_digest() != profile.profile_digest
        || layout.topology_digest() != bindings.topology_digest
        || layout.release_candidate_digest() != bindings.release_candidate_digest
    {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let source = ValidatedSourceContextV1 {
        bindings,
        statement_digest: layout.statement_digest(),
        operational_context_digest: layout.operational_context_digest(),
        source_receipt_digest: receipt.receipt_digest,
    };
    validate_identity_digests_v1(
        source.bindings,
        source.statement_digest,
        source.operational_context_digest,
        source.source_receipt_digest,
    )?;
    Ok(source)
}

fn validate_identity_digests_v1(
    bindings: ReplacementBindingsV1,
    statement_digest: [u8; 32],
    operational_context_digest: [u8; 32],
    source_receipt_digest: [u8; 32],
) -> Result<(), ZkAmsMkheErrorV1> {
    let identities = [
        bindings.profile_manifest_digest,
        bindings.topology_digest,
        bindings.release_candidate_digest,
        statement_digest,
        operational_context_digest,
        source_receipt_digest,
    ];
    if identities.into_iter().any(|digest| digest == [0; 32])
        || identities
            .iter()
            .enumerate()
            .any(|(index, digest)| identities[index + 1..].contains(digest))
    {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    Ok(())
}

fn validate_section_length_v1(
    kind: ZkAmsMkheRnsNativeProofSectionKindV1,
    length: usize,
) -> Result<(), ZkAmsMkheErrorV1> {
    if length == 0 {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    if length
        > usize::try_from(kind.max_bytes())
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
    {
        return Err(ZkAmsMkheErrorV1::WireTooLarge);
    }
    Ok(())
}

fn checked_total_wire_bytes_v1(
    descriptors: &[ZkAmsMkheRnsNativeProofSectionDescriptorV1;
         ZK_AMS_MKHE_RNS_NATIVE_PROOF_SECTION_COUNT_V1],
) -> Result<u32, ZkAmsMkheErrorV1> {
    let mut total = ZK_AMS_MKHE_RNS_NATIVE_PROOF_ENVELOPE_HEADER_BYTES_V1;
    for descriptor in descriptors {
        total = total
            .checked_add(
                usize::try_from(descriptor.encoded_bytes)
                    .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
            )
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    }
    if total
        > usize::try_from(ZK_AMS_MKHE_RNS_NATIVE_PROOF_ENVELOPE_MAX_BYTES_V1)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
    {
        return Err(ZkAmsMkheErrorV1::WireTooLarge);
    }
    u32::try_from(total).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)
}

fn section_slice_v1(
    bytes: &[u8],
    offset: usize,
    descriptor: ZkAmsMkheRnsNativeProofSectionDescriptorV1,
) -> Result<&[u8], ZkAmsMkheErrorV1> {
    let length = usize::try_from(descriptor.encoded_bytes)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let end = offset
        .checked_add(length)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    bytes
        .get(offset..end)
        .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)
}

fn section_digest_v1(kind: ZkAmsMkheRnsNativeProofSectionKindV1, bytes: &[u8]) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v1.mkhe.rns-native-proof-section");
    hash.update(&[ZK_AMS_MKHE_RNS_NATIVE_PROOF_ENVELOPE_VERSION_V1, kind as u8]);
    hash.update(&kind.max_bytes().to_be_bytes());
    hash.update(&(bytes.len() as u64).to_be_bytes());
    hash.update(bytes);
    hash.finalize()
}

fn whole_proof_digest_v1(envelope: &ZkAmsMkheRnsNativeProofEnvelopeV1) -> [u8; 32] {
    whole_proof_digest_from_parts_v1(
        ReplacementBindingsV1 {
            profile_manifest_digest: envelope.profile_manifest_digest,
            topology_digest: envelope.topology_digest,
            release_candidate_digest: envelope.release_candidate_digest,
        },
        envelope.statement_digest,
        envelope.operational_context_digest,
        envelope.source_receipt_digest,
        envelope.total_wire_bytes,
        &envelope.descriptors,
        [
            &envelope.sections[0],
            &envelope.sections[1],
            &envelope.sections[2],
            &envelope.sections[3],
        ],
    )
}

#[allow(clippy::too_many_arguments)]
fn whole_proof_digest_from_parts_v1(
    bindings: ReplacementBindingsV1,
    statement_digest: [u8; 32],
    operational_context_digest: [u8; 32],
    source_receipt_digest: [u8; 32],
    total_wire_bytes: u32,
    descriptors: &[ZkAmsMkheRnsNativeProofSectionDescriptorV1;
         ZK_AMS_MKHE_RNS_NATIVE_PROOF_SECTION_COUNT_V1],
    sections: [&[u8]; ZK_AMS_MKHE_RNS_NATIVE_PROOF_SECTION_COUNT_V1],
) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v1.mkhe.rns-native-composite-proof-envelope");
    hash.update(&RNS_NATIVE_PROOF_ENVELOPE_TAG_V1);
    hash.update(&[ZK_AMS_MKHE_RNS_NATIVE_PROOF_ENVELOPE_VERSION_V1]);
    hash.update(&bindings.profile_manifest_digest);
    hash.update(&bindings.topology_digest);
    hash.update(&bindings.release_candidate_digest);
    hash.update(&statement_digest);
    hash.update(&operational_context_digest);
    hash.update(&source_receipt_digest);
    hash.update(&[ZK_AMS_MKHE_RNS_NATIVE_PROOF_SECTION_COUNT_V1 as u8]);
    hash.update(&total_wire_bytes.to_be_bytes());
    for descriptor in descriptors {
        hash.update(&[descriptor.kind as u8]);
        hash.update(&descriptor.max_bytes.to_be_bytes());
        hash.update(&descriptor.encoded_bytes.to_be_bytes());
        hash.update(&descriptor.section_digest);
    }
    for section in sections {
        hash.update(section);
    }
    hash.finalize()
}

struct RnsNativeProofDecoderV1<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> RnsNativeProofDecoderV1<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], ZkAmsMkheErrorV1> {
        let end = self
            .cursor
            .checked_add(N)
            .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
        let value = self
            .bytes
            .get(self.cursor..end)
            .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?
            .try_into()
            .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?;
        self.cursor = end;
        Ok(value)
    }

    fn expect_bytes<const N: usize>(&mut self, expected: &[u8; N]) -> Result<(), ZkAmsMkheErrorV1> {
        if self.array::<N>()? != *expected {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(())
    }

    fn u8(&mut self) -> Result<u8, ZkAmsMkheErrorV1> {
        Ok(self.array::<1>()?[0])
    }

    fn expect_u8(&mut self, expected: u8) -> Result<(), ZkAmsMkheErrorV1> {
        if self.u8()? != expected {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(())
    }

    fn u32(&mut self) -> Result<u32, ZkAmsMkheErrorV1> {
        Ok(u32::from_be_bytes(self.array()?))
    }

    const fn position(&self) -> usize {
        self.cursor
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        rns_native_profile::zk_ams_mkhe_rns_native_topology_v1,
        rns_native_source::{
            ZkAmsMkheRnsNativeSecretChunkV1, ZkAmsMkheRnsNativeSourceArenaV1,
            ZkAmsMkheRnsNativeSourceErrorV1, ZkAmsMkheRnsNativeSourceSnapshotV1,
        },
    };
    use super::*;

    const STATEMENT_DIGEST: [u8; 32] = [0x51; 32];
    const OPERATIONAL_CONTEXT_DIGEST: [u8; 32] = [0x52; 32];
    const TOTAL_WIRE_BYTES_OFFSET: usize = 4 + 1 + IDENTITY_DIGEST_COUNT_V1 * 32 + 1;

    struct TestSecretChunk {
        arena: ZkAmsMkheRnsNativeSourceArenaV1,
        bytes: Vec<u8>,
    }

    impl ZkAmsMkheRnsNativeSecretChunkV1 for TestSecretChunk {
        fn arena(&self) -> ZkAmsMkheRnsNativeSourceArenaV1 {
            self.arena
        }

        fn as_slice(&self) -> &[u8] {
            &self.bytes
        }

        fn as_mut_slice(&mut self) -> &mut [u8] {
            &mut self.bytes
        }
    }

    struct TestSourceSnapshot {
        layout: ZkAmsMkheRnsNativeSourceLayoutV1,
        main_snapshot_digest: [u8; 32],
        nonce_snapshot_digest: [u8; 32],
    }

    impl ZkAmsMkheRnsNativeSourceSnapshotV1 for TestSourceSnapshot {
        type Chunk = TestSecretChunk;

        fn layout(&self) -> ZkAmsMkheRnsNativeSourceLayoutV1 {
            self.layout
        }

        fn snapshot_digest(&self, arena: ZkAmsMkheRnsNativeSourceArenaV1) -> [u8; 32] {
            match arena {
                ZkAmsMkheRnsNativeSourceArenaV1::Main => self.main_snapshot_digest,
                ZkAmsMkheRnsNativeSourceArenaV1::Nonce => self.nonce_snapshot_digest,
            }
        }

        fn read_slot(
            &mut self,
            _arena: ZkAmsMkheRnsNativeSourceArenaV1,
            _slot: u64,
        ) -> Result<Self::Chunk, ZkAmsMkheRnsNativeSourceErrorV1> {
            Err(ZkAmsMkheRnsNativeSourceErrorV1::Storage)
        }
    }

    fn source_context(
        statement_digest: [u8; 32],
        operational_context_digest: [u8; 32],
        main_snapshot_digest: [u8; 32],
        nonce_snapshot_digest: [u8; 32],
    ) -> (
        ZkAmsMkheRnsNativeSourceLayoutV1,
        ZkAmsMkheRnsNativeSourceReceiptV1,
    ) {
        let profile = zk_ams_mkhe_rns_native_profile_v1().unwrap();
        let topology = zk_ams_mkhe_rns_native_topology_v1().unwrap();
        let layout = ZkAmsMkheRnsNativeSourceLayoutV1::new(
            profile.profile_digest,
            topology.topology_digest,
            zk_ams_mkhe_rns_native_release_candidate_digest_v1().unwrap(),
            statement_digest,
            operational_context_digest,
        )
        .unwrap();
        let snapshot = TestSourceSnapshot {
            layout,
            main_snapshot_digest,
            nonce_snapshot_digest,
        };
        let receipt = snapshot.structural_receipt().unwrap();
        (layout, receipt)
    }

    fn canonical_source_context() -> (
        ZkAmsMkheRnsNativeSourceLayoutV1,
        ZkAmsMkheRnsNativeSourceReceiptV1,
    ) {
        source_context(
            STATEMENT_DIGEST,
            OPERATIONAL_CONTEXT_DIGEST,
            [0x53; 32],
            [0x54; 32],
        )
    }

    fn envelope() -> ZkAmsMkheRnsNativeProofEnvelopeV1 {
        let (layout, receipt) = canonical_source_context();
        ZkAmsMkheRnsNativeProofEnvelopeV1::new(
            layout,
            receipt,
            vec![0x11; 3],
            vec![0x22; 5],
            vec![0x33; 7],
            vec![0x44; 9],
        )
        .expect("canonical replacement proof envelope")
    }

    fn decode(bytes: &[u8]) -> Result<ZkAmsMkheRnsNativeProofEnvelopeV1, ZkAmsMkheErrorV1> {
        let (layout, receipt) = canonical_source_context();
        ZkAmsMkheRnsNativeProofEnvelopeV1::from_canonical_bytes_exact_v1(bytes, layout, receipt)
    }

    #[test]
    fn canonical_envelope_roundtrips_without_section_aliases() {
        let (layout, receipt) = canonical_source_context();
        let envelope = envelope();
        let bytes = envelope.to_canonical_bytes_v1().unwrap();
        let decoded = ZkAmsMkheRnsNativeProofEnvelopeV1::from_canonical_bytes_exact_v1(
            &bytes, layout, receipt,
        )
        .unwrap();

        assert_eq!(
            bytes.len(),
            ZK_AMS_MKHE_RNS_NATIVE_PROOF_ENVELOPE_HEADER_BYTES_V1 + 24
        );
        assert_eq!(decoded.to_canonical_bytes_v1().unwrap(), bytes);
        assert_eq!(decoded, envelope);
        assert_eq!(decoded.total_wire_bytes() as usize, bytes.len());
        assert_ne!(decoded.profile_manifest_digest(), [0; 32]);
        assert_ne!(decoded.topology_digest(), [0; 32]);
        assert_eq!(
            decoded.release_candidate_digest(),
            layout.release_candidate_digest()
        );
        assert_eq!(decoded.statement_digest(), layout.statement_digest());
        assert_eq!(
            decoded.operational_context_digest(),
            layout.operational_context_digest()
        );
        assert_eq!(decoded.source_receipt_digest(), receipt.receipt_digest);
        assert_ne!(decoded.proof_digest(), [0; 32]);
        for (index, kind) in ZK_AMS_MKHE_RNS_NATIVE_PROOF_SECTION_ORDER_V1
            .into_iter()
            .enumerate()
        {
            assert_eq!(decoded.descriptors()[index].kind(), kind);
            assert_eq!(decoded.descriptors()[index].max_bytes(), kind.max_bytes());
            assert_eq!(
                decoded.descriptors()[index].encoded_bytes() as usize,
                decoded.section(kind).len()
            );
            assert_ne!(decoded.descriptors()[index].section_digest(), [0; 32]);
        }
        assert!(format!("{decoded:?}").contains("<redacted>"));
    }

    #[test]
    fn exact_decoder_rejects_every_truncation_and_trailing_length() {
        let canonical = envelope().to_canonical_bytes_v1().unwrap();
        for length in 0..canonical.len() {
            assert!(
                decode(&canonical[..length]).is_err(),
                "accepted length {length}"
            );
        }
        for trailing in 1..=8 {
            let mut changed = canonical.clone();
            changed.resize(canonical.len() + trailing, 0);
            assert_eq!(decode(&changed), Err(ZkAmsMkheErrorV1::InvalidWireEncoding));
        }
    }

    #[test]
    fn exact_decoder_rejects_every_tag_version_and_order_mutation() {
        let canonical = envelope().to_canonical_bytes_v1().unwrap();
        for tag_byte in 0..RNS_NATIVE_PROOF_ENVELOPE_TAG_V1.len() {
            let mut changed = canonical.clone();
            changed[tag_byte] ^= 1;
            assert_eq!(decode(&changed), Err(ZkAmsMkheErrorV1::InvalidWireEncoding));
        }
        let mut changed_version = canonical.clone();
        changed_version[4] ^= 1;
        assert_eq!(
            decode(&changed_version),
            Err(ZkAmsMkheErrorV1::InvalidWireEncoding)
        );

        for left in 0..ZK_AMS_MKHE_RNS_NATIVE_PROOF_SECTION_COUNT_V1 {
            for right in left + 1..ZK_AMS_MKHE_RNS_NATIVE_PROOF_SECTION_COUNT_V1 {
                let mut changed_order = canonical.clone();
                let left_start = SECTION_DESCRIPTORS_OFFSET_V1 + left * SECTION_DESCRIPTOR_BYTES_V1;
                let right_start =
                    SECTION_DESCRIPTORS_OFFSET_V1 + right * SECTION_DESCRIPTOR_BYTES_V1;
                let left_descriptor: [u8; SECTION_DESCRIPTOR_BYTES_V1] = changed_order
                    [left_start..left_start + SECTION_DESCRIPTOR_BYTES_V1]
                    .try_into()
                    .unwrap();
                let right_descriptor: [u8; SECTION_DESCRIPTOR_BYTES_V1] = changed_order
                    [right_start..right_start + SECTION_DESCRIPTOR_BYTES_V1]
                    .try_into()
                    .unwrap();
                changed_order[left_start..left_start + SECTION_DESCRIPTOR_BYTES_V1]
                    .copy_from_slice(&right_descriptor);
                changed_order[right_start..right_start + SECTION_DESCRIPTOR_BYTES_V1]
                    .copy_from_slice(&left_descriptor);
                assert_eq!(
                    decode(&changed_order),
                    Err(ZkAmsMkheErrorV1::InvalidWireEncoding),
                    "accepted descriptor swap {left}<->{right}"
                );
            }
        }
    }

    #[test]
    fn every_identity_section_and_whole_digest_mutation_is_rejected() {
        let canonical = envelope().to_canonical_bytes_v1().unwrap();
        for identity in 0..IDENTITY_DIGEST_COUNT_V1 {
            let mut changed = canonical.clone();
            changed[5 + identity * 32] ^= 1;
            assert_eq!(decode(&changed), Err(ZkAmsMkheErrorV1::InvalidWireEncoding));
        }
        for section in 0..ZK_AMS_MKHE_RNS_NATIVE_PROOF_SECTION_COUNT_V1 {
            let mut changed = canonical.clone();
            let digest_offset =
                SECTION_DESCRIPTORS_OFFSET_V1 + section * SECTION_DESCRIPTOR_BYTES_V1 + 1 + 4 + 4;
            changed[digest_offset] ^= 1;
            assert_eq!(decode(&changed), Err(ZkAmsMkheErrorV1::InvalidWireEncoding));
        }
        let mut changed_whole = canonical.clone();
        let whole_offset = SECTION_DESCRIPTORS_OFFSET_V1
            + ZK_AMS_MKHE_RNS_NATIVE_PROOF_SECTION_COUNT_V1 * SECTION_DESCRIPTOR_BYTES_V1;
        changed_whole[whole_offset] ^= 1;
        assert_eq!(
            decode(&changed_whole),
            Err(ZkAmsMkheErrorV1::InvalidWireEncoding)
        );

        let mut payload_cursor = ZK_AMS_MKHE_RNS_NATIVE_PROOF_ENVELOPE_HEADER_BYTES_V1;
        for length in [3, 5, 7, 9] {
            let mut changed_payload = canonical.clone();
            changed_payload[payload_cursor] ^= 1;
            assert_eq!(
                decode(&changed_payload),
                Err(ZkAmsMkheErrorV1::InvalidWireEncoding)
            );
            payload_cursor += length;
        }
    }

    #[test]
    fn independently_expected_contexts_reject_cross_statement_splicing() {
        let canonical = envelope().to_canonical_bytes_v1().unwrap();
        for (layout, receipt) in [
            source_context(
                [0x61; 32],
                OPERATIONAL_CONTEXT_DIGEST,
                [0x53; 32],
                [0x54; 32],
            ),
            source_context(STATEMENT_DIGEST, [0x62; 32], [0x53; 32], [0x54; 32]),
            source_context(
                STATEMENT_DIGEST,
                OPERATIONAL_CONTEXT_DIGEST,
                [0x63; 32],
                [0x64; 32],
            ),
        ] {
            assert_eq!(
                ZkAmsMkheRnsNativeProofEnvelopeV1::from_canonical_bytes_exact_v1(
                    &canonical, layout, receipt
                ),
                Err(ZkAmsMkheErrorV1::InvalidWireEncoding)
            );
        }

        let (layout, receipt) = canonical_source_context();
        let mut forged_receipt = receipt;
        forged_receipt.receipt_digest[0] ^= 1;
        assert!(
            ZkAmsMkheRnsNativeProofEnvelopeV1::new(
                layout,
                forged_receipt,
                vec![1],
                vec![2],
                vec![3],
                vec![4],
            )
            .is_err()
        );

        let profile = zk_ams_mkhe_rns_native_profile_v1().unwrap();
        let topology = zk_ams_mkhe_rns_native_topology_v1().unwrap();
        let foreign_candidate_layout = ZkAmsMkheRnsNativeSourceLayoutV1::new(
            profile.profile_digest,
            topology.topology_digest,
            [0x65; 32],
            STATEMENT_DIGEST,
            OPERATIONAL_CONTEXT_DIGEST,
        )
        .unwrap();
        let foreign_candidate_receipt = TestSourceSnapshot {
            layout: foreign_candidate_layout,
            main_snapshot_digest: [0x53; 32],
            nonce_snapshot_digest: [0x54; 32],
        }
        .structural_receipt()
        .unwrap();
        assert!(
            ZkAmsMkheRnsNativeProofEnvelopeV1::new(
                foreign_candidate_layout,
                foreign_candidate_receipt,
                vec![1],
                vec![2],
                vec![3],
                vec![4],
            )
            .is_err()
        );
    }

    #[test]
    fn preflight_rejects_every_section_and_total_cap_plus_one_before_allocation() {
        let canonical = envelope().to_canonical_bytes_v1().unwrap();
        for (index, kind) in ZK_AMS_MKHE_RNS_NATIVE_PROOF_SECTION_ORDER_V1
            .into_iter()
            .enumerate()
        {
            assert_eq!(
                validate_section_length_v1(kind, kind.max_bytes() as usize + 1),
                Err(ZkAmsMkheErrorV1::WireTooLarge)
            );
            let mut changed_length = canonical.clone();
            let descriptor_offset =
                SECTION_DESCRIPTORS_OFFSET_V1 + index * SECTION_DESCRIPTOR_BYTES_V1;
            let length_offset = descriptor_offset + 1 + 4;
            changed_length[length_offset..length_offset + 4]
                .copy_from_slice(&(kind.max_bytes() + 1).to_be_bytes());
            assert_eq!(decode(&changed_length), Err(ZkAmsMkheErrorV1::WireTooLarge));

            let mut changed_cap = canonical.clone();
            changed_cap[descriptor_offset + 1..descriptor_offset + 5]
                .copy_from_slice(&(kind.max_bytes() + 1).to_be_bytes());
            assert_eq!(
                decode(&changed_cap),
                Err(ZkAmsMkheErrorV1::InvalidWireEncoding)
            );
        }

        let mut changed_total = canonical;
        changed_total[TOTAL_WIRE_BYTES_OFFSET..TOTAL_WIRE_BYTES_OFFSET + 4].copy_from_slice(
            &(ZK_AMS_MKHE_RNS_NATIVE_PROOF_ENVELOPE_MAX_BYTES_V1 + 1).to_be_bytes(),
        );
        assert_eq!(decode(&changed_total), Err(ZkAmsMkheErrorV1::WireTooLarge));
    }
}
