//! Halo2 constraints for the private Offline Cash STATE relation.

use halo2_proofs::{
    circuit::{Cell, Layouter, Value},
    halo2curves::ff::PrimeField,
    plonk::{Advice, Column, ConstraintSystem, Error as PlonkError, Expression, Selector},
    poly::Rotation,
};

use super::{
    BALANCE_HEAD_MESSAGE_BYTES_V1, CREDIT_HEAD_MESSAGE_BYTES_V1, OfflineCashStatePrivateWitnessV1,
    RECEIVE_OPENING_DOMAIN_V1, RECEIVE_OPENING_MESSAGE_BYTES_V1, RECEIVE_SEMANTIC_DOMAIN_V1,
    RECEIVE_SEMANTIC_MESSAGE_BYTES_V1, RECEIVE_TRANSITION_DOMAIN_V1,
    RECEIVE_TRANSITION_MESSAGE_BYTES_V1, SEND_SPLIT_BRANCH_DOMAIN_V1,
    SEND_SPLIT_RECEIVER_BRANCH_MESSAGE_BYTES_V1, SEND_SPLIT_RECEIVER_BRANCH_V1,
    SEND_SPLIT_SEED_DOMAIN_V1, SEND_SPLIT_SEED_MESSAGE_BYTES_V1,
    SEND_SPLIT_SENDER_BRANCH_MESSAGE_BYTES_V1, SEND_SPLIT_SENDER_BRANCH_V1,
    STATE_HEAD_FRAME_VERSION_V1, STATE_LINEAGE_DOMAIN_V1, STATE_LINEAGE_MESSAGE_BYTES_V1,
};
use crate::zk::offline_cash_v1::state_abi::{
    AMOUNT_WORD_START, CONTEXT_WORD_START, LINK_WORD_START, PARENT_0_WORD_START,
    PARENT_1_WORD_START, RELEASE_WORD_START, REQUEST_WORD_START, RESULT_WORD_START, SCALE_WORD,
    SEMANTIC_WORD_START, STATE_ABI_WORDS, STATE_OPERATION_WORD, TRANSITION_WORD_START,
};
use crate::zk::offline_cash_v1::state_sha::{
    OfflineCashStateShaByteV1, OfflineCashStateShaConfigV1, OfflineCashStateShaWordV1,
};

const BYTE_BITS: usize = 8;
const BYTE_ROWS: usize = BYTE_BITS + 1;
const DIGEST_BYTES: usize = 32;
const AMOUNT_BYTES: usize = 16;
const SEQUENCE_BYTES: usize = 8;
const DIGEST_WORDS: usize = DIGEST_BYTES / 4;
const AMOUNT_WORDS: usize = AMOUNT_BYTES / 4;
const SEQUENCE_WORDS: usize = SEQUENCE_BYTES / 4;
const STATE_NONZERO_BINDING_MAX_ROTATION_V1: i32 = 1;

const _: () = assert!(STATE_NONZERO_BINDING_MAX_ROTATION_V1 <= 1);

#[derive(Clone, Debug)]
pub(in crate::zk::offline_cash_v1) struct OfflineCashStateRelationConfigV1 {
    byte: Column<Advice>,
    byte_bit: Column<Advice>,
    byte_accumulator: Column<Advice>,
    packed_byte_word: Column<Advice>,
    packed_byte_lanes: [Column<Advice>; 4],
    operation: Column<Advice>,
    before_limb: Column<Advice>,
    after_limb: Column<Advice>,
    transfer_limb: Column<Advice>,
    nonzero_limb: Column<Advice>,
    nonzero_sum: Column<Advice>,
    carry: Column<Advice>,
    nonzero_inverse: Column<Advice>,
    select_left: Column<Advice>,
    select_right: Column<Advice>,
    select_output: Column<Advice>,
    sha_word: Column<Advice>,
    sha_bytes: [Column<Advice>; 4],
    q_byte_start: Selector,
    q_byte_bit: Selector,
    q_byte: Selector,
    q_pack_bytes: Selector,
    q_conservation: Selector,
    q_operation: Selector,
    q_sequence_low: Selector,
    q_sequence_high: Selector,
    q_transfer_nonzero: Selector,
    q_binding_sum_start: Selector,
    q_binding_sum_step: Selector,
    q_binding_nonzero: Selector,
    q_send_binding_nonzero: Selector,
    q_select: Selector,
    q_sha_word: Selector,
    q_send_sha_word: Selector,
    q_receive_sha_word: Selector,
    sha: OfflineCashStateShaConfigV1,
}

pub(in crate::zk::offline_cash_v1) fn configure_relation_v1<F: PrimeField>(
    meta: &mut ConstraintSystem<F>,
) -> OfflineCashStateRelationConfigV1 {
    let byte = meta.advice_column();
    let byte_bit = meta.advice_column();
    let byte_accumulator = meta.advice_column();
    let packed_byte_word = meta.advice_column();
    let packed_byte_lanes = std::array::from_fn(|_| meta.advice_column());
    let operation = meta.advice_column();
    let before_limb = meta.advice_column();
    let after_limb = meta.advice_column();
    let transfer_limb = meta.advice_column();
    let nonzero_limb = meta.advice_column();
    let nonzero_sum = meta.advice_column();
    let carry = meta.advice_column();
    let nonzero_inverse = meta.advice_column();
    let select_left = meta.advice_column();
    let select_right = meta.advice_column();
    let select_output = meta.advice_column();
    let sha_word = meta.advice_column();
    let sha_bytes = std::array::from_fn(|_| meta.advice_column());
    for column in packed_byte_lanes
        .into_iter()
        .chain([
            byte,
            packed_byte_word,
            operation,
            before_limb,
            after_limb,
            transfer_limb,
            nonzero_limb,
            carry,
        ])
        .chain([select_left, select_right, select_output, sha_word])
        .chain(sha_bytes)
    {
        meta.enable_equality(column);
    }

    let q_byte_start = meta.selector();
    meta.create_gate("offline cash STATE byte start", |meta| {
        let enabled = meta.query_selector(q_byte_start);
        let accumulator = meta.query_advice(byte_accumulator, Rotation::cur());
        vec![enabled * accumulator]
    });
    let q_byte_bit = meta.selector();
    meta.create_gate("offline cash STATE byte bit", |meta| {
        let enabled = meta.query_selector(q_byte_bit);
        let bit = meta.query_advice(byte_bit, Rotation::cur());
        let current = meta.query_advice(byte_accumulator, Rotation::cur());
        let next = meta.query_advice(byte_accumulator, Rotation::next());
        let one = Expression::Constant(F::ONE);
        vec![
            enabled.clone() * bit.clone() * (bit.clone() - one),
            enabled * (next - current * Expression::Constant(F::from(2)) - bit),
        ]
    });
    let q_byte = meta.selector();
    meta.create_gate("offline cash STATE reconstructed byte", |meta| {
        let enabled = meta.query_selector(q_byte);
        let byte = meta.query_advice(byte, Rotation::cur());
        let accumulator = meta.query_advice(byte_accumulator, Rotation::cur());
        vec![enabled * (byte - accumulator)]
    });

    let q_pack_bytes = meta.selector();
    meta.create_gate("offline cash STATE four bytes to u32 LE", |meta| {
        let enabled = meta.query_selector(q_pack_bytes);
        let packed = meta.query_advice(packed_byte_word, Rotation::cur());
        let mut coefficient = F::ONE;
        let mut reconstructed = Expression::Constant(F::ZERO);
        for column in packed_byte_lanes {
            reconstructed = reconstructed
                + meta.query_advice(column, Rotation::cur()) * Expression::Constant(coefficient);
            coefficient *= F::from(256);
        }
        vec![enabled * (packed - reconstructed)]
    });

    let q_conservation = meta.selector();
    meta.create_gate("offline cash STATE exact u128 conservation", |meta| {
        let enabled = meta.query_selector(q_conservation);
        let operation = meta.query_advice(operation, Rotation::cur());
        let receive = operation - Expression::Constant(F::ONE);
        let before = meta.query_advice(before_limb, Rotation::cur());
        let after = meta.query_advice(after_limb, Rotation::cur());
        let transfer = meta.query_advice(transfer_limb, Rotation::cur());
        let current_carry = meta.query_advice(carry, Rotation::cur());
        let next_carry = meta.query_advice(carry, Rotation::next());
        let selected_lhs = after.clone() + receive.clone() * (before.clone() - after.clone());
        let selected_rhs = before.clone() + receive * (after - before);
        let one = Expression::Constant(F::ONE);
        vec![
            enabled.clone()
                * (selected_lhs + transfer + current_carry.clone()
                    - selected_rhs
                    - next_carry.clone() * Expression::Constant(F::from(1_u64 << 32))),
            enabled.clone() * current_carry.clone() * (current_carry - one.clone()),
            enabled * next_carry.clone() * (next_carry - one),
        ]
    });

    let q_operation = meta.selector();
    meta.create_gate("offline cash STATE operation is Send or Receive", |meta| {
        let enabled = meta.query_selector(q_operation);
        let operation = meta.query_advice(operation, Rotation::cur());
        vec![
            enabled
                * (operation.clone() - Expression::Constant(F::ONE))
                * (operation - Expression::Constant(F::from(2))),
        ]
    });

    let q_sequence_low = meta.selector();
    meta.create_gate("offline cash STATE exact-next sequence low limb", |meta| {
        let enabled = meta.query_selector(q_sequence_low);
        let from = meta.query_advice(before_limb, Rotation::cur());
        let to = meta.query_advice(after_limb, Rotation::cur());
        let carry = meta.query_advice(carry, Rotation::next());
        vec![
            enabled.clone()
                * (from + Expression::Constant(F::ONE)
                    - to
                    - carry.clone() * Expression::Constant(F::from(1_u64 << 32))),
            enabled * carry.clone() * (carry - Expression::Constant(F::ONE)),
        ]
    });

    let q_sequence_high = meta.selector();
    meta.create_gate("offline cash STATE exact-next sequence high limb", |meta| {
        let enabled = meta.query_selector(q_sequence_high);
        let from = meta.query_advice(before_limb, Rotation::cur());
        let to = meta.query_advice(after_limb, Rotation::cur());
        let current_carry = meta.query_advice(carry, Rotation::cur());
        let overflow = meta.query_advice(carry, Rotation::next());
        vec![
            enabled
                * (from + current_carry
                    - to
                    - overflow * Expression::Constant(F::from(1_u64 << 32))),
        ]
    });

    let q_transfer_nonzero = meta.selector();
    meta.create_gate("offline cash STATE positive transfer", |meta| {
        let enabled = meta.query_selector(q_transfer_nonzero);
        let inverse = meta.query_advice(nonzero_inverse, Rotation::cur());
        let mut sum = Expression::Constant(F::ZERO);
        for offset in 0..AMOUNT_WORDS {
            sum = sum
                + meta.query_advice(
                    transfer_limb,
                    Rotation(i32::try_from(offset).expect("four amount limbs fit i32")),
                );
        }
        vec![enabled * (sum * inverse - Expression::Constant(F::ONE))]
    });

    let q_binding_sum_start = meta.selector();
    meta.create_gate("offline cash STATE nonzero binding sum start", |meta| {
        let enabled = meta.query_selector(q_binding_sum_start);
        let sum = meta.query_advice(nonzero_sum, Rotation::cur());
        vec![enabled * sum]
    });

    let q_binding_sum_step = meta.selector();
    meta.create_gate("offline cash STATE nonzero binding running sum", |meta| {
        let enabled = meta.query_selector(q_binding_sum_step);
        let limb = meta.query_advice(nonzero_limb, Rotation::cur());
        let sum = meta.query_advice(nonzero_sum, Rotation::cur());
        let next_sum = meta.query_advice(nonzero_sum, Rotation::next());
        vec![enabled * (next_sum - sum - limb)]
    });

    let q_binding_nonzero = meta.selector();
    meta.create_gate(
        "offline cash STATE nonzero private binding terminal",
        |meta| {
            let enabled = meta.query_selector(q_binding_nonzero);
            let inverse = meta.query_advice(nonzero_inverse, Rotation::cur());
            let sum = meta.query_advice(nonzero_sum, Rotation::cur());
            // Eight canonical u32 limbs sum to less than 2^35, so this inverse
            // check is zero exactly when all 32 source bytes are zero.
            vec![enabled * (sum * inverse - Expression::Constant(F::ONE))]
        },
    );

    let q_send_binding_nonzero = meta.selector();
    meta.create_gate(
        "offline cash STATE nonzero SendSplit private binding terminal",
        |meta| {
            let enabled = meta.query_selector(q_send_binding_nonzero);
            let operation = meta.query_advice(operation, Rotation::cur());
            let send = Expression::Constant(F::from(2)) - operation;
            let inverse = meta.query_advice(nonzero_inverse, Rotation::cur());
            let sum = meta.query_advice(nonzero_sum, Rotation::cur());
            vec![enabled * send * (sum * inverse - Expression::Constant(F::ONE))]
        },
    );

    let q_select = meta.selector();
    meta.create_gate("offline cash STATE operation-selected byte", |meta| {
        let enabled = meta.query_selector(q_select);
        let operation = meta.query_advice(operation, Rotation::cur());
        let left = meta.query_advice(select_left, Rotation::cur());
        let right = meta.query_advice(select_right, Rotation::cur());
        let output = meta.query_advice(select_output, Rotation::cur());
        let one = Expression::Constant(F::ONE);
        let two = Expression::Constant(F::from(2));
        vec![enabled * (output - (two - operation.clone()) * left - (operation - one) * right)]
    });

    let q_sha_word = meta.selector();
    meta.create_gate(
        "offline cash STATE SHA word big-endian byte binding",
        |meta| {
            let enabled = meta.query_selector(q_sha_word);
            let word = meta.query_advice(sha_word, Rotation::cur());
            let coefficients = [1_u64 << 24, 1_u64 << 16, 1_u64 << 8, 1];
            let reconstructed = sha_bytes.into_iter().zip(coefficients).fold(
                Expression::Constant(F::ZERO),
                |sum, (column, coefficient)| {
                    sum + meta.query_advice(column, Rotation::cur())
                        * Expression::Constant(F::from(coefficient))
                },
            );
            vec![enabled * (word - reconstructed)]
        },
    );

    let q_receive_sha_word = meta.selector();
    meta.create_gate("offline cash STATE ReceiveFold SHA word binding", |meta| {
        let enabled = meta.query_selector(q_receive_sha_word);
        let operation = meta.query_advice(operation, Rotation::cur());
        let receive = operation - Expression::Constant(F::ONE);
        let word = meta.query_advice(sha_word, Rotation::cur());
        let coefficients = [1_u64 << 24, 1_u64 << 16, 1_u64 << 8, 1];
        let reconstructed = sha_bytes.into_iter().zip(coefficients).fold(
            Expression::Constant(F::ZERO),
            |sum, (column, coefficient)| {
                sum + meta.query_advice(column, Rotation::cur())
                    * Expression::Constant(F::from(coefficient))
            },
        );
        vec![enabled * receive * (word - reconstructed)]
    });

    let q_send_sha_word = meta.selector();
    meta.create_gate("offline cash STATE SendSplit SHA word binding", |meta| {
        let enabled = meta.query_selector(q_send_sha_word);
        let operation = meta.query_advice(operation, Rotation::cur());
        let send = Expression::Constant(F::from(2)) - operation;
        let word = meta.query_advice(sha_word, Rotation::cur());
        let coefficients = [1_u64 << 24, 1_u64 << 16, 1_u64 << 8, 1];
        let reconstructed = sha_bytes.into_iter().zip(coefficients).fold(
            Expression::Constant(F::ZERO),
            |sum, (column, coefficient)| {
                sum + meta.query_advice(column, Rotation::cur())
                    * Expression::Constant(F::from(coefficient))
            },
        );
        vec![enabled * send * (word - reconstructed)]
    });

    OfflineCashStateRelationConfigV1 {
        byte,
        byte_bit,
        byte_accumulator,
        packed_byte_word,
        packed_byte_lanes,
        operation,
        before_limb,
        after_limb,
        transfer_limb,
        nonzero_limb,
        nonzero_sum,
        carry,
        nonzero_inverse,
        select_left,
        select_right,
        select_output,
        sha_word,
        sha_bytes,
        q_byte_start,
        q_byte_bit,
        q_byte,
        q_pack_bytes,
        q_conservation,
        q_operation,
        q_sequence_low,
        q_sequence_high,
        q_transfer_nonzero,
        q_binding_sum_start,
        q_binding_sum_step,
        q_binding_nonzero,
        q_send_binding_nonzero,
        q_select,
        q_sha_word,
        q_send_sha_word,
        q_receive_sha_word,
        sha: OfflineCashStateShaConfigV1::configure(meta),
    }
}

#[derive(Clone, Copy, Debug)]
struct AssignedStateByteV1 {
    known: Option<u8>,
    value: Value<u8>,
    cell: Cell,
}

#[derive(Clone, Copy, Debug)]
struct AssignedStateWordV1 {
    known: Option<u32>,
    value: Value<u32>,
    cell: Cell,
}

fn option_field<F: PrimeField>(value: Option<u64>) -> Value<F> {
    value.map_or_else(Value::unknown, |value| Value::known(F::from(value)))
}

fn assign_ranged_bytes_v1<F: PrimeField>(
    layouter: &mut impl Layouter<F>,
    config: &OfflineCashStateRelationConfigV1,
    label: &'static str,
    values: &[Option<u8>],
) -> Result<Vec<AssignedStateByteV1>, PlonkError> {
    layouter.assign_region(
        || label,
        |mut region| {
            let mut assigned = Vec::with_capacity(values.len());
            for (byte_index, witness_byte) in values.iter().copied().enumerate() {
                let base = byte_index * BYTE_ROWS;
                config.q_byte_start.enable(&mut region, base)?;
                region.assign_advice(config.byte_accumulator, base, Value::known(F::ZERO));
                let mut reconstructed = witness_byte.map(|_| 0_u64);
                for bit_index in 0..BYTE_BITS {
                    let row = base + bit_index;
                    config.q_byte_bit.enable(&mut region, row)?;
                    let witness_bit = witness_byte
                        .map(|byte| u64::from((byte >> (BYTE_BITS - 1 - bit_index)) & 1));
                    region.assign_advice(config.byte_bit, row, option_field::<F>(witness_bit));
                    reconstructed = reconstructed
                        .zip(witness_bit)
                        .map(|(accumulator, bit)| accumulator * 2 + bit);
                    region.assign_advice(
                        config.byte_accumulator,
                        row + 1,
                        option_field::<F>(reconstructed),
                    );
                }
                let row = base + BYTE_BITS;
                config.q_byte.enable(&mut region, row)?;
                let value = witness_byte.map_or_else(Value::unknown, Value::known);
                let cell = region
                    .assign_advice(config.byte, row, value.map(|byte| F::from(u64::from(byte))))
                    .cell();
                assigned.push(AssignedStateByteV1 {
                    known: witness_byte,
                    value,
                    cell,
                });
            }
            Ok(assigned)
        },
    )
}

fn pack_bytes_as_words_v1<F: PrimeField>(
    layouter: &mut impl Layouter<F>,
    config: &OfflineCashStateRelationConfigV1,
    label: &'static str,
    bytes: &[AssignedStateByteV1],
    expected_word_cells: Option<&[Cell]>,
) -> Result<Vec<AssignedStateWordV1>, PlonkError> {
    if bytes.len() % 4 != 0
        || expected_word_cells.is_some_and(|cells| cells.len() != bytes.len() / 4)
    {
        return Err(PlonkError::Synthesis);
    }
    layouter.assign_region(
        || label,
        |mut region| {
            let mut words = Vec::with_capacity(bytes.len() / 4);
            for (word_index, chunk) in bytes.chunks_exact(4).enumerate() {
                config.q_pack_bytes.enable(&mut region, word_index)?;
                for (lane, source) in chunk.iter().enumerate() {
                    let copied = region
                        .assign_advice(
                            config.packed_byte_lanes[lane],
                            word_index,
                            source.value.map(|byte| F::from(u64::from(byte))),
                        )
                        .cell();
                    region.constrain_equal(copied, source.cell);
                }
                let value =
                    chunk
                        .iter()
                        .enumerate()
                        .fold(Value::known(0_u32), |sum, (lane, byte)| {
                            sum.zip(byte.value)
                                .map(|(sum, byte)| sum | (u32::from(byte) << (lane * 8)))
                        });
                let cell = region
                    .assign_advice(
                        config.packed_byte_word,
                        word_index,
                        value.map(|word| F::from(u64::from(word))),
                    )
                    .cell();
                if let Some(expected) = expected_word_cells {
                    region.constrain_equal(cell, expected[word_index]);
                }
                words.push(AssignedStateWordV1 {
                    known: chunk
                        .iter()
                        .enumerate()
                        .try_fold(0_u32, |word, (lane, byte)| {
                            byte.known
                                .map(|byte| word | (u32::from(byte) << (lane * 8)))
                        }),
                    value,
                    cell,
                });
            }
            Ok(words)
        },
    )
}

fn optional_bytes<const N: usize>(value: Option<[u8; N]>) -> [Option<u8>; N] {
    std::array::from_fn(|index| value.map(|bytes| bytes[index]))
}

fn words_as_bytes<const N: usize>(
    words: Option<&[u32; STATE_ABI_WORDS]>,
    start: usize,
) -> [Option<u8>; N] {
    std::array::from_fn(|byte_index| {
        words.map(|words| words[start + byte_index / 4].to_le_bytes()[byte_index % 4])
    })
}

fn assign_public_bytes_v1<const N: usize, F: PrimeField>(
    layouter: &mut impl Layouter<F>,
    config: &OfflineCashStateRelationConfigV1,
    label: &'static str,
    words: Option<&[u32; STATE_ABI_WORDS]>,
    word_cells: &[Cell],
    start: usize,
) -> Result<[AssignedStateByteV1; N], PlonkError> {
    let bytes =
        assign_ranged_bytes_v1(layouter, config, label, &words_as_bytes::<N>(words, start))?;
    let _ = pack_bytes_as_words_v1(
        layouter,
        config,
        "bind public STATE bytes to u32 words",
        &bytes,
        Some(&word_cells[start..start + N / 4]),
    )?;
    bytes.try_into().map_err(|_| PlonkError::Synthesis)
}

fn assign_private_bytes_v1<const N: usize, F: PrimeField>(
    layouter: &mut impl Layouter<F>,
    config: &OfflineCashStateRelationConfigV1,
    label: &'static str,
    value: Option<[u8; N]>,
) -> Result<[AssignedStateByteV1; N], PlonkError> {
    assign_ranged_bytes_v1(layouter, config, label, &optional_bytes(value))?
        .try_into()
        .map_err(|_| PlonkError::Synthesis)
}

fn assign_private_amount_v1<F: PrimeField>(
    layouter: &mut impl Layouter<F>,
    config: &OfflineCashStateRelationConfigV1,
    label: &'static str,
    amount: Option<u128>,
) -> Result<
    (
        [AssignedStateByteV1; AMOUNT_BYTES],
        [AssignedStateWordV1; AMOUNT_WORDS],
    ),
    PlonkError,
> {
    let bytes = assign_private_bytes_v1(layouter, config, label, amount.map(u128::to_le_bytes))?;
    let words =
        pack_bytes_as_words_v1(layouter, config, "pack private STATE amount", &bytes, None)?;
    Ok((bytes, words.try_into().map_err(|_| PlonkError::Synthesis)?))
}

fn assign_sequence_increment_v1<F: PrimeField>(
    layouter: &mut impl Layouter<F>,
    config: &OfflineCashStateRelationConfigV1,
    from_sequence: Option<u64>,
) -> Result<
    (
        [AssignedStateByteV1; SEQUENCE_BYTES],
        [AssignedStateByteV1; SEQUENCE_BYTES],
    ),
    PlonkError,
> {
    let to_sequence = from_sequence.map(|sequence| sequence.wrapping_add(1));
    let from_bytes = assign_private_bytes_v1(
        layouter,
        config,
        "STATE current guard sequence",
        from_sequence.map(u64::to_le_bytes),
    )?;
    let to_bytes = assign_private_bytes_v1(
        layouter,
        config,
        "STATE successor guard sequence",
        to_sequence.map(u64::to_le_bytes),
    )?;
    let from_words: [AssignedStateWordV1; SEQUENCE_WORDS] = pack_bytes_as_words_v1(
        layouter,
        config,
        "pack current STATE guard sequence",
        &from_bytes,
        None,
    )?
    .try_into()
    .map_err(|_| PlonkError::Synthesis)?;
    let to_words: [AssignedStateWordV1; SEQUENCE_WORDS] = pack_bytes_as_words_v1(
        layouter,
        config,
        "pack successor STATE guard sequence",
        &to_bytes,
        None,
    )?
    .try_into()
    .map_err(|_| PlonkError::Synthesis)?;
    let low_carry = from_sequence.map(|sequence| ((sequence & u64::from(u32::MAX)) + 1) >> 32);
    layouter.assign_region(
        || "offline cash STATE exact-next guard sequence",
        |mut region| {
            config.q_sequence_low.enable(&mut region, 0)?;
            config.q_sequence_high.enable(&mut region, 1)?;
            for index in 0..SEQUENCE_WORDS {
                for (column, source) in [
                    (config.before_limb, from_words[index]),
                    (config.after_limb, to_words[index]),
                ] {
                    let copied = region
                        .assign_advice(
                            column,
                            index,
                            source.value.map(|word| F::from(u64::from(word))),
                        )
                        .cell();
                    region.constrain_equal(copied, source.cell);
                }
            }
            region.assign_advice_from_constant(
                || "zero sequence initial carry",
                config.carry,
                0,
                F::ZERO,
            )?;
            region.assign_advice(config.carry, 1, option_field::<F>(low_carry));
            region.assign_advice_from_constant(
                || "reject sequence overflow",
                config.carry,
                2,
                F::ZERO,
            )?;
            Ok(())
        },
    )?;
    Ok((from_bytes, to_bytes))
}

fn constrain_nonzero_binding_v1<F: PrimeField>(
    layouter: &mut impl Layouter<F>,
    config: &OfflineCashStateRelationConfigV1,
    label: &'static str,
    bytes: &[AssignedStateByteV1; DIGEST_BYTES],
    send_operation: Option<(Option<u32>, Cell)>,
) -> Result<(), PlonkError> {
    let words = pack_bytes_as_words_v1(
        layouter,
        config,
        "pack nonzero private STATE binding",
        bytes,
        None,
    )?;
    let words: [AssignedStateWordV1; DIGEST_WORDS] =
        words.try_into().map_err(|_| PlonkError::Synthesis)?;
    layouter.assign_region(
        || label,
        |mut region| {
            config.q_binding_sum_start.enable(&mut region, 0)?;
            region.assign_advice(config.nonzero_sum, 0, Value::known(F::ZERO));
            let mut running_sum = Some(0_u64);
            for (row, source) in words.iter().copied().enumerate() {
                config.q_binding_sum_step.enable(&mut region, row)?;
                let copied = region
                    .assign_advice(
                        config.nonzero_limb,
                        row,
                        source.value.map(|word| F::from(u64::from(word))),
                    )
                    .cell();
                region.constrain_equal(copied, source.cell);
                running_sum = running_sum
                    .zip(source.known.map(u64::from))
                    .map(|(sum, limb)| sum + limb);
                region.assign_advice(config.nonzero_sum, row + 1, option_field::<F>(running_sum));
            }
            if let Some((operation, operation_cell)) = send_operation {
                config
                    .q_send_binding_nonzero
                    .enable(&mut region, DIGEST_WORDS)?;
                let copied_operation = region
                    .assign_advice(
                        config.operation,
                        DIGEST_WORDS,
                        option_field::<F>(operation.map(u64::from)),
                    )
                    .cell();
                region.constrain_equal(copied_operation, operation_cell);
            } else {
                config.q_binding_nonzero.enable(&mut region, DIGEST_WORDS)?;
            }
            let inverse = running_sum.map_or_else(Value::unknown, |sum| {
                Value::known(Option::<F>::from(F::from(sum).invert()).unwrap_or(F::ZERO))
            });
            region.assign_advice(config.nonzero_inverse, DIGEST_WORDS, inverse);
            Ok(())
        },
    )
}

fn select_bytes_v1<const N: usize, F: PrimeField>(
    layouter: &mut impl Layouter<F>,
    config: &OfflineCashStateRelationConfigV1,
    operation: Option<u32>,
    operation_cell: Cell,
    left: &[AssignedStateByteV1; N],
    right: &[AssignedStateByteV1; N],
) -> Result<[AssignedStateByteV1; N], PlonkError> {
    let values: [Option<u8>; N] = std::array::from_fn(|index| match operation {
        Some(2) => right[index].known,
        Some(_) => left[index].known,
        None => None,
    });
    let selected = assign_ranged_bytes_v1(
        layouter,
        config,
        "range-check operation-selected STATE bytes",
        &values,
    )?;
    layouter.assign_region(
        || "bind operation-selected STATE bytes",
        |mut region| {
            for index in 0..N {
                config.q_select.enable(&mut region, index)?;
                let op = region
                    .assign_advice(
                        config.operation,
                        index,
                        option_field::<F>(operation.map(u64::from)),
                    )
                    .cell();
                region.constrain_equal(op, operation_cell);
                for (column, source) in [
                    (config.select_left, left[index]),
                    (config.select_right, right[index]),
                    (config.select_output, selected[index]),
                ] {
                    let copied = region
                        .assign_advice(
                            column,
                            index,
                            source.value.map(|byte| F::from(u64::from(byte))),
                        )
                        .cell();
                    region.constrain_equal(copied, source.cell);
                }
            }
            Ok(())
        },
    )?;
    selected.try_into().map_err(|_| PlonkError::Synthesis)
}

fn assign_conservation_v1<F: PrimeField>(
    layouter: &mut impl Layouter<F>,
    config: &OfflineCashStateRelationConfigV1,
    operation: Option<u32>,
    operation_cell: Cell,
    before: &[AssignedStateWordV1; AMOUNT_WORDS],
    after: &[AssignedStateWordV1; AMOUNT_WORDS],
    transfer: &[AssignedStateWordV1; AMOUNT_WORDS],
) -> Result<(), PlonkError> {
    let carry_values = match operation {
        Some(operation) => {
            let mut carries = [Some(0_u64); AMOUNT_WORDS + 1];
            for index in 0..AMOUNT_WORDS {
                let left = if operation == 2 {
                    before[index].known
                } else {
                    after[index].known
                };
                carries[index + 1] = left.zip(transfer[index].known).zip(carries[index]).map(
                    |((left, transfer), carry)| {
                        (u64::from(left) + u64::from(transfer) + carry) >> 32
                    },
                );
            }
            carries
        }
        _ => [None; AMOUNT_WORDS + 1],
    };
    layouter.assign_region(
        || "offline cash STATE u128 conservation",
        |mut region| {
            for index in 0..AMOUNT_WORDS {
                config.q_conservation.enable(&mut region, index)?;
                if index == 0 {
                    config.q_operation.enable(&mut region, index)?;
                }
                for (column, source) in [
                    (config.before_limb, before[index]),
                    (config.after_limb, after[index]),
                    (config.transfer_limb, transfer[index]),
                ] {
                    let copied = region
                        .assign_advice(
                            column,
                            index,
                            source.value.map(|word| F::from(u64::from(word))),
                        )
                        .cell();
                    region.constrain_equal(copied, source.cell);
                }
                let op = region
                    .assign_advice(
                        config.operation,
                        index,
                        option_field::<F>(operation.map(u64::from)),
                    )
                    .cell();
                region.constrain_equal(op, operation_cell);
                if index == 0 {
                    region.assign_advice_from_constant(
                        || "zero initial carry",
                        config.carry,
                        index,
                        F::ZERO,
                    )?;
                } else {
                    region.assign_advice(
                        config.carry,
                        index,
                        option_field::<F>(carry_values[index]),
                    );
                }
            }
            region.assign_advice_from_constant(
                || "zero final carry",
                config.carry,
                AMOUNT_WORDS,
                F::ZERO,
            )?;
            config.q_transfer_nonzero.enable(&mut region, 0)?;
            let transfer_sum = transfer.iter().try_fold(0_u64, |sum, limb| {
                limb.known.map(|limb| sum + u64::from(limb))
            });
            let inverse = transfer_sum.map_or_else(Value::unknown, |sum| {
                Value::known(Option::<F>::from(F::from(sum).invert()).unwrap_or(F::ZERO))
            });
            region.assign_advice(config.nonzero_inverse, 0, inverse);
            Ok(())
        },
    )
}

fn append_constant<F: PrimeField>(target: &mut Vec<OfflineCashStateShaByteV1<F>>, value: &[u8]) {
    target.extend(
        value
            .iter()
            .copied()
            .map(OfflineCashStateShaByteV1::constant),
    );
}

fn append_dynamic<F: PrimeField>(
    target: &mut Vec<OfflineCashStateShaByteV1<F>>,
    value: &[AssignedStateByteV1],
) {
    append_constant(
        target,
        &u64::try_from(value.len())
            .expect("fixed STATE head field length fits u64")
            .to_le_bytes(),
    );
    target.extend(
        value
            .iter()
            .map(|byte| OfflineCashStateShaByteV1::constrained(byte.value, byte.cell)),
    );
}

fn append_constant_field<F: PrimeField>(
    target: &mut Vec<OfflineCashStateShaByteV1<F>>,
    value: &[u8],
) {
    append_constant(
        target,
        &u64::try_from(value.len())
            .expect("fixed STATE field length fits u64")
            .to_le_bytes(),
    );
    append_constant(target, value);
}

fn begin_framed_message<F: PrimeField>(domain: &[u8]) -> Vec<OfflineCashStateShaByteV1<F>> {
    let mut message = Vec::new();
    append_constant(
        &mut message,
        &u64::try_from(domain.len())
            .expect("fixed STATE head domain length fits u64")
            .to_le_bytes(),
    );
    append_constant(&mut message, domain);
    message
}

fn begin_head_message<F: PrimeField>(domain: &[u8]) -> Vec<OfflineCashStateShaByteV1<F>> {
    let mut message = begin_framed_message(domain);
    append_constant_field(&mut message, &STATE_HEAD_FRAME_VERSION_V1.to_le_bytes());
    message
}

fn balance_message_v1<F: PrimeField>(
    context: &[AssignedStateByteV1; DIGEST_BYTES],
    wallet: &[AssignedStateByteV1; DIGEST_BYTES],
    device: &[AssignedStateByteV1; DIGEST_BYTES],
    policy: &[AssignedStateByteV1; DIGEST_BYTES],
    sequence: &[AssignedStateByteV1; SEQUENCE_BYTES],
    lineage: &[AssignedStateByteV1; DIGEST_BYTES],
    amount: &[AssignedStateByteV1; AMOUNT_BYTES],
    opening: &[AssignedStateByteV1; DIGEST_BYTES],
) -> Vec<OfflineCashStateShaByteV1<F>> {
    let mut message = begin_head_message(super::BALANCE_HEAD_DOMAIN_V1);
    for field in [
        context.as_slice(),
        wallet.as_slice(),
        device.as_slice(),
        policy.as_slice(),
        sequence.as_slice(),
        lineage.as_slice(),
        amount.as_slice(),
        opening.as_slice(),
    ] {
        append_dynamic(&mut message, field);
    }
    message
}

#[allow(clippy::too_many_arguments)]
fn state_lineage_message_v1<F: PrimeField>(
    operation: &[AssignedStateByteV1; 4],
    context: &[AssignedStateByteV1; DIGEST_BYTES],
    current_head: &[AssignedStateByteV1; DIGEST_BYTES],
    current_lineage: &[AssignedStateByteV1; DIGEST_BYTES],
    from_sequence: &[AssignedStateByteV1; SEQUENCE_BYTES],
    to_sequence: &[AssignedStateByteV1; SEQUENCE_BYTES],
    request: &[AssignedStateByteV1; DIGEST_BYTES],
    parent_1: &[AssignedStateByteV1; DIGEST_BYTES],
    link: &[AssignedStateByteV1; DIGEST_BYTES],
    transfer: &[AssignedStateByteV1; AMOUNT_BYTES],
) -> Vec<OfflineCashStateShaByteV1<F>> {
    let mut message = begin_framed_message(STATE_LINEAGE_DOMAIN_V1);
    append_constant_field(&mut message, &STATE_HEAD_FRAME_VERSION_V1.to_le_bytes());
    for field in [
        operation.as_slice(),
        context.as_slice(),
        current_head.as_slice(),
        current_lineage.as_slice(),
        from_sequence.as_slice(),
        to_sequence.as_slice(),
        request.as_slice(),
        parent_1.as_slice(),
        link.as_slice(),
        transfer.as_slice(),
    ] {
        append_dynamic(&mut message, field);
    }
    message
}

fn credit_message_v1<F: PrimeField>(
    context: &[AssignedStateByteV1; DIGEST_BYTES],
    request: &[AssignedStateByteV1; DIGEST_BYTES],
    receiver: &[AssignedStateByteV1; DIGEST_BYTES],
    recipient_key: &[AssignedStateByteV1; DIGEST_BYTES],
    transfer: &[AssignedStateByteV1; AMOUNT_BYTES],
    opening: &[AssignedStateByteV1; DIGEST_BYTES],
) -> Vec<OfflineCashStateShaByteV1<F>> {
    let mut message = begin_head_message(super::CREDIT_HEAD_DOMAIN_V1);
    for field in [
        context.as_slice(),
        request.as_slice(),
        receiver.as_slice(),
        recipient_key.as_slice(),
        transfer.as_slice(),
        opening.as_slice(),
    ] {
        append_dynamic(&mut message, field);
    }
    message
}

#[allow(clippy::too_many_arguments)]
fn send_split_seed_message_v1<F: PrimeField>(
    context: &[AssignedStateByteV1; DIGEST_BYTES],
    wallet: &[AssignedStateByteV1; DIGEST_BYTES],
    current_head: &[AssignedStateByteV1; DIGEST_BYTES],
    current_opening: &[AssignedStateByteV1; DIGEST_BYTES],
    guard_sequence: &[AssignedStateByteV1; SEQUENCE_BYTES],
    request: &[AssignedStateByteV1; DIGEST_BYTES],
    receiver_head: &[AssignedStateByteV1; DIGEST_BYTES],
    recipient_key: &[AssignedStateByteV1; DIGEST_BYTES],
    transfer: &[AssignedStateByteV1; AMOUNT_BYTES],
) -> Vec<OfflineCashStateShaByteV1<F>> {
    let mut message = begin_framed_message(SEND_SPLIT_SEED_DOMAIN_V1);
    for field in [
        context.as_slice(),
        wallet.as_slice(),
        current_head.as_slice(),
        current_opening.as_slice(),
        guard_sequence.as_slice(),
        request.as_slice(),
        receiver_head.as_slice(),
        recipient_key.as_slice(),
        transfer.as_slice(),
    ] {
        append_dynamic(&mut message, field);
    }
    message
}

fn send_split_branch_message_v1<F: PrimeField>(
    split_seed: &[AssignedStateByteV1; DIGEST_BYTES],
    branch: &'static [u8],
) -> Vec<OfflineCashStateShaByteV1<F>> {
    let mut message = begin_framed_message(SEND_SPLIT_BRANCH_DOMAIN_V1);
    append_dynamic(&mut message, split_seed);
    append_constant_field(&mut message, branch);
    message
}

fn receive_opening_message_v1<F: PrimeField>(
    context: &[AssignedStateByteV1; DIGEST_BYTES],
    before_opening: &[AssignedStateByteV1; DIGEST_BYTES],
    credit_opening: &[AssignedStateByteV1; DIGEST_BYTES],
    request: &[AssignedStateByteV1; DIGEST_BYTES],
    send_transition: &[AssignedStateByteV1; DIGEST_BYTES],
    transfer: &[AssignedStateByteV1; AMOUNT_BYTES],
) -> Vec<OfflineCashStateShaByteV1<F>> {
    let mut message = begin_framed_message(RECEIVE_OPENING_DOMAIN_V1);
    for field in [
        context.as_slice(),
        before_opening.as_slice(),
        credit_opening.as_slice(),
        request.as_slice(),
        send_transition.as_slice(),
        transfer.as_slice(),
    ] {
        append_dynamic(&mut message, field);
    }
    message
}

#[allow(clippy::too_many_arguments)]
fn receive_transition_message_v1<F: PrimeField>(
    context: &[AssignedStateByteV1; DIGEST_BYTES],
    balance_parent: &[AssignedStateByteV1; DIGEST_BYTES],
    credit_parent: &[AssignedStateByteV1; DIGEST_BYTES],
    request: &[AssignedStateByteV1; DIGEST_BYTES],
    send_transition: &[AssignedStateByteV1; DIGEST_BYTES],
    transfer: &[AssignedStateByteV1; AMOUNT_BYTES],
    next_amount: &[AssignedStateByteV1; AMOUNT_BYTES],
    next_head: &[AssignedStateByteV1; DIGEST_BYTES],
) -> Vec<OfflineCashStateShaByteV1<F>> {
    let mut message = begin_framed_message(RECEIVE_TRANSITION_DOMAIN_V1);
    for field in [
        context.as_slice(),
        balance_parent.as_slice(),
        credit_parent.as_slice(),
        request.as_slice(),
        send_transition.as_slice(),
        transfer.as_slice(),
        next_amount.as_slice(),
        next_head.as_slice(),
    ] {
        append_dynamic(&mut message, field);
    }
    message
}

#[allow(clippy::too_many_arguments)]
fn receive_semantic_message_v1<F: PrimeField>(
    operation: &[AssignedStateByteV1; 4],
    release: &[AssignedStateByteV1; DIGEST_BYTES],
    context: &[AssignedStateByteV1; DIGEST_BYTES],
    request: &[AssignedStateByteV1; DIGEST_BYTES],
    balance_parent: &[AssignedStateByteV1; DIGEST_BYTES],
    credit_parent: &[AssignedStateByteV1; DIGEST_BYTES],
    next_head: &[AssignedStateByteV1; DIGEST_BYTES],
    send_transition: &[AssignedStateByteV1; DIGEST_BYTES],
    receive_transition: &[AssignedStateByteV1; DIGEST_BYTES],
    transfer: &[AssignedStateByteV1; AMOUNT_BYTES],
    scale: &[AssignedStateByteV1; 4],
) -> Vec<OfflineCashStateShaByteV1<F>> {
    let mut message = begin_framed_message(RECEIVE_SEMANTIC_DOMAIN_V1);
    append_constant_field(&mut message, &STATE_HEAD_FRAME_VERSION_V1.to_le_bytes());
    for field in [
        operation.as_slice(),
        release.as_slice(),
        context.as_slice(),
        request.as_slice(),
        balance_parent.as_slice(),
        credit_parent.as_slice(),
        next_head.as_slice(),
        send_transition.as_slice(),
        receive_transition.as_slice(),
        transfer.as_slice(),
        scale.as_slice(),
    ] {
        append_dynamic(&mut message, field);
    }
    message
}

fn bind_sha_digest_v1<F: PrimeField>(
    layouter: &mut impl Layouter<F>,
    config: &OfflineCashStateRelationConfigV1,
    digest: &[OfflineCashStateShaWordV1; DIGEST_WORDS],
    expected: &[AssignedStateByteV1; DIGEST_BYTES],
) -> Result<(), PlonkError> {
    layouter.assign_region(
        || "bind SHA-256 big-endian words to canonical digest bytes",
        |mut region| {
            for (word_index, word) in digest.iter().copied().enumerate() {
                config.q_sha_word.enable(&mut region, word_index)?;
                let copied_word = region
                    .assign_advice(
                        config.sha_word,
                        word_index,
                        word.value().map(|word: u32| F::from(u64::from(word))),
                    )
                    .cell();
                region.constrain_equal(copied_word, word.cell());
                for lane in 0..4 {
                    let source = expected[word_index * 4 + lane];
                    let copied = region
                        .assign_advice(
                            config.sha_bytes[lane],
                            word_index,
                            source.value.map(|byte: u8| F::from(u64::from(byte))),
                        )
                        .cell();
                    region.constrain_equal(copied, source.cell);
                }
            }
            Ok(())
        },
    )
}

fn bind_receive_sha_digest_v1<F: PrimeField>(
    layouter: &mut impl Layouter<F>,
    config: &OfflineCashStateRelationConfigV1,
    operation: Option<u32>,
    operation_cell: Cell,
    digest: &[OfflineCashStateShaWordV1; DIGEST_WORDS],
    expected: &[AssignedStateByteV1; DIGEST_BYTES],
) -> Result<(), PlonkError> {
    layouter.assign_region(
        || "bind ReceiveFold SHA-256 word to canonical digest bytes",
        |mut region| {
            for (word_index, word) in digest.iter().copied().enumerate() {
                config.q_receive_sha_word.enable(&mut region, word_index)?;
                let copied_operation = region
                    .assign_advice(
                        config.operation,
                        word_index,
                        option_field::<F>(operation.map(u64::from)),
                    )
                    .cell();
                region.constrain_equal(copied_operation, operation_cell);
                let copied_word = region
                    .assign_advice(
                        config.sha_word,
                        word_index,
                        word.value().map(|word: u32| F::from(u64::from(word))),
                    )
                    .cell();
                region.constrain_equal(copied_word, word.cell());
                for lane in 0..4 {
                    let source = expected[word_index * 4 + lane];
                    let copied = region
                        .assign_advice(
                            config.sha_bytes[lane],
                            word_index,
                            source.value.map(|byte: u8| F::from(u64::from(byte))),
                        )
                        .cell();
                    region.constrain_equal(copied, source.cell);
                }
            }
            Ok(())
        },
    )
}

fn bind_send_sha_digest_v1<F: PrimeField>(
    layouter: &mut impl Layouter<F>,
    config: &OfflineCashStateRelationConfigV1,
    operation: Option<u32>,
    operation_cell: Cell,
    digest: &[OfflineCashStateShaWordV1; DIGEST_WORDS],
    expected: &[AssignedStateByteV1; DIGEST_BYTES],
) -> Result<(), PlonkError> {
    layouter.assign_region(
        || "bind SendSplit SHA-256 word to canonical digest bytes",
        |mut region| {
            for (word_index, word) in digest.iter().copied().enumerate() {
                config.q_send_sha_word.enable(&mut region, word_index)?;
                let copied_operation = region
                    .assign_advice(
                        config.operation,
                        word_index,
                        option_field::<F>(operation.map(u64::from)),
                    )
                    .cell();
                region.constrain_equal(copied_operation, operation_cell);
                let copied_word = region
                    .assign_advice(
                        config.sha_word,
                        word_index,
                        word.value().map(|word: u32| F::from(u64::from(word))),
                    )
                    .cell();
                region.constrain_equal(copied_word, word.cell());
                for lane in 0..4 {
                    let source = expected[word_index * 4 + lane];
                    let copied = region
                        .assign_advice(
                            config.sha_bytes[lane],
                            word_index,
                            source.value.map(|byte: u8| F::from(u64::from(byte))),
                        )
                        .cell();
                    region.constrain_equal(copied, source.cell);
                }
            }
            Ok(())
        },
    )
}

pub(in crate::zk::offline_cash_v1) fn synthesize_relation_v1<F: PrimeField>(
    words: Option<&[u32; STATE_ABI_WORDS]>,
    witness: Option<&OfflineCashStatePrivateWitnessV1>,
    config: &OfflineCashStateRelationConfigV1,
    word_cells: &[Cell],
    layouter: &mut impl Layouter<F>,
) -> Result<(), PlonkError> {
    if word_cells.len() != STATE_ABI_WORDS {
        return Err(PlonkError::Synthesis);
    }
    let operation = words.map(|words| words[STATE_OPERATION_WORD]);
    let operation_bytes = assign_public_bytes_v1::<4, F>(
        layouter,
        config,
        "STATE operation bytes",
        words,
        word_cells,
        STATE_OPERATION_WORD,
    )?;
    let release = assign_public_bytes_v1::<DIGEST_BYTES, F>(
        layouter,
        config,
        "STATE release bytes",
        words,
        word_cells,
        RELEASE_WORD_START,
    )?;
    let semantic = assign_public_bytes_v1::<DIGEST_BYTES, F>(
        layouter,
        config,
        "STATE semantic bytes",
        words,
        word_cells,
        SEMANTIC_WORD_START,
    )?;
    let context = assign_public_bytes_v1::<DIGEST_BYTES, F>(
        layouter,
        config,
        "STATE context bytes",
        words,
        word_cells,
        CONTEXT_WORD_START,
    )?;
    let request = assign_public_bytes_v1::<DIGEST_BYTES, F>(
        layouter,
        config,
        "STATE request bytes",
        words,
        word_cells,
        REQUEST_WORD_START,
    )?;
    let parent_0 = assign_public_bytes_v1::<DIGEST_BYTES, F>(
        layouter,
        config,
        "STATE parent-0 bytes",
        words,
        word_cells,
        PARENT_0_WORD_START,
    )?;
    let parent_1 = assign_public_bytes_v1::<DIGEST_BYTES, F>(
        layouter,
        config,
        "STATE parent-1 bytes",
        words,
        word_cells,
        PARENT_1_WORD_START,
    )?;
    let result = assign_public_bytes_v1::<DIGEST_BYTES, F>(
        layouter,
        config,
        "STATE result bytes",
        words,
        word_cells,
        RESULT_WORD_START,
    )?;
    let link = assign_public_bytes_v1::<DIGEST_BYTES, F>(
        layouter,
        config,
        "STATE link bytes",
        words,
        word_cells,
        LINK_WORD_START,
    )?;
    let transition = assign_public_bytes_v1::<DIGEST_BYTES, F>(
        layouter,
        config,
        "STATE transition bytes",
        words,
        word_cells,
        TRANSITION_WORD_START,
    )?;
    let transfer = assign_public_bytes_v1::<AMOUNT_BYTES, F>(
        layouter,
        config,
        "STATE transfer bytes",
        words,
        word_cells,
        AMOUNT_WORD_START,
    )?;
    let scale = assign_public_bytes_v1::<4, F>(
        layouter,
        config,
        "STATE scale bytes",
        words,
        word_cells,
        SCALE_WORD,
    )?;
    let transfer_words = pack_bytes_as_words_v1(
        layouter,
        config,
        "recover public STATE transfer words",
        &transfer,
        Some(&word_cells[AMOUNT_WORD_START..AMOUNT_WORD_START + AMOUNT_WORDS]),
    )?;
    let transfer_words: [AssignedStateWordV1; AMOUNT_WORDS] = transfer_words
        .try_into()
        .map_err(|_| PlonkError::Synthesis)?;

    let before_amount = witness.map(|witness| witness.before_amount);
    let after_amount = witness.map(|witness| witness.after_amount);
    let (before_amount_bytes, before_words) =
        assign_private_amount_v1(layouter, config, "STATE before amount", before_amount)?;
    let (after_amount_bytes, after_words) =
        assign_private_amount_v1(layouter, config, "STATE after amount", after_amount)?;
    assign_conservation_v1(
        layouter,
        config,
        operation,
        word_cells[STATE_OPERATION_WORD],
        &before_words,
        &after_words,
        &transfer_words,
    )?;
    let (before_sequence, after_sequence) = assign_sequence_increment_v1(
        layouter,
        config,
        witness.map(|witness| witness.guard_sequence),
    )?;

    let wallet = assign_private_bytes_v1(
        layouter,
        config,
        "STATE wallet binding",
        witness.map(|w| w.wallet_binding),
    )?;
    let device = assign_private_bytes_v1(
        layouter,
        config,
        "STATE guard device",
        witness.map(|w| w.guard_device_id),
    )?;
    let policy = assign_private_bytes_v1(
        layouter,
        config,
        "STATE hardware policy",
        witness.map(|w| w.hardware_policy_id),
    )?;
    let lineage = assign_private_bytes_v1(
        layouter,
        config,
        "STATE current lineage anchor",
        witness.map(|w| w.lineage_digest),
    )?;
    let next_lineage = assign_private_bytes_v1(
        layouter,
        config,
        "STATE successor lineage anchor",
        witness.map(|w| w.next_lineage_digest),
    )?;
    let send_split_seed = assign_private_bytes_v1(
        layouter,
        config,
        "STATE deterministic SendSplit seed",
        witness.map(|w| w.send_split_seed),
    )?;
    let before_opening = assign_private_bytes_v1(
        layouter,
        config,
        "STATE before opening",
        witness.map(|w| w.before_opening),
    )?;
    let after_opening = assign_private_bytes_v1(
        layouter,
        config,
        "STATE after opening",
        witness.map(|w| w.after_opening),
    )?;
    let credit_opening = assign_private_bytes_v1(
        layouter,
        config,
        "STATE credit opening",
        witness.map(|w| w.credit_opening),
    )?;
    let recipient_key = assign_private_bytes_v1(
        layouter,
        config,
        "STATE recipient key reference",
        witness.map(|w| w.recipient_key_reference),
    )?;
    for (label, binding) in [
        ("nonzero STATE wallet binding", &wallet),
        ("nonzero STATE guard device", &device),
        ("nonzero STATE hardware policy", &policy),
        ("nonzero STATE current lineage", &lineage),
        ("nonzero STATE successor lineage", &next_lineage),
        ("nonzero STATE before opening", &before_opening),
        ("nonzero STATE after opening", &after_opening),
        ("nonzero STATE credit opening", &credit_opening),
        ("nonzero STATE recipient key reference", &recipient_key),
    ] {
        constrain_nonzero_binding_v1(layouter, config, label, binding, None)?;
    }
    constrain_nonzero_binding_v1(
        layouter,
        config,
        "nonzero SendSplit deterministic seed",
        &send_split_seed,
        Some((operation, word_cells[STATE_OPERATION_WORD])),
    )?;
    let receiver = select_bytes_v1(
        layouter,
        config,
        operation,
        word_cells[STATE_OPERATION_WORD],
        &parent_1,
        &parent_0,
    )?;
    let expected_credit = select_bytes_v1(
        layouter,
        config,
        operation,
        word_cells[STATE_OPERATION_WORD],
        &link,
        &parent_1,
    )?;

    let jobs = [
        balance_message_v1::<F>(
            &context,
            &wallet,
            &device,
            &policy,
            &before_sequence,
            &lineage,
            &before_amount_bytes,
            &before_opening,
        ),
        balance_message_v1::<F>(
            &context,
            &wallet,
            &device,
            &policy,
            &after_sequence,
            &next_lineage,
            &after_amount_bytes,
            &after_opening,
        ),
        credit_message_v1::<F>(
            &context,
            &request,
            &receiver,
            &recipient_key,
            &transfer,
            &credit_opening,
        ),
        state_lineage_message_v1::<F>(
            &operation_bytes,
            &context,
            &parent_0,
            &lineage,
            &before_sequence,
            &after_sequence,
            &request,
            &parent_1,
            &link,
            &transfer,
        ),
        send_split_seed_message_v1::<F>(
            &context,
            &wallet,
            &parent_0,
            &before_opening,
            &before_sequence,
            &request,
            &parent_1,
            &recipient_key,
            &transfer,
        ),
        send_split_branch_message_v1::<F>(&send_split_seed, SEND_SPLIT_SENDER_BRANCH_V1),
        send_split_branch_message_v1::<F>(&send_split_seed, SEND_SPLIT_RECEIVER_BRANCH_V1),
        receive_opening_message_v1::<F>(
            &context,
            &before_opening,
            &credit_opening,
            &request,
            &link,
            &transfer,
        ),
        receive_transition_message_v1::<F>(
            &context,
            &parent_0,
            &parent_1,
            &request,
            &link,
            &transfer,
            &after_amount_bytes,
            &result,
        ),
        receive_semantic_message_v1::<F>(
            &operation_bytes,
            &release,
            &context,
            &request,
            &parent_0,
            &parent_1,
            &result,
            &link,
            &transition,
            &transfer,
            &scale,
        ),
    ];
    let expected_job_bytes = [
        BALANCE_HEAD_MESSAGE_BYTES_V1,
        BALANCE_HEAD_MESSAGE_BYTES_V1,
        CREDIT_HEAD_MESSAGE_BYTES_V1,
        STATE_LINEAGE_MESSAGE_BYTES_V1,
        SEND_SPLIT_SEED_MESSAGE_BYTES_V1,
        SEND_SPLIT_SENDER_BRANCH_MESSAGE_BYTES_V1,
        SEND_SPLIT_RECEIVER_BRANCH_MESSAGE_BYTES_V1,
        RECEIVE_OPENING_MESSAGE_BYTES_V1,
        RECEIVE_TRANSITION_MESSAGE_BYTES_V1,
        RECEIVE_SEMANTIC_MESSAGE_BYTES_V1,
    ];
    if jobs
        .iter()
        .zip(expected_job_bytes)
        .any(|(job, expected)| job.len() != expected)
    {
        return Err(PlonkError::Synthesis);
    }
    let digests = config.sha.synthesize_jobs(layouter, jobs)?;
    bind_sha_digest_v1(layouter, config, &digests[0], &parent_0)?;
    bind_sha_digest_v1(layouter, config, &digests[1], &result)?;
    bind_sha_digest_v1(layouter, config, &digests[2], &expected_credit)?;
    bind_sha_digest_v1(layouter, config, &digests[3], &next_lineage)?;
    bind_send_sha_digest_v1(
        layouter,
        config,
        operation,
        word_cells[STATE_OPERATION_WORD],
        &digests[4],
        &send_split_seed,
    )?;
    bind_send_sha_digest_v1(
        layouter,
        config,
        operation,
        word_cells[STATE_OPERATION_WORD],
        &digests[5],
        &after_opening,
    )?;
    bind_send_sha_digest_v1(
        layouter,
        config,
        operation,
        word_cells[STATE_OPERATION_WORD],
        &digests[6],
        &credit_opening,
    )?;
    bind_receive_sha_digest_v1(
        layouter,
        config,
        operation,
        word_cells[STATE_OPERATION_WORD],
        &digests[7],
        &after_opening,
    )?;
    bind_receive_sha_digest_v1(
        layouter,
        config,
        operation,
        word_cells[STATE_OPERATION_WORD],
        &digests[8],
        &transition,
    )?;
    bind_receive_sha_digest_v1(
        layouter,
        config,
        operation,
        word_cells[STATE_OPERATION_WORD],
        &digests[9],
        &semantic,
    )?;
    Ok(())
}
