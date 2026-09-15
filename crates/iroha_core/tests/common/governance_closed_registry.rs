//! Rejected-input fixtures for the current closed production election registry.
//!
//! No fixture in this module represents an admitted election or a valid proof.

use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{ElectionState, State, StateTransaction, World},
    zk::{ZK_BACKEND_HALO2_IPA, hash_vk},
};
use iroha_data_model::{
    Registrable,
    account::Account,
    confidential::ConfidentialStatus,
    domain::Domain,
    permission::Permission,
    prelude::Grant,
    proof::{VerifyingKeyBox, VerifyingKeyId, VerifyingKeyRecord},
    zk::BackendTag,
};
use iroha_executor_data_model::permission::governance::{
    CanEnactGovernance, CanManageParliament, CanSubmitGovernanceBallot,
};
use iroha_model_base::domain::DomainId;
use iroha_primitives::json::Json;
use iroha_test_samples::ALICE_ID;

pub(super) fn state() -> State {
    let account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
    let domain =
        Domain::new(DomainId::try_new("wonderland", "universal").expect("domain")).build(&ALICE_ID);
    let mut state = State::new_for_testing(
        World::with([domain], [account], []),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    // Isolate proof/lock admission from citizenship and asset escrow accounting.
    state.gov.citizenship_bond_amount = 0_u64.into();
    state.gov.min_bond_amount = 0_u64.into();
    state.zk.halo2.enabled = true;
    state
}

pub(super) fn grant_permissions(transaction: &mut StateTransaction<'_, '_>, referendum: &str) {
    for permission in [
        Permission::new("CanManageVerifyingKeys".to_owned(), Json::new(())),
        CanManageParliament.into(),
        CanEnactGovernance.into(),
        CanSubmitGovernanceBallot {
            referendum_id: referendum.to_owned(),
        }
        .into(),
    ] {
        Grant::account_permission(permission, ALICE_ID.clone())
            .execute(&ALICE_ID, transaction)
            .expect("grant exact fixture permission");
    }
    transaction.world.take_external_events();
}

pub(super) fn unqualified_key(circuit_id: &str) -> (VerifyingKeyId, VerifyingKeyRecord) {
    let id = VerifyingKeyId::new(ZK_BACKEND_HALO2_IPA, "unqualified");
    // Deliberately opaque rejected input. The role gate must reject before key decode.
    let key = VerifyingKeyBox::new(ZK_BACKEND_HALO2_IPA.to_owned(), vec![1, 2, 3, 4]);
    let mut record = VerifyingKeyRecord::new(
        1,
        circuit_id,
        BackendTag::Halo2IpaPasta,
        "pallas",
        [0x11; 32],
        hash_vk(&key),
    );
    record.status = ConfidentialStatus::Active;
    record.vk_len = u32::try_from(key.bytes.len()).expect("key extent");
    record.key = Some(key);
    record.max_proof_bytes = 1024;
    record.gas_schedule_id = Some("halo2_default".to_owned());
    (id, record)
}

pub(super) fn retained_election(id: &VerifyingKeyId, record: &VerifyingKeyRecord) -> ElectionState {
    // Test-only retained-state adversary: this record was NOT created by a valid proof.
    ElectionState {
        options: 2,
        tally: vec![0, 0],
        eligible_root: [0x22; 32],
        vk_ballot: Some(id.clone()),
        vk_ballot_commitment: Some(record.commitment),
        vk_tally: Some(id.clone()),
        vk_tally_commitment: Some(record.commitment),
        domain_tag: "gov:ballot:v1".to_owned(),
        ..ElectionState::default()
    }
}
