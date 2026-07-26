//! AoS borrowed views honor COMPACT_LEN for per-field lengths.

use norito::{
    aos,
    columnar::{
        AosEnumRef, EnumBorrow, view_aos_u64_bytes_bool, view_aos_u64_bytes_u32_bool,
        view_aos_u64_enum_bool, view_aos_u64_optstr_bool, view_aos_u64_optu32_bool,
        view_aos_u64_str_bool, view_aos_u64_str_u32_bool,
    },
    core::{DecodeFlagsGuard, header_flags, reset_decode_state},
};

#[test]
fn aos_view_str_bool_compact_len() {
    {
        let _guard = DecodeFlagsGuard::enter(header_flags::COMPACT_LEN);
        let rows = [(1u64, "hi", true), (2u64, "yo", false)];
        let body = aos::encode_rows_u64_str_bool(&rows);
        let view = view_aos_u64_str_bool(&body).expect("view");
        assert_eq!(view.len(), rows.len());
        assert_eq!(view.id(0), 1);
        assert_eq!(view.name(0).expect("name"), "hi");
        assert!(view.flag(0));
        assert_eq!(view.name(1).expect("name"), "yo");
        assert!(!view.flag(1));
    }
    reset_decode_state();
}

#[test]
fn aos_view_bytes_bool_compact_len() {
    {
        let _guard = DecodeFlagsGuard::enter(header_flags::COMPACT_LEN);
        let payload = [1u8, 2u8, 3u8];
        let rows = [(9u64, payload.as_slice(), true)];
        let body = aos::encode_rows_u64_bytes_bool(&rows);
        let view = view_aos_u64_bytes_bool(&body).expect("view");
        assert_eq!(view.id(0), 9);
        assert_eq!(view.data(0), payload.as_slice());
        assert!(view.flag(0));
    }
    reset_decode_state();
}

#[test]
fn aos_view_str_u32_bool_compact_len() {
    {
        let _guard = DecodeFlagsGuard::enter(header_flags::COMPACT_LEN);
        let rows = [(7u64, "a", 42u32, true)];
        let body = aos::encode_rows_u64_str_u32_bool(&rows);
        let view = view_aos_u64_str_u32_bool(&body).expect("view");
        assert_eq!(view.id(0), 7);
        assert_eq!(view.name(0).expect("name"), "a");
        assert_eq!(view.val(0), 42);
        assert!(view.flag(0));
    }
    reset_decode_state();
}

#[test]
fn aos_view_bytes_u32_bool_compact_len() {
    {
        let _guard = DecodeFlagsGuard::enter(header_flags::COMPACT_LEN);
        let payload = [9u8, 8u8];
        let rows = [(4u64, payload.as_slice(), 11u32, false)];
        let body = aos::encode_rows_u64_bytes_u32_bool(&rows);
        let view = view_aos_u64_bytes_u32_bool(&body).expect("view");
        assert_eq!(view.id(0), 4);
        assert_eq!(view.data(0), payload.as_slice());
        assert_eq!(view.val(0), 11);
        assert!(!view.flag(0));
    }
    reset_decode_state();
}

#[test]
fn aos_view_optstr_bool_compact_len() {
    {
        let _guard = DecodeFlagsGuard::enter(header_flags::COMPACT_LEN);
        let rows = [(1u64, Some("ok"), true), (2u64, None, false)];
        let body = aos::encode_rows_u64_optstr_bool(&rows);
        let view = view_aos_u64_optstr_bool(&body).expect("view");
        assert_eq!(view.id(0), 1);
        assert_eq!(view.name(0).expect("name"), Some("ok"));
        assert!(view.flag(0));
        assert_eq!(view.id(1), 2);
        assert_eq!(view.name(1).expect("name"), None);
        assert!(!view.flag(1));
    }
    reset_decode_state();
}

#[test]
fn aos_view_optu32_bool_compact_len() {
    {
        let _guard = DecodeFlagsGuard::enter(header_flags::COMPACT_LEN);
        let rows = [(1u64, Some(7u32), true), (2u64, None, false)];
        let body = aos::encode_rows_u64_optu32_bool(&rows);
        let view = view_aos_u64_optu32_bool(&body).expect("view");
        assert_eq!(view.id(0), 1);
        assert_eq!(view.val(0), Some(7));
        assert!(view.flag(0));
        assert_eq!(view.id(1), 2);
        assert_eq!(view.val(1), None);
        assert!(!view.flag(1));
    }
    reset_decode_state();
}

#[test]
fn aos_view_enum_bool_compact_len() {
    {
        let _guard = DecodeFlagsGuard::enter(header_flags::COMPACT_LEN);
        let rows = [
            (5u64, EnumBorrow::Name("hi"), true),
            (6u64, EnumBorrow::Code(9), false),
        ];
        let body = aos::encode_rows_u64_enum_bool(&rows);
        let view = view_aos_u64_enum_bool(&body).expect("view");
        assert_eq!(view.id(0), 5);
        match view.payload(0).expect("payload") {
            AosEnumRef::Name(name) => assert_eq!(name, "hi"),
            AosEnumRef::Code(_) => panic!("expected name payload"),
        }
        assert!(view.flag(0));
        assert_eq!(view.id(1), 6);
        match view.payload(1).expect("payload") {
            AosEnumRef::Code(code) => assert_eq!(code, 9),
            AosEnumRef::Name(_) => panic!("expected code payload"),
        }
        assert!(!view.flag(1));
    }
    reset_decode_state();
}
