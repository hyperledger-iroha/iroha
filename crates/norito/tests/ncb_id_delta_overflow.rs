//! Ensure forced id-delta encoding falls back when deltas exceed i64 range.

use norito::columnar::{
    ComboPolicy, EnumBorrow, encode_ncb_u64_enum_bool,
    encode_ncb_u64_str_u32_bool_with_policy, encode_ncb_u64_u32_bool,
    view_ncb_u64_enum_bool, view_ncb_u64_str_u32_bool, view_ncb_u64_u32_bool,
};

#[test]
fn ncb_u64_u32_bool_forced_delta_overflow_falls_back() {
    let rows = vec![(0u64, 1u32, false), (u64::MAX, 2u32, true)];
    let bytes = encode_ncb_u64_u32_bool(&rows, true, false);
    let view = view_ncb_u64_u32_bool(&bytes).expect("view ncb u64-u32-bool");
    assert_eq!(view.id(0), 0);
    assert_eq!(view.id(1), u64::MAX);
    assert_eq!(view.val(1), 2u32);
    assert!(view.flag(1));
}

#[test]
fn ncb_u64_str_u32_bool_forced_delta_overflow_falls_back() {
    let rows = vec![
        (0u64, "alpha", 1u32, false),
        (u64::MAX, "beta", 2u32, true),
    ];
    let policy = ComboPolicy {
        force_id_delta: Some(true),
        ..ComboPolicy::default()
    };
    let bytes = encode_ncb_u64_str_u32_bool_with_policy(&rows, policy);
    let view = view_ncb_u64_str_u32_bool(&bytes).expect("view ncb u64-str-u32-bool");
    assert_eq!(view.id(1), u64::MAX);
    assert_eq!(view.name(1).expect("name"), "beta");
    assert_eq!(view.val(1), 2u32);
    assert!(view.flag(1));
}

#[test]
fn ncb_u64_enum_bool_forced_delta_overflow_falls_back() {
    let rows = vec![
        (0u64, EnumBorrow::Code(1), false),
        (u64::MAX, EnumBorrow::Code(2), true),
    ];
    let bytes = encode_ncb_u64_enum_bool(&rows, true, false, false);
    let view = view_ncb_u64_enum_bool(&bytes).expect("view ncb u64-enum-bool");
    assert_eq!(view.id(1), u64::MAX);
    assert!(view.flag(1));
}
