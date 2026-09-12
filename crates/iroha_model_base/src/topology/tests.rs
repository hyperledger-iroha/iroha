//! Numeric topology roundtrip, lane bounds and hash-prefix contracts.

use super::*;
use norito::codec::{DecodeAll, Encode};

#[test]
fn lane_id_roundtrip() {
    let original = LaneId::new(42);
    let bytes = Encode::encode(&original);
    let mut slice: &[u8] = &bytes;
    let decoded = LaneId::decode_all(&mut slice).expect("decode LaneId");
    assert_eq!(decoded, original);
    assert_eq!(LaneId::SINGLE.as_u32(), 0);
}

#[test]
fn shard_id_roundtrip() {
    let original = ShardId::new(24);
    let bytes = Encode::encode(&original);
    let mut slice: &[u8] = &bytes;
    let decoded = ShardId::decode_all(&mut slice).expect("decode ShardId");
    assert_eq!(decoded, original);
    assert_eq!(ShardId::new(0).as_u32(), 0);
}

#[test]
fn lane_id_from_lane_index_enforces_bounds() {
    let lane_count = NonZeroU32::new(2).expect("nonzero");
    let lane = LaneId::from_lane_index(1, lane_count).expect("valid lane");
    assert_eq!(lane.as_u32(), 1);
    let err = LaneId::from_lane_index(2, lane_count).expect_err("should be out of bounds");
    assert_eq!(
        err,
        LaneIdError::OutOfBounds {
            index: 2,
            lane_count: 2
        }
    );
}

#[test]
fn dataspace_id_roundtrip() {
    let original = DataSpaceId::new(7);
    let bytes = Encode::encode(&original);
    let mut slice: &[u8] = &bytes;
    let decoded = DataSpaceId::decode_all(&mut slice).expect("decode DataSpaceId");
    assert_eq!(decoded, original);
    assert_eq!(DataSpaceId::UNIVERSAL.as_u64(), 0);
    assert_eq!(
        "7".parse::<DataSpaceId>().expect("parse DataSpaceId"),
        original
    );
    assert!("-1".parse::<DataSpaceId>().is_err());
}

#[test]
fn dataspace_id_parses_decimal_cli_form() {
    assert_eq!("0".parse(), Ok(DataSpaceId::UNIVERSAL));
    assert_eq!(u64::MAX.to_string().parse(), Ok(DataSpaceId::new(u64::MAX)));
    assert!("-1".parse::<DataSpaceId>().is_err());
    assert!("not-a-dataspace".parse::<DataSpaceId>().is_err());
}

#[test]
fn dataspace_id_from_hash_uses_low_bytes() {
    let mut hash = [0u8; 32];
    hash[0..8].copy_from_slice(&[0xAB, 0xCD, 0xEF, 0x01, 0x02, 0x03, 0x04, 0x05]);
    let expected = u64::from_le_bytes(hash[..8].try_into().expect("slice length"));
    let id = DataSpaceId::from_hash(&hash);
    assert_eq!(id.as_u64(), expected);
}

#[test]
fn numeric_json_enforces_full_width_unsigned_bounds() {
    let max32 = u32::MAX.to_string();
    assert_eq!(json::from_str::<LaneId>(&max32).unwrap().as_u32(), u32::MAX);
    assert_eq!(
        json::from_str::<ShardId>(&max32).unwrap().as_u32(),
        u32::MAX
    );
    let max64 = u64::MAX.to_string();
    assert_eq!(
        json::from_str::<DataSpaceId>(&max64).unwrap().as_u64(),
        u64::MAX
    );
    for invalid in ["4294967296", "18446744073709551615"] {
        assert!(json::from_str::<LaneId>(invalid).is_err(), "{invalid}");
        assert!(json::from_str::<ShardId>(invalid).is_err(), "{invalid}");
    }
    for invalid in ["-1", "1.5", "null", "[]", "\"1\"", "18446744073709551616"] {
        assert!(json::from_str::<LaneId>(invalid).is_err(), "{invalid}");
        assert!(json::from_str::<ShardId>(invalid).is_err(), "{invalid}");
        assert!(json::from_str::<DataSpaceId>(invalid).is_err(), "{invalid}");
    }
}

#[test]
fn object_and_storage_keys_retain_full_width_values() {
    use norito::json::JsonObjectKeyOwned;

    let lane = LaneId::new(u32::MAX);
    let dataspace = DataSpaceId::new(u64::MAX);
    assert_eq!(LaneId::from_json_key_text("4294967295").unwrap(), lane);
    assert_eq!(LaneId::decode_json_key("4294967295").unwrap(), lane);
    assert_eq!(
        DataSpaceId::from_json_key_text("18446744073709551615").unwrap(),
        dataspace
    );
    assert_eq!(
        DataSpaceId::decode_json_key("18446744073709551615").unwrap(),
        dataspace
    );
    let mut lane_key = String::new();
    lane.encode_json_key(&mut lane_key);
    assert_eq!(lane_key, "\"4294967295\"");
    let mut dataspace_key = String::new();
    dataspace.encode_json_key(&mut dataspace_key);
    assert_eq!(dataspace_key, "\"18446744073709551615\"");
}

#[test]
fn object_and_storage_keys_reject_negative_and_overflow_values() {
    use norito::json::JsonObjectKeyOwned;

    for invalid in [
        "-1",
        "4294967296",
        "18446744073709551615",
        "18446744073709551616",
    ] {
        assert!(LaneId::from_json_key_text(invalid).is_err(), "{invalid}");
        assert!(LaneId::decode_json_key(invalid).is_err(), "{invalid}");
    }
    for invalid in ["-1", "18446744073709551616"] {
        assert!(
            DataSpaceId::from_json_key_text(invalid).is_err(),
            "{invalid}"
        );
        assert!(DataSpaceId::decode_json_key(invalid).is_err(), "{invalid}");
    }
}
