//! Regression fixtures for the FASTPQ stage 2 backend artifacts.

use std::fs;

use fastpq_prover::{
    ExecutionMode, OperationKind, Prover, PublicInputs, StateTransition, TransitionBatch,
};
use iroha_crypto::{Algorithm, Hash, KeyPair};
use iroha_data_model::{
    account::AccountId,
    asset::id::AssetDefinitionId,
    domain::DomainId,
    fastpq::{TRANSFER_TRANSCRIPTS_METADATA_KEY, TransferDeltaTranscript, TransferTranscript},
};
use iroha_primitives::numeric::Numeric;
use norito::core::to_bytes;
use norito::to_bytes as norito_bytes;

fn synthetic_batch(rows: usize) -> TransitionBatch {
    let mut batch = TransitionBatch::new("fastpq-lane-balanced", PublicInputs::default());
    let mut transcripts = Vec::new();
    let mut row_idx = 0usize;
    let mut transfer_idx = 0usize;
    while row_idx < rows {
        match row_idx % 3 {
            0 if row_idx + 1 < rows => {
                let (transcript, sender, receiver) = transfer_pair(transfer_idx);
                batch.push(sender);
                batch.push(receiver);
                transcripts.push(transcript);
                row_idx += 2;
                transfer_idx = transfer_idx.wrapping_add(1);
            }
            1 => {
                let key = format!("asset/xor#fixture/mint/{row_idx:08}").into_bytes();
                let pre = (row_idx as u64).to_le_bytes().to_vec();
                let post = (row_idx as u64 + 1).to_le_bytes().to_vec();
                batch.push(StateTransition::new(key, pre, post, OperationKind::Mint));
                row_idx += 1;
            }
            _ => {
                let key = format!("asset/xor#fixture/burn/{row_idx:08}").into_bytes();
                let pre = (row_idx as u64 + 2).to_le_bytes().to_vec();
                let post = (row_idx as u64 + 1).to_le_bytes().to_vec();
                batch.push(StateTransition::new(key, pre, post, OperationKind::Burn));
                row_idx += 1;
            }
        }
    }
    if !transcripts.is_empty() {
        batch.metadata.insert(
            TRANSFER_TRANSCRIPTS_METADATA_KEY.into(),
            norito_bytes(&transcripts).expect("encode transcripts"),
        );
    }
    batch.sort();
    batch.public_inputs.dsid = [0u8; 16];
    batch.public_inputs.slot = 0;
    batch.public_inputs.old_root = [0u8; 32];
    batch.public_inputs.new_root = [0u8; 32];
    batch.public_inputs.perm_root = [0u8; 32];
    batch.public_inputs.tx_set_hash = [0u8; 32];
    batch
}

fn deterministic_account(label: &str, domain: &DomainId) -> AccountId {
    let seed: [u8; Hash::LENGTH] = Hash::new(format!("{label}@{domain}")).into();
    let keypair = KeyPair::from_seed(seed.to_vec(), Algorithm::default());
    let _ = domain;
    AccountId::new(keypair.public_key().clone())
}

fn transfer_pair(index: usize) -> (TransferTranscript, StateTransition, StateTransition) {
    let asset_definition = AssetDefinitionId::new(
        DomainId::try_new("fixture", "universal").unwrap(),
        "xor".parse().unwrap(),
    );
    let domain = DomainId::try_new("fixture", "universal").expect("domain id");
    let from_account = deterministic_account(&format!("sender_{index:08}"), &domain);
    let to_account = deterministic_account(&format!("receiver_{index:08}"), &domain);
    let amount = 1 + (index as u64 % 100);
    let from_pre = 1_000_000u64 + index as u64;
    let from_post = from_pre.saturating_sub(amount);
    let to_pre = 500_000u64 + index as u64;
    let to_post = to_pre.saturating_add(amount);
    let delta = TransferDeltaTranscript {
        from_account: from_account.clone(),
        to_account: to_account.clone(),
        asset_definition: asset_definition.clone(),
        amount: Numeric::from(amount),
        from_balance_before: Numeric::from(from_pre),
        from_balance_after: Numeric::from(from_post),
        to_balance_before: Numeric::from(to_pre),
        to_balance_after: Numeric::from(to_post),
        from_merkle_proof: None,
        to_merkle_proof: None,
    };
    let mut payload = Vec::with_capacity(32);
    payload.extend_from_slice(b"fastpq-stage2");
    payload.extend_from_slice(&(index as u64).to_le_bytes());
    let batch_hash = Hash::new(payload);
    let digest = fastpq_prover::gadgets::transfer::compute_poseidon_digest(&delta, &batch_hash);
    let transcript = TransferTranscript {
        batch_hash,
        deltas: vec![delta],
        authority_digest: Hash::new(b"authority"),
        poseidon_preimage_digest: Some(digest),
    };
    let sender = StateTransition::new(
        format!("asset/{asset_definition}/{from_account}").into_bytes(),
        from_pre.to_le_bytes().to_vec(),
        from_post.to_le_bytes().to_vec(),
        OperationKind::Transfer,
    );
    let receiver = StateTransition::new(
        format!("asset/{asset_definition}/{to_account}").into_bytes(),
        to_pre.to_le_bytes().to_vec(),
        to_post.to_le_bytes().to_vec(),
        OperationKind::Transfer,
    );
    (transcript, sender, receiver)
}

#[test]
fn stage2_artifact_balanced_1k_matches_fixture() {
    let prover = Prover::canonical_with_execution_mode("fastpq-lane-balanced", ExecutionMode::Cpu)
        .expect("prover");
    let batch = synthetic_batch(1_000);
    let proof = prover.prove(&batch).expect("proof");
    let expected = include_bytes!("fixtures/stage2_balanced_1k.bin");
    let encoded = to_bytes(&proof).expect("encode proof");
    if expected.is_empty() {
        println!(
            "generating stage2_balanced_1k.bin ({} bytes)",
            encoded.len()
        );
        fs::write("tests/fixtures/stage2_balanced_1k.bin", &encoded).expect("write fixture");
        panic!("wrote stage2_balanced_1k.bin fixture; re-run tests");
    }
    if std::env::var("FASTPQ_UPDATE_FIXTURES").is_ok() {
        fs::write("tests/fixtures/stage2_balanced_1k.bin", &encoded).expect("write fixture");
        return;
    }
    assert_eq!(encoded.as_slice(), expected);
}

#[cfg(feature = "fastpq-gpu")]
#[test]
fn stage2_artifact_balanced_cpu_gpu_parity() {
    if !matches!(ExecutionMode::Auto.resolve(), ExecutionMode::Gpu) {
        eprintln!("skipping cpu/gpu parity test; gpu backend unavailable");
        return;
    }
    let expected = include_bytes!("fixtures/stage2_balanced_1k.bin");
    let batch = synthetic_batch(1_000);
    let cpu = Prover::canonical_with_execution_mode("fastpq-lane-balanced", ExecutionMode::Cpu)
        .expect("cpu prover");
    let gpu = Prover::canonical_with_execution_mode("fastpq-lane-balanced", ExecutionMode::Gpu)
        .expect("gpu prover");
    let cpu_proof = cpu.prove(&batch).expect("cpu proof");
    let gpu_proof = gpu.prove(&batch).expect("gpu proof");
    let cpu_encoded = to_bytes(&cpu_proof).expect("encode cpu proof");
    let gpu_encoded = to_bytes(&gpu_proof).expect("encode gpu proof");
    assert_eq!(
        cpu_encoded.as_slice(),
        expected,
        "cpu proof should match canonical stage2 fixture"
    );
    assert_eq!(
        gpu_encoded.as_slice(),
        expected,
        "gpu proof should match canonical stage2 fixture"
    );
    assert_eq!(cpu_encoded, gpu_encoded, "stage2 cpu/gpu proofs must match");
}

#[test]
fn stage2_artifact_balanced_5k_matches_fixture() {
    let prover = Prover::canonical_with_execution_mode("fastpq-lane-balanced", ExecutionMode::Cpu)
        .expect("prover");
    let batch = synthetic_batch(5_000);
    let proof = prover.prove(&batch).expect("proof");
    let expected = include_bytes!("fixtures/stage2_balanced_5k.bin");
    let encoded = to_bytes(&proof).expect("encode proof");
    if expected.is_empty() {
        println!(
            "generating stage2_balanced_5k.bin ({} bytes)",
            encoded.len()
        );
        fs::write("tests/fixtures/stage2_balanced_5k.bin", &encoded).expect("write fixture");
        panic!("wrote stage2_balanced_5k.bin fixture; re-run tests");
    }
    if std::env::var("FASTPQ_UPDATE_FIXTURES").is_ok() {
        fs::write("tests/fixtures/stage2_balanced_5k.bin", &encoded).expect("write fixture");
        return;
    }
    assert_eq!(encoded.as_slice(), expected);
}
