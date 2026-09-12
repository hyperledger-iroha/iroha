//! Manifest path payload codecs preserve String bytes, boundaries, and decode budgets.

use std::path::{Path, PathBuf};

use norito::{
    Error, SerializePayload,
    codec::{Decode, Encode},
    core::{
        DecodeFlagsGuard, DecodeFromSlice, DecodeLimits, Encoder, decode_field_canonical,
        get_decode_flags, header_flags, with_decode_limits_scope,
    },
};

use super::IvmPath;

fn path(value: &str) -> IvmPath {
    IvmPath::from(PathBuf::from(value))
}

fn payload<T: SerializePayload>(value: &T, flags: u8) -> Vec<u8> {
    let _flags = DecodeFlagsGuard::enter(flags);
    let mut bytes = Vec::new();
    value.serialize(&mut Encoder::new(&mut bytes)).unwrap();
    bytes
}

#[test]
fn path_payload_preserves_exact_string_bytes_and_bare_roundtrips() {
    for text in ["", "contracts/bootstrap.to", "contracts/雪 \"quoted\".to"] {
        let value = path(text);
        assert_eq!(value.encode(), text.to_owned().encode());
        assert_eq!(
            IvmPath::decode(&mut value.encode().as_slice())
                .unwrap()
                .as_path(),
            Path::new(text)
        );
        for flags in [0, header_flags::COMPACT_LEN] {
            let _flags = DecodeFlagsGuard::enter(flags);
            let bytes = payload(&value, flags);
            assert_eq!(bytes, payload(&text.to_owned(), flags));
            let (decoded, used) = decode_field_canonical::<IvmPath>(&bytes).unwrap();
            assert_eq!(decoded.as_path(), Path::new(text));
            assert_eq!(used, bytes.len());
            assert_eq!(get_decode_flags(), flags);
        }
    }
}

#[test]
fn path_slice_preserves_prefix_consumption_and_canonical_tail_rejection() {
    let previous = get_decode_flags();
    for flags in [0, header_flags::COMPACT_LEN] {
        let _flags = DecodeFlagsGuard::enter(flags);
        let bytes = payload(&path("contracts/bootstrap.to"), flags);
        let mut extended = bytes.clone();
        extended.extend_from_slice(&[0xa5, 0x5a]);
        let (decoded, used) = IvmPath::decode_from_slice(&extended).unwrap();
        assert_eq!(decoded.as_path(), Path::new("contracts/bootstrap.to"));
        assert_eq!(used, bytes.len());
        assert_eq!(&extended[used..], &[0xa5, 0x5a]);
        assert!(matches!(
            decode_field_canonical::<IvmPath>(&extended),
            Err(Error::LengthMismatch)
        ));
        assert_eq!(get_decode_flags(), flags);
    }
    assert_eq!(get_decode_flags(), previous);

    let mut bare = path("contracts/bootstrap.to").encode();
    bare.push(0);
    assert!(IvmPath::decode(&mut bare.as_slice()).is_err());
}

#[test]
fn path_containers_preserve_string_payloads_in_both_length_layouts() {
    for flags in [
        0,
        header_flags::COMPACT_LEN,
        header_flags::PACKED_SEQ,
        header_flags::COMPACT_LEN | header_flags::PACKED_SEQ,
    ] {
        let _flags = DecodeFlagsGuard::enter(flags);
        let values = vec![path("one.to"), path("two/雪.to")];
        let strings = vec!["one.to".to_owned(), "two/雪.to".to_owned()];
        let bytes = payload(&values, flags);
        assert_eq!(bytes, payload(&strings, flags));
        let (decoded, used) = decode_field_canonical::<Vec<IvmPath>>(&bytes).unwrap();
        assert_eq!(used, bytes.len());
        assert_eq!(
            decoded.iter().map(IvmPath::as_path).collect::<Vec<_>>(),
            vec![Path::new("one.to"), Path::new("two/雪.to")]
        );
        let mut extended = bytes.clone();
        extended.push(0xa5);
        let (decoded, used) = Vec::<IvmPath>::decode_from_slice(&extended).unwrap();
        assert_eq!(decoded.len(), 2);
        assert_eq!(used, bytes.len());
        assert!(decode_field_canonical::<Vec<IvmPath>>(&extended).is_err());

        for text in [None, Some("contracts/bootstrap.to")] {
            let value = text.map(path);
            let bytes = payload(&value, flags);
            assert_eq!(bytes, payload(&text.map(str::to_owned), flags));
            let (decoded, used) = decode_field_canonical::<Option<IvmPath>>(&bytes).unwrap();
            assert_eq!(decoded.as_ref().map(IvmPath::as_path), text.map(Path::new));
            assert_eq!(used, bytes.len());
        }
        let empty = payload(&Vec::<IvmPath>::new(), flags);
        assert_eq!(empty, payload(&Vec::<String>::new(), flags));
        assert!(
            decode_field_canonical::<Vec<IvmPath>>(&empty)
                .unwrap()
                .0
                .is_empty()
        );
        assert_eq!(get_decode_flags(), flags);
    }
}

#[test]
fn path_decoders_return_typed_malformed_errors_without_resetting_layout() {
    for flags in [0, header_flags::COMPACT_LEN] {
        let _flags = DecodeFlagsGuard::enter(flags);
        let valid = payload(&path("abc"), flags);
        for end in 0..valid.len() {
            assert!(IvmPath::decode_from_slice(&valid[..end]).is_err());
            assert!(decode_field_canonical::<IvmPath>(&valid[..end]).is_err());
            assert_eq!(get_decode_flags(), flags);
        }
        let mut invalid_utf8 = valid;
        *invalid_utf8.last_mut().unwrap() = 0xff;
        assert!(matches!(
            IvmPath::decode_from_slice(&invalid_utf8),
            Err(Error::InvalidUtf8)
        ));
        assert!(matches!(
            decode_field_canonical::<IvmPath>(&invalid_utf8),
            Err(Error::InvalidUtf8)
        ));
        assert_eq!(get_decode_flags(), flags);
    }
}

#[test]
fn path_decoding_preserves_enclosing_resource_limits_on_success_and_failure() {
    let previous = get_decode_flags();
    for flags in [0, header_flags::COMPACT_LEN] {
        let _flags = DecodeFlagsGuard::enter(flags);
        let bytes = payload(&path("abc"), flags);
        let outer = DecodeLimits::new(16, 64, 16, 6, 8);
        with_decode_limits_scope(outer, || {
            assert_eq!(
                IvmPath::decode_from_slice(&bytes).unwrap().0.as_path(),
                Path::new("abc")
            );
            let inner = DecodeLimits::new(16, 0, 16, 64, 8);
            with_decode_limits_scope(inner, || {
                assert!(matches!(
                    IvmPath::decode_from_slice(&bytes),
                    Err(Error::FieldLengthExceeded {
                        length: 3,
                        limit: 0
                    })
                ));
            });
            assert_eq!(get_decode_flags(), flags);
            assert_eq!(
                IvmPath::decode_from_slice(&bytes).unwrap().0.as_path(),
                Path::new("abc")
            );
            assert!(matches!(
                IvmPath::decode_from_slice(&bytes),
                Err(Error::TotalAllocationExceeded {
                    attempted: 9,
                    limit: 6
                })
            ));
            assert_eq!(get_decode_flags(), flags);
        });
        assert_eq!(
            IvmPath::decode_from_slice(&bytes).unwrap().0.as_path(),
            Path::new("abc")
        );
        let field_limit = DecodeLimits::new(16, 0, 16, 64, 8);
        with_decode_limits_scope(field_limit, || {
            // Canonical fields check the entire wire slice, including its length prefix.
            let error = decode_field_canonical::<IvmPath>(&bytes).unwrap_err();
            assert!(
                matches!(
                    error,
                    Error::FieldLengthExceeded { length, limit: 0 }
                        if length == u64::try_from(bytes.len()).unwrap()
                ),
                "unexpected canonical field error: {error:?}"
            );
        });
        assert_eq!(
            decode_field_canonical::<IvmPath>(&bytes)
                .unwrap()
                .0
                .as_path(),
            Path::new("abc")
        );
        assert_eq!(get_decode_flags(), flags);
    }
    assert_eq!(get_decode_flags(), previous);
}

#[test]
fn path_json_remains_an_exact_string_with_strict_input_shape() {
    for text in ["", "contracts/bootstrap.to", "contracts/雪 \"quoted\".to"] {
        let encoded = norito::json::to_json(&path(text)).unwrap();
        assert_eq!(encoded, norito::json::to_json(&text).unwrap());
        assert_eq!(
            norito::json::from_str::<IvmPath>(&encoded)
                .unwrap()
                .as_path(),
            Path::new(text)
        );
    }
    for invalid in [
        "null",
        "42",
        "[]",
        "{}",
        "\"unterminated",
        "\"ok.to\" false",
    ] {
        assert!(norito::json::from_str::<IvmPath>(invalid).is_err());
    }
    assert_eq!(IvmPath::default().as_path(), Path::new("."));
}

fn action_fixture() -> super::GenesisIvmAction {
    use iroha_data_model::events::execute_trigger::ExecuteTriggerEventFilter;

    use super::Repeats;

    super::GenesisIvmAction::new(
        "contracts/bootstrap.to",
        Repeats::Exactly(2),
        iroha_test_samples::ALICE_ID.clone(),
        ExecuteTriggerEventFilter::new(),
    )
}

fn trigger_fixture() -> super::GenesisIvmTrigger {
    super::GenesisIvmTrigger::new("bootstrap".parse().unwrap(), action_fixture())
}

fn record_prefix<T>(value: &T, flags: u8) -> T
where
    T: SerializePayload + for<'de> norito::DeserializePayload<'de> + for<'de> DecodeFromSlice<'de>,
{
    let bytes = payload(value, flags);
    let mut extended = bytes.clone();
    extended.extend_from_slice(&[0xa5, 0x5a]);
    let (decoded, used) = T::decode_from_slice(&extended).unwrap();
    assert_eq!(used, bytes.len());
    assert_eq!(&extended[used..], &[0xa5, 0x5a]);
    assert_eq!(payload(&decoded, flags), bytes);
    let (canonical, canonical_used) = decode_field_canonical::<T>(&bytes).unwrap();
    assert_eq!(canonical_used, bytes.len());
    assert_eq!(payload(&canonical, flags), bytes);
    assert!(matches!(
        decode_field_canonical::<T>(&extended),
        Err(Error::LengthMismatch)
    ));
    for end in 0..bytes.len() {
        assert!(T::decode_from_slice(&bytes[..end]).is_err());
        assert_eq!(get_decode_flags(), flags);
    }
    assert_eq!(get_decode_flags(), flags);
    decoded
}

#[test]
fn genesis_parent_records_decode_prefixes_with_their_advertised_layout() {
    for flags in (0..=norito::core::supported_header_flags())
        .filter(|flags| norito::core::validate_header_flags(*flags).is_ok())
    {
        let _flags = DecodeFlagsGuard::enter(flags);
        let empty = record_prefix(&super::RawGenesisTx::default(), flags);
        assert!(empty.parameters.is_none());
        assert!(empty.instructions.is_empty());
        assert!(empty.ivm_triggers.is_empty());
        assert!(empty.topology.is_empty());

        let action = record_prefix(&action_fixture(), flags);
        assert_eq!(
            action.executable.as_path(),
            Path::new("contracts/bootstrap.to")
        );
        assert_eq!(action.authority, *iroha_test_samples::ALICE_ID);
        assert_eq!(action.repeats, action_fixture().repeats);
        assert_eq!(action.filter, action_fixture().filter);

        let trigger = record_prefix(&trigger_fixture(), flags);
        assert_eq!(trigger.id, "bootstrap".parse::<super::TriggerId>().unwrap());
        assert_eq!(
            trigger.action.executable.as_path(),
            action.executable.as_path()
        );
        assert_eq!(trigger.action.authority, action.authority);
    }
}

#[test]
fn genesis_parent_containers_preserve_nested_path_values_and_prefixes() {
    for flags in (0..=norito::core::supported_header_flags())
        .filter(|flags| norito::core::validate_header_flags(*flags).is_ok())
    {
        let _flags = DecodeFlagsGuard::enter(flags);
        let transaction = super::RawGenesisTx {
            ivm_triggers: vec![trigger_fixture(), trigger_fixture()],
            ..super::RawGenesisTx::default()
        };
        let decoded = record_prefix(&transaction, flags);
        assert_eq!(decoded.ivm_triggers.len(), 2);
        for trigger in &decoded.ivm_triggers {
            assert_eq!(trigger.id, "bootstrap".parse::<super::TriggerId>().unwrap());
            assert_eq!(trigger.action.authority, *iroha_test_samples::ALICE_ID);
            assert_eq!(
                trigger.action.executable.as_path(),
                Path::new("contracts/bootstrap.to")
            );
        }
        let records = vec![super::RawGenesisTx::default(), transaction];
        let decoded = record_prefix(&records, flags);
        assert_eq!(decoded.len(), 2);
        assert!(decoded[0].ivm_triggers.is_empty());
        assert_eq!(decoded[1].ivm_triggers.len(), 2);
        let optional = record_prefix(&Some(action_fixture()), flags);
        assert_eq!(
            optional.unwrap().executable.as_path(),
            Path::new("contracts/bootstrap.to")
        );
    }
}

#[test]
fn genesis_parent_slice_decoders_inherit_limits_and_restore_caller_state() {
    let previous = get_decode_flags();
    for flags in [
        0,
        header_flags::COMPACT_LEN,
        header_flags::PACKED_STRUCT,
        header_flags::PACKED_STRUCT | header_flags::COMPACT_LEN,
    ] {
        let _flags = DecodeFlagsGuard::enter(flags);
        let value = trigger_fixture();
        let bytes = payload(&value, flags);
        let outer = DecodeLimits::new(128, 4096, 4096, 65536, 32);
        with_decode_limits_scope(outer, || {
            let deny_allocations = DecodeLimits::new(128, 4096, 4096, 0, 32);
            with_decode_limits_scope(deny_allocations, || {
                assert!(matches!(
                    super::GenesisIvmTrigger::decode_from_slice(&bytes),
                    Err(Error::TotalAllocationExceeded { .. })
                ));
                assert_eq!(get_decode_flags(), flags);
            });
            let (decoded, used) = super::GenesisIvmTrigger::decode_from_slice(&bytes).unwrap();
            assert_eq!(used, bytes.len());
            assert_eq!(payload(&decoded, flags), bytes);
            let deny_fields = DecodeLimits::new(128, 0, 4096, 65536, 32);
            with_decode_limits_scope(deny_fields, || {
                assert!(matches!(
                    super::GenesisIvmTrigger::decode_from_slice(&bytes),
                    Err(Error::FieldLengthExceeded { .. })
                ));
                assert_eq!(get_decode_flags(), flags);
            });
            assert!(super::GenesisIvmTrigger::decode_from_slice(&bytes).is_ok());
        });
        assert!(super::GenesisIvmTrigger::decode_from_slice(&bytes).is_ok());
        assert_eq!(get_decode_flags(), flags);
    }
    assert_eq!(get_decode_flags(), previous);
}
