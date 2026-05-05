//! Stage 4 transcript replay regression fixtures.

#[cfg(feature = "fastpq-gpu")]
use std::{env, fs, path::Path};

#[cfg(feature = "fastpq-gpu")]
use fastpq_prover::{
    OperationKind, Proof, Prover, PublicInputs, StateTransition, TransitionBatch, verify,
};
#[cfg(feature = "fastpq-gpu")]
use iroha_crypto::{Algorithm, Hash, KeyPair};
#[cfg(feature = "fastpq-gpu")]
use iroha_data_model::{
    account::AccountId,
    asset::id::AssetDefinitionId,
    domain::DomainId,
    fastpq::{TRANSFER_TRANSCRIPTS_METADATA_KEY, TransferDeltaTranscript, TransferTranscript},
};
#[cfg(feature = "fastpq-gpu")]
use iroha_primitives::numeric::Numeric;
#[cfg(feature = "fastpq-gpu")]
use norito::core::to_bytes;
#[cfg(feature = "fastpq-gpu")]
use norito::to_bytes as norito_bytes;
#[cfg(feature = "fastpq-gpu")]
#[cfg(feature = "fastpq-gpu")]
const FIXTURE_NAME: &str = "stage4_balanced_preview.bin";

#[cfg(feature = "fastpq-gpu")]
fn annotate_batch(batch: &mut TransitionBatch) {
    batch.public_inputs.dsid = [0x11; 16];
    batch.public_inputs.slot = 42;
    batch.public_inputs.old_root = [0xAA; 32];
    batch.public_inputs.new_root = [0xBB; 32];
    batch.public_inputs.perm_root = [0xCC; 32];
    batch.public_inputs.tx_set_hash = [0xDD; 32];
}

#[cfg(feature = "fastpq-gpu")]
fn preview_fixture_batch(rows: usize) -> TransitionBatch {
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
                let key = format!("asset/xor#fixture/mint/{row_idx:04}").into_bytes();
                let pre = (row_idx as u64).to_le_bytes().to_vec();
                let post = (row_idx as u64 + 1).to_le_bytes().to_vec();
                batch.push(StateTransition::new(key, pre, post, OperationKind::Mint));
                row_idx += 1;
            }
            _ => {
                let key = format!("asset/xor#fixture/burn/{row_idx:04}").into_bytes();
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
    annotate_batch(&mut batch);
    batch
}

#[cfg(feature = "fastpq-gpu")]
fn fixture_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(FIXTURE_NAME)
}

#[cfg(feature = "fastpq-gpu")]
fn deterministic_account(label: &str, domain: &DomainId) -> AccountId {
    let seed: [u8; Hash::LENGTH] = Hash::new(format!("{label}@{domain}")).into();
    let keypair = KeyPair::from_seed(seed.to_vec(), Algorithm::default());
    let _ = domain;
    AccountId::new(keypair.public_key().clone())
}

#[cfg(feature = "fastpq-gpu")]
fn transfer_pair(index: usize) -> (TransferTranscript, StateTransition, StateTransition) {
    let asset_definition = AssetDefinitionId::new(
        DomainId::try_new("fixture", "universal").unwrap(),
        "xor".parse().unwrap(),
    );
    let domain = DomainId::try_new("fixture", "universal").expect("domain id");
    let from_account = deterministic_account(&format!("sender_{index:04}"), &domain);
    let to_account = deterministic_account(&format!("receiver_{index:04}"), &domain);
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
    payload.extend_from_slice(b"fastpq-stage4");
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
        format!("asset/{}/{}", asset_definition, from_account).into_bytes(),
        from_pre.to_le_bytes().to_vec(),
        from_post.to_le_bytes().to_vec(),
        OperationKind::Transfer,
    );
    let receiver = StateTransition::new(
        format!("asset/{}/{}", asset_definition, to_account).into_bytes(),
        to_pre.to_le_bytes().to_vec(),
        to_post.to_le_bytes().to_vec(),
        OperationKind::Transfer,
    );
    (transcript, sender, receiver)
}

#[cfg(feature = "fastpq-gpu")]
#[test]
fn stage4_preview_fixture_replays_transcript() {
    let batch = preview_fixture_batch(64);
    let path = fixture_path();

    if env::var("FASTPQ_UPDATE_FIXTURES").is_ok() {
        let prover = Prover::canonical("fastpq-lane-balanced").expect("prover");
        let proof = prover.prove(&batch).expect("proof");
        let encoded = to_bytes(&proof).expect("encode proof");
        fs::write(&path, &encoded).expect("write fixture");
        panic!("updated {FIXTURE_NAME}; re-run tests without FASTPQ_UPDATE_FIXTURES to validate");
    }

    let expected = include_bytes!("fixtures/stage4_balanced_preview.bin");
    assert!(
        !expected.is_empty(),
        "fixture {FIXTURE_NAME} is empty; set FASTPQ_UPDATE_FIXTURES=1 and re-run tests"
    );

    let proof: Proof = norito::decode_from_bytes(expected).expect("decode proof");
    verify(&batch, &proof).expect("fixture proof verifies");

    let prover = Prover::canonical("fastpq-lane-balanced").expect("prover");
    let regenerated = prover.prove(&batch).expect("regenerate proof");
    let encoded = to_bytes(&regenerated).expect("encode regenerated proof");
    assert_eq!(
        encoded.as_slice(),
        expected,
        "regenerated proof diverged from fixture"
    );
}
