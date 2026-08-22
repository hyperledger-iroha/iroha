//! Sealed pre-transcript public-statement coordinator.
//!
//! This source-only child retains the exact confidential snapshot, complete
//! publication/reader bridge, all 3,520 individual public artifact identities,
//! and all nonce-derived record facts in one move-only owner.  Its sole public
//! transition consumes that owner to start the transcript and returns a
//! distinct started owner beside it.  A sealed child now consumes that owner
//! through the source-only claimed-qPCS/source-preflight chronology, but there
//! remains no live production correspondence or source-preflight integration,
//! raw-parts escape, readiness evidence, or release authority.

#![allow(
    dead_code,
    reason = "the sealed source-only coordinator awaits live correspondence and repeat-read conformance"
)]

use core::{convert::Infallible, fmt};

use super::super::super::super::{
    direct_object_transport::ZkAmsMkheDirectObjectReadAtProviderV1,
    rns_native_profile::ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1,
    rns_native_public_polynomial_reader::RnsNativePublicPolynomialRoleV1,
    rns_native_rlwe_source_statement::{
        RNS_NATIVE_PRETRANSCRIPT_CANONICAL_CHECKS_V1,
        RNS_NATIVE_PRETRANSCRIPT_GLOBAL_ALIAS_BYTES_V1,
        RNS_NATIVE_PRETRANSCRIPT_GLOBAL_ALIAS_DIGESTS_V1,
        RNS_NATIVE_PRETRANSCRIPT_PUBLIC_ALIAS_BYTES_V1,
        RNS_NATIVE_PRETRANSCRIPT_PUBLIC_ALIAS_DIGESTS_V1,
        RNS_NATIVE_PRETRANSCRIPT_PUBLIC_ARTIFACT_DIGEST_BYTES_V1,
        RNS_NATIVE_PRETRANSCRIPT_PUBLIC_ARTIFACT_DIGESTS_V1,
        RNS_NATIVE_PRETRANSCRIPT_PUBLIC_DIGEST_HASH_BYTES_V1,
        RNS_NATIVE_PRETRANSCRIPT_SIGNED_CHECKS_V1,
        RNS_NATIVE_PRETRANSCRIPT_SOURCE_PLAINTEXT_BYTES_V1,
        RNS_NATIVE_PRETRANSCRIPT_SOURCE_READS_V1, RnsNativePublicRecordMetadataV1,
        derive_rns_native_pre_transcript_record_facts_v1,
        validate_rns_native_pre_transcript_public_facts_v1,
    },
    rns_native_source::{
        ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_PLAINTEXT_BYTES_V1,
        ZK_AMS_MKHE_RNS_NATIVE_SOURCE_TOTAL_FILE_BYTES_V1,
        ZkAmsMkheRnsNativeRepeatableSourceSnapshotV1, ZkAmsMkheRnsNativeSourceLayoutV1,
        ZkAmsMkheRnsNativeSourceReceiptV1, ZkAmsMkheRnsNativeSourceSnapshotV1,
    },
    rns_native_transcript::{ZkAmsMkheRnsNativePublicContextV1, ZkAmsMkheRnsNativeTranscriptV1},
};
use super::RnsNativeExistingReaderBridgeV2;

const RECORDS_V2: usize = 43;
const TARGET_LIMBS_V2: usize = ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1;
const CIPHERTEXT_ARTIFACTS_PER_ROLE_V2: usize = RECORDS_V2 * TARGET_LIMBS_V2;
const INLINE_FACTS_BYTES_CURRENT_TARGET_V2: usize = 4_000;
const NONCE_BINDING_HASH_BYTES_PER_RECORD_V2: u64 = 370;
const BEGIN_PUBLIC_DIGEST_HASH_BYTES_V2: u64 = RNS_NATIVE_PRETRANSCRIPT_PUBLIC_DIGEST_HASH_BYTES_V1
    - RECORDS_V2 as u64 * NONCE_BINDING_HASH_BYTES_PER_RECORD_V2;
const PREPARATION_AND_BEGIN_PUBLIC_DIGEST_HASH_BYTES_V2: u64 =
    RNS_NATIVE_PRETRANSCRIPT_PUBLIC_DIGEST_HASH_BYTES_V1 + BEGIN_PUBLIC_DIGEST_HASH_BYTES_V2;
const KNOWN_NEW_PEAK_BYTES_V2: usize = INLINE_FACTS_BYTES_CURRENT_TARGET_V2
    + RNS_NATIVE_PRETRANSCRIPT_PUBLIC_ARTIFACT_DIGEST_BYTES_V1
    + RNS_NATIVE_PRETRANSCRIPT_PUBLIC_ALIAS_BYTES_V1
    + ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_PLAINTEXT_BYTES_V1 as usize;

/// Source-only declaration and accounting are settled; no live gate follows.
pub(super) const RNS_NATIVE_PRETRANSCRIPT_PUBLIC_STATEMENT_SOURCE_SETTLED_V2: bool = true;
pub(super) const RNS_NATIVE_PRETRANSCRIPT_PUBLIC_STATEMENT_CONTRACT_IMPLEMENTED_V2: bool = true;

/// Every production/integration/evidence gate remains deliberately closed.
pub(super) const RNS_NATIVE_PRETRANSCRIPT_LIVE_CORRESPONDENCE_AVAILABLE_V2: bool = false;
pub(super) const RNS_NATIVE_PRETRANSCRIPT_REPEAT_READ_CONFORMANCE_QUALIFIED_V2: bool = false;
pub(super) const RNS_NATIVE_PRETRANSCRIPT_SOURCE_PREFLIGHT_INTEGRATED_V2: bool = false;
pub(super) const RNS_NATIVE_PRETRANSCRIPT_RESOURCE_EVIDENCE_QUALIFIED_V2: bool = false;
pub(super) const RNS_NATIVE_PRETRANSCRIPT_READINESS_V2: bool = false;
pub(super) const RNS_NATIVE_PRETRANSCRIPT_RELEASE_AUTHORIZED_V2: bool = false;

/// Exact bounded accounting for the pre-transcript source pass, consuming
/// transcript start, and retained digest/facts payload.
///
/// `known_new_peak_bytes` excludes the source snapshot and its existing
/// encrypted backing, the complete publication/reader bridge, allocator and
/// `Vec` headers, allocation metadata, AEAD/hash implementation work, and the
/// later transcript owner.  The 3,609-entry registry is allocated once during
/// preparation and again during begin revalidation; those instances and the
/// later 3,754-entry global registry are sequential phase-local allocations
/// and are not added together.  Work counts do not claim the input-dependent
/// registry search/move count, backend AEAD work, or hash implementation cost;
/// the ledger pins only reads, validated coefficients, and absorbed bytes.
/// Preparation hashes all 43 nonces. Begin revalidates the retained public key,
/// record, and bundle facts but has no nonce bytes to rehash, so it omits the
/// exact `43 * 370` nonce-binding absorption.
pub(super) struct RnsNativePreTranscriptResourceLedgerV2 {
    pub(super) authenticated_source_reads: u64,
    pub(super) source_plaintext_bytes: u64,
    pub(super) authenticated_backing_bytes: u64,
    pub(super) canonical_coefficient_checks: u64,
    pub(super) signed_coefficient_checks: u64,
    pub(super) retained_artifact_digests: u16,
    pub(super) retained_artifact_digest_bytes: u32,
    pub(super) inline_facts_bytes_current_target: u32,
    pub(super) pretranscript_public_alias_digests: u16,
    pub(super) pretranscript_public_alias_bytes: u32,
    pub(super) later_global_alias_digests: u16,
    pub(super) later_global_alias_bytes: u32,
    pub(super) preparation_public_digest_hash_bytes: u64,
    pub(super) begin_public_digest_hash_bytes: u64,
    pub(super) preparation_and_begin_public_digest_hash_bytes: u64,
    pub(super) known_new_peak_bytes: u32,
}

pub(super) const RNS_NATIVE_PRETRANSCRIPT_RESOURCE_LEDGER_V2:
    RnsNativePreTranscriptResourceLedgerV2 = RnsNativePreTranscriptResourceLedgerV2 {
    authenticated_source_reads: RNS_NATIVE_PRETRANSCRIPT_SOURCE_READS_V1,
    source_plaintext_bytes: RNS_NATIVE_PRETRANSCRIPT_SOURCE_PLAINTEXT_BYTES_V1,
    authenticated_backing_bytes: ZK_AMS_MKHE_RNS_NATIVE_SOURCE_TOTAL_FILE_BYTES_V1,
    canonical_coefficient_checks: RNS_NATIVE_PRETRANSCRIPT_CANONICAL_CHECKS_V1,
    signed_coefficient_checks: RNS_NATIVE_PRETRANSCRIPT_SIGNED_CHECKS_V1,
    retained_artifact_digests: RNS_NATIVE_PRETRANSCRIPT_PUBLIC_ARTIFACT_DIGESTS_V1 as u16,
    retained_artifact_digest_bytes: RNS_NATIVE_PRETRANSCRIPT_PUBLIC_ARTIFACT_DIGEST_BYTES_V1 as u32,
    inline_facts_bytes_current_target: INLINE_FACTS_BYTES_CURRENT_TARGET_V2 as u32,
    pretranscript_public_alias_digests: RNS_NATIVE_PRETRANSCRIPT_PUBLIC_ALIAS_DIGESTS_V1 as u16,
    pretranscript_public_alias_bytes: RNS_NATIVE_PRETRANSCRIPT_PUBLIC_ALIAS_BYTES_V1 as u32,
    later_global_alias_digests: RNS_NATIVE_PRETRANSCRIPT_GLOBAL_ALIAS_DIGESTS_V1 as u16,
    later_global_alias_bytes: RNS_NATIVE_PRETRANSCRIPT_GLOBAL_ALIAS_BYTES_V1 as u32,
    preparation_public_digest_hash_bytes: RNS_NATIVE_PRETRANSCRIPT_PUBLIC_DIGEST_HASH_BYTES_V1,
    begin_public_digest_hash_bytes: BEGIN_PUBLIC_DIGEST_HASH_BYTES_V2,
    preparation_and_begin_public_digest_hash_bytes:
        PREPARATION_AND_BEGIN_PUBLIC_DIGEST_HASH_BYTES_V2,
    known_new_peak_bytes: KNOWN_NEW_PEAK_BYTES_V2 as u32,
};

const _: () = {
    assert!(RECORDS_V2 == 43);
    assert!(TARGET_LIMBS_V2 == 40);
    assert!(CIPHERTEXT_ARTIFACTS_PER_ROLE_V2 == 1_720);
    assert!(RNS_NATIVE_PRETRANSCRIPT_RESOURCE_LEDGER_V2.authenticated_source_reads == 38_571);
    assert!(RNS_NATIVE_PRETRANSCRIPT_RESOURCE_LEDGER_V2.source_plaintext_bytes == 315_622_752);
    assert!(RNS_NATIVE_PRETRANSCRIPT_RESOURCE_LEDGER_V2.authenticated_backing_bytes == 316_239_888);
    assert!(RNS_NATIVE_PRETRANSCRIPT_RESOURCE_LEDGER_V2.canonical_coefficient_checks == 5_636_096);
    assert!(RNS_NATIVE_PRETRANSCRIPT_RESOURCE_LEDGER_V2.signed_coefficient_checks == 16_908_288);
    assert!(RNS_NATIVE_PRETRANSCRIPT_RESOURCE_LEDGER_V2.retained_artifact_digests == 3_520);
    assert!(RNS_NATIVE_PRETRANSCRIPT_RESOURCE_LEDGER_V2.retained_artifact_digest_bytes == 112_640);
    assert!(RNS_NATIVE_PRETRANSCRIPT_RESOURCE_LEDGER_V2.inline_facts_bytes_current_target == 4_000);
    assert!(
        RNS_NATIVE_PRETRANSCRIPT_RESOURCE_LEDGER_V2.pretranscript_public_alias_digests == 3_609
    );
    assert!(
        RNS_NATIVE_PRETRANSCRIPT_RESOURCE_LEDGER_V2.pretranscript_public_alias_bytes == 115_488
    );
    assert!(RNS_NATIVE_PRETRANSCRIPT_RESOURCE_LEDGER_V2.later_global_alias_digests == 3_754);
    assert!(RNS_NATIVE_PRETRANSCRIPT_RESOURCE_LEDGER_V2.later_global_alias_bytes == 120_128);
    assert!(
        RNS_NATIVE_PRETRANSCRIPT_RESOURCE_LEDGER_V2.preparation_public_digest_hash_bytes == 154_158
    );
    assert!(RNS_NATIVE_PRETRANSCRIPT_RESOURCE_LEDGER_V2.begin_public_digest_hash_bytes == 138_248);
    assert!(
        RNS_NATIVE_PRETRANSCRIPT_RESOURCE_LEDGER_V2.preparation_and_begin_public_digest_hash_bytes
            == 292_406
    );
    assert!(RNS_NATIVE_PRETRANSCRIPT_RESOURCE_LEDGER_V2.known_new_peak_bytes == 240_320);
    assert!(RNS_NATIVE_PRETRANSCRIPT_PUBLIC_STATEMENT_SOURCE_SETTLED_V2);
    assert!(RNS_NATIVE_PRETRANSCRIPT_PUBLIC_STATEMENT_CONTRACT_IMPLEMENTED_V2);
    assert!(!RNS_NATIVE_PRETRANSCRIPT_LIVE_CORRESPONDENCE_AVAILABLE_V2);
    assert!(!RNS_NATIVE_PRETRANSCRIPT_REPEAT_READ_CONFORMANCE_QUALIFIED_V2);
    assert!(!RNS_NATIVE_PRETRANSCRIPT_SOURCE_PREFLIGHT_INTEGRATED_V2);
    assert!(!RNS_NATIVE_PRETRANSCRIPT_RESOURCE_EVIDENCE_QUALIFIED_V2);
    assert!(!RNS_NATIVE_PRETRANSCRIPT_READINESS_V2);
    assert!(!RNS_NATIVE_PRETRANSCRIPT_RELEASE_AUTHORIZED_V2);
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RnsNativePreTranscriptPublicStatementErrorV2 {
    InvalidBridge,
    InvalidInventory,
    ResourceCeiling,
    Source,
    Transcript,
}

impl fmt::Display for RnsNativePreTranscriptPublicStatementErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for RnsNativePreTranscriptPublicStatementErrorV2 {}

/// Exact source-derived facts retained only inside the consuming coordinator.
/// The four boxed arrays preserve all 3,520 individual identities required by
/// the later global alias check; no public-bundle-only authority exists.
#[repr(C)]
struct RnsNativePreTranscriptPublicStatementFactsV2 {
    layout: ZkAmsMkheRnsNativeSourceLayoutV1,
    receipt: ZkAmsMkheRnsNativeSourceReceiptV1,
    epoch: u64,
    governed_roster_digest: [u8; 32],
    public_a_limb_digests: Box<[[u8; 32]]>,
    public_b_limb_digests: Box<[[u8; 32]]>,
    ciphertext_c0_limb_digests: Box<[[u8; 32]]>,
    ciphertext_c1_limb_digests: Box<[[u8; 32]]>,
    records: [RnsNativePublicRecordMetadataV1; RECORDS_V2],
    public_key_digest: [u8; 32],
    public_bundle_digest: [u8; 32],
}

const _: () = {
    assert!(
        usize::BITS != 64
            || core::mem::size_of::<RnsNativePreTranscriptPublicStatementFactsV2>()
                == INLINE_FACTS_BYTES_CURRENT_TARGET_V2
    );
};

impl RnsNativePreTranscriptPublicStatementFactsV2 {
    fn start_transcript_v2(
        &self,
    ) -> Result<ZkAmsMkheRnsNativeTranscriptV1, RnsNativePreTranscriptPublicStatementErrorV2> {
        self.layout
            .validate()
            .map_err(|_| RnsNativePreTranscriptPublicStatementErrorV2::Transcript)?;
        self.receipt
            .validate(self.layout)
            .map_err(|_| RnsNativePreTranscriptPublicStatementErrorV2::Transcript)?;
        validate_rns_native_pre_transcript_public_facts_v1(
            self.layout,
            self.epoch,
            self.governed_roster_digest,
            &self.public_a_limb_digests,
            &self.public_b_limb_digests,
            &self.ciphertext_c0_limb_digests,
            &self.ciphertext_c1_limb_digests,
            &self.records,
            self.public_key_digest,
            self.public_bundle_digest,
        )
        .map_err(|_| RnsNativePreTranscriptPublicStatementErrorV2::Transcript)?;
        let public_context = ZkAmsMkheRnsNativePublicContextV1::new(
            self.governed_roster_digest,
            self.public_bundle_digest,
        )
        .map_err(|_| RnsNativePreTranscriptPublicStatementErrorV2::Transcript)?;
        let transcript =
            ZkAmsMkheRnsNativeTranscriptV1::new(self.layout, self.receipt, public_context)
                .map_err(|_| RnsNativePreTranscriptPublicStatementErrorV2::Transcript)?;
        if transcript.governed_roster_digest() != self.governed_roster_digest
            || transcript.public_ciphertext_digest() != self.public_bundle_digest
            || transcript.source_binding_digest() != self.layout.source_binding_digest()
            || transcript.main_snapshot_digest() != self.receipt.main_snapshot_digest
            || transcript.nonce_snapshot_digest() != self.receipt.nonce_snapshot_digest
            || transcript.source_receipt_digest() != self.receipt.receipt_digest
        {
            return Err(RnsNativePreTranscriptPublicStatementErrorV2::Transcript);
        }
        Ok(transcript)
    }
}

/// Explicitly uninhabited live correspondence.  A future production delta
/// must replace this token only after proving that the source snapshot and
/// publication bridge originate from the same encryption lifecycle and after
/// qualifying repeated random reads.
pub(super) struct RnsNativePreTranscriptLiveCorrespondenceV2 {
    never: Infallible,
}

/// Move-only owner before the exact public transcript context is started.
#[must_use = "the source/publication authority must be consumed exactly once"]
pub(super) struct RnsNativePreTranscriptPublicStatementV2<K, P, S>
where
    K: ZkAmsMkheDirectObjectReadAtProviderV1,
    P: ZkAmsMkheDirectObjectReadAtProviderV1,
    S: ZkAmsMkheRnsNativeRepeatableSourceSnapshotV1,
{
    bridge: RnsNativeExistingReaderBridgeV2<K, P>,
    source: S,
    facts: RnsNativePreTranscriptPublicStatementFactsV2,
}

/// Move-only owner returned beside the newly started transcript. It exposes no
/// fields, facts, source, bridge, or raw-parts transition. Only the sealed
/// source-only child can consume it into claimed-qPCS/source preflight; no live
/// production source-preflight integration exists.
#[must_use = "started pre-transcript authority must be consumed only by its sealed handoff"]
pub(super) struct RnsNativeStartedPreTranscriptPublicStatementV2<K, P, S>
where
    K: ZkAmsMkheDirectObjectReadAtProviderV1,
    P: ZkAmsMkheDirectObjectReadAtProviderV1,
    S: ZkAmsMkheRnsNativeRepeatableSourceSnapshotV1,
{
    bridge: RnsNativeExistingReaderBridgeV2<K, P>,
    source: S,
    facts: RnsNativePreTranscriptPublicStatementFactsV2,
}

impl<K, P, S> RnsNativePreTranscriptPublicStatementV2<K, P, S>
where
    K: ZkAmsMkheDirectObjectReadAtProviderV1,
    P: ZkAmsMkheDirectObjectReadAtProviderV1,
    S: ZkAmsMkheRnsNativeRepeatableSourceSnapshotV1,
{
    /// Deliberately uncallable production constructor.  It consumes the exact
    /// bridge and source only when a live correspondence capability exists.
    pub(super) fn from_live_correspondence_v2(
        bridge: RnsNativeExistingReaderBridgeV2<K, P>,
        source: S,
        correspondence: RnsNativePreTranscriptLiveCorrespondenceV2,
    ) -> Result<Self, RnsNativePreTranscriptPublicStatementErrorV2> {
        let RnsNativePreTranscriptLiveCorrespondenceV2 { never } = correspondence;
        let _ = (bridge, source);
        match never {}
    }

    /// Consume the only prepared owner, construct the exact public context,
    /// and return the still-intact source/publication owner beside the fresh
    /// transcript.  Any error destroys every input together.
    pub(super) fn begin_transcript_v2(
        self,
    ) -> Result<
        (
            RnsNativeStartedPreTranscriptPublicStatementV2<K, P, S>,
            ZkAmsMkheRnsNativeTranscriptV1,
        ),
        RnsNativePreTranscriptPublicStatementErrorV2,
    > {
        let Self {
            bridge,
            source,
            facts,
        } = self;
        let transcript = facts
            .start_transcript_v2()
            .map_err(|_| RnsNativePreTranscriptPublicStatementErrorV2::Transcript)?;
        Ok((
            RnsNativeStartedPreTranscriptPublicStatementV2 {
                bridge,
                source,
                facts,
            },
            transcript,
        ))
    }
}

fn prepare_existing_reader_inner_v2<K, P, S>(
    bridge: RnsNativeExistingReaderBridgeV2<K, P>,
    mut source: S,
) -> Result<
    RnsNativePreTranscriptPublicStatementV2<K, P, S>,
    RnsNativePreTranscriptPublicStatementErrorV2,
>
where
    K: ZkAmsMkheDirectObjectReadAtProviderV1,
    P: ZkAmsMkheDirectObjectReadAtProviderV1,
    S: ZkAmsMkheRnsNativeRepeatableSourceSnapshotV1,
{
    let epoch = bridge.owners.key.key_authority.epoch();
    let governed_roster_digest = bridge.owners.key.key_authority.roster_digest();
    if epoch == 0 || governed_roster_digest == [0; 32] {
        return Err(RnsNativePreTranscriptPublicStatementErrorV2::InvalidBridge);
    }
    let (public_a, public_b, ciphertext_c0, ciphertext_c1) =
        derive_artifact_inventory_v2(|role, record, limb| {
            bridge
                .reader
                .manifest()
                .statement_artifact_digest_v1(role, record, limb)
        })?;
    let layout = source.layout();
    layout
        .validate()
        .map_err(|_| RnsNativePreTranscriptPublicStatementErrorV2::Source)?;
    let receipt = source
        .structural_receipt()
        .map_err(|_| RnsNativePreTranscriptPublicStatementErrorV2::Source)?;
    receipt
        .validate(layout)
        .map_err(|_| RnsNativePreTranscriptPublicStatementErrorV2::Source)?;
    let (records, public_key_digest, public_bundle_digest) =
        derive_rns_native_pre_transcript_record_facts_v1(
            &mut source,
            layout,
            epoch,
            governed_roster_digest,
            &public_a,
            &public_b,
            &ciphertext_c0,
            &ciphertext_c1,
        )
        .map_err(|_| RnsNativePreTranscriptPublicStatementErrorV2::Source)?;
    let facts = RnsNativePreTranscriptPublicStatementFactsV2 {
        layout,
        receipt,
        epoch,
        governed_roster_digest,
        public_a_limb_digests: public_a,
        public_b_limb_digests: public_b,
        ciphertext_c0_limb_digests: ciphertext_c0,
        ciphertext_c1_limb_digests: ciphertext_c1,
        records,
        public_key_digest,
        public_bundle_digest,
    };
    Ok(RnsNativePreTranscriptPublicStatementV2 {
        bridge,
        source,
        facts,
    })
}

type ArtifactInventoryV2 = (
    Box<[[u8; 32]]>,
    Box<[[u8; 32]]>,
    Box<[[u8; 32]]>,
    Box<[[u8; 32]]>,
);

fn derive_artifact_inventory_v2<F>(
    mut artifact_digest: F,
) -> Result<ArtifactInventoryV2, RnsNativePreTranscriptPublicStatementErrorV2>
where
    F: FnMut(RnsNativePublicPolynomialRoleV1, Option<usize>, usize) -> Option<[u8; 32]>,
{
    let mut public_a = Vec::new();
    let mut public_b = Vec::new();
    let mut ciphertext_c0 = Vec::new();
    let mut ciphertext_c1 = Vec::new();
    public_a
        .try_reserve_exact(TARGET_LIMBS_V2)
        .map_err(|_| RnsNativePreTranscriptPublicStatementErrorV2::ResourceCeiling)?;
    public_b
        .try_reserve_exact(TARGET_LIMBS_V2)
        .map_err(|_| RnsNativePreTranscriptPublicStatementErrorV2::ResourceCeiling)?;
    ciphertext_c0
        .try_reserve_exact(CIPHERTEXT_ARTIFACTS_PER_ROLE_V2)
        .map_err(|_| RnsNativePreTranscriptPublicStatementErrorV2::ResourceCeiling)?;
    ciphertext_c1
        .try_reserve_exact(CIPHERTEXT_ARTIFACTS_PER_ROLE_V2)
        .map_err(|_| RnsNativePreTranscriptPublicStatementErrorV2::ResourceCeiling)?;

    for limb in 0..TARGET_LIMBS_V2 {
        public_a.push(required_artifact_digest_v2(
            &mut artifact_digest,
            RnsNativePublicPolynomialRoleV1::PublicA,
            None,
            limb,
        )?);
    }
    for limb in 0..TARGET_LIMBS_V2 {
        public_b.push(required_artifact_digest_v2(
            &mut artifact_digest,
            RnsNativePublicPolynomialRoleV1::PublicB,
            None,
            limb,
        )?);
    }
    for record in 0..RECORDS_V2 {
        for limb in 0..TARGET_LIMBS_V2 {
            ciphertext_c0.push(required_artifact_digest_v2(
                &mut artifact_digest,
                RnsNativePublicPolynomialRoleV1::CiphertextC0,
                Some(record),
                limb,
            )?);
        }
    }
    for record in 0..RECORDS_V2 {
        for limb in 0..TARGET_LIMBS_V2 {
            ciphertext_c1.push(required_artifact_digest_v2(
                &mut artifact_digest,
                RnsNativePublicPolynomialRoleV1::CiphertextC1,
                Some(record),
                limb,
            )?);
        }
    }
    if public_a.len() != TARGET_LIMBS_V2
        || public_b.len() != TARGET_LIMBS_V2
        || ciphertext_c0.len() != CIPHERTEXT_ARTIFACTS_PER_ROLE_V2
        || ciphertext_c1.len() != CIPHERTEXT_ARTIFACTS_PER_ROLE_V2
    {
        return Err(RnsNativePreTranscriptPublicStatementErrorV2::InvalidInventory);
    }
    Ok((
        public_a.into_boxed_slice(),
        public_b.into_boxed_slice(),
        ciphertext_c0.into_boxed_slice(),
        ciphertext_c1.into_boxed_slice(),
    ))
}

fn required_artifact_digest_v2<F>(
    artifact_digest: &mut F,
    role: RnsNativePublicPolynomialRoleV1,
    record: Option<usize>,
    limb: usize,
) -> Result<[u8; 32], RnsNativePreTranscriptPublicStatementErrorV2>
where
    F: FnMut(RnsNativePublicPolynomialRoleV1, Option<usize>, usize) -> Option<[u8; 32]>,
{
    let digest = artifact_digest(role, record, limb)
        .ok_or(RnsNativePreTranscriptPublicStatementErrorV2::InvalidInventory)?;
    if digest == [0; 32] {
        return Err(RnsNativePreTranscriptPublicStatementErrorV2::InvalidInventory);
    }
    Ok(digest)
}

#[cfg(test)]
trait RnsNativePreTranscriptFixtureBridgeV2 {
    fn epoch_v2(&self) -> u64;
    fn governed_roster_digest_v2(&self) -> [u8; 32];
    fn statement_artifact_digest_v2(
        &self,
        role: RnsNativePublicPolynomialRoleV1,
        record: Option<usize>,
        limb: usize,
    ) -> Option<[u8; 32]>;
}

/// Test-only owner used to exercise the same derivation and consuming start
/// without making an alternate bridge type production authority.
#[cfg(test)]
struct RnsNativePreTranscriptFixtureHarnessV2<B, S>
where
    S: ZkAmsMkheRnsNativeRepeatableSourceSnapshotV1,
{
    bridge: B,
    source: S,
    facts: RnsNativePreTranscriptPublicStatementFactsV2,
}

#[cfg(test)]
struct RnsNativeStartedPreTranscriptFixtureHarnessV2<B, S>
where
    S: ZkAmsMkheRnsNativeRepeatableSourceSnapshotV1,
{
    bridge: B,
    source: S,
    facts: RnsNativePreTranscriptPublicStatementFactsV2,
}

#[cfg(test)]
impl<B, S> RnsNativePreTranscriptFixtureHarnessV2<B, S>
where
    S: ZkAmsMkheRnsNativeRepeatableSourceSnapshotV1,
{
    fn begin_transcript_v2(
        self,
    ) -> Result<
        (
            RnsNativeStartedPreTranscriptFixtureHarnessV2<B, S>,
            ZkAmsMkheRnsNativeTranscriptV1,
        ),
        RnsNativePreTranscriptPublicStatementErrorV2,
    > {
        let Self {
            bridge,
            source,
            facts,
        } = self;
        let transcript = facts
            .start_transcript_v2()
            .map_err(|_| RnsNativePreTranscriptPublicStatementErrorV2::Transcript)?;
        Ok((
            RnsNativeStartedPreTranscriptFixtureHarnessV2 {
                bridge,
                source,
                facts,
            },
            transcript,
        ))
    }
}

#[cfg(test)]
fn prepare_fixture_harness_v2<B, S>(
    bridge: B,
    mut source: S,
) -> Result<
    RnsNativePreTranscriptFixtureHarnessV2<B, S>,
    RnsNativePreTranscriptPublicStatementErrorV2,
>
where
    B: RnsNativePreTranscriptFixtureBridgeV2,
    S: ZkAmsMkheRnsNativeRepeatableSourceSnapshotV1,
{
    let epoch = bridge.epoch_v2();
    let governed_roster_digest = bridge.governed_roster_digest_v2();
    if epoch == 0 || governed_roster_digest == [0; 32] {
        return Err(RnsNativePreTranscriptPublicStatementErrorV2::InvalidBridge);
    }
    let (public_a, public_b, ciphertext_c0, ciphertext_c1) =
        derive_artifact_inventory_v2(|role, record, limb| {
            bridge.statement_artifact_digest_v2(role, record, limb)
        })?;
    let layout = source.layout();
    layout
        .validate()
        .map_err(|_| RnsNativePreTranscriptPublicStatementErrorV2::Source)?;
    let receipt = source
        .structural_receipt()
        .map_err(|_| RnsNativePreTranscriptPublicStatementErrorV2::Source)?;
    receipt
        .validate(layout)
        .map_err(|_| RnsNativePreTranscriptPublicStatementErrorV2::Source)?;
    let (records, public_key_digest, public_bundle_digest) =
        derive_rns_native_pre_transcript_record_facts_v1(
            &mut source,
            layout,
            epoch,
            governed_roster_digest,
            &public_a,
            &public_b,
            &ciphertext_c0,
            &ciphertext_c1,
        )
        .map_err(|_| RnsNativePreTranscriptPublicStatementErrorV2::Source)?;
    let facts = RnsNativePreTranscriptPublicStatementFactsV2 {
        layout,
        receipt,
        epoch,
        governed_roster_digest,
        public_a_limb_digests: public_a,
        public_b_limb_digests: public_b,
        ciphertext_c0_limb_digests: ciphertext_c0,
        ciphertext_c1_limb_digests: ciphertext_c1,
        records,
        public_key_digest,
        public_bundle_digest,
    };
    Ok(RnsNativePreTranscriptFixtureHarnessV2 {
        bridge,
        source,
        facts,
    })
}

#[path = "pretranscript_public_statement_v2/claimed_qpcs_source_carrier_v2.rs"]
mod claimed_qpcs_source_carrier_v2;

#[cfg(test)]
#[path = "pretranscript_public_statement_v2_tests.rs"]
mod tests;
