//! Inactive retained election keys reject before proof-envelope decoding.
//! Adversarial retained state does not represent a registered production election.
#![cfg(feature = "zk-tests")]

#[path = "common/governance_closed_registry.rs"]
mod closed_registry;
#[path = "common/governance_closed_state.rs"]
mod closed_state;

use iroha_core::{
    smartcontracts::Execute,
    state::{
        GovernanceReferendumMode, GovernanceReferendumRecord, GovernanceReferendumStatus,
        WorldReadOnly,
    },
};
use iroha_data_model::{
    block::BlockHeader,
    confidential::ConfidentialStatus,
    events::data::{DataEvent, governance::GovernanceEvent},
    isi::{error::InstructionExecutionError, governance::CastZkBallot},
};
use iroha_test_samples::ALICE_ID;
use mv::storage::StorageReadOnly;
use nonzero_ext::nonzero;

#[test]
fn zk_ballot_rejects_when_vk_not_active() {
    let state = closed_state::state();
    let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 0, 0));
    for (status, activation, withdrawal, expected) in [
        (
            ConfidentialStatus::Proposed,
            None,
            None,
            "verifying key is not Active",
        ),
        (
            ConfidentialStatus::Withdrawn,
            None,
            None,
            "verifying key is not Active",
        ),
        (
            ConfidentialStatus::Active,
            Some(2),
            None,
            "verifying key is not Active",
        ),
        (
            ConfidentialStatus::Active,
            None,
            Some(1),
            "verifying key is not Active",
        ),
        (
            ConfidentialStatus::Active,
            None,
            Some(0),
            "verifying key is not Active",
        ),
        // A permitted height crosses the status check but cannot admit an unsupported role.
        (
            ConfidentialStatus::Active,
            Some(1),
            Some(2),
            "ballot verifying key circuit mismatch",
        ),
    ] {
        let mut transaction = block.transaction();
        closed_state::grant_permissions(&mut transaction, "ref-vk");
        let (id, mut record) = closed_registry::unqualified_key("vote-ballot");
        record.status = status;
        record.activation_height = activation;
        record.withdraw_height = withdrawal;
        let election = closed_registry::retained_election(&id, &record);
        let before = norito::to_bytes(&election).unwrap();
        transaction
            .world
            .verifying_keys_mut_for_testing()
            .insert(id.clone(), record.clone());
        transaction
            .world
            .elections_mut()
            .insert("ref-vk".into(), election);
        let referendum = GovernanceReferendumRecord {
            h_start: 0,
            h_end: 100,
            status: GovernanceReferendumStatus::Open,
            mode: GovernanceReferendumMode::Zk,
            plain_context:
                iroha_data_model::governance::conviction::PlainVotingContextV1::NotApplicable,
            plain_result:
                iroha_data_model::governance::conviction::PlainVotingResultV1::NotApplicable,
        };
        transaction
            .world
            .governance_referenda_mut()
            .insert("ref-vk".into(), referendum.clone());
        transaction.world.take_external_events();
        let error = CastZkBallot {
            election_id: "ref-vk".into(),
            // Valid nonempty base64 of an invalid envelope; no fabricated proof positive.
            proof_b64: "AQID".into(),
            public_inputs_json: "{}".into(),
        }
        .execute(&ALICE_ID, &mut transaction)
        .expect_err("retained unqualified key must reject");
        assert_eq!(
            error,
            InstructionExecutionError::InvariantViolation(expected.into())
        );
        assert_eq!(
            norito::to_bytes(transaction.world.elections().get("ref-vk").unwrap()).unwrap(),
            before
        );
        assert_eq!(transaction.world.verifying_keys().get(&id), Some(&record));
        assert_eq!(
            transaction.world.governance_referenda().get("ref-vk"),
            Some(&referendum)
        );
        assert!(transaction.world.governance_locks().get("ref-vk").is_none());
        let events = transaction.world.take_external_events();
        assert_eq!(events.len(), 1);
        assert!(
            matches!(events[0].as_data_event(), Some(DataEvent::Governance(GovernanceEvent::BallotRejected(rejection)))
            if rejection.referendum_id == "ref-vk" && rejection.reason == expected)
        );
    }
}
