//! Golden Stage 2 proof fixture parity; regeneration gated by `FASTPQ_UPDATE_FIXTURES`.

use std::{env, fs, path::PathBuf};

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
use std::str::FromStr;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// Construct the regression batch used for Stage 2 fixtures.
fn stage2_fixture_batch(rows: usize) -> TransitionBatch {
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
    AccountId::new(domain.clone(), keypair.public_key().clone())
}

fn transfer_pair(index: usize) -> (TransferTranscript, StateTransition, StateTransition) {
    let asset_definition = AssetDefinitionId::from_str("xor#fixture").expect("asset definition");
    let domain = DomainId::from_str("fixture").expect("domain id");
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
fn golden_stage2_proof_matches_fixture() {
    let path = fixture_path("stage2_balanced_1k.bin");
    if env::var("FASTPQ_UPDATE_FIXTURES").is_ok() {
        let prover =
            Prover::canonical_with_execution_mode("fastpq-lane-balanced", ExecutionMode::Cpu)
                .unwrap();
        let batch = stage2_fixture_batch(1_000);
        let proof = prover.prove(&batch).expect("generate proof for fixture");
        let reencoded = to_bytes(&proof).expect("encode regenerated proof");
        fs::write(&path, &reencoded).expect("write regenerated fixture");
        return;
    }

    let bytes = fs::read(&path).expect("read proof fixture");
    let prover =
        Prover::canonical_with_execution_mode("fastpq-lane-balanced", ExecutionMode::Cpu).unwrap();
    let batch = stage2_fixture_batch(1_000);
    let proof = prover.prove(&batch).expect("produce proof for comparison");
    let reencoded = to_bytes(&proof).expect("encode proof for comparison");
    assert_eq!(
        reencoded, bytes,
        "golden proof fixture drifted; regenerate with FASTPQ_UPDATE_FIXTURES=1"
    );
}
