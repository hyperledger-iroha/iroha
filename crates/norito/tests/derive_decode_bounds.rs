use norito::{Error, codec::Decode as NoritoDecode};
use std::io::Cursor;
#[cfg_attr(feature = "schema-structural", derive(iroha_schema::IntoSchema))]
#[derive(norito::derive::Encode, norito::derive::Decode)]
struct Wrapper {
    value: String,
}
#[cfg_attr(feature = "schema-structural", derive(iroha_schema::IntoSchema))]
#[derive(norito::derive::Encode, norito::derive::Decode)]
struct Dual {
    first: String,
    second: String,
}
#[cfg_attr(feature = "schema-structural", derive(iroha_schema::IntoSchema))]
#[derive(norito::derive::Encode, norito::derive::Decode)]
struct TupleDual(String, String);
#[cfg_attr(feature = "schema-structural", derive(iroha_schema::IntoSchema))]
#[derive(Debug, PartialEq, Eq, norito::derive::Encode, norito::derive::Decode)]
struct EnumField(u32);
#[cfg_attr(feature = "schema-structural", derive(iroha_schema::TypeId))]
#[derive(Debug, PartialEq, Eq, norito::derive::Encode, norito::derive::Decode)]
enum DerivedTupleEnum {
    Pair(EnumField, u32),
    Boundary(EnumField, #[norito(skip)] ()),
}
#[cfg(feature = "schema-structural")]
#[derive(iroha_schema::IntoSchema)]
#[allow(dead_code)]
struct DerivedTupleEnumPairSchema(EnumField, u32);

#[cfg(feature = "schema-structural")]
impl iroha_schema::IntoSchema for DerivedTupleEnum {
    fn type_name() -> String {
        "DerivedTupleEnum".to_owned()
    }

    fn update_schema_map(map: &mut iroha_schema::MetaMap) {
        if map.contains_key::<Self>() {
            return;
        }
        map.insert::<Self>(iroha_schema::Metadata::Enum(iroha_schema::EnumMeta {
            variants: vec![
                iroha_schema::EnumVariant {
                    tag: "Pair".to_owned(),
                    discriminant: 0,
                    ty: Some(core::any::TypeId::of::<DerivedTupleEnumPairSchema>()),
                },
                iroha_schema::EnumVariant {
                    tag: "Boundary".to_owned(),
                    discriminant: 1,
                    ty: Some(core::any::TypeId::of::<EnumField>()),
                },
            ],
        }));
        <DerivedTupleEnumPairSchema as iroha_schema::IntoSchema>::update_schema_map(map);
        <EnumField as iroha_schema::IntoSchema>::update_schema_map(map);
    }
}
#[cfg_attr(feature = "schema-structural", derive(iroha_schema::IntoSchema))]
#[derive(Debug, PartialEq, Eq)]
struct LooseScalar(u32);
impl norito::NoritoSerialize for LooseScalar {
    fn serialize(&self, encoder: &mut norito::core::Encoder<'_>) -> Result<(), Error> {
        norito::NoritoSerialize::serialize(&self.0, encoder)
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        norito::NoritoSerialize::encoded_len_hint(&self.0)
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        norito::NoritoSerialize::encoded_len_exact(&self.0)
    }
}
impl<'a> norito::NoritoDeserialize<'a> for LooseScalar {
    fn deserialize(archived: &'a norito::core::Archived<Self>) -> Self {
        Self(<u32 as norito::NoritoDeserialize>::deserialize(
            archived.cast::<u32>(),
        ))
    }
    fn try_deserialize(archived: &'a norito::core::Archived<Self>) -> Result<Self, Error> {
        <u32 as norito::NoritoDeserialize>::try_deserialize(archived.cast::<u32>()).map(Self)
    }
}
#[cfg_attr(feature = "schema-structural", derive(iroha_schema::IntoSchema))]
#[derive(Debug, PartialEq, Eq, norito::derive::Encode, norito::derive::Decode)]
enum LooseTupleEnum {
    Value(LooseScalar),
}
#[test]
fn derive_decode_rejects_overlong_field() {
    // Craft a payload where the field length claims more bytes than are available.
    let mut payload = Vec::new();
    payload.extend_from_slice(&u64::MAX.to_le_bytes());
    payload.extend_from_slice(b"abc");
    let mut cursor = Cursor::new(payload);
    let decoded = Wrapper::decode(&mut cursor);
    match decoded {
        Err(Error::LengthMismatch) => {}
        Err(err) => panic!("unexpected decode error: {err:?}"),
        Ok(_) => panic!("expected decode failure for truncated payload"),
    }
}
#[test]
fn derive_decode_rejects_truncated_second_field() {
    // Encoded layout for `Dual { first, second }` with default len headers (8-byte LE).
    // first = "abc" (len=3), second declares len=10 but only provides 2 bytes.
    let mut payload = Vec::new();
    payload.extend_from_slice(&(3u64).to_le_bytes());
    payload.extend_from_slice(b"abc");
    payload.extend_from_slice(&(10u64).to_le_bytes());
    payload.extend_from_slice(b"xy"); // insufficient bytes for declared length
    let mut cursor = Cursor::new(payload);
    let decoded = Dual::decode(&mut cursor);
    match decoded {
        Err(Error::LengthMismatch) => {}
        Err(err) => panic!("unexpected decode error: {err:?}"),
        Ok(_) => panic!("expected decode failure for truncated second field"),
    }
}
#[test]
fn tuple_decode_rejects_truncated_second_field() {
    // Same scenario as above but exercising the tuple-field decode path.
    let mut payload = Vec::new();
    payload.extend_from_slice(&(1u64).to_le_bytes());
    payload.extend_from_slice(b"x");
    payload.extend_from_slice(&(5u64).to_le_bytes());
    payload.extend_from_slice(b"yz"); // missing 3 bytes
    let mut cursor = Cursor::new(payload);
    let decoded = TupleDual::decode(&mut cursor);
    match decoded {
        Err(Error::LengthMismatch) => {}
        Err(err) => panic!("unexpected decode error: {err:?}"),
        Ok(_) => panic!("expected decode failure for tuple second field"),
    }
}
#[test]
fn derived_multi_field_tuple_enum_canonical_roundtrips() {
    let value = DerivedTupleEnum::Pair(EnumField(0x1122_3344), 0x5566_7788);
    let frame = norito::encode_canonical(&value).expect("encode canonical tuple enum");
    let decoded: DerivedTupleEnum =
        norito::decode_canonical(&frame).expect("decode canonical tuple enum");
    assert_eq!(decoded, value);
}
#[test]
fn derived_tuple_enum_rejects_understated_first_field_length() {
    let value = DerivedTupleEnum::Boundary(EnumField(0x1122_3344), ());
    let frame = norito::encode_canonical(&value).expect("encode canonical tuple enum");
    let view = norito::core::from_bytes_view(&frame).expect("inspect canonical frame");
    let flags = view.flags();
    let mut payload = view.as_bytes().to_vec();
    assert_eq!(
        norito::core::NoritoSerialize::encoded_len_exact(&value),
        Some(payload.len()),
        "skipped enum fields must not contribute to the exact payload length"
    );
    let first_field_prefix = 4;
    let (declared, prefix_len) =
        norito::core::read_len_from_slice_with_flags(&payload[first_field_prefix..], flags)
            .expect("read first field length");
    assert_eq!(declared, core::mem::size_of::<u32>() + prefix_len);
    assert_eq!(prefix_len, 1);
    payload[first_field_prefix] = u8::try_from(declared - 1).expect("shortened field length");
    let forged = norito::core::frame_bare_with_header_flags::<DerivedTupleEnum>(&payload, flags)
        .expect("frame corrupted tuple enum");
    assert!(matches!(
        norito::decode_canonical::<DerivedTupleEnum>(&forged),
        Err(Error::LengthMismatch)
    ));
}

#[test]
fn derived_tuple_enum_rejects_trailing_bytes_inside_a_field_frame() {
    let value = LooseTupleEnum::Value(LooseScalar(0x1122_3344));
    let frame = norito::encode_canonical(&value).expect("encode canonical tuple enum");
    let view = norito::core::from_bytes_view(&frame).expect("inspect canonical frame");
    let flags = view.flags();
    let mut payload = view.as_bytes().to_vec();
    let field_prefix = 4;
    let (declared, prefix_len) =
        norito::core::read_len_from_slice_with_flags(&payload[field_prefix..], flags)
            .expect("read tuple field length");
    assert_eq!(
        prefix_len, 1,
        "test fixture expects a one-byte canonical length"
    );
    payload[field_prefix] = u8::try_from(declared + 1).expect("extend tuple field length");
    payload.push(0xA5);
    let forged = norito::core::frame_bare_with_header_flags::<LooseTupleEnum>(&payload, flags)
        .expect("frame corrupted tuple enum");

    assert!(matches!(
        norito::decode_canonical::<LooseTupleEnum>(&forged),
        Err(Error::LengthMismatch)
    ));
}
