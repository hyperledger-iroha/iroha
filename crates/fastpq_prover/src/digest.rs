use fastpq_isi::StarkParameterSet;
use iroha_crypto::Hash;

use crate::{
    Error, Result,
    batch::TransitionBatch,
    trace::{ColumnDigests, Trace, build_trace, column_hashes, merkle_root_with_first_level},
};

/// Domain separator applied to the Stage 1 commitment payload.
const TRACE_COMMITMENT_DOMAIN: &[u8] = b"fastpq:v1:trace_commitment";

/// Compute the deterministic commitment over a transition batch.
///
/// The commitment is derived by building the canonical FASTPQ trace,
/// hashing each column with Poseidon2, folding the resulting digests into a
/// Poseidon2 Merkle root, and finally hashing a length-prefixed payload with
/// `Hash::new`. The payload captures the domain separator, parameter name, trace
/// dimensions, column digests, and the Merkle root so downstream consumers can
/// validate the commitment using only public data.
///
/// # Errors
///
/// Returns [`Error::ParameterMismatch`] when the provided parameters do not
/// match the batch annotation, or propagates Norito encoding failures.
pub fn trace_commitment(params: &StarkParameterSet, batch: &TransitionBatch) -> Result<Hash> {
    if params.name != batch.parameter {
        return Err(Error::ParameterMismatch {
            expected: params.name.to_string(),
            actual: batch.parameter.clone(),
        });
    }
    let trace = build_trace(batch)?;
    let column_digests = column_hashes(&trace, params)?;
    trace_commitment_from_digests(params, &trace, &column_digests)
}

pub fn trace_commitment_from_digests(
    params: &StarkParameterSet,
    trace: &Trace,
    column_digests: &ColumnDigests,
) -> Result<Hash> {
    let root =
        merkle_root_with_first_level(column_digests.leaves(), column_digests.fused_parents());

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

    let mut payload = Vec::with_capacity(
        TRACE_COMMITMENT_DOMAIN.len()
            + params.name.len()
            + core::mem::size_of_val(column_digests.leaves())
            + 32,
    );
    append_length_prefixed(&mut payload, TRACE_COMMITMENT_DOMAIN)?;
    append_length_prefixed(&mut payload, params.name.as_bytes())?;
    payload.extend_from_slice(&rows.to_le_bytes());
    payload.extend_from_slice(&padded_len.to_le_bytes());
    payload.extend_from_slice(&column_count.to_le_bytes());
    for digest in column_digests.leaves() {
        payload.extend_from_slice(&digest.to_le_bytes());
    }
    payload.extend_from_slice(&root.to_le_bytes());

    Ok(Hash::new(payload))
}

fn append_length_prefixed(buffer: &mut Vec<u8>, bytes: &[u8]) -> Result<(), Error> {
    let len: u64 = bytes
        .len()
        .try_into()
        .map_err(|_| Error::PayloadLengthOverflow {
            length: bytes.len(),
        })?;
    buffer.extend_from_slice(&len.to_le_bytes());
    buffer.extend_from_slice(bytes);
    Ok(())
}

#[cfg(test)]
mod tests {
    use fastpq_isi::CANONICAL_PARAMETER_SETS;

    use super::*;
    use crate::{
        OperationKind, Planner, PublicInputs, StateTransition, TransitionBatch,
        backend::{self, ExecutionMode},
        gadgets::transfer,
        trace::{
            ColumnDigests, RowUsage, TraceColumn, derive_polynomial_data,
            hash_columns_from_coefficients,
        },
    };
    use iroha_data_model::{
        DomainId,
        asset::id::AssetDefinitionId,
        fastpq::{TRANSFER_TRANSCRIPTS_METADATA_KEY, TransferDeltaTranscript, TransferTranscript},
    };
    use iroha_primitives::numeric::Numeric;
    use iroha_test_samples::{ALICE_ID, BOB_ID};
    use norito::to_bytes;

    fn sample_batch() -> TransitionBatch {
        let mut batch = TransitionBatch::new("fastpq-lane-balanced", PublicInputs::default());
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
            OperationKind::Mint,
        ));
        batch.push(StateTransition::new(
            b"asset/xor/bob".to_vec(),
            u64::to_le_bytes(500).to_vec(),
            u64::to_le_bytes(475).to_vec(),
            OperationKind::Burn,
        ));
        batch.sort();
        batch
    }

    fn build_fixture(name: &str) -> TransitionBatch {
        let mut batch = TransitionBatch::new("fastpq-lane-balanced", PublicInputs::default());
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
            "mint" => {
                batch.push(StateTransition::new(
                    b"asset/xor/reserve".to_vec(),
                    u64_bytes(4_096),
                    u64_bytes(5_120),
                    OperationKind::Mint,
                ));
                batch.push(StateTransition::new(
                    b"asset/xor/treasury".to_vec(),
                    u64_bytes(64),
                    u64_bytes(1_024),
                    OperationKind::Mint,
                ));
            }
            "burn" => {
                batch.push(StateTransition::new(
                    b"asset/xor/liability".to_vec(),
                    u64_bytes(8_192),
                    u64_bytes(6_656),
                    OperationKind::Burn,
                ));
                batch.push(StateTransition::new(
                    b"asset/xor/supply".to_vec(),
                    u64_bytes(16_384),
                    u64_bytes(14_848),
                    OperationKind::Burn,
                ));
            }
            other => panic!("unknown fixture {other}"),
        }

        batch.sort();
        batch
    }

    fn u64_bytes(value: u64) -> Vec<u8> {
        value.to_le_bytes().to_vec()
    }

    fn sample_transfer_transcript() -> TransferTranscript {
        let mut delta = TransferDeltaTranscript {
            from_account: (*ALICE_ID).clone(),
            to_account: (*BOB_ID).clone(),
            asset_definition: AssetDefinitionId::new(
                DomainId::try_new("fixture", "universal").unwrap(),
                "xor".parse().unwrap(),
            ),
            amount: Numeric::from(75u32),
            from_balance_before: Numeric::from(1_000u32),
            from_balance_after: Numeric::from(925u32),
            to_balance_before: Numeric::from(75u32),
            to_balance_after: Numeric::from(150u32),
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

    fn numeric_u64(value: &Numeric) -> u64 {
        iroha_data_model::fastpq::normalized_numeric_to_u64(value, value.scale())
            .expect("numeric fits")
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

    fn numeric_to_bytes(value: &Numeric) -> Vec<u8> {
        let amount: u64 = value.clone().try_into().expect("numeric fits u64");
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
        let params = CANONICAL_PARAMETER_SETS
            .iter()
            .find(|set| set.name == "fastpq-lane-latency")
            .copied()
            .expect("canonical latency parameter set");
        let batch = sample_batch();

        let err = trace_commitment(&params, &batch).unwrap_err();

        assert!(matches!(
            err,
            Error::ParameterMismatch {
                expected,
                actual
            } if expected == "fastpq-lane-latency" && actual == "fastpq-lane-balanced"
        ));
    }

    #[test]
    fn trace_commitment_from_digests_binds_parameter_name_trace_shape_and_leaves() {
        let balanced = CANONICAL_PARAMETER_SETS
            .iter()
            .find(|set| set.name == "fastpq-lane-balanced")
            .copied()
            .expect("canonical balanced parameter set");
        let latency = CANONICAL_PARAMETER_SETS
            .iter()
            .find(|set| set.name == "fastpq-lane-latency")
            .copied()
            .expect("canonical latency parameter set");
        let trace = synthetic_trace();
        let digests = ColumnDigests::new(vec![1, 2, 3], None);

        let base =
            trace_commitment_from_digests(&balanced, &trace, &digests).expect("base commitment");
        let other_parameter =
            trace_commitment_from_digests(&latency, &trace, &digests).expect("parameter change");
        assert_ne!(base, other_parameter);

        let mut row_changed = trace.clone();
        row_changed.rows = 2;
        let other_rows =
            trace_commitment_from_digests(&balanced, &row_changed, &digests).expect("row change");
        assert_ne!(base, other_rows);

        let mut padded_changed = trace.clone();
        padded_changed.padded_len = 2;
        let other_padded = trace_commitment_from_digests(&balanced, &padded_changed, &digests)
            .expect("padded length change");
        assert_ne!(base, other_padded);

        let mut column_changed = trace.clone();
        column_changed.columns.push(TraceColumn {
            name: "extra".to_owned(),
            values: vec![0],
        });
        let other_columns = trace_commitment_from_digests(&balanced, &column_changed, &digests)
            .expect("column count change");
        assert_ne!(base, other_columns);

        let other_leaves = trace_commitment_from_digests(
            &balanced,
            &trace,
            &ColumnDigests::new(vec![1, 2, 4], None),
        )
        .expect("leaf change");
        assert_ne!(base, other_leaves);
    }

    #[test]
    fn trace_commitment_from_digests_uses_fused_parent_roots() {
        let params = CANONICAL_PARAMETER_SETS
            .iter()
            .find(|set| set.name == "fastpq-lane-balanced")
            .copied()
            .expect("canonical balanced parameter set");
        let trace = synthetic_trace();
        let leaves = vec![11, 22, 33, 44];
        let scalar = ColumnDigests::new(leaves.clone(), None);
        let fused = ColumnDigests::new(leaves, Some(vec![55, 66]));

        let scalar_commitment =
            trace_commitment_from_digests(&params, &trace, &scalar).expect("scalar commitment");
        let fused_commitment =
            trace_commitment_from_digests(&params, &trace, &fused).expect("fused commitment");

        assert_ne!(scalar_commitment, fused_commitment);
    }

    #[test]
    fn append_length_prefixed_writes_little_endian_length_and_payload() {
        let mut payload = vec![0xAA];

        append_length_prefixed(&mut payload, b"fastpq").unwrap();

        assert_eq!(payload[0], 0xAA);
        assert_eq!(&payload[1..9], &6u64.to_le_bytes());
        assert_eq!(&payload[9..], b"fastpq");
    }

    #[test]
    fn commitment_matches_manual_merkle() {
        let params = CANONICAL_PARAMETER_SETS
            .iter()
            .find(|set| set.name == "fastpq-lane-balanced")
            .copied()
            .expect("canonical parameter set");
        let planner = Planner::new(&params);
        let cases = [
            ("synthetic", sample_batch()),
            ("transfer", build_fixture("transfer")),
            ("mint", build_fixture("mint")),
            ("burn", build_fixture("burn")),
        ];

        for (label, batch) in cases {
            let commitment = trace_commitment(&params, &batch).expect("trace commitment");
            let trace = build_trace(&batch).expect("build trace");
            let data = derive_polynomial_data(&trace, &planner, ExecutionMode::Cpu);
            let digests = hash_columns_from_coefficients(
                &trace,
                &data.coefficients,
                &planner,
                ExecutionMode::Cpu,
                crate::trace::PoseidonPipelinePolicy::for_mode(ExecutionMode::Cpu),
            );
            let manual =
                trace_commitment_from_digests(&params, &trace, &digests).expect("manual commit");
            assert_eq!(
                commitment, manual,
                "{label} manual commitment must match trace_commitment()"
            );
        }
    }

    #[test]
    fn row_hash_extension_is_idempotent_for_cpu_mode() {
        let params = CANONICAL_PARAMETER_SETS
            .iter()
            .find(|set| set.name == "fastpq-lane-balanced")
            .copied()
            .expect("canonical parameter set");
        let planner = Planner::new(&params);
        let cases = [
            ("synthetic", sample_batch()),
            ("transfer", build_fixture("transfer")),
            ("mint", build_fixture("mint")),
            ("burn", build_fixture("burn")),
        ];

        for (label, batch) in cases {
            let trace = build_trace(&batch).expect("trace");
            let mut data = derive_polynomial_data(&trace, &planner, ExecutionMode::Cpu);
            let row_hashes = backend::hash_trace_rows(data.lde_columns());
            let extended = backend::extend_row_hashes(
                &planner,
                ExecutionMode::Cpu,
                row_hashes.clone(),
                trace.padded_len,
            );
            assert_eq!(
                extended, row_hashes,
                "{label} CPU extension should not alter hashed LDE rows"
            );
        }
    }
}
