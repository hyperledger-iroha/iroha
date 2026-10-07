//! Closed carrier DATA tests. These inventory fixtures contain no source keys or authority.
use super::*;
use iroha_core_zk::kagemusha_wallet_artifacts_v1::producer_inventory::{FinalityV1, OriginalV1};
fn fixture() -> ProducerInventoryV1 {
    let blob = |n| BlobV1 {
        bytes: 16,
        sha256: [n; 32],
    };
    ProducerInventoryV1 {
        version: 1,
        native_profile: [0; 32],
        originals: vec![
            OriginalV1 {
                descriptor: blob(1),
                verifying_key: blob(2),
                proving_key: blob(3),
            },
            OriginalV1 {
                descriptor: blob(3),
                verifying_key: blob(4),
                proving_key: blob(5),
            },
        ],
        sigma: [0; 16],
        operations: vec![],
        routes: vec![],
        terminals: vec![],
        omega: 0,
        finality: FinalityV1 {
            network: [0; 32],
            instance: [0; 32],
            initial_context: [0; 32],
            initial_epoch: 0,
            parameters: [0; 6],
            originals: vec![],
        },
    }
}
fn transport(rows: norito::json::Value) -> Vec<u8> {
    norito::json::to_json(
        &norito::json!({"schema":"iroha.kagemusha.wallet-artifact-original-transport.v1",
        "walletOriginals":rows,"finalityOriginals":[]}),
    )
    .unwrap()
    .into_bytes()
}
fn rows() -> norito::json::Value {
    norito::json::Value::Array(
        (1u8..=5)
            .map(|n| norito::json!({"bytes":16,"sha256":(hex::encode([n;32]))}))
            .collect(),
    )
}
#[test]
fn transport_is_the_exact_sorted_deduplicated_whole_catalog_closure() {
    require_transport(&fixture(), &transport(rows())).unwrap();
    let norito::json::Value::Array(mut actual) = rows() else {
        panic!("rows")
    };
    actual.swap(0, 1);
    assert!(require_transport(&fixture(), &transport(norito::json::Value::Array(actual))).is_err());
    let norito::json::Value::Array(mut actual) = rows() else {
        panic!("rows")
    };
    actual.push(actual[0].clone());
    assert!(require_transport(&fixture(), &transport(norito::json::Value::Array(actual))).is_err());
    let norito::json::Value::Array(mut actual) = rows() else {
        panic!("rows")
    };
    actual.pop();
    assert!(require_transport(&fixture(), &transport(norito::json::Value::Array(actual))).is_err());
}
#[test]
fn transport_extent_and_cross_role_conflicts_cannot_choose_other_originals() {
    let mut inventory = fixture();
    inventory.originals[1].descriptor.bytes = 17;
    assert!(require_transport(&inventory, &transport(rows())).is_err());
    let mut roles = BTreeMap::new();
    let original = BlobV1 {
        bytes: 16,
        sha256: [3; 32],
    };
    insert(&mut roles, original, Store::Finality).unwrap();
    insert(&mut roles, original, Store::Wallet).unwrap();
    assert_eq!(roles[&original.sha256].store, Store::Wallet);
    assert!(
        insert(
            &mut roles,
            BlobV1 {
                bytes: 17,
                ..original
            },
            Store::Finality
        )
        .is_err()
    );
    assert!(
        insert(
            &mut roles,
            BlobV1 {
                bytes: 0,
                ..original
            },
            Store::Wallet
        )
        .is_err()
    );
    assert!(
        insert(
            &mut roles,
            BlobV1 {
                sha256: [0; 32],
                ..original
            },
            Store::Wallet
        )
        .is_err()
    );
}
#[test]
fn partial_offerings_are_invalid_and_io_never_means_financial_absence() {
    assert_eq!(
        storage(std::io::Error::from(std::io::ErrorKind::NotFound)).status,
        INVALID
    );
    assert_eq!(
        storage(std::io::Error::from(std::io::ErrorKind::InvalidData)).status,
        INVALID
    );
    assert_eq!(
        storage(std::io::Error::from(std::io::ErrorKind::PermissionDenied)).status,
        UNAVAILABLE
    );
    let extra = norito::json::to_json(
        &norito::json!({"schema":"iroha.kagemusha.wallet-artifact-original-transport.v1",
        "walletOriginals":[],"finalityOriginals":[],"ready":true}),
    )
    .unwrap()
    .into_bytes();
    assert!(require_transport(&fixture(), &extra).is_err());
}

#[test]
fn retired_project_transport_schema_is_rejected_even_with_exact_rows() {
    let raw = transport(rows());
    let old = std::str::from_utf8(&raw).unwrap().replace(
        "iroha.kagemusha.wallet-artifact-original-transport.v1",
        "bpng.current-wallet-artifact-original-transport.v1",
    );
    assert!(require_transport(&fixture(), old.as_bytes()).is_err());
}
