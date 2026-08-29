//! FASTPQ proof smoke tests covering realistic governance and remittance scenarios.
use fastpq_prover::{
    Error, OperationKind, Prover, PublicInputs, StateTransition, TransitionBatch,
    gadgets::transfer::attach_transfer_smt_witnesses, verify,
};
use iroha_crypto::{Algorithm, Hash, KeyPair};
use iroha_data_model::{
    account::AccountId,
    asset::id::AssetDefinitionId,
    domain::DomainId,
    fastpq::{TRANSFER_TRANSCRIPTS_METADATA_KEY, TransferDeltaTranscript, TransferTranscript},
};
use iroha_primitives::numeric::Quantity;
use norito::to_bytes;
fn annotate_inputs(batch: &mut TransitionBatch, slot: u64) {
    batch.public_inputs.dsid = [0x3D; 16];
    batch.public_inputs.slot = slot;
    batch.public_inputs.old_root = [0x11; 32];
    batch.public_inputs.new_root = [0x22; 32];
    batch.public_inputs.perm_root = [0x33; 32];
    batch.public_inputs.tx_set_hash = [0x44; 32];
}
fn encode_u64(value: u64) -> Vec<u8> {
    value.to_le_bytes().to_vec()
}
fn deterministic_account(label: &str, domain: &DomainId) -> AccountId {
    let seed: [u8; Hash::LENGTH] = Hash::new(format!("{label}@{domain}")).into();
    let keypair = KeyPair::try_from_seed(seed.to_vec(), Algorithm::default())
        .expect("fixture FASTPQ account key");
    let _ = domain;
    AccountId::new(keypair.public_key().clone())
}
fn governance_batch() -> TransitionBatch {
    let mut batch =
        TransitionBatch::new("fastpq-state-transition-stark-v1", PublicInputs::default());
    annotate_inputs(&mut batch, 7);
    batch.push(StateTransition::new(
        b"account/governance.ballot@governance/metadata/ballot_2025_02".to_vec(),
        vec![],
        vec![1],
        OperationKind::MetaSet,
    ));
    batch.sort();
    batch
}
fn remittance_batch() -> TransitionBatch {
    const REMIT_AMOUNT: u64 = 75_000;
    const ALICE_START: u64 = 500_000;
    const BOB_START: u64 = 120_000;
    let mut batch =
        TransitionBatch::new("fastpq-state-transition-stark-v1", PublicInputs::default());
    annotate_inputs(&mut batch, 23);
    let domain = DomainId::try_new("remit", "universal").expect("domain id");
    let asset_definition = AssetDefinitionId::derive_from_components(
        DomainId::try_new("remit", "universal").unwrap(),
        "xor".parse().unwrap(),
    );
    let from_account = deterministic_account("alice", &domain);
    let to_account = deterministic_account("bob", &domain);
    batch.push(StateTransition::new(
        format!("asset/{asset_definition}/{from_account}").into_bytes(),
        encode_u64(ALICE_START),
        encode_u64(ALICE_START - REMIT_AMOUNT),
        OperationKind::Transfer,
    ));
    batch.push(StateTransition::new(
        format!("asset/{asset_definition}/{to_account}").into_bytes(),
        encode_u64(BOB_START),
        encode_u64(BOB_START + REMIT_AMOUNT),
        OperationKind::Transfer,
    ));
    let mut transcripts = vec![remittance_transcript(
        &asset_definition,
        &from_account,
        &to_account,
        REMIT_AMOUNT,
        ALICE_START,
        BOB_START,
    )];
    let (old_root, new_root) =
        attach_transfer_smt_witnesses(&mut transcripts).expect("attach witnesses");
    batch.public_inputs.old_root = old_root;
    batch.public_inputs.new_root = new_root;
    batch.metadata.insert(
        TRANSFER_TRANSCRIPTS_METADATA_KEY.into(),
        to_bytes(&transcripts).expect("encode transcripts"),
    );
    batch.sort();
    batch
}
fn combined_batch() -> TransitionBatch {
    let mut governance = governance_batch();
    let mut remit = remittance_batch();
    governance.public_inputs.old_root = remit.public_inputs.old_root;
    governance.public_inputs.new_root = remit.public_inputs.new_root;
    governance.transitions.append(&mut remit.transitions);
    governance.metadata.extend(remit.metadata);
    governance.sort();
    governance
}
fn prove_and_verify(mut batch: TransitionBatch) {
    batch.sort();
    let prover = Prover::canonical("fastpq-state-transition-stark-v1").expect("canonical prover");
    let proof = prover.prove(&batch).expect("FASTPQ proof");
    verify(&batch, &proof).expect("FASTPQ verification");
}
fn assert_strict_state_profile_rejects(mut batch: TransitionBatch) {
    batch.sort();
    let prover = Prover::canonical("fastpq-state-transition-stark-v1").expect("canonical prover");
    assert!(matches!(
        prover.prove(&batch),
        Err(Error::InvalidProofSemantics {
            profile: "transfer_state_transition",
            ..
        })
    ));
}
#[test]
fn opaque_governance_flow_fails_closed_under_transfer_semantics() {
    assert_strict_state_profile_rejects(governance_batch());
}
#[test]
fn remittance_flow_proof_verifies() {
    prove_and_verify(remittance_batch());
}
#[test]
fn governance_and_remittance_combined_flow_fails_closed() {
    assert_strict_state_profile_rejects(combined_batch());
}
fn remittance_transcript(
    asset_definition: &AssetDefinitionId,
    from_account: &AccountId,
    to_account: &AccountId,
    remit_amount: u64,
    alice_start: u64,
    bob_start: u64,
) -> TransferTranscript {
    let delta = TransferDeltaTranscript {
        from_account: from_account.clone(),
        to_account: to_account.clone(),
        asset_definition: asset_definition.clone(),
        amount: Quantity::from(remit_amount),
        from_balance_before: Quantity::from(alice_start),
        from_balance_after: Quantity::from(alice_start - remit_amount),
        to_balance_before: Quantity::from(bob_start),
        to_balance_after: Quantity::from(bob_start + remit_amount),
        from_smt_witness: iroha_data_model::fastpq::TransferSmtWitness::default(),
        to_smt_witness: iroha_data_model::fastpq::TransferSmtWitness::default(),
    };
    let batch_hash = Hash::new(b"remit-batch");
    let digest = fastpq_prover::gadgets::transfer::compute_poseidon_digest(&delta, &batch_hash);
    TransferTranscript {
        batch_hash,
        deltas: vec![delta],
        authority_digest: Hash::new(b"authority"),
        poseidon_preimage_digest: Some(digest),
    }
}
