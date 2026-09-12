//! Direct fallible `IdBox` decoding rejects malformed tags after frame authentication.

use iroha_data_model::id::IdBox;
use iroha_model_base::topology::LaneId;
use norito::{SerializePayload as _, core as ncore};

fn layouts() -> impl Iterator<Item = u8> {
    (0..=ncore::supported_header_flags())
        .filter(|flags| ncore::validate_header_flags(*flags).is_ok())
}

fn payload(value: &IdBox, flags: u8) -> Vec<u8> {
    let _flags = ncore::DecodeFlagsGuard::enter(flags);
    let mut bytes = Vec::new();
    value
        .serialize(&mut ncore::Encoder::for_buffer(&mut bytes))
        .expect("serialize the valid IdBox baseline");
    bytes
}

fn try_decode_authenticated_payload(bytes: &[u8], flags: u8) -> Result<IdBox, ncore::Error> {
    let frame = ncore::frame_bare_with_header_flags::<IdBox>(bytes, flags)
        .expect("frame the exact payload with its own length and checksum");
    let view = ncore::from_bytes_view(&frame).expect("authenticate the complete frame");
    assert_eq!(view.as_bytes(), bytes);
    assert_eq!(view.flags(), flags);
    let _flags = ncore::DecodeFlagsGuard::enter(flags);
    let _payload = ncore::PayloadCtxGuard::enter_with_schema_and_flags(bytes, view.schema(), flags);
    let archived = ncore::from_bytes::<IdBox>(&frame)
        .expect("header, schema, checksum, length and alignment must be valid");
    // Deliberately call the owner's fallible method directly. A panic is a test
    // failure; a higher-level decoder must not mask an infallible delegation.
    <IdBox as ncore::DeserializePayload<'_>>::try_deserialize(archived)
}

#[test]
fn id_box_try_deserialize_rejects_unknown_tag_without_panicking() {
    let value = IdBox::LaneId(LaneId::new(7));
    for flags in layouts() {
        let valid = payload(&value, flags);
        assert_eq!(&valid[..4], &7_u32.to_le_bytes());
        assert_eq!(
            try_decode_authenticated_payload(&valid, flags).unwrap(),
            value
        );
        let mut unknown = valid.clone();
        unknown[..4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_ne!(unknown, valid);
        let error = try_decode_authenticated_payload(&unknown, flags)
            .expect_err("unknown IdBox discriminant must be a typed rejection");
        assert!(
            matches!(error, ncore::Error::Message(ref message) if message == "invalid enum discriminant"),
            "flags {flags:#04x}: unexpected unknown-tag error: {error:?}",
        );
        assert_eq!(
            try_decode_authenticated_payload(&valid, flags).unwrap(),
            value
        );
    }
}

#[test]
fn id_box_try_deserialize_rejects_truncated_tag_without_panicking() {
    let value = IdBox::LaneId(LaneId::new(7));
    for flags in layouts() {
        let valid = payload(&value, flags);
        assert_eq!(&valid[..4], &7_u32.to_le_bytes());
        assert_eq!(
            try_decode_authenticated_payload(&valid, flags).unwrap(),
            value
        );
        for tag_len in 0..4 {
            let truncated = &valid[..tag_len];
            assert!(truncated.len() < valid.len());
            let error = try_decode_authenticated_payload(truncated, flags)
                .expect_err("incomplete IdBox discriminant must be a typed rejection");
            assert!(
                matches!(error, ncore::Error::LengthMismatch),
                "flags {flags:#04x}, tag bytes {tag_len}: unexpected error: {error:?}",
            );
            assert_eq!(
                try_decode_authenticated_payload(&valid, flags).unwrap(),
                value
            );
        }
    }
}
