//! Compact Phase-23 radix witness materialization.
//!
//! This private transition consumes authenticated source-replay evidence through
//! a strict canonical-read cursor and writes only three packed comparator lanes
//! per 16,384-coefficient group. It constructs no commitments, transcript,
//! proof, final arithmetic plane, receipt, or release authority.

#![allow(
    dead_code,
    reason = "the production scratch-sink authority remains uninhabited"
)]

use super::{
    PHASE23_CANONICAL_BLOCKS_PER_RECORD_V1, PHASE23_CANONICAL_COEFFICIENTS_PER_BLOCK_V1,
    PHASE23_MAIN_BLOCK_BYTES_V1, PHASE23_RECORD_COUNT_V1, PHASE23_RING_DEGREE_V1, ZkAmsMkheErrorV1,
    phase23_record_position_v1,
    source_algebra::{
        Phase23GlobalLookupRadixSourceCursorV2, Phase23GlobalLookupSourceReplayEvidenceV1,
        Phase23GlobalLookupSourceReplayV1, RadixHyraxProofSealV2,
        bind_radix_hyrax_replay_after_materialization_v2,
    },
};
use crate::vega::{VEGA_T256_SCALAR_MODULUS_BE_V1, sponge::Keccak256};
use core::convert::Infallible;
#[cfg(test)]
use core::sync::atomic::{AtomicUsize, Ordering};
use iroha_crypto::confidential_spool::{
    ConfidentialSpoolChunkV1, ConfidentialSpoolLayoutV1, ConfidentialSpoolSnapshotV1,
    ConfidentialSpoolWriterV1,
};
use std::path::PathBuf;

const RADIX_WITNESS_VERSION_V2: u8 = 2;
const RADIX_BASE_V2: u16 = 1 << 15;
const RADIX_LOW_LIMBS_V2: usize = 17;
const RADIX_COMPARATOR_BITS_V2: usize = 18;
const RADIX_GROUPS_PER_RECORD_V2: usize = 8;
const RADIX_SOURCE_BLOCKS_PER_GROUP_V2: usize = 64;
const RADIX_SOURCE_COEFFICIENTS_PER_BLOCK_V2: usize = 256;
const RADIX_COEFFICIENTS_PER_GROUP_V2: usize = 16_384;
const RADIX_PACKED_LANES_PER_GROUP_V2: usize = 3;
const RADIX_GROUP_COUNT_V2: usize = PHASE23_RECORD_COUNT_V1 * RADIX_GROUPS_PER_RECORD_V2;
const RADIX_WITNESS_SLOT_COUNT_V2: usize = RADIX_GROUP_COUNT_V2 * RADIX_PACKED_LANES_PER_GROUP_V2;
const RADIX_WITNESS_SLOT_PLAINTEXT_BYTES_V2: u64 = RADIX_COEFFICIENTS_PER_GROUP_V2 as u64;
const RADIX_WITNESS_PLAINTEXT_BYTES_V2: u64 =
    RADIX_WITNESS_SLOT_COUNT_V2 as u64 * RADIX_WITNESS_SLOT_PLAINTEXT_BYTES_V2;
const RADIX_WITNESS_AUTHENTICATION_TAG_BYTES_V2: u64 = RADIX_WITNESS_SLOT_COUNT_V2 as u64 * 16;
const RADIX_WITNESS_FILE_BYTES_V2: u64 =
    RADIX_WITNESS_PLAINTEXT_BYTES_V2 + RADIX_WITNESS_AUTHENTICATION_TAG_BYTES_V2;
const RADIX_WITNESS_SPOOL_IO_BYTES_V2: u64 = 2 * RADIX_WITNESS_FILE_BYTES_V2;
const RADIX_SOURCE_REREAD_BLOCKS_V2: usize =
    PHASE23_RECORD_COUNT_V1 * PHASE23_CANONICAL_BLOCKS_PER_RECORD_V1;
const RADIX_SOURCE_REREAD_PLAINTEXT_BYTES_V2: u64 =
    RADIX_SOURCE_REREAD_BLOCKS_V2 as u64 * PHASE23_MAIN_BLOCK_BYTES_V1 as u64;
const RADIX_SOURCE_REREAD_TAG_BYTES_V2: u64 = RADIX_SOURCE_REREAD_BLOCKS_V2 as u64 * 16;
const RADIX_SOURCE_REREAD_AUTHENTICATED_BYTES_V2: u64 =
    RADIX_SOURCE_REREAD_PLAINTEXT_BYTES_V2 + RADIX_SOURCE_REREAD_TAG_BYTES_V2;
const RADIX_WITNESS_TOTAL_IO_BYTES_V2: u64 =
    RADIX_SOURCE_REREAD_AUTHENTICATED_BYTES_V2 + RADIX_WITNESS_SPOOL_IO_BYTES_V2;
const RADIX_COEFFICIENT_SCRATCH_BUDGET_BYTES_V2: usize = 384;
const RADIX_COEFFICIENT_BYTE_OWNERS_V2: usize = 5;
const RADIX_COEFFICIENT_PUBLIC_THRESHOLD_BYTES_V2: usize =
    RADIX_LOW_LIMBS_V2 * core::mem::size_of::<u16>();
const RADIX_COEFFICIENT_COPY_OWNER_ALLOWANCE_BYTES_V2: usize = 32;
// Algorithmic named payload only: three 16-KiB output chunks, one 8-KiB
// authenticated input chunk, and a conservative scalar-scratch allowance. It
// is not an RSS, allocator, filesystem, kernel-cache, or provider-internal claim.
const RADIX_WITNESS_NAMED_LIVE_PAYLOAD_BYTES_V2: usize = 3 * RADIX_COEFFICIENTS_PER_GROUP_V2
    + PHASE23_MAIN_BLOCK_BYTES_V1
    + RADIX_COEFFICIENT_SCRATCH_BUDGET_BYTES_V2;

const RADIX_SOURCE_MAPPING_FORMULA_V2: &[u8] =
    b"source=(((record*8+group)*64+block)*256)+coefficient";
const RADIX_PACKING_MAPPING_FORMULA_V2: &[u8] =
    b"packing=((record*8+group)*16384)+(coefficient*64)+block";
const RADIX_SLOT_MAPPING_FORMULA_V2: &[u8] =
    b"slot=((record*8+group)*3)+lane;lane-order=packed0,packed1,packed2";
const RADIX_PACKED_LANE_FORMULA_V2: &[u8] =
    b"lane0=(bD,bS,beta0..5);lane1=(beta6..13);lane2=(beta14..17,m,0,0,0);least-significant-bit-first";
const RADIX_DECOMPOSITION_FORMULA_V2: &[u8] =
    b"D=sum(h=0..16,2^(15h)*d_h)+2^255*bD;S=pT-1-D=sum(h=0..16,2^(15h)*s_h)+2^255*bS";
const RADIX_COMPARATOR_FORMULA_V2: &[u8] =
    b"K=(pT-1)/2+1;fixed-h=0..16-subtraction-borrows;m=bD*beta16;beta17=beta16-m;beta17=(D<K)";
const RADIX_WITNESS_MAPPING_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.phase23.radix-witness.mapping\0";
const RADIX_WITNESS_CONTEXT_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.phase23.radix-witness.spool-context\0";
const RADIX_WITNESS_RECORD_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.phase23.radix-witness.materialization-record\0";
const RADIX_WITNESS_SEAL_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.phase23.radix-witness.materialization-seal\0";

const AUTHENTICATED_CANONICAL_REREAD_COMPLETE_V2: bool = true;
const COMPACT_RADIX_WITNESS_MATERIALIZED_V2: bool = true;
const COMMITMENTS_CONSTRUCTED_V2: bool = false;
const TRANSCRIPT_BOUND_V2: bool = false;
const FINAL_ARITHMETIC_PLANE_CONSTRUCTED_V2: bool = false;
const RADIX_PROOF_VERIFIED_V2: bool = false;
const ZERO_KNOWLEDGE_ACCEPTED_V2: bool = false;
const AUTHORITY_MINTED_V2: bool = false;
const RSS_QUALIFIED_V2: bool = false;
const OPERATIONAL_RECEIPT_ACCEPTED_V2: bool = false;
const RELEASE_READY_V2: bool = false;
const RELEASE_COMPLETE_V2: bool = false;

const fn decrement_be_v2(mut value: [u8; 32]) -> [u8; 32] {
    let mut borrow = 1_u16;
    let mut offset = 0_usize;
    while offset < value.len() {
        let index = value.len() - 1 - offset;
        let byte = value[index] as u16;
        value[index] = byte.wrapping_sub(borrow) as u8;
        borrow = if byte < borrow { 1 } else { 0 };
        offset += 1;
    }
    value
}

const fn centering_threshold_be_v2() -> [u8; 32] {
    let mut threshold = [0_u8; 32];
    let mut incoming = 0_u8;
    let mut index = 0_usize;
    while index < threshold.len() {
        let byte = VEGA_T256_SCALAR_MODULUS_BE_V1[index];
        threshold[index] = (incoming << 7) | (byte >> 1);
        incoming = byte & 1;
        index += 1;
    }
    let mut carry = 1_u16;
    let mut offset = 0_usize;
    while offset < threshold.len() {
        let index = threshold.len() - 1 - offset;
        let sum = threshold[index] as u16 + carry;
        threshold[index] = sum as u8;
        carry = sum >> 8;
        offset += 1;
    }
    threshold
}

const RADIX_MODULUS_MINUS_ONE_BE_V2: [u8; 32] = decrement_be_v2(VEGA_T256_SCALAR_MODULUS_BE_V1);
const RADIX_CENTERING_THRESHOLD_BE_V2: [u8; 32] = centering_threshold_be_v2();

const _: () = {
    assert!(RADIX_GROUP_COUNT_V2 == 344);
    assert!(RADIX_WITNESS_SLOT_COUNT_V2 == 1_032);
    assert!(RADIX_SOURCE_BLOCKS_PER_GROUP_V2 * RADIX_SOURCE_COEFFICIENTS_PER_BLOCK_V2 == 16_384);
    assert!(RADIX_GROUPS_PER_RECORD_V2 * RADIX_COEFFICIENTS_PER_GROUP_V2 == PHASE23_RING_DEGREE_V1);
    assert!(PHASE23_CANONICAL_COEFFICIENTS_PER_BLOCK_V1 == RADIX_SOURCE_COEFFICIENTS_PER_BLOCK_V2);
    assert!(RADIX_WITNESS_PLAINTEXT_BYTES_V2 == 16_908_288);
    assert!(RADIX_WITNESS_AUTHENTICATION_TAG_BYTES_V2 == 16_512);
    assert!(RADIX_WITNESS_FILE_BYTES_V2 == 16_924_800);
    assert!(RADIX_WITNESS_SPOOL_IO_BYTES_V2 == 33_849_600);
    assert!(RADIX_SOURCE_REREAD_BLOCKS_V2 == 43 * 512);
    assert!(RADIX_SOURCE_REREAD_PLAINTEXT_BYTES_V2 == 180_355_072);
    assert!(RADIX_SOURCE_REREAD_AUTHENTICATED_BYTES_V2 == 180_707_328);
    assert!(RADIX_WITNESS_TOTAL_IO_BYTES_V2 == 214_556_928);
    assert!(RADIX_WITNESS_NAMED_LIVE_PAYLOAD_BYTES_V2 <= 64 * 1_024);
    assert!(AUTHENTICATED_CANONICAL_REREAD_COMPLETE_V2);
    assert!(COMPACT_RADIX_WITNESS_MATERIALIZED_V2);
    assert!(!COMMITMENTS_CONSTRUCTED_V2);
    assert!(!TRANSCRIPT_BOUND_V2);
    assert!(!FINAL_ARITHMETIC_PLANE_CONSTRUCTED_V2);
    assert!(!RADIX_PROOF_VERIFIED_V2);
    assert!(!ZERO_KNOWLEDGE_ACCEPTED_V2);
    assert!(!AUTHORITY_MINTED_V2);
    assert!(!RSS_QUALIFIED_V2);
    assert!(!OPERATIONAL_RECEIPT_ACCEPTED_V2);
    assert!(!RELEASE_READY_V2);
    assert!(!RELEASE_COMPLETE_V2);
};

struct RadixWitnessCoordinateV2 {
    record: u16,
    family: u8,
    group: u8,
    source_block: u8,
    coefficient: u16,
    source_index: u32,
    packing_index: u32,
    first_slot: u16,
}

fn radix_witness_coordinate_v2(
    record: usize,
    group: usize,
    source_block: usize,
    coefficient: usize,
) -> Result<RadixWitnessCoordinateV2, ZkAmsMkheErrorV1> {
    if record >= PHASE23_RECORD_COUNT_V1
        || group >= RADIX_GROUPS_PER_RECORD_V2
        || source_block >= RADIX_SOURCE_BLOCKS_PER_GROUP_V2
        || coefficient >= RADIX_SOURCE_COEFFICIENTS_PER_BLOCK_V2
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let record = u16::try_from(record).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let position = phase23_record_position_v1(record)?;
    let group_base = (usize::from(record) * RADIX_GROUPS_PER_RECORD_V2 + group)
        * RADIX_COEFFICIENTS_PER_GROUP_V2;
    let source_index =
        group_base + source_block * RADIX_SOURCE_COEFFICIENTS_PER_BLOCK_V2 + coefficient;
    let packing_index = group_base + coefficient * RADIX_SOURCE_BLOCKS_PER_GROUP_V2 + source_block;
    let first_slot = (usize::from(record) * RADIX_GROUPS_PER_RECORD_V2 + group)
        * RADIX_PACKED_LANES_PER_GROUP_V2;
    Ok(RadixWitnessCoordinateV2 {
        record,
        family: position.family as u8,
        group: u8::try_from(group).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        source_block: u8::try_from(source_block)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        coefficient: u16::try_from(coefficient)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        source_index: u32::try_from(source_index)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        packing_index: u32::try_from(packing_index)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        first_slot: u16::try_from(first_slot)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
    })
}

fn radix_witness_slot_v2(
    record: usize,
    group: usize,
    lane: usize,
) -> Result<u64, ZkAmsMkheErrorV1> {
    if record >= PHASE23_RECORD_COUNT_V1
        || group >= RADIX_GROUPS_PER_RECORD_V2
        || lane >= RADIX_PACKED_LANES_PER_GROUP_V2
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    u64::try_from(
        (record * RADIX_GROUPS_PER_RECORD_V2 + group) * RADIX_PACKED_LANES_PER_GROUP_V2 + lane,
    )
    .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)
}

fn radix_witness_packing_index_v2(
    source_block: usize,
    coefficient: usize,
) -> Result<usize, ZkAmsMkheErrorV1> {
    if source_block >= RADIX_SOURCE_BLOCKS_PER_GROUP_V2
        || coefficient >= RADIX_SOURCE_COEFFICIENTS_PER_BLOCK_V2
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(coefficient * RADIX_SOURCE_BLOCKS_PER_GROUP_V2 + source_block)
}

fn exact_radix_witness_mapping_digest_v2() -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(RADIX_WITNESS_MAPPING_DOMAIN_V2);
    hash.update(&[RADIX_WITNESS_VERSION_V2]);
    for value in [
        PHASE23_RECORD_COUNT_V1 as u32,
        RADIX_GROUPS_PER_RECORD_V2 as u32,
        RADIX_SOURCE_BLOCKS_PER_GROUP_V2 as u32,
        RADIX_SOURCE_COEFFICIENTS_PER_BLOCK_V2 as u32,
        RADIX_COEFFICIENTS_PER_GROUP_V2 as u32,
        RADIX_PACKED_LANES_PER_GROUP_V2 as u32,
        RADIX_WITNESS_SLOT_COUNT_V2 as u32,
    ] {
        hash.update(&value.to_be_bytes());
    }
    for formula in [
        RADIX_SOURCE_MAPPING_FORMULA_V2,
        RADIX_PACKING_MAPPING_FORMULA_V2,
        RADIX_SLOT_MAPPING_FORMULA_V2,
        RADIX_PACKED_LANE_FORMULA_V2,
        RADIX_DECOMPOSITION_FORMULA_V2,
        RADIX_COMPARATOR_FORMULA_V2,
    ] {
        hash.update(&(formula.len() as u16).to_be_bytes());
        hash.update(formula);
    }
    for record in 0..PHASE23_RECORD_COUNT_V1 {
        let ordinal =
            u16::try_from(record).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let position = phase23_record_position_v1(ordinal)?;
        hash.update(&ordinal.to_be_bytes());
        hash.update(&[position.family as u8]);
        hash.update(&position.chunk_index.to_be_bytes());
        hash.update(&position.family_chunk_count.to_be_bytes());
        hash.update(&position.logical_value_count.to_be_bytes());
        for group in 0..RADIX_GROUPS_PER_RECORD_V2 {
            for source_block in 0..RADIX_SOURCE_BLOCKS_PER_GROUP_V2 {
                for coefficient in 0..RADIX_SOURCE_COEFFICIENTS_PER_BLOCK_V2 {
                    let coordinate =
                        radix_witness_coordinate_v2(record, group, source_block, coefficient)?;
                    hash.update(&coordinate.record.to_be_bytes());
                    hash.update(&[coordinate.family, coordinate.group, coordinate.source_block]);
                    hash.update(&coordinate.coefficient.to_be_bytes());
                    hash.update(&coordinate.source_index.to_be_bytes());
                    hash.update(&coordinate.packing_index.to_be_bytes());
                    hash.update(&coordinate.first_slot.to_be_bytes());
                }
            }
        }
    }
    require_nonzero_radix_digest_v2(hash.finalize())
}

fn radix_witness_context_digest_v2(
    replay_record_digest: [u8; 32],
    source_receipt_digest: [u8; 32],
    mapping_digest: [u8; 32],
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(RADIX_WITNESS_CONTEXT_DOMAIN_V2);
    hash.update(&[RADIX_WITNESS_VERSION_V2]);
    for digest in [replay_record_digest, source_receipt_digest, mapping_digest] {
        hash.update(&require_nonzero_radix_digest_v2(digest)?);
    }
    hash.update(&(RADIX_WITNESS_SLOT_COUNT_V2 as u64).to_be_bytes());
    hash.update(&RADIX_WITNESS_SLOT_PLAINTEXT_BYTES_V2.to_be_bytes());
    hash.update(&RADIX_WITNESS_FILE_BYTES_V2.to_be_bytes());
    require_nonzero_radix_digest_v2(hash.finalize())
}

#[cfg(test)]
static RADIX_SECRET_BYTE_DROPS_V2: AtomicUsize = AtomicUsize::new(0);
#[cfg(test)]
static RADIX_COEFFICIENT_WITNESS_DROPS_V2: AtomicUsize = AtomicUsize::new(0);
#[cfg(test)]
static RADIX_PACKED_COMPARATOR_DROPS_V2: AtomicUsize = AtomicUsize::new(0);
#[cfg(test)]
static RADIX_SECRET_COPY_DROPS_V2: AtomicUsize = AtomicUsize::new(0);

trait RadixSecretCopyValueV2: Copy {
    fn zeroize_v2(&mut self);
}

impl RadixSecretCopyValueV2 for u8 {
    fn zeroize_v2(&mut self) {
        *self = 0;
    }
}

impl RadixSecretCopyValueV2 for u16 {
    fn zeroize_v2(&mut self) {
        *self = 0;
    }
}

struct RadixSecretCopyV2<T: RadixSecretCopyValueV2>(T);

impl<T: RadixSecretCopyValueV2> RadixSecretCopyV2<T> {
    fn new(mut value: T) -> Self {
        let owned = Self(value);
        value.zeroize_v2();
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut value);
        owned
    }

    fn as_ref_v2(&self) -> &T {
        &self.0
    }

    fn replace_v2(&mut self, mut value: T) {
        self.0.zeroize_v2();
        self.0 = value;
        value.zeroize_v2();
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut value);
    }
}

impl RadixSecretCopyV2<u16> {
    fn or_assign_v2(&mut self, mut value: u16) {
        self.0 |= value;
        value.zeroize_v2();
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut value);
    }
}

impl RadixSecretCopyV2<u8> {
    fn or_assign_v2(&mut self, mut value: u8) {
        self.0 |= value;
        value.zeroize_v2();
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut value);
    }
}

impl<T: RadixSecretCopyValueV2> Drop for RadixSecretCopyV2<T> {
    fn drop(&mut self) {
        self.0.zeroize_v2();
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut self.0);
        #[cfg(test)]
        RADIX_SECRET_COPY_DROPS_V2.fetch_add(1, Ordering::SeqCst);
    }
}

struct RadixSecretBytesV2([u8; 32]);

impl RadixSecretBytesV2 {
    fn zeroed_v2() -> Self {
        Self([0; 32])
    }

    fn as_ref_v2(&self) -> &[u8; 32] {
        &self.0
    }

    fn as_mut_v2(&mut self) -> &mut [u8; 32] {
        &mut self.0
    }
}

impl Drop for RadixSecretBytesV2 {
    fn drop(&mut self) {
        self.0.fill(0);
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut self.0);
        #[cfg(test)]
        RADIX_SECRET_BYTE_DROPS_V2.fetch_add(1, Ordering::SeqCst);
    }
}

struct RadixCoefficientWitnessV2 {
    slack: RadixSecretBytesV2,
    d_low: [u16; RADIX_LOW_LIMBS_V2],
    s_low: [u16; RADIX_LOW_LIMBS_V2],
    beta: [u8; RADIX_COMPARATOR_BITS_V2],
    b_d: u8,
    b_s: u8,
    m: u8,
}

impl RadixCoefficientWitnessV2 {
    fn zeroed_v2() -> Self {
        Self {
            slack: RadixSecretBytesV2::zeroed_v2(),
            d_low: [0; RADIX_LOW_LIMBS_V2],
            s_low: [0; RADIX_LOW_LIMBS_V2],
            beta: [0; RADIX_COMPARATOR_BITS_V2],
            b_d: 0,
            b_s: 0,
            m: 0,
        }
    }
}

impl Drop for RadixCoefficientWitnessV2 {
    fn drop(&mut self) {
        self.d_low.fill(0);
        self.s_low.fill(0);
        self.beta.fill(0);
        self.b_d = 0;
        self.b_s = 0;
        self.m = 0;
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut self.d_low);
        let _ = core::hint::black_box(&mut self.s_low);
        let _ = core::hint::black_box(&mut self.beta);
        let _ = core::hint::black_box(&mut self.b_d);
        let _ = core::hint::black_box(&mut self.b_s);
        let _ = core::hint::black_box(&mut self.m);
        #[cfg(test)]
        RADIX_COEFFICIENT_WITNESS_DROPS_V2.fetch_add(1, Ordering::SeqCst);
    }
}

struct RadixPackedComparatorV2([u8; RADIX_PACKED_LANES_PER_GROUP_V2]);

impl RadixPackedComparatorV2 {
    fn as_ref_v2(&self) -> &[u8; RADIX_PACKED_LANES_PER_GROUP_V2] {
        &self.0
    }
}

impl Drop for RadixPackedComparatorV2 {
    fn drop(&mut self) {
        self.0.fill(0);
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut self.0);
        #[cfg(test)]
        RADIX_PACKED_COMPARATOR_DROPS_V2.fetch_add(1, Ordering::SeqCst);
    }
}

const _: () = {
    assert!(
        core::mem::size_of::<RadixCoefficientWitnessV2>()
            + RADIX_COEFFICIENT_BYTE_OWNERS_V2 * 32
            + RADIX_COEFFICIENT_PUBLIC_THRESHOLD_BYTES_V2
            + RADIX_COEFFICIENT_COPY_OWNER_ALLOWANCE_BYTES_V2
            <= RADIX_COEFFICIENT_SCRATCH_BUDGET_BYTES_V2
    );
};

fn fixed_subtract_be_v2(
    left: &[u8; 32],
    right: &[u8; 32],
    output: &mut [u8; 32],
) -> RadixSecretCopyV2<u8> {
    let mut borrow = RadixSecretCopyV2::new(0_u16);
    for offset in 0..32 {
        let index = 31 - offset;
        let left_byte = RadixSecretCopyV2::new(u16::from(left[index]));
        let right_byte = RadixSecretCopyV2::new(u16::from(right[index]) + *borrow.as_ref_v2());
        output[index] = left_byte.as_ref_v2().wrapping_sub(*right_byte.as_ref_v2()) as u8;
        let next_borrow = RadixSecretCopyV2::new(u16::from(u8::from(
            left_byte.as_ref_v2() < right_byte.as_ref_v2(),
        )));
        borrow.replace_v2(*next_borrow.as_ref_v2());
    }
    RadixSecretCopyV2::new(*borrow.as_ref_v2() as u8)
}

fn fixed_add_be_v2(
    left: &[u8; 32],
    right: &[u8; 32],
    output: &mut [u8; 32],
) -> RadixSecretCopyV2<u8> {
    let mut carry = RadixSecretCopyV2::new(0_u16);
    for offset in 0..32 {
        let index = 31 - offset;
        let sum = RadixSecretCopyV2::new(
            u16::from(left[index]) + u16::from(right[index]) + *carry.as_ref_v2(),
        );
        output[index] = *sum.as_ref_v2() as u8;
        carry.replace_v2(*sum.as_ref_v2() >> 8);
    }
    RadixSecretCopyV2::new(*carry.as_ref_v2() as u8)
}

fn fixed_equal_bytes_v2(left: &[u8; 32], right: &[u8; 32]) -> RadixSecretCopyV2<u8> {
    let mut difference = RadixSecretCopyV2::new(0_u8);
    for index in 0..32 {
        difference.or_assign_v2(left[index] ^ right[index]);
    }
    RadixSecretCopyV2::new(u8::from(*difference.as_ref_v2() == 0))
}

fn fixed_less_than_be_v2(left: &[u8; 32], right: &[u8; 32]) -> RadixSecretCopyV2<u8> {
    let mut difference = RadixSecretBytesV2::zeroed_v2();
    fixed_subtract_be_v2(left, right, difference.as_mut_v2())
}

fn bit_le_from_be_v2(bytes: &[u8; 32], bit: usize) -> RadixSecretCopyV2<u8> {
    let byte = 31 - bit / 8;
    RadixSecretCopyV2::new((bytes[byte] >> (bit % 8)) & 1)
}

fn extract_radix_digits_v2(
    bytes: &[u8; 32],
    low: &mut [u16; RADIX_LOW_LIMBS_V2],
) -> RadixSecretCopyV2<u8> {
    for (limb, destination) in low.iter_mut().enumerate() {
        let mut digit = RadixSecretCopyV2::new(0_u16);
        for bit in 0..15 {
            let source_bit = bit_le_from_be_v2(bytes, limb * 15 + bit);
            digit.or_assign_v2(u16::from(*source_bit.as_ref_v2()) << bit);
        }
        *destination = *digit.as_ref_v2();
    }
    bit_le_from_be_v2(bytes, 255)
}

fn reconstruct_radix_v2(
    low: &[u16; RADIX_LOW_LIMBS_V2],
    top: &u8,
) -> Result<RadixSecretBytesV2, ZkAmsMkheErrorV1> {
    let mut reconstructed = RadixSecretBytesV2::zeroed_v2();
    let mut invalid = RadixSecretCopyV2::new(*top >> 1);
    for (limb, digit) in low.iter().enumerate() {
        invalid.or_assign_v2(u8::from(*digit >= RADIX_BASE_V2));
        for bit in 0..15 {
            let absolute_bit = limb * 15 + bit;
            let byte = 31 - absolute_bit / 8;
            reconstructed.as_mut_v2()[byte] |= ((*digit >> bit) as u8 & 1) << (absolute_bit % 8);
        }
    }
    reconstructed.as_mut_v2()[0] |= (*top & 1) << 7;
    if *invalid.as_ref_v2() != 0 {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(reconstructed)
}

fn radix_coefficient_witness_v2(
    encoded: &[u8; 32],
) -> Result<RadixCoefficientWitnessV2, ZkAmsMkheErrorV1> {
    let mut witness = RadixCoefficientWitnessV2::zeroed_v2();
    let mut canonical_difference = RadixSecretBytesV2::zeroed_v2();
    let canonical_borrow = fixed_subtract_be_v2(
        encoded,
        &VEGA_T256_SCALAR_MODULUS_BE_V1,
        canonical_difference.as_mut_v2(),
    );
    if *canonical_borrow.as_ref_v2() != 1 {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let complement_borrow = fixed_subtract_be_v2(
        &RADIX_MODULUS_MINUS_ONE_BE_V2,
        encoded,
        witness.slack.as_mut_v2(),
    );
    if *complement_borrow.as_ref_v2() != 0 {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }

    let b_d = extract_radix_digits_v2(encoded, &mut witness.d_low);
    witness.b_d = *b_d.as_ref_v2();
    let b_s = extract_radix_digits_v2(witness.slack.as_ref_v2(), &mut witness.s_low);
    witness.b_s = *b_s.as_ref_v2();
    let reconstructed_d = reconstruct_radix_v2(&witness.d_low, &witness.b_d)?;
    let reconstructed_s = reconstruct_radix_v2(&witness.s_low, &witness.b_s)?;
    let mut reconstructed_sum = RadixSecretBytesV2::zeroed_v2();
    let sum_carry = fixed_add_be_v2(
        reconstructed_d.as_ref_v2(),
        reconstructed_s.as_ref_v2(),
        reconstructed_sum.as_mut_v2(),
    );
    let d_matches = fixed_equal_bytes_v2(reconstructed_d.as_ref_v2(), encoded);
    let s_matches = fixed_equal_bytes_v2(reconstructed_s.as_ref_v2(), witness.slack.as_ref_v2());
    let sum_matches = fixed_equal_bytes_v2(
        reconstructed_sum.as_ref_v2(),
        &RADIX_MODULUS_MINUS_ONE_BE_V2,
    );
    if *d_matches.as_ref_v2() != 1
        || *s_matches.as_ref_v2() != 1
        || *sum_carry.as_ref_v2() != 0
        || *sum_matches.as_ref_v2() != 1
        || witness.b_d > 1
        || witness.b_s > 1
        || witness.b_d * witness.b_s != 0
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }

    let mut threshold_low = [0_u16; RADIX_LOW_LIMBS_V2];
    let threshold_top =
        extract_radix_digits_v2(&RADIX_CENTERING_THRESHOLD_BE_V2, &mut threshold_low);
    let mut prior_borrow = RadixSecretCopyV2::new(0_u16);
    let mut invalid = RadixSecretCopyV2::new(u16::from(u8::from(*threshold_top.as_ref_v2() != 0)));
    for (limb, (&threshold, &digit)) in threshold_low.iter().zip(&witness.d_low).enumerate() {
        let right = RadixSecretCopyV2::new(threshold + *prior_borrow.as_ref_v2());
        let borrow = RadixSecretCopyV2::new(u16::from(u8::from(digit < *right.as_ref_v2())));
        let delta = RadixSecretCopyV2::new(
            digit + RADIX_BASE_V2 * *borrow.as_ref_v2() - *right.as_ref_v2(),
        );
        invalid.or_assign_v2(u16::from(u8::from(*delta.as_ref_v2() >= RADIX_BASE_V2)));
        invalid.or_assign_v2(u16::from(u8::from(
            digit + RADIX_BASE_V2 * *borrow.as_ref_v2() != *right.as_ref_v2() + *delta.as_ref_v2(),
        )));
        witness.beta[limb] = *borrow.as_ref_v2() as u8;
        prior_borrow.replace_v2(*borrow.as_ref_v2());
    }
    witness.m = witness.b_d * witness.beta[16];
    witness.beta[17] = witness.beta[16]
        .checked_sub(witness.m)
        .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
    for beta in &witness.beta {
        invalid.or_assign_v2(u16::from(u8::from(*beta > 1)));
    }
    invalid.or_assign_v2(u16::from(u8::from(witness.m > 1)));
    invalid.or_assign_v2(u16::from(u8::from(
        witness.m != witness.b_d * witness.beta[16],
    )));
    invalid.or_assign_v2(u16::from(u8::from(
        witness.beta[17] != witness.beta[16] - witness.m,
    )));
    let less_than_threshold = fixed_less_than_be_v2(encoded, &RADIX_CENTERING_THRESHOLD_BE_V2);
    invalid.or_assign_v2(u16::from(u8::from(
        witness.beta[17] != *less_than_threshold.as_ref_v2(),
    )));
    threshold_low.fill(0);
    if *invalid.as_ref_v2() != 0 {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(witness)
}

fn pack_comparator_lanes_v2(
    witness: &RadixCoefficientWitnessV2,
) -> Result<RadixPackedComparatorV2, ZkAmsMkheErrorV1> {
    let mut packed = RadixPackedComparatorV2([0; RADIX_PACKED_LANES_PER_GROUP_V2]);
    if witness.b_d > 1 || witness.b_s > 1 || witness.m > 1 {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    packed.0[0] = witness.b_d | (witness.b_s << 1);
    for bit in 0..6 {
        if witness.beta[bit] > 1 {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        packed.0[0] |= witness.beta[bit] << (bit + 2);
    }
    for bit in 0..8 {
        if witness.beta[bit + 6] > 1 {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        packed.0[1] |= witness.beta[bit + 6] << bit;
    }
    for bit in 0..4 {
        if witness.beta[bit + 14] > 1 {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        packed.0[2] |= witness.beta[bit + 14] << bit;
    }
    packed.0[2] |= witness.m << 4;
    if packed.0[2] & 0xe0 != 0 {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(packed)
}

/// Purpose-bound scratch authority. Production cannot construct it in this
/// slice; tests may supply only an unlinked confidential-spool directory.
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) enum Phase23RadixWitnessScratchSinkV2
{
    Production {
        confidential_spool_directory: Infallible,
    },
    #[cfg(test)]
    TestOnly(PathBuf),
}

impl Phase23RadixWitnessScratchSinkV2 {
    fn into_directory_v2(self) -> PathBuf {
        match self {
            Self::Production {
                confidential_spool_directory,
            } => match confidential_spool_directory {},
            #[cfg(test)]
            Self::TestOnly(directory) => directory,
        }
    }
}

struct RadixWitnessMaterializationRecordV2 {
    replay_record_digest: [u8; 32],
    source_receipt_digest: [u8; 32],
    mapping_digest: [u8; 32],
    spool_context_digest: [u8; 32],
    authenticated_read_schedule_root: [u8; 32],
    snapshot_root: [u8; 32],
    source_reread_blocks: u32,
    source_reread_plaintext_bytes: u64,
    source_reread_authenticated_bytes: u64,
    output_slot_count: u16,
    output_plaintext_bytes: u64,
    output_authentication_tag_bytes: u64,
    output_file_bytes: u64,
    output_spool_io_bytes: u64,
    total_io_bytes: u64,
    named_live_payload_bytes: u32,
    authenticated_canonical_reread_complete: bool,
    compact_radix_witness_materialized: bool,
    commitments_constructed: bool,
    transcript_bound: bool,
    final_arithmetic_plane_constructed: bool,
    radix_proof_verified: bool,
    zero_knowledge_accepted: bool,
    authority_minted: bool,
    rss_qualified: bool,
    operational_receipt_accepted: bool,
    release_ready: bool,
    release_complete: bool,
    record_digest: [u8; 32],
}

/// Non-authorizing public axes copied while Evidence is consumed into its
/// strict cursor. They have no constructor or accessor outside that transition.
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23)
struct Phase23RadixSourceCursorAxesV2
{
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) replay_record_digest:
        [u8; 32],
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) source_receipt_digest:
        [u8; 32],
}

fn radix_witness_record_digest_v2(
    record: &RadixWitnessMaterializationRecordV2,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(RADIX_WITNESS_RECORD_DOMAIN_V2);
    hash.update(&[RADIX_WITNESS_VERSION_V2]);
    for digest in [
        record.replay_record_digest,
        record.source_receipt_digest,
        record.mapping_digest,
        record.spool_context_digest,
        record.authenticated_read_schedule_root,
        record.snapshot_root,
    ] {
        hash.update(&require_nonzero_radix_digest_v2(digest)?);
    }
    hash.update(&record.source_reread_blocks.to_be_bytes());
    hash.update(&record.source_reread_plaintext_bytes.to_be_bytes());
    hash.update(&record.source_reread_authenticated_bytes.to_be_bytes());
    hash.update(&record.output_slot_count.to_be_bytes());
    hash.update(&record.output_plaintext_bytes.to_be_bytes());
    hash.update(&record.output_authentication_tag_bytes.to_be_bytes());
    hash.update(&record.output_file_bytes.to_be_bytes());
    hash.update(&record.output_spool_io_bytes.to_be_bytes());
    hash.update(&record.total_io_bytes.to_be_bytes());
    hash.update(&record.named_live_payload_bytes.to_be_bytes());
    hash.update(&[
        record.authenticated_canonical_reread_complete as u8,
        record.compact_radix_witness_materialized as u8,
        record.commitments_constructed as u8,
        record.transcript_bound as u8,
        record.final_arithmetic_plane_constructed as u8,
        record.radix_proof_verified as u8,
        record.zero_knowledge_accepted as u8,
        record.authority_minted as u8,
        record.rss_qualified as u8,
        record.operational_receipt_accepted as u8,
        record.release_ready as u8,
        record.release_complete as u8,
    ]);
    require_nonzero_radix_digest_v2(hash.finalize())
}

fn validate_radix_witness_record_v2(
    record: &RadixWitnessMaterializationRecordV2,
) -> Result<(), ZkAmsMkheErrorV1> {
    if [
        record.replay_record_digest,
        record.source_receipt_digest,
        record.mapping_digest,
        record.spool_context_digest,
        record.authenticated_read_schedule_root,
        record.snapshot_root,
        record.record_digest,
    ]
    .contains(&[0; 32])
        || record.source_reread_blocks != RADIX_SOURCE_REREAD_BLOCKS_V2 as u32
        || record.source_reread_plaintext_bytes != RADIX_SOURCE_REREAD_PLAINTEXT_BYTES_V2
        || record.source_reread_authenticated_bytes != RADIX_SOURCE_REREAD_AUTHENTICATED_BYTES_V2
        || usize::from(record.output_slot_count) != RADIX_WITNESS_SLOT_COUNT_V2
        || record.output_plaintext_bytes != RADIX_WITNESS_PLAINTEXT_BYTES_V2
        || record.output_authentication_tag_bytes != RADIX_WITNESS_AUTHENTICATION_TAG_BYTES_V2
        || record.output_file_bytes != RADIX_WITNESS_FILE_BYTES_V2
        || record.output_spool_io_bytes != RADIX_WITNESS_SPOOL_IO_BYTES_V2
        || record.total_io_bytes != RADIX_WITNESS_TOTAL_IO_BYTES_V2
        || record.named_live_payload_bytes != RADIX_WITNESS_NAMED_LIVE_PAYLOAD_BYTES_V2 as u32
        || !record.authenticated_canonical_reread_complete
        || !record.compact_radix_witness_materialized
        || record.commitments_constructed
        || record.transcript_bound
        || record.final_arithmetic_plane_constructed
        || record.radix_proof_verified
        || record.zero_knowledge_accepted
        || record.authority_minted
        || record.rss_qualified
        || record.operational_receipt_accepted
        || record.release_ready
        || record.release_complete
        || record.record_digest != radix_witness_record_digest_v2(record)?
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(())
}

/// Unforgeable proof-order seal. Only successful materialization can mint it.
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23)
struct RadixWitnessMaterializationSealV2
{
    replay_record_digest: [u8; 32],
    spool_context_digest: [u8; 32],
    snapshot_root: [u8; 32],
    materialization_record_digest: [u8; 32],
    seal_digest: [u8; 32],
}

impl RadixWitnessMaterializationSealV2 {
    fn mint_v2(
        replay_record_digest: [u8; 32],
        spool_context_digest: [u8; 32],
        snapshot_root: [u8; 32],
        materialization_record_digest: [u8; 32],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let seal_digest = radix_witness_seal_digest_v2(
            replay_record_digest,
            spool_context_digest,
            snapshot_root,
            materialization_record_digest,
        )?;
        Ok(Self {
            replay_record_digest,
            spool_context_digest,
            snapshot_root,
            materialization_record_digest,
            seal_digest,
        })
    }

    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn validate_for_replay_v2(
        &self,
        replay_record_digest: [u8; 32],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        if replay_record_digest != self.replay_record_digest
            || self.seal_digest
                != radix_witness_seal_digest_v2(
                    self.replay_record_digest,
                    self.spool_context_digest,
                    self.snapshot_root,
                    self.materialization_record_digest,
                )?
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Ok(())
    }

    fn validate_for_materialized_record_v2(
        &self,
        record: &RadixWitnessMaterializationRecordV2,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        if self.replay_record_digest != record.replay_record_digest
            || self.spool_context_digest != record.spool_context_digest
            || self.snapshot_root != record.snapshot_root
            || self.materialization_record_digest != record.record_digest
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        self.validate_for_replay_v2(record.replay_record_digest)
    }
}

fn radix_witness_seal_digest_v2(
    replay_record_digest: [u8; 32],
    spool_context_digest: [u8; 32],
    snapshot_root: [u8; 32],
    materialization_record_digest: [u8; 32],
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(RADIX_WITNESS_SEAL_DOMAIN_V2);
    hash.update(&[RADIX_WITNESS_VERSION_V2]);
    for digest in [
        replay_record_digest,
        spool_context_digest,
        snapshot_root,
        materialization_record_digest,
    ] {
        hash.update(&require_nonzero_radix_digest_v2(digest)?);
    }
    require_nonzero_radix_digest_v2(hash.finalize())
}

/// Move-only compact witness owner. It deliberately has no snapshot, Evidence,
/// seal, commitment, proof, serialization, or tuple-splitting accessor.
#[must_use = "dropping this owner closes replay evidence and the radix witness spool"]
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23)
struct Phase23RadixWitnessMaterializedV2
<K, P> {
    evidence: Option<Phase23GlobalLookupSourceReplayEvidenceV1<K, P>>,
    snapshot: ConfidentialSpoolSnapshotV1,
    record: RadixWitnessMaterializationRecordV2,
    materialization_seal: RadixWitnessMaterializationSealV2,
}

struct RadixWitnessProofBindingV2<K, P> {
    materialized: Option<Phase23RadixWitnessMaterializedV2<K, P>>,
    radix_hyrax_proof: Option<RadixHyraxProofSealV2>,
}

struct Phase23RadixWitnessProofBoundV2<K, P> {
    replay: Phase23GlobalLookupSourceReplayV1<K, P>,
    witness_snapshot: ConfidentialSpoolSnapshotV1,
    witness_record: RadixWitnessMaterializationRecordV2,
}

/// Private future transition only: consume the entire compact materialized
/// owner and proof authority before validation. It is intentionally not exposed
/// by the materialized owner in this slice.
fn bind_materialized_radix_hyrax_replay_v2<K, P>(
    materialized: Phase23RadixWitnessMaterializedV2<K, P>,
    radix_hyrax_proof: RadixHyraxProofSealV2,
) -> Result<Phase23RadixWitnessProofBoundV2<K, P>, ZkAmsMkheErrorV1> {
    RadixWitnessProofBindingV2 {
        materialized: Some(materialized),
        radix_hyrax_proof: Some(radix_hyrax_proof),
    }
    .finish_v2()
}

impl<K, P> RadixWitnessProofBindingV2<K, P> {
    fn finish_v2(mut self) -> Result<Phase23RadixWitnessProofBoundV2<K, P>, ZkAmsMkheErrorV1> {
        let materialized = self
            .materialized
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let radix_hyrax_proof = self
            .radix_hyrax_proof
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let Phase23RadixWitnessMaterializedV2 {
            mut evidence,
            snapshot,
            record,
            materialization_seal,
        } = materialized;
        let evidence = evidence
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        validate_radix_witness_record_v2(&record)?;
        if snapshot.slot_count_v1() != RADIX_WITNESS_SLOT_COUNT_V2 as u64
            || snapshot.plaintext_len_v1() != RADIX_WITNESS_SLOT_PLAINTEXT_BYTES_V2
            || snapshot.file_len_v1() != RADIX_WITNESS_FILE_BYTES_V2
            || *snapshot.snapshot_digest_v1() != record.snapshot_root
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        materialization_seal.validate_for_materialized_record_v2(&record)?;
        let replay = bind_radix_hyrax_replay_after_materialization_v2(
            evidence,
            materialization_seal,
            radix_hyrax_proof,
        )?;
        Ok(Phase23RadixWitnessProofBoundV2 {
            replay,
            witness_snapshot: snapshot,
            witness_record: record,
        })
    }
}

fn materialize_radix_group_v2<K, P>(
    cursor: &mut Phase23GlobalLookupRadixSourceCursorV2<K, P>,
    writer: &mut ConfidentialSpoolWriterV1,
    record: usize,
    group: usize,
) -> Result<(), ZkAmsMkheErrorV1> {
    let mut lane0 = ConfidentialSpoolChunkV1::new_zeroed_v1(RADIX_WITNESS_SLOT_PLAINTEXT_BYTES_V2)
        .map_err(map_spool_error_v2)?;
    let mut lane1 = ConfidentialSpoolChunkV1::new_zeroed_v1(RADIX_WITNESS_SLOT_PLAINTEXT_BYTES_V2)
        .map_err(map_spool_error_v2)?;
    let mut lane2 = ConfidentialSpoolChunkV1::new_zeroed_v1(RADIX_WITNESS_SLOT_PLAINTEXT_BYTES_V2)
        .map_err(map_spool_error_v2)?;
    for local_block in 0..RADIX_SOURCE_BLOCKS_PER_GROUP_V2 {
        let block = group * RADIX_SOURCE_BLOCKS_PER_GROUP_V2 + local_block;
        let mut source = cursor.read_next_canonical_block_v2(record, block)?;
        let source_bytes = source.as_mut_bytes_v1();
        if source_bytes.len() != PHASE23_MAIN_BLOCK_BYTES_V1 {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        for coefficient in 0..RADIX_SOURCE_COEFFICIENTS_PER_BLOCK_V2 {
            let start = coefficient * 32;
            let encoded: &[u8; 32] = source_bytes[start..start + 32]
                .try_into()
                .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
            let witness = radix_coefficient_witness_v2(encoded)?;
            let packed = pack_comparator_lanes_v2(&witness)?;
            let destination = radix_witness_packing_index_v2(local_block, coefficient)?;
            lane0.as_mut_slice_v1()[destination] = packed.as_ref_v2()[0];
            lane1.as_mut_slice_v1()[destination] = packed.as_ref_v2()[1];
            lane2.as_mut_slice_v1()[destination] = packed.as_ref_v2()[2];
        }
    }
    writer
        .write_slot_v1(radix_witness_slot_v2(record, group, 0)?, lane0)
        .map_err(map_spool_error_v2)?;
    writer
        .write_slot_v1(radix_witness_slot_v2(record, group, 1)?, lane1)
        .map_err(map_spool_error_v2)?;
    writer
        .write_slot_v1(radix_witness_slot_v2(record, group, 2)?, lane2)
        .map_err(map_spool_error_v2)?;
    Ok(())
}

pub(in crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23) fn materialize_phase23_radix_witness_v2<
    K,
    P,
>(
    mut cursor: Phase23GlobalLookupRadixSourceCursorV2<K, P>,
    axes: Phase23RadixSourceCursorAxesV2,
    sink: Phase23RadixWitnessScratchSinkV2,
) -> Result<Phase23RadixWitnessMaterializedV2<K, P>, ZkAmsMkheErrorV1> {
    let replay_record_digest = axes.replay_record_digest;
    let source_receipt_digest = axes.source_receipt_digest;
    let mapping_digest = exact_radix_witness_mapping_digest_v2()?;
    let spool_context_digest = radix_witness_context_digest_v2(
        replay_record_digest,
        source_receipt_digest,
        mapping_digest,
    )?;
    let layout = ConfidentialSpoolLayoutV1::new_v1(
        RADIX_WITNESS_SLOT_COUNT_V2 as u64,
        RADIX_WITNESS_SLOT_PLAINTEXT_BYTES_V2,
        spool_context_digest,
    )
    .map_err(map_spool_error_v2)?;
    if layout.slot_count_v1() != RADIX_WITNESS_SLOT_COUNT_V2 as u64
        || layout.plaintext_len_v1() != RADIX_WITNESS_SLOT_PLAINTEXT_BYTES_V2
        || layout.file_len_v1() != RADIX_WITNESS_FILE_BYTES_V2
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let directory = sink.into_directory_v2();
    let mut writer =
        ConfidentialSpoolWriterV1::create_in_v1(&directory, layout).map_err(map_spool_error_v2)?;
    for record in 0..PHASE23_RECORD_COUNT_V1 {
        for group in 0..RADIX_GROUPS_PER_RECORD_V2 {
            materialize_radix_group_v2(&mut cursor, &mut writer, record, group)?;
        }
    }
    // This is the only restricted Evidence return from the strict cursor.
    let (evidence, authenticated_read_schedule_root) =
        cursor.complete_for_radix_materializer_v2()?;
    let snapshot = writer.seal_v1().map_err(map_spool_error_v2)?;
    if snapshot.slot_count_v1() != RADIX_WITNESS_SLOT_COUNT_V2 as u64
        || snapshot.plaintext_len_v1() != RADIX_WITNESS_SLOT_PLAINTEXT_BYTES_V2
        || snapshot.file_len_v1() != RADIX_WITNESS_FILE_BYTES_V2
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    let snapshot_root = require_nonzero_radix_digest_v2(*snapshot.snapshot_digest_v1())?;
    let mut record = RadixWitnessMaterializationRecordV2 {
        replay_record_digest,
        source_receipt_digest,
        mapping_digest,
        spool_context_digest,
        authenticated_read_schedule_root,
        snapshot_root,
        source_reread_blocks: RADIX_SOURCE_REREAD_BLOCKS_V2 as u32,
        source_reread_plaintext_bytes: RADIX_SOURCE_REREAD_PLAINTEXT_BYTES_V2,
        source_reread_authenticated_bytes: RADIX_SOURCE_REREAD_AUTHENTICATED_BYTES_V2,
        output_slot_count: RADIX_WITNESS_SLOT_COUNT_V2 as u16,
        output_plaintext_bytes: RADIX_WITNESS_PLAINTEXT_BYTES_V2,
        output_authentication_tag_bytes: RADIX_WITNESS_AUTHENTICATION_TAG_BYTES_V2,
        output_file_bytes: RADIX_WITNESS_FILE_BYTES_V2,
        output_spool_io_bytes: RADIX_WITNESS_SPOOL_IO_BYTES_V2,
        total_io_bytes: RADIX_WITNESS_TOTAL_IO_BYTES_V2,
        named_live_payload_bytes: RADIX_WITNESS_NAMED_LIVE_PAYLOAD_BYTES_V2 as u32,
        authenticated_canonical_reread_complete: AUTHENTICATED_CANONICAL_REREAD_COMPLETE_V2,
        compact_radix_witness_materialized: COMPACT_RADIX_WITNESS_MATERIALIZED_V2,
        commitments_constructed: COMMITMENTS_CONSTRUCTED_V2,
        transcript_bound: TRANSCRIPT_BOUND_V2,
        final_arithmetic_plane_constructed: FINAL_ARITHMETIC_PLANE_CONSTRUCTED_V2,
        radix_proof_verified: RADIX_PROOF_VERIFIED_V2,
        zero_knowledge_accepted: ZERO_KNOWLEDGE_ACCEPTED_V2,
        authority_minted: AUTHORITY_MINTED_V2,
        rss_qualified: RSS_QUALIFIED_V2,
        operational_receipt_accepted: OPERATIONAL_RECEIPT_ACCEPTED_V2,
        release_ready: RELEASE_READY_V2,
        release_complete: RELEASE_COMPLETE_V2,
        record_digest: [0; 32],
    };
    record.record_digest = radix_witness_record_digest_v2(&record)?;
    validate_radix_witness_record_v2(&record)?;
    let materialization_seal = RadixWitnessMaterializationSealV2::mint_v2(
        replay_record_digest,
        spool_context_digest,
        snapshot_root,
        record.record_digest,
    )?;
    materialization_seal.validate_for_materialized_record_v2(&record)?;
    Ok(Phase23RadixWitnessMaterializedV2 {
        evidence: Some(evidence),
        snapshot,
        record,
        materialization_seal,
    })
}

fn require_nonzero_radix_digest_v2(digest: [u8; 32]) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    (digest != [0; 32])
        .then_some(digest)
        .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)
}

fn map_spool_error_v2(
    _: iroha_crypto::confidential_spool::ConfidentialSpoolErrorV1,
) -> ZkAmsMkheErrorV1 {
    ZkAmsMkheErrorV1::InvalidPhase23Fold
}

#[cfg(test)]
#[path = "incremental_source_phase23_radix_range_v2_tests.rs"]
mod tests;
