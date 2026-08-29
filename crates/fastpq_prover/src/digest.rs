use crate::{
    Error, Result,
    batch::TransitionBatch,
    field::GOLDILOCKS_MODULUS_V1,
    trace::{Trace, build_trace},
};
use fastpq_isi::{
    FASTPQ_CATALOG_V1, FASTPQ_FINAL_V1_ID, GoldilocksDigest384V1 as NativeDigest384V1,
    GoldilocksDigestDomainV1, StarkParameterSet, hash_bytes_384_v1,
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
pub(crate) fn trace_commitment_from_trace(
    params: &StarkParameterSet,
    trace: &Trace,
) -> Result<GoldilocksDigest384V1> {
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
    let mut current = trace
        .columns
        .iter()
        .enumerate()
        .map(|(index, column)| {
            if column.values.len() != trace.padded_len {
                return Err(Error::InvalidTraceShape {
                    details: format!(
                        "column `{}` has {} values; expected {}",
                        column.name,
                        column.values.len(),
                        trace.padded_len
                    ),
                });
            }
            let mut values = Vec::with_capacity(column.values.len().saturating_mul(8));
            for (row, value) in column.values.iter().copied().enumerate() {
                if value >= GOLDILOCKS_MODULUS_V1 {
                    return Err(Error::NonCanonicalGoldilocksElement {
                        context: "trace_commitment_column",
                        indices: vec![index, row],
                    });
                }
                values.extend_from_slice(&value.to_le_bytes());
            }
            hash_trace_bytes_v1(
                params,
                TRACE_COLUMN_LEAF_PHASE_V1,
                0,
                index,
                &[column.name.as_bytes(), &values],
            )
        })
        .collect::<Result<Vec<_>>>()?;

    if current.is_empty() {
        return hash_trace_bytes_v1(params, TRACE_EMPTY_PHASE_V1, 0, 0, &[]);
    }
    let mut level = 1_usize;
    while current.len() > 1 {
        if !current.len().is_multiple_of(2) {
            current.push(*current.last().expect("non-empty trace commitment level"));
        }
        current = current
            .chunks_exact(2)
            .enumerate()
            .map(|(index, children)| {
                hash_trace_bytes_v1(
                    params,
                    TRACE_NODE_PHASE_V1,
                    level,
                    index,
                    &[&children[0].to_le_bytes(), &children[1].to_le_bytes()],
                )
            })
            .collect::<Result<Vec<_>>>()?;
        level = level
            .checked_add(1)
            .ok_or(Error::QueryIndexOverflow { index: level })?;
    }
    Ok(current[0])
}

fn hash_trace_bytes_v1(
    params: &StarkParameterSet,
    phase: &[u8],
    level: usize,
    index: usize,
    fields: &[&[u8]],
) -> Result<NativeDigest384V1> {
    let domain = GoldilocksDigestDomainV1 {
        catalog: FASTPQ_CATALOG_V1.as_bytes(),
        protocol: FASTPQ_FINAL_V1_ID.as_bytes(),
        profile: params.name.as_bytes(),
        role: TRACE_COMMITMENT_ROLE_V1,
        phase,
        level: u64::try_from(level).map_err(|_| Error::QueryIndexOverflow { index: level })?,
        index: u64::try_from(index).map_err(|_| Error::QueryIndexOverflow { index })?,
        counter: 0,
    };
    hash_bytes_384_v1(domain, fields).ok_or_else(|| Error::PayloadLengthOverflow {
        length: fields
            .iter()
            .fold(0_usize, |total, field| total.saturating_add(field.len())),
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
        DomainId,
        asset::id::AssetDefinitionId,
        fastpq::{TRANSFER_TRANSCRIPTS_METADATA_KEY, TransferDeltaTranscript, TransferTranscript},
    };
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
        let sender_key =
            format!("asset/{}/{}", delta.asset_definition, delta.from_account).into_bytes();
        let receiver_key =
            format!("asset/{}/{}", delta.asset_definition, delta.to_account).into_bytes();
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
                    format!("asset/{}/{}", delta.asset_definition, delta.from_account).into_bytes(),
                    numeric_to_bytes(&delta.from_balance_before),
                    numeric_to_bytes(&delta.from_balance_after),
                    OperationKind::Transfer,
                );
                let receiver = StateTransition::new(
                    format!("asset/{}/{}", delta.asset_definition, delta.to_account).into_bytes(),
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
        row_changed.rows = 2;
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
