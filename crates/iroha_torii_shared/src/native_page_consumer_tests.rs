//! Application consumers verify real native proof pages before promoting state or checking witnesses.
//! Fixture result values are synthetic; these controls do not execute World or qualify monetary policy.

use iroha_crypto::Hash;
use iroha_data_model::{
    governance::types::BallotAttemptId,
    sumeragi_finality::{
        SumeragiFinalityCheckpoint, SumeragiFinalityVerifier, VerifiedSumeragiBlock,
        test_fixtures::NativeFinalityFixture, verify_checkpoint_page,
    },
    validation_fee::{
        VALIDATION_FEE_POLICY_WITNESS_KEY_V1, VALIDATION_FEE_POLICY_WITNESS_SIBLINGS_V1,
        ValidationFeePolicySnapshotCommitmentV1, ValidationFeePolicyWitnessProofV1,
    },
};

use crate::{
    parliament_api::{
        PARLIAMENT_TIMED_OVN_CASTING_PROOF_VERSION_V1, ParliamentTimedOvnCastingProofResponseV1,
    },
    validation_fee_api::{
        VALIDATION_FEE_POLICY_PROOF_VERSION_V1, ValidationFeeCurrentPolicyProofV1,
    },
};

fn native_page_inputs() -> (
    NativeFinalityFixture,
    SumeragiFinalityCheckpoint,
    VerifiedSumeragiBlock,
) {
    let fixture = NativeFinalityFixture::new();
    // Select genesis and committee independently, then authenticate the complete
    // prefix through the production verifier. No decoded object grants trust.
    let mut verifier = SumeragiFinalityVerifier::new(
        fixture.genesis(),
        fixture.chain_id(),
        fixture.genesis_proof().committee.clone(),
    )
    .unwrap();
    verifier.verify(fixture.genesis_proof()).unwrap();
    let checkpoint = verifier.export_checkpoint(fixture.genesis_proof()).unwrap();
    let evaluated = verifier.verify(fixture.latest()).unwrap();
    assert_eq!(checkpoint.height(), 1);
    assert_eq!(evaluated.height(), 2);
    (fixture, checkpoint, evaluated)
}

#[test]
fn parliament_native_page_promotes_only_verified_matching_checkpoint() {
    let (fixture, checkpoint, evaluated) = native_page_inputs();
    let original_checkpoint = checkpoint.encode_canonical().unwrap();
    let expected_ballot = BallotAttemptId::new([0x42; 32]);
    let response = ParliamentTimedOvnCastingProofResponseV1 {
        version: PARLIAMENT_TIMED_OVN_CASTING_PROOF_VERSION_V1,
        casting_context_archive: None,
        casting_context_binding: None,
        context_membership_proof: None,
        casting_witness: None,
        finality_chain: vec![fixture.genesis_proof().clone(), fixture.latest().clone()],
        evaluated_context_id: evaluated.context_id(),
        evaluated_block_height: evaluated.height(),
        evaluated_block_hash: hex::encode(evaluated.header().hash().as_ref()),
        observed_ledger_tip_height: 3,
        more_available: true,
    };
    let (binding, promoted) = response
        .verify_consensus_page_against(fixture.network_id(), &checkpoint, expected_ballot)
        .unwrap();
    assert!(binding.is_none());
    assert_eq!(promoted.height(), 2);
    assert_eq!(
        promoted.encode_canonical().unwrap(),
        fixture.checkpoint().encode_canonical().unwrap()
    );
    // The returned owner is usable as the next independently retained checkpoint.
    let resumed = verify_checkpoint_page(
        fixture.network_id(),
        &promoted,
        std::slice::from_ref(fixture.latest()),
        4,
        4 * 1024 * 1024,
    )
    .unwrap();
    assert_eq!(resumed.tip().context_id(), evaluated.context_id());
    for mutation in 0..3 {
        let mut changed = response.clone();
        match mutation {
            0 => changed.evaluated_context_id = Hash::new(b"wrong application context"),
            1 => {
                changed.evaluated_block_hash =
                    hex::encode(Hash::new(b"wrong application block").as_ref())
            }
            _ => {
                changed.evaluated_block_height = 1;
            }
        }
        assert_eq!(
            changed
                .verify_consensus_page_against(fixture.network_id(), &checkpoint, expected_ballot)
                .unwrap_err(),
            "native finality page tip does not match the evaluated application block"
        );
        assert_eq!(checkpoint.encode_canonical().unwrap(), original_checkpoint);
    }
    let mut terminal = response.clone();
    terminal.observed_ledger_tip_height = 2;
    terminal.more_available = false;
    assert_eq!(
        terminal
            .verify_consensus_page_against(fixture.network_id(), &checkpoint, expected_ballot)
            .unwrap_err(),
        "terminal Parliament casting proof is incomplete"
    );
    terminal.finality_chain[1].committee[0].proof_of_possession[0] ^= 1;
    assert!(
        terminal
            .verify_consensus_page_against(fixture.network_id(), &checkpoint, expected_ballot)
            .unwrap_err()
            .starts_with("native finality page failed:")
    );
    assert_eq!(checkpoint.encode_canonical().unwrap(), original_checkpoint);
}

#[test]
fn validation_fee_native_page_rejects_unbound_witness_without_promotion() {
    let (fixture, checkpoint, evaluated) = native_page_inputs();
    let original_checkpoint = checkpoint.encode_canonical().unwrap();
    let commitment = ValidationFeePolicySnapshotCommitmentV1::from_registry(2, None);
    let policy_witness = ValidationFeePolicyWitnessProofV1 {
        key: VALIDATION_FEE_POLICY_WITNESS_KEY_V1.to_vec(),
        value: norito::encode_canonical(&commitment).unwrap(),
        siblings: vec![Hash::new(b"unbound sibling"); VALIDATION_FEE_POLICY_WITNESS_SIBLINGS_V1],
    };
    assert_eq!(policy_witness.commitment().unwrap(), commitment);
    assert!(!policy_witness.verify(evaluated.execution().ordinary_writes_root));
    let response = ValidationFeeCurrentPolicyProofV1 {
        version: VALIDATION_FEE_POLICY_PROOF_VERSION_V1,
        registry: None,
        policy_witness,
        finality_chain: vec![fixture.genesis_proof().clone(), fixture.latest().clone()],
        evaluated_context_id: evaluated.context_id(),
        evaluated_block_height: evaluated.height(),
        evaluated_block_hash: hex::encode(evaluated.header().hash().as_ref()),
        observed_ledger_tip_height: 2,
        more_available: false,
    };
    // This exact error occurs only after the whole native page and tip binding
    // succeed. A canonical policy value alone cannot authorize state promotion.
    assert_eq!(
        response
            .verify_against(fixture.network_id(), &checkpoint)
            .unwrap_err(),
        "validation-fee synthetic write proof is invalid"
    );
    assert_eq!(checkpoint.encode_canonical().unwrap(), original_checkpoint);
    let mut wrong_context = response.clone();
    wrong_context.evaluated_context_id = Hash::new(b"wrong policy context");
    assert_eq!(
        wrong_context
            .verify_against(fixture.network_id(), &checkpoint)
            .unwrap_err(),
        "native finality page tip does not match the evaluated application block"
    );
    let mut bad_native = response;
    bad_native.finality_chain[1].committee[0].proof_of_possession[0] ^= 1;
    assert!(
        bad_native
            .verify_against(fixture.network_id(), &checkpoint)
            .unwrap_err()
            .starts_with("native finality page failed:")
    );
    assert_eq!(checkpoint.encode_canonical().unwrap(), original_checkpoint);
}
