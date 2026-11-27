//! Original signed runtime creators derive metadata; generic creation cannot forge it.

use super::*;
use crate::smartcontracts::Execute;
use iroha_crypto::Hash;
use iroha_data_model::{
    governance::{
        anonymous_ballot::{
            AnonymousRuntimeUpgradeRequestV1 as Request,
            CreateAnonymousRuntimeUpgradeAttemptV1 as Create,
        },
        types::{ParliamentBody, RuntimeUpgradeProposal},
    },
    parameter::RuntimeUpgradePolicyV1,
    runtime::lifecycle::{
        ChangeSetId, ChangeSetV1, RootScopeV1, RuntimeNoticeV1, RuntimeUpgradeActionV1,
    },
    transaction::{Executable, FeePaymentIntent, SignedTransaction, TransactionBuilder},
};
use iroha_executor_data_model::permission::governance::CanProposeRuntimeChangeSet;
use iroha_test_samples::ALICE_KEYPAIR;

fn setup(tx: &mut StateTransaction<'_, '_>) {
    crate::governance::parliament::seed_npos_sortition_cadence_for_tests(tx, 100);
    tx.world
        .set_parameter_for_testing(crate::sumeragi::lanes::routing::test_support::metadata(
            iroha_data_model::block::consensus::SumeragiRootScope::Global,
        ))
        .unwrap();
    tx.world.account_permissions.insert(
        ALICE_ID.clone(),
        std::collections::BTreeSet::from([Permission::from(CanProposeRuntimeChangeSet)]),
    );
    tx.world
        .set_parameter_for_testing(Parameter::Custom(
            RuntimeUpgradePolicyV1 {
                activation: RuntimeNoticeV1 {
                    blocks: 2,
                    ms: 30_000,
                    approval_cushion_ms: 0,
                },
                disable: RuntimeNoticeV1 {
                    blocks: 4,
                    ms: 60_000,
                    approval_cushion_ms: 0,
                },
            }
            .to_custom_parameter()
            .unwrap(),
        ))
        .unwrap();
}

fn request(tx: &StateTransaction<'_, '_>) -> Create {
    let selection = tx.world.runtime_selection.get().id().unwrap();
    Create {
        request: Request::Initial {
            action: RuntimeUpgradeActionV1::Schedule(ChangeSetV1 {
                network: network(),
                root: RootScopeV1::Universal,
                predecessor: selection,
                successor: selection,
                effective_height: HEIGHT + 20,
                publish: Vec::new(),
                transitions: Vec::new(),
                qualification: [0x74; 32],
            }),
            provenance: Vec::new(),
        },
    }
}

fn signed(instruction: Create) -> SignedTransaction {
    TransactionBuilder::new(
        network(),
        ALICE_ID.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([instruction])
    .sign(ALICE_KEYPAIR.private_key())
}

fn arm(tx: &mut StateTransaction<'_, '_>, signed: &SignedTransaction, direct: bool) {
    tx.current_tx_hash = Some(signed.hash());
    tx.tx_call_hash = Some(Hash::from(signed.hash_as_entrypoint()));
    tx.current_network_entrypoint_hash = Some(signed.hash_as_entrypoint());
    tx.current_entrypoint_index = Some(0);
    let Executable::Instructions(instructions) = signed.instructions() else {
        unreachable!()
    };
    tx.arm_direct_parliament_runtime(signed, signed.authority(), &instructions[0], 0, direct);
}

#[test]
fn runtime_creator_freezes_real_notice_time_operator_and_requires_original_once() {
    let state = state(world(true, None));
    let mut block = state.block(header(HEIGHT, NOW_MS));
    let mut tx = block.transaction();
    setup(&mut tx);
    let instruction = request(&tx);
    let original = signed(instruction.clone());
    assert!(instruction.clone().execute(&ALICE_ID, &mut tx).is_err());
    arm(&mut tx, &original, false);
    assert!(instruction.clone().execute(&ALICE_ID, &mut tx).is_err());
    arm(&mut tx, &original, true);
    instruction.clone().execute(&ALICE_ID, &mut tx).unwrap();
    assert!(instruction.execute(&ALICE_ID, &mut tx).is_err());
    let attempts: Vec<_> = tx.world.parliament_anonymous_attempts.iter().collect();
    assert_eq!(attempts.len(), 1);
    let ProposalKind::RuntimeUpgrade(proposal) = attempts[0].1.proposal() else {
        panic!("runtime proposal")
    };
    assert_eq!(proposal.proposal_operator, *ALICE_ID);
    assert_eq!(proposal.proposed_ms, NOW_MS);
    assert_eq!(
        proposal.notice,
        RuntimeNoticeV1 {
            blocks: 2,
            ms: 30_000,
            approval_cushion_ms: 0,
        }
    );
    assert!(
        tx.world.runtime_change_sets.iter().next().is_none(),
        "admission never enacts"
    );
}

#[test]
fn generic_creator_refuses_forged_runtime_notice_time_and_operator() {
    let state = state(world(true, None));
    let mut block = state.block(header(HEIGHT, NOW_MS));
    let mut tx = block.transaction();
    let raw = CreateAnonymousParliamentAttemptV1 {
        proposal: ProposalKind::RuntimeUpgrade(RuntimeUpgradeProposal {
            proposal_operator: ALICE_ID.clone(),
            action: RuntimeUpgradeActionV1::Cancel(ChangeSetId([9; 32])),
            notice: RuntimeNoticeV1 {
                blocks: 0,
                ms: 0,
                approval_cushion_ms: 0,
            },
            proposed_ms: 0,
        }),
        attempt_sequence: 0,
    };
    let error = raw.execute(&ALICE_ID, &mut tx).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("original signed runtime creator")
    );
    assert!(
        tx.world
            .parliament_anonymous_attempts
            .iter()
            .next()
            .is_none()
    );
}

#[test]
fn original_runtime_creator_refuses_changed_source_and_active_retry() {
    let state = state(world(true, None));
    let mut block = state.block(header(HEIGHT, NOW_MS));
    let mut tx = block.transaction();
    setup(&mut tx);
    let create = request(&tx);
    let original = signed(create.clone());
    arm(&mut tx, &original, true);
    let substitute = Create {
        request: Request::Initial {
            action: RuntimeUpgradeActionV1::Cancel(ChangeSetId([1; 32])),
            provenance: Vec::new(),
        },
    };
    assert!(
        substitute
            .execute(&ALICE_ID, &mut tx)
            .unwrap_err()
            .to_string()
            .contains("source changed")
    );
    arm(&mut tx, &original, true);
    tx.current_entrypoint_index = Some(1);
    assert!(create.clone().execute(&ALICE_ID, &mut tx).is_err());
    arm(&mut tx, &original, true);
    create.execute(&ALICE_ID, &mut tx).unwrap();
    let predecessor = *tx
        .world
        .parliament_anonymous_attempts
        .iter()
        .next()
        .unwrap()
        .0;
    let retry = Create {
        request: Request::Retry { predecessor },
    };
    arm(&mut tx, &signed(retry.clone()), true);
    assert!(
        retry
            .execute(&ALICE_ID, &mut tx)
            .unwrap_err()
            .to_string()
            .contains("terminal non-enacted")
    );
    assert_eq!(tx.world.parliament_anonymous_attempts.iter().count(), 1);
}

#[test]
fn runtime_creator_requires_committed_release_endorsements_before_storing_any_attempt() {
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::parameter::runtime_provenance_policy::RuntimeProvenancePolicyV1;
    let state = state(world(true, None));
    let mut block = state.block(header(HEIGHT, NOW_MS));
    let mut tx = block.transaction();
    setup(&mut tx);
    let signer = KeyPair::from_seed(vec![0x72; 32], Algorithm::Ed25519);
    tx.world
        .set_parameter_for_testing(Parameter::Custom(
            RuntimeProvenancePolicyV1 {
                signers: vec![signer.public_key().clone()],
                threshold: 1,
            }
            .to_custom_parameter()
            .unwrap(),
        ))
        .unwrap();
    let create = request(&tx);
    arm(&mut tx, &signed(create.clone()), true);
    assert!(create.execute(&ALICE_ID, &mut tx).is_err());
    assert!(
        tx.world
            .parliament_anonymous_attempts
            .iter()
            .next()
            .is_none()
    );
    assert!(tx.world.runtime_change_sets.iter().next().is_none());
}

#[test]
fn registered_runtime_creator_uses_distinct_tolerant_interface() {
    assert_eq!(
        iroha_data_model::isi::wire_id_of::<Create>(),
        Some(Create::WIRE_ID)
    );
}

#[test]
fn runtime_retry_reuses_exact_terminal_proposal_metadata_at_the_later_clock() {
    use crate::governance::parliament_anonymous::{AnonymousBallotDecisionV1, test_driver::Driver};
    let frozen = RuntimeUpgradeProposal {
        proposal_operator: ALICE_ID.clone(),
        action: RuntimeUpgradeActionV1::Cancel(ChangeSetId([0x67; 32])),
        notice: RuntimeNoticeV1 {
            blocks: 9,
            ms: 700_000,
            approval_cushion_ms: 30_000,
        },
        proposed_ms: 1_000_000,
    };
    let mut driver = Driver::for_proposal(
        ProposalKind::RuntimeUpgrade(frozen.clone()),
        RiskTierV1::Standard,
    )
    .drive_to_policy_reflection();
    assert!(matches!(
        driver
            .close_ballot(ParliamentBody::PolicyJury, (5, 20, 0))
            .unwrap(),
        AnonymousBallotDecisionV1::Rejected(_)
    ));
    let predecessor = driver.id();
    let content = driver.state.attempt().proposal_content_id;
    let state = state(world(true, None));
    let mut block = state.block(header(driver.height + 1, driver.now_ms + 1));
    let mut tx = block.transaction();
    setup(&mut tx);
    crate::state::parliament_anonymous_tables::store_attempt(
        &mut tx.world,
        predecessor,
        driver.state,
    );
    let retry = Create {
        request: Request::Retry { predecessor },
    };
    arm(&mut tx, &signed(retry.clone()), true);
    retry.execute(&ALICE_ID, &mut tx).unwrap();
    let id = iroha_data_model::governance::anonymous_ballot::anonymous_governance_attempt_id_v1(
        content, 1,
    );
    let stored = tx.world.parliament_anonymous_attempts.get(&id).unwrap();
    assert_eq!(stored.proposal(), &ProposalKind::RuntimeUpgrade(frozen));
    assert_eq!(stored.attempt().proposal_content_id, content);
    assert_eq!(stored.attempt().sequence, 1);
}
