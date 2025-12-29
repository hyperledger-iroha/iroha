#![doc = "Governance ZK ballot basic test (requires explicit election creation)."]
#![cfg(all(feature = "zk-tests", feature = "halo2-dev-tests"))]
#![cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
//! Governance ZK ballot basic test (requires explicit election creation).

mod zk_testkit;

use base64::Engine as _;
use core::num::NonZeroU64;
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{State, World},
};
use iroha_data_model::{
    block::BlockHeader,
    events::data::{DataEvent, governance::GovernanceEvent},
    isi::{governance::CastZkBallot, zk::CreateElection},
    permission::Permission,
    prelude::Grant,
};
use iroha_executor_data_model::permission::governance::{
    CanManageParliament, CanSubmitGovernanceBallot,
};
use iroha_primitives::json::Json;
use iroha_test_samples::ALICE_ID;

#[test]
fn zk_ballot_records_and_dedupes() {
    // Build minimal state/transaction
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new_for_testing(World::default(), kura, query_handle);
    // Leader keypair not needed in this simplified setup
    let header = BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, None, 0, 0);
    let mut sblock = state.block(header);
    let mut stx = sblock.transaction();

    // Register a real Halo2 verifying key and wire config defaults
    let bundle = zk_testkit::tiny_add_bundle();
    let vk_id = bundle.vk_id.clone();

    // Submit two identical ballots (same proof → same derived nullifier) → second must fail
    let proof_b64 = bundle.proof_b64.clone();
    let public_inputs = norito::json::object([
        (
            "nullifier_hex",
            norito::json::to_value(&"11".repeat(32)).expect("serialize nullifier"),
        ),
        (
            "owner",
            norito::json::to_value(&ALICE_ID.to_string()).expect("serialize owner"),
        ),
        (
            "salt",
            norito::json::to_value(&"22".repeat(32)).expect("serialize salt"),
        ),
        (
            "root_hint",
            norito::json::to_value(&"00".repeat(32)).expect("serialize root"),
        ),
    ])
    .expect("serialize public inputs");
    let public_inputs = norito::json::to_json(&public_inputs).expect("serialize public inputs");
    let election_id = "referendum-1".to_string();
    let instr = CastZkBallot {
        election_id: election_id.clone(),
        proof_b64: proof_b64.clone(),
        public_inputs_json: public_inputs.clone(),
    };
    // Guard: casting against an unknown election must fail
    let err = instr.clone().execute(&ALICE_ID, &mut stx).unwrap_err();
    let s = format!("{err}");
    assert!(s.contains("unknown election id"));
    stx.world.take_external_events();

    // Grant VK management permission and register a verifying key via instruction
    let perm = Permission::new("CanManageVerifyingKeys".to_string(), Json::new(()));
    Grant::account_permission(perm, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant VK management");

    iroha_data_model::isi::verifying_keys::RegisterVerifyingKey {
        id: vk_id.clone(),
        record: bundle.vk_record.clone(),
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("register verifying key");
    let parliament_perm: Permission = CanManageParliament.into();
    Grant::account_permission(parliament_perm, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant CanManageParliament");
    let ballot_perm: Permission = CanSubmitGovernanceBallot {
        referendum_id: election_id.clone(),
    }
    .into();
    Grant::account_permission(ballot_perm, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant CanSubmitGovernanceBallot");

    let create = CreateElection {
        election_id: election_id.clone(),
        options: 2,
        eligible_root: [0u8; 32],
        start_ts: 0,
        end_ts: 0,
        vk_ballot: vk_id.clone(),
        vk_tally: vk_id,
        domain_tag: "gov:ballot:v1".to_string(),
    };
    create.execute(&ALICE_ID, &mut stx).expect("create ok");
    stx.world.governance_referenda_mut().insert(
        election_id.clone(),
        iroha_core::state::GovernanceReferendumRecord {
            h_start: 0,
            h_end: 100,
            status: iroha_core::state::GovernanceReferendumStatus::Proposed,
            mode: iroha_core::state::GovernanceReferendumMode::Zk,
        },
    );

    instr
        .clone()
        .execute(&ALICE_ID, &mut stx)
        .expect("first ok");
    // Check that a BallotAccepted event was emitted
    let events = stx.world.take_external_events();
    assert!(
        events.iter().any(|event| matches!(
            event.as_data_event(),
            Some(DataEvent::Governance(GovernanceEvent::BallotAccepted(_)))
        )),
        "expected a BallotAccepted event"
    );
    let err = CastZkBallot {
        election_id,
        proof_b64,
        public_inputs_json: public_inputs,
    }
    .execute(&ALICE_ID, &mut stx)
    .unwrap_err();
    let s = format!("{err}");
    assert!(s.contains("duplicate ballot nullifier"));
}

#[test]
fn zk_ballot_rejects_invalid_proof() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new_for_testing(World::default(), kura, query_handle);
    let header = BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, None, 0, 0);
    let mut sblock = state.block(header);
    let mut stx = sblock.transaction();

    let bundle = zk_testkit::tiny_add_bundle();
    let vk_id = bundle.vk_id.clone();

    let perm = Permission::new("CanManageVerifyingKeys".to_string(), Json::new(()));
    Grant::account_permission(perm, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant VK management");

    iroha_data_model::isi::verifying_keys::RegisterVerifyingKey {
        id: vk_id.clone(),
        record: bundle.vk_record.clone(),
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("register verifying key");
    let parliament_perm: Permission = CanManageParliament.into();
    Grant::account_permission(parliament_perm, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant CanManageParliament");

    let election_id = "ref-invalid-proof".to_string();
    let ballot_perm: Permission = CanSubmitGovernanceBallot {
        referendum_id: election_id.clone(),
    }
    .into();
    Grant::account_permission(ballot_perm, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant CanSubmitGovernanceBallot");
    let create = CreateElection {
        election_id: election_id.clone(),
        options: 2,
        eligible_root: [0u8; 32],
        start_ts: 0,
        end_ts: 0,
        vk_ballot: vk_id.clone(),
        vk_tally: vk_id.clone(),
        domain_tag: "gov:ballot:v1".to_string(),
    };
    create
        .execute(&ALICE_ID, &mut stx)
        .expect("create election");
    stx.world.governance_referenda_mut().insert(
        election_id.clone(),
        iroha_core::state::GovernanceReferendumRecord {
            h_start: 0,
            h_end: 100,
            status: iroha_core::state::GovernanceReferendumStatus::Proposed,
            mode: iroha_core::state::GovernanceReferendumMode::Zk,
        },
    );
    stx.world.governance_referenda_mut().insert(
        election_id.clone(),
        iroha_core::state::GovernanceReferendumRecord {
            h_start: 0,
            h_end: 100,
            status: iroha_core::state::GovernanceReferendumStatus::Proposed,
            mode: iroha_core::state::GovernanceReferendumMode::Zk,
        },
    );
    stx.world.governance_referenda_mut().insert(
        election_id.clone(),
        iroha_core::state::GovernanceReferendumRecord {
            h_start: 0,
            h_end: 100,
            status: iroha_core::state::GovernanceReferendumStatus::Proposed,
            mode: iroha_core::state::GovernanceReferendumMode::Zk,
        },
    );

    let mut corrupted_proof = bundle.proof_bytes.clone();
    if let Some(last) = corrupted_proof.last_mut() {
        *last ^= 0x01;
    }
    let proof_b64 = base64::engine::general_purpose::STANDARD.encode(corrupted_proof);
    let public_inputs = norito::json::object([
        (
            "nullifier_hex",
            norito::json::to_value(&"33".repeat(32)).expect("serialize nullifier"),
        ),
        (
            "owner",
            norito::json::to_value(&ALICE_ID.to_string()).expect("serialize owner"),
        ),
        (
            "salt",
            norito::json::to_value(&"44".repeat(32)).expect("serialize salt"),
        ),
        (
            "root_hint",
            norito::json::to_value(&"00".repeat(32)).expect("serialize root"),
        ),
    ])
    .expect("serialize public inputs");
    let public_inputs = norito::json::to_json(&public_inputs).expect("serialize public inputs");

    let err = CastZkBallot {
        election_id: election_id.clone(),
        proof_b64,
        public_inputs_json: public_inputs,
    }
    .execute(&ALICE_ID, &mut stx)
    .unwrap_err();
    let s = format!("{err}");
    assert!(s.contains("invalid proof"));

    let events = stx.world.take_external_events();
    assert!(events.iter().any(|event| matches!(
        event.as_data_event(),
        Some(DataEvent::Governance(GovernanceEvent::BallotRejected(rej)))
            if rej.reason.contains("invalid proof")
    )));
}

#[test]
fn zk_ballot_rejects_malformed_public_inputs() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new_for_testing(World::default(), kura, query_handle);
    let header = BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, None, 0, 0);
    let mut sblock = state.block(header);
    let mut stx = sblock.transaction();

    let bundle = zk_testkit::tiny_add_bundle();
    let vk_id = bundle.vk_id.clone();

    let perm = Permission::new("CanManageVerifyingKeys".to_string(), Json::new(()));
    Grant::account_permission(perm, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant VK management");

    iroha_data_model::isi::verifying_keys::RegisterVerifyingKey {
        id: vk_id.clone(),
        record: bundle.vk_record.clone(),
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("register verifying key");

    let election_id = "ref-public-inputs".to_string();
    let create = CreateElection {
        election_id: election_id.clone(),
        options: 2,
        eligible_root: [0u8; 32],
        start_ts: 0,
        end_ts: 0,
        vk_ballot: vk_id.clone(),
        vk_tally: vk_id,
        domain_tag: "gov:ballot:v1".to_string(),
    };
    create
        .execute(&ALICE_ID, &mut stx)
        .expect("create election");
    stx.world.governance_referenda_mut().insert(
        election_id.clone(),
        iroha_core::state::GovernanceReferendumRecord {
            h_start: 0,
            h_end: 100,
            status: iroha_core::state::GovernanceReferendumStatus::Proposed,
            mode: iroha_core::state::GovernanceReferendumMode::Zk,
        },
    );

    let malformed_public_inputs = "{\"owner\": \"alice#wonderland\"".to_string();
    let err = CastZkBallot {
        election_id,
        proof_b64: bundle.proof_b64.clone(),
        public_inputs_json: malformed_public_inputs,
    }
    .execute(&ALICE_ID, &mut stx)
    .unwrap_err();
    let s = format!("{err}");
    assert!(s.contains("public inputs must be valid JSON"));

    let events = stx.world.take_external_events();
    assert!(events.iter().any(|event| matches!(
        event.as_data_event(),
        Some(DataEvent::Governance(GovernanceEvent::BallotRejected(rej)))
            if rej.reason.contains("public inputs must be valid JSON")
    )));
}

#[test]
fn zk_ballot_rejects_when_vk_commitment_mismatched() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new_for_testing(World::default(), kura, query_handle);
    let header = BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, None, 0, 0);
    let mut sblock = state.block(header);
    let mut stx = sblock.transaction();

    let bundle = zk_testkit::tiny_add_bundle();
    let vk_id = bundle.vk_id.clone();

    let perm = Permission::new("CanManageVerifyingKeys".to_string(), Json::new(()));
    Grant::account_permission(perm, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant VK management");

    iroha_data_model::isi::verifying_keys::RegisterVerifyingKey {
        id: vk_id.clone(),
        record: bundle.vk_record.clone(),
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("register verifying key");

    // Corrupt the stored commitment while keeping the verifying key bytes intact.
    let mut corrupted = stx
        .world
        .verifying_keys_mut_for_testing()
        .get(&vk_id)
        .cloned()
        .expect("vk present");
    corrupted.commitment[0] ^= 0x01;
    stx.world
        .verifying_keys_mut_for_testing()
        .insert(vk_id.clone(), corrupted);

    let election_id = "ref-vk-commitment".to_string();
    let parliament_perm: Permission = CanManageParliament.into();
    Grant::account_permission(parliament_perm, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant CanManageParliament");
    let ballot_perm: Permission = CanSubmitGovernanceBallot {
        referendum_id: election_id.clone(),
    }
    .into();
    Grant::account_permission(ballot_perm, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant CanSubmitGovernanceBallot");
    let create = CreateElection {
        election_id: election_id.clone(),
        options: 2,
        eligible_root: [0u8; 32],
        start_ts: 0,
        end_ts: 0,
        vk_ballot: vk_id.clone(),
        vk_tally: vk_id.clone(),
        domain_tag: "gov:ballot:v1".to_string(),
    };
    create
        .execute(&ALICE_ID, &mut stx)
        .expect("create election");

    let public_inputs = norito::json::object([
        (
            "nullifier_hex",
            norito::json::to_value(&"55".repeat(32)).expect("serialize nullifier"),
        ),
        (
            "owner",
            norito::json::to_value(&ALICE_ID.to_string()).expect("serialize owner"),
        ),
        (
            "salt",
            norito::json::to_value(&"66".repeat(32)).expect("serialize salt"),
        ),
        (
            "root_hint",
            norito::json::to_value(&"00".repeat(32)).expect("serialize root"),
        ),
    ])
    .expect("serialize public inputs");
    let public_inputs = norito::json::to_json(&public_inputs).expect("serialize public inputs");

    let err = CastZkBallot {
        election_id,
        proof_b64: bundle.proof_b64.clone(),
        public_inputs_json: public_inputs,
    }
    .execute(&ALICE_ID, &mut stx)
    .unwrap_err();
    let s = format!("{err}");
    assert!(s.contains("verifying key commitment mismatch"));
}
