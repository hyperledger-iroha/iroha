//! Authenticated multiplicity table and real statement-15 cubic oracle.
//!
//! The oracle evaluates the frozen lookup polynomial from independently
//! stored A, U, and M columns. It never treats a Boolean F table as its
//! multilinear extension. Production prefix and transcript capabilities are
//! deliberately uninhabited until the shared challenge typestate is wired.

#![allow(
    dead_code,
    reason = "production prefix/transcript capabilities remain uninhabited"
)]

use core::convert::Infallible;
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;

use super::super::ACTIVE_LOOKUP_VALUES_V1;
use super::*;

const M_ORACLE_VERSION_V1: u8 = 1;
const M_VALUES_V1: u64 = 1 << 15;
const M_INITIAL_SLOTS_V1: u64 = 128;
const M_INITIAL_FILE_BYTES_V1: u64 = 1_050_624;
const M_INITIAL_WRITE_AND_SEAL_IO_BYTES_V1: u64 = 2_101_248;
const M_COORDINATE_ROUND_READ_IO_BYTES_V1: u64 = 11_556_864;
const M_PLANE_ROUND_IO_BYTES_V1: u64 = 6_517_152;
const M_TOTAL_IO_BYTES_V1: u64 = 20_175_264;
const M_AUTHENTICATED_READS_V1: u64 = 1_932;
const M_NEXT_WRITE_AND_SEAL_RECORDS_V1: u64 = 270;
const M_SCALAR_FOLDS_V1: u64 = 32_767;
const COMBINED_AU_M_PEAK_FILE_BYTES_V1: u64 = 5_380_245_504;
const M_NAMED_CHUNK_HEAP_BYTES_V1: u64 = 24_576;
const ACTIVE_PLANES_V1: u64 = 31_768;
const TABLE_VALUES_V1: u64 = 32_768;
const EXTERNAL_FIRST_ROUND_V1: u8 = 3;
const FIRST_PLANE_ROUND_V1: u8 = 14;
const FINAL_ROUND_V1: u8 = 28;
const MASK_ROUNDS_V1: usize = 26;

const M_MAPPING_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.phase23.global-lookup.m-table.mapping\0";
const M_CONTEXT_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.phase23.global-lookup.m-table.context\0";
const M_LINEAGE_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.phase23.global-lookup.m-table.lineage\0";
const M_MANIFEST_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.global-lookup.m-table-oracle.manifest\0";
const M_MAPPING_LANGUAGE_V1: &[u8] = b"M(y)=canonical-u32-multiplicity-before-z;sum(M)=520486912;bits=y0..y14-little-endian;initial-width=32768;slot=floor(index/256);lane=index%256;canonical-T256-scalar-big-endian-32;coordinate-rounds3..13=M-unchanged;plane-rounds14..28=M-low+r*(M-high-M-low);final-unused-lanes-zero";
const ORACLE_LANGUAGE_V1: &[u8] = b"for-round-k=3..28,prefix=r0..r(k-1),suffix-boolean:chi=eq(rho,x);S=MLE(plane<31768);E0=prod-c(1-c);Qz=MLE_t((z-t)^-1);V=(z-A)U;F=alpha*chi*(V-S)+lambda*(U-E0*M*Qz)+mu*(E0*M-S);evaluate-current-line-at-t=0,1,2,3;sum-over-suffix;interpolate-cubic;require-g(0)+g(1)=base-claim;mask-Z=aT^3+bT^2+cT+(carry-a-b-c)/2;wire=(masked-constant,masked-quadratic,masked-cubic)-canonical-le;derive-nonzero-r-only-after-wire;fold-A,U,and-M-only-for-plane-rounds;final=(A*,U*,M*,R*=F(r),Z*=mask-carry)";
const ACCOUNTING_LANGUAGE_V1: &[u8] = b"M-initial=write+seal-read;coordinate-rounds=11-authenticated-M-full-reads;plane-round=evaluator-M-read+fold-M-read+next-write+seal-read;combined-file-peak=AU-exact-peak+live-initial-M;derived-context-chains-prior-lineage+parent-snapshot+generation;Qz-and-S-derived-not-stored;OS-page-cache,allocator,stack,AAD,cipher-state,handles-excluded";

const AUTHENTICATED_M_TABLE_COMPLETE_V1: bool = true;
const REAL_GLOBAL_CUBIC_ORACLE_COMPLETE_V1: bool = true;
const PREFIX_THREE_ROUNDS_WIRED_V1: bool = false;
const SHARED_TRANSCRIPT_WIRED_V1: bool = false;
const MASK_COMMITMENT_OPENING_WIRED_V1: bool = false;
const COMMITTED_MLE_OPENINGS_WIRED_V1: bool = false;
const PROOF_VERIFIED_V1: bool = false;
const ZERO_KNOWLEDGE_ACCEPTED_V1: bool = false;
const RECEIPT_ACCEPTED_V1: bool = false;
const RSS_QUALIFIED_V1: bool = false;
const RELEASE_READY_V1: bool = false;

const _: () = {
    assert!(M_VALUES_V1 == M_INITIAL_SLOTS_V1 * SCALARS_PER_SLOT_V1);
    assert!(M_INITIAL_FILE_BYTES_V1 == M_INITIAL_SLOTS_V1 * SLOT_CIPHERTEXT_BYTES_V1);
    assert!(
        M_TOTAL_IO_BYTES_V1
            == M_INITIAL_WRITE_AND_SEAL_IO_BYTES_V1
                + M_COORDINATE_ROUND_READ_IO_BYTES_V1
                + M_PLANE_ROUND_IO_BYTES_V1
    );
    assert!(MASK_ROUNDS_V1 == (GLOBAL_SUMCHECK_ROUNDS_V1 - STREAMING_PREFIX_ROUNDS_V1) as usize);
    assert!(AUTHENTICATED_M_TABLE_COMPLETE_V1 && REAL_GLOBAL_CUBIC_ORACLE_COMPLETE_V1);
    assert!(!PREFIX_THREE_ROUNDS_WIRED_V1);
    assert!(!SHARED_TRANSCRIPT_WIRED_V1);
    assert!(!MASK_COMMITMENT_OPENING_WIRED_V1);
    assert!(!COMMITTED_MLE_OPENINGS_WIRED_V1);
    assert!(!PROOF_VERIFIED_V1 && !ZERO_KNOWLEDGE_ACCEPTED_V1);
    assert!(!RECEIPT_ACCEPTED_V1 && !RSS_QUALIFIED_V1 && !RELEASE_READY_V1);
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MOracleErrorV1 {
    Shape,
    Order,
    Context,
    Arithmetic,
    Encoding,
    Relation,
    Spool,
}

fn map_parent_v1(error: ExternalStorageErrorV1) -> MOracleErrorV1 {
    match error {
        ExternalStorageErrorV1::Shape => MOracleErrorV1::Shape,
        ExternalStorageErrorV1::Order => MOracleErrorV1::Order,
        ExternalStorageErrorV1::Context => MOracleErrorV1::Context,
        ExternalStorageErrorV1::Arithmetic => MOracleErrorV1::Arithmetic,
        ExternalStorageErrorV1::Encoding => MOracleErrorV1::Encoding,
        ExternalStorageErrorV1::Spool => MOracleErrorV1::Spool,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MDescriptorV1 {
    completed_plane_rounds: u8,
    value_count: u64,
    slot_count: u64,
    file_bytes: u64,
    mapping_digest: [u8; 32],
}

fn m_descriptor_v1(completed_plane_rounds: u8) -> Result<MDescriptorV1, MOracleErrorV1> {
    if completed_plane_rounds > 15 {
        return Err(MOracleErrorV1::Shape);
    }
    let value_count = M_VALUES_V1 >> completed_plane_rounds;
    let slot_count = value_count.div_ceil(SCALARS_PER_SLOT_V1);
    let file_bytes = slot_count
        .checked_mul(SLOT_CIPHERTEXT_BYTES_V1)
        .ok_or(MOracleErrorV1::Arithmetic)?;
    let mut descriptor = MDescriptorV1 {
        completed_plane_rounds,
        value_count,
        slot_count,
        file_bytes,
        mapping_digest: [0; 32],
    };
    descriptor.mapping_digest = m_mapping_digest_v1(&descriptor)?;
    Ok(descriptor)
}

fn m_valid_lanes_v1(descriptor: &MDescriptorV1, slot: u64) -> Result<usize, MOracleErrorV1> {
    if slot >= descriptor.slot_count {
        return Err(MOracleErrorV1::Shape);
    }
    let first = slot
        .checked_mul(SCALARS_PER_SLOT_V1)
        .ok_or(MOracleErrorV1::Arithmetic)?;
    Ok(
        usize::try_from((descriptor.value_count - first).min(SCALARS_PER_SLOT_V1))
            .map_err(|_| MOracleErrorV1::Arithmetic)?,
    )
}

fn m_mapping_digest_v1(descriptor: &MDescriptorV1) -> Result<[u8; 32], MOracleErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(M_MAPPING_DOMAIN_V1);
    hash.update(&[M_ORACLE_VERSION_V1, GLOBAL_STATEMENT_ORDINAL_V1]);
    hash.update(&global_lookup_topology_digest_v1());
    hash.update(&[descriptor.completed_plane_rounds]);
    for value in [
        descriptor.value_count,
        descriptor.slot_count,
        SCALARS_PER_SLOT_V1,
        SLOT_PLAINTEXT_BYTES_V1,
        SLOT_CIPHERTEXT_BYTES_V1,
        descriptor.file_bytes,
    ] {
        hash.update(&value.to_be_bytes());
    }
    hash.update(&(M_MAPPING_LANGUAGE_V1.len() as u16).to_be_bytes());
    hash.update(M_MAPPING_LANGUAGE_V1);
    for slot in 0..descriptor.slot_count {
        hash.update(&slot.to_be_bytes());
        hash.update(&(slot * SCALARS_PER_SLOT_V1).to_be_bytes());
        hash.update(&(m_valid_lanes_v1(descriptor, slot)? as u16).to_be_bytes());
    }
    let digest = hash.finalize();
    (digest != [0; 32])
        .then_some(digest)
        .ok_or(MOracleErrorV1::Context)
}

fn m_manifest_digest_v1() -> Result<[u8; 32], MOracleErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(M_MANIFEST_DOMAIN_V1);
    hash.update(&[M_ORACLE_VERSION_V1, GLOBAL_STATEMENT_ORDINAL_V1]);
    hash.update(&global_lookup_topology_digest_v1());
    hash.update(&manifest_digest_v1().map_err(map_parent_v1)?);
    for value in [
        M_VALUES_V1,
        M_INITIAL_SLOTS_V1,
        M_INITIAL_FILE_BYTES_V1,
        M_INITIAL_WRITE_AND_SEAL_IO_BYTES_V1,
        M_COORDINATE_ROUND_READ_IO_BYTES_V1,
        M_PLANE_ROUND_IO_BYTES_V1,
        M_TOTAL_IO_BYTES_V1,
        M_AUTHENTICATED_READS_V1,
        M_NEXT_WRITE_AND_SEAL_RECORDS_V1,
        M_SCALAR_FOLDS_V1,
        COMBINED_AU_M_PEAK_FILE_BYTES_V1,
        M_NAMED_CHUNK_HEAP_BYTES_V1,
    ] {
        hash.update(&value.to_be_bytes());
    }
    for language in [
        M_MAPPING_LANGUAGE_V1,
        ORACLE_LANGUAGE_V1,
        ACCOUNTING_LANGUAGE_V1,
    ] {
        hash.update(&(language.len() as u16).to_be_bytes());
        hash.update(language);
    }
    hash.update(&[
        AUTHENTICATED_M_TABLE_COMPLETE_V1 as u8,
        REAL_GLOBAL_CUBIC_ORACLE_COMPLETE_V1 as u8,
        PREFIX_THREE_ROUNDS_WIRED_V1 as u8,
        SHARED_TRANSCRIPT_WIRED_V1 as u8,
        MASK_COMMITMENT_OPENING_WIRED_V1 as u8,
        COMMITTED_MLE_OPENINGS_WIRED_V1 as u8,
        PROOF_VERIFIED_V1 as u8,
        ZERO_KNOWLEDGE_ACCEPTED_V1 as u8,
        RECEIPT_ACCEPTED_V1 as u8,
        RSS_QUALIFIED_V1 as u8,
        RELEASE_READY_V1 as u8,
    ]);
    let digest = hash.finalize();
    (digest != [0; 32])
        .then_some(digest)
        .ok_or(MOracleErrorV1::Context)
}

fn m_context_v1(
    public_context: [u8; 32],
    descriptor: &MDescriptorV1,
    lineage_digest: [u8; 32],
) -> Result<[u8; 32], MOracleErrorV1> {
    if public_context == [0; 32]
        || (descriptor.completed_plane_rounds != 0 && lineage_digest == [0; 32])
    {
        return Err(MOracleErrorV1::Context);
    }
    let mut hash = Keccak256::new();
    hash.update(M_CONTEXT_DOMAIN_V1);
    hash.update(&[M_ORACLE_VERSION_V1, GLOBAL_STATEMENT_ORDINAL_V1]);
    hash.update(&m_manifest_digest_v1()?);
    hash.update(&public_context);
    hash.update(&descriptor.mapping_digest);
    hash.update(&lineage_digest);
    hash.update(&[descriptor.completed_plane_rounds]);
    let digest = hash.finalize();
    (digest != [0; 32])
        .then_some(digest)
        .ok_or(MOracleErrorV1::Context)
}

enum MProducerSealV1 {
    Production {
        committed_m_opening: Infallible,
    },
    #[cfg(test)]
    TestOnly {
        directory: PathBuf,
    },
}

impl MProducerSealV1 {
    fn directory_v1(self) -> PathBuf {
        match self {
            Self::Production {
                committed_m_opening,
            } => match committed_m_opening {},
            #[cfg(test)]
            Self::TestOnly { directory } => directory,
        }
    }
}

struct MWriterV1 {
    writer: Option<ConfidentialSpoolWriterV1>,
    descriptor: MDescriptorV1,
    public_context: [u8; 32],
    lineage_digest: [u8; 32],
    context_digest: [u8; 32],
    next_slot: u64,
    initial_sum: Option<u64>,
}

fn begin_m_table_v1(
    public_context: [u8; 32],
    seal: MProducerSealV1,
) -> Result<MWriterV1, MOracleErrorV1> {
    let directory = seal.directory_v1();
    MWriterV1::create_v1(
        &directory,
        m_descriptor_v1(0)?,
        public_context,
        [0; 32],
        true,
    )
}

impl MWriterV1 {
    fn create_v1(
        directory: &Path,
        descriptor: MDescriptorV1,
        public_context: [u8; 32],
        lineage_digest: [u8; 32],
        initial: bool,
    ) -> Result<Self, MOracleErrorV1> {
        let context_digest = m_context_v1(public_context, &descriptor, lineage_digest)?;
        let layout = ConfidentialSpoolLayoutV1::new_v1(
            descriptor.slot_count,
            SLOT_PLAINTEXT_BYTES_V1,
            context_digest,
        )
        .map_err(|_| MOracleErrorV1::Spool)?;
        if layout.file_len_v1() != descriptor.file_bytes {
            return Err(MOracleErrorV1::Shape);
        }
        let writer = ConfidentialSpoolWriterV1::create_in_v1(directory, layout)
            .map_err(|_| MOracleErrorV1::Spool)?;
        Ok(Self {
            writer: Some(writer),
            descriptor,
            public_context,
            lineage_digest,
            context_digest,
            next_slot: 0,
            initial_sum: initial.then_some(0),
        })
    }

    fn push_next_slot_v1(&mut self, chunk: ConfidentialSpoolChunkV1) -> Result<(), MOracleErrorV1> {
        let mut writer = self.writer.take().ok_or(MOracleErrorV1::Order)?;
        let slot = self.next_slot;
        let slot_sum = validate_m_chunk_v1(
            &self.descriptor,
            slot,
            chunk.as_slice_v1(),
            self.initial_sum.is_some(),
        )?;
        writer
            .write_slot_v1(slot, chunk)
            .map_err(|_| MOracleErrorV1::Spool)?;
        if let Some(sum) = self.initial_sum.as_mut() {
            *sum = sum
                .checked_add(slot_sum)
                .ok_or(MOracleErrorV1::Arithmetic)?;
        }
        self.next_slot += 1;
        self.writer = Some(writer);
        Ok(())
    }

    fn seal_v1(mut self) -> Result<MTableV1, MOracleErrorV1> {
        let writer = self.writer.take().ok_or(MOracleErrorV1::Order)?;
        if self.next_slot != self.descriptor.slot_count
            || self
                .initial_sum
                .is_some_and(|sum| sum != ACTIVE_LOOKUP_VALUES_V1)
        {
            return Err(MOracleErrorV1::Relation);
        }
        let snapshot = writer.seal_v1().map_err(|_| MOracleErrorV1::Spool)?;
        if snapshot.slot_count_v1() != self.descriptor.slot_count
            || snapshot.plaintext_len_v1() != SLOT_PLAINTEXT_BYTES_V1
            || snapshot.file_len_v1() != self.descriptor.file_bytes
        {
            return Err(MOracleErrorV1::Shape);
        }
        let snapshot_digest = *snapshot.snapshot_digest_v1();
        let lineage_digest = if self.descriptor.completed_plane_rounds == 0 {
            snapshot_digest
        } else {
            self.lineage_digest
        };
        Ok(MTableV1 {
            snapshot: Some(snapshot),
            descriptor: self.descriptor,
            public_context: self.public_context,
            lineage_digest,
            context_digest: self.context_digest,
            snapshot_digest,
        })
    }
}

struct MTableV1 {
    snapshot: Option<ConfidentialSpoolSnapshotV1>,
    descriptor: MDescriptorV1,
    public_context: [u8; 32],
    lineage_digest: [u8; 32],
    context_digest: [u8; 32],
    snapshot_digest: [u8; 32],
}

impl MTableV1 {
    fn read_slot_v1(&mut self, slot: u64) -> Result<ConfidentialSpoolChunkV1, MOracleErrorV1> {
        let mut snapshot = self.snapshot.take().ok_or(MOracleErrorV1::Order)?;
        let chunk = snapshot
            .read_slot_v1(slot, self.context_digest)
            .map_err(|_| MOracleErrorV1::Spool)?;
        validate_m_chunk_v1(&self.descriptor, slot, chunk.as_slice_v1(), false)?;
        self.snapshot = Some(snapshot);
        Ok(chunk)
    }
}

fn validate_m_chunk_v1(
    descriptor: &MDescriptorV1,
    slot: u64,
    bytes: &[u8],
    canonical_u32: bool,
) -> Result<u64, MOracleErrorV1> {
    if bytes.len() != SLOT_PLAINTEXT_BYTES_V1 as usize {
        return Err(MOracleErrorV1::Shape);
    }
    let valid = m_valid_lanes_v1(descriptor, slot)?;
    let mut sum = 0_u64;
    for encoded in bytes[..valid * 32].chunks_exact(32) {
        decode_scalar_be_v1(encoded).map_err(map_parent_v1)?;
        if canonical_u32 {
            if encoded[..28].iter().any(|byte| *byte != 0) {
                return Err(MOracleErrorV1::Encoding);
            }
            sum = sum
                .checked_add(u64::from(u32::from_be_bytes(
                    encoded[28..]
                        .try_into()
                        .map_err(|_| MOracleErrorV1::Encoding)?,
                )))
                .ok_or(MOracleErrorV1::Arithmetic)?;
        }
    }
    if bytes[valid * 32..].iter().any(|byte| *byte != 0) {
        return Err(MOracleErrorV1::Encoding);
    }
    Ok(sum)
}

struct ColumnCursorV1<'a> {
    column: &'a mut ExternalColumnSnapshotV1,
    next_index: u64,
    chunk: Option<ConfidentialSpoolChunkV1>,
}

impl ColumnCursorV1<'_> {
    fn next_v1(&mut self) -> Result<Scalar, MOracleErrorV1> {
        if self.next_index >= self.column.descriptor.value_count {
            return Err(MOracleErrorV1::Order);
        }
        if self.next_index % SCALARS_PER_SLOT_V1 == 0 {
            drop(self.chunk.take());
            self.chunk = Some(
                self.column
                    .read_slot_v1(self.next_index / SCALARS_PER_SLOT_V1)
                    .map_err(map_parent_v1)?,
            );
        }
        let lane = usize::try_from(self.next_index % SCALARS_PER_SLOT_V1)
            .map_err(|_| MOracleErrorV1::Arithmetic)?;
        let chunk = self.chunk.as_ref().ok_or(MOracleErrorV1::Order)?;
        let value = decode_scalar_be_v1(&chunk.as_slice_v1()[lane * 32..lane * 32 + 32])
            .map_err(map_parent_v1)?;
        self.next_index += 1;
        Ok(value)
    }
}

struct MCursorV1<'a> {
    table: &'a mut MTableV1,
    next_index: u64,
    chunk: Option<ConfidentialSpoolChunkV1>,
}

impl MCursorV1<'_> {
    fn next_v1(&mut self) -> Result<Scalar, MOracleErrorV1> {
        if self.next_index >= self.table.descriptor.value_count {
            return Err(MOracleErrorV1::Order);
        }
        if self.next_index % SCALARS_PER_SLOT_V1 == 0 {
            drop(self.chunk.take());
            self.chunk = Some(
                self.table
                    .read_slot_v1(self.next_index / SCALARS_PER_SLOT_V1)?,
            );
        }
        let lane = usize::try_from(self.next_index % SCALARS_PER_SLOT_V1)
            .map_err(|_| MOracleErrorV1::Arithmetic)?;
        let chunk = self.chunk.as_ref().ok_or(MOracleErrorV1::Order)?;
        let value = decode_scalar_be_v1(&chunk.as_slice_v1()[lane * 32..lane * 32 + 32])
            .map_err(map_parent_v1)?;
        self.next_index += 1;
        Ok(value)
    }
}

#[derive(Clone, Copy)]
struct OracleAxesV1 {
    z: Scalar,
    rho: [Scalar; 29],
    alpha: Scalar,
    lambda: Scalar,
    mu: Scalar,
}

struct MaskCoefficientsV1([[Scalar; 3]; MASK_ROUNDS_V1]);

impl Drop for MaskCoefficientsV1 {
    fn drop(&mut self) {
        for row in &mut self.0 {
            for value in row {
                value.clear_secret();
            }
        }
    }
}

enum OraclePrefixSealV1 {
    Production {
        prefix_three_rounds: Infallible,
    },
    #[cfg(test)]
    TestOnly {
        pair: ExternalTablePairV1,
        multiplicity: MTableV1,
        axes: OracleAxesV1,
        point: [Scalar; 29],
        base_claim: Scalar,
        mask_carry: Scalar,
        masks: MaskCoefficientsV1,
    },
}

struct OracleLiveV1 {
    pair: ExternalTablePairV1,
    multiplicity: MTableV1,
}

#[must_use = "dropping this oracle closes all authenticated lookup columns"]
struct GlobalCubicOracleV1 {
    live: Option<OracleLiveV1>,
    axes: OracleAxesV1,
    point: [Scalar; 29],
    base_claim: Scalar,
    mask_carry: Scalar,
    masks: MaskCoefficientsV1,
    next_round: u8,
}

fn begin_global_cubic_oracle_v1(
    seal: OraclePrefixSealV1,
) -> Result<GlobalCubicOracleV1, MOracleErrorV1> {
    let (pair, multiplicity, axes, point, base_claim, mask_carry, masks) = match seal {
        OraclePrefixSealV1::Production {
            prefix_three_rounds,
        } => match prefix_three_rounds {},
        #[cfg(test)]
        OraclePrefixSealV1::TestOnly {
            pair,
            multiplicity,
            axes,
            point,
            base_claim,
            mask_carry,
            masks,
        } => (
            pair,
            multiplicity,
            axes,
            point,
            base_claim,
            mask_carry,
            masks,
        ),
    };
    let next_round = pair.descriptor.completed_rounds;
    validate_oracle_shape_v1(next_round, &pair, &multiplicity)?;
    validate_axes_and_prefix_v1(&axes, &point, next_round)?;
    Ok(GlobalCubicOracleV1 {
        live: Some(OracleLiveV1 { pair, multiplicity }),
        axes,
        point,
        base_claim,
        mask_carry,
        masks,
        next_round,
    })
}

fn validate_axes_and_prefix_v1(
    axes: &OracleAxesV1,
    point: &[Scalar; 29],
    next_round: u8,
) -> Result<(), MOracleErrorV1> {
    if axes.alpha.is_zero()
        || axes.lambda.is_zero()
        || axes.mu.is_zero()
        || axes.rho.iter().any(|value| value.is_zero())
        || (0..TABLE_VALUES_V1).any(|value| axes.z == Scalar::from_u64(value))
    {
        return Err(MOracleErrorV1::Context);
    }
    for (coordinate, value) in point.iter().enumerate() {
        if (coordinate < usize::from(next_round)) == value.is_zero() {
            return Err(MOracleErrorV1::Order);
        }
    }
    Ok(())
}

fn validate_oracle_shape_v1(
    round: u8,
    pair: &ExternalTablePairV1,
    multiplicity: &MTableV1,
) -> Result<(), MOracleErrorV1> {
    if !(EXTERNAL_FIRST_ROUND_V1..=GLOBAL_SUMCHECK_ROUNDS_V1).contains(&round)
        || pair.descriptor.completed_rounds != round
        || pair.public_context != multiplicity.public_context
    {
        return Err(MOracleErrorV1::Shape);
    }
    let expected_m_rounds = round.saturating_sub(FIRST_PLANE_ROUND_V1);
    if multiplicity.descriptor.completed_plane_rounds != expected_m_rounds {
        return Err(MOracleErrorV1::Shape);
    }
    Ok(())
}

struct EvaluatedGlobalRoundV1 {
    oracle: Option<GlobalCubicOracleV1>,
    message: [u8; 96],
    base_coefficients: [Scalar; 4],
    mask_coefficients: [Scalar; 4],
}

impl Drop for EvaluatedGlobalRoundV1 {
    fn drop(&mut self) {
        for value in &mut self.base_coefficients {
            value.clear_secret();
        }
        for value in &mut self.mask_coefficients {
            value.clear_secret();
        }
    }
}

impl GlobalCubicOracleV1 {
    fn evaluate_next_v1(mut self) -> Result<EvaluatedGlobalRoundV1, MOracleErrorV1> {
        let mut live = self.live.take().ok_or(MOracleErrorV1::Order)?;
        if self.next_round > FINAL_ROUND_V1 {
            return Err(MOracleErrorV1::Order);
        }
        validate_oracle_shape_v1(self.next_round, &live.pair, &live.multiplicity)?;
        let evaluations = evaluate_round_polynomial_v1(
            self.next_round,
            &self.axes,
            &self.point,
            &mut live.pair,
            &mut live.multiplicity,
        )?;
        if evaluations[0] + evaluations[1] != self.base_claim {
            return Err(MOracleErrorV1::Relation);
        }
        let base_coefficients = interpolate_cubic_v1(evaluations)?;
        let mask = self.masks.0[usize::from(self.next_round - EXTERNAL_FIRST_ROUND_V1)];
        let half = Scalar::from_u64(2)
            .inverse()
            .map_err(|_| MOracleErrorV1::Arithmetic)?;
        let mask_coefficients = [
            (self.mask_carry - mask[0] - mask[1] - mask[2]) * half,
            mask[2],
            mask[1],
            mask[0],
        ];
        let masked: [Scalar; 4] =
            core::array::from_fn(|index| base_coefficients[index] + mask_coefficients[index]);
        if Scalar::from_u64(2) * masked[0] + masked[1] + masked[2] + masked[3]
            != self.base_claim + self.mask_carry
        {
            return Err(MOracleErrorV1::Relation);
        }
        let mut message = [0_u8; 96];
        for (destination, source) in message
            .chunks_exact_mut(32)
            .zip([masked[0], masked[2], masked[3]])
        {
            destination.copy_from_slice(&source.to_le_bytes());
        }
        self.live = Some(live);
        Ok(EvaluatedGlobalRoundV1 {
            oracle: Some(self),
            message,
            base_coefficients,
            mask_coefficients,
        })
    }
}

enum OracleTranscriptSealV1 {
    Production {
        shared_transcript: Infallible,
    },
    #[cfg(test)]
    TestOnly {
        challenge: Scalar,
    },
}

impl OracleTranscriptSealV1 {
    fn challenge_v1(self) -> Scalar {
        match self {
            Self::Production { shared_transcript } => match shared_transcript {},
            #[cfg(test)]
            Self::TestOnly { challenge } => challenge,
        }
    }
}

enum OracleTransitionV1 {
    Continue(GlobalCubicOracleV1),
    Complete(GlobalCubicCompleteV1),
}

struct GlobalCubicCompleteV1 {
    live: OracleLiveV1,
    point: [Scalar; 29],
    candidate: Scalar,
    inverse: Scalar,
    multiplicity: Scalar,
    relation: Scalar,
    mask_carry: Scalar,
}

impl EvaluatedGlobalRoundV1 {
    fn message_v1(&self) -> &[u8; 96] {
        &self.message
    }

    fn derive_and_fold_v1(
        mut self,
        transcript: OracleTranscriptSealV1,
        sink: FoldSinkSealV1,
    ) -> Result<OracleTransitionV1, MOracleErrorV1> {
        let mut oracle = self.oracle.take().ok_or(MOracleErrorV1::Order)?;
        let mut live = oracle.live.take().ok_or(MOracleErrorV1::Order)?;
        let challenge = transcript.challenge_v1();
        if challenge == Scalar::zero() {
            return Err(MOracleErrorV1::Encoding);
        }
        let directory = sink.directory_v1();
        oracle.point[usize::from(oracle.next_round)] = challenge;
        oracle.base_claim = evaluate_cubic_v1(self.base_coefficients, challenge);
        oracle.mask_carry = evaluate_cubic_v1(self.mask_coefficients, challenge);
        live.pair = fold_pair_real_v1(live.pair, challenge, &directory)?;
        if oracle.next_round >= FIRST_PLANE_ROUND_V1 {
            live.multiplicity = fold_m_v1(live.multiplicity, challenge, &directory)?;
        }
        oracle.next_round += 1;
        if oracle.next_round == GLOBAL_SUMCHECK_ROUNDS_V1 {
            let candidate =
                read_only_scalar_v1(live.pair.candidate.as_mut().ok_or(MOracleErrorV1::Order)?)?;
            let inverse =
                read_only_scalar_v1(live.pair.inverse.as_mut().ok_or(MOracleErrorV1::Order)?)?;
            let multiplicity = read_only_m_scalar_v1(&mut live.multiplicity)?;
            if endpoint_relation_v1(
                &oracle.axes,
                &oracle.point,
                candidate,
                inverse,
                multiplicity,
            )? != oracle.base_claim
            {
                return Err(MOracleErrorV1::Relation);
            }
            return Ok(OracleTransitionV1::Complete(GlobalCubicCompleteV1 {
                live,
                point: oracle.point,
                candidate,
                inverse,
                multiplicity,
                relation: oracle.base_claim,
                mask_carry: oracle.mask_carry,
            }));
        }
        oracle.live = Some(live);
        Ok(OracleTransitionV1::Continue(oracle))
    }
}

fn fold_pair_real_v1(
    mut pair: ExternalTablePairV1,
    challenge: Scalar,
    directory: &Path,
) -> Result<ExternalTablePairV1, MOracleErrorV1> {
    let next = descriptor_v1(pair.descriptor.completed_rounds + 1).map_err(map_parent_v1)?;
    let candidate = pair.candidate.take().ok_or(MOracleErrorV1::Order)?;
    let inverse = pair.inverse.take().ok_or(MOracleErrorV1::Order)?;
    let candidate = fold_column_v1(candidate, next, pair.public_context, challenge, directory)
        .map_err(map_parent_v1)?;
    let inverse = fold_column_v1(inverse, next, pair.public_context, challenge, directory)
        .map_err(map_parent_v1)?;
    Ok(ExternalTablePairV1 {
        candidate: Some(candidate),
        inverse: Some(inverse),
        descriptor: next,
        public_context: pair.public_context,
    })
}

fn fold_m_v1(
    mut current: MTableV1,
    challenge: Scalar,
    directory: &Path,
) -> Result<MTableV1, MOracleErrorV1> {
    let next_descriptor = m_descriptor_v1(current.descriptor.completed_plane_rounds + 1)?;
    let next_lineage = m_fold_lineage_v1(
        current.lineage_digest,
        current.snapshot_digest,
        next_descriptor.completed_plane_rounds,
    )?;
    let mut next = MWriterV1::create_v1(
        directory,
        next_descriptor,
        current.public_context,
        next_lineage,
        false,
    )?;
    let mut cursor = MCursorV1 {
        table: &mut current,
        next_index: 0,
        chunk: None,
    };
    let mut output = ConfidentialSpoolChunkV1::new_zeroed_v1(SLOT_PLAINTEXT_BYTES_V1)
        .map_err(|_| MOracleErrorV1::Spool)?;
    let mut output_lanes = 0_usize;
    for _ in 0..next_descriptor.value_count {
        let low = cursor.next_v1()?;
        let high = cursor.next_v1()?;
        let folded = low + challenge * (high - low);
        output.as_mut_slice_v1()[output_lanes * 32..output_lanes * 32 + 32]
            .copy_from_slice(&folded.to_be_bytes());
        output_lanes += 1;
        if output_lanes == SCALARS_PER_SLOT_V1 as usize
            || next.next_slot + 1 == next.descriptor.slot_count
                && output_lanes == m_valid_lanes_v1(&next.descriptor, next.next_slot)?
        {
            next.push_next_slot_v1(output)?;
            output_lanes = 0;
            output = ConfidentialSpoolChunkV1::new_zeroed_v1(SLOT_PLAINTEXT_BYTES_V1)
                .map_err(|_| MOracleErrorV1::Spool)?;
        }
    }
    if output_lanes != 0 || cursor.next_index != current.descriptor.value_count {
        return Err(MOracleErrorV1::Order);
    }
    drop(cursor);
    drop(current);
    next.seal_v1()
}

fn m_fold_lineage_v1(
    root_lineage: [u8; 32],
    parent_snapshot: [u8; 32],
    completed_plane_rounds: u8,
) -> Result<[u8; 32], MOracleErrorV1> {
    if root_lineage == [0; 32] || parent_snapshot == [0; 32] || completed_plane_rounds == 0 {
        return Err(MOracleErrorV1::Context);
    }
    let mut hash = Keccak256::new();
    hash.update(M_LINEAGE_DOMAIN_V1);
    hash.update(&[M_ORACLE_VERSION_V1, GLOBAL_STATEMENT_ORDINAL_V1]);
    hash.update(&root_lineage);
    hash.update(&parent_snapshot);
    hash.update(&[completed_plane_rounds]);
    let digest = hash.finalize();
    (digest != [0; 32])
        .then_some(digest)
        .ok_or(MOracleErrorV1::Context)
}

fn evaluate_round_polynomial_v1(
    round: u8,
    axes: &OracleAxesV1,
    point: &[Scalar; 29],
    pair: &mut ExternalTablePairV1,
    multiplicity: &mut MTableV1,
) -> Result<[Scalar; 4], MOracleErrorV1> {
    let mut candidate = pair.candidate.take().ok_or(MOracleErrorV1::Order)?;
    let mut inverse = pair.inverse.take().ok_or(MOracleErrorV1::Order)?;
    let mut a = ColumnCursorV1 {
        column: &mut candidate,
        next_index: 0,
        chunk: None,
    };
    let mut u = ColumnCursorV1 {
        column: &mut inverse,
        next_index: 0,
        chunk: None,
    };
    let mut m = MCursorV1 {
        table: multiplicity,
        next_index: 0,
        chunk: None,
    };
    let mut evaluations = [Scalar::zero(); 4];
    if round < FIRST_PLANE_ROUND_V1 {
        let repeat = 1_u64 << (13 - round);
        for y in 0..TABLE_VALUES_V1 {
            let multiplicity_value = m.next_v1()?;
            let selector = Scalar::from_u64(u64::from(y < ACTIVE_PLANES_V1));
            let table_inverse = (axes.z - Scalar::from_u64(y))
                .inverse()
                .map_err(|_| MOracleErrorV1::Relation)?;
            for coordinate_suffix in 0..repeat {
                let pair_index = y * repeat + coordinate_suffix;
                accumulate_pair_v1(
                    &mut evaluations,
                    round,
                    pair_index,
                    axes,
                    point,
                    [a.next_v1()?, a.next_v1()?],
                    [u.next_v1()?, u.next_v1()?],
                    [multiplicity_value; 2],
                    [selector; 2],
                    [table_inverse; 2],
                )?;
            }
        }
    } else {
        let plane_prefix = usize::from(round - FIRST_PLANE_ROUND_V1);
        let pairs = 1_u64 << (14 - plane_prefix);
        for pair_index in 0..pairs {
            let (selector, table_inverse) =
                public_plane_pair_v1(axes.z, &point[14..usize::from(round)], pair_index)?;
            accumulate_pair_v1(
                &mut evaluations,
                round,
                pair_index,
                axes,
                point,
                [a.next_v1()?, a.next_v1()?],
                [u.next_v1()?, u.next_v1()?],
                [m.next_v1()?, m.next_v1()?],
                selector,
                table_inverse,
            )?;
        }
    }
    if a.next_index != candidate.descriptor.value_count
        || u.next_index != inverse.descriptor.value_count
        || m.next_index != multiplicity.descriptor.value_count
    {
        return Err(MOracleErrorV1::Order);
    }
    drop((a, u, m));
    pair.candidate = Some(candidate);
    pair.inverse = Some(inverse);
    Ok(evaluations)
}

#[allow(clippy::too_many_arguments)]
fn accumulate_pair_v1(
    evaluations: &mut [Scalar; 4],
    round: u8,
    pair_index: u64,
    axes: &OracleAxesV1,
    point: &[Scalar; 29],
    candidate: [Scalar; 2],
    inverse: [Scalar; 2],
    multiplicity: [Scalar; 2],
    selector: [Scalar; 2],
    table_inverse: [Scalar; 2],
) -> Result<(), MOracleErrorV1> {
    for (evaluation, t) in evaluations.iter_mut().zip([
        Scalar::zero(),
        Scalar::one(),
        Scalar::from_u64(2),
        Scalar::from_u64(3),
    ]) {
        let a = affine_v1(candidate, t);
        let u = affine_v1(inverse, t);
        let m = affine_v1(multiplicity, t);
        let s = affine_v1(selector, t);
        let q = affine_v1(table_inverse, t);
        let chi = chi_line_v1(&axes.rho, point, round, pair_index, t)?;
        let e0 = coordinate_zero_line_v1(point, round, pair_index, t)?;
        let product = (axes.z - a) * u;
        *evaluation += axes.alpha * chi * (product - s)
            + axes.lambda * (u - e0 * m * q)
            + axes.mu * (e0 * m - s);
    }
    Ok(())
}

fn affine_v1(pair: [Scalar; 2], t: Scalar) -> Scalar {
    pair[0] + t * (pair[1] - pair[0])
}

fn chi_line_v1(
    rho: &[Scalar; 29],
    point: &[Scalar; 29],
    round: u8,
    pair_index: u64,
    t: Scalar,
) -> Result<Scalar, MOracleErrorV1> {
    let mut weight = Scalar::one();
    for coordinate in 0..29_u8 {
        let value = if coordinate < round {
            point[usize::from(coordinate)]
        } else if coordinate == round {
            t
        } else {
            Scalar::from_u64((pair_index >> (coordinate - round - 1)) & 1)
        };
        let target = rho[usize::from(coordinate)];
        weight *= (Scalar::one() - target) * (Scalar::one() - value) + target * value;
    }
    Ok(weight)
}

fn coordinate_zero_line_v1(
    point: &[Scalar; 29],
    round: u8,
    pair_index: u64,
    t: Scalar,
) -> Result<Scalar, MOracleErrorV1> {
    let mut result = Scalar::one();
    for coordinate in 0..14_u8 {
        let value = if coordinate < round {
            point[usize::from(coordinate)]
        } else if coordinate == round {
            t
        } else {
            Scalar::from_u64((pair_index >> (coordinate - round - 1)) & 1)
        };
        result *= Scalar::one() - value;
    }
    Ok(result)
}

fn public_plane_pair_v1(
    z: Scalar,
    prefix: &[Scalar],
    pair_index: u64,
) -> Result<([Scalar; 2], [Scalar; 2]), MOracleErrorV1> {
    let prefix_values = 1_u64 << prefix.len();
    let mut selector = [Scalar::zero(); 2];
    let mut table_inverse = [Scalar::zero(); 2];
    for prefix_bits in 0..prefix_values {
        let mut weight = Scalar::one();
        for (coordinate, challenge) in prefix.iter().enumerate() {
            weight *= if prefix_bits >> coordinate & 1 == 1 {
                *challenge
            } else {
                Scalar::one() - *challenge
            };
        }
        for branch in 0..2_u64 {
            let index = prefix_bits | (branch << prefix.len()) | (pair_index << (prefix.len() + 1));
            selector[branch as usize] +=
                weight * Scalar::from_u64(u64::from(index < ACTIVE_PLANES_V1));
            table_inverse[branch as usize] += weight
                * (z - Scalar::from_u64(index))
                    .inverse()
                    .map_err(|_| MOracleErrorV1::Relation)?;
        }
    }
    Ok((selector, table_inverse))
}

fn endpoint_relation_v1(
    axes: &OracleAxesV1,
    point: &[Scalar; 29],
    candidate: Scalar,
    inverse: Scalar,
    multiplicity: Scalar,
) -> Result<Scalar, MOracleErrorV1> {
    let (selector_pair, table_inverse_pair) = public_plane_pair_v1(axes.z, &point[14..28], 0)?;
    let selector = affine_v1(selector_pair, point[28]);
    let table_inverse = affine_v1(table_inverse_pair, point[28]);
    let chi = axes
        .rho
        .iter()
        .zip(point)
        .fold(Scalar::one(), |weight, (target, value)| {
            weight * ((Scalar::one() - *target) * (Scalar::one() - *value) + *target * *value)
        });
    let coordinate_zero = point[..14].iter().fold(Scalar::one(), |weight, value| {
        weight * (Scalar::one() - *value)
    });
    let inverse_product = (axes.z - candidate) * inverse;
    Ok(axes.alpha * chi * (inverse_product - selector)
        + axes.lambda * (inverse - coordinate_zero * multiplicity * table_inverse)
        + axes.mu * (coordinate_zero * multiplicity - selector))
}

fn interpolate_cubic_v1(evaluations: [Scalar; 4]) -> Result<[Scalar; 4], MOracleErrorV1> {
    let half = Scalar::from_u64(2)
        .inverse()
        .map_err(|_| MOracleErrorV1::Arithmetic)?;
    let sixth = Scalar::from_u64(6)
        .inverse()
        .map_err(|_| MOracleErrorV1::Arithmetic)?;
    let cubic = (evaluations[3] - Scalar::from_u64(3) * evaluations[2]
        + Scalar::from_u64(3) * evaluations[1]
        - evaluations[0])
        * sixth;
    let quadratic = (evaluations[2] - Scalar::from_u64(2) * evaluations[1] + evaluations[0]) * half
        - Scalar::from_u64(3) * cubic;
    let linear = evaluations[1] - evaluations[0] - quadratic - cubic;
    Ok([evaluations[0], linear, quadratic, cubic])
}

fn evaluate_cubic_v1(coefficients: [Scalar; 4], point: Scalar) -> Scalar {
    ((coefficients[3] * point + coefficients[2]) * point + coefficients[1]) * point
        + coefficients[0]
}

fn read_only_scalar_v1(column: &mut ExternalColumnSnapshotV1) -> Result<Scalar, MOracleErrorV1> {
    if column.descriptor.value_count != 1 {
        return Err(MOracleErrorV1::Shape);
    }
    let chunk = column.read_slot_v1(0).map_err(map_parent_v1)?;
    decode_scalar_be_v1(&chunk.as_slice_v1()[..32]).map_err(map_parent_v1)
}

fn read_only_m_scalar_v1(table: &mut MTableV1) -> Result<Scalar, MOracleErrorV1> {
    if table.descriptor.value_count != 1 {
        return Err(MOracleErrorV1::Shape);
    }
    let chunk = table.read_slot_v1(0)?;
    decode_scalar_be_v1(&chunk.as_slice_v1()[..32]).map_err(map_parent_v1)
}

#[cfg(test)]
#[path = "m_table_oracle_v1_tests.rs"]
mod tests;
