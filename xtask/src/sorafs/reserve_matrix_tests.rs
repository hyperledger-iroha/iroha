//! Reserve pricing matrix and ledger projection regressions.
use super::*;
fn render_matrix_json(options: ReserveMatrixOptions) -> json::Value {
    let value = reserve_matrix_report(options).expect("matrix report");
    let rendered = norito::json::to_json_pretty(&value).expect("matrix payload renders to JSON");
    json::from_str(&rendered).expect("matrix payload parses")
}
#[test]
fn reserve_matrix_includes_ledger_projection() {
    let options = ReserveMatrixOptions {
        capacities_gib: vec![10, 25],
        storage_classes: vec![StorageClass::Hot],
        tiers: vec![ReserveTier::TierA, ReserveTier::TierB],
        durations: vec![ReserveDuration::Monthly],
        reserve_balance: XorQuantity::try_from_micro(5 * MICRO_XOR_PER_XOR)
            .expect("legacy micro-XOR value is representable"),
        policy_json: None,
        policy_norito: None,
        label: Some("matrix-test".into()),
    };
    let json = render_matrix_json(options);
    assert_eq!(json["policy_version"].as_u64().unwrap(), 1);
    assert_eq!(
        json["policy_source"].as_str().unwrap(),
        "embedded default policy"
    );
    assert_eq!(json["matrix_entry_count"].as_u64().unwrap(), 4);
    assert_eq!(json["matrix"].as_array().unwrap().len(), 4);
    assert_eq!(json["label"].as_str().unwrap(), "matrix-test");
    assert_eq!(
        json["reserve_balance_micro_xor"].as_u64().unwrap(),
        5 * MICRO_XOR_PER_XOR as u64
    );
    assert_eq!(json["policy_sha256"].as_str().unwrap().len(), 64);
    let entry = &json["matrix"][0];
    assert_eq!(entry["storage_class"].as_str().unwrap(), "hot");
    assert_eq!(entry["tier"].as_str().unwrap(), "tier-a");
    assert_eq!(entry["inputs"]["storage_class"].as_str().unwrap(), "hot");
    assert_eq!(entry["inputs"]["reserve_balance"].as_str(), Some("5"));
    let reserve_balance: XorQuantity =
        json::value::from_value(entry["inputs"]["reserve_balance"].clone())
            .expect("canonical XOR quantity");
    assert_eq!(
        reserve_balance
            .try_to_micro()
            .expect("exact micro-XOR projection"),
        u128::from(json["reserve_balance_micro_xor"].as_u64().unwrap())
    );
    let rent_due: XorQuantity =
        json::value::from_value(entry["ledger_projection"]["rent_due"].clone())
            .expect("canonical projected rent");
    let effective_rent: XorQuantity =
        json::value::from_value(entry["quote"]["effective_rent"].clone())
            .expect("canonical quoted rent");
    assert_eq!(rent_due, effective_rent);
}
#[test]
fn reserve_matrix_requires_capacity() {
    let options = ReserveMatrixOptions {
        capacities_gib: vec![],
        storage_classes: vec![StorageClass::Hot],
        tiers: vec![ReserveTier::TierA],
        durations: vec![ReserveDuration::Monthly],
        reserve_balance: XorQuantity::zero(),
        policy_json: None,
        policy_norito: None,
        label: None,
    };
    let err = reserve_matrix_report(options).expect_err("matrix should fail without capacities");
    assert!(
        err.to_string()
            .contains("reserve matrix requires at least one --capacity")
    );
}
