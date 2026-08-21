//! Private fixed-geometry SHA-256 bridge for the Offline Cash STATE relation.
//!
//! The bridge deliberately wraps the repository Table16 implementation instead
//! of exposing Kagemusha types through the Offline Cash module boundary. The
//! ordered ten jobs use exactly `[6, 6, 5, 6, 6, 2, 2, 5, 6, 7]` SHA-256 blocks and are
//! routed round-robin across five lanes sharing one k=16 spread table.

use core::marker::PhantomData;
use halo2_proofs::{
    circuit::{Cell, Layouter, Value},
    halo2curves::ff::PrimeField,
    plonk::{ConstraintSystem, Error as PlonkError},
};

use super::protocol::{
    OFFLINE_CASH_STATE_SHA_JOB_BLOCKS_V1, OFFLINE_CASH_STATE_SHA_JOBS_V1,
    OFFLINE_CASH_STATE_SHA_LANES_V1, OFFLINE_CASH_STATE_SHA_TOTAL_BLOCKS_V1,
};
use crate::zk::kagemusha_sha256_table16_v4::{
    AssignedByte, BLOCK_BYTE_SIZE, PaddedByte, Sha256Instructions, Table16Chip, Table16Config,
    canonical_padding_suffix,
};

pub(super) const STATE_SHA_LANES_V1: usize = OFFLINE_CASH_STATE_SHA_LANES_V1 as usize;
pub(super) const STATE_SHA_JOBS_V1: usize = OFFLINE_CASH_STATE_SHA_JOBS_V1 as usize;
pub(super) const STATE_SHA_TOTAL_BLOCKS_V1: usize = OFFLINE_CASH_STATE_SHA_TOTAL_BLOCKS_V1 as usize;

const _: () = assert!(STATE_SHA_JOBS_V1 == 10);
const _: () = assert!(STATE_SHA_TOTAL_BLOCKS_V1 == 51);
const _: () = assert!(STATE_SHA_TOTAL_BLOCKS_V1.div_ceil(STATE_SHA_LANES_V1) == 11);

/// One message byte without leaking the underlying Table16 type.
///
/// `source_cell = None` denotes a fixed byte; constrained bytes retain their
/// already range-checked source cell. The uniform representation avoids a
/// large enum variant and does not allocate per byte.
#[derive(Clone, Debug)]
pub(super) struct OfflineCashStateShaByteV1<F: PrimeField> {
    value: Value<u8>,
    source_cell: Option<Cell>,
    constant: u8,
    _field: PhantomData<fn() -> F>,
}

impl<F: PrimeField> OfflineCashStateShaByteV1<F> {
    pub(super) const fn constant(value: u8) -> Self {
        Self {
            value: Value::known(value),
            source_cell: None,
            constant: value,
            _field: PhantomData,
        }
    }

    pub(super) const fn constrained(value: Value<u8>, cell: Cell) -> Self {
        Self {
            value,
            source_cell: Some(cell),
            constant: 0,
            _field: PhantomData,
        }
    }

    fn into_padded(self) -> Result<PaddedByte<F>, PlonkError> {
        match self.source_cell {
            Some(cell) => Ok(PaddedByte::Source(AssignedByte::from_range_checked_cell(
                self.value, cell,
            ))),
            None => Ok(PaddedByte::Constant(self.constant)),
        }
    }
}

/// One exact big-endian SHA-256 output word.
#[derive(Clone, Copy, Debug)]
pub(super) struct OfflineCashStateShaWordV1 {
    value: Value<u32>,
    cell: Cell,
}

impl OfflineCashStateShaWordV1 {
    pub(super) const fn value(self) -> Value<u32> {
        self.value
    }

    pub(super) const fn cell(self) -> Cell {
        self.cell
    }
}

/// Five Table16 lanes sharing the one canonical spread table.
#[derive(Clone, Debug)]
pub(super) struct OfflineCashStateShaConfigV1 {
    lanes: [Table16Config; STATE_SHA_LANES_V1],
}

impl OfflineCashStateShaConfigV1 {
    pub(super) fn configure<F: PrimeField>(meta: &mut ConstraintSystem<F>) -> Self {
        Self {
            lanes: Table16Chip::<F>::configure_lanes::<STATE_SHA_LANES_V1>(meta),
        }
    }

    pub(super) fn synthesize_jobs<F: PrimeField>(
        &self,
        layouter: &mut impl Layouter<F>,
        jobs: [Vec<OfflineCashStateShaByteV1<F>>; STATE_SHA_JOBS_V1],
    ) -> Result<[[OfflineCashStateShaWordV1; 8]; STATE_SHA_JOBS_V1], PlonkError> {
        Table16Chip::<F>::load(self.lanes[0].clone(), layouter)?;
        let chips = self.lanes.clone().map(Table16Chip::<F>::construct);
        let mut global_block = 0_usize;
        let mut outputs = Vec::with_capacity(STATE_SHA_JOBS_V1);
        for (job_index, message) in jobs.into_iter().enumerate() {
            let suffix = canonical_padding_suffix(message.len()).ok_or(PlonkError::Synthesis)?;
            let padded_len = message
                .len()
                .checked_add(suffix.len())
                .ok_or(PlonkError::Synthesis)?;
            if padded_len / BLOCK_BYTE_SIZE
                != OFFLINE_CASH_STATE_SHA_JOB_BLOCKS_V1[job_index] as usize
                || padded_len % BLOCK_BYTE_SIZE != 0
            {
                return Err(PlonkError::Synthesis);
            }
            let mut padded = Vec::new();
            padded
                .try_reserve_exact(padded_len)
                .map_err(|_| PlonkError::Synthesis)?;
            for byte in message {
                padded.push(byte.into_padded()?);
            }
            padded.extend(suffix.into_iter().map(PaddedByte::Constant));
            let mut blocks = padded.chunks_exact(BLOCK_BYTE_SIZE);
            let first: [PaddedByte<F>; BLOCK_BYTE_SIZE] = blocks
                .next()
                .ok_or(PlonkError::Synthesis)?
                .to_vec()
                .try_into()
                .map_err(|_| PlonkError::Synthesis)?;
            let first_lane = global_block % STATE_SHA_LANES_V1;
            let first_words =
                chips[first_lane].assign_padded_block(layouter, first, global_block)?;
            let mut state = chips[first_lane].initialization_vector(layouter)?;
            state = chips[first_lane].compress(layouter, &state, first_words)?;
            global_block += 1;
            for block in blocks {
                let block: [PaddedByte<F>; BLOCK_BYTE_SIZE] = block
                    .to_vec()
                    .try_into()
                    .map_err(|_| PlonkError::Synthesis)?;
                let lane = global_block % STATE_SHA_LANES_V1;
                let words = chips[lane].assign_padded_block(layouter, block, global_block)?;
                state = chips[lane].compress(layouter, &state, words)?;
                global_block += 1;
            }
            let terminal_lane = (global_block - 1) % STATE_SHA_LANES_V1;
            let digest = chips[terminal_lane].digest(layouter, &state)?;
            outputs.push(digest.map(|word| OfflineCashStateShaWordV1 {
                value: word.value_u32(),
                cell: word.cell(),
            }));
        }
        if global_block != STATE_SHA_TOTAL_BLOCKS_V1 {
            return Err(PlonkError::Synthesis);
        }
        outputs.try_into().map_err(|_| PlonkError::Synthesis)
    }
}
