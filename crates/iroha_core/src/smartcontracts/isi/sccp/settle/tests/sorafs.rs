//! SCCP keeps unrelated `SoraFS` custody decoder refusals local to the attempt.

use super::*;
use iroha_data_model::{
    isi::sorafs::SetSorafsReservePolicy,
    permission::{Permission, Permissions},
    sorafs::reserve::{
        RESERVE_AUTHORITY_POLICY_VERSION_V1, ReserveAuthorityPolicyV1, ReservePolicyV1,
        history::reserve_state_key,
    },
};
use iroha_primitives::json::Json;
use mv::storage::StorageReadOnly as _;
use sorafs_manifest::deal::XorQuantity;

#[test]
fn sccp_unrelated_sorafs_reserve_decode_refusal_aborts_release_without_pending_credit_refusal() {
    let state = state();
    let next_height = u64::try_from(state.view().block_hashes().len()).unwrap() + 1;
    let mut block = state.block(header(next_height));
    let mut tx = block.transaction_for_fastpq_testing(Hash::new("sccp-sorafs-decode"));
    install(&mut tx);
    fund(&mut tx, ETH, 1, 10 * ONE_XOR);
    let recipient = authority(70);
    let custody = authority(71);
    for account in [&recipient, &custody] {
        recipients::ensure_registered(&mut tx, account).unwrap();
    }
    set_enabled(&mut tx, false);
    let id = prove(&mut tx, 0, ONE_XOR, address_bytes(&recipient), 0);
    set_enabled(&mut tx, true);
    let mut permissions = Permissions::new();
    permissions.insert(Permission::new(
        "CanSetSorafsReservePolicy".to_owned(),
        Json::new(()),
    ));
    tx.world.account_permissions.insert(holder(), permissions);
    SetSorafsReservePolicy::new(ReserveAuthorityPolicyV1 {
        version: RESERVE_AUTHORITY_POLICY_VERSION_V1,
        revision: 1,
        predecessor_policy_digest: None,
        economics: ReservePolicyV1::default(),
        asset_definition: sccp_taira_xor_asset_definition_id(),
        custody_account: custody,
        treasury_account: holder(),
        operations_authority: holder(),
        decision_authority: holder(),
        grace_period_days: 7,
        default_after_days: 30,
        max_provider_debt: XorQuantity::try_from_micro(1_000_000_000).unwrap(),
        max_pending_movements_per_provider: 4,
        max_open_appeals_per_provider: 2,
    })
    .execute(&holder(), &mut tx)
    .expect("install a real reserve policy and its canonical activation journal");
    escrow::precheck_release(&tx, ETH, &recipient, ONE_XOR)
        .expect("unrelated reserve custody must not prevent this release");
    assert!(!tx.is_execution_deferred());

    let key = reserve_state_key().clone();
    let original = tx.world.smart_contract_state.get(&key).unwrap().clone();
    tx.world.smart_contract_state.insert(key.clone(), vec![0]);
    escrow::precheck_release(&tx, ETH, &recipient, ONE_XOR).unwrap_err();
    assert!(
        !tx.is_execution_deferred(),
        "malformed state is deterministic"
    );
    tx.world.smart_contract_state.insert(key, original);
    let before: Vec<_> = tx
        .world
        .smart_contract_state
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    let events = sccp_events(&tx);
    let status = inbound_status(&tx, &id);
    let record = store::inbound_messages::get(&*tx.world, &id)
        .unwrap()
        .clone();
    let classified_recipient = (recipient.clone(), true);

    // Copies and arithmetic remain admitted. The first state decode in the
    // full movement precheck is the unrelated reserve's canonical singleton.
    norito::with_decode_limits_scope(
        norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, usize::MAX, 0),
        || release_inbound(&mut tx, id, record, ONE_XOR, classified_recipient),
    )
    .expect_err("the real release caller must abort its local attempt");
    assert!(tx.is_execution_deferred());
    assert!(matches!(
        tx.borrow_execution_deferral()
            .unwrap()
            .decode_refusal()
            .unwrap()
            .resource_error(),
        norito::core::DecodeResourceError::NestingDepthExceeded { limit: 0, .. }
    ));
    let after: Vec<_> = tx
        .world
        .smart_contract_state
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    assert_eq!(before, after);
    assert_eq!(
        inbound_status(&tx, &id),
        status,
        "no permanent CreditRefused"
    );
    assert_eq!(sccp_events(&tx), events);
    assert_eq!(xor_balance(&*tx.world, &recipient), 0);
    assert_eq!(liability(&tx, ETH, 1), 10 * ONE_XOR);
    assert_eq!(store::pending_count(&*tx.world, &(ETH, 1)), (1, 0));
    assert_escrow_invariant(&*tx.world, "unrelated reserve decoder refused locally");
}
