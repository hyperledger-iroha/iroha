//! SCCP cumulative source controls preserve the proof and both settlement credits.

use super::*;
use iroha_data_model::{
    asset::{
        ASSET_TRANSFER_CONTROL_METADATA_KEY, AssetTransferControlRecord,
        AssetTransferControlStoreV1, AssetTransferControlWindow, AssetTransferLimit,
    },
    isi::SetAssetTransferControl,
};

fn set_source_cap(tx: &mut StateTransaction<'_, '_>, account: &AccountId, cap: u128) {
    SetAssetTransferControl::new(
        account.clone(),
        sccp_taira_xor_asset_definition_id(),
        vec![AssetTransferLimit {
            window: AssetTransferControlWindow::Day,
            cap_amount: Some(escrow::xor_quantity(cap).expect("source cap quantity")),
        }],
    )
    .execute(&holder(), tx)
    .expect("XOR owner configures the route escrow source cap");
}

fn source_control(
    tx: &StateTransaction<'_, '_>,
    account: &AccountId,
) -> AssetTransferControlRecord {
    tx.world
        .account(account)
        .expect("route escrow")
        .metadata()
        .get(ASSET_TRANSFER_CONTROL_METADATA_KEY)
        .expect("source control metadata")
        .try_into_any_norito::<AssetTransferControlStoreV1>()
        .expect("canonical source controls")
        .find(&sccp_taira_xor_asset_definition_id())
        .expect("XOR source control")
        .clone()
}

#[test]
fn source_control_prefix_refusal_keeps_proof_and_both_credits_then_retries() {
    for (cap, reason) in [
        (ONE_XOR / 2, SccpPendingReasonV1::CreditRefused),
        (3 * ONE_XOR / 2, SccpPendingReasonV1::FeeSinkUnavailable),
    ] {
        let state = state();
        let mut block = state.block(header(5));
        let mut tx = block.transaction_for_fastpq_testing(Hash::new("sccp-source-control-prefix"));
        install(&mut tx);
        fund(&mut tx, ETH, 1, 10 * ONE_XOR);
        let route_escrow = escrow_of(&tx, ETH);
        let recipient = authority(71);
        let sink = authority(72);
        for account in [&recipient, &sink] {
            recipients::ensure_registered(&mut tx, account).unwrap();
            // Each receiver admits its own leg; the total must never be checked
            // as an individual receiver's credit.
            set_holding_limit(&mut tx, account, Some(ONE_XOR));
        }
        set_fee_sink(&mut tx, Some(&sink));
        set_source_cap(&mut tx, &route_escrow, cap);
        let control_before = source_control(&tx, &route_escrow);
        if cap > ONE_XOR {
            escrow::precheck_release(&tx, ETH, &recipient, ONE_XOR).expect("recipient fits alone");
            escrow::precheck_release(&tx, ETH, &sink, ONE_XOR).expect("fee fits alone");
        }
        let payload = SccpTransferPayloadV1::inbound(
            ETH,
            0,
            1,
            2 * ONE_XOR,
            external_account(ETH, 0x44),
            address_bytes(&recipient),
        )
        .expect("payload");
        let message_id = payload.message_id(tx.network_id().as_bytes()).unwrap();
        let result = record_proven(
            &mut tx,
            SccpProvenInboundV1 {
                network: ETH,
                revision: 1,
                payload: payload.encode().unwrap(),
                amount: 2 * ONE_XOR,
                message_id,
                source_locator: SccpSourceLocatorV1 {
                    source_height: 100,
                    block_hash: [7; 32],
                    index_in_block: 0,
                },
                fee_due: ONE_XOR,
            },
        );
        assert!(
            result.is_ok(),
            "SCCP source-control refusal must preserve the proven burn"
        );
        assert_eq!(inbound_status(&tx, &message_id), pending(reason));
        assert_eq!(xor_balance(&*tx.world, &recipient), 0);
        assert_eq!(xor_balance(&*tx.world, &sink), 0);
        assert_eq!(source_control(&tx, &route_escrow), control_before);
        assert_eq!(liability(&tx, ETH, 1), 10 * ONE_XOR);
        assert_eq!(store::pending_count(&*tx.world, &(ETH, 1)), (1, 0));
        assert_escrow_invariant(&*tx.world, "source control prefix held");
        assert_pending_counts(&tx, "source control prefix held");

        set_source_cap(&mut tx, &route_escrow, 2 * ONE_XOR);
        execute_settle(SettleSccpV1::inbound(message_id), &holder(), &mut tx)
            .expect("source cap lifted");
        assert!(matches!(
            inbound_status(&tx, &message_id),
            SccpInboundStatusV1::Released(_)
        ));
        assert_eq!(xor_balance(&*tx.world, &recipient), ONE_XOR);
        assert_eq!(xor_balance(&*tx.world, &sink), ONE_XOR);
        let control_after = source_control(&tx, &route_escrow);
        assert_eq!(control_after.usages.len(), 1);
        assert_eq!(
            control_after.usages[0].window,
            AssetTransferControlWindow::Day
        );
        assert_eq!(
            control_after.usages[0].spent_amount,
            escrow::xor_quantity(2 * ONE_XOR).unwrap()
        );
        assert_eq!(liability(&tx, ETH, 1), 8 * ONE_XOR);
        assert_escrow_invariant(&*tx.world, "source control prefix retry");
        assert_pending_counts(&tx, "source control prefix retry");
    }
}
