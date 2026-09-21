//! Shared history extraction retains receipt bytes and rejects cross-purpose retained state.
use super::*;
use crate::query::signer_custody_history::{
    self as shared, AccountPurpose, ControlAction, ControlTransition, CustodyPurpose, HistoryError,
};

mod first_use;

#[test]
fn prepared_shared_control_is_unpublished_and_matches_exact_native_provenance() {
    let mut f = fixture();
    let bytes = encode(&f.policy).unwrap();
    transact(&mut f.state, 1_000, |tx| {
        let native = instruction(tx, Action::Configure(bytes.clone()));
        let digest = final_promotion_authority_request_digest_v1(&native, &f.manager).unwrap();
        let before = retained(tx);
        let prepared = shared::prepare_control::<ReceiptPurpose>(
            tx,
            &f.manager,
            None,
            ControlTransition {
                deployment: DEPLOYMENT,
                expected_revision: 0,
                expected_digest: [0; 32],
                request_digest: digest,
                action: ControlAction::Configure(&bytes),
            },
        )
        .unwrap();
        assert_eq!(retained(tx), before, "prepared writes are not published");
        let staged =
            shared::staging_fixture::staged_control::<ReceiptPurpose>(&prepared, DEPLOYMENT);
        assert_eq!(staged.record.request_digest, digest);
        assert_eq!(staged.record.execution.authority, f.manager);
        assert_eq!(staged.record.execution.height, 1);
        assert_eq!(staged.record.execution.ordinal, 0);
        assert_eq!(staged.record.execution.recorded_at_unix_ms, 1_000);
        assert_eq!(staged.state.policy, f.policy);
        assert_eq!(staged.record.enrollment, None);
        native.execute(&f.manager, tx).unwrap();
        let actual = shared::read_control::<ReceiptPurpose>(tx.world(), DEPLOYMENT)
            .unwrap()
            .unwrap();
        assert_eq!(actual.record, staged.record);
        assert_eq!(actual.state, staged.state);
        assert_eq!(actual.index, staged.index);
        for (path, bytes) in prepared {
            assert_eq!(tx.world().smart_contract_state().get(&path), Some(&bytes));
        }
    });
}

#[test]
fn receipt_control_bytes_cannot_be_replayed_under_account_custody_namespace() {
    let mut f = fixture();
    configure(&mut f);
    transact(&mut f.state, 2_000, |tx| {
        let receipt_scope = shared::scope::<ReceiptPurpose>(DEPLOYMENT);
        let account_scope = shared::scope::<AccountPurpose>(DEPLOYMENT);
        assert_ne!(receipt_scope, account_scope);
        assert_ne!(ReceiptPurpose::RECORD_DOMAIN, AccountPurpose::RECORD_DOMAIN);
        let rows = retained(tx);
        for (path, bytes) in &rows {
            if let Some(suffix) = path.as_ref().strip_prefix(receipt_scope.as_str()) {
                let path: StatePath = format!("{account_scope}{suffix}").parse().unwrap();
                tx.world.smart_contract_state.insert(path, bytes.clone());
            }
        }
        let before = retained(tx);
        assert!(matches!(
            shared::read_control::<AccountPurpose>(tx.world(), DEPLOYMENT),
            Err(HistoryError::CorruptHistory)
        ));
        assert!(
            shared::read_control::<ReceiptPurpose>(tx.world(), DEPLOYMENT)
                .unwrap()
                .is_some()
        );
        assert_eq!(
            retained(tx),
            before,
            "a rejected cross-purpose read changes no native state"
        );
    });
}

#[test]
fn shared_control_keeps_declared_index_identity_and_bounded_canonical_frames() {
    assert_eq!(
        <ControlIndexV1 as norito::NoritoSchema>::nominal_name(),
        "iroha_core::query::final_promotion_authority::ControlIndexV1"
    );
    let index = ControlIndexV1 {
        revision: 7,
        digest: [0x31; 32],
        height: 11,
        ordinal: 2,
    };
    let bytes = shared::encode(&index).unwrap();
    assert_eq!(shared::decode::<ControlIndexV1>(&bytes).unwrap(), index);
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert_eq!(
        shared::decode::<ControlIndexV1>(&trailing),
        Err(HistoryError::Invalid)
    );
    assert_eq!(
        shared::decode::<ControlIndexV1>(&[]),
        Err(HistoryError::Invalid)
    );
    assert_eq!(
        shared::encode(&vec![0_u8; 32 * 1024]),
        Err(HistoryError::Invalid)
    );
    assert_eq!(
        shared::decode::<ControlIndexV1>(&vec![0; 32 * 1024 + 1]),
        Err(HistoryError::Invalid)
    );
    for bad in ["", "other deployment", "test-deployment"] {
        assert!(!shared::valid_deployment::<ReceiptPurpose>(bad));
        assert!(!shared::valid_deployment::<AccountPurpose>(bad));
    }
}
