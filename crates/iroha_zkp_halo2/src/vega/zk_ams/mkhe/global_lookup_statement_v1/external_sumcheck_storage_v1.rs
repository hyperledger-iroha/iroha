//! Authenticated external storage prerequisite for global-lookup sumcheck.
//!
//! This implements only move-only A/U storage/folding for statement 15. The
//! first three rounds and evaluator/transcript are production-uninhabited. An
//! A/U pair is not the cubic relation: M and its oracle remain separate, and
//! all equation, proof, ZK, receipt, RSS, and release gates remain false.

#![allow(
    dead_code,
    reason = "production producer/evaluator/transcript/sink seals are uninhabited"
)]

use core::convert::Infallible;
use std::path::{Path, PathBuf};

use iroha_confidential_spool::{
    ConfidentialSpoolChunkV1, ConfidentialSpoolLayoutV1, ConfidentialSpoolSnapshotV1,
    ConfidentialSpoolWriterV1,
};

use crate::vega::{
    VegaT256ScalarV1 as Scalar, bulletproof_t256::ZeroizingT256ScalarCopyV1 as SecretScalarV1,
    sponge::Keccak256,
};

use super::global_lookup_topology_digest_v1;

const STORAGE_VERSION_V1: u8 = 1;
const GLOBAL_STATEMENT_ORDINAL_V1: u8 = 15;
const GLOBAL_SUMCHECK_ROUNDS_V1: u8 = 29;
const STREAMING_PREFIX_ROUNDS_V1: u8 = 3;
const EXTERNAL_FOLD_ROUNDS_V1: u8 = 26;
const RELEASE_INITIAL_LOG_VALUES_V1: u8 = 26;
const SCALAR_BYTES_V1: u64 = 32;
const SCALARS_PER_SLOT_V1: u64 = 256;
const SLOT_PLAINTEXT_BYTES_V1: u64 = 8_192;
const SLOT_CIPHERTEXT_BYTES_V1: u64 = 8_208;

struct SecretScalarBytesV1([u8; 32]);

impl Drop for SecretScalarBytesV1 {
    fn drop(&mut self) {
        let bytes = core::hint::black_box(&mut self.0);
        bytes.fill(0);
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut *bytes);
    }
}
const RELEASE_INITIAL_VALUES_V1: u64 = 1 << RELEASE_INITIAL_LOG_VALUES_V1;
const RELEASE_INITIAL_SLOTS_V1: u64 = 262_144;
const RELEASE_COLUMN_FILE_BYTES_V1: u64 = 2_151_677_952;
const RELEASE_FIRST_NEXT_COLUMN_FILE_BYTES_V1: u64 = 1_075_838_976;
const RELEASE_PEAK_FILE_BYTES_V1: u64 = 5_379_194_880;
const RELEASE_INITIAL_WRITE_AND_SEAL_IO_BYTES_V1: u64 = 8_606_711_808;
const RELEASE_ROUND_IO_BYTES_V1: u64 = 25_820_562_240;
const RELEASE_TOTAL_IO_BYTES_V1: u64 = 34_427_274_048;
const RELEASE_AUTHENTICATED_ROUND_READS_V1: u64 = 2_097_176;
const RELEASE_NEXT_WRITE_AND_SEAL_RECORDS_V1: u64 = 1_048_604;
const RELEASE_SCALAR_FOLDS_V1: u64 = 134_217_726;
const FOLD_NAMED_CHUNK_HEAP_BYTES_V1: u64 = 16_384;

const MAPPING_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.global-lookup.external-sumcheck.mapping\0";
const CONTEXT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.global-lookup.external-sumcheck.context\0";
const MANIFEST_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.phase23.global-lookup.external-sumcheck.manifest\0";
const MAPPING_LANGUAGE_V1: &[u8] = b"statement=15;variables=(c0..c13,y0..y14);rounds0..2=streamed;materialize-after-round2;columns=A,U;index-little-endian-over-remaining-variables;slot=floor(index/256);lane=index%256;canonical-T256-scalar-big-endian-32;fold=low+r*(high-low);A-complete-before-U;fresh-sealed-output;final-unused-lanes-zero";
const ACCOUNTING_LANGUAGE_V1: &[u8] = b"initial=two-columns*(write+seal-read);each-round=evaluator-read-AU+fold-read-AU+next-write-and-seal-AU;file-peak=current-A+current-U+one-next-column;OS-page-cache,allocator,stack,AAD,cipher-state,handles-excluded";

const STORAGE_MECHANICS_COMPLETE_V1: bool = true;
const AUTHENTICATED_M_TABLE_WIRED_V1: bool = false;
const EQUATION_CORRECTNESS_VERIFIED_V1: bool = false;
const TRANSCRIPT_WIRED_V1: bool = false;
const PROOF_VERIFIED_V1: bool = false;
const ZERO_KNOWLEDGE_ACCEPTED_V1: bool = false;
const RECEIPT_ACCEPTED_V1: bool = false;
const RSS_QUALIFIED_V1: bool = false;
const RELEASE_READY_V1: bool = false;

const _: () = {
    assert!(GLOBAL_SUMCHECK_ROUNDS_V1 == STREAMING_PREFIX_ROUNDS_V1 + EXTERNAL_FOLD_ROUNDS_V1);
    assert!(SLOT_PLAINTEXT_BYTES_V1 == SCALAR_BYTES_V1 * SCALARS_PER_SLOT_V1);
    assert!(SLOT_CIPHERTEXT_BYTES_V1 == SLOT_PLAINTEXT_BYTES_V1 + 16);
    assert!(RELEASE_INITIAL_VALUES_V1 == RELEASE_INITIAL_SLOTS_V1 * SCALARS_PER_SLOT_V1);
    assert!(RELEASE_COLUMN_FILE_BYTES_V1 == RELEASE_INITIAL_SLOTS_V1 * SLOT_CIPHERTEXT_BYTES_V1);
    assert!(RELEASE_FIRST_NEXT_COLUMN_FILE_BYTES_V1 * 2 == RELEASE_COLUMN_FILE_BYTES_V1);
    assert!(
        RELEASE_PEAK_FILE_BYTES_V1
            == 2 * RELEASE_COLUMN_FILE_BYTES_V1 + RELEASE_FIRST_NEXT_COLUMN_FILE_BYTES_V1
    );
    assert!(
        RELEASE_TOTAL_IO_BYTES_V1
            == RELEASE_INITIAL_WRITE_AND_SEAL_IO_BYTES_V1 + RELEASE_ROUND_IO_BYTES_V1
    );
    assert!(STORAGE_MECHANICS_COMPLETE_V1);
    assert!(!AUTHENTICATED_M_TABLE_WIRED_V1);
    assert!(!EQUATION_CORRECTNESS_VERIFIED_V1);
    assert!(!TRANSCRIPT_WIRED_V1);
    assert!(!PROOF_VERIFIED_V1);
    assert!(!ZERO_KNOWLEDGE_ACCEPTED_V1);
    assert!(!RECEIPT_ACCEPTED_V1);
    assert!(!RSS_QUALIFIED_V1);
    assert!(!RELEASE_READY_V1);
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExternalStorageErrorV1 {
    Shape,
    Order,
    Context,
    Arithmetic,
    Encoding,
    Spool,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExternalColumnRoleV1 {
    CandidateA = 1,
    InverseU = 2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ExternalColumnDescriptorV1 {
    completed_rounds: u8,
    remaining_log_values: u8,
    value_count: u64,
    slot_count: u64,
    plaintext_bytes: u64,
    file_bytes: u64,
    mapping_digest: [u8; 32],
}

fn descriptor_v1(
    completed_rounds: u8,
) -> Result<ExternalColumnDescriptorV1, ExternalStorageErrorV1> {
    if !(STREAMING_PREFIX_ROUNDS_V1..=GLOBAL_SUMCHECK_ROUNDS_V1).contains(&completed_rounds) {
        return Err(ExternalStorageErrorV1::Shape);
    }
    let remaining_log_values = GLOBAL_SUMCHECK_ROUNDS_V1 - completed_rounds;
    let value_count = 1_u64
        .checked_shl(u32::from(remaining_log_values))
        .ok_or(ExternalStorageErrorV1::Arithmetic)?;
    let slot_count = value_count
        .checked_add(SCALARS_PER_SLOT_V1 - 1)
        .ok_or(ExternalStorageErrorV1::Arithmetic)?
        / SCALARS_PER_SLOT_V1;
    let plaintext_bytes = value_count
        .checked_mul(SCALAR_BYTES_V1)
        .ok_or(ExternalStorageErrorV1::Arithmetic)?;
    let file_bytes = slot_count
        .checked_mul(SLOT_CIPHERTEXT_BYTES_V1)
        .ok_or(ExternalStorageErrorV1::Arithmetic)?;
    let mut descriptor = ExternalColumnDescriptorV1 {
        completed_rounds,
        remaining_log_values,
        value_count,
        slot_count,
        plaintext_bytes,
        file_bytes,
        mapping_digest: [0; 32],
    };
    descriptor.mapping_digest = mapping_digest_v1(&descriptor)?;
    Ok(descriptor)
}

fn valid_lanes_v1(
    descriptor: &ExternalColumnDescriptorV1,
    slot: u64,
) -> Result<u16, ExternalStorageErrorV1> {
    if slot >= descriptor.slot_count {
        return Err(ExternalStorageErrorV1::Shape);
    }
    let first = slot
        .checked_mul(SCALARS_PER_SLOT_V1)
        .ok_or(ExternalStorageErrorV1::Arithmetic)?;
    let remaining = descriptor
        .value_count
        .checked_sub(first)
        .ok_or(ExternalStorageErrorV1::Arithmetic)?;
    u16::try_from(remaining.min(SCALARS_PER_SLOT_V1))
        .map_err(|_| ExternalStorageErrorV1::Arithmetic)
}

fn mapping_digest_v1(
    descriptor: &ExternalColumnDescriptorV1,
) -> Result<[u8; 32], ExternalStorageErrorV1> {
    let topology = global_lookup_topology_digest_v1();
    if topology == [0; 32] {
        return Err(ExternalStorageErrorV1::Context);
    }
    let mut hash = Keccak256::new();
    hash.update(MAPPING_DOMAIN_V1);
    hash.update(&[STORAGE_VERSION_V1, GLOBAL_STATEMENT_ORDINAL_V1]);
    hash.update(&topology);
    hash.update(&[
        GLOBAL_SUMCHECK_ROUNDS_V1,
        STREAMING_PREFIX_ROUNDS_V1,
        descriptor.completed_rounds,
        descriptor.remaining_log_values,
    ]);
    for value in [
        descriptor.value_count,
        descriptor.slot_count,
        SCALARS_PER_SLOT_V1,
        SLOT_PLAINTEXT_BYTES_V1,
        SLOT_CIPHERTEXT_BYTES_V1,
        descriptor.plaintext_bytes,
        descriptor.file_bytes,
    ] {
        hash.update(&value.to_be_bytes());
    }
    hash.update(&(MAPPING_LANGUAGE_V1.len() as u16).to_be_bytes());
    hash.update(MAPPING_LANGUAGE_V1);
    for slot in 0..descriptor.slot_count {
        hash.update(&slot.to_be_bytes());
        hash.update(&(slot * SCALARS_PER_SLOT_V1).to_be_bytes());
        hash.update(&valid_lanes_v1(descriptor, slot)?.to_be_bytes());
    }
    let digest = hash.finalize();
    (digest != [0; 32])
        .then_some(digest)
        .ok_or(ExternalStorageErrorV1::Context)
}

fn manifest_digest_v1() -> Result<[u8; 32], ExternalStorageErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(MANIFEST_DOMAIN_V1);
    hash.update(&[STORAGE_VERSION_V1, GLOBAL_STATEMENT_ORDINAL_V1]);
    hash.update(&global_lookup_topology_digest_v1());
    for value in [
        RELEASE_INITIAL_VALUES_V1,
        RELEASE_INITIAL_SLOTS_V1,
        RELEASE_COLUMN_FILE_BYTES_V1,
        RELEASE_FIRST_NEXT_COLUMN_FILE_BYTES_V1,
        RELEASE_PEAK_FILE_BYTES_V1,
        RELEASE_INITIAL_WRITE_AND_SEAL_IO_BYTES_V1,
        RELEASE_ROUND_IO_BYTES_V1,
        RELEASE_TOTAL_IO_BYTES_V1,
        RELEASE_AUTHENTICATED_ROUND_READS_V1,
        RELEASE_NEXT_WRITE_AND_SEAL_RECORDS_V1,
        RELEASE_SCALAR_FOLDS_V1,
        FOLD_NAMED_CHUNK_HEAP_BYTES_V1,
    ] {
        hash.update(&value.to_be_bytes());
    }
    hash.update(&(MAPPING_LANGUAGE_V1.len() as u16).to_be_bytes());
    hash.update(MAPPING_LANGUAGE_V1);
    hash.update(&(ACCOUNTING_LANGUAGE_V1.len() as u16).to_be_bytes());
    hash.update(ACCOUNTING_LANGUAGE_V1);
    hash.update(&[
        STORAGE_MECHANICS_COMPLETE_V1 as u8,
        AUTHENTICATED_M_TABLE_WIRED_V1 as u8,
        EQUATION_CORRECTNESS_VERIFIED_V1 as u8,
        TRANSCRIPT_WIRED_V1 as u8,
        PROOF_VERIFIED_V1 as u8,
        ZERO_KNOWLEDGE_ACCEPTED_V1 as u8,
        RECEIPT_ACCEPTED_V1 as u8,
        RSS_QUALIFIED_V1 as u8,
        RELEASE_READY_V1 as u8,
    ]);
    let digest = hash.finalize();
    (digest != [0; 32])
        .then_some(digest)
        .ok_or(ExternalStorageErrorV1::Context)
}

fn column_context_v1(
    public_context: [u8; 32],
    descriptor: &ExternalColumnDescriptorV1,
    role: ExternalColumnRoleV1,
    generation: u8,
) -> Result<[u8; 32], ExternalStorageErrorV1> {
    if public_context == [0; 32]
        || generation != descriptor.completed_rounds - STREAMING_PREFIX_ROUNDS_V1
    {
        return Err(ExternalStorageErrorV1::Context);
    }
    let mut hash = Keccak256::new();
    hash.update(CONTEXT_DOMAIN_V1);
    hash.update(&[STORAGE_VERSION_V1, GLOBAL_STATEMENT_ORDINAL_V1]);
    hash.update(&manifest_digest_v1()?);
    hash.update(&public_context);
    hash.update(&descriptor.mapping_digest);
    hash.update(&[role as u8, generation, descriptor.completed_rounds]);
    let digest = hash.finalize();
    (digest != [0; 32])
        .then_some(digest)
        .ok_or(ExternalStorageErrorV1::Context)
}

enum InitialProducerSealV1 {
    Production {
        producer: Infallible,
    },
    #[cfg(test)]
    TestOnly {
        directory: PathBuf,
        completed_rounds: u8,
    },
}

impl InitialProducerSealV1 {
    fn open_v1(self) -> (PathBuf, u8) {
        match self {
            Self::Production { producer } => match producer {},
            #[cfg(test)]
            Self::TestOnly {
                directory,
                completed_rounds,
            } => (directory, completed_rounds),
        }
    }
}

enum RoundEvaluatorSealV1 {
    Production {
        evaluator: Infallible,
    },
    #[cfg(test)]
    TestOnly {
        message: [u8; 96],
    },
}

impl RoundEvaluatorSealV1 {
    fn message_v1(self) -> [u8; 96] {
        match self {
            Self::Production { evaluator } => match evaluator {},
            #[cfg(test)]
            Self::TestOnly { message } => message,
        }
    }
}

enum RoundTranscriptSealV1 {
    Production {
        transcript: Infallible,
    },
    #[cfg(test)]
    TestOnly {
        challenge: Scalar,
    },
}

impl RoundTranscriptSealV1 {
    fn challenge_v1(self) -> Scalar {
        match self {
            Self::Production { transcript } => match transcript {},
            #[cfg(test)]
            Self::TestOnly { challenge } => challenge,
        }
    }
}

pub(super) enum FoldSinkSealV1 {
    Production {
        sink: Infallible,
    },
    #[cfg(test)]
    TestOnly {
        directory: PathBuf,
    },
}

impl FoldSinkSealV1 {
    pub(super) fn directory_v1(self) -> PathBuf {
        match self {
            Self::Production { sink } => match sink {},
            #[cfg(test)]
            Self::TestOnly { directory } => directory,
        }
    }
}

struct ExternalColumnWriterV1 {
    writer: Option<ConfidentialSpoolWriterV1>,
    descriptor: ExternalColumnDescriptorV1,
    role: ExternalColumnRoleV1,
    generation: u8,
    context_digest: [u8; 32],
    next_slot: u64,
}

impl ExternalColumnWriterV1 {
    fn create_v1(
        directory: &Path,
        descriptor: ExternalColumnDescriptorV1,
        role: ExternalColumnRoleV1,
        public_context: [u8; 32],
    ) -> Result<Self, ExternalStorageErrorV1> {
        let generation = descriptor.completed_rounds - STREAMING_PREFIX_ROUNDS_V1;
        let context_digest = column_context_v1(public_context, &descriptor, role, generation)?;
        let layout = ConfidentialSpoolLayoutV1::new_v1(
            descriptor.slot_count,
            SLOT_PLAINTEXT_BYTES_V1,
            context_digest,
        )
        .map_err(map_spool_v1)?;
        if layout.file_len_v1() != descriptor.file_bytes {
            return Err(ExternalStorageErrorV1::Shape);
        }
        let writer =
            ConfidentialSpoolWriterV1::create_in_v1(directory, layout).map_err(map_spool_v1)?;
        Ok(Self {
            writer: Some(writer),
            descriptor,
            role,
            generation,
            context_digest,
            next_slot: 0,
        })
    }

    fn push_slot_v1(
        &mut self,
        slot: u64,
        chunk: ConfidentialSpoolChunkV1,
    ) -> Result<(), ExternalStorageErrorV1> {
        let mut writer = self.writer.take().ok_or(ExternalStorageErrorV1::Order)?;
        if slot != self.next_slot {
            return Err(ExternalStorageErrorV1::Order);
        }
        validate_chunk_v1(&self.descriptor, slot, chunk.as_slice_v1())?;
        writer.write_slot_v1(slot, chunk).map_err(map_spool_v1)?;
        self.next_slot = self
            .next_slot
            .checked_add(1)
            .ok_or(ExternalStorageErrorV1::Arithmetic)?;
        self.writer = Some(writer);
        Ok(())
    }

    fn seal_v1(mut self) -> Result<ExternalColumnSnapshotV1, ExternalStorageErrorV1> {
        let writer = self.writer.take().ok_or(ExternalStorageErrorV1::Order)?;
        if self.next_slot != self.descriptor.slot_count {
            return Err(ExternalStorageErrorV1::Order);
        }
        let snapshot = writer.seal_v1().map_err(map_spool_v1)?;
        if snapshot.slot_count_v1() != self.descriptor.slot_count
            || snapshot.plaintext_len_v1() != SLOT_PLAINTEXT_BYTES_V1
            || snapshot.ciphertext_record_len_v1() != SLOT_CIPHERTEXT_BYTES_V1
            || snapshot.file_len_v1() != self.descriptor.file_bytes
        {
            return Err(ExternalStorageErrorV1::Shape);
        }
        let snapshot_digest = *snapshot.snapshot_digest_v1();
        Ok(ExternalColumnSnapshotV1 {
            snapshot: Some(snapshot),
            descriptor: self.descriptor,
            role: self.role,
            generation: self.generation,
            context_digest: self.context_digest,
            snapshot_digest,
        })
    }
}

struct ExternalColumnSnapshotV1 {
    snapshot: Option<ConfidentialSpoolSnapshotV1>,
    descriptor: ExternalColumnDescriptorV1,
    role: ExternalColumnRoleV1,
    generation: u8,
    context_digest: [u8; 32],
    snapshot_digest: [u8; 32],
}

impl ExternalColumnSnapshotV1 {
    fn read_slot_v1(
        &mut self,
        slot: u64,
    ) -> Result<ConfidentialSpoolChunkV1, ExternalStorageErrorV1> {
        let mut snapshot = self.snapshot.take().ok_or(ExternalStorageErrorV1::Order)?;
        let chunk = snapshot
            .read_slot_v1(slot, self.context_digest)
            .map_err(map_spool_v1)?;
        validate_chunk_v1(&self.descriptor, slot, chunk.as_slice_v1())?;
        self.snapshot = Some(snapshot);
        Ok(chunk)
    }

    fn authenticate_all_v1(&mut self) -> Result<(), ExternalStorageErrorV1> {
        for slot in 0..self.descriptor.slot_count {
            drop(self.read_slot_v1(slot)?);
        }
        Ok(())
    }
}

struct InitialPairWriterV1 {
    candidate: Option<ExternalColumnWriterV1>,
    inverse: Option<ExternalColumnWriterV1>,
    descriptor: ExternalColumnDescriptorV1,
    public_context: [u8; 32],
    candidate_complete: bool,
}

fn begin_initial_pair_v1(
    public_context: [u8; 32],
    seal: InitialProducerSealV1,
) -> Result<InitialPairWriterV1, ExternalStorageErrorV1> {
    let (directory, completed_rounds) = seal.open_v1();
    let descriptor = descriptor_v1(completed_rounds)?;
    Ok(InitialPairWriterV1 {
        candidate: Some(ExternalColumnWriterV1::create_v1(
            &directory,
            descriptor,
            ExternalColumnRoleV1::CandidateA,
            public_context,
        )?),
        inverse: Some(ExternalColumnWriterV1::create_v1(
            &directory,
            descriptor,
            ExternalColumnRoleV1::InverseU,
            public_context,
        )?),
        descriptor,
        public_context,
        candidate_complete: false,
    })
}

impl InitialPairWriterV1 {
    fn push_candidate_slot_v1(
        &mut self,
        slot: u64,
        chunk: ConfidentialSpoolChunkV1,
    ) -> Result<(), ExternalStorageErrorV1> {
        let mut writer = self.candidate.take().ok_or(ExternalStorageErrorV1::Order)?;
        if self.candidate_complete {
            return Err(ExternalStorageErrorV1::Order);
        }
        writer.push_slot_v1(slot, chunk)?;
        self.candidate_complete = writer.next_slot == self.descriptor.slot_count;
        self.candidate = Some(writer);
        Ok(())
    }

    fn push_inverse_slot_v1(
        &mut self,
        slot: u64,
        chunk: ConfidentialSpoolChunkV1,
    ) -> Result<(), ExternalStorageErrorV1> {
        let mut writer = self.inverse.take().ok_or(ExternalStorageErrorV1::Order)?;
        if !self.candidate_complete {
            return Err(ExternalStorageErrorV1::Order);
        }
        writer.push_slot_v1(slot, chunk)?;
        self.inverse = Some(writer);
        Ok(())
    }

    fn seal_v1(mut self) -> Result<ExternalTablePairV1, ExternalStorageErrorV1> {
        let candidate = self
            .candidate
            .take()
            .ok_or(ExternalStorageErrorV1::Order)?
            .seal_v1()?;
        let inverse = self
            .inverse
            .take()
            .ok_or(ExternalStorageErrorV1::Order)?
            .seal_v1()?;
        Ok(ExternalTablePairV1 {
            candidate: Some(candidate),
            inverse: Some(inverse),
            descriptor: self.descriptor,
            public_context: self.public_context,
        })
    }
}

#[must_use = "dropping this pair closes both authenticated columns"]
struct ExternalTablePairV1 {
    candidate: Option<ExternalColumnSnapshotV1>,
    inverse: Option<ExternalColumnSnapshotV1>,
    descriptor: ExternalColumnDescriptorV1,
    public_context: [u8; 32],
}

struct EvaluatedRoundV1 {
    pair: ExternalTablePairV1,
    round: u8,
    message: [u8; 96],
}

fn evaluate_round_v1(
    mut pair: ExternalTablePairV1,
    seal: RoundEvaluatorSealV1,
) -> Result<EvaluatedRoundV1, ExternalStorageErrorV1> {
    if pair.descriptor.completed_rounds >= GLOBAL_SUMCHECK_ROUNDS_V1 {
        return Err(ExternalStorageErrorV1::Order);
    }
    let mut candidate = pair.candidate.take().ok_or(ExternalStorageErrorV1::Order)?;
    let mut inverse = pair.inverse.take().ok_or(ExternalStorageErrorV1::Order)?;
    candidate.authenticate_all_v1()?;
    inverse.authenticate_all_v1()?;
    let message = seal.message_v1();
    validate_message_v1(&message)?;
    pair.candidate = Some(candidate);
    pair.inverse = Some(inverse);
    Ok(EvaluatedRoundV1 {
        round: pair.descriptor.completed_rounds,
        pair,
        message,
    })
}

struct ChallengedRoundV1 {
    pair: ExternalTablePairV1,
    round: u8,
    challenge: Scalar,
}

impl EvaluatedRoundV1 {
    fn derive_challenge_v1(
        self,
        seal: RoundTranscriptSealV1,
    ) -> Result<ChallengedRoundV1, ExternalStorageErrorV1> {
        validate_message_v1(&self.message)?;
        let challenge = seal.challenge_v1();
        if challenge == Scalar::zero() {
            return Err(ExternalStorageErrorV1::Encoding);
        }
        Ok(ChallengedRoundV1 {
            pair: self.pair,
            round: self.round,
            challenge,
        })
    }
}

impl ChallengedRoundV1 {
    fn fold_v1(
        mut self,
        sink: FoldSinkSealV1,
    ) -> Result<ExternalTablePairV1, ExternalStorageErrorV1> {
        if self.round != self.pair.descriptor.completed_rounds
            || self.round >= GLOBAL_SUMCHECK_ROUNDS_V1
        {
            return Err(ExternalStorageErrorV1::Order);
        }
        let directory = sink.directory_v1();
        let next_descriptor = descriptor_v1(self.round + 1)?;
        let candidate = self
            .pair
            .candidate
            .take()
            .ok_or(ExternalStorageErrorV1::Order)?;
        let inverse = self
            .pair
            .inverse
            .take()
            .ok_or(ExternalStorageErrorV1::Order)?;
        let next_candidate = fold_column_v1(
            candidate,
            next_descriptor,
            self.pair.public_context,
            self.challenge,
            &directory,
        )?;
        let next_inverse = fold_column_v1(
            inverse,
            next_descriptor,
            self.pair.public_context,
            self.challenge,
            &directory,
        )?;
        Ok(ExternalTablePairV1 {
            candidate: Some(next_candidate),
            inverse: Some(next_inverse),
            descriptor: next_descriptor,
            public_context: self.pair.public_context,
        })
    }
}

struct ColumnFoldStateV1 {
    current: Option<ExternalColumnSnapshotV1>,
    next: Option<ExternalColumnWriterV1>,
    output: Option<ConfidentialSpoolChunkV1>,
    challenge: Scalar,
    next_input_slot: u64,
    output_lanes: usize,
}

fn fold_column_v1(
    current: ExternalColumnSnapshotV1,
    next_descriptor: ExternalColumnDescriptorV1,
    public_context: [u8; 32],
    challenge: Scalar,
    directory: &Path,
) -> Result<ExternalColumnSnapshotV1, ExternalStorageErrorV1> {
    if next_descriptor.completed_rounds != current.descriptor.completed_rounds + 1
        || next_descriptor.value_count * 2 != current.descriptor.value_count
    {
        return Err(ExternalStorageErrorV1::Shape);
    }
    let role = current.role;
    let next = ExternalColumnWriterV1::create_v1(directory, next_descriptor, role, public_context)?;
    let output =
        ConfidentialSpoolChunkV1::new_zeroed_v1(SLOT_PLAINTEXT_BYTES_V1).map_err(map_spool_v1)?;
    let mut state = ColumnFoldStateV1 {
        current: Some(current),
        next: Some(next),
        output: Some(output),
        challenge,
        next_input_slot: 0,
        output_lanes: 0,
    };
    while state.next_input_slot
        < state
            .current
            .as_ref()
            .ok_or(ExternalStorageErrorV1::Order)?
            .descriptor
            .slot_count
    {
        state.fold_next_slot_v1()?;
    }
    if state.output_lanes != 0 || state.output.is_some() {
        return Err(ExternalStorageErrorV1::Order);
    }
    let next = state.next.take().ok_or(ExternalStorageErrorV1::Order)?;
    drop(state.current.take());
    next.seal_v1()
}

impl ColumnFoldStateV1 {
    fn fold_next_slot_v1(&mut self) -> Result<(), ExternalStorageErrorV1> {
        let mut current = self.current.take().ok_or(ExternalStorageErrorV1::Order)?;
        let mut next = self.next.take().ok_or(ExternalStorageErrorV1::Order)?;
        let mut output = self.output.take().ok_or(ExternalStorageErrorV1::Order)?;
        let input_slot = self.next_input_slot;
        let input = current.read_slot_v1(input_slot)?;
        let valid = usize::from(valid_lanes_v1(&current.descriptor, input_slot)?);
        if valid == 0 || !valid.is_multiple_of(2) {
            return Err(ExternalStorageErrorV1::Shape);
        }
        for pair in input.as_slice_v1()[..valid * SCALAR_BYTES_V1 as usize]
            .chunks_exact(2 * SCALAR_BYTES_V1 as usize)
        {
            let low = SecretScalarV1::new(decode_scalar_be_v1(&pair[..32])?);
            let high = SecretScalarV1::new(decode_scalar_be_v1(&pair[32..])?);
            let folded = SecretScalarV1::new(low.get() + self.challenge * (high.get() - low.get()));
            let encoded = SecretScalarBytesV1(folded.get().to_be_bytes());
            let offset = self
                .output_lanes
                .checked_mul(SCALAR_BYTES_V1 as usize)
                .ok_or(ExternalStorageErrorV1::Arithmetic)?;
            output.as_mut_slice_v1()[offset..offset + SCALAR_BYTES_V1 as usize]
                .copy_from_slice(&encoded.0);
            self.output_lanes += 1;
        }
        self.next_input_slot += 1;
        let final_input = self.next_input_slot == current.descriptor.slot_count;
        if self.output_lanes == SCALARS_PER_SLOT_V1 as usize || final_input {
            let output_slot = next.next_slot;
            next.push_slot_v1(output_slot, output)?;
            self.output_lanes = 0;
            self.output = if final_input {
                None
            } else {
                Some(
                    ConfidentialSpoolChunkV1::new_zeroed_v1(SLOT_PLAINTEXT_BYTES_V1)
                        .map_err(map_spool_v1)?,
                )
            };
        } else {
            self.output = Some(output);
        }
        self.current = Some(current);
        self.next = Some(next);
        Ok(())
    }
}

fn validate_chunk_v1(
    descriptor: &ExternalColumnDescriptorV1,
    slot: u64,
    bytes: &[u8],
) -> Result<(), ExternalStorageErrorV1> {
    if bytes.len() != SLOT_PLAINTEXT_BYTES_V1 as usize {
        return Err(ExternalStorageErrorV1::Shape);
    }
    let valid = usize::from(valid_lanes_v1(descriptor, slot)?);
    for encoded in bytes[..valid * SCALAR_BYTES_V1 as usize].chunks_exact(SCALAR_BYTES_V1 as usize)
    {
        drop(SecretScalarV1::new(decode_scalar_be_v1(encoded)?));
    }
    if bytes[valid * SCALAR_BYTES_V1 as usize..]
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(ExternalStorageErrorV1::Encoding);
    }
    Ok(())
}

fn decode_scalar_be_v1(bytes: &[u8]) -> Result<Scalar, ExternalStorageErrorV1> {
    let encoded: &[u8; 32] = bytes
        .try_into()
        .map_err(|_| ExternalStorageErrorV1::Encoding)?;
    Scalar::from_be_bytes_exact_ref(encoded).map_err(|_| ExternalStorageErrorV1::Encoding)
}

fn validate_message_v1(message: &[u8; 96]) -> Result<(), ExternalStorageErrorV1> {
    for encoded in message.chunks_exact(32) {
        let scalar: [u8; 32] = encoded
            .try_into()
            .map_err(|_| ExternalStorageErrorV1::Encoding)?;
        Scalar::from_le_bytes_exact(scalar).map_err(|_| ExternalStorageErrorV1::Encoding)?;
    }
    Ok(())
}

fn map_spool_v1(_: iroha_confidential_spool::ConfidentialSpoolErrorV1) -> ExternalStorageErrorV1 {
    ExternalStorageErrorV1::Spool
}

#[path = "external_sumcheck_storage_v1/m_table_oracle_v1.rs"]
mod m_table_oracle_v1;

pub(super) use m_table_oracle_v1::{
    EvaluatedGlobalRoundV1, GlobalCubicCompleteV1, GlobalCubicOracleV1, GlobalCubicPrefixReadyV1,
    MOracleErrorV1, OracleTransitionV1, begin_global_cubic_oracle_v1,
};

#[cfg(test)]
pub(super) use m_table_oracle_v1::{
    global_cubic_final_round_fixture_v1, global_cubic_hollow_fixture_v1,
};

#[cfg(test)]
#[path = "external_sumcheck_storage_v1_tests.rs"]
mod tests;
