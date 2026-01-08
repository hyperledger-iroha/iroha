//! FASTPQ proof smoke tests covering realistic governance and remittance scenarios.

use fastpq_prover::{
    OperationKind, Prover, PublicInputs, StateTransition, TransitionBatch, verify,
};

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

fn governance_batch() -> TransitionBatch {
    let mut batch = TransitionBatch::new("fastpq-lane-balanced", PublicInputs::default());
    annotate_inputs(&mut batch, 7);

    batch.push(StateTransition::new(
        b"role/council@governance/permission/vote.sora".to_vec(),
        encode_u64(0),
        encode_u64(1),
        OperationKind::RoleGrant {
            role_id: vec![0xA1; 32],
            permission_id: vec![0xB2; 32],
            epoch: 42,
        },
    ));

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
    let mut batch = TransitionBatch::new("fastpq-lane-balanced", PublicInputs::default());
    annotate_inputs(&mut batch, 23);

    batch.push(StateTransition::new(
        b"asset/xor#remit/alice@remit".to_vec(),
        encode_u64(ALICE_START),
        encode_u64(ALICE_START - REMIT_AMOUNT),
        OperationKind::Transfer,
    ));
    batch.push(StateTransition::new(
        b"asset/xor#remit/bob@remit".to_vec(),
        encode_u64(BOB_START),
        encode_u64(BOB_START + REMIT_AMOUNT),
        OperationKind::Transfer,
    ));

    batch.sort();
    batch
}

fn combined_batch() -> TransitionBatch {
    let mut governance = governance_batch();
    let mut remit = remittance_batch();
    governance.transitions.append(&mut remit.transitions);
    governance.sort();
    governance
}

fn prove_and_verify(mut batch: TransitionBatch) {
    batch.sort();
    let prover = Prover::canonical("fastpq-lane-balanced").expect("canonical prover");
    let proof = prover.prove(&batch).expect("FASTPQ proof");
    verify(&batch, &proof).expect("FASTPQ verification");
}

#[test]
fn governance_flow_proof_verifies() {
    prove_and_verify(governance_batch());
}

#[test]
fn remittance_flow_proof_verifies() {
    prove_and_verify(remittance_batch());
}

#[test]
fn governance_and_remittance_combined_proof_verifies() {
    prove_and_verify(combined_batch());
}
