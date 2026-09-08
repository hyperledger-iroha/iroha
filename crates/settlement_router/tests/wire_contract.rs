//! Captured settlement frame identities, scalar bytes and validation boundaries.
use norito::{NoritoSchema, json::Value};
use settlement_router::{
    BufferCapacity, BufferPolicy, CollateralKind, DurationSeconds, EpsilonBps, HaircutTier,
    LiquidityProfile, SettlementConfig, SettlementReceipt, ShadowPrice, SwapLineConfig,
    SwapLineExposure, SwapLineId, TimestampMs, VolatilityBucket,
};

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(result, "{byte:02x}").expect("write hexadecimal");
    }
    result
}

fn unhex(value: &Value) -> Vec<u8> {
    let text = value.as_str().expect("hexadecimal fixture");
    assert_eq!(text.len() % 2, 0);
    text.as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let pair = std::str::from_utf8(pair).expect("ASCII hexadecimal");
            u8::from_str_radix(pair, 16).expect("hexadecimal byte")
        })
        .collect()
}

fn identity<T: NoritoSchema>() -> (String, String, String) {
    (
        T::nominal_name(),
        T::frame_name(),
        hex(&norito::schema::identity::frame_hash::<T>()),
    )
}

#[test]
fn settlement_owners_match_captured_serializer_and_decoder_identities() {
    let fixture: Value = norito::json::from_str(include_str!("fixtures/schema_identities.v1.json"))
        .expect("identity fixture");
    let rows = fixture["identities"].as_array().expect("identity rows");
    let actual = [
        identity::<TimestampMs>(),
        identity::<DurationSeconds>(),
        identity::<EpsilonBps>(),
        identity::<SettlementConfig>(),
        identity::<HaircutTier>(),
        identity::<LiquidityProfile>(),
        identity::<BufferCapacity>(),
        identity::<BufferPolicy>(),
        identity::<ShadowPrice>(),
        identity::<SettlementReceipt>(),
        identity::<CollateralKind>(),
        identity::<SwapLineConfig>(),
        identity::<SwapLineExposure>(),
        identity::<SwapLineId>(),
        identity::<VolatilityBucket>(),
    ];
    assert_eq!(rows.len(), actual.len());
    for (nominal, frame, digest) in actual {
        let matches: Vec<_> = rows
            .iter()
            .filter(|row| row["nominal"].as_str() == Some(nominal.as_str()))
            .collect();
        assert_eq!(matches.len(), 1, "unique identity {nominal}");
        assert_eq!(matches[0]["frame"].as_str(), Some(frame.as_str()));
        assert_eq!(matches[0]["schema_hash"].as_str(), Some(digest.as_str()));
    }
}

fn assert_scalar_frame<T>(value: T, fixture: &Value)
where
    T: NoritoSchema
        + norito::SerializePayload
        + for<'de> norito::DeserializePayload<'de>
        + Copy
        + Eq
        + std::fmt::Debug,
{
    let frame = unhex(&fixture["frame_hex"]);
    assert_eq!(norito::to_bytes(&value).expect("encode scalar"), frame);
    assert_eq!(norito::decode_from_bytes::<T>(&frame).unwrap(), value);
    assert_eq!(
        norito::to_bytes(&None::<T>).unwrap(),
        unhex(&fixture["option_none_hex"])
    );
    let some = unhex(&fixture["option_some_hex"]);
    assert_eq!(norito::to_bytes(&Some(value)).unwrap(), some);
    assert_eq!(
        norito::decode_from_bytes::<Option<T>>(&some).unwrap(),
        Some(value)
    );
    assert_eq!(
        norito::decode_from_bytes::<Option<T>>(&unhex(&fixture["option_none_hex"])).unwrap(),
        None
    );
    assert_eq!(
        norito::to_bytes(&Vec::<T>::new()).unwrap(),
        unhex(&fixture["vec_empty_hex"])
    );
    assert!(
        norito::decode_from_bytes::<Vec<T>>(&unhex(&fixture["vec_empty_hex"]))
            .unwrap()
            .is_empty()
    );
    let two = unhex(&fixture["vec_two_hex"]);
    assert_eq!(norito::to_bytes(&vec![value, value]).unwrap(), two);
    assert_eq!(
        norito::decode_from_bytes::<Vec<T>>(&two).unwrap(),
        [value, value]
    );
    for end in 0..frame.len() {
        assert!(norito::decode_from_bytes::<T>(&frame[..end]).is_err());
    }
    let mut wrong_identity = frame;
    wrong_identity[6] ^= 1;
    assert!(matches!(
        norito::decode_from_bytes::<T>(&wrong_identity),
        Err(norito::Error::SchemaMismatch)
    ));
}

#[test]
fn scalar_frames_preserve_root_and_composed_golden_bytes() {
    let fixture: Value = norito::json::from_str(include_str!("fixtures/scalar_frames.v1.json"))
        .expect("scalar fixture");
    assert_eq!(
        fixture["layout_flags"].as_u64(),
        Some(u64::from(norito::core::default_encode_flags()))
    );
    assert_scalar_frame(
        TimestampMs::from_unix_millis(1_700_000_000_000).unwrap(),
        &fixture["timestamp_ms"],
    );
    assert_scalar_frame(
        DurationSeconds::new(time::Duration::seconds(-37)),
        &fixture["duration_seconds"],
    );
}

#[test]
fn scalar_payloads_preserve_bounds_and_prefix_consumption() {
    use norito::core::DecodeFromSlice;
    let mut bytes = 1_700_000_000_000_u64.to_le_bytes().to_vec();
    for end in 0..8 {
        assert!(TimestampMs::decode_from_slice(&bytes[..end]).is_err());
        assert!(DurationSeconds::decode_from_slice(&bytes[..end]).is_err());
    }
    bytes.extend_from_slice(&[0xa5, 0x5a]);
    let (timestamp, used) = TimestampMs::decode_from_slice(&bytes).unwrap();
    assert_eq!(timestamp.as_unix_millis(), 1_700_000_000_000);
    assert_eq!(used, 8);
    assert_eq!(&bytes[used..], &[0xa5, 0x5a]);
    assert!(norito::codec::decode_exact_from_slice::<TimestampMs>(&bytes).is_err());
    assert!(TimestampMs::decode_from_slice(&u64::MAX.to_le_bytes()).is_err());
    assert!(norito::json::from_str::<TimestampMs>(&u64::MAX.to_string()).is_err());
    for seconds in [i64::MIN, -1, 0, 1, i64::MAX] {
        let expected = DurationSeconds::new(time::Duration::seconds(seconds));
        let (decoded, used) = DurationSeconds::decode_from_slice(&seconds.to_le_bytes()).unwrap();
        assert_eq!(used, 8);
        assert_eq!(decoded, expected);
        let frame = norito::to_bytes(&expected).unwrap();
        assert_eq!(
            norito::decode_from_bytes::<DurationSeconds>(&frame).unwrap(),
            expected
        );
    }
}
