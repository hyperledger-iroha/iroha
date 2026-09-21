//! Raw development-membership proof checks and explicit production rejection.
//! These tests do not qualify credential-linked ballots or sound election tallies.
#![cfg(all(
    feature = "zk-tests",
    feature = "halo2-dev-tests",
    any(feature = "zk-halo2", feature = "zk-halo2-ipa")
))]
#[path = "common/governance_closed_state.rs"]
mod closed_state;
#[path = "zk_testkit.rs"]
mod zk_testkit;
use iroha_core::{smartcontracts::Execute, state::WorldReadOnly, zk};
use iroha_data_model::{
    block::BlockHeader,
    isi::{
        error::{InstructionExecutionError, InvalidParameterError},
        verifying_keys::RegisterVerifyingKey,
        zk::VerifyProof,
    },
    proof::{ProofAttachment, ProofBox},
    zk::OpenVerifyEnvelope,
};
use iroha_test_samples::ALICE_ID;
use mv::storage::StorageReadOnly;
use nonzero_ext::nonzero;
fn assert_closed_registry(error: InstructionExecutionError) {
    assert_eq!(
        error,
        InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
            "Halo2 OpenVerify circuit_id is not in the production circuit registry".into()
        ))
    );
}
#[test]
fn development_membership_raw_proof_verifies() {
    let bundle = zk_testkit::dev_vote_merkle8_bundle();
    assert!(bundle.verify_raw(bundle.raw_proof(), bundle.commit, bundle.root));
}
#[test]
fn development_membership_raw_proof_rejects_commit_tampering() {
    let bundle = zk_testkit::dev_vote_merkle8_bundle();
    assert!(!bundle.verify_raw(
        bundle.raw_proof(),
        bundle.commit + halo2_proofs::halo2curves::pasta::Fp::one(),
        bundle.root
    ));
}
#[test]
fn development_membership_raw_proof_rejects_root_tampering() {
    let bundle = zk_testkit::dev_vote_merkle8_bundle();
    assert!(!bundle.verify_raw(
        bundle.raw_proof(),
        bundle.commit,
        bundle.root + halo2_proofs::halo2curves::pasta::Fp::one()
    ));
}
#[test]
fn development_membership_raw_proof_rejects_transcript_tampering() {
    let bundle = zk_testkit::dev_vote_merkle8_bundle();
    let mut proof = bundle.raw_proof().to_vec();
    proof[0] ^= 1;
    assert!(!bundle.verify_raw(&proof, bundle.commit, bundle.root));
    assert!(!bundle.verify_raw(&proof[..proof.len() / 2], bundle.commit, bundle.root));
}
#[test]
fn development_membership_proof_is_rejected_by_production_dispatch() {
    let bundle = zk_testkit::dev_vote_merkle8_bundle();
    let key = bundle.vk_record.key.as_ref().unwrap();
    let proof = ProofBox::new(bundle.backend.into(), bundle.proof_bytes.clone());
    assert!(!zk::verify_backend(bundle.backend, &proof, Some(key)));
}
#[test]
fn development_membership_key_and_schema_mutation_cannot_register() {
    let state = closed_state::state();
    let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 0, 0));
    let bundle = zk_testkit::dev_vote_merkle8_bundle();
    for mutated_schema in [false, true] {
        let mut transaction = block.transaction();
        closed_state::grant_permissions(&mut transaction, "dev-membership");
        let mut record = bundle.vk_record.clone();
        if mutated_schema {
            record.public_inputs_schema_hash[0] ^= 1;
        }
        assert_closed_registry(
            RegisterVerifyingKey {
                id: bundle.vk_id.clone(),
                record,
            }
            .execute(&ALICE_ID, &mut transaction)
            .expect_err("development key must not register"),
        );
        assert!(
            transaction
                .world
                .verifying_keys()
                .get(&bundle.vk_id)
                .is_none()
        );
        assert!(transaction.world.take_external_events().is_empty());
    }
}
#[test]
fn development_membership_verify_isi_rejects_missing_and_retained_keys() {
    let state = closed_state::state();
    let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 0, 0));
    let bundle = zk_testkit::dev_vote_merkle8_bundle();
    // Preserve both original public-input mutation controls without attributing registry
    // rejection to a fictitious schema verifier. Actual raw-proof binding is tested above.
    for tamper_column in [None, Some(0_usize), Some(1)] {
        for retained_key in [false, true] {
            let mut transaction = block.transaction();
            let mut envelope: OpenVerifyEnvelope =
                norito::decode_from_bytes(&bundle.proof_bytes).unwrap();
            if let Some(column) = tamper_column {
                envelope.public_inputs[column * 32] ^= 1;
            }
            let proof = ProofBox::new(bundle.backend.into(), norito::to_bytes(&envelope).unwrap());
            if retained_key {
                // Explicit adversarial retained state, never successful registration.
                transaction
                    .world
                    .verifying_keys_mut_for_testing()
                    .insert(bundle.vk_id.clone(), bundle.vk_record.clone());
            }
            let proof_id = iroha_data_model::proof::ProofId {
                backend: bundle.backend.into(),
                proof_hash: zk::hash_proof(&proof),
            };
            let mut attachment =
                ProofAttachment::new_ref(bundle.backend.into(), proof, bundle.vk_id.clone());
            attachment.vk_commitment = Some(bundle.vk_record.commitment);
            assert_closed_registry(
                VerifyProof::new(attachment)
                    .execute(&ALICE_ID, &mut transaction)
                    .expect_err("development envelope must stay outside VerifyProof"),
            );
            assert!(transaction.world.proofs().get(&proof_id).is_none());
            assert!(transaction.world.take_external_events().is_empty());
        }
    }
}
