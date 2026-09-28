//! Final V1 raw-transcript fixture with six-lane digests and exact 32-byte Fp4 values.
use fastpq_prover::{
    Error, ExecutionMode, OperationKind, Proof, Prover, PublicInputs, StateTransition,
    TransitionBatch, VerifyLimits, gadgets::transfer::attach_transfer_smt_witnesses,
    verify_raw_statement, verify_raw_statement_with_limits,
};
use iroha_crypto::{Algorithm, Hash, KeyPair};
use iroha_data_model::{
    account::AccountId,
    asset::id::AssetDefinitionId,
    fastpq::{TRANSFER_TRANSCRIPTS_METADATA_KEY, TransferDeltaTranscript, TransferTranscript},
};
use iroha_model_base::domain::DomainId;
use iroha_primitives::numeric::Quantity;
use norito::core::to_bytes;
use std::{fs, path::Path};
const FIXTURE_NAME: &str = "v1_raw_transcript_64.bin";
// This mixed raw fixture opens all 136 queries. Keep its finite diagnostic
// budget explicit even when the default resource profile admits its size; this is
// neither a state-transition admission test nor an AXT payload-size exception.
const RAW_FIXTURE_MAX_PROOF_BYTES: usize = 2 * 1024 * 1024;
mod common;
use common::fixture_update_requested;
fn fixture_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(FIXTURE_NAME)
}
#[test]
fn v1_raw_transcript_64_fixture_verifies() {
    let mut public_inputs = PublicInputs::default();
    public_inputs.dsid = [0x11; 16];
    public_inputs.slot = 42;
    public_inputs.perm_root = [0xCC; 32];
    public_inputs.tx_set_hash = [0xDD; 32];
    let batch = v1_fixture_batch(64, public_inputs);
    let path = fixture_path();
    if fixture_update_requested() {
        let prover = Prover::canonical_with_execution_mode(
            "fastpq-state-transition-stark-v1",
            ExecutionMode::Cpu,
        )
        .expect("prover");
        let proof = prover
            .prove_raw_statement(&batch)
            .expect("raw fixture proof");
        let encoded = to_bytes(&proof).expect("encode proof");
        fs::write(&path, &encoded).expect("write fixture");
        return;
    }
    let expected = fs::read(&path).expect("read canonical proof fixture");
    assert!(
        !expected.is_empty(),
        "fixture {FIXTURE_NAME} is empty; set FASTPQ_UPDATE_FIXTURES=1 and re-run tests"
    );
    let proof: Proof = norito::decode_from_bytes(&expected).expect("decode proof");
    assert!(matches!(
        verify_raw_statement_with_limits(
            &batch,
            &proof,
            VerifyLimits {
                max_proof_bytes: 512 * 1024,
                ..VerifyLimits::default()
            }
        ),
        Err(Error::VerifierLimitExceeded {
            limit: "max_proof_bytes",
            ..
        })
    ));
    verify_raw_statement(&batch, &proof).expect("raw fixture fits derived default resources");
    let limits = VerifyLimits {
        max_proof_bytes: RAW_FIXTURE_MAX_PROOF_BYTES,
        ..VerifyLimits::default()
    };
    verify_raw_statement_with_limits(&batch, &proof, limits).expect("raw fixture proof verifies");
    let prover = Prover::canonical_with_execution_mode(
        "fastpq-state-transition-stark-v1",
        ExecutionMode::Cpu,
    )
    .expect("prover");
    let regenerated = prover
        .prove_raw_statement(&batch)
        .expect("regenerate raw fixture proof");
    let encoded = to_bytes(&regenerated).expect("encode regenerated proof");
    assert_eq!(
        encoded.as_slice(),
        expected.as_slice(),
        "regenerated proof diverged from fixture"
    );
}

/// Construct the deterministic mixed-operation batch for the raw transcript fixture.
fn v1_fixture_batch(rows: usize, public_inputs: PublicInputs) -> TransitionBatch {
    let mut batch = TransitionBatch::new("fastpq-state-transition-stark-v1", public_inputs);
    let mut transcripts = Vec::new();
    let mut row_idx = 0usize;
    let mut transfer_idx = 0usize;
    while row_idx < rows {
        if row_idx % 3 == 0 && rows - row_idx >= 2 {
            let (transcript, sender, receiver) = transfer_pair(transfer_idx);
            batch.push(sender);
            batch.push(receiver);
            transcripts.push(transcript);
            row_idx += 2;
            transfer_idx += 1;
        } else {
            let key = format!("metadata/fixture/{row_idx:08}").into_bytes();
            let pre = (row_idx as u64 + 2).to_le_bytes().to_vec();
            let post = (row_idx as u64 + 1).to_le_bytes().to_vec();
            batch.push(StateTransition::new(key, pre, post, OperationKind::MetaSet));
            row_idx += 1;
        }
    }
    let (old_root, new_root) = if transcripts.is_empty() {
        ([0u8; 32], [0u8; 32])
    } else {
        attach_transfer_smt_witnesses(&mut transcripts).expect("attach transfer SMT witnesses")
    };
    if !transcripts.is_empty() {
        batch.metadata.insert(
            TRANSFER_TRANSCRIPTS_METADATA_KEY.into(),
            to_bytes(&transcripts).expect("encode transcripts"),
        );
    }
    batch.sort();
    batch.public_inputs.old_root = old_root;
    batch.public_inputs.new_root = new_root;
    batch
}

fn transfer_pair(index: usize) -> (TransferTranscript, StateTransition, StateTransition) {
    let domain = DomainId::try_new("fixture", "universal").expect("domain id");
    let asset_definition =
        AssetDefinitionId::derive_from_components(domain.clone(), "xor".parse().unwrap());
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
        amount: Quantity::from(amount),
        from_balance_before: Quantity::from(from_pre),
        from_balance_after: Quantity::from(from_post),
        to_balance_before: Quantity::from(to_pre),
        to_balance_after: Quantity::from(to_post),
        from_smt_witness: iroha_data_model::fastpq::TransferSmtWitness::default(),
        to_smt_witness: iroha_data_model::fastpq::TransferSmtWitness::default(),
    };
    let mut payload = Vec::with_capacity(32);
    payload.extend_from_slice(b"fastpq-v1-fixture");
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
        iroha_data_model::fastpq::transfer_balance_key(&asset_definition, &from_account)
            .expect("canonical balance key"),
        from_pre.to_le_bytes().to_vec(),
        from_post.to_le_bytes().to_vec(),
        OperationKind::Transfer,
    );
    let receiver = StateTransition::new(
        iroha_data_model::fastpq::transfer_balance_key(&asset_definition, &to_account)
            .expect("canonical balance key"),
        to_pre.to_le_bytes().to_vec(),
        to_post.to_le_bytes().to_vec(),
        OperationKind::Transfer,
    );
    (transcript, sender, receiver)
}

fn deterministic_account(label: &str, domain: &DomainId) -> AccountId {
    let seed: [u8; Hash::LENGTH] = Hash::new(format!("{label}@{domain}")).into();
    let keypair = KeyPair::try_from_seed(seed.to_vec(), Algorithm::default())
        .expect("fixture FASTPQ account key");
    AccountId::new(keypair.public_key().clone())
}
