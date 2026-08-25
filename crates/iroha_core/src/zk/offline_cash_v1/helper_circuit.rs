//! Fixed `k=16` Eq/Fp and Ep/Fq binding circuits for helper statements.
//!
//! These circuits prove canonical 184-word/27-cell public encoding, exact role
//! and protocol selection, closed operation/Android flags, exact-next `u64`
//! arithmetic without overflow, mandatory digest presence, canonical absence
//! of optional Android digests, two required digest inequalities, and all nine
//! fixed helper SHA-256 jobs. The private keys are copied into fixed-size,
//! zeroizing circuit owners and are byte-range constrained before entering the
//! repository Table16 gadget. Each leaf configures only its owned SHA jobs:
//! `GuardUse` owns jobs 0/1/2/4, `PlatformBind` owns 3/5,
//! `AndroidKeyCert` owns 6/7, and `GuardBundleLeaf` owns 8.
//!
//! Canonical low-S P-256 V3 statements and bounded Android DER/KeyMint/root
//! facts enter through a governed fixed-source mapper. Authenticated helper
//! child IPA proofs use the ordinary Poseidon transcript in the sibling
//! composition boundary, which joins all common public words exactly, binds a
//! carried lineage, and reciprocally enforces every deferred equation. This
//! individual circuit type remains only one member of that closed topology.

use core::marker::PhantomData;

use halo2_proofs::{
    circuit::{Cell, Layouter, V1, Value},
    halo2curves::{
        ff::PrimeField,
        pasta::{Fp, Fq},
    },
    plonk::{
        Advice, Circuit, Column, ConstraintSystem, Error as PlonkError, Expression, Instance,
        Selector,
    },
    poly::Rotation,
};

use super::{
    OfflineCashHalo2ParityV1,
    helper_abi::{
        ANDROID_ATTESTATION_WORD_START, ANDROID_CERTIFICATE_WORD_START, ANDROID_CLAIM_WORD_START,
        ANDROID_DIGEST_OFFSETS, ANDROID_ISSUER_KEY_WORD_START, ANDROID_TBS_WORD_START,
        BUNDLE_WORD_START, CONTEXT_WORD_START, CURRENT_GUARD_WORD_START, CURRENT_HEAD_WORD_START,
        CURRENT_LINEAGE_WORD_START, DEVICE_WORD_START, GUARD_USE_CLAIM_WORD_START,
        HELPER_ABI_WORDS, HELPER_ANDROID_PRESENT_WORD, HELPER_FROM_HIGH_WORD, HELPER_FROM_LOW_WORD,
        HELPER_INSTANCE_CELLS, HELPER_INSTANCE_CELLS_MAX, HELPER_OPERATION_WORD,
        HELPER_TO_HIGH_WORD, HELPER_TO_LOW_WORD, HELPER_WORDS_PER_INSTANCE, NEXT_GUARD_WORD_START,
        OfflineCashHelperAbiErrorV1, PLATFORM_BIND_CLAIM_WORD_START, PLATFORM_KEY_WORD_START,
        PLATFORM_MESSAGE_WORD_START, POLICY_WORD_START, RELEASE_WORD_START,
        REQUIRED_DIGEST_OFFSETS, TRANSITION_WORD_START, WALLET_WORD_START, fixed_helper_word_v1,
        pack_words_as_field,
    },
    helper_relation::{OfflineCashHelperCircuitWitnessV1, OfflineCashValidatedHelperRelationV1},
    protocol::{OFFLINE_CASH_HELPER_P256_AUX_INSTANCE_CELLS_V1, OfflineCashHalo2CircuitRoleV1},
};
use crate::zk::kagemusha_sha256_table16_v4::{
    AssignedByte, BLOCK_BYTE_SIZE, PaddedByte, Sha256Instructions, Table16Chip, Table16Config,
    canonical_padding_suffix,
};

const U32_BITS: usize = 32;
const U8_BITS: usize = 8;
const WORD_ROWS: usize = U32_BITS + 1;
const DIGEST_WORDS: usize = 8;
const DIGEST_BYTES: usize = 32;
const PRIVATE_KEY_BYTES: usize = 65;
const HELPER_SHA_JOBS_V1: usize = 9;
const HELPER_SHA_TOTAL_BLOCKS_V1: usize = 59;
const PACKED_BITS: u32 = (HELPER_WORDS_PER_INSTANCE * U32_BITS) as u32;

const fn framed_len(domain_len: usize, fields: &[usize]) -> usize {
    let mut length = 8 + domain_len;
    let mut index = 0;
    while index < fields.len() {
        length += 8 + fields[index];
        index += 1;
    }
    length
}

#[derive(Clone, Copy, Debug)]
struct OfflineCashHelperFixedShaJobV1 {
    exact_message_bytes: usize,
    compression_blocks: usize,
}

const HELPER_FIXED_SHA_JOBS_V1: [OfflineCashHelperFixedShaJobV1; HELPER_SHA_JOBS_V1] = [
    OfflineCashHelperFixedShaJobV1 {
        exact_message_bytes: framed_len(42, &[1, 32, 32, 32, 32, 32, 32, 32, 8]),
        compression_blocks: 6,
    },
    OfflineCashHelperFixedShaJobV1 {
        exact_message_bytes: framed_len(39, &[1, 32, 32, 32, 32, 32, 32, 32, 32, 32, 8]),
        compression_blocks: 7,
    },
    OfflineCashHelperFixedShaJobV1 {
        exact_message_bytes: framed_len(45, &[1, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 8, 8]),
        compression_blocks: 8,
    },
    OfflineCashHelperFixedShaJobV1 {
        exact_message_bytes: 65,
        compression_blocks: 2,
    },
    OfflineCashHelperFixedShaJobV1 {
        exact_message_bytes: framed_len(44, &[1, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 8, 8, 32]),
        compression_blocks: 9,
    },
    OfflineCashHelperFixedShaJobV1 {
        exact_message_bytes: framed_len(48, &[32, 32, 32, 32, 32, 32, 32, 32]),
        compression_blocks: 7,
    },
    OfflineCashHelperFixedShaJobV1 {
        exact_message_bytes: 65,
        compression_blocks: 2,
    },
    OfflineCashHelperFixedShaJobV1 {
        exact_message_bytes: framed_len(51, &[32, 32, 32, 32, 32, 32, 32, 32, 17, 29, 4, 7, 4]),
        compression_blocks: 8,
    },
    OfflineCashHelperFixedShaJobV1 {
        exact_message_bytes: framed_len(
            41,
            &[
                1, 1, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 8, 8, 32, 32, 32,
            ],
        ),
        compression_blocks: 10,
    },
];

const _: () = assert!(HELPER_INSTANCE_CELLS <= HELPER_INSTANCE_CELLS_MAX);
const _: () = assert!(HELPER_ABI_WORDS * WORD_ROWS + HELPER_INSTANCE_CELLS < (1 << 16));
const _: () = assert!(HELPER_FIXED_SHA_JOBS_V1.len() == HELPER_SHA_JOBS_V1);
const _: () = assert!(6 + 7 + 8 + 2 + 9 + 7 + 2 + 8 + 10 == HELPER_SHA_TOTAL_BLOCKS_V1);

const GUARD_USE_SHA_JOBS_V1: [usize; 4] = [0, 1, 2, 4];
const PLATFORM_BIND_SHA_JOBS_V1: [usize; 2] = [3, 5];
const ANDROID_KEY_CERT_SHA_JOBS_V1: [usize; 2] = [6, 7];
const GUARD_BUNDLE_LEAF_SHA_JOBS_V1: [usize; 1] = [8];

#[derive(Clone, Copy, Debug)]
struct OfflineCashHelperRoleShaSpecV1 {
    job_indices: &'static [usize],
    lanes: usize,
    total_blocks: usize,
}

fn helper_role_sha_spec_v1(role: OfflineCashHalo2CircuitRoleV1) -> OfflineCashHelperRoleShaSpecV1 {
    match role {
        OfflineCashHalo2CircuitRoleV1::GuardUse => OfflineCashHelperRoleShaSpecV1 {
            job_indices: &GUARD_USE_SHA_JOBS_V1,
            lanes: 2,
            total_blocks: 30,
        },
        OfflineCashHalo2CircuitRoleV1::PlatformBind => OfflineCashHelperRoleShaSpecV1 {
            job_indices: &PLATFORM_BIND_SHA_JOBS_V1,
            lanes: 1,
            total_blocks: 9,
        },
        OfflineCashHalo2CircuitRoleV1::AndroidKeyCert => OfflineCashHelperRoleShaSpecV1 {
            job_indices: &ANDROID_KEY_CERT_SHA_JOBS_V1,
            lanes: 1,
            total_blocks: 10,
        },
        OfflineCashHalo2CircuitRoleV1::GuardBundleLeaf => OfflineCashHelperRoleShaSpecV1 {
            job_indices: &GUARD_BUNDLE_LEAF_SHA_JOBS_V1,
            lanes: 1,
            total_blocks: 10,
        },
        OfflineCashHalo2CircuitRoleV1::State
        | OfflineCashHalo2CircuitRoleV1::StateLeaf
        | OfflineCashHalo2CircuitRoleV1::GuardBundle
        | OfflineCashHalo2CircuitRoleV1::P256V3 => {
            panic!("STATE/recursive GuardBundle/P256 are not helper-leaf roles")
        }
    }
}

const _: () = assert!(30 + 9 + 10 + 10 == HELPER_SHA_TOTAL_BLOCKS_V1);
const _: () = assert!(30_usize.div_ceil(2) <= 20);
const _: () = assert!(10_usize.div_ceil(1) <= 20);

#[derive(Clone, Debug)]
struct OfflineCashHelperShaByteV1<F: PrimeField> {
    value: Value<u8>,
    source_cell: Option<Cell>,
    constant: u8,
    _field: PhantomData<fn() -> F>,
}

impl<F: PrimeField> OfflineCashHelperShaByteV1<F> {
    const fn constant(value: u8) -> Self {
        Self {
            value: Value::known(value),
            source_cell: None,
            constant: value,
            _field: PhantomData,
        }
    }

    const fn constrained(value: Value<u8>, source_cell: Cell) -> Self {
        Self {
            value,
            source_cell: Some(source_cell),
            constant: 0,
            _field: PhantomData,
        }
    }

    fn into_padded(self) -> PaddedByte<F> {
        match self.source_cell {
            Some(cell) => {
                PaddedByte::Source(AssignedByte::from_range_checked_cell(self.value, cell))
            }
            None => PaddedByte::Constant(self.constant),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct OfflineCashHelperShaWordV1 {
    value: Value<u32>,
    cell: Cell,
}

#[derive(Clone, Debug)]
struct OfflineCashHelperShaConfigV1 {
    lanes: Vec<Table16Config>,
    spec: OfflineCashHelperRoleShaSpecV1,
}

impl OfflineCashHelperShaConfigV1 {
    fn configure<F: PrimeField>(
        meta: &mut ConstraintSystem<F>,
        role: OfflineCashHalo2CircuitRoleV1,
    ) -> Self {
        let spec = helper_role_sha_spec_v1(role);
        let lanes = match spec.lanes {
            1 => Table16Chip::<F>::configure_lanes::<1>(meta)
                .into_iter()
                .collect(),
            2 => Table16Chip::<F>::configure_lanes::<2>(meta)
                .into_iter()
                .collect(),
            _ => unreachable!("fixed helper SHA lane inventory"),
        };
        Self { lanes, spec }
    }

    fn synthesize_jobs<F: PrimeField>(
        &self,
        layouter: &mut impl Layouter<F>,
        jobs: [Vec<OfflineCashHelperShaByteV1<F>>; HELPER_SHA_JOBS_V1],
    ) -> Result<Vec<(usize, [OfflineCashHelperShaWordV1; DIGEST_WORDS])>, PlonkError> {
        Table16Chip::<F>::load(self.lanes[0].clone(), layouter)?;
        let chips = self
            .lanes
            .iter()
            .cloned()
            .map(Table16Chip::<F>::construct)
            .collect::<Vec<_>>();
        let mut global_block = 0_usize;
        let mut jobs = jobs.into_iter().map(Some).collect::<Vec<_>>();
        let mut outputs = Vec::with_capacity(self.spec.job_indices.len());
        for job_index in self.spec.job_indices.iter().copied() {
            let message = jobs[job_index].take().ok_or(PlonkError::Synthesis)?;
            let expected = HELPER_FIXED_SHA_JOBS_V1[job_index];
            if message.len() != expected.exact_message_bytes {
                return Err(PlonkError::Synthesis);
            }
            let suffix = canonical_padding_suffix(message.len()).ok_or(PlonkError::Synthesis)?;
            let padded_len = message
                .len()
                .checked_add(suffix.len())
                .ok_or(PlonkError::Synthesis)?;
            if padded_len % BLOCK_BYTE_SIZE != 0
                || padded_len / BLOCK_BYTE_SIZE != expected.compression_blocks
            {
                return Err(PlonkError::Synthesis);
            }

            let mut padded = Vec::new();
            padded
                .try_reserve_exact(padded_len)
                .map_err(|_| PlonkError::Synthesis)?;
            padded.extend(
                message
                    .into_iter()
                    .map(OfflineCashHelperShaByteV1::into_padded),
            );
            padded.extend(suffix.into_iter().map(PaddedByte::Constant));

            let mut blocks = padded.chunks_exact(BLOCK_BYTE_SIZE);
            let first: [PaddedByte<F>; BLOCK_BYTE_SIZE] = blocks
                .next()
                .ok_or(PlonkError::Synthesis)?
                .to_vec()
                .try_into()
                .map_err(|_| PlonkError::Synthesis)?;
            let first_lane = global_block % self.spec.lanes;
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
                let lane = global_block % self.spec.lanes;
                let words = chips[lane].assign_padded_block(layouter, block, global_block)?;
                state = chips[lane].compress(layouter, &state, words)?;
                global_block += 1;
            }
            let terminal_lane = (global_block - 1) % self.spec.lanes;
            outputs.push((
                job_index,
                chips[terminal_lane].digest(layouter, &state)?.map(|word| {
                    OfflineCashHelperShaWordV1 {
                        value: word.value_u32(),
                        cell: word.cell(),
                    }
                }),
            ));
        }
        if global_block != self.spec.total_blocks {
            return Err(PlonkError::Synthesis);
        }
        Ok(outputs)
    }
}

#[derive(Clone, Copy, Debug)]
struct AssignedHelperByteV1 {
    value: Value<u8>,
    cell: Cell,
}

#[derive(Clone, Debug)]
pub(super) struct OfflineCashHelperBindingConfigV1 {
    word: Column<Advice>,
    bit: Column<Advice>,
    accumulator: Column<Advice>,
    packed: Column<Advice>,
    lanes: [Column<Advice>; DIGEST_WORDS],
    instance: Column<Instance>,
    aux_instance: Option<Column<Instance>>,
    q_start: Selector,
    q_bit: Selector,
    q_word: Selector,
    q_operation: Selector,
    q_boolean: Selector,
    q_pack: Selector,
    q_exact_next: Selector,
    q_required_digest: Selector,
    q_android_digest: Selector,
    q_difference_start: Selector,
    q_difference_step: Selector,
    q_difference_terminal: Selector,
    q_bytes_to_word: Selector,
    q_sha_word: Selector,
    q_optional_sha_word: Selector,
    sha: OfflineCashHelperShaConfigV1,
}

fn configure_helper_v1<F: PrimeField>(
    meta: &mut ConstraintSystem<F>,
    role: OfflineCashHalo2CircuitRoleV1,
) -> OfflineCashHelperBindingConfigV1 {
    assert!(
        F::CAPACITY >= PACKED_BITS,
        "Offline Cash helper 224-bit packing requires sufficient field capacity"
    );
    let word = meta.advice_column();
    let bit = meta.advice_column();
    let accumulator = meta.advice_column();
    let packed = meta.advice_column();
    let lanes: [Column<Advice>; DIGEST_WORDS] = std::array::from_fn(|_| meta.advice_column());
    let constant = meta.fixed_column();
    let instance = meta.instance_column();
    let aux_instance = matches!(
        role,
        OfflineCashHalo2CircuitRoleV1::PlatformBind | OfflineCashHalo2CircuitRoleV1::AndroidKeyCert
    )
    .then(|| meta.instance_column());
    meta.enable_equality(word);
    meta.enable_equality(bit);
    meta.enable_equality(packed);
    for column in lanes {
        meta.enable_equality(column);
    }
    meta.enable_equality(instance);
    if let Some(aux_instance) = aux_instance {
        meta.enable_equality(aux_instance);
    }
    meta.enable_constant(constant);

    let q_start = meta.selector();
    meta.create_gate("offline cash helper u32 start", |meta| {
        let enabled = meta.query_selector(q_start);
        let accumulator = meta.query_advice(accumulator, Rotation::cur());
        vec![enabled * accumulator]
    });

    let q_bit = meta.selector();
    meta.create_gate("offline cash helper u32 bit", |meta| {
        let enabled = meta.query_selector(q_bit);
        let bit = meta.query_advice(bit, Rotation::cur());
        let current = meta.query_advice(accumulator, Rotation::cur());
        let next = meta.query_advice(accumulator, Rotation::next());
        let one = Expression::Constant(F::ONE);
        let two = Expression::Constant(F::from(2));
        vec![
            enabled.clone() * bit.clone() * (bit.clone() - one),
            enabled * (next - current * two - bit),
        ]
    });

    let q_word = meta.selector();
    meta.create_gate("offline cash helper reconstructed u32", |meta| {
        let enabled = meta.query_selector(q_word);
        let word = meta.query_advice(word, Rotation::cur());
        let accumulator = meta.query_advice(accumulator, Rotation::cur());
        vec![enabled * (word - accumulator)]
    });

    let q_operation = meta.selector();
    meta.create_gate("offline cash helper closed operation", |meta| {
        let enabled = meta.query_selector(q_operation);
        let word = meta.query_advice(word, Rotation::cur());
        let one = Expression::Constant(F::ONE);
        let two = Expression::Constant(F::from(2));
        vec![enabled * (word.clone() - one) * (word - two)]
    });

    let q_boolean = meta.selector();
    meta.create_gate("offline cash helper closed Android flag", |meta| {
        let enabled = meta.query_selector(q_boolean);
        let word = meta.query_advice(word, Rotation::cur());
        let one = Expression::Constant(F::ONE);
        vec![enabled * word.clone() * (word - one)]
    });

    let q_pack = meta.selector();
    meta.create_gate("offline cash helper 7x32 little-endian pack", |meta| {
        let enabled = meta.query_selector(q_pack);
        let packed = meta.query_advice(packed, Rotation::cur());
        let radix = F::from(1_u64 << 32);
        let mut coefficient = F::ONE;
        let mut reconstructed = Expression::Constant(F::ZERO);
        for column in &lanes[..HELPER_WORDS_PER_INSTANCE] {
            reconstructed = reconstructed
                + meta.query_advice(*column, Rotation::cur()) * Expression::Constant(coefficient);
            coefficient *= radix;
        }
        vec![enabled * (packed - reconstructed)]
    });

    let q_bytes_to_word = meta.selector();
    meta.create_gate("offline cash helper four bytes to u32 LE", |meta| {
        let enabled = meta.query_selector(q_bytes_to_word);
        let word = meta.query_advice(packed, Rotation::cur());
        let mut coefficient = F::ONE;
        let mut reconstructed = Expression::Constant(F::ZERO);
        for column in &lanes[..4] {
            reconstructed = reconstructed
                + meta.query_advice(*column, Rotation::cur()) * Expression::Constant(coefficient);
            coefficient *= F::from(256);
        }
        vec![enabled * (word - reconstructed)]
    });

    let q_exact_next = meta.selector();
    meta.create_gate("offline cash helper exact-next u64", |meta| {
        let enabled = meta.query_selector(q_exact_next);
        let from_low = meta.query_advice(lanes[0], Rotation::cur());
        let from_high = meta.query_advice(lanes[1], Rotation::cur());
        let to_low = meta.query_advice(lanes[2], Rotation::cur());
        let to_high = meta.query_advice(lanes[3], Rotation::cur());
        let carry = meta.query_advice(bit, Rotation::cur());
        let one = Expression::Constant(F::ONE);
        let radix = Expression::Constant(F::from(1_u64 << 32));
        vec![
            enabled.clone() * carry.clone() * (carry.clone() - one.clone()),
            enabled.clone() * (from_low + one - to_low - carry.clone() * radix),
            enabled * (from_high + carry - to_high),
        ]
    });

    let q_required_digest = meta.selector();
    meta.create_gate("offline cash helper required digest", |meta| {
        let enabled = meta.query_selector(q_required_digest);
        let inverse = meta.query_advice(accumulator, Rotation::cur());
        let sum = lanes[..DIGEST_WORDS]
            .iter()
            .fold(Expression::Constant(F::ZERO), |sum, column| {
                sum + meta.query_advice(*column, Rotation::cur())
            });
        vec![enabled * (sum * inverse - Expression::Constant(F::ONE))]
    });

    let q_android_digest = meta.selector();
    meta.create_gate("offline cash helper optional Android digest", |meta| {
        let enabled = meta.query_selector(q_android_digest);
        let present = meta.query_advice(bit, Rotation::cur());
        let inverse = meta.query_advice(accumulator, Rotation::cur());
        let one = Expression::Constant(F::ONE);
        let sum = lanes[..DIGEST_WORDS]
            .iter()
            .fold(Expression::Constant(F::ZERO), |sum, column| {
                sum + meta.query_advice(*column, Rotation::cur())
            });
        let mut constraints = vec![
            enabled.clone() * present.clone() * (present.clone() - one.clone()),
            enabled.clone() * (sum * inverse - present.clone()),
        ];
        constraints.extend(lanes[..DIGEST_WORDS].iter().map(|column| {
            enabled.clone()
                * (one.clone() - present.clone())
                * meta.query_advice(*column, Rotation::cur())
        }));
        constraints
    });

    let q_difference_start = meta.selector();
    meta.create_gate("offline cash helper digest difference start", |meta| {
        let enabled = meta.query_selector(q_difference_start);
        vec![enabled * meta.query_advice(accumulator, Rotation::cur())]
    });
    let q_difference_step = meta.selector();
    meta.create_gate(
        "offline cash helper digest difference accumulator",
        |meta| {
            let enabled = meta.query_selector(q_difference_step);
            let lhs = meta.query_advice(word, Rotation::cur());
            let rhs = meta.query_advice(bit, Rotation::cur());
            let current = meta.query_advice(accumulator, Rotation::cur());
            let next = meta.query_advice(accumulator, Rotation::next());
            let difference = lhs - rhs;
            vec![enabled * (next - current - difference.clone() * difference)]
        },
    );
    let q_difference_terminal = meta.selector();
    meta.create_gate("offline cash helper distinct digest terminal", |meta| {
        let enabled = meta.query_selector(q_difference_terminal);
        let accumulator = meta.query_advice(accumulator, Rotation::cur());
        let inverse = meta.query_advice(packed, Rotation::cur());
        vec![enabled * (accumulator * inverse - Expression::Constant(F::ONE))]
    });

    let q_sha_word = meta.selector();
    meta.create_gate("offline cash helper SHA-256 digest word", |meta| {
        let enabled = meta.query_selector(q_sha_word);
        let sha_word = meta.query_advice(word, Rotation::cur());
        let coefficients = [1_u64 << 24, 1_u64 << 16, 1_u64 << 8, 1];
        let reconstructed = lanes[..4].iter().zip(coefficients).fold(
            Expression::Constant(F::ZERO),
            |sum, (column, coefficient)| {
                sum + meta.query_advice(*column, Rotation::cur())
                    * Expression::Constant(F::from(coefficient))
            },
        );
        vec![enabled * (sha_word - reconstructed)]
    });

    let q_optional_sha_word = meta.selector();
    meta.create_gate(
        "offline cash helper optional Android SHA-256 digest word",
        |meta| {
            let enabled = meta.query_selector(q_optional_sha_word);
            let present = meta.query_advice(bit, Rotation::cur());
            let sha_word = meta.query_advice(word, Rotation::cur());
            let coefficients = [1_u64 << 24, 1_u64 << 16, 1_u64 << 8, 1];
            let reconstructed = lanes[..4].iter().zip(coefficients).fold(
                Expression::Constant(F::ZERO),
                |sum, (column, coefficient)| {
                    sum + meta.query_advice(*column, Rotation::cur())
                        * Expression::Constant(F::from(coefficient))
                },
            );
            vec![enabled * present * (sha_word - reconstructed)]
        },
    );

    OfflineCashHelperBindingConfigV1 {
        word,
        bit,
        accumulator,
        packed,
        lanes,
        instance,
        aux_instance,
        q_start,
        q_bit,
        q_word,
        q_operation,
        q_boolean,
        q_pack,
        q_exact_next,
        q_required_digest,
        q_android_digest,
        q_difference_start,
        q_difference_step,
        q_difference_terminal,
        q_bytes_to_word,
        q_sha_word,
        q_optional_sha_word,
        sha: OfflineCashHelperShaConfigV1::configure(meta, role),
    }
}

fn option_field<F: PrimeField>(value: Option<u64>) -> Value<F> {
    value.map_or_else(Value::unknown, |value| Value::known(F::from(value)))
}

fn option_inverse<F: PrimeField>(value: Option<F>) -> Value<F> {
    value.map_or_else(Value::unknown, |value| {
        Value::known(Option::<F>::from(value.invert()).unwrap_or(F::ZERO))
    })
}

fn option_byte(value: Option<u8>) -> Value<u8> {
    value.map_or_else(Value::unknown, Value::known)
}

fn assign_range_checked_bytes_v1<F: PrimeField>(
    layouter: &mut impl Layouter<F>,
    config: &OfflineCashHelperBindingConfigV1,
    label: &'static str,
    values: &[Option<u8>],
) -> Result<Vec<AssignedHelperByteV1>, PlonkError> {
    layouter.assign_region(
        || label,
        |mut region| {
            let mut assigned = Vec::with_capacity(values.len());
            for (byte_index, witness_byte) in values.iter().copied().enumerate() {
                let base = byte_index * (U8_BITS + 1);
                config.q_start.enable(&mut region, base)?;
                region.assign_advice(config.accumulator, base, Value::known(F::ZERO));
                let mut reconstructed = witness_byte.map(|_| 0_u64);
                for bit_index in 0..U8_BITS {
                    let row = base + bit_index;
                    config.q_bit.enable(&mut region, row)?;
                    let witness_bit =
                        witness_byte.map(|byte| u64::from((byte >> (U8_BITS - 1 - bit_index)) & 1));
                    region.assign_advice(config.bit, row, option_field::<F>(witness_bit));
                    reconstructed = reconstructed
                        .zip(witness_bit)
                        .map(|(accumulator, bit)| accumulator * 2 + bit);
                    region.assign_advice(
                        config.accumulator,
                        row + 1,
                        option_field::<F>(reconstructed),
                    );
                }
                let terminal = base + U8_BITS;
                config.q_word.enable(&mut region, terminal)?;
                let cell = region
                    .assign_advice(
                        config.word,
                        terminal,
                        option_field::<F>(witness_byte.map(u64::from)),
                    )
                    .cell();
                assigned.push(AssignedHelperByteV1 {
                    value: option_byte(witness_byte),
                    cell,
                });
            }
            Ok(assigned)
        },
    )
}

fn bind_public_word_bytes_v1<F: PrimeField>(
    layouter: &mut impl Layouter<F>,
    config: &OfflineCashHelperBindingConfigV1,
    words: Option<&[u32; HELPER_ABI_WORDS]>,
    word_cells: &[Cell],
    bytes: &[AssignedHelperByteV1],
) -> Result<(), PlonkError> {
    layouter.assign_region(
        || "offline cash helper canonical word bytes",
        |mut region| {
            for word_index in 0..HELPER_ABI_WORDS {
                config.q_bytes_to_word.enable(&mut region, word_index)?;
                for lane in 0..4 {
                    let source = bytes[word_index * 4 + lane];
                    let copied = region
                        .assign_advice(
                            config.lanes[lane],
                            word_index,
                            source.value.map(|byte| F::from(u64::from(byte))),
                        )
                        .cell();
                    region.constrain_equal(copied, source.cell);
                }
                let copied_word = region
                    .assign_advice(
                        config.packed,
                        word_index,
                        option_field::<F>(words.map(|words| u64::from(words[word_index]))),
                    )
                    .cell();
                region.constrain_equal(copied_word, word_cells[word_index]);
            }
            Ok(())
        },
    )
}

fn append_constant<F: PrimeField>(target: &mut Vec<OfflineCashHelperShaByteV1<F>>, value: &[u8]) {
    target.extend(
        value
            .iter()
            .copied()
            .map(OfflineCashHelperShaByteV1::constant),
    );
}

fn append_dynamic<F: PrimeField>(
    target: &mut Vec<OfflineCashHelperShaByteV1<F>>,
    value: &[AssignedHelperByteV1],
) {
    append_constant(
        target,
        &u64::try_from(value.len())
            .expect("fixed helper field length fits u64")
            .to_le_bytes(),
    );
    target.extend(
        value
            .iter()
            .map(|byte| OfflineCashHelperShaByteV1::constrained(byte.value, byte.cell)),
    );
}

fn append_constant_field<F: PrimeField>(
    target: &mut Vec<OfflineCashHelperShaByteV1<F>>,
    value: &[u8],
) {
    append_constant(
        target,
        &u64::try_from(value.len())
            .expect("fixed helper constant field length fits u64")
            .to_le_bytes(),
    );
    append_constant(target, value);
}

fn begin_framed_message<F: PrimeField>(domain: &[u8]) -> Vec<OfflineCashHelperShaByteV1<F>> {
    let mut message = Vec::new();
    append_constant(
        &mut message,
        &u64::try_from(domain.len())
            .expect("fixed helper domain length fits u64")
            .to_le_bytes(),
    );
    append_constant(&mut message, domain);
    message
}

fn public_word_bytes(
    bytes: &[AssignedHelperByteV1],
    word_start: usize,
    word_count: usize,
) -> &[AssignedHelperByteV1] {
    &bytes[word_start * 4..(word_start + word_count) * 4]
}

fn fixed_helper_sha_jobs_v1<F: PrimeField>(
    public: &[AssignedHelperByteV1],
    platform_key: &[AssignedHelperByteV1],
    android_issuer_key: &[AssignedHelperByteV1],
) -> [Vec<OfflineCashHelperShaByteV1<F>>; HELPER_SHA_JOBS_V1] {
    const CURRENT_GUARD_DOMAIN: &[u8] = b"iroha:offline-cash:v1:helper:current-guard";
    const NEXT_GUARD_DOMAIN: &[u8] = b"iroha:offline-cash:v1:helper:next-guard";
    const PLATFORM_MESSAGE_DOMAIN: &[u8] = b"iroha:offline-cash:v1:helper:platform-message";
    const GUARD_USE_CLAIM_DOMAIN: &[u8] = b"iroha:offline-cash:v1:helper:guard-use-claim";
    const PLATFORM_BIND_CLAIM_DOMAIN: &[u8] = b"iroha:offline-cash:v1:helper:platform-bind-claim";
    const ANDROID_KEY_CERT_CLAIM_DOMAIN: &[u8] =
        b"iroha:offline-cash:v1:helper:android-key-cert-claim";
    const GUARD_BUNDLE_DOMAIN: &[u8] = b"iroha:offline-cash:v1:helper:guard-bundle";
    const P256_ALGORITHM: &[u8] = b"ecdsa-p256-sha256";
    const ANDROID_KEY_ORIGIN: &[u8] = b"generated-in-keymint-hardware";
    const ANDROID_KEY_PURPOSE: &[u8] = b"sign";
    const ANDROID_DIGEST_MODE: &[u8] = b"sha-256";
    const ANDROID_USAGE_LIMIT_ONE: [u8; 4] = 1_u32.to_le_bytes();

    let operation = public_word_bytes(public, HELPER_OPERATION_WORD, 1);
    let operation = &operation[..1];
    let android_present = public_word_bytes(public, HELPER_ANDROID_PRESENT_WORD, 1);
    let android_present = &android_present[..1];
    let from_sequence = public_word_bytes(public, HELPER_FROM_LOW_WORD, 2);
    let to_sequence = public_word_bytes(public, HELPER_TO_LOW_WORD, 2);
    let release = public_word_bytes(public, RELEASE_WORD_START, DIGEST_WORDS);
    let context = public_word_bytes(public, CONTEXT_WORD_START, DIGEST_WORDS);
    let current_head = public_word_bytes(public, CURRENT_HEAD_WORD_START, DIGEST_WORDS);
    let lineage = public_word_bytes(public, CURRENT_LINEAGE_WORD_START, DIGEST_WORDS);
    let transition = public_word_bytes(public, TRANSITION_WORD_START, DIGEST_WORDS);
    let wallet = public_word_bytes(public, WALLET_WORD_START, DIGEST_WORDS);
    let policy = public_word_bytes(public, POLICY_WORD_START, DIGEST_WORDS);
    let device = public_word_bytes(public, DEVICE_WORD_START, DIGEST_WORDS);
    let current_guard = public_word_bytes(public, CURRENT_GUARD_WORD_START, DIGEST_WORDS);
    let next_guard = public_word_bytes(public, NEXT_GUARD_WORD_START, DIGEST_WORDS);
    let platform_key_digest = public_word_bytes(public, PLATFORM_KEY_WORD_START, DIGEST_WORDS);
    let platform_message_digest =
        public_word_bytes(public, PLATFORM_MESSAGE_WORD_START, DIGEST_WORDS);
    let guard_use_claim = public_word_bytes(public, GUARD_USE_CLAIM_WORD_START, DIGEST_WORDS);
    let platform_bind_claim =
        public_word_bytes(public, PLATFORM_BIND_CLAIM_WORD_START, DIGEST_WORDS);
    let android_certificate =
        public_word_bytes(public, ANDROID_CERTIFICATE_WORD_START, DIGEST_WORDS);
    let android_tbs = public_word_bytes(public, ANDROID_TBS_WORD_START, DIGEST_WORDS);
    let android_issuer_digest =
        public_word_bytes(public, ANDROID_ISSUER_KEY_WORD_START, DIGEST_WORDS);
    let android_attestation =
        public_word_bytes(public, ANDROID_ATTESTATION_WORD_START, DIGEST_WORDS);
    let android_claim = public_word_bytes(public, ANDROID_CLAIM_WORD_START, DIGEST_WORDS);

    let mut current_guard_message = begin_framed_message(CURRENT_GUARD_DOMAIN);
    for field in [
        operation,
        release,
        context,
        current_head,
        lineage,
        wallet,
        policy,
        device,
        from_sequence,
    ] {
        append_dynamic(&mut current_guard_message, field);
    }

    let mut next_guard_message = begin_framed_message(NEXT_GUARD_DOMAIN);
    for field in [
        operation,
        release,
        context,
        current_head,
        lineage,
        transition,
        wallet,
        policy,
        device,
        current_guard,
        to_sequence,
    ] {
        append_dynamic(&mut next_guard_message, field);
    }

    let mut platform_message = begin_framed_message(PLATFORM_MESSAGE_DOMAIN);
    for field in [
        operation,
        release,
        context,
        current_head,
        lineage,
        transition,
        wallet,
        policy,
        device,
        current_guard,
        next_guard,
        from_sequence,
        to_sequence,
    ] {
        append_dynamic(&mut platform_message, field);
    }

    let platform_key_message = platform_key
        .iter()
        .map(|byte| OfflineCashHelperShaByteV1::constrained(byte.value, byte.cell))
        .collect();

    let mut guard_use_message = begin_framed_message(GUARD_USE_CLAIM_DOMAIN);
    for field in [
        operation,
        release,
        context,
        current_head,
        lineage,
        transition,
        wallet,
        policy,
        device,
        current_guard,
        next_guard,
        from_sequence,
        to_sequence,
        platform_message_digest,
    ] {
        append_dynamic(&mut guard_use_message, field);
    }

    let mut platform_bind_message = begin_framed_message(PLATFORM_BIND_CLAIM_DOMAIN);
    for field in [
        release,
        policy,
        wallet,
        device,
        platform_key_digest,
        platform_message_digest,
        current_guard,
        next_guard,
    ] {
        append_dynamic(&mut platform_bind_message, field);
    }

    let android_issuer_key_message = android_issuer_key
        .iter()
        .map(|byte| OfflineCashHelperShaByteV1::constrained(byte.value, byte.cell))
        .collect();

    let mut android_claim_message = begin_framed_message(ANDROID_KEY_CERT_CLAIM_DOMAIN);
    for field in [
        release,
        policy,
        device,
        platform_key_digest,
        android_certificate,
        android_tbs,
        android_issuer_digest,
        android_attestation,
    ] {
        append_dynamic(&mut android_claim_message, field);
    }
    for field in [
        P256_ALGORITHM,
        ANDROID_KEY_ORIGIN,
        ANDROID_KEY_PURPOSE,
        ANDROID_DIGEST_MODE,
        ANDROID_USAGE_LIMIT_ONE.as_slice(),
    ] {
        append_constant_field(&mut android_claim_message, field);
    }

    let mut bundle_message = begin_framed_message(GUARD_BUNDLE_DOMAIN);
    for field in [
        operation,
        android_present,
        release,
        context,
        current_head,
        lineage,
        transition,
        wallet,
        policy,
        device,
        current_guard,
        next_guard,
        from_sequence,
        to_sequence,
        guard_use_claim,
        platform_bind_claim,
        android_claim,
    ] {
        append_dynamic(&mut bundle_message, field);
    }

    [
        current_guard_message,
        next_guard_message,
        platform_message,
        platform_key_message,
        guard_use_message,
        platform_bind_message,
        android_issuer_key_message,
        android_claim_message,
        bundle_message,
    ]
}

fn bind_sha_digest_v1<F: PrimeField>(
    layouter: &mut impl Layouter<F>,
    config: &OfflineCashHelperBindingConfigV1,
    digest: &[OfflineCashHelperShaWordV1; DIGEST_WORDS],
    expected: &[AssignedHelperByteV1],
    android_present: AssignedHelperByteV1,
    optional_android: bool,
) -> Result<(), PlonkError> {
    if expected.len() != DIGEST_BYTES {
        return Err(PlonkError::Synthesis);
    }
    layouter.assign_region(
        || "bind helper SHA-256 word to canonical digest bytes",
        |mut region| {
            for (word_index, digest_word) in digest.iter().copied().enumerate() {
                if optional_android {
                    config.q_optional_sha_word.enable(&mut region, word_index)?;
                    let copied_present = region
                        .assign_advice(
                            config.bit,
                            word_index,
                            android_present.value.map(|byte| F::from(u64::from(byte))),
                        )
                        .cell();
                    region.constrain_equal(copied_present, android_present.cell);
                } else {
                    config.q_sha_word.enable(&mut region, word_index)?;
                }
                let copied_word = region
                    .assign_advice(
                        config.word,
                        word_index,
                        digest_word.value.map(|word| F::from(u64::from(word))),
                    )
                    .cell();
                region.constrain_equal(copied_word, digest_word.cell);
                for lane in 0..4 {
                    let source = expected[word_index * 4 + lane];
                    let copied = region
                        .assign_advice(
                            config.lanes[lane],
                            word_index,
                            source.value.map(|byte| F::from(u64::from(byte))),
                        )
                        .cell();
                    region.constrain_equal(copied, source.cell);
                }
            }
            Ok(())
        },
    )
}

fn synthesize_helper_v1<F: PrimeField>(
    words: Option<&[u32; HELPER_ABI_WORDS]>,
    private_witness: Option<&OfflineCashHelperCircuitWitnessV1>,
    parity: OfflineCashHalo2ParityV1,
    role: OfflineCashHalo2CircuitRoleV1,
    config: OfflineCashHelperBindingConfigV1,
    mut layouter: impl Layouter<F>,
) -> Result<(), PlonkError> {
    let word_cells = layouter.assign_region(
        || "offline cash helper canonical u32 words",
        |mut region| {
            let mut cells = Vec::with_capacity(HELPER_ABI_WORDS);
            for word_index in 0..HELPER_ABI_WORDS {
                let base = word_index * WORD_ROWS;
                config.q_start.enable(&mut region, base)?;
                region.assign_advice(config.accumulator, base, Value::known(F::ZERO));
                let witness_word = words.map(|words| words[word_index]);
                let mut reconstructed = witness_word.map(|_| 0_u64);
                for bit_index in 0..U32_BITS {
                    let row = base + bit_index;
                    config.q_bit.enable(&mut region, row)?;
                    let witness_bit = witness_word
                        .map(|word| u64::from((word >> (U32_BITS - 1 - bit_index)) & 1));
                    region.assign_advice(config.bit, row, option_field::<F>(witness_bit));
                    reconstructed = reconstructed
                        .zip(witness_bit)
                        .map(|(accumulator, bit)| accumulator * 2 + bit);
                    region.assign_advice(
                        config.accumulator,
                        row + 1,
                        option_field::<F>(reconstructed),
                    );
                }
                let word_row = base + U32_BITS;
                config.q_word.enable(&mut region, word_row)?;
                if word_index == HELPER_OPERATION_WORD {
                    config.q_operation.enable(&mut region, word_row)?;
                }
                if word_index == HELPER_ANDROID_PRESENT_WORD {
                    config.q_boolean.enable(&mut region, word_row)?;
                }
                let cell = if let Some(constant) = fixed_helper_word_v1(parity, role, word_index) {
                    region
                        .assign_advice_from_constant(
                            || format!("fixed helper word {word_index}"),
                            config.word,
                            word_row,
                            F::from(u64::from(constant)),
                        )?
                        .cell()
                } else {
                    region
                        .assign_advice(
                            config.word,
                            word_row,
                            option_field::<F>(witness_word.map(u64::from)),
                        )
                        .cell()
                };
                cells.push(cell);
            }
            Ok(cells)
        },
    )?;

    let public_byte_values = (0..HELPER_ABI_WORDS)
        .flat_map(|word_index| {
            let bytes = words.map(|words| words[word_index].to_le_bytes());
            (0..4).map(move |lane| bytes.map(|bytes| bytes[lane]))
        })
        .collect::<Vec<_>>();
    let public_bytes = assign_range_checked_bytes_v1(
        &mut layouter,
        &config,
        "offline cash helper public bytes",
        &public_byte_values,
    )?;
    bind_public_word_bytes_v1(&mut layouter, &config, words, &word_cells, &public_bytes)?;

    let platform_key_values = if role == OfflineCashHalo2CircuitRoleV1::PlatformBind {
        (0..PRIVATE_KEY_BYTES)
            .map(|index| {
                private_witness
                    .and_then(|witness| witness.platform_public_key_sec1.as_ref())
                    .map(|key| key[index])
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let platform_key = assign_range_checked_bytes_v1(
        &mut layouter,
        &config,
        "offline cash helper private platform key bytes",
        &platform_key_values,
    )?;
    let android_issuer_key_values = if role == OfflineCashHalo2CircuitRoleV1::AndroidKeyCert {
        (0..PRIVATE_KEY_BYTES)
            .map(|index| {
                private_witness.map(|witness| {
                    witness
                        .android_issuer_public_key_sec1
                        .as_ref()
                        .map_or(0, |key| key[index])
                })
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let android_issuer_key = assign_range_checked_bytes_v1(
        &mut layouter,
        &config,
        "offline cash helper private Android issuer key bytes",
        &android_issuer_key_values,
    )?;

    if let Some(aux_instance) = config.aux_instance {
        let aux_bytes = match role {
            OfflineCashHalo2CircuitRoleV1::PlatformBind => platform_key
                .iter()
                .chain(public_word_bytes(
                    &public_bytes,
                    PLATFORM_MESSAGE_WORD_START,
                    DIGEST_WORDS,
                ))
                .copied()
                .collect::<Vec<_>>(),
            OfflineCashHalo2CircuitRoleV1::AndroidKeyCert => android_issuer_key
                .iter()
                .chain(public_word_bytes(
                    &public_bytes,
                    ANDROID_TBS_WORD_START,
                    DIGEST_WORDS,
                ))
                .copied()
                .collect::<Vec<_>>(),
            _ => return Err(PlonkError::Synthesis),
        };
        if aux_bytes.len()
            != usize::try_from(OFFLINE_CASH_HELPER_P256_AUX_INSTANCE_CELLS_V1)
                .map_err(|_| PlonkError::Synthesis)?
        {
            return Err(PlonkError::Synthesis);
        }
        for (row, byte) in aux_bytes.into_iter().enumerate() {
            layouter.constrain_instance(byte.cell, aux_instance, row);
        }
    }

    let sha_jobs = fixed_helper_sha_jobs_v1::<F>(&public_bytes, &platform_key, &android_issuer_key);
    let sha_digests = config.sha.synthesize_jobs(&mut layouter, sha_jobs)?;
    let android_present = public_bytes[HELPER_ANDROID_PRESENT_WORD * 4];
    let sha_bindings = [
        (CURRENT_GUARD_WORD_START, false),
        (NEXT_GUARD_WORD_START, false),
        (PLATFORM_MESSAGE_WORD_START, false),
        (PLATFORM_KEY_WORD_START, false),
        (GUARD_USE_CLAIM_WORD_START, false),
        (PLATFORM_BIND_CLAIM_WORD_START, false),
        (ANDROID_ISSUER_KEY_WORD_START, true),
        (ANDROID_CLAIM_WORD_START, true),
        (BUNDLE_WORD_START, false),
    ];
    for (job_index, digest) in sha_digests {
        let (expected_offset, optional_android) = sha_bindings[job_index];
        bind_sha_digest_v1(
            &mut layouter,
            &config,
            &digest,
            public_word_bytes(&public_bytes, expected_offset, DIGEST_WORDS),
            android_present,
            optional_android,
        )?;
    }

    layouter.assign_region(
        || "offline cash helper exact-next",
        |mut region| {
            config.q_exact_next.enable(&mut region, 0)?;
            for (lane, word_index) in [
                HELPER_FROM_LOW_WORD,
                HELPER_FROM_HIGH_WORD,
                HELPER_TO_LOW_WORD,
                HELPER_TO_HIGH_WORD,
            ]
            .into_iter()
            .enumerate()
            {
                let value = words.map(|words| u64::from(words[word_index]));
                let cell = region
                    .assign_advice(config.lanes[lane], 0, option_field::<F>(value))
                    .cell();
                region.constrain_equal(cell, word_cells[word_index]);
            }
            let carry = words.map(|words| u64::from(words[HELPER_FROM_LOW_WORD] == u32::MAX));
            region.assign_advice(config.bit, 0, option_field::<F>(carry));
            Ok(())
        },
    )?;

    layouter.assign_region(
        || "offline cash helper digest presence",
        |mut region| {
            let mut row = 0_usize;
            for offset in REQUIRED_DIGEST_OFFSETS {
                config.q_required_digest.enable(&mut region, row)?;
                let mut sum = words.map(|_| F::ZERO);
                for lane in 0..DIGEST_WORDS {
                    let word_index = offset + lane;
                    let value = words.map(|words| u64::from(words[word_index]));
                    let cell = region
                        .assign_advice(config.lanes[lane], row, option_field::<F>(value))
                        .cell();
                    region.constrain_equal(cell, word_cells[word_index]);
                    sum = sum.zip(value).map(|(sum, value)| sum + F::from(value));
                }
                region.assign_advice(config.accumulator, row, option_inverse(sum));
                row += 1;
            }
            for offset in ANDROID_DIGEST_OFFSETS {
                config.q_android_digest.enable(&mut region, row)?;
                let present = words.map(|words| u64::from(words[HELPER_ANDROID_PRESENT_WORD]));
                let present_cell = region
                    .assign_advice(config.bit, row, option_field::<F>(present))
                    .cell();
                region.constrain_equal(present_cell, word_cells[HELPER_ANDROID_PRESENT_WORD]);
                let mut sum = words.map(|_| F::ZERO);
                for lane in 0..DIGEST_WORDS {
                    let word_index = offset + lane;
                    let value = words.map(|words| u64::from(words[word_index]));
                    let cell = region
                        .assign_advice(config.lanes[lane], row, option_field::<F>(value))
                        .cell();
                    region.constrain_equal(cell, word_cells[word_index]);
                    sum = sum.zip(value).map(|(sum, value)| sum + F::from(value));
                }
                region.assign_advice(config.accumulator, row, option_inverse(sum));
                row += 1;
            }
            Ok(())
        },
    )?;

    for (label, lhs_offset, rhs_offset) in [
        (
            "offline cash helper current/next guard inequality",
            CURRENT_GUARD_WORD_START,
            NEXT_GUARD_WORD_START,
        ),
        (
            "offline cash helper current/transition inequality",
            CURRENT_HEAD_WORD_START,
            TRANSITION_WORD_START,
        ),
    ] {
        layouter.assign_region(
            || label,
            |mut region| {
                config.q_difference_start.enable(&mut region, 0)?;
                region.assign_advice(config.accumulator, 0, Value::known(F::ZERO));
                let mut running = words.map(|_| F::ZERO);
                for lane in 0..DIGEST_WORDS {
                    config.q_difference_step.enable(&mut region, lane)?;
                    let lhs_index = lhs_offset + lane;
                    let rhs_index = rhs_offset + lane;
                    let lhs = words.map(|words| u64::from(words[lhs_index]));
                    let rhs = words.map(|words| u64::from(words[rhs_index]));
                    let lhs_cell = region
                        .assign_advice(config.word, lane, option_field::<F>(lhs))
                        .cell();
                    let rhs_cell = region
                        .assign_advice(config.bit, lane, option_field::<F>(rhs))
                        .cell();
                    region.constrain_equal(lhs_cell, word_cells[lhs_index]);
                    region.constrain_equal(rhs_cell, word_cells[rhs_index]);
                    running = running.zip(lhs.zip(rhs)).map(|(sum, (lhs, rhs))| {
                        let difference = F::from(lhs) - F::from(rhs);
                        sum + difference * difference
                    });
                    region.assign_advice(
                        config.accumulator,
                        lane + 1,
                        running.map_or_else(Value::unknown, Value::known),
                    );
                }
                config
                    .q_difference_terminal
                    .enable(&mut region, DIGEST_WORDS)?;
                region.assign_advice(config.packed, DIGEST_WORDS, option_inverse(running));
                Ok(())
            },
        )?;
    }

    let packed_cells = layouter.assign_region(
        || "offline cash helper canonical public cells",
        |mut region| {
            let mut cells = Vec::with_capacity(HELPER_INSTANCE_CELLS);
            for cell_index in 0..HELPER_INSTANCE_CELLS {
                config.q_pack.enable(&mut region, cell_index)?;
                let start = cell_index * HELPER_WORDS_PER_INSTANCE;
                let end = (start + HELPER_WORDS_PER_INSTANCE).min(HELPER_ABI_WORDS);
                for lane in 0..HELPER_WORDS_PER_INSTANCE {
                    let word_index = start + lane;
                    if word_index < end {
                        let value = words.map(|words| u64::from(words[word_index]));
                        let lane_cell = region
                            .assign_advice(config.lanes[lane], cell_index, option_field::<F>(value))
                            .cell();
                        region.constrain_equal(lane_cell, word_cells[word_index]);
                    } else {
                        region.assign_advice_from_constant(
                            || format!("zero helper padding lane {lane}"),
                            config.lanes[lane],
                            cell_index,
                            F::ZERO,
                        )?;
                    }
                }
                let packed = words.map(|words| pack_words_as_field::<F>(&words[start..end]));
                cells.push(
                    region
                        .assign_advice(
                            config.packed,
                            cell_index,
                            packed.map_or_else(Value::unknown, Value::known),
                        )
                        .cell(),
                );
            }
            Ok(cells)
        },
    )?;
    for (row, cell) in packed_cells.into_iter().enumerate() {
        layouter.constrain_instance(cell, config.instance, row);
    }
    Ok(())
}

macro_rules! define_helper_binding_circuit {
    ($name:ident, $field:ty, $parity:expr, $role:expr) => {
        pub(super) struct $name {
            words: Option<[u32; HELPER_ABI_WORDS]>,
            private_witness: Option<OfflineCashHelperCircuitWitnessV1>,
        }

        impl core::fmt::Debug for $name {
            fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                formatter
                    .debug_struct(stringify!($name))
                    .field("has_public_words", &self.words.is_some())
                    .field("has_private_sha_evidence", &self.private_witness.is_some())
                    .field("p256_child_contract", &"governed-fixed-source-v3")
                    .field("child_ipa_verification", &"authenticated-sibling-boundary")
                    .finish()
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self {
                    words: None,
                    private_witness: None,
                }
            }
        }

        impl $name {
            pub(super) fn new(
                relation: &OfflineCashValidatedHelperRelationV1,
            ) -> Result<Self, OfflineCashHelperAbiErrorV1> {
                let instances = relation.public_instances($parity, $role)?;
                Ok(Self {
                    words: Some(*instances.words()),
                    private_witness: Some(relation.circuit_witness($role)?),
                })
            }

            /// Exact direct-instance columns, including the intermediate-only
            /// SEC1-plus-digest column for PlatformBind/AndroidKeyCert.
            pub(super) fn public_instance_columns(&self) -> Vec<Vec<$field>> {
                let Some(words) = self.words.as_ref() else {
                    return vec![
                        Vec::new();
                        if matches!(
                            $role,
                            OfflineCashHalo2CircuitRoleV1::PlatformBind
                                | OfflineCashHalo2CircuitRoleV1::AndroidKeyCert
                        ) {
                            2
                        } else {
                            1
                        }
                    ];
                };
                let common = (0..HELPER_INSTANCE_CELLS)
                    .map(|cell_index| {
                        let start = cell_index * HELPER_WORDS_PER_INSTANCE;
                        let end = (start + HELPER_WORDS_PER_INSTANCE).min(HELPER_ABI_WORDS);
                        pack_words_as_field::<$field>(&words[start..end])
                    })
                    .collect::<Vec<_>>();
                let mut columns = vec![common];
                if matches!(
                    $role,
                    OfflineCashHalo2CircuitRoleV1::PlatformBind
                        | OfflineCashHalo2CircuitRoleV1::AndroidKeyCert
                ) {
                    let key = match $role {
                        OfflineCashHalo2CircuitRoleV1::PlatformBind => self
                            .private_witness
                            .as_ref()
                            .and_then(|witness| witness.platform_public_key_sec1.as_ref()),
                        OfflineCashHalo2CircuitRoleV1::AndroidKeyCert => self
                            .private_witness
                            .as_ref()
                            .and_then(|witness| witness.android_issuer_public_key_sec1.as_ref()),
                        _ => None,
                    };
                    let digest_word_start = match $role {
                        OfflineCashHalo2CircuitRoleV1::PlatformBind => PLATFORM_MESSAGE_WORD_START,
                        OfflineCashHalo2CircuitRoleV1::AndroidKeyCert => ANDROID_TBS_WORD_START,
                        _ => unreachable!("fixed helper auxiliary role"),
                    };
                    let mut aux =
                        Vec::with_capacity(OFFLINE_CASH_HELPER_P256_AUX_INSTANCE_CELLS_V1 as usize);
                    aux.extend(
                        (0..PRIVATE_KEY_BYTES).map(|index| {
                            <$field>::from(u64::from(key.map_or(0, |key| key[index])))
                        }),
                    );
                    aux.extend(
                        words[digest_word_start..digest_word_start + DIGEST_WORDS]
                            .iter()
                            .flat_map(|word| word.to_le_bytes())
                            .map(|byte| <$field>::from(u64::from(byte))),
                    );
                    columns.push(aux);
                }
                columns
            }

            #[cfg(test)]
            pub(super) fn from_relation_and_words_for_test(
                relation: &OfflineCashValidatedHelperRelationV1,
                words: [u32; HELPER_ABI_WORDS],
            ) -> Self {
                Self {
                    words: Some(words),
                    private_witness: Some(
                        relation
                            .circuit_witness($role)
                            .expect("test relation supports helper role"),
                    ),
                }
            }

            #[cfg(test)]
            pub(super) fn mutate_platform_key_for_test(&mut self) {
                if let Some(witness) = self.private_witness.as_mut() {
                    if let Some(key) = witness.platform_public_key_sec1.as_mut() {
                        key[1] ^= 1;
                    }
                }
            }
        }

        impl Circuit<$field> for $name {
            type Config = OfflineCashHelperBindingConfigV1;
            type FloorPlanner = V1;
            #[cfg(feature = "circuit-params")]
            type Params = ();

            fn without_witnesses(&self) -> Self {
                Self::default()
            }

            fn configure(meta: &mut ConstraintSystem<$field>) -> Self::Config {
                configure_helper_v1(meta, $role)
            }

            fn synthesize(
                &self,
                config: Self::Config,
                layouter: impl Layouter<$field>,
            ) -> Result<(), PlonkError> {
                synthesize_helper_v1(
                    self.words.as_ref(),
                    self.private_witness.as_ref(),
                    $parity,
                    $role,
                    config,
                    layouter,
                )
            }
        }
    };
}

define_helper_binding_circuit!(
    OfflineCashEqGuardUseBindingCircuitV1,
    Fp,
    OfflineCashHalo2ParityV1::Eq,
    OfflineCashHalo2CircuitRoleV1::GuardUse
);
define_helper_binding_circuit!(
    OfflineCashEpGuardUseBindingCircuitV1,
    Fq,
    OfflineCashHalo2ParityV1::Ep,
    OfflineCashHalo2CircuitRoleV1::GuardUse
);
define_helper_binding_circuit!(
    OfflineCashEqPlatformBindBindingCircuitV1,
    Fp,
    OfflineCashHalo2ParityV1::Eq,
    OfflineCashHalo2CircuitRoleV1::PlatformBind
);
define_helper_binding_circuit!(
    OfflineCashEpPlatformBindBindingCircuitV1,
    Fq,
    OfflineCashHalo2ParityV1::Ep,
    OfflineCashHalo2CircuitRoleV1::PlatformBind
);
define_helper_binding_circuit!(
    OfflineCashEqAndroidKeyCertBindingCircuitV1,
    Fp,
    OfflineCashHalo2ParityV1::Eq,
    OfflineCashHalo2CircuitRoleV1::AndroidKeyCert
);
define_helper_binding_circuit!(
    OfflineCashEpAndroidKeyCertBindingCircuitV1,
    Fq,
    OfflineCashHalo2ParityV1::Ep,
    OfflineCashHalo2CircuitRoleV1::AndroidKeyCert
);
define_helper_binding_circuit!(
    OfflineCashEqGuardBundleLeafBindingCircuitV1,
    Fp,
    OfflineCashHalo2ParityV1::Eq,
    OfflineCashHalo2CircuitRoleV1::GuardBundleLeaf
);
define_helper_binding_circuit!(
    OfflineCashEpGuardBundleLeafBindingCircuitV1,
    Fq,
    OfflineCashHalo2ParityV1::Ep,
    OfflineCashHalo2CircuitRoleV1::GuardBundleLeaf
);
