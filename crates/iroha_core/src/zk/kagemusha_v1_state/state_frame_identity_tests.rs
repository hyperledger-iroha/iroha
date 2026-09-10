//! Original compiler identities and payload accounting for state-owned records.
//!
//! The captured fixture records names and hashes, not historical complete frame bytes.
//! Codec roundtrips do not authenticate proof, hardware, or recovery authority.

use norito::{core::NoritoSerialize, json::Value};

pub(super) fn observed<T: NoritoSerialize>(nominal: &str) {
    let rows: Vec<Value> = norito::json::from_slice(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/core/kagemusha_state_frame_identity_observations.v1.json"
    )))
    .expect("original compiler observations");
    assert_eq!(rows.len(), 64);
    let mut directions = std::collections::BTreeSet::new();
    for row in rows
        .iter()
        .filter(|row| row.get("nominal").and_then(Value::as_str) == Some(nominal))
    {
        let field = |key| row.get(key).and_then(Value::as_str).expect("captured text");
        assert!(directions.insert(field("direction")));
        assert_eq!(T::nominal_name(), nominal);
        assert_eq!(T::frame_name(), field("root_hint"));
        let expected: Vec<u8> = field("schema_hash")
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        assert_eq!(
            norito::schema::identity::frame_hash::<T>().as_slice(),
            expected
        );
    }
    assert!(directions.contains("serialize"));
    assert!(directions.len() <= 2);
    let frame = norito::encode_canonical(&Option::<T>::None).unwrap();
    let view = norito::core::from_bytes_view(&frame).unwrap();
    assert_eq!(
        view.schema(),
        norito::schema::identity::frame_hash::<Option<T>>()
    );
}

pub(super) fn roundtrip<T>(value: &T)
where
    T: NoritoSerialize + for<'de> norito::core::NoritoDeserialize<'de>,
{
    let frame = norito::encode_canonical(value).unwrap();
    let value: T = norito::decode_canonical(&frame).unwrap();
    assert_eq!(norito::encode_canonical(&value).unwrap(), frame);
    let mut wrong_root = frame.clone();
    wrong_root[6] ^= 1;
    assert!(norito::decode_canonical::<T>(&wrong_root).is_err());
    let mut trailing = frame.clone();
    trailing.push(0);
    assert!(norito::decode_canonical::<T>(&trailing).is_err());
    assert!(norito::decode_canonical::<T>(&frame[..frame.len() - 1]).is_err());
}

#[test]
fn receiver_capacity_accepts_payload_only_records_and_restores_flags() {
    #[derive(norito::Encode)]
    struct PayloadOnly {
        sequence: u64,
        retained: Vec<u8>,
    }
    let flags = norito::core::default_encode_flags();
    let ambient = flags ^ norito::core::header_flags::COMPACT_LEN;
    let _ambient = norito::core::DecodeFlagsGuard::enter(ambient);
    for size in [0, 1, 127, 128, 1024] {
        let value = PayloadOnly {
            sequence: u64::MAX,
            retained: vec![7; size],
        };
        let expected = {
            let _canonical = norito::core::DecodeFlagsGuard::enter(flags);
            let mut payload = Vec::new();
            norito::core::serialize_to_writer(&value, &mut payload).unwrap();
            let mut prefix = Vec::new();
            if norito::core::packed_seq_enabled_for_flags(flags) {
                prefix.extend_from_slice(&(payload.len() as u64).to_le_bytes());
            } else {
                norito::core::write_len_with_flags(&mut prefix, payload.len() as u64, flags)
                    .unwrap();
            }
            (prefix.len() + payload.len()) as u64
        };
        assert_eq!(
            super::receiver_sequence_entry_bytes(&value).unwrap(),
            expected
        );
        assert_eq!(norito::core::get_decode_flags(), ambient);
    }
}

#[test]
fn receiver_capacity_propagates_payload_failure_and_restores_flags() {
    struct FailedPayload;
    impl norito::core::SerializePayload for FailedPayload {
        fn serialize(&self, _: &mut norito::core::Encoder<'_>) -> Result<(), norito::Error> {
            Err(norito::Error::LengthMismatch)
        }
    }
    let ambient = norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
    let _ambient = norito::core::DecodeFlagsGuard::enter(ambient);
    assert!(matches!(
        super::receiver_sequence_entry_bytes(&FailedPayload),
        Err(super::KagemushaStateErrorV1::CanonicalEncoding)
    ));
    assert_eq!(norito::core::get_decode_flags(), ambient);
}

#[test]
fn state_capacity_and_empty_mint_journal_roundtrip() {
    roundtrip(&super::KagemushaReceiverInboxCapacityV1::new(1_048_576));
    roundtrip(&super::KagemushaSenderOutboxCapacityV1::new(1_048_576));
    roundtrip(&super::KagemushaMintInboxV1::default());
}
