//! SCCP composition with exact native UID holdings, activity and enrollment policies.

use super::*;
use crate::state::asset_uid;
use iroha_data_model::{
    asset::{
        uid::{AssetUidDailyWindowV1, AssetUidPolicyV1},
        uid_enrollment::AssetUidSystemAccountV1,
    },
    isi::{
        asset_uid::SetAssetUidPolicyV1,
        asset_uid_enrollment::{ActivateAssetUidEnrollmentV1, SetAssetUidSystemAccountV1},
    },
};
use iroha_primitives::numeric::Quantity;
use std::collections::BTreeSet;

fn policy(
    tx: &mut StateTransaction<'_, '_>,
    uid: u8,
    revision: u64,
    accounts: &[AccountId],
    holding: Option<u128>,
    spend: Option<u128>,
    outgoing: Option<u128>,
) {
    SetAssetUidPolicyV1 {
        policy: AssetUidPolicyV1 {
            asset_definition_id: sccp_taira_xor_asset_definition_id(),
            uid: [uid; 32],
            revision,
            accounts: accounts.iter().cloned().collect::<BTreeSet<_>>(),
            holding_limit: holding.map(|amount| escrow::xor_quantity(amount).unwrap()),
            daily_window: AssetUidDailyWindowV1::UtcDay,
            daily_spend_limit: spend.map(|amount| escrow::xor_quantity(amount).unwrap()),
            daily_outgoing_limit: outgoing.map(|amount| escrow::xor_quantity(amount).unwrap()),
        },
    }
    .execute(&holder(), tx)
    .expect("exact XOR owner configures the UID policy");
}

fn usage(tx: &StateTransaction<'_, '_>, uid: u8) -> (Quantity, Quantity) {
    let rows = asset_uid::views(
        &*tx.world,
        &sccp_taira_xor_asset_definition_id(),
        Some(&[uid; 32]),
        tx.block_unix_timestamp_ms(),
        tx.block_height(),
    )
    .expect("UID view");
    assert_eq!(rows.len(), 1);
    (
        rows[0].daily_spend_usage.clone(),
        rows[0].daily_outgoing_usage.clone(),
    )
}

fn proven_with_uid_policy(
    tx: &mut StateTransaction<'_, '_>,
    recipient: &AccountId,
    amount: u128,
    fee_due: u128,
) -> [u8; 32] {
    let payload = SccpTransferPayloadV1::inbound(
        ETH,
        0,
        1,
        amount,
        external_account(ETH, 0x44),
        address_bytes(recipient),
    )
    .expect("payload");
    let message_id = payload.message_id(tx.network_id().as_bytes()).unwrap();
    let result = record_proven(
        tx,
        SccpProvenInboundV1 {
            network: ETH,
            revision: 1,
            payload: payload.encode().unwrap(),
            amount,
            message_id,
            source_locator: SccpSourceLocatorV1 {
                source_height: 100,
                block_hash: [7; 32],
                index_in_block: 0,
            },
            fee_due,
        },
    );
    assert!(
        result.is_ok(),
        "SCCP UID refusal must preserve the proven burn"
    );
    message_id
}

#[test]
fn uid_recipient_holding_refusal_is_pending_and_retries_without_movement() {
    let state = state();
    let mut block = state.block(header(5));
    let mut tx = block.transaction_for_fastpq_testing(Hash::new("sccp-uid-recipient"));
    install(&mut tx);
    fund(&mut tx, ETH, 1, 10 * ONE_XOR);
    let recipient = authority(61);
    recipients::ensure_registered(&mut tx, &recipient).unwrap();
    policy(
        &mut tx,
        61,
        1,
        std::slice::from_ref(&recipient),
        Some(0),
        None,
        None,
    );
    let id = proven_with_uid_policy(&mut tx, &recipient, 2 * ONE_XOR, 0);
    assert_eq!(
        inbound_status(&tx, &id),
        pending(SccpPendingReasonV1::CreditRefused)
    );
    assert_eq!(xor_balance(&*tx.world, &recipient), 0);
    assert_eq!(liability(&tx, ETH, 1), 10 * ONE_XOR);
    assert_eq!(store::pending_count(&*tx.world, &(ETH, 1)), (1, 0));
    assert_eq!(usage(&tx, 61), (Quantity::zero(), Quantity::zero()));
    assert_escrow_invariant(&*tx.world, "UID recipient held");
    policy(
        &mut tx,
        61,
        2,
        std::slice::from_ref(&recipient),
        Some(2 * ONE_XOR),
        None,
        None,
    );
    execute_settle(SettleSccpV1::inbound(id), &holder(), &mut tx).expect("UID cap lifted");
    assert!(matches!(
        inbound_status(&tx, &id),
        SccpInboundStatusV1::Released(_)
    ));
    assert_eq!(xor_balance(&*tx.world, &recipient), 2 * ONE_XOR);
    assert_eq!(liability(&tx, ETH, 1), 8 * ONE_XOR);
    assert_escrow_invariant(&*tx.world, "UID recipient retry");
    assert_pending_counts(&tx, "UID recipient retry");
}

#[test]
fn uid_shared_recipient_fee_cap_holds_before_either_credit_and_retries() {
    let state = state();
    set_original_fee_sink(&state, &authority(63));
    let mut block = state.block(header(5));
    let mut tx = block.transaction_for_fastpq_testing(Hash::new("sccp-uid-shared-fee"));
    install(&mut tx);
    fund(&mut tx, ETH, 1, 10 * ONE_XOR);
    let recipient = authority(62);
    let sink = authority(63);
    for account in [&recipient, &sink] {
        recipients::ensure_registered(&mut tx, account).unwrap();
    }
    set_fee_sink(&mut tx, Some(&sink));
    let accounts = [recipient.clone(), sink.clone()];
    policy(&mut tx, 62, 1, &accounts, Some(3 * ONE_XOR / 2), None, None);
    // Each leg fits alone. Only the complete ordered prefix detects the second refusal.
    escrow::precheck_release(&tx, ETH, &recipient, ONE_XOR).unwrap();
    escrow::precheck_release(&tx, ETH, &sink, ONE_XOR).unwrap();
    let id = proven_with_uid_policy(&mut tx, &recipient, 2 * ONE_XOR, ONE_XOR);
    assert_eq!(
        inbound_status(&tx, &id),
        pending(SccpPendingReasonV1::FeeSinkUnavailable)
    );
    assert_eq!(
        xor_balance(&*tx.world, &recipient),
        0,
        "no first credit on later UID refusal"
    );
    assert_eq!(xor_balance(&*tx.world, &sink), 0);
    assert_eq!(liability(&tx, ETH, 1), 10 * ONE_XOR);
    assert_eq!(usage(&tx, 62), (Quantity::zero(), Quantity::zero()));
    assert_escrow_invariant(&*tx.world, "shared UID fee held");
    policy(&mut tx, 62, 2, &accounts, Some(2 * ONE_XOR), None, None);
    execute_settle(SettleSccpV1::inbound(id), &holder(), &mut tx).expect("aggregate cap lifted");
    assert_eq!(xor_balance(&*tx.world, &recipient), ONE_XOR);
    assert_eq!(xor_balance(&*tx.world, &sink), ONE_XOR);
    assert_eq!(liability(&tx, ETH, 1), 8 * ONE_XOR);
    assert_escrow_invariant(&*tx.world, "shared UID fee retry");
    assert_pending_counts(&tx, "shared UID fee retry");
}

#[test]
fn uid_source_activity_prefix_holds_both_ceilings_without_consuming_usage() {
    for outgoing_only in [false, true] {
        let state = state();
        set_original_fee_sink(&state, &authority(65));
        let mut block = state.block(header(5));
        let mut tx = block.transaction_for_fastpq_testing(Hash::new("sccp-uid-source-activity"));
        install(&mut tx);
        fund(&mut tx, ETH, 1, 10 * ONE_XOR);
        let route_escrow = escrow_of(&tx, ETH);
        let recipient = authority(64);
        let sink = authority(65);
        for account in [&recipient, &sink] {
            recipients::ensure_registered(&mut tx, account).unwrap();
        }
        set_fee_sink(&mut tx, Some(&sink));
        let cap = 3 * ONE_XOR / 2;
        policy(
            &mut tx,
            64,
            1,
            std::slice::from_ref(&route_escrow),
            None,
            (!outgoing_only).then_some(cap),
            outgoing_only.then_some(cap),
        );
        let id = proven_with_uid_policy(&mut tx, &recipient, 2 * ONE_XOR, ONE_XOR);
        assert_eq!(
            inbound_status(&tx, &id),
            pending(SccpPendingReasonV1::FeeSinkUnavailable)
        );
        assert_eq!(xor_balance(&*tx.world, &recipient), 0);
        assert_eq!(xor_balance(&*tx.world, &sink), 0);
        assert_eq!(
            usage(&tx, 64),
            (Quantity::zero(), Quantity::zero()),
            "precheck never commits UID usage"
        );
        assert_eq!(liability(&tx, ETH, 1), 10 * ONE_XOR);
        assert_escrow_invariant(&*tx.world, "UID activity held");
        policy(
            &mut tx,
            64,
            2,
            &[route_escrow],
            None,
            (!outgoing_only).then_some(2 * ONE_XOR),
            outgoing_only.then_some(2 * ONE_XOR),
        );
        execute_settle(SettleSccpV1::inbound(id), &holder(), &mut tx)
            .expect("daily ceiling lifted");
        let charged = escrow::xor_quantity(2 * ONE_XOR).unwrap();
        assert_eq!(
            usage(&tx, 64),
            (charged.clone(), charged),
            "actual releases charge each debit exactly once"
        );
        assert_eq!(xor_balance(&*tx.world, &recipient), ONE_XOR);
        assert_eq!(xor_balance(&*tx.world, &sink), ONE_XOR);
        assert_escrow_invariant(&*tx.world, "UID activity retry");
        assert_pending_counts(&tx, "UID activity retry");
    }
}

#[test]
fn uid_refund_refusal_preserves_void_and_retries_after_cap_increase() {
    let state = state();
    let mut block = state.block(header(5));
    let mut tx = block.transaction_for_fastpq_testing(Hash::new("sccp-uid-refund"));
    install(&mut tx);
    let id = record_outbound(&mut tx, ETH, 2, &holder()).unwrap();
    let before = xor_balance(&*tx.world, &holder());
    policy(&mut tx, 66, 1, &[holder()], Some(before), None, None);
    let result = void_expired(&mut tx, ETH, 0);
    assert!(
        result.is_ok(),
        "SCCP UID refusal must preserve the proven void"
    );
    assert!(matches!(
        outbound_status(&tx, &id),
        SccpOutboundStatusV1::Voided(SccpVoidStatusV1 {
            refund_pending: Some(SccpPendingReasonV1::CreditRefused),
            ..
        })
    ));
    assert_eq!(xor_balance(&*tx.world, &holder()), before);
    assert_eq!(liability(&tx, ETH, 1), 2 * ONE_XOR);
    assert_eq!(store::pending_count(&*tx.world, &(ETH, 1)), (0, 1));
    assert_escrow_invariant(&*tx.world, "UID refund held");
    policy(
        &mut tx,
        66,
        2,
        &[holder()],
        Some(before + 2 * ONE_XOR),
        None,
        None,
    );
    execute_settle(SettleSccpV1::refund(ETH, 1, 0), &holder(), &mut tx).expect("refund cap lifted");
    assert!(matches!(
        outbound_status(&tx, &id),
        SccpOutboundStatusV1::Refunded(_)
    ));
    assert_eq!(xor_balance(&*tx.world, &holder()), before + 2 * ONE_XOR);
    assert_eq!(liability(&tx, ETH, 1), 0);
    assert_escrow_invariant(&*tx.world, "UID refund retry");
    assert_pending_counts(&tx, "UID refund retry");
}

fn enroll_system(tx: &mut StateTransaction<'_, '_>, account: &AccountId) {
    SetAssetUidSystemAccountV1 {
        account: AssetUidSystemAccountV1 {
            asset_definition_id: sccp_taira_xor_asset_definition_id(),
            account_id: account.clone(),
            revision: 1,
            enabled: true,
        },
    }
    .execute(&holder(), tx)
    .expect("issuer classifies exact system account");
}

#[test]
fn enrollment_refusal_retains_new_recipient_registration_and_retries() {
    let state = state();
    let mut block = state.block(header(5));
    let mut tx = block.transaction_for_fastpq_testing(Hash::new("sccp-uid-enrollment"));
    install(&mut tx);
    fund(&mut tx, ETH, 1, 10 * ONE_XOR);
    let route_escrow = escrow_of(&tx, ETH);
    enroll_system(&mut tx, &holder());
    enroll_system(&mut tx, &route_escrow);
    ActivateAssetUidEnrollmentV1 {
        asset_definition_id: sccp_taira_xor_asset_definition_id(),
    }
    .execute(&holder(), &mut tx)
    .expect("all positive balances classified");
    let recipient = authority(67);
    assert!(tx.world.account(&recipient).is_err());
    let id = proven_with_uid_policy(&mut tx, &recipient, 2 * ONE_XOR, 0);
    assert!(
        tx.world.account(&recipient).is_ok(),
        "registration remains after enrollment refusal"
    );
    assert_eq!(
        inbound_status(&tx, &id),
        pending(SccpPendingReasonV1::CreditRefused)
    );
    assert_eq!(xor_balance(&*tx.world, &recipient), 0);
    assert_eq!(liability(&tx, ETH, 1), 10 * ONE_XOR);
    assert_escrow_invariant(&*tx.world, "enrollment held");
    enroll_system(&mut tx, &recipient);
    execute_settle(SettleSccpV1::inbound(id), &holder(), &mut tx).expect("recipient enrolled");
    assert_eq!(xor_balance(&*tx.world, &recipient), 2 * ONE_XOR);
    assert_escrow_invariant(&*tx.world, "enrollment retry");
    assert_pending_counts(&tx, "enrollment retry");
}
