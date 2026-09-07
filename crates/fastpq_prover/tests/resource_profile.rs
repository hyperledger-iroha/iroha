//! Opt-in measurements of the public transfer prover and its default verifier.
//!
//! Run each ignored case in a fresh, already compiled test process under
//! `/usr/bin/time -l` to measure process peak RSS without including Cargo.
//! Fixture construction, SMT witnesses, encoding, and artifact writes are outside
//! the prove/verify timers. `Prover::prove` includes its mandatory self-check.
//! `FASTPQ_RESOURCE_OUTPUT_DIR` optionally saves canonical inputs and returned
//! proofs for these developer tests. One sample is diagnostic, not a latency SLO.

use std::{fs, path::PathBuf, time::Instant};

use fastpq_isi::{
    FASTPQ_CATALOG_V1, FASTPQ_COMPOSITION_DEGREE_EXPANSION_V1, FASTPQ_FINAL_V1, FASTPQ_FINAL_V1_ID,
    FASTPQ_FRI_TERMINAL_DOMAIN_SIZE_V1, GOLDILOCKS_DIGEST384_PARAMETER_SHA3_256_V1,
};
use fastpq_prover::{
    Error, ExecutionMode, OperationKind, Proof, ProofSemantics, Prover, PublicInputs,
    StateTransition, TransitionBatch, VerifyLimits,
    gadgets::transfer::{
        attach_transfer_smt_witnesses, compute_poseidon_digest, decode_transcripts,
        transcripts_to_witnesses, verify_transcripts,
    },
    validate_batch_semantics, verify,
};
use iroha_crypto::{Algorithm, Hash, KeyPair};
use iroha_data_model::{
    account::AccountId,
    asset::id::AssetDefinitionId,
    domain::DomainId,
    fastpq::{TRANSFER_TRANSCRIPTS_METADATA_KEY, TransferDeltaTranscript, TransferTranscript},
};
use iroha_primitives::numeric::Quantity;
use norito::{json, to_bytes};

fn transfer_fixture(rows: usize) -> TransitionBatch {
    assert!(matches!(rows, 2 | 4 | 8 | 16));
    let domain = DomainId::try_new("resource", "universal").expect("fixture domain");
    let asset = AssetDefinitionId::derive_from_components(domain, "xor".parse().unwrap());
    let mut batch = TransitionBatch::new(
        FASTPQ_FINAL_V1_ID,
        PublicInputs {
            dsid: [0x3D; 16],
            slot: 23,
            perm_root: [0x33; 32],
            tx_set_hash: [0x44; 32],
            ..PublicInputs::default()
        },
    );
    let mut transcripts = Vec::with_capacity(rows / 2);
    for index in 0..rows / 2 {
        let account = |role: &str| {
            let seed: [u8; Hash::LENGTH] =
                Hash::new(format!("fastpq-resource-v1/{role}/{index:08}")).into();
            let keypair = KeyPair::try_from_seed(seed.to_vec(), Algorithm::default())
                .expect("deterministic fixture account");
            AccountId::new(keypair.public_key().clone())
        };
        let sender = account("sender");
        let receiver = account("receiver");
        let amount = 1 + index as u64;
        let sender_before = 1_000_000 + index as u64;
        let receiver_before = 500_000 + index as u64;
        let delta = TransferDeltaTranscript {
            from_account: sender.clone(),
            to_account: receiver.clone(),
            asset_definition: asset.clone(),
            amount: Quantity::from(amount),
            from_balance_before: Quantity::from(sender_before),
            from_balance_after: Quantity::from(sender_before - amount),
            to_balance_before: Quantity::from(receiver_before),
            to_balance_after: Quantity::from(receiver_before + amount),
            from_smt_witness: Default::default(),
            to_smt_witness: Default::default(),
        };
        let batch_hash = Hash::new(format!("fastpq-resource-v1/batch/{index:08}"));
        let digest = compute_poseidon_digest(&delta, &batch_hash);
        transcripts.push(TransferTranscript {
            batch_hash,
            deltas: vec![delta],
            authority_digest: Hash::new(b"fastpq-resource-v1/authority"),
            poseidon_preimage_digest: Some(digest),
        });
        for (owner, before, after) in [
            (sender, sender_before, sender_before - amount),
            (receiver, receiver_before, receiver_before + amount),
        ] {
            batch.push(StateTransition::new(
                format!("asset/{asset}/{owner}").into_bytes(),
                before.to_le_bytes().to_vec(),
                after.to_le_bytes().to_vec(),
                OperationKind::Transfer,
            ));
        }
    }
    let (old_root, new_root) =
        attach_transfer_smt_witnesses(&mut transcripts).expect("chained transfer SMT witnesses");
    batch.public_inputs.old_root = old_root;
    batch.public_inputs.new_root = new_root;
    batch.metadata.insert(
        TRANSFER_TRANSCRIPTS_METADATA_KEY.into(),
        to_bytes(&transcripts).expect("canonical transfer transcripts"),
    );
    batch.sort();
    batch
}

#[test]
fn resource_fixtures_are_deterministic_fully_witnessed_transfers() {
    for rows in [2, 4, 8, 16] {
        let batch = transfer_fixture(rows);
        assert_eq!(batch.transitions.len(), rows);
        validate_batch_semantics(&batch, ProofSemantics::TransferStateTransition)
            .expect("public transfer profile");
        let transcripts = decode_transcripts(&batch.metadata)
            .expect("decode transcripts")
            .expect("transfer transcripts");
        assert_eq!(transcripts.len(), rows / 2);
        verify_transcripts(&batch.transitions, &transcripts).expect("matching transfer arithmetic");
        transcripts_to_witnesses(
            &transcripts,
            &batch.public_inputs.old_root,
            &batch.public_inputs.new_root,
        )
        .expect("every SMT update chains between the public roots");
        let encoded = to_bytes(&batch).expect("encode fixture");
        let decoded: TransitionBatch = norito::decode_from_bytes(&encoded).expect("decode fixture");
        assert_eq!(decoded, batch);
        assert_eq!(encoded, to_bytes(&transfer_fixture(rows)).unwrap());
    }
}

fn measure_public_cpu(rows: usize) {
    let batch = transfer_fixture(rows);
    let batch_bytes = to_bytes(&batch).expect("encode input outside measured segment");
    let output_dir = std::env::var_os("FASTPQ_RESOURCE_OUTPUT_DIR").map(PathBuf::from);
    if let Some(path) = &output_dir {
        fs::create_dir_all(path).expect("create measurement output directory");
        fs::write(
            path.join(format!("cpu_{rows}_rows.batch.norito")),
            &batch_bytes,
        )
        .expect("write canonical measured input");
        fs::write(
            path.join(format!("cpu_{rows}_rows.batch.json")),
            json::to_vec(&batch).expect("encode readable fixture"),
        )
        .expect("write readable measured input");
    }
    let prover = Prover::canonical_with_execution_mode(FASTPQ_FINAL_V1_ID, ExecutionMode::Cpu)
        .expect("canonical CPU prover");
    let started = Instant::now();
    let result = prover.prove(&batch);
    let prove_ms = started.elapsed().as_secs_f64() * 1000.0;
    let limits = VerifyLimits::default();
    let params = FASTPQ_FINAL_V1;
    let digest_parameters_sha3 = GOLDILOCKS_DIGEST384_PARAMETER_SHA3_256_V1
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let record = norito::json!({
        "measurement_version": 2_u64,
        "profile": "transfer_state_transition",
        "parameter": FASTPQ_FINAL_V1_ID,
        "execution_mode": "cpu",
        "debug_assertions": (cfg!(debug_assertions)),
        "transition_rows": rows,
        "transfer_count": (rows / 2),
        "sample_count": 1_u64,
        "prove_includes_self_verification": true,
        "fixture_and_encoding_in_timers": false,
        "batch_wire_bytes": (batch_bytes.len()),
        "prove_ms": prove_ms,
        "verify_ms": null,
        "proof_wire_bytes": null,
        "default_max_transitions": (limits.max_transitions),
        "default_max_batch_bytes": (limits.max_batch_bytes),
        "default_max_proof_bytes": (limits.max_proof_bytes),
        "default_max_queries": (limits.max_queries),
        "default_max_fri_layers": (limits.max_fri_layers),
        "default_max_query_chunk_values": (limits.max_query_chunk_values),
        "default_max_query_path_len": (limits.max_query_path_len),
        "default_max_fri_round_values": (limits.max_fri_round_values),
        "default_max_air_row_values": (limits.max_air_row_values),
        "default_byte_limits_metric": "approximate_payload_size_hint",
        "wire_bytes_metric": "complete_norito_to_bytes_output",
        "parameter_descriptor": {
            "name": (params.name),
            "catalog": FASTPQ_CATALOG_V1,
            "required_security_bits": (params.required_security_bits),
            "grinding_bits": (params.grinding_bits),
            "trace_log_size": (params.trace_log_size),
            "trace_root": (params.trace_root),
            "lde_log_size": (params.lde_log_size),
            "lde_root": (params.lde_root),
            "omega_coset": (params.omega_coset),
            "field": (params.field.name),
            "modulus_decimal": (params.field.modulus_decimal),
            "extension_degree": (params.field.extension_degree),
            "extension_polynomial": (params.field.extension_polynomial),
            "trace_commitment_hash": (params.hash.trace_commitment),
            "transcript_hash": (params.hash.transcript),
            "digest_bytes": (params.hash.digest_bytes),
            "digest_parameter_sha3_256": digest_parameters_sha3,
            "fri_arity": (params.fri.arity),
            "fri_blowup_factor": (params.fri.blowup_factor),
            "fri_max_reductions": (params.fri.max_reductions),
            "configured_queries": (params.fri.queries),
            "terminal_domain_size": FASTPQ_FRI_TERMINAL_DOMAIN_SIZE_V1,
            "composition_degree_expansion": FASTPQ_COMPOSITION_DEGREE_EXPANSION_V1,
        },
    });
    let mut record = record.as_object().expect("measurement object").clone();
    let mut unexpected_error = None;
    match result {
        Ok(proof) => {
            let started = Instant::now();
            let verified = verify(&batch, &proof);
            let verify_ms = started.elapsed().as_secs_f64() * 1000.0;
            let proof_bytes = to_bytes(&proof).expect("encode returned proof outside timer");
            let decoded: Proof = norito::decode_from_bytes(&proof_bytes)
                .expect("decode returned proof outside timer");
            assert_eq!(
                decoded, proof,
                "recorded wire bytes reproduce the returned proof"
            );
            record.insert("verify_ms".into(), norito::json!(verify_ms));
            record.insert("proof_wire_bytes".into(), norito::json!(proof_bytes.len()));
            record.insert("proof_wire_roundtrip_matches".into(), norito::json!(true));
            record.insert(
                "proof_protocol_version".into(),
                norito::json!(proof.protocol_version),
            );
            record.insert(
                "proof_parameter".into(),
                norito::json!(proof.parameter.clone()),
            );
            record.insert(
                "lde_domain_size".into(),
                norito::json!(proof.lde_domain_size),
            );
            record.insert("query_count".into(), norito::json!(proof.queries.len()));
            record.insert(
                "air_opening_count".into(),
                norito::json!(proof.air_openings.len()),
            );
            record.insert(
                "fri_query_count".into(),
                norito::json!(proof.fri_queries.len()),
            );
            record.insert("air_alpha_count".into(), norito::json!(proof.alphas.len()));
            record.insert("fri_beta_count".into(), norito::json!(proof.betas.len()));
            record.insert(
                "fri_layer_count".into(),
                norito::json!(proof.fri_layers.len()),
            );
            record.insert(
                "air_columns".into(),
                norito::json!(
                    proof
                        .air_openings
                        .first()
                        .map(|opening| opening.current_row.len())
                ),
            );
            match verified {
                Ok(()) => {
                    record.insert("status".into(), norito::json!("accepted"));
                }
                Err(error) => {
                    record.insert("status".into(), norito::json!("unexpected_verify_error"));
                    record.insert("error".into(), norito::json!(error.to_string()));
                    unexpected_error = Some(error);
                }
            }
            if let Some(path) = &output_dir {
                fs::write(
                    path.join(format!("cpu_{rows}_rows.proof.norito")),
                    proof_bytes,
                )
                .expect("write returned proof");
            }
        }
        Err(Error::VerifierLimitExceeded { limit, actual, max }) => {
            record.insert("status".into(), norito::json!("rejected_by_default_limit"));
            record.insert("limit".into(), norito::json!(limit));
            record.insert("limit_actual".into(), norito::json!(actual));
            record.insert("limit_max".into(), norito::json!(max));
            // The public API deliberately does not expose a rejected proof. Do not
            // replace unknown wire bytes with the verifier's approximate size hint.
        }
        Err(error) => {
            record.insert("status".into(), norito::json!("unexpected_prove_error"));
            record.insert("error".into(), norito::json!(error.to_string()));
            unexpected_error = Some(error);
        }
    }
    let encoded = json::to_string(&record).expect("encode compact measurement");
    println!("{encoded}");
    if let Some(path) = &output_dir {
        fs::write(
            path.join(format!("cpu_{rows}_rows.measurement.json")),
            encoded,
        )
        .expect("write measurement");
    }
    assert!(unexpected_error.is_none(), "{unexpected_error:?}");
}

#[test]
#[ignore = "public CPU resource measurement; run alone under /usr/bin/time -l"]
fn public_transfer_cpu_2_rows() {
    measure_public_cpu(2);
}

#[test]
#[ignore = "public CPU resource measurement; run alone under /usr/bin/time -l"]
fn public_transfer_cpu_4_rows() {
    measure_public_cpu(4);
}

#[test]
#[ignore = "public CPU resource measurement; run alone under /usr/bin/time -l"]
fn public_transfer_cpu_8_rows() {
    measure_public_cpu(8);
}

#[test]
#[ignore = "public CPU resource measurement; run alone under /usr/bin/time -l"]
fn public_transfer_cpu_16_rows() {
    measure_public_cpu(16);
}
