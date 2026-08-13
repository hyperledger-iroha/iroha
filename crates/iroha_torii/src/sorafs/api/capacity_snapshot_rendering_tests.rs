// Capacity snapshot JSON projection regression tests.
//
// Included by `advert_tests` to preserve the exact libtest path.

#[test]
fn snapshot_to_json_renders_counts() {
    let declaration = RegistryDeclaration {
        provider_id_hex: "aa".into(),
        committed_capacity_gib: 10,
        registered_epoch: 1,
        valid_from_epoch: 2,
        valid_until_epoch: 3,
        declaration_json: json_object(vec![json_entry("version", 1u64)]),
        metadata_json: Value::Object(Map::new()),
    };
    let ledger = RegistryFeeLedgerEntry {
        provider_id_hex: "aa".into(),
        total_declared_gib: 100,
        total_utilised_gib: 80,
        storage_fee: 30_u64.into(),
        egress_fee: 12_u64.into(),
        accrued_fee: 42_u64.into(),
        expected_settlement: 84_u64.into(),
        penalty_slashed: Quantity::zero(),
        penalty_events: 0,
        last_updated_epoch: 4,
    };
    let credit = RegistryCreditLedgerEntry {
        provider_id_hex: "aa".into(),
        available_credit: 1_000_u64.into(),
        bonded: 500_u64.into(),
        required_bond: 400_u64.into(),
        expected_settlement: 300_u64.into(),
        onboarding_epoch: 1,
        last_settlement_epoch: 2,
        low_balance_since_epoch: None,
        slashed: Quantity::zero(),
        under_delivery_strikes: 0,
        last_penalty_epoch: None,
        metadata_json: Value::Object(Map::new()),
    };
    let snapshot = CapacitySnapshot {
        declaration_count: 1,
        fee_ledger_count: 1,
        credit_ledger_count: 1,
        dispute_count: 0,
        declarations: vec![declaration],
        fee_ledger: vec![ledger],
        credit_ledger: vec![credit],
        disputes: Vec::new(),
    };
    let json = snapshot_to_json(snapshot, DEFAULT_LIST_LIMIT).expect("serialize snapshot");
    let map = json.as_object().expect("json object");
    assert_eq!(map.json_u64(&["declaration_count"]), Some(1));
    assert_eq!(map.json_u64(&["ledger_count"]), Some(1));
    assert_eq!(map.json_u64(&["credit_ledger_count"]), Some(1));
    assert_eq!(map.json_u64(&["dispute_count"]), Some(0));
    assert_eq!(map.json_u64(&["returned_declaration_count"]), Some(1));
    assert_eq!(map.json_u64(&["returned_ledger_count"]), Some(1));
    assert_eq!(map.json_u64(&["returned_credit_ledger_count"]), Some(1));
    assert_eq!(map.json_u64(&["returned_dispute_count"]), Some(0));
    assert_eq!(map.json_u64(&["limit"]), Some(DEFAULT_LIST_LIMIT as u64));
    assert_eq!(map.json_bool(&["truncated_declarations"]), Some(false));
    assert_eq!(map.json_bool(&["truncated_fee_ledger"]), Some(false));
    assert_eq!(map.json_bool(&["truncated_credit_ledger"]), Some(false));
    assert_eq!(map.json_bool(&["truncated_disputes"]), Some(false));
    assert!(!map.contains_key("local_usage"));
    assert!(map.get("disputes").is_some());
    assert!(map.get("credit_ledger").is_some());
}
