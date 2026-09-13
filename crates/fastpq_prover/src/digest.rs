//! Canonical six-lane preprocessing trace commitment framing.
use crate::{
    Error, Result,
    batch::TransitionBatch,
    field::GOLDILOCKS_MODULUS_V1,
    trace::{Trace, TraceColumn, build_trace},
};
use fastpq_isi::{
    FASTPQ_CATALOG_V1, FASTPQ_FINAL_V1_ID, GoldilocksDigest384FrameV1,
    GoldilocksDigest384V1 as NativeDigest384V1, GoldilocksDigestDomainV1, StarkParameterSet,
    hash_bytes_384_v1,
};
use iroha_data_model::privacy::GoldilocksDigest384V1;

/// Typed role for the canonical preprocessing-trace commitment tree.
const TRACE_COMMITMENT_ROLE_V1: &[u8] = b"fastpq:v1:preprocessing-trace";
/// Typed phase for one named trace-column leaf.
const TRACE_COLUMN_LEAF_PHASE_V1: &[u8] = b"column-leaf";
/// Typed phase for a binary trace-commitment interior node.
const TRACE_NODE_PHASE_V1: &[u8] = b"binary-node";
/// Typed phase for an empty trace-commitment tree.
const TRACE_EMPTY_PHASE_V1: &[u8] = b"empty-tree";
/// Typed phase for the final shape-bound preprocessing commitment.
const TRACE_FINAL_PHASE_V1: &[u8] = b"final-commitment";
/// Compute the deterministic commitment over a transition batch.
///
/// The commitment is derived by building the canonical FASTPQ trace,
/// hashing each named column into six independently parameterised Poseidon-x7
/// Goldilocks lanes, folding those digests through a typed binary Merkle tree,
/// and binding the exact trace shape into one final six-lane digest. The typed
/// frame binds the final catalog, protocol, profile, tree role, phase,
/// level/index, lane, and counter; no legacy 32-byte hash participates in this
/// native-STARK commitment.
///
/// # Errors
///
/// Returns [`Error::ParameterMismatch`] when the provided parameters do not
/// match the batch annotation, [`Error::TraceDomainCapacityExceeded`] when the
/// padded rows exceed the parameter domain, [`Error::VerifierLimitExceeded`]
/// when the canonical trace schema is too wide, or propagates trace encoding
/// failures.
pub fn trace_commitment(
    params: &StarkParameterSet,
    batch: &TransitionBatch,
) -> Result<GoldilocksDigest384V1> {
    if params.name != batch.parameter {
        return Err(Error::ParameterMismatch {
            expected: params.name.to_string(),
            actual: batch.parameter.clone(),
        });
    }
    ensure_trace_capacity(params, batch.transitions.len())?;
    crate::trace::ensure_trace_schema_limit(batch, crate::trace::DEFAULT_MAX_TRACE_COLUMNS)?;
    let trace = build_trace(batch)?;
    trace_commitment_from_trace(params, &trace)
}
/// Ensure the mandatory power-of-two trace padding fits the selected parameter domain.
///
/// This check lives ahead of trace construction so oversized statements fail with a structured
/// error instead of reaching assertion-based FFT planner geometry.
pub fn ensure_trace_capacity(params: &StarkParameterSet, transition_rows: usize) -> Result<()> {
    let padded_rows =
        transition_rows
            .max(1)
            .checked_next_power_of_two()
            .ok_or(Error::TraceLengthOverflow {
                rows: transition_rows,
            })?;
    let max_rows = 1usize
        .checked_shl(params.trace_log_size)
        .ok_or(Error::TraceLengthOverflow {
            rows: transition_rows,
        })?;
    if padded_rows > max_rows {
        return Err(Error::TraceDomainCapacityExceeded {
            rows: transition_rows,
            padded_rows,
            max_rows,
        });
    }
    Ok(())
}
fn validate_trace_shape(trace: &Trace, params: &StarkParameterSet) -> Result<()> {
    if trace.columns.len() > crate::trace::DEFAULT_MAX_TRACE_COLUMNS {
        return Err(Error::VerifierLimitExceeded {
            limit: "max_air_row_values",
            actual: trace.columns.len(),
            max: crate::trace::DEFAULT_MAX_TRACE_COLUMNS,
        });
    }
    let required_padded_len = trace
        .rows
        .max(1)
        .checked_next_power_of_two()
        .ok_or(Error::TraceLengthOverflow { rows: trace.rows })?;
    if trace.padded_len != required_padded_len {
        return Err(Error::InvalidTraceShape {
            details: format!(
                "padded length {} does not match canonical length {required_padded_len} for {} rows",
                trace.padded_len, trace.rows
            ),
        });
    }
    let max_rows = 1usize
        .checked_shl(params.trace_log_size)
        .ok_or(Error::TraceLengthOverflow {
            rows: trace.padded_len,
        })?;
    if trace.padded_len > max_rows {
        return Err(Error::TraceDomainCapacityExceeded {
            rows: trace.rows,
            padded_rows: trace.padded_len,
            max_rows,
        });
    }
    for (index, column) in trace.columns.iter().enumerate() {
        if column.values.len() != trace.padded_len {
            return Err(Error::InvalidTraceShape {
                details: format!(
                    "column {index} (`{}`) has length {}, expected {}",
                    column.name,
                    column.values.len(),
                    trace.padded_len
                ),
            });
        }
    }
    Ok(())
}
pub(crate) fn trace_commitment_from_trace(
    params: &StarkParameterSet,
    trace: &Trace,
) -> Result<GoldilocksDigest384V1> {
    validate_trace_shape(trace, params)?;
    let root = trace_column_root_v1(params, trace)?;
    let rows: u64 = trace
        .rows
        .try_into()
        .map_err(|_| Error::TraceLengthOverflow { rows: trace.rows })?;
    let padded_len: u64 = trace
        .padded_len
        .try_into()
        .map_err(|_| Error::TraceLengthOverflow {
            rows: trace.padded_len,
        })?;
    let column_count: u64 =
        trace
            .columns
            .len()
            .try_into()
            .map_err(|_| Error::PayloadLengthOverflow {
                length: trace.columns.len(),
            })?;
    let rows = rows.to_le_bytes();
    let padded_len = padded_len.to_le_bytes();
    let column_count = column_count.to_le_bytes();
    hash_trace_bytes_v1(
        params,
        TRACE_FINAL_PHASE_V1,
        0,
        0,
        &[&rows, &padded_len, &column_count, &root.to_le_bytes()],
    )
    .map(Into::into)
}

fn trace_column_root_v1(params: &StarkParameterSet, trace: &Trace) -> Result<NativeDigest384V1> {
    let mut current =
        hash_trace_columns_v1(params, &trace.columns, trace.padded_len, &mut |frames| {
            crate::digest_executor::execute_digest384_frames_v1(
                frames,
                crate::digest_executor::DigestExecutionV1::Cpu,
            )
        })?;

    if current.is_empty() {
        return hash_trace_bytes_v1(params, TRACE_EMPTY_PHASE_V1, 0, 0, &[]);
    }
    let mut level = 1_usize;
    while current.len() > 1 {
        if !current.len().is_multiple_of(2) {
            current.push(*current.last().expect("non-empty trace commitment level"));
        }
        current = hash_trace_pairs_v1(params, &current, level, &mut |frames| {
            crate::digest_executor::execute_digest384_frames_v1(
                frames,
                crate::digest_executor::DigestExecutionV1::Cpu,
            )
        })?;
        level = level
            .checked_add(1)
            .ok_or(Error::QueryIndexOverflow { index: level })?;
    }
    Ok(current[0])
}

// Bound canonical byte copies while allowing device dispatch to batch complete
// column frames. No prefix seed or scalar digest projection participates.
const TRACE_COLUMN_PREPARATION_BYTES_V1: usize = 8 * 1024 * 1024;

pub(crate) fn hash_trace_columns_v1(
    params: &StarkParameterSet,
    columns: &[TraceColumn],
    rows: usize,
    execute: &mut impl FnMut(&[GoldilocksDigest384FrameV1<'_>]) -> Result<Vec<NativeDigest384V1>>,
) -> Result<Vec<NativeDigest384V1>> {
    let column_bytes = rows
        .checked_mul(8)
        .ok_or(Error::PayloadLengthOverflow { length: rows })?;
    // Reject malformed/noncanonical inputs before any execution callback.
    for (index, column) in columns.iter().enumerate() {
        if column.values.len() != rows {
            return Err(Error::InvalidTraceShape {
                details: format!(
                    "column `{}` has {} values; expected {}",
                    column.name,
                    column.values.len(),
                    rows
                ),
            });
        }
        for (row, value) in column.values.iter().enumerate() {
            if *value >= GOLDILOCKS_MODULUS_V1 {
                return Err(Error::NonCanonicalGoldilocksElement {
                    context: "trace_commitment_column",
                    indices: vec![index, row],
                });
            }
        }
    }
    let chunk_columns = (TRACE_COLUMN_PREPARATION_BYTES_V1 / column_bytes.max(1)).max(1);
    let mut output = Vec::with_capacity(columns.len());
    for (chunk_index, chunk) in columns.chunks(chunk_columns).enumerate() {
        let encoded: Vec<Vec<u8>> = chunk
            .iter()
            .map(|column| {
                let mut bytes = Vec::with_capacity(column_bytes);
                for value in &column.values {
                    bytes.extend_from_slice(&value.to_le_bytes());
                }
                bytes
            })
            .collect();
        let fields: Vec<[&[u8]; 2]> = chunk
            .iter()
            .zip(&encoded)
            .map(|(column, values)| [column.name.as_bytes(), values.as_slice()])
            .collect();
        let frames = fields
            .iter()
            .enumerate()
            .map(|(local_index, fields)| {
                let index = chunk_index * chunk_columns + local_index;
                GoldilocksDigest384FrameV1::new(
                    trace_digest_domain_v1(params, TRACE_COLUMN_LEAF_PHASE_V1, 0, index)?,
                    fields,
                )
                .ok_or(Error::PayloadLengthOverflow {
                    length: column_bytes,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let digests = execute(&frames)?;
        if digests.len() != frames.len() {
            return Err(Error::NativeDigestExecution {
                details: "trace column executor returned an incorrect digest count".into(),
            });
        }
        output.extend(digests);
    }
    Ok(output)
}

pub(crate) fn hash_trace_pairs_v1(
    params: &StarkParameterSet,
    children: &[NativeDigest384V1],
    level: usize,
    execute: &mut impl FnMut(&[GoldilocksDigest384FrameV1<'_>]) -> Result<Vec<NativeDigest384V1>>,
) -> Result<Vec<NativeDigest384V1>> {
    crate::digest_executor::hash_digest384_pairs_v1(
        children,
        |index| trace_digest_domain_v1(params, TRACE_NODE_PHASE_V1, level, index),
        execute,
    )
}

fn hash_trace_bytes_v1(
    params: &StarkParameterSet,
    phase: &[u8],
    level: usize,
    index: usize,
    fields: &[&[u8]],
) -> Result<NativeDigest384V1> {
    let domain = trace_digest_domain_v1(params, phase, level, index)?;
    hash_bytes_384_v1(domain, fields).ok_or_else(|| Error::PayloadLengthOverflow {
        length: fields
            .iter()
            .fold(0_usize, |total, field| total.saturating_add(field.len())),
    })
}

fn trace_digest_domain_v1<'a>(
    params: &'a StarkParameterSet,
    phase: &'a [u8],
    level: usize,
    index: usize,
) -> Result<GoldilocksDigestDomainV1<'a>> {
    Ok(GoldilocksDigestDomainV1 {
        catalog: FASTPQ_CATALOG_V1.as_bytes(),
        protocol: FASTPQ_FINAL_V1_ID.as_bytes(),
        profile: params.name.as_bytes(),
        role: TRACE_COMMITMENT_ROLE_V1,
        phase,
        level: u64::try_from(level).map_err(|_| Error::QueryIndexOverflow { index: level })?,
        index: u64::try_from(index).map_err(|_| Error::QueryIndexOverflow { index })?,
        counter: 0,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        OperationKind, PublicInputs, StateTransition, TransitionBatch,
        gadgets::transfer,
        trace::{RowUsage, TraceColumn},
    };
    use fastpq_isi::CANONICAL_PARAMETER_SETS;
    use iroha_crypto::Hash;
    use iroha_data_model::{
        asset::id::AssetDefinitionId,
        fastpq::{TRANSFER_TRANSCRIPTS_METADATA_KEY, TransferDeltaTranscript, TransferTranscript},
    };
    use iroha_model_base::domain::DomainId;
    use iroha_primitives::numeric::Quantity;
    use iroha_test_samples::{ALICE_ID, BOB_ID};
    use norito::to_bytes;
    fn sample_batch() -> TransitionBatch {
        let mut batch =
            TransitionBatch::new("fastpq-state-transition-stark-v1", PublicInputs::default());
        batch.public_inputs.dsid = [0xAA; 16];
        batch.public_inputs.slot = 42;
        batch.public_inputs.old_root = [0x11; 32];
        batch.public_inputs.new_root = [0x22; 32];
        batch.public_inputs.perm_root = [0x33; 32];
        batch.public_inputs.tx_set_hash = [0x44; 32];
        batch.push(StateTransition::new(
            b"asset/xor/alice".to_vec(),
            u64::to_le_bytes(1_000).to_vec(),
            u64::to_le_bytes(1_100).to_vec(),
            OperationKind::MetaSet,
        ));
        batch.push(StateTransition::new(
            b"asset/xor/bob".to_vec(),
            u64::to_le_bytes(500).to_vec(),
            u64::to_le_bytes(475).to_vec(),
            OperationKind::MetaSet,
        ));
        batch.sort();
        batch
    }
    fn build_fixture(name: &str) -> TransitionBatch {
        let mut batch =
            TransitionBatch::new("fastpq-state-transition-stark-v1", PublicInputs::default());
        batch.public_inputs.dsid = [0xAA; 16];
        batch.public_inputs.slot = 42;
        batch.public_inputs.old_root = [0x11; 32];
        batch.public_inputs.new_root = [0x22; 32];
        batch.public_inputs.perm_root = [0x33; 32];
        batch.public_inputs.tx_set_hash = [0x44; 32];
        match name {
            "transfer" => {
                let transcript = sample_transfer_transcript();
                let delta = transcript
                    .deltas
                    .first()
                    .expect("transfer fixture has delta");
                batch.public_inputs.old_root = delta.from_smt_witness.root_before;
                batch.public_inputs.new_root = delta.to_smt_witness.root_after;
                for transition in sample_transfer_transitions(&transcript) {
                    batch.push(transition);
                }
                batch.metadata.insert(
                    TRANSFER_TRANSCRIPTS_METADATA_KEY.into(),
                    to_bytes(&vec![transcript]).expect("encode transcripts"),
                );
            }
            other => panic!("unknown fixture {other}"),
        }
        batch.sort();
        batch
    }
    fn sample_transfer_transcript() -> TransferTranscript {
        let mut delta = TransferDeltaTranscript {
            from_account: (*ALICE_ID).clone(),
            to_account: (*BOB_ID).clone(),
            asset_definition: AssetDefinitionId::derive_from_components(
                DomainId::try_new("fixture", "universal").unwrap(),
                "xor".parse().unwrap(),
            ),
            amount: Quantity::from(75u32),
            from_balance_before: Quantity::from(1_000u32),
            from_balance_after: Quantity::from(925u32),
            to_balance_before: Quantity::from(75u32),
            to_balance_after: Quantity::from(150u32),
            from_smt_witness: iroha_data_model::fastpq::TransferSmtWitness::default(),
            to_smt_witness: iroha_data_model::fastpq::TransferSmtWitness::default(),
        };
        attach_delta_witnesses(&mut delta);
        let batch_hash = Hash::prehashed([0x11; 32]);
        let digest = crate::gadgets::transfer::compute_poseidon_digest(&delta, &batch_hash);
        TransferTranscript {
            batch_hash,
            deltas: vec![delta],
            authority_digest: Hash::new(b"authority"),
            poseidon_preimage_digest: Some(digest),
        }
    }
    fn attach_delta_witnesses(delta: &mut TransferDeltaTranscript) {
        let sender_key = iroha_data_model::fastpq::transfer_balance_key(
            &delta.asset_definition,
            &delta.from_account,
        )
        .expect("canonical balance key");
        let receiver_key = iroha_data_model::fastpq::transfer_balance_key(
            &delta.asset_definition,
            &delta.to_account,
        )
        .expect("canonical balance key");
        let (from, to) = transfer::build_transfer_smt_witness_pair(
            &sender_key,
            numeric_u64(&delta.from_balance_before),
            numeric_u64(&delta.from_balance_after),
            &receiver_key,
            numeric_u64(&delta.to_balance_before),
            numeric_u64(&delta.to_balance_after),
        )
        .expect("transfer witness");
        delta.from_smt_witness = from;
        delta.to_smt_witness = to;
    }
    fn numeric_u64(value: &Quantity) -> u64 {
        iroha_data_model::fastpq::normalized_numeric_to_u64(value.as_numeric(), value.scale())
            .expect("quantity fits")
    }
    fn sample_transfer_transitions(transcript: &TransferTranscript) -> Vec<StateTransition> {
        transcript
            .deltas
            .iter()
            .flat_map(|delta| {
                let sender = StateTransition::new(
                    iroha_data_model::fastpq::transfer_balance_key(
                        &delta.asset_definition,
                        &delta.from_account,
                    )
                    .expect("canonical balance key"),
                    numeric_to_bytes(&delta.from_balance_before),
                    numeric_to_bytes(&delta.from_balance_after),
                    OperationKind::Transfer,
                );
                let receiver = StateTransition::new(
                    iroha_data_model::fastpq::transfer_balance_key(
                        &delta.asset_definition,
                        &delta.to_account,
                    )
                    .expect("canonical balance key"),
                    numeric_to_bytes(&delta.to_balance_before),
                    numeric_to_bytes(&delta.to_balance_after),
                    OperationKind::Transfer,
                );
                [sender, receiver]
            })
            .collect()
    }
    fn numeric_to_bytes(value: &Quantity) -> Vec<u8> {
        let amount: u64 = value
            .as_numeric()
            .clone()
            .try_into()
            .expect("quantity fits u64");
        amount.to_le_bytes().to_vec()
    }
    fn synthetic_trace() -> Trace {
        Trace {
            rows: 1,
            padded_len: 1,
            columns: vec![TraceColumn {
                name: "synthetic".to_owned(),
                values: vec![9],
            }],
            transfer_witnesses: Vec::new(),
            row_usage: RowUsage {
                total_rows: 1,
                ..RowUsage::default()
            },
        }
    }
    #[test]
    fn trace_commitment_rejects_parameter_mismatch_before_trace_build() {
        let mut params = CANONICAL_PARAMETER_SETS[0];
        params.name = "retired-fastpq-profile-v0";
        let batch = sample_batch();
        let err = trace_commitment(&params, &batch).unwrap_err();
        assert!(matches!(
            err,
            Error::ParameterMismatch {
                expected,
                actual
            } if expected == "retired-fastpq-profile-v0" && actual == "fastpq-state-transition-stark-v1"
        ));
    }
    #[test]
    fn trace_commitment_rejects_rows_exceeding_parameter_domain_without_panicking() {
        let mut params = CANONICAL_PARAMETER_SETS[0];
        params.trace_log_size = 1;
        let mut batch = TransitionBatch::new(params.name, PublicInputs::default());
        for key in [b"a".as_slice(), b"b".as_slice(), b"c".as_slice()] {
            batch.push(StateTransition::new(
                key.to_vec(),
                Vec::new(),
                Vec::new(),
                OperationKind::MetaSet,
            ));
        }

        let error =
            trace_commitment(&params, &batch).expect_err("three rows exceed a two-row domain");
        assert!(matches!(
            error,
            Error::TraceDomainCapacityExceeded {
                rows: 3,
                padded_rows: 4,
                max_rows: 2,
            }
        ));
        ensure_trace_capacity(&params, 0).expect("empty trace still occupies one padding row");
    }
    #[test]
    fn trace_commitment_rejects_wide_schema_before_trace_allocation() {
        let params = CANONICAL_PARAMETER_SETS[0];
        let mut batch = TransitionBatch::new(params.name, PublicInputs::default());
        batch.push(StateTransition::new(
            b"wide-value".to_vec(),
            vec![0xA5; (crate::trace::DEFAULT_MAX_TRACE_COLUMNS + 1) * crate::LIMB_BYTES],
            Vec::new(),
            OperationKind::MetaSet,
        ));
        let actual = crate::trace::column_count_for_batch(&batch).expect("schema count");
        let error = trace_commitment(&params, &batch)
            .expect_err("wide commitment schema must fail before materialisation");
        assert!(matches!(
            error,
            Error::VerifierLimitExceeded {
                limit: "max_air_row_values",
                actual: observed,
                max: crate::trace::DEFAULT_MAX_TRACE_COLUMNS,
            } if observed == actual
        ));
    }
    #[test]
    fn trace_commitment_from_trace_binds_parameter_name_shape_names_and_values() {
        let canonical = CANONICAL_PARAMETER_SETS[0];
        let mut relabelled = canonical;
        relabelled.name = "different-fastpq-profile-v1";
        let trace = synthetic_trace();
        let base = trace_commitment_from_trace(&canonical, &trace).expect("base commitment");
        let other_parameter =
            trace_commitment_from_trace(&relabelled, &trace).expect("parameter change");
        assert_ne!(base, other_parameter);
        let mut row_changed = trace.clone();
        row_changed.rows = 0;
        let other_rows = trace_commitment_from_trace(&canonical, &row_changed).expect("row change");
        assert_ne!(base, other_rows);
        let mut padded_changed = trace.clone();
        padded_changed.padded_len = 2;
        assert!(matches!(
            trace_commitment_from_trace(&canonical, &padded_changed),
            Err(Error::InvalidTraceShape { .. })
        ));
        let mut name_changed = trace.clone();
        name_changed.columns[0].name = "renamed".to_owned();
        assert_ne!(
            base,
            trace_commitment_from_trace(&canonical, &name_changed).expect("column-name change")
        );
        let mut value_changed = trace.clone();
        value_changed.columns[0].values[0] = 10;
        assert_ne!(
            base,
            trace_commitment_from_trace(&canonical, &value_changed).expect("column-value change")
        );
        let mut extra_column = trace.clone();
        extra_column.columns.push(TraceColumn {
            name: "extra".to_owned(),
            values: vec![0],
        });
        assert_ne!(
            base,
            trace_commitment_from_trace(&canonical, &extra_column).expect("column-count change")
        );
        let mut noncanonical = trace;
        noncanonical.columns[0].values[0] = GOLDILOCKS_MODULUS_V1;
        assert!(matches!(
            trace_commitment_from_trace(&canonical, &noncanonical),
            Err(Error::NonCanonicalGoldilocksElement {
                context: "trace_commitment_column",
                indices,
            }) if indices == vec![0, 0]
        ));
    }
    fn independent_hash(
        phase: &[u8],
        level: usize,
        index: usize,
        fields: &[&[u8]],
    ) -> NativeDigest384V1 {
        let params = CANONICAL_PARAMETER_SETS[0];
        hash_bytes_384_v1(
            GoldilocksDigestDomainV1 {
                catalog: FASTPQ_CATALOG_V1.as_bytes(),
                protocol: FASTPQ_FINAL_V1_ID.as_bytes(),
                profile: params.name.as_bytes(),
                role: b"fastpq:v1:preprocessing-trace",
                phase,
                level: u64::try_from(level).unwrap(),
                index: u64::try_from(index).unwrap(),
                counter: 0,
            },
            fields,
        )
        .unwrap()
    }
    #[test]
    fn complete_column_tree_matches_independent_framing_for_empty_odd_and_parallel_shapes() {
        let params = CANONICAL_PARAMETER_SETS[0];
        for count in [0, 1, 2, 3, 7, 65] {
            let columns: Vec<_> = (0..count)
                .map(|index| TraceColumn {
                    name: format!("col_{index:02}"),
                    values: vec![index as u64, index as u64 + 1],
                })
                .collect();
            let mut expected: Vec<_> = columns
                .iter()
                .enumerate()
                .map(|(index, column)| {
                    let bytes: Vec<_> = column
                        .values
                        .iter()
                        .flat_map(|word| word.to_le_bytes())
                        .collect();
                    independent_hash(b"column-leaf", 0, index, &[column.name.as_bytes(), &bytes])
                })
                .collect();
            let actual = hash_trace_columns_v1(&params, &columns, 2, &mut |frames| {
                crate::digest_executor::execute_digest384_frames_v1(
                    frames,
                    crate::digest_executor::DigestExecutionV1::Cpu,
                )
            })
            .unwrap();
            assert_eq!(
                actual, expected,
                "every six-lane column and its absolute index must match"
            );
            let mut level = 1;
            while expected.len() > 1 {
                expected = expected
                    .chunks(2)
                    .enumerate()
                    .map(|(index, pair)| {
                        let left = pair[0].to_le_bytes();
                        let right = pair.last().unwrap().to_le_bytes();
                        independent_hash(b"binary-node", level, index, &[&left, &right])
                    })
                    .collect();
                level += 1;
            }
            let expected_root = expected
                .first()
                .copied()
                .unwrap_or_else(|| independent_hash(b"empty-tree", 0, 0, &[]));
            let trace = Trace {
                rows: 2,
                padded_len: 2,
                columns,
                transfer_witnesses: Vec::new(),
                row_usage: RowUsage::default(),
            };
            assert_eq!(
                trace_column_root_v1(&params, &trace).unwrap(),
                expected_root,
                "column count {count}"
            );
            let rows = 2_u64.to_le_bytes();
            let count_bytes = (count as u64).to_le_bytes();
            let expected_commitment: GoldilocksDigest384V1 = independent_hash(
                b"final-commitment",
                0,
                0,
                &[&rows, &rows, &count_bytes, &expected_root.to_le_bytes()],
            )
            .into();
            assert_eq!(
                trace_commitment_from_trace(&params, &trace).unwrap(),
                expected_commitment
            );
        }
    }
    #[test]
    fn canonical_column_executor_rejects_late_invalid_input_and_wrong_output_count() {
        let params = CANONICAL_PARAMETER_SETS[0];
        let mut columns = vec![
            TraceColumn {
                name: "first".into(),
                values: vec![1, 2],
            },
            TraceColumn {
                name: "late".into(),
                values: vec![3, GOLDILOCKS_MODULUS_V1],
            },
        ];
        assert!(
            hash_trace_columns_v1(&params, &columns, 2, &mut |_| panic!(
                "all columns must preflight before execution"
            ))
            .is_err()
        );
        columns[1].values.pop();
        assert!(
            hash_trace_columns_v1(&params, &columns, 2, &mut |_| panic!(
                "all shapes must preflight before execution"
            ))
            .is_err()
        );
        columns[1].values.push(4);
        assert!(hash_trace_columns_v1(&params, &columns, 2, &mut |_| Ok(Vec::new())).is_err());
        let value = NativeDigest384V1::new([1; 6]).unwrap();
        assert!(
            hash_trace_pairs_v1(&params, &[value], 1, &mut |_| panic!(
                "incomplete pairs must not dispatch"
            ))
            .is_err()
        );
        assert!(hash_trace_pairs_v1(&params, &[value, value], 1, &mut |_| Ok(Vec::new())).is_err());
    }
    #[test]
    fn every_child_digest_lane_and_tree_coordinate_changes_the_parent() {
        let params = CANONICAL_PARAMETER_SETS[0];
        let left = NativeDigest384V1::new([1, 2, 3, 4, 5, 6]).unwrap();
        let right = NativeDigest384V1::new([7, 8, 9, 10, 11, 12]).unwrap();
        let base = independent_hash(
            b"binary-node",
            1,
            0,
            &[&left.to_le_bytes(), &right.to_le_bytes()],
        );
        for child in 0..2 {
            for lane in 0..6 {
                let mut words = if child == 0 {
                    [1, 2, 3, 4, 5, 6]
                } else {
                    [7, 8, 9, 10, 11, 12]
                };
                words[lane] += 1;
                let changed = NativeDigest384V1::new(words).unwrap();
                let children = if child == 0 {
                    [changed, right]
                } else {
                    [left, changed]
                };
                let parent = hash_trace_pairs_v1(&params, &children, 1, &mut |frames| {
                    Ok(frames
                        .iter()
                        .map(GoldilocksDigest384FrameV1::hash)
                        .collect())
                })
                .unwrap();
                assert_ne!(parent[0], base, "child {child} lane {lane}");
            }
        }
        for (phase, level, index) in [
            (b"binary-node".as_slice(), 2, 0),
            (b"binary-node".as_slice(), 1, 1),
            (b"column-leaf".as_slice(), 1, 0),
        ] {
            assert_ne!(
                independent_hash(
                    phase,
                    level,
                    index,
                    &[&left.to_le_bytes(), &right.to_le_bytes()]
                ),
                base
            );
        }
        assert_ne!(
            independent_hash(
                b"binary-node",
                1,
                0,
                &[&right.to_le_bytes(), &left.to_le_bytes()]
            ),
            base
        );
    }
    #[test]
    fn commitment_matches_manual_merkle() {
        let params = CANONICAL_PARAMETER_SETS
            .iter()
            .find(|set| set.name == "fastpq-state-transition-stark-v1")
            .copied()
            .expect("canonical parameter set");
        let cases = [
            ("synthetic", sample_batch()),
            ("transfer", build_fixture("transfer")),
        ];
        for (label, batch) in cases {
            let commitment = trace_commitment(&params, &batch).expect("trace commitment");
            let trace = build_trace(&batch).expect("build trace");
            let manual = trace_commitment_from_trace(&params, &trace).expect("manual commitment");
            assert_eq!(
                commitment, manual,
                "{label} manual commitment must match trace_commitment()"
            );
        }
    }
}
