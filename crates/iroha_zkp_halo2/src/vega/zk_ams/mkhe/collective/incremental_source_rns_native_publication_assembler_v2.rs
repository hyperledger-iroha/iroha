//! Fail-closed contract for assembling the authenticated 40-limb public-polynomial view.
//!
//! This privately declared, non-authorizing module describes the ownership and validation
//! boundary between the finalized 38-limb streaming authority,
//! the 43 finalized V1 ciphertext manifests, the V2 basis-extension tail lifecycle, and
//! the existing public-polynomial reader.  In particular, this contract never republishes
//! a V1 prefix and never turns a digest copy into publication authority.
//!
//! Production adapters remain uninhabited until the live owners can be consumed here and
//! the reader can consume the resulting handoff without exposing an untyped constructor.

#![allow(dead_code)]

use core::{convert::Infallible, fmt};
use std::collections::BTreeSet;

pub(super) const RNS_NATIVE_LEGACY_LIMB_COUNT_V2: usize = 38;
pub(super) const RNS_NATIVE_TARGET_LIMB_COUNT_V2: usize = 40;
pub(super) const RNS_NATIVE_RECORD_COUNT_V2: usize = 43;
pub(super) const RNS_NATIVE_KEY_PREFIX_RECEIPT_COUNT_V2: usize = 76;
pub(super) const RNS_NATIVE_CIPHERTEXT_PREFIX_RECEIPT_COUNT_V2: usize = 3_268;
pub(super) const RNS_NATIVE_PREFIX_RECEIPT_COUNT_V2: usize = 3_344;
pub(super) const RNS_NATIVE_KEY_TAIL_RECEIPT_COUNT_V2: usize = 4;
pub(super) const RNS_NATIVE_CIPHERTEXT_TAIL_RECEIPT_COUNT_V2: usize = 172;
pub(super) const RNS_NATIVE_TAIL_RECEIPT_COUNT_V2: usize = 176;
pub(super) const RNS_NATIVE_CANONICAL_DESCRIPTOR_COUNT_V2: usize = 3_520;
pub(super) const RNS_NATIVE_OBJECT_COEFFICIENT_COUNT_V2: usize = 131_072;
pub(super) const RNS_NATIVE_OBJECT_ENCODED_BYTE_COUNT_V2: usize =
    4 + 8 * RNS_NATIVE_OBJECT_COEFFICIENT_COUNT_V2;

const _: [(); RNS_NATIVE_KEY_PREFIX_RECEIPT_COUNT_V2] = [(); 2 * RNS_NATIVE_LEGACY_LIMB_COUNT_V2];
const _: [(); RNS_NATIVE_CIPHERTEXT_PREFIX_RECEIPT_COUNT_V2] =
    [(); 2 * RNS_NATIVE_RECORD_COUNT_V2 * RNS_NATIVE_LEGACY_LIMB_COUNT_V2];
const _: [(); RNS_NATIVE_PREFIX_RECEIPT_COUNT_V2] = [(); 3_344];
const _: [(); RNS_NATIVE_KEY_TAIL_RECEIPT_COUNT_V2] =
    [(); 2 * (RNS_NATIVE_TARGET_LIMB_COUNT_V2 - RNS_NATIVE_LEGACY_LIMB_COUNT_V2)];
const _: [(); RNS_NATIVE_CIPHERTEXT_TAIL_RECEIPT_COUNT_V2] = [(); 2
    * RNS_NATIVE_RECORD_COUNT_V2
    * (RNS_NATIVE_TARGET_LIMB_COUNT_V2 - RNS_NATIVE_LEGACY_LIMB_COUNT_V2)];
const _: [(); RNS_NATIVE_TAIL_RECEIPT_COUNT_V2] = [(); 176];
const _: [(); RNS_NATIVE_CANONICAL_DESCRIPTOR_COUNT_V2] =
    [(); RNS_NATIVE_PREFIX_RECEIPT_COUNT_V2 + RNS_NATIVE_TAIL_RECEIPT_COUNT_V2];

pub(super) const RNS_NATIVE_PUBLICATION_ASSEMBLER_CONTRACT_IMPLEMENTED_V2: bool = true;
pub(super) const RNS_NATIVE_PUBLICATION_ASSEMBLER_LIVE_OWNER_INTEGRATED_V2: bool = false;
pub(super) const RNS_NATIVE_PUBLICATION_ASSEMBLER_PRODUCTION_ADAPTER_AVAILABLE_V2: bool = false;
pub(super) const RNS_NATIVE_PUBLICATION_ASSEMBLER_READER_INTEGRATED_V2: bool = false;
pub(super) const RNS_NATIVE_PUBLICATION_ASSEMBLER_READINESS_V2: bool = false;
pub(super) const RNS_NATIVE_PUBLICATION_ASSEMBLER_RELEASE_AUTHORIZED_V2: bool = false;

const COMPOSITE_PROVIDER_DOMAIN_V2: &[u8] =
    b"iroha.zk_ams.rns_native.publication.composite_provider.v2";
const CANONICAL_MANIFEST_DOMAIN_V2: &[u8] =
    b"iroha.zk_ams.rns_native.publication.canonical_manifest.v2";
const READER_HANDOFF_DOMAIN_V2: &[u8] = b"iroha.zk_ams.rns_native.publication.reader_handoff.v2";

/// A digest value is evidence data, not an authority token, and may therefore be copied.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) struct RnsNativeContractDigestV2([u8; 32]);

impl RnsNativeContractDigestV2 {
    const ZERO: Self = Self([0_u8; 32]);

    fn is_zero_v2(self) -> bool {
        self == Self::ZERO
    }

    fn append_to_v2(self, transcript: &mut Vec<u8>) {
        transcript.extend_from_slice(&self.0);
    }
}

impl fmt::Debug for RnsNativeContractDigestV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RnsNativeContractDigestV2(")?;
        for byte in &self.0[..6] {
            write!(formatter, "{byte:02x}")?;
        }
        formatter.write_str("…)")
    }
}

/// Production must implement this with the crate's approved transcript hash.
/// The contract supplies no non-cryptographic production fallback.
#[allow(private_bounds)]
pub(super) trait RnsNativePublicationDigestEngineV2:
    sealed::PublicationDigestEngineV2
{
    fn digest_v2(&mut self, domain: &'static [u8], transcript: &[u8]) -> RnsNativeContractDigestV2;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub(super) enum RnsNativePublicPolynomialRoleV2 {
    A = 0,
    B = 1,
    C0 = 2,
    C1 = 3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct RnsNativeCanonicalPositionV2 {
    role: RnsNativePublicPolynomialRoleV2,
    record_ordinal: Option<usize>,
    limb_ordinal: usize,
}

impl RnsNativeCanonicalPositionV2 {
    fn canonical_ordinal_v2(self) -> Option<usize> {
        if self.limb_ordinal >= RNS_NATIVE_TARGET_LIMB_COUNT_V2 {
            return None;
        }
        match (self.role, self.record_ordinal) {
            (RnsNativePublicPolynomialRoleV2::A, None) => Some(self.limb_ordinal),
            (RnsNativePublicPolynomialRoleV2::B, None) => {
                Some(RNS_NATIVE_TARGET_LIMB_COUNT_V2 + self.limb_ordinal)
            }
            (RnsNativePublicPolynomialRoleV2::C0, Some(record))
                if record < RNS_NATIVE_RECORD_COUNT_V2 =>
            {
                Some(
                    2 * RNS_NATIVE_TARGET_LIMB_COUNT_V2
                        + record * RNS_NATIVE_TARGET_LIMB_COUNT_V2
                        + self.limb_ordinal,
                )
            }
            (RnsNativePublicPolynomialRoleV2::C1, Some(record))
                if record < RNS_NATIVE_RECORD_COUNT_V2 =>
            {
                Some(
                    2 * RNS_NATIVE_TARGET_LIMB_COUNT_V2
                        + RNS_NATIVE_RECORD_COUNT_V2 * RNS_NATIVE_TARGET_LIMB_COUNT_V2
                        + record * RNS_NATIVE_TARGET_LIMB_COUNT_V2
                        + self.limb_ordinal,
                )
            }
            _ => None,
        }
    }

    fn from_canonical_ordinal_v2(ordinal: usize) -> Option<Self> {
        if ordinal < RNS_NATIVE_TARGET_LIMB_COUNT_V2 {
            return Some(Self {
                role: RnsNativePublicPolynomialRoleV2::A,
                record_ordinal: None,
                limb_ordinal: ordinal,
            });
        }
        if ordinal < 2 * RNS_NATIVE_TARGET_LIMB_COUNT_V2 {
            return Some(Self {
                role: RnsNativePublicPolynomialRoleV2::B,
                record_ordinal: None,
                limb_ordinal: ordinal - RNS_NATIVE_TARGET_LIMB_COUNT_V2,
            });
        }
        let c0_end = 2 * RNS_NATIVE_TARGET_LIMB_COUNT_V2
            + RNS_NATIVE_RECORD_COUNT_V2 * RNS_NATIVE_TARGET_LIMB_COUNT_V2;
        if ordinal < c0_end {
            let relative = ordinal - 2 * RNS_NATIVE_TARGET_LIMB_COUNT_V2;
            return Some(Self {
                role: RnsNativePublicPolynomialRoleV2::C0,
                record_ordinal: Some(relative / RNS_NATIVE_TARGET_LIMB_COUNT_V2),
                limb_ordinal: relative % RNS_NATIVE_TARGET_LIMB_COUNT_V2,
            });
        }
        if ordinal < RNS_NATIVE_CANONICAL_DESCRIPTOR_COUNT_V2 {
            let relative = ordinal - c0_end;
            return Some(Self {
                role: RnsNativePublicPolynomialRoleV2::C1,
                record_ordinal: Some(relative / RNS_NATIVE_TARGET_LIMB_COUNT_V2),
                limb_ordinal: relative % RNS_NATIVE_TARGET_LIMB_COUNT_V2,
            });
        }
        None
    }

    fn provider_route_v2(self) -> RnsNativeProviderRouteV2 {
        match self.role {
            RnsNativePublicPolynomialRoleV2::A | RnsNativePublicPolynomialRoleV2::B => {
                RnsNativeProviderRouteV2::CollectiveKey
            }
            RnsNativePublicPolynomialRoleV2::C0 | RnsNativePublicPolynomialRoleV2::C1 => {
                RnsNativeProviderRouteV2::Ciphertext
            }
        }
    }

    fn append_to_v2(self, transcript: &mut Vec<u8>) {
        transcript.push(self.role as u8);
        append_u64_v2(
            transcript,
            self.record_ordinal.map_or(u64::MAX, |value| value as u64),
        );
        append_u64_v2(transcript, self.limb_ordinal as u64);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub(super) enum RnsNativeProviderRouteV2 {
    CollectiveKey = 0,
    Ciphertext = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RnsNativeProviderSnapshotReadbackV2 {
    provider_identity: RnsNativeContractDigestV2,
    snapshot_identity: RnsNativeContractDigestV2,
    readback_identity: RnsNativeContractDigestV2,
}

impl RnsNativeProviderSnapshotReadbackV2 {
    fn validate_v2(
        self,
        component: &'static str,
    ) -> Result<(), RnsNativePublicationAssemblyErrorV2> {
        ensure_nonzero_v2(self.provider_identity, component, "provider_identity", None)?;
        ensure_nonzero_v2(self.snapshot_identity, component, "snapshot_identity", None)?;
        ensure_nonzero_v2(self.readback_identity, component, "readback_identity", None)
    }

    fn append_to_v2(self, transcript: &mut Vec<u8>) {
        self.provider_identity.append_to_v2(transcript);
        self.snapshot_identity.append_to_v2(transcript);
        self.readback_identity.append_to_v2(transcript);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RnsNativeCompositeProviderIdentityV2 {
    collective_key: RnsNativeProviderSnapshotReadbackV2,
    ciphertext: RnsNativeProviderSnapshotReadbackV2,
    composite_identity: RnsNativeContractDigestV2,
}

impl RnsNativeCompositeProviderIdentityV2 {
    fn append_to_v2(self, transcript: &mut Vec<u8>) {
        self.collective_key.append_to_v2(transcript);
        self.ciphertext.append_to_v2(transcript);
        self.composite_identity.append_to_v2(transcript);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RnsNativeFinalizedStreamingAuthorityIdentityV2 {
    governed_context_digest: RnsNativeContractDigestV2,
    collective_key_digest: RnsNativeContractDigestV2,
    streaming_binding_digest: RnsNativeContractDigestV2,
    authority_digest: RnsNativeContractDigestV2,
}

impl RnsNativeFinalizedStreamingAuthorityIdentityV2 {
    fn validate_v2(self) -> Result<(), RnsNativePublicationAssemblyErrorV2> {
        ensure_nonzero_v2(
            self.governed_context_digest,
            "v1_authority",
            "governed_context_digest",
            None,
        )?;
        ensure_nonzero_v2(
            self.collective_key_digest,
            "v1_authority",
            "collective_key_digest",
            None,
        )?;
        ensure_nonzero_v2(
            self.streaming_binding_digest,
            "v1_authority",
            "streaming_binding_digest",
            None,
        )?;
        ensure_nonzero_v2(
            self.authority_digest,
            "v1_authority",
            "authority_digest",
            None,
        )
    }

    fn append_to_v2(self, transcript: &mut Vec<u8>) {
        self.governed_context_digest.append_to_v2(transcript);
        self.collective_key_digest.append_to_v2(transcript);
        self.streaming_binding_digest.append_to_v2(transcript);
        self.authority_digest.append_to_v2(transcript);
    }
}

/// One exact publication receipt and its authenticated readback receipt.
///
/// This type is intentionally non-`Clone` outside tests.  The containing evidence owners
/// retain it; the canonical manifest merely borrows it through an ordinal locator.
#[cfg_attr(test, derive(Clone))]
#[derive(Debug)]
enum RnsNativeExactFinalizedV1AuthorityOwnerV2 {
    #[cfg(test)]
    Fixture,
    #[cfg(test)]
    DropProbe(RnsNativeTestOwnerDropProbeV2),
    Production(Infallible),
}

#[cfg_attr(test, derive(Clone))]
#[derive(Debug)]
enum RnsNativeExactV1CiphertextManifestOwnerV2 {
    #[cfg(test)]
    Fixture,
    #[cfg(test)]
    DropProbe(RnsNativeTestOwnerDropProbeV2),
    Production(Infallible),
}

#[cfg_attr(test, derive(Clone))]
#[derive(Debug)]
enum RnsNativeExactBasisExtensionLifecycleOwnerV2 {
    #[cfg(test)]
    Fixture,
    #[cfg(test)]
    DropProbe(RnsNativeTestOwnerDropProbeV2),
    Production(Infallible),
}

/// Test-only move-owner witness used to prove destruction on every consuming failure path.
#[cfg(test)]
#[derive(Clone, Debug)]
struct RnsNativeTestOwnerDropProbeV2 {
    drops: std::rc::Rc<std::cell::Cell<usize>>,
}

#[cfg(test)]
impl Drop for RnsNativeTestOwnerDropProbeV2 {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}

#[cfg_attr(test, derive(Clone))]
#[derive(Debug)]
struct RnsNativeObjectPublicationEvidenceV2 {
    position: RnsNativeCanonicalPositionV2,
    provider_route: RnsNativeProviderRouteV2,
    provider_binding: RnsNativeProviderSnapshotReadbackV2,
    origin_binding_digest: RnsNativeContractDigestV2,
    object_pointer_digest: RnsNativeContractDigestV2,
    artifact_digest: RnsNativeContractDigestV2,
    publication_receipt_digest: RnsNativeContractDigestV2,
    read_receipt_digest: RnsNativeContractDigestV2,
    readback_artifact_digest: RnsNativeContractDigestV2,
    encoded_byte_count: usize,
}

#[cfg_attr(test, derive(Clone))]
#[derive(Debug)]
struct RnsNativeV1CiphertextManifestEvidenceV2 {
    exact_manifest_owner: RnsNativeExactV1CiphertextManifestOwnerV2,
    record_ordinal: usize,
    sample_index: usize,
    authority_digest: RnsNativeContractDigestV2,
    manifest_digest: RnsNativeContractDigestV2,
    provider_binding: RnsNativeProviderSnapshotReadbackV2,
    /// Physical V1 order: C0[0..38], then C1[0..38].
    prefix_receipts: Box<[RnsNativeObjectPublicationEvidenceV2]>,
}

/// Owns the exact finalized V1 authority evidence and all 3,344 prefix receipts.
#[cfg_attr(test, derive(Clone))]
#[derive(Debug)]
struct RnsNativeFinalizedV1PublicationEvidenceV2 {
    exact_authority_owner: RnsNativeExactFinalizedV1AuthorityOwnerV2,
    authority_identity: RnsNativeFinalizedStreamingAuthorityIdentityV2,
    collective_key_provider: RnsNativeProviderSnapshotReadbackV2,
    ciphertext_provider: RnsNativeProviderSnapshotReadbackV2,
    /// Physical authority order: A[0..38], then B[0..38].
    key_prefix_receipts: Box<[RnsNativeObjectPublicationEvidenceV2]>,
    ciphertext_manifests: Box<[RnsNativeV1CiphertextManifestEvidenceV2]>,
}

#[cfg_attr(test, derive(Clone))]
#[derive(Debug)]
struct RnsNativeV2TailRecordEvidenceV2 {
    record_ordinal: usize,
    sample_index: usize,
    v1_manifest_digest: RnsNativeContractDigestV2,
    lifecycle_digest: RnsNativeContractDigestV2,
    completion_digest: RnsNativeContractDigestV2,
    /// Physical synchronous-callback order: C0[38], C0[39], C1[38], C1[39].
    tail_receipts: Box<[RnsNativeObjectPublicationEvidenceV2]>,
}

/// Owns the exact four key tails and 172 record-local ciphertext tails.
#[cfg_attr(test, derive(Clone))]
#[derive(Debug)]
struct RnsNativeBasisExtensionTailLifecycleEvidenceV2 {
    exact_lifecycle_owner: RnsNativeExactBasisExtensionLifecycleOwnerV2,
    authority_identity: RnsNativeFinalizedStreamingAuthorityIdentityV2,
    collective_key_provider: RnsNativeProviderSnapshotReadbackV2,
    ciphertext_provider: RnsNativeProviderSnapshotReadbackV2,
    key_tail_owner_digest: RnsNativeContractDigestV2,
    lifecycle_digest: RnsNativeContractDigestV2,
    /// Physical key-tail order: A[38], A[39], B[38], B[39].
    key_tail_receipts: Box<[RnsNativeObjectPublicationEvidenceV2]>,
    records: Box<[RnsNativeV2TailRecordEvidenceV2]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RnsNativePublicPolynomialDescriptorContractV2 {
    position: RnsNativeCanonicalPositionV2,
    provider_route: RnsNativeProviderRouteV2,
    object_pointer_digest: RnsNativeContractDigestV2,
    artifact_digest: RnsNativeContractDigestV2,
    publication_receipt_digest: RnsNativeContractDigestV2,
    read_receipt_digest: RnsNativeContractDigestV2,
}

impl RnsNativePublicPolynomialDescriptorContractV2 {
    fn from_receipt_v2(receipt: &RnsNativeObjectPublicationEvidenceV2) -> Self {
        Self {
            position: receipt.position,
            provider_route: receipt.provider_route,
            object_pointer_digest: receipt.object_pointer_digest,
            artifact_digest: receipt.artifact_digest,
            publication_receipt_digest: receipt.publication_receipt_digest,
            read_receipt_digest: receipt.read_receipt_digest,
        }
    }

    fn append_to_v2(self, transcript: &mut Vec<u8>) {
        self.position.append_to_v2(transcript);
        transcript.push(self.provider_route as u8);
        self.object_pointer_digest.append_to_v2(transcript);
        self.artifact_digest.append_to_v2(transcript);
        self.publication_receipt_digest.append_to_v2(transcript);
        self.read_receipt_digest.append_to_v2(transcript);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RnsNativePublicationAssemblyErrorV2 {
    Count {
        component: &'static str,
        expected: usize,
        actual: usize,
    },
    InvalidCanonicalPosition {
        ordinal: usize,
    },
    PositionMismatch {
        expected: RnsNativeCanonicalPositionV2,
        actual: RnsNativeCanonicalPositionV2,
    },
    ProviderRouteMismatch {
        position: RnsNativeCanonicalPositionV2,
        expected: RnsNativeProviderRouteV2,
        actual: RnsNativeProviderRouteV2,
    },
    ProviderBindingMismatch {
        component: &'static str,
        position: Option<RnsNativeCanonicalPositionV2>,
    },
    AuthorityIdentityMismatch,
    AuthorityBindingMismatch {
        component: &'static str,
        record_ordinal: Option<usize>,
    },
    RecordOrder {
        component: &'static str,
        expected: usize,
        actual: usize,
    },
    SampleOrder {
        component: &'static str,
        record_ordinal: usize,
        expected: usize,
        actual: usize,
    },
    OriginBindingMismatch {
        position: RnsNativeCanonicalPositionV2,
    },
    ZeroDigest {
        component: &'static str,
        field: &'static str,
        position: Option<RnsNativeCanonicalPositionV2>,
    },
    ReadbackArtifactMismatch {
        position: RnsNativeCanonicalPositionV2,
    },
    EncodedByteCount {
        position: RnsNativeCanonicalPositionV2,
        expected: usize,
        actual: usize,
    },
    DuplicateObjectPointer {
        position: RnsNativeCanonicalPositionV2,
    },
    DuplicateArtifact {
        position: RnsNativeCanonicalPositionV2,
    },
    DuplicatePublicationReceipt {
        position: RnsNativeCanonicalPositionV2,
    },
    DuplicateReadReceipt {
        position: RnsNativeCanonicalPositionV2,
    },
    DuplicateV1ManifestDigest {
        record_ordinal: usize,
    },
    DuplicateV2CompletionDigest {
        record_ordinal: usize,
    },
    DigestEngineReturnedZero {
        component: &'static str,
    },
    HandoffProviderMismatch,
}

/// Consuming assembler: failed validation drops both move-only evidence owners and mints
/// neither a canonical manifest nor a reader handoff.
pub(super) struct RnsNativePublicPolynomialPublicationAssemblerV2 {
    v1: RnsNativeFinalizedV1PublicationEvidenceV2,
    tails: RnsNativeBasisExtensionTailLifecycleEvidenceV2,
}

impl RnsNativePublicPolynomialPublicationAssemblerV2 {
    fn from_contract_evidence_v2(
        v1: RnsNativeFinalizedV1PublicationEvidenceV2,
        tails: RnsNativeBasisExtensionTailLifecycleEvidenceV2,
    ) -> Self {
        Self { v1, tails }
    }

    pub(super) fn assemble_v2<H: RnsNativePublicationDigestEngineV2>(
        self,
        digest_engine: &mut H,
    ) -> Result<RnsNativePublicPolynomialPublishedSetV2, RnsNativePublicationAssemblyErrorV2> {
        validate_owner_shapes_v2(&self.v1, &self.tails)?;

        let composite_provider = composite_provider_identity_v2(
            self.v1.collective_key_provider,
            self.v1.ciphertext_provider,
            digest_engine,
        )?;

        let mut pointers = BTreeSet::new();
        let mut artifacts = BTreeSet::new();
        let mut publication_receipts = BTreeSet::new();
        let mut read_receipts = BTreeSet::new();
        let mut descriptors = Vec::with_capacity(RNS_NATIVE_CANONICAL_DESCRIPTOR_COUNT_V2);

        for ordinal in 0..RNS_NATIVE_CANONICAL_DESCRIPTOR_COUNT_V2 {
            let expected = RnsNativeCanonicalPositionV2::from_canonical_ordinal_v2(ordinal)
                .ok_or(RnsNativePublicationAssemblyErrorV2::InvalidCanonicalPosition { ordinal })?;
            let (receipt, origin) = receipt_and_origin_at_v2(&self.v1, &self.tails, expected);
            let expected_binding = match expected.provider_route_v2() {
                RnsNativeProviderRouteV2::CollectiveKey => self.v1.collective_key_provider,
                RnsNativeProviderRouteV2::Ciphertext => self.v1.ciphertext_provider,
            };
            validate_receipt_v2(receipt, expected, expected_binding, origin)?;
            if !pointers.insert(receipt.object_pointer_digest) {
                return Err(
                    RnsNativePublicationAssemblyErrorV2::DuplicateObjectPointer {
                        position: expected,
                    },
                );
            }
            if !artifacts.insert(receipt.artifact_digest) {
                return Err(RnsNativePublicationAssemblyErrorV2::DuplicateArtifact {
                    position: expected,
                });
            }
            if !publication_receipts.insert(receipt.publication_receipt_digest) {
                return Err(
                    RnsNativePublicationAssemblyErrorV2::DuplicatePublicationReceipt {
                        position: expected,
                    },
                );
            }
            if !read_receipts.insert(receipt.read_receipt_digest) {
                return Err(RnsNativePublicationAssemblyErrorV2::DuplicateReadReceipt {
                    position: expected,
                });
            }
            descriptors
                .push(RnsNativePublicPolynomialDescriptorContractV2::from_receipt_v2(receipt));
        }

        ensure_count_v2(
            "canonical_descriptors",
            RNS_NATIVE_CANONICAL_DESCRIPTOR_COUNT_V2,
            descriptors.len(),
        )?;

        let canonical_manifest_digest = canonical_manifest_digest_v2(
            RnsNativeCanonicalManifestInputsV2 {
                authority: self.v1.authority_identity,
                key_tail_owner_digest: self.tails.key_tail_owner_digest,
                lifecycle_digest: self.tails.lifecycle_digest,
                composite_provider,
                v1_manifests: &self.v1.ciphertext_manifests,
                tail_records: &self.tails.records,
                descriptors: &descriptors,
            },
            digest_engine,
        )?;
        let reader_handoff_digest =
            reader_handoff_digest_v2(canonical_manifest_digest, composite_provider, digest_engine)?;

        Ok(RnsNativePublicPolynomialPublishedSetV2 {
            v1: self.v1,
            tails: self.tails,
            descriptors: descriptors.into_boxed_slice(),
            composite_provider,
            canonical_manifest_digest,
            reader_handoff_digest,
        })
    }
}

/// Move-only owner of all original receipts plus their canonical descriptor view.
pub(super) struct RnsNativePublicPolynomialPublishedSetV2 {
    v1: RnsNativeFinalizedV1PublicationEvidenceV2,
    tails: RnsNativeBasisExtensionTailLifecycleEvidenceV2,
    descriptors: Box<[RnsNativePublicPolynomialDescriptorContractV2]>,
    composite_provider: RnsNativeCompositeProviderIdentityV2,
    canonical_manifest_digest: RnsNativeContractDigestV2,
    reader_handoff_digest: RnsNativeContractDigestV2,
}

impl RnsNativePublicPolynomialPublishedSetV2 {
    pub(super) fn descriptors_v2(&self) -> &[RnsNativePublicPolynomialDescriptorContractV2] {
        &self.descriptors
    }

    pub(super) fn canonical_manifest_digest_v2(&self) -> RnsNativeContractDigestV2 {
        self.canonical_manifest_digest
    }

    pub(super) fn reader_handoff_digest_v2(&self) -> RnsNativeContractDigestV2 {
        self.reader_handoff_digest
    }

    fn receipt_at_v2(
        &self,
        canonical_ordinal: usize,
    ) -> Option<&RnsNativeObjectPublicationEvidenceV2> {
        let position = RnsNativeCanonicalPositionV2::from_canonical_ordinal_v2(canonical_ordinal)?;
        Some(receipt_and_origin_at_v2(&self.v1, &self.tails, position).0)
    }

    pub(super) fn into_reader_handoff_v2<P: RnsNativeCompositeReadProviderV2>(
        self,
        provider: P,
    ) -> Result<RnsNativePublicPolynomialReaderHandoffV2<P>, RnsNativeReaderHandoffFailureV2<P>>
    {
        if provider.composite_provider_identity_v2() != self.composite_provider {
            return Err(RnsNativeReaderHandoffFailureV2 {
                error: RnsNativePublicationAssemblyErrorV2::HandoffProviderMismatch,
                published: Box::new(self),
                provider,
            });
        }
        Ok(RnsNativePublicPolynomialReaderHandoffV2 {
            published: self,
            provider,
        })
    }
}

/// The future composite provider routes A/B to the collective-key provider and C0/C1 to
/// the ciphertext provider.  Its identity must bind both immutable snapshots/readbacks.
#[allow(private_bounds)]
pub(super) trait RnsNativeCompositeReadProviderV2: sealed::CompositeReadProviderV2 {
    type Error;

    fn composite_provider_identity_v2(&self) -> RnsNativeCompositeProviderIdentityV2;

    fn read_exact_at_v2(
        &mut self,
        route: RnsNativeProviderRouteV2,
        object_pointer_digest: RnsNativeContractDigestV2,
        byte_offset: usize,
        destination: &mut [u8],
    ) -> Result<(), Self::Error>;
}

/// A failed handoff owns and destroys both inputs; it deliberately offers no recovery API.
pub(super) struct RnsNativeReaderHandoffFailureV2<P> {
    error: RnsNativePublicationAssemblyErrorV2,
    published: Box<RnsNativePublicPolynomialPublishedSetV2>,
    provider: P,
}

impl<P> RnsNativeReaderHandoffFailureV2<P> {
    pub(super) fn error_v2(&self) -> RnsNativePublicationAssemblyErrorV2 {
        self.error
    }
}

/// Move-only capability presented to the existing reader adapter.
pub(super) struct RnsNativePublicPolynomialReaderHandoffV2<P> {
    published: RnsNativePublicPolynomialPublishedSetV2,
    provider: P,
}

pub(super) struct RnsNativeExistingReaderBuildRequestV2<'a> {
    descriptors: &'a [RnsNativePublicPolynomialDescriptorContractV2],
    receipt_locator: RnsNativeCanonicalReceiptLocatorV2<'a>,
    composite_provider: RnsNativeCompositeProviderIdentityV2,
    canonical_manifest_digest: RnsNativeContractDigestV2,
    reader_handoff_digest: RnsNativeContractDigestV2,
}

mod sealed {
    pub trait PublicationDigestEngineV2 {}
    pub trait CompositeReadProviderV2 {}
    pub trait ExistingReaderAdapterV2 {}
}

/// A live implementation must construct the existing reader from exactly this descriptor
/// slice and move the provider into it.  On error it must return the provider so the
/// fail-closed owner can destroy all evidence together.
#[allow(private_bounds)]
pub(super) trait RnsNativeExistingPublicReaderAdapterV2<P>:
    sealed::ExistingReaderAdapterV2
{
    type Reader;
    type Error;

    fn try_build_existing_reader_v2(
        self,
        provider: P,
        request: RnsNativeExistingReaderBuildRequestV2<'_>,
    ) -> Result<Self::Reader, (Self::Error, P)>;
}

pub(super) struct RnsNativeIntegratedPublicReaderCapabilityV2<R> {
    reader: R,
    published: RnsNativePublicPolynomialPublishedSetV2,
}

impl<R> RnsNativeIntegratedPublicReaderCapabilityV2<R> {
    pub(super) fn reader_mut_v2(&mut self) -> &mut R {
        &mut self.reader
    }

    pub(super) fn canonical_manifest_digest_v2(&self) -> RnsNativeContractDigestV2 {
        self.published.canonical_manifest_digest
    }
}

pub(super) struct RnsNativeExistingReaderBuildFailureV2<E, P> {
    error: E,
    published: Box<RnsNativePublicPolynomialPublishedSetV2>,
    provider: P,
}

impl<P> RnsNativePublicPolynomialReaderHandoffV2<P> {
    pub(super) fn try_into_existing_reader_v2<A>(
        self,
        adapter: A,
    ) -> Result<
        RnsNativeIntegratedPublicReaderCapabilityV2<A::Reader>,
        RnsNativeExistingReaderBuildFailureV2<A::Error, P>,
    >
    where
        A: RnsNativeExistingPublicReaderAdapterV2<P>,
    {
        let Self {
            published,
            provider,
        } = self;
        let request = RnsNativeExistingReaderBuildRequestV2 {
            descriptors: &published.descriptors,
            receipt_locator: RnsNativeCanonicalReceiptLocatorV2 {
                v1: &published.v1,
                tails: &published.tails,
            },
            composite_provider: published.composite_provider,
            canonical_manifest_digest: published.canonical_manifest_digest,
            reader_handoff_digest: published.reader_handoff_digest,
        };
        match adapter.try_build_existing_reader_v2(provider, request) {
            Ok(reader) => Ok(RnsNativeIntegratedPublicReaderCapabilityV2 { reader, published }),
            Err((error, provider)) => Err(RnsNativeExistingReaderBuildFailureV2 {
                error,
                published: Box::new(published),
                provider,
            }),
        }
    }
}

#[derive(Clone, Copy)]
struct RnsNativeCanonicalReceiptLocatorV2<'a> {
    v1: &'a RnsNativeFinalizedV1PublicationEvidenceV2,
    tails: &'a RnsNativeBasisExtensionTailLifecycleEvidenceV2,
}

impl<'a> RnsNativeCanonicalReceiptLocatorV2<'a> {
    fn receipt_at_v2(
        self,
        canonical_ordinal: usize,
    ) -> Option<&'a RnsNativeObjectPublicationEvidenceV2> {
        let position = RnsNativeCanonicalPositionV2::from_canonical_ordinal_v2(canonical_ordinal)?;
        Some(receipt_and_origin_at_v2(self.v1, self.tails, position).0)
    }
}

fn validate_owner_shapes_v2(
    v1: &RnsNativeFinalizedV1PublicationEvidenceV2,
    tails: &RnsNativeBasisExtensionTailLifecycleEvidenceV2,
) -> Result<(), RnsNativePublicationAssemblyErrorV2> {
    v1.authority_identity.validate_v2()?;
    v1.collective_key_provider
        .validate_v2("v1_collective_key_provider")?;
    v1.ciphertext_provider
        .validate_v2("v1_ciphertext_provider")?;
    ensure_count_v2(
        "v1_key_prefix_receipts",
        RNS_NATIVE_KEY_PREFIX_RECEIPT_COUNT_V2,
        v1.key_prefix_receipts.len(),
    )?;
    ensure_count_v2(
        "v1_ciphertext_manifests",
        RNS_NATIVE_RECORD_COUNT_V2,
        v1.ciphertext_manifests.len(),
    )?;

    if tails.authority_identity != v1.authority_identity {
        return Err(RnsNativePublicationAssemblyErrorV2::AuthorityIdentityMismatch);
    }
    if tails.collective_key_provider != v1.collective_key_provider {
        return Err(
            RnsNativePublicationAssemblyErrorV2::ProviderBindingMismatch {
                component: "tail_collective_key_provider",
                position: None,
            },
        );
    }
    if tails.ciphertext_provider != v1.ciphertext_provider {
        return Err(
            RnsNativePublicationAssemblyErrorV2::ProviderBindingMismatch {
                component: "tail_ciphertext_provider",
                position: None,
            },
        );
    }
    ensure_nonzero_v2(
        tails.key_tail_owner_digest,
        "v2_tails",
        "key_tail_owner_digest",
        None,
    )?;
    ensure_nonzero_v2(tails.lifecycle_digest, "v2_tails", "lifecycle_digest", None)?;
    ensure_count_v2(
        "v2_key_tail_receipts",
        RNS_NATIVE_KEY_TAIL_RECEIPT_COUNT_V2,
        tails.key_tail_receipts.len(),
    )?;
    ensure_count_v2(
        "v2_tail_records",
        RNS_NATIVE_RECORD_COUNT_V2,
        tails.records.len(),
    )?;

    // These are record-owner roots, not reusable labels.  One set deliberately spans
    // both phases so a V2 completion cannot alias any V1 manifest (or vice versa).
    let mut record_owner_digests = BTreeSet::new();
    for record_ordinal in 0..RNS_NATIVE_RECORD_COUNT_V2 {
        let manifest = &v1.ciphertext_manifests[record_ordinal];
        if manifest.record_ordinal != record_ordinal {
            return Err(RnsNativePublicationAssemblyErrorV2::RecordOrder {
                component: "v1_ciphertext_manifest",
                expected: record_ordinal,
                actual: manifest.record_ordinal,
            });
        }
        if manifest.sample_index != record_ordinal {
            return Err(RnsNativePublicationAssemblyErrorV2::SampleOrder {
                component: "v1_ciphertext_manifest",
                record_ordinal,
                expected: record_ordinal,
                actual: manifest.sample_index,
            });
        }
        if manifest.authority_digest != v1.authority_identity.authority_digest {
            return Err(
                RnsNativePublicationAssemblyErrorV2::AuthorityBindingMismatch {
                    component: "v1_ciphertext_manifest",
                    record_ordinal: Some(record_ordinal),
                },
            );
        }
        ensure_nonzero_v2(
            manifest.manifest_digest,
            "v1_ciphertext_manifest",
            "manifest_digest",
            None,
        )?;
        if !record_owner_digests.insert(manifest.manifest_digest) {
            return Err(
                RnsNativePublicationAssemblyErrorV2::DuplicateV1ManifestDigest { record_ordinal },
            );
        }
        if manifest.provider_binding != v1.ciphertext_provider {
            return Err(
                RnsNativePublicationAssemblyErrorV2::ProviderBindingMismatch {
                    component: "v1_ciphertext_manifest",
                    position: None,
                },
            );
        }
        ensure_count_v2(
            "v1_ciphertext_prefix_receipts_per_manifest",
            2 * RNS_NATIVE_LEGACY_LIMB_COUNT_V2,
            manifest.prefix_receipts.len(),
        )?;

        let tail_record = &tails.records[record_ordinal];
        if tail_record.record_ordinal != record_ordinal {
            return Err(RnsNativePublicationAssemblyErrorV2::RecordOrder {
                component: "v2_tail_record",
                expected: record_ordinal,
                actual: tail_record.record_ordinal,
            });
        }
        if tail_record.sample_index != record_ordinal {
            return Err(RnsNativePublicationAssemblyErrorV2::SampleOrder {
                component: "v2_tail_record",
                record_ordinal,
                expected: record_ordinal,
                actual: tail_record.sample_index,
            });
        }
        if tail_record.v1_manifest_digest != manifest.manifest_digest {
            return Err(
                RnsNativePublicationAssemblyErrorV2::AuthorityBindingMismatch {
                    component: "v2_tail_record_to_v1_manifest",
                    record_ordinal: Some(record_ordinal),
                },
            );
        }
        if tail_record.lifecycle_digest != tails.lifecycle_digest {
            return Err(
                RnsNativePublicationAssemblyErrorV2::AuthorityBindingMismatch {
                    component: "v2_tail_record_to_lifecycle",
                    record_ordinal: Some(record_ordinal),
                },
            );
        }
        ensure_nonzero_v2(
            tail_record.completion_digest,
            "v2_tail_record",
            "completion_digest",
            None,
        )?;
        if !record_owner_digests.insert(tail_record.completion_digest) {
            return Err(
                RnsNativePublicationAssemblyErrorV2::DuplicateV2CompletionDigest { record_ordinal },
            );
        }
        ensure_count_v2(
            "v2_ciphertext_tail_receipts_per_record",
            4,
            tail_record.tail_receipts.len(),
        )?;
    }

    Ok(())
}

fn receipt_and_origin_at_v2<'a>(
    v1: &'a RnsNativeFinalizedV1PublicationEvidenceV2,
    tails: &'a RnsNativeBasisExtensionTailLifecycleEvidenceV2,
    position: RnsNativeCanonicalPositionV2,
) -> (
    &'a RnsNativeObjectPublicationEvidenceV2,
    RnsNativeContractDigestV2,
) {
    match position.role {
        RnsNativePublicPolynomialRoleV2::A => {
            if position.limb_ordinal < RNS_NATIVE_LEGACY_LIMB_COUNT_V2 {
                (
                    &v1.key_prefix_receipts[position.limb_ordinal],
                    v1.authority_identity.authority_digest,
                )
            } else {
                (
                    &tails.key_tail_receipts
                        [position.limb_ordinal - RNS_NATIVE_LEGACY_LIMB_COUNT_V2],
                    tails.key_tail_owner_digest,
                )
            }
        }
        RnsNativePublicPolynomialRoleV2::B => {
            if position.limb_ordinal < RNS_NATIVE_LEGACY_LIMB_COUNT_V2 {
                (
                    &v1.key_prefix_receipts
                        [RNS_NATIVE_LEGACY_LIMB_COUNT_V2 + position.limb_ordinal],
                    v1.authority_identity.authority_digest,
                )
            } else {
                (
                    &tails.key_tail_receipts
                        [2 + position.limb_ordinal - RNS_NATIVE_LEGACY_LIMB_COUNT_V2],
                    tails.key_tail_owner_digest,
                )
            }
        }
        RnsNativePublicPolynomialRoleV2::C0 => {
            let record = position
                .record_ordinal
                .expect("validated canonical C0 position");
            if position.limb_ordinal < RNS_NATIVE_LEGACY_LIMB_COUNT_V2 {
                (
                    &v1.ciphertext_manifests[record].prefix_receipts[position.limb_ordinal],
                    v1.ciphertext_manifests[record].manifest_digest,
                )
            } else {
                (
                    &tails.records[record].tail_receipts
                        [position.limb_ordinal - RNS_NATIVE_LEGACY_LIMB_COUNT_V2],
                    tails.records[record].completion_digest,
                )
            }
        }
        RnsNativePublicPolynomialRoleV2::C1 => {
            let record = position
                .record_ordinal
                .expect("validated canonical C1 position");
            if position.limb_ordinal < RNS_NATIVE_LEGACY_LIMB_COUNT_V2 {
                (
                    &v1.ciphertext_manifests[record].prefix_receipts
                        [RNS_NATIVE_LEGACY_LIMB_COUNT_V2 + position.limb_ordinal],
                    v1.ciphertext_manifests[record].manifest_digest,
                )
            } else {
                (
                    &tails.records[record].tail_receipts
                        [2 + position.limb_ordinal - RNS_NATIVE_LEGACY_LIMB_COUNT_V2],
                    tails.records[record].completion_digest,
                )
            }
        }
    }
}

fn validate_receipt_v2(
    receipt: &RnsNativeObjectPublicationEvidenceV2,
    expected_position: RnsNativeCanonicalPositionV2,
    expected_provider: RnsNativeProviderSnapshotReadbackV2,
    expected_origin: RnsNativeContractDigestV2,
) -> Result<(), RnsNativePublicationAssemblyErrorV2> {
    if receipt.position != expected_position {
        return Err(RnsNativePublicationAssemblyErrorV2::PositionMismatch {
            expected: expected_position,
            actual: receipt.position,
        });
    }
    let expected_route = expected_position.provider_route_v2();
    if receipt.provider_route != expected_route {
        return Err(RnsNativePublicationAssemblyErrorV2::ProviderRouteMismatch {
            position: expected_position,
            expected: expected_route,
            actual: receipt.provider_route,
        });
    }
    if receipt.provider_binding != expected_provider {
        return Err(
            RnsNativePublicationAssemblyErrorV2::ProviderBindingMismatch {
                component: "object_receipt",
                position: Some(expected_position),
            },
        );
    }
    if receipt.origin_binding_digest != expected_origin {
        return Err(RnsNativePublicationAssemblyErrorV2::OriginBindingMismatch {
            position: expected_position,
        });
    }
    ensure_nonzero_v2(
        receipt.object_pointer_digest,
        "object_receipt",
        "object_pointer_digest",
        Some(expected_position),
    )?;
    ensure_nonzero_v2(
        receipt.artifact_digest,
        "object_receipt",
        "artifact_digest",
        Some(expected_position),
    )?;
    ensure_nonzero_v2(
        receipt.publication_receipt_digest,
        "object_receipt",
        "publication_receipt_digest",
        Some(expected_position),
    )?;
    ensure_nonzero_v2(
        receipt.read_receipt_digest,
        "object_receipt",
        "read_receipt_digest",
        Some(expected_position),
    )?;
    ensure_nonzero_v2(
        receipt.readback_artifact_digest,
        "object_receipt",
        "readback_artifact_digest",
        Some(expected_position),
    )?;
    if receipt.readback_artifact_digest != receipt.artifact_digest {
        return Err(
            RnsNativePublicationAssemblyErrorV2::ReadbackArtifactMismatch {
                position: expected_position,
            },
        );
    }
    if receipt.encoded_byte_count != RNS_NATIVE_OBJECT_ENCODED_BYTE_COUNT_V2 {
        return Err(RnsNativePublicationAssemblyErrorV2::EncodedByteCount {
            position: expected_position,
            expected: RNS_NATIVE_OBJECT_ENCODED_BYTE_COUNT_V2,
            actual: receipt.encoded_byte_count,
        });
    }
    Ok(())
}

fn composite_provider_identity_v2<H: RnsNativePublicationDigestEngineV2>(
    collective_key: RnsNativeProviderSnapshotReadbackV2,
    ciphertext: RnsNativeProviderSnapshotReadbackV2,
    digest_engine: &mut H,
) -> Result<RnsNativeCompositeProviderIdentityV2, RnsNativePublicationAssemblyErrorV2> {
    collective_key.validate_v2("collective_key_provider")?;
    ciphertext.validate_v2("ciphertext_provider")?;
    let mut transcript = Vec::with_capacity(6 * 32 + 16);
    append_u64_v2(&mut transcript, 2);
    collective_key.append_to_v2(&mut transcript);
    ciphertext.append_to_v2(&mut transcript);
    let composite_identity = digest_engine.digest_v2(COMPOSITE_PROVIDER_DOMAIN_V2, &transcript);
    if composite_identity.is_zero_v2() {
        return Err(
            RnsNativePublicationAssemblyErrorV2::DigestEngineReturnedZero {
                component: "composite_provider_identity",
            },
        );
    }
    Ok(RnsNativeCompositeProviderIdentityV2 {
        collective_key,
        ciphertext,
        composite_identity,
    })
}

struct RnsNativeCanonicalManifestInputsV2<'a> {
    authority: RnsNativeFinalizedStreamingAuthorityIdentityV2,
    key_tail_owner_digest: RnsNativeContractDigestV2,
    lifecycle_digest: RnsNativeContractDigestV2,
    composite_provider: RnsNativeCompositeProviderIdentityV2,
    v1_manifests: &'a [RnsNativeV1CiphertextManifestEvidenceV2],
    tail_records: &'a [RnsNativeV2TailRecordEvidenceV2],
    descriptors: &'a [RnsNativePublicPolynomialDescriptorContractV2],
}

fn canonical_manifest_digest_v2<H: RnsNativePublicationDigestEngineV2>(
    inputs: RnsNativeCanonicalManifestInputsV2<'_>,
    digest_engine: &mut H,
) -> Result<RnsNativeContractDigestV2, RnsNativePublicationAssemblyErrorV2> {
    let RnsNativeCanonicalManifestInputsV2 {
        authority,
        key_tail_owner_digest,
        lifecycle_digest,
        composite_provider,
        v1_manifests,
        tail_records,
        descriptors,
    } = inputs;
    let mut transcript = Vec::with_capacity(320 + descriptors.len() * 145);
    append_u64_v2(&mut transcript, 2);
    append_u64_v2(&mut transcript, RNS_NATIVE_LEGACY_LIMB_COUNT_V2 as u64);
    append_u64_v2(&mut transcript, RNS_NATIVE_TARGET_LIMB_COUNT_V2 as u64);
    append_u64_v2(&mut transcript, RNS_NATIVE_RECORD_COUNT_V2 as u64);
    append_u64_v2(&mut transcript, descriptors.len() as u64);
    append_u64_v2(&mut transcript, RNS_NATIVE_PREFIX_RECEIPT_COUNT_V2 as u64);
    append_u64_v2(&mut transcript, RNS_NATIVE_TAIL_RECEIPT_COUNT_V2 as u64);
    append_u64_v2(
        &mut transcript,
        RNS_NATIVE_OBJECT_ENCODED_BYTE_COUNT_V2 as u64,
    );
    authority.append_to_v2(&mut transcript);
    key_tail_owner_digest.append_to_v2(&mut transcript);
    lifecycle_digest.append_to_v2(&mut transcript);
    composite_provider.append_to_v2(&mut transcript);
    append_u64_v2(&mut transcript, v1_manifests.len() as u64);
    for (manifest, tail_record) in v1_manifests.iter().zip(tail_records) {
        append_u64_v2(&mut transcript, manifest.record_ordinal as u64);
        append_u64_v2(&mut transcript, manifest.sample_index as u64);
        manifest.manifest_digest.append_to_v2(&mut transcript);
        tail_record.completion_digest.append_to_v2(&mut transcript);
    }
    for descriptor in descriptors {
        descriptor.append_to_v2(&mut transcript);
    }
    let digest = digest_engine.digest_v2(CANONICAL_MANIFEST_DOMAIN_V2, &transcript);
    if digest.is_zero_v2() {
        return Err(
            RnsNativePublicationAssemblyErrorV2::DigestEngineReturnedZero {
                component: "canonical_manifest_digest",
            },
        );
    }
    Ok(digest)
}

fn reader_handoff_digest_v2<H: RnsNativePublicationDigestEngineV2>(
    canonical_manifest_digest: RnsNativeContractDigestV2,
    composite_provider: RnsNativeCompositeProviderIdentityV2,
    digest_engine: &mut H,
) -> Result<RnsNativeContractDigestV2, RnsNativePublicationAssemblyErrorV2> {
    let mut transcript = Vec::with_capacity(256);
    append_u64_v2(&mut transcript, 2);
    append_u64_v2(
        &mut transcript,
        RNS_NATIVE_CANONICAL_DESCRIPTOR_COUNT_V2 as u64,
    );
    canonical_manifest_digest.append_to_v2(&mut transcript);
    composite_provider.append_to_v2(&mut transcript);
    let digest = digest_engine.digest_v2(READER_HANDOFF_DOMAIN_V2, &transcript);
    if digest.is_zero_v2() {
        return Err(
            RnsNativePublicationAssemblyErrorV2::DigestEngineReturnedZero {
                component: "reader_handoff_digest",
            },
        );
    }
    Ok(digest)
}

fn ensure_count_v2(
    component: &'static str,
    expected: usize,
    actual: usize,
) -> Result<(), RnsNativePublicationAssemblyErrorV2> {
    if actual == expected {
        Ok(())
    } else {
        Err(RnsNativePublicationAssemblyErrorV2::Count {
            component,
            expected,
            actual,
        })
    }
}

fn ensure_nonzero_v2(
    digest: RnsNativeContractDigestV2,
    component: &'static str,
    field: &'static str,
    position: Option<RnsNativeCanonicalPositionV2>,
) -> Result<(), RnsNativePublicationAssemblyErrorV2> {
    if digest.is_zero_v2() {
        Err(RnsNativePublicationAssemblyErrorV2::ZeroDigest {
            component,
            field,
            position,
        })
    } else {
        Ok(())
    }
}

fn append_u64_v2(transcript: &mut Vec<u8>, value: u64) {
    transcript.extend_from_slice(&value.to_be_bytes());
}

/// These adapters cannot be constructed.  Replacing each with a consuming live adapter is
/// an explicit integration step and must not be inferred from digest equality.
pub(super) struct RnsNativeFinalizedV1ProductionAdapterV2 {
    never: Infallible,
}

pub(super) struct RnsNativeBasisExtensionLifecycleProductionAdapterV2 {
    never: Infallible,
}

pub(super) struct RnsNativeCompositeProviderProductionAdapterV2 {
    never: Infallible,
}

pub(super) struct RnsNativeExistingReaderProductionAdapterV2 {
    never: Infallible,
}

pub(super) struct RnsNativeApprovedDigestProductionAdapterV2 {
    never: Infallible,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RnsNativePublicationAssemblerBlockerV2 {
    code: &'static str,
    required_delta: &'static str,
}

/// Exact minimal live-tree deltas required after this contract is reviewed.
pub(super) const RNS_NATIVE_PUBLICATION_ASSEMBLER_BLOCKERS_V2:
    &[RnsNativePublicationAssemblerBlockerV2] = &[
    RnsNativePublicationAssemblerBlockerV2 {
        code: "CONSUME_FINALIZED_V1_OWNER",
        required_delta: "add one parent-private consuming adapter exposing the exact V1 authority identity, 76 key prefix receipts, and 43 ordered ciphertext manifest owners without cloning receipts",
    },
    RnsNativePublicationAssemblerBlockerV2 {
        code: "CONSUME_V2_TAIL_LIFECYCLE",
        required_delta: "add one parent-private consuming adapter for the completed basis-extension lifecycle owner containing exactly four key and 172 ciphertext tail receipts",
    },
    RnsNativePublicationAssemblerBlockerV2 {
        code: "APPROVED_TRANSCRIPT_HASH",
        required_delta: "implement the digest-engine seam with the crate-approved domain-separated transcript hash; the deterministic fixture hash is cfg(test)-only",
    },
    RnsNativePublicationAssemblerBlockerV2 {
        code: "COMPOSITE_READ_PROVIDER",
        required_delta: "implement a move-only provider routing A/B to the key snapshot and C0/C1 to the ciphertext snapshot while reporting the composite identity bound here",
    },
    RnsNativePublicationAssemblerBlockerV2 {
        code: "EXISTING_READER_ADAPTER",
        required_delta: "bind the existing parent-private reader entry path to this sealed consuming adapter seam and retain the published evidence beside the reader; no reader visibility or public/untyped constructor change is required",
    },
    RnsNativePublicationAssemblerBlockerV2 {
        code: "PHASE23_SOURCE_ALGEBRA_UNINHABITED",
        required_delta: "the private move-only Phase23 context owner and external-source topology now compile continuously, but the production source-algebra adapters and seals remain uninhabited",
    },
];

#[cfg(test)]
#[path = "incremental_source_rns_native_publication_assembler_v2_tests.rs"]
mod tests;
