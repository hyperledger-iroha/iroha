//! Sanity checks for schema hash helpers.

#[test]
fn type_name_and_string_based_schema_hash_agree() {
    let via_type = norito::core::type_name_schema_hash::<String>();
    let name = core::any::type_name::<String>();
    let via_name = norito::core::schema_hash_for_name(name);
    assert_eq!(via_type, via_name);

    // Different types should not collide in this test set
    let h_u32 = norito::core::type_name_schema_hash::<u32>();
    assert_ne!(via_type, h_u32);
}

#[test]
fn schema_hash_deterministic_across_calls() {
    let a1 = norito::core::type_name_schema_hash::<(u8, bool)>();
    let a2 = norito::core::type_name_schema_hash::<(u8, bool)>();
    assert_eq!(a1, a2);
}

#[cfg(feature = "schema-structural")]
#[test]
fn structural_schema_hash_matches_reference() {
    let structural = norito::json!({
        "Sample": {"Struct": [
            {"name": "id", "type": "u64"},
            {"name": "name", "type": "String"},
            {"name": "flag", "type": "bool"},
        ]},
        "String": "String",
        "bool": "bool",
        "u64": {"Int": "FixedWidth"},
    });

    let expected = [
        0x3A, 0xE1, 0x59, 0x17, 0x41, 0xF6, 0x66, 0x46, 0x2F, 0xB7, 0x66, 0x57, 0x20, 0xDD, 0xDE,
        0x6C,
    ];

    let value_hash = norito::core::schema_hash_structural_value(&structural);
    assert_eq!(value_hash, expected);

    let json = norito::json::to_json(&structural).expect("serialize structural value");
    let from_str =
        norito::core::schema_hash_structural_from_json_str(&json).expect("hash from str");
    assert_eq!(from_str, expected);

    let from_bytes = norito::core::schema_hash_structural_from_json_bytes(json.as_bytes())
        .expect("hash from bytes");
    assert_eq!(from_bytes, expected);
}
