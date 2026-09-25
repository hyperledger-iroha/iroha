//! Durable, release-scoped experimental mint-credit ledger tests.

use super::*;
use crate::zk::{
    kagemusha_v1_recursion::testnet_observation::KagemushaTestnetValueAdmissionV1,
    kagemusha_v1_state::TestPersistenceFailure,
};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    NetworkId, block::consensus_v2::HeightContextId,
    isi::kagemusha_v1::KagemushaFinalityTrustAnchorV1,
};

fn scope_and_anchor() -> (
    KagemushaTestnetStateObservationScopeV1,
    KagemushaFinalityTrustAnchorV1,
) {
    let network_id =
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([1; 32])));
    let scope = KagemushaTestnetStateObservationScopeV1::new(
        *network_id.as_bytes(),
        [2; 32],
        [3; 32],
        2,
        [4; 32],
        [5; 32],
        [6; 32],
    )
    .unwrap();
    let anchor = KagemushaFinalityTrustAnchorV1 {
        network_id,
        block_height: 9,
        height_context_id: HeightContextId(HashOf::from_untyped_unchecked(Hash::prehashed(
            [7; 32],
        ))),
    };
    anchor.validate().unwrap();
    (scope, anchor)
}

fn admission(
    scope: KagemushaTestnetStateObservationScopeV1,
    anchor: KagemushaFinalityTrustAnchorV1,
    operation_id: DigestV1,
    credit_id: DigestV1,
    amount: u128,
    mint_digest: DigestV1,
) -> KagemushaTestnetValueAdmissionV1 {
    KagemushaTestnetValueAdmissionV1::test_only_admission(
        scope,
        operation_id,
        credit_id,
        amount,
        mint_digest,
        anchor,
    )
}

fn path(directory: &tempfile::TempDir) -> std::path::PathBuf {
    directory
        .path()
        .canonicalize()
        .unwrap()
        .join("value-ledger")
}

#[test]
fn opaque_admission_is_durable_scoped_and_idempotent() {
    let directory = tempfile::tempdir().unwrap();
    let (scope, anchor) = scope_and_anchor();
    let mut ledger =
        KagemushaTestnetMintCreditLedgerV1::create_new_with_scope(&path(&directory), scope)
            .unwrap();
    let first = admission(scope, anchor, [0x11; 32], [0x21; 32], 17, [0x31; 32]);
    let credited = ledger.credit(&first).unwrap();
    assert_eq!(credited.scope(), scope);
    assert_eq!(credited.operation_id(), [0x11; 32]);
    assert_eq!(credited.credit_id(), [0x21; 32]);
    assert_eq!(credited.amount(), 17);
    assert_eq!(ledger.credit(&first).unwrap(), credited);
    assert_eq!(ledger.credit_count(), 1);
    assert_eq!(ledger.total_admitted(), 17);
    assert_eq!(ledger.credit_by_operation([0x11; 32]), Some(credited));
    assert_eq!(ledger.credit_by_credit_id([0x21; 32]), Some(credited));
    assert_eq!(ledger.credit_by_credit_id([0x22; 32]), None);
    let changed = admission(scope, anchor, [0x11; 32], [0x21; 32], 18, [0x31; 32]);
    assert!(ledger.credit(&changed).is_err());
    let duplicate_credit = admission(scope, anchor, [0x12; 32], [0x21; 32], 1, [0x32; 32]);
    assert!(ledger.credit(&duplicate_credit).is_err());
    let wrong_scope = KagemushaTestnetStateObservationScopeV1::new(
        scope.network_id(),
        scope.asset_identity_digest(),
        scope.asset_incarnation(),
        scope.asset_scale(),
        scope.liability_pool_id(),
        [0x55; 32],
        scope.release_attestation_digest(),
    )
    .unwrap();
    assert!(
        ledger
            .credit(&admission(
                wrong_scope,
                anchor,
                [0x13; 32],
                [0x23; 32],
                1,
                [0x33; 32]
            ))
            .is_err()
    );
    assert!(
        ledger
            .credit(&admission(
                scope, anchor, [0x14; 32], [0x24; 32], 0, [0x34; 32]
            ))
            .is_err()
    );
    assert_eq!(ledger.total_admitted(), 17);
}

#[test]
fn recovery_rederives_every_credit_and_rejects_changed_or_missing_source() {
    let directory = tempfile::tempdir().unwrap();
    let ledger_path = path(&directory);
    let (scope, anchor) = scope_and_anchor();
    let first = admission(scope, anchor, [0x11; 32], [0x21; 32], 17, [0x31; 32]);
    let mut ledger =
        KagemushaTestnetMintCreditLedgerV1::create_new_with_scope(&ledger_path, scope).unwrap();
    ledger.credit(&first).unwrap();
    drop(ledger);
    let wrong_scope = KagemushaTestnetStateObservationScopeV1::new(
        scope.network_id(),
        scope.asset_identity_digest(),
        scope.asset_incarnation(),
        scope.asset_scale(),
        scope.liability_pool_id(),
        [0x55; 32],
        scope.release_attestation_digest(),
    )
    .unwrap();
    assert!(
        KagemushaTestnetMintCreditLedgerV1::open_existing_with(&ledger_path, wrong_scope, |_| {
            Ok(admission(
                scope, anchor, [0x11; 32], [0x21; 32], 17, [0x31; 32],
            ))
        })
        .is_err()
    );
    assert!(
        KagemushaTestnetMintCreditLedgerV1::open_existing_with(&ledger_path, scope, |_| Err(
            ledger_error("source proof missing")
        ))
        .is_err()
    );
    assert!(
        KagemushaTestnetMintCreditLedgerV1::open_existing_with(&ledger_path, scope, |_| {
            Ok(admission(
                scope, anchor, [0x11; 32], [0x21; 32], 18, [0x31; 32],
            ))
        })
        .is_err()
    );
    let recovered =
        KagemushaTestnetMintCreditLedgerV1::open_existing_with(&ledger_path, scope, |op| {
            assert_eq!(op, [0x11; 32]);
            Ok(admission(
                scope, anchor, [0x11; 32], [0x21; 32], 17, [0x31; 32],
            ))
        })
        .unwrap();
    assert_eq!(recovered.credit_count(), 1);
    assert_eq!(recovered.total_admitted(), 17);
    assert!(
        KagemushaTestnetMintCreditLedgerV1::open_existing_with(&ledger_path, scope, |_| {
            Ok(admission(
                scope, anchor, [0x11; 32], [0x21; 32], 17, [0x31; 32],
            ))
        })
        .is_err()
    );
}

#[test]
fn duplicate_credit_on_disk_and_uncertain_append_fail_closed() {
    let directory = tempfile::tempdir().unwrap();
    let ledger_path = path(&directory);
    let (scope, anchor) = scope_and_anchor();
    let first = admission(scope, anchor, [0x11; 32], [0x21; 32], 17, [0x31; 32]);
    let mut ledger =
        KagemushaTestnetMintCreditLedgerV1::create_new_with_scope(&ledger_path, scope).unwrap();
    ledger.credit(&first).unwrap();
    let conflicting = admission(scope, anchor, [0x12; 32], [0x21; 32], 5, [0x32; 32]);
    let conflicting_facts = CreditFacts::from_admission(&conflicting).unwrap();
    ledger
        .wal
        .append(&encode_record(&Record::Credit(conflicting_facts)).unwrap())
        .unwrap();
    drop(ledger);
    assert!(
        KagemushaTestnetMintCreditLedgerV1::open_existing_with(&ledger_path, scope, |op| {
            if op == first.operation_id() {
                Ok(admission(
                    scope, anchor, [0x11; 32], [0x21; 32], 17, [0x31; 32],
                ))
            } else {
                Ok(admission(
                    scope, anchor, [0x12; 32], [0x21; 32], 5, [0x32; 32],
                ))
            }
        })
        .is_err()
    );

    let second_directory = tempfile::tempdir().unwrap();
    let second_path = path(&second_directory);
    let mut ledger =
        KagemushaTestnetMintCreditLedgerV1::create_new_with_scope(&second_path, scope).unwrap();
    ledger
        .wal
        .failure
        .set(Some(TestPersistenceFailure::BeforeSync));
    assert!(ledger.credit(&first).is_err());
    assert_eq!(ledger.credit_count(), 0);
    assert!(ledger.credit(&first).is_err());
    drop(ledger);
    let recovered =
        KagemushaTestnetMintCreditLedgerV1::open_existing_with(&second_path, scope, |_| {
            Ok(admission(
                scope, anchor, [0x11; 32], [0x21; 32], 17, [0x31; 32],
            ))
        })
        .unwrap();
    assert_eq!(recovered.total_admitted(), 17);
}
