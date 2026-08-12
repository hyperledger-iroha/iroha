//! Private Phase-23 materialize/encrypt/source correspondence transition.
//!
//! The only production constructor is deliberately uninhabited until the
//! parent RNS-Link module can mint a context-correspondence seal from its
//! otherwise private context axes.  The implementation behind that seal is
//! complete enough to freeze the one-pass ownership and memory topology: one
//! validated packed chunk is borrowed by the exact live encryption witnesses,
//! persisted in canonical source order before C0/C1 publication, and then
//! moved unchanged into scalar materialization.
//!
//! A private child also freezes the scalable Phase-23 radix/range topology as
//! static planning evidence. Its production seals and every stronger gate are
//! deliberately uninhabited.

#![allow(
    dead_code,
    reason = "the production context-correspondence seal is intentionally uninhabited"
)]

use std::path::Path;

use crate::vega::sponge::Keccak256;

use super::super::super::{
    ZkAmsMkheErrorV1,
    packing::{
        T256PackedPlaintextDecodeWorkspaceV1, ZkAmsT256PackedPlaintextV1, ZkAmsT256PackingLayoutV1,
        visit_zk_ams_t256_packed_plaintext_used_slots_with_workspace_v1,
        zk_ams_t256_packing_layout_v1,
    },
    phase23_encrypted::{
        ZkAmsPhase23AccumulatorShapeV1, ZkAmsPhase23MaterializedAccumulatorsV1,
        materialize_release_accumulator_chunk_stream_with_decoder_v1, validate_materialized,
    },
    phase23_rns_link::{
        ZkAmsPhase23RnsLinkContextV1, ZkAmsPhase23RnsLinkExternalSourceAssemblyV1,
        ZkAmsPhase23RnsLinkExternalSourcePublicationV1, ZkAmsPhase23RnsLinkFamilyV1,
        ZkAmsPhase23RnsLinkSecretChunkV1,
    },
};
use super::{
    MaskedRelaxedRandomSourceV1, ZkAmsMkheDirectObjectCasPublicationV1,
    ZkAmsMkheDirectObjectReadAtProviderV1, ZkAmsMkheStreamingCollectiveCiphertextV1,
    ZkAmsMkheStreamingCollectiveEncryptionKeyAuthorityV1,
    encrypt_zk_ams_mkhe_collective_packed_streaming_borrowed_with_prepublication_v1,
};

const PHASE23_ORCHESTRATOR_VERSION_V1: u8 = 1;
const PHASE23_RECORD_COUNT_V1: usize = 43;
const PHASE23_MAIN_BLOCKS_PER_RECORD_V1: usize = 896;
const PHASE23_CANONICAL_BLOCKS_PER_RECORD_V1: usize = 512;
const PHASE23_SIGNED_BLOCKS_PER_WITNESS_V1: usize = 128;
const PHASE23_CANONICAL_COEFFICIENTS_PER_BLOCK_V1: usize = 256;
const PHASE23_SIGNED_COEFFICIENTS_PER_BLOCK_V1: usize = 1_024;
const PHASE23_MAIN_BLOCK_BYTES_V1: usize = 8_192;
const PHASE23_NONCE_BYTES_V1: usize = 32;
const PHASE23_RING_DEGREE_V1: usize = 131_072;
const PHASE23_MANIFEST_CAPACITY_V1: usize = PHASE23_RECORD_COUNT_V1;
const PHASE23_BUNDLE_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.materialize-encrypt-source-bundle";

const PHASE23_X_VALUES_V1: u32 = 89;
const PHASE23_U_AND_E_VALUES_V1: u32 = 1_048_576;
const PHASE23_RE_VALUES_V1: u32 = 1_024;
const PHASE23_W_VALUES_V1: u32 = 524_288;
const PHASE23_RW_VALUES_V1: u32 = 512;

// Conservative named-heap equation. Direct-object/confidential-provider
// internals, kernel or OS page cache, allocator metadata outside the named Vec
// owners, and the confidential files themselves are explicitly excluded.
const PHASE23_MATERIALIZED_SCALAR_OWNER_BYTES_V1: usize = 1_574_490 * 32;
const PHASE23_ONE_PACKED_CHUNK_BYTES_V1: usize = PHASE23_RING_DEGREE_V1 * 32;
const PHASE23_DECODER_WORKSPACE_BYTES_V1: usize = 8 * 1_048_576;
const PHASE23_ENCRYPTION_OWNER_BYTES_V1: usize = 10_066_330; // conservative 9.6 MiB
const PHASE23_COMPACT_MANIFEST_OWNER_BYTES_V1: usize = 4_718_592; // 4.5 MiB
const PHASE23_SECRET_CHUNK_POOL_PAYLOAD_BYTES_V1: usize =
    PHASE23_MAIN_BLOCKS_PER_RECORD_V1 * PHASE23_MAIN_BLOCK_BYTES_V1 + PHASE23_NONCE_BYTES_V1;
const PHASE23_SECRET_CHUNK_POOL_METADATA_BYTES_V1: usize = PHASE23_MAIN_BLOCKS_PER_RECORD_V1
    * core::mem::size_of::<ZkAmsPhase23RnsLinkSecretChunkV1>()
    + core::mem::size_of::<Vec<ZkAmsPhase23RnsLinkSecretChunkV1>>()
    + core::mem::size_of::<Option<ZkAmsPhase23RnsLinkSecretChunkV1>>();
const PHASE23_SMALL_SPOOL_HANDLE_BYTES_V1: usize = 65_536;
const PHASE23_NAMED_HEAP_PEAK_BYTES_V1: usize = PHASE23_MATERIALIZED_SCALAR_OWNER_BYTES_V1
    + PHASE23_ONE_PACKED_CHUNK_BYTES_V1
    + PHASE23_DECODER_WORKSPACE_BYTES_V1
    + PHASE23_ENCRYPTION_OWNER_BYTES_V1
    + PHASE23_COMPACT_MANIFEST_OWNER_BYTES_V1
    + PHASE23_SECRET_CHUNK_POOL_PAYLOAD_BYTES_V1
    + PHASE23_SECRET_CHUNK_POOL_METADATA_BYTES_V1
    + PHASE23_SMALL_SPOOL_HANDLE_BYTES_V1;
const PHASE23_NAMED_HEAP_CEILING_BYTES_V1: usize = 160 * 1_048_576;

const _: () = {
    assert!(PHASE23_RECORD_COUNT_V1 == 1 + 16 + 16 + 1 + 8 + 1);
    assert!(PHASE23_MAIN_BLOCKS_PER_RECORD_V1 == 512 + 128 + 128 + 128);
    assert!(PHASE23_CANONICAL_BLOCKS_PER_RECORD_V1 * 256 == PHASE23_RING_DEGREE_V1);
    assert!(PHASE23_SIGNED_BLOCKS_PER_WITNESS_V1 * 1_024 == PHASE23_RING_DEGREE_V1);
    assert!(PHASE23_SECRET_CHUNK_POOL_PAYLOAD_BYTES_V1 == 7_340_064);
    assert!(PHASE23_NAMED_HEAP_PEAK_BYTES_V1 < PHASE23_NAMED_HEAP_CEILING_BYTES_V1);
};

/// Production has no variant. Tests can exercise framing helpers without
/// claiming that the parent-private context axes have been connected.
enum Phase23ContextCorrespondenceSealV1 {
    #[cfg(test)]
    TestOnly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Phase23RecordPositionV1 {
    ordinal: u16,
    family: ZkAmsPhase23RnsLinkFamilyV1,
    chunk_index: u16,
    family_chunk_count: u16,
    logical_value_count: u32,
}

impl Phase23RecordPositionV1 {
    fn layout_v1(self) -> Result<ZkAmsT256PackingLayoutV1, ZkAmsMkheErrorV1> {
        let layout = zk_ams_t256_packing_layout_v1(self.logical_value_count)?;
        if layout.chunk_count != u32::from(self.family_chunk_count)
            || u32::from(self.chunk_index) >= layout.chunk_count
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Ok(layout)
    }

    fn used_slots_v1(self) -> Result<u32, ZkAmsMkheErrorV1> {
        let layout = self.layout_v1()?;
        if u32::from(self.chunk_index) + 1 == layout.chunk_count {
            Ok(layout.final_chunk_used_slots)
        } else {
            Ok(layout.slots_per_chunk)
        }
    }
}

fn phase23_record_position_v1(ordinal: u16) -> Result<Phase23RecordPositionV1, ZkAmsMkheErrorV1> {
    let (family, chunk_index, family_chunk_count, logical_value_count) = match ordinal {
        0 => (ZkAmsPhase23RnsLinkFamilyV1::X, 0, 1, PHASE23_X_VALUES_V1),
        1..=16 => (
            ZkAmsPhase23RnsLinkFamilyV1::U,
            ordinal - 1,
            16,
            PHASE23_U_AND_E_VALUES_V1,
        ),
        17..=32 => (
            ZkAmsPhase23RnsLinkFamilyV1::E,
            ordinal - 17,
            16,
            PHASE23_U_AND_E_VALUES_V1,
        ),
        33 => (ZkAmsPhase23RnsLinkFamilyV1::RE, 0, 1, PHASE23_RE_VALUES_V1),
        34..=41 => (
            ZkAmsPhase23RnsLinkFamilyV1::W,
            ordinal - 34,
            8,
            PHASE23_W_VALUES_V1,
        ),
        42 => (ZkAmsPhase23RnsLinkFamilyV1::RW, 0, 1, PHASE23_RW_VALUES_V1),
        _ => return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold),
    };
    Ok(Phase23RecordPositionV1 {
        ordinal,
        family,
        chunk_index,
        family_chunk_count,
        logical_value_count,
    })
}

fn require_expected_packed_coordinate_v1(
    position: Phase23RecordPositionV1,
    layout: ZkAmsT256PackingLayoutV1,
    packed: &ZkAmsT256PackedPlaintextV1,
) -> Result<u32, ZkAmsMkheErrorV1> {
    let expected_layout = position.layout_v1()?;
    let expected_used_slots = position.used_slots_v1()?;
    if layout != expected_layout
        || packed.layout_digest != expected_layout.digest
        || packed.chunk_index != u32::from(position.chunk_index)
        || packed.used_slots != expected_used_slots
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(expected_used_slots)
}

/// Every owner is allocated while it still contains zeros. Moving the pool or
/// one chunk moves pointers only; every unused or failed-path chunk keeps the
/// confidential leaf's zeroizing `Drop` implementation.
struct Phase23SecretRecordChunkPoolV1 {
    main: Vec<ZkAmsPhase23RnsLinkSecretChunkV1>,
    nonce: Option<ZkAmsPhase23RnsLinkSecretChunkV1>,
}

impl Phase23SecretRecordChunkPoolV1 {
    fn try_new_exact_v1() -> Result<Self, ZkAmsMkheErrorV1> {
        let mut main = Vec::new();
        main.try_reserve_exact(PHASE23_MAIN_BLOCKS_PER_RECORD_V1)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if main.capacity() != PHASE23_MAIN_BLOCKS_PER_RECORD_V1 {
            return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
        }
        for _ in 0..PHASE23_MAIN_BLOCKS_PER_RECORD_V1 {
            main.push(ZkAmsPhase23RnsLinkSecretChunkV1::new_main_block_zeroed_v1()?);
        }
        let nonce = Some(ZkAmsPhase23RnsLinkSecretChunkV1::new_nonce_zeroed_v1()?);
        Ok(Self { main, nonce })
    }

    #[allow(clippy::too_many_arguments)]
    fn persist_exact_record_v1(
        self,
        source: &mut ZkAmsPhase23RnsLinkExternalSourceAssemblyV1,
        position: Phase23RecordPositionV1,
        canonical_plaintext: &[[u8; 32]],
        ephemeral: &[i64],
        error_zero: &[i64],
        error_one: &[i64],
        nonce: &[u8; 32],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        if canonical_plaintext.len() != PHASE23_RING_DEGREE_V1
            || ephemeral.len() != PHASE23_RING_DEGREE_V1
            || error_zero.len() != PHASE23_RING_DEGREE_V1
            || error_one.len() != PHASE23_RING_DEGREE_V1
            || self.main.len() != PHASE23_MAIN_BLOCKS_PER_RECORD_V1
            || self.main.capacity() != PHASE23_MAIN_BLOCKS_PER_RECORD_V1
            || self.nonce.is_none()
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }

        let Self {
            main,
            nonce: mut nonce_owner,
        } = self;
        let mut chunks = main.into_iter();
        for (block, coefficients) in canonical_plaintext
            .chunks_exact(PHASE23_CANONICAL_COEFFICIENTS_PER_BLOCK_V1)
            .enumerate()
        {
            let mut chunk = chunks.next().ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
            fill_canonical_source_block_v1(chunk.as_mut_bytes_v1(), coefficients)?;
            source.write_next_canonical_plaintext_block_v1(
                position.ordinal,
                u16::try_from(block).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
                chunk,
            )?;
        }
        for (block, coefficients) in ephemeral
            .chunks_exact(PHASE23_SIGNED_COEFFICIENTS_PER_BLOCK_V1)
            .enumerate()
        {
            let mut chunk = chunks.next().ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
            fill_signed_source_block_v1(chunk.as_mut_bytes_v1(), coefficients)?;
            source.write_next_ephemeral_block_v1(
                position.ordinal,
                u16::try_from(block).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
                chunk,
            )?;
        }
        for (block, coefficients) in error_zero
            .chunks_exact(PHASE23_SIGNED_COEFFICIENTS_PER_BLOCK_V1)
            .enumerate()
        {
            let mut chunk = chunks.next().ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
            fill_signed_source_block_v1(chunk.as_mut_bytes_v1(), coefficients)?;
            source.write_next_error_zero_block_v1(
                position.ordinal,
                u16::try_from(block).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
                chunk,
            )?;
        }
        for (block, coefficients) in error_one
            .chunks_exact(PHASE23_SIGNED_COEFFICIENTS_PER_BLOCK_V1)
            .enumerate()
        {
            let mut chunk = chunks.next().ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
            fill_signed_source_block_v1(chunk.as_mut_bytes_v1(), coefficients)?;
            source.write_next_error_one_block_v1(
                position.ordinal,
                u16::try_from(block).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
                chunk,
            )?;
        }
        if chunks.next().is_some() {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let mut nonce_owner = nonce_owner
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        nonce_owner.as_mut_bytes_v1().copy_from_slice(nonce);
        source.write_next_nonce_v1(position.ordinal, nonce_owner)
    }
}

fn fill_canonical_source_block_v1(
    output: &mut [u8],
    coefficients: &[[u8; 32]],
) -> Result<(), ZkAmsMkheErrorV1> {
    if output.len() != PHASE23_MAIN_BLOCK_BYTES_V1
        || coefficients.len() != PHASE23_CANONICAL_COEFFICIENTS_PER_BLOCK_V1
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    for (encoded, coefficient) in output.chunks_exact_mut(32).zip(coefficients) {
        encoded.copy_from_slice(coefficient);
    }
    Ok(())
}

fn fill_signed_source_block_v1(
    output: &mut [u8],
    coefficients: &[i64],
) -> Result<(), ZkAmsMkheErrorV1> {
    if output.len() != PHASE23_MAIN_BLOCK_BYTES_V1
        || coefficients.len() != PHASE23_SIGNED_COEFFICIENTS_PER_BLOCK_V1
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    for (encoded, coefficient) in output.chunks_exact_mut(8).zip(coefficients) {
        encoded.copy_from_slice(&coefficient.to_be_bytes());
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct Phase23BundleDigestAxesV1 {
    profile_digest: [u8; 32],
    roster_digest: [u8; 32],
    materialized_transcript_digest: [u8; 32],
    batch_id: [u8; 32],
    ordered_batch_input_digest: [u8; 32],
    fold_count: u8,
    shape: ZkAmsPhase23AccumulatorShapeV1,
    materialized_digest: [u8; 32],
    key_digest: [u8; 32],
    key_authority_digest: [u8; 32],
    key_epoch: u64,
    source_receipt_digest: [u8; 32],
    public_artifact_manifest_bound: bool,
}

fn phase23_bundle_digest_from_frames_v1(
    axes: Phase23BundleDigestAxesV1,
    ordered_manifest_digests: &[[u8; 32]; PHASE23_RECORD_COUNT_V1],
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    if [
        axes.profile_digest,
        axes.roster_digest,
        axes.materialized_transcript_digest,
        axes.batch_id,
        axes.ordered_batch_input_digest,
        axes.materialized_digest,
        axes.key_digest,
        axes.key_authority_digest,
        axes.source_receipt_digest,
    ]
    .contains(&[0; 32])
        || axes.fold_count == 0
        || axes.key_epoch == 0
        || !axes.public_artifact_manifest_bound
        || ordered_manifest_digests.contains(&[0; 32])
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    require_exact_release_shape_v1(axes.shape)?;

    let mut hash = Keccak256::new();
    hash.update(PHASE23_BUNDLE_DOMAIN_V1);
    hash.update(&[PHASE23_ORCHESTRATOR_VERSION_V1]);
    hash.update(&axes.profile_digest);
    hash.update(&axes.roster_digest);
    hash.update(&axes.materialized_transcript_digest);
    hash.update(&axes.batch_id);
    hash.update(&axes.ordered_batch_input_digest);
    hash.update(&[axes.fold_count]);
    for value in [
        axes.shape.x,
        axes.shape.e,
        axes.shape.r_e,
        axes.shape.w,
        axes.shape.r_w,
    ] {
        hash.update(&value.to_be_bytes());
    }
    hash.update(&axes.materialized_digest);
    hash.update(&axes.key_digest);
    hash.update(&axes.key_authority_digest);
    hash.update(&axes.key_epoch.to_be_bytes());
    hash.update(&axes.source_receipt_digest);
    hash.update(&[axes.public_artifact_manifest_bound as u8]);
    hash.update(&(PHASE23_RECORD_COUNT_V1 as u16).to_be_bytes());
    for (ordinal, manifest_digest) in ordered_manifest_digests.iter().enumerate() {
        let position = phase23_record_position_v1(
            u16::try_from(ordinal).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        )?;
        hash.update(&position.ordinal.to_be_bytes());
        hash.update(&[position.family as u8]);
        hash.update(&position.chunk_index.to_be_bytes());
        hash.update(manifest_digest);
    }
    let digest = hash.finalize();
    if digest == [0; 32] {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(digest)
}

fn require_exact_release_shape_v1(
    shape: ZkAmsPhase23AccumulatorShapeV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    if shape.x != PHASE23_X_VALUES_V1
        || shape.e != PHASE23_U_AND_E_VALUES_V1
        || shape.r_e != PHASE23_RE_VALUES_V1
        || shape.w != PHASE23_W_VALUES_V1
        || shape.r_w != PHASE23_RW_VALUES_V1
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(())
}

/// One move-only correspondence owner. It deliberately exposes no getters,
/// codec, clone, or tuple decomposition; later stages must add a
/// purpose-specific consuming transition.
#[must_use = "dropping this owner closes the source snapshots and all correspondence capability"]
struct ZkAmsPhase23MaterializedEncryptedSourceOwnerV1<K, P> {
    materialized: ZkAmsPhase23MaterializedAccumulatorsV1,
    manifests: Vec<ZkAmsMkheStreamingCollectiveCiphertextV1>,
    source: ZkAmsPhase23RnsLinkExternalSourcePublicationV1,
    authority: ZkAmsMkheStreamingCollectiveEncryptionKeyAuthorityV1,
    key_provider: K,
    ciphertext_publisher: P,
    public_artifact_manifest_bound: bool,
    bundle_digest: [u8; 32],
}

impl<K, P> ZkAmsPhase23MaterializedEncryptedSourceOwnerV1<K, P> {
    fn digest_axes_v1(&self) -> Phase23BundleDigestAxesV1 {
        Phase23BundleDigestAxesV1 {
            profile_digest: self.materialized.profile_digest,
            roster_digest: self.materialized.roster_digest,
            materialized_transcript_digest: self.materialized.transcript_digest,
            batch_id: self.materialized.batch_id,
            ordered_batch_input_digest: self.materialized.ordered_batch_input_digest,
            fold_count: self.materialized.fold_count,
            shape: self.materialized.shape,
            materialized_digest: self.materialized.digest,
            key_digest: self.authority.key_digest(),
            key_authority_digest: self.authority.authority_digest(),
            key_epoch: self.authority.epoch(),
            source_receipt_digest: self.source.receipt_v1().receipt_digest_v1(),
            public_artifact_manifest_bound: self.public_artifact_manifest_bound,
        }
    }

    fn validate_v1(&self) -> Result<(), ZkAmsMkheErrorV1> {
        validate_materialized(&self.materialized)?;
        require_exact_release_shape_v1(self.materialized.shape)?;
        if self.manifests.len() != PHASE23_MANIFEST_CAPACITY_V1
            || self.manifests.capacity() != PHASE23_MANIFEST_CAPACITY_V1
            || self.authority.next_sample_index() != PHASE23_RECORD_COUNT_V1 as u64
            || self.authority.profile_digest() != self.materialized.profile_digest
            || self.authority.roster_digest() != self.materialized.roster_digest
            || !self.public_artifact_manifest_bound
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let mut manifest_digests = [[0_u8; 32]; PHASE23_RECORD_COUNT_V1];
        for (ordinal, manifest) in self.manifests.iter().enumerate() {
            manifest.validate_for_authority_v1(&self.authority)?;
            let position = phase23_record_position_v1(
                u16::try_from(ordinal).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
            )?;
            if manifest.sample_index != ordinal as u64
                || manifest.topology.layout_digest != position.layout_v1()?.digest
                || manifest.topology.plaintext_chunk_index != u32::from(position.chunk_index)
                || manifest.topology.plaintext_used_slots != position.used_slots_v1()?
            {
                return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
            }
            manifest_digests[ordinal] = manifest.manifest_digest();
        }
        if self.bundle_digest
            != phase23_bundle_digest_from_frames_v1(self.digest_axes_v1(), &manifest_digests)?
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Ok(())
    }
}

struct Phase23MaterializeEncryptChunkStreamV1<'a, I, R, K, P> {
    chunks: I,
    authority: &'a mut ZkAmsMkheStreamingCollectiveEncryptionKeyAuthorityV1,
    random: &'a mut R,
    key_provider: &'a mut K,
    ciphertext_publisher: &'a mut P,
    source: &'a mut ZkAmsPhase23RnsLinkExternalSourceAssemblyV1,
    manifests: &'a mut Vec<ZkAmsMkheStreamingCollectiveCiphertextV1>,
    next_record: u16,
}

impl<I, R, K, P> Iterator for Phase23MaterializeEncryptChunkStreamV1<'_, I, R, K, P>
where
    I: Iterator<Item = Result<ZkAmsT256PackedPlaintextV1, ZkAmsMkheErrorV1>>,
    R: MaskedRelaxedRandomSourceV1,
    K: ZkAmsMkheDirectObjectReadAtProviderV1,
    P: ZkAmsMkheDirectObjectCasPublicationV1,
{
    type Item = Result<ZkAmsT256PackedPlaintextV1, ZkAmsMkheErrorV1>;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.chunks.next()?;
        if usize::from(self.next_record) >= PHASE23_RECORD_COUNT_V1 {
            return Some(Err(ZkAmsMkheErrorV1::InvalidPhase23Fold));
        }
        let packed = match next {
            Ok(packed) => packed,
            Err(error) => return Some(Err(error)),
        };
        let position = match phase23_record_position_v1(self.next_record) {
            Ok(position) => position,
            Err(error) => return Some(Err(error)),
        };
        let layout = match position.layout_v1() {
            Ok(layout) => layout,
            Err(error) => return Some(Err(error)),
        };
        let expected_used_slots =
            match require_expected_packed_coordinate_v1(position, layout, &packed) {
                Ok(used_slots) => used_slots,
                Err(error) => return Some(Err(error)),
            };
        // Schedule coordinates are checked before the parent core performs
        // full artifact validation, allocates the source pool, or samples
        // entropy. An otherwise valid chunk from another family position must
        // never reach source persistence or output publication.
        if self.manifests.len() != usize::from(self.next_record)
            || self.manifests.capacity() != PHASE23_MANIFEST_CAPACITY_V1
        {
            return Some(Err(ZkAmsMkheErrorV1::InvalidPhase23Fold));
        }

        let source = &mut *self.source;
        let result =
            encrypt_zk_ams_mkhe_collective_packed_streaming_borrowed_with_prepublication_v1(
                self.authority,
                layout,
                &packed,
                self.random,
                self.key_provider,
                self.ciphertext_publisher,
                || {
                    let pool = Phase23SecretRecordChunkPoolV1::try_new_exact_v1()?;
                    Ok(
                        move |canonical_plaintext: &[[u8; 32]],
                              ephemeral: &[i64],
                              error_zero: &[i64],
                              error_one: &[i64],
                              nonce: &[u8; 32]| {
                            pool.persist_exact_record_v1(
                                source,
                                position,
                                canonical_plaintext,
                                ephemeral,
                                error_zero,
                                error_one,
                                nonce,
                            )
                        },
                    )
                },
            );
        let manifest = match result {
            Ok(manifest) => manifest,
            Err(error) => return Some(Err(error)),
        };
        if manifest.sample_index != u64::from(position.ordinal)
            || manifest.topology.layout_digest != layout.digest
            || manifest.topology.plaintext_chunk_index != u32::from(position.chunk_index)
            || manifest.topology.plaintext_used_slots != expected_used_slots
        {
            return Some(Err(ZkAmsMkheErrorV1::InvalidPhase23Fold));
        }
        self.manifests.push(manifest);
        self.next_record += 1;
        Some(Ok(packed))
    }
}

#[allow(
    dead_code,
    clippy::too_many_arguments,
    reason = "production remains uninhabited until the parent mints a context-correspondence seal"
)]
fn materialize_encrypt_and_publish_phase23_source_v1<I, R, K, P>(
    _correspondence: Phase23ContextCorrespondenceSealV1,
    context: ZkAmsPhase23RnsLinkContextV1,
    directory: impl AsRef<Path>,
    mut authority: ZkAmsMkheStreamingCollectiveEncryptionKeyAuthorityV1,
    mut random: R,
    mut key_provider: K,
    mut ciphertext_publisher: P,
    profile_digest: [u8; 32],
    roster_digest: [u8; 32],
    transcript_digest: [u8; 32],
    batch_id: [u8; 32],
    ordered_batch_input_digest: [u8; 32],
    fold_count: u8,
    shape: ZkAmsPhase23AccumulatorShapeV1,
    packed_chunks: I,
) -> Result<ZkAmsPhase23MaterializedEncryptedSourceOwnerV1<K, P>, ZkAmsMkheErrorV1>
where
    I: IntoIterator<Item = Result<ZkAmsT256PackedPlaintextV1, ZkAmsMkheErrorV1>>,
    R: MaskedRelaxedRandomSourceV1,
    K: ZkAmsMkheDirectObjectReadAtProviderV1,
    P: ZkAmsMkheDirectObjectCasPublicationV1,
{
    require_exact_release_shape_v1(shape)?;
    if authority.next_sample_index() != 0
        || authority.profile_digest() != profile_digest
        || authority.roster_digest() != roster_digest
        || [
            profile_digest,
            roster_digest,
            transcript_digest,
            batch_id,
            ordered_batch_input_digest,
        ]
        .contains(&[0; 32])
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }

    let mut manifests = Vec::new();
    manifests
        .try_reserve_exact(PHASE23_MANIFEST_CAPACITY_V1)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if manifests.capacity() != PHASE23_MANIFEST_CAPACITY_V1 {
        return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
    }
    let mut source = ZkAmsPhase23RnsLinkExternalSourceAssemblyV1::begin_v1(context, directory)?;
    let stream = Phase23MaterializeEncryptChunkStreamV1 {
        chunks: packed_chunks.into_iter(),
        authority: &mut authority,
        random: &mut random,
        key_provider: &mut key_provider,
        ciphertext_publisher: &mut ciphertext_publisher,
        source: &mut source,
        manifests: &mut manifests,
        next_record: 0,
    };
    let materialized = materialize_release_accumulator_chunk_stream_with_decoder_v1(
        profile_digest,
        roster_digest,
        transcript_digest,
        batch_id,
        ordered_batch_input_digest,
        fold_count,
        shape,
        stream,
        &mut |layout, packed, visit| {
            // The encryption call and all live witness owners have completed
            // before this temporary decoder allocation. It is erased before
            // the next record validates or samples entropy.
            let mut workspace = T256PackedPlaintextDecodeWorkspaceV1::try_new_v1()?;
            visit_zk_ams_t256_packed_plaintext_used_slots_with_workspace_v1(
                layout,
                packed,
                &mut workspace,
                visit,
            )
        },
    )?;
    if manifests.len() != PHASE23_RECORD_COUNT_V1
        || manifests.capacity() != PHASE23_MANIFEST_CAPACITY_V1
        || authority.next_sample_index() != PHASE23_RECORD_COUNT_V1 as u64
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let source = source.finish_v1()?;
    drop(random);

    let mut manifest_digests = [[0_u8; 32]; PHASE23_RECORD_COUNT_V1];
    for (ordinal, manifest) in manifests.iter().enumerate() {
        manifest_digests[ordinal] = manifest.manifest_digest();
    }
    let axes = Phase23BundleDigestAxesV1 {
        profile_digest: materialized.profile_digest,
        roster_digest: materialized.roster_digest,
        materialized_transcript_digest: materialized.transcript_digest,
        batch_id: materialized.batch_id,
        ordered_batch_input_digest: materialized.ordered_batch_input_digest,
        fold_count: materialized.fold_count,
        shape: materialized.shape,
        materialized_digest: materialized.digest,
        key_digest: authority.key_digest(),
        key_authority_digest: authority.authority_digest(),
        key_epoch: authority.epoch(),
        source_receipt_digest: source.receipt_v1().receipt_digest_v1(),
        public_artifact_manifest_bound: true,
    };
    let bundle_digest = phase23_bundle_digest_from_frames_v1(axes, &manifest_digests)?;
    let owner = ZkAmsPhase23MaterializedEncryptedSourceOwnerV1 {
        materialized,
        manifests,
        source,
        authority,
        key_provider,
        ciphertext_publisher,
        public_artifact_manifest_bound: true,
        bundle_digest,
    };
    owner.validate_v1()?;
    Ok(owner)
}

#[path = "incremental_source_phase23_radix_range_v2.rs"]
mod radix_range_v2;

#[cfg(test)]
const _: () = {
    assert!(include_bytes!("incremental_source_phase23_radix_range_v2.rs").len() <= 52_000);
    assert!(include_bytes!("incremental_source_phase23_radix_range_v2_tests.rs").len() <= 34_000);
};

#[path = "incremental_source_phase23_source_algebra.rs"]
mod source_algebra;

impl<K, P> ZkAmsPhase23MaterializedEncryptedSourceOwnerV1<K, P> {
    /// Sole private consuming seam into the still-uninhabited source-algebra
    /// prerequisite. No tuple split or borrowed callback exposes the owner.
    fn into_source_algebra_prerequisite_v2(
        self,
        ordered_ciphertexts: source_algebra::OrderedCiphertextBundleSealV2,
        radix_hyrax_proof: source_algebra::RadixHyraxProofSealV2,
    ) -> Result<source_algebra::Phase23SourceAlgebraPrerequisiteV2<K, P>, ZkAmsMkheErrorV1> {
        source_algebra::consume_phase23_source_algebra_prerequisite_v2(
            self,
            ordered_ciphertexts,
            radix_hyrax_proof,
        )
    }
}

#[cfg(test)]
#[path = "incremental_source_phase23_tests.rs"]
mod tests;
