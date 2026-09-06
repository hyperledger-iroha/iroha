//! Pre-cutover frame fixtures for primitive wrappers and their private wire views.

use crate::{
    bigint::BigInt,
    const_vec::ConstVec,
    conststr::ConstString,
    json::Json,
    numeric::{Numeric, NumericSpec, Quantity, XorQuantity},
    numeric_abi::{
        DECIMAL_SCHEMA_NAME_V1, DecimalValueV1, INT_SCHEMA_NAME_V1, IntValueV1,
        QUANTITY_SCHEMA_NAME_V1, QuantityValueV1,
    },
    small::{SmallStr, SmallVec},
    unique_vec::UniqueVec,
};
use norito::{NoritoDeserialize, NoritoSchema, NoritoSerialize, json};

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(crate) fn record_encode<T: NoritoSchema + NoritoSerialize>(
    value: &T,
    frame_name: &str,
) -> json::Value {
    assert_eq!(T::nominal_name(), std::any::type_name::<T>());
    assert_eq!(T::frame_name(), frame_name);
    assert_eq!(
        norito::schema::identity::frame_hash::<T>(),
        <T as NoritoSerialize>::schema_hash()
    );
    let frame = norito::to_bytes(value).unwrap();
    assert_eq!(frame[6..22], <T as NoritoSerialize>::schema_hash());
    norito::json!({
        "nominal": (std::any::type_name::<T>()),
        "frame_name": frame_name,
        "serialize_hash": (hex(&<T as NoritoSerialize>::schema_hash())),
        "deserialize_hash": (json::Value::Null),
        "frame_hex": (hex(&frame)),
    })
}

pub(crate) fn record<T: NoritoSchema + NoritoSerialize + for<'a> NoritoDeserialize<'a>>(
    value: T,
    frame_name: &str,
) -> json::Value {
    assert_eq!(
        norito::schema::identity::frame_hash::<T>(),
        <T as NoritoDeserialize>::schema_hash()
    );
    let mut record = record_encode(&value, frame_name);
    let frame = norito::to_bytes(&value).unwrap();
    let decoded: T = norito::decode_from_bytes(&frame).unwrap();
    assert_eq!(norito::to_bytes(&decoded).unwrap(), frame);
    record.as_object_mut().unwrap().insert(
        "deserialize_hash".to_owned(),
        json::Value::from(hex(&<T as NoritoDeserialize>::schema_hash())),
    );
    record
}

pub(crate) fn record_nominal<T: NoritoSchema + NoritoSerialize + for<'a> NoritoDeserialize<'a>>(
    value: T,
) -> json::Value {
    record(value, std::any::type_name::<T>())
}

fn current_frames() -> Vec<json::Value> {
    let mut records = vec![
        record_nominal(ConstVec::from(vec![7_u32, 11])),
        record_nominal([7_u32, 11, 7].into_iter().collect::<UniqueVec<_>>()),
        record_nominal(SmallStr::from_str("schema")),
        record_nominal(SmallVec::<[u32; 2]>::from(vec![7, 11])),
        record_nominal(SmallVec::<[u32; 8]>::from(vec![7, 11])),
        record_nominal(ConstString::from("schema")),
        record_nominal(Json::new(norito::json!({"a": 7, "b": [true, false]}))),
        record_nominal(BigInt::from_i128(-129)),
        record_nominal(Numeric::new(-129_i64, 2)),
        record_nominal(Quantity::try_from_numeric(Numeric::new(129_i64, 2)).unwrap()),
        record_nominal("1.29".parse::<XorQuantity>().unwrap()),
        record_nominal(NumericSpec::try_fractional(2).unwrap()),
        record(
            IntValueV1::try_new(BigInt::from_i128(-129)).unwrap(),
            INT_SCHEMA_NAME_V1,
        ),
        record(
            DecimalValueV1::new(Numeric::new(-129_i64, 2)),
            DECIMAL_SCHEMA_NAME_V1,
        ),
        record(
            QuantityValueV1::new("1.29".parse().unwrap()),
            QUANTITY_SCHEMA_NAME_V1,
        ),
        record_nominal(vec![SmallStr::from_str("schema")]),
        record_nominal(Some(DecimalValueV1::new(Numeric::new(-129_i64, 2)))),
        record_nominal(vec![ConstVec::from(vec![7_u32, 11])]),
    ];
    records.extend(crate::json::schema_identity::records());
    records.extend(crate::numeric::schema_identity::records());
    records
}

fn current_abi_frames() -> Vec<json::Value> {
    let integer = IntValueV1::try_new(BigInt::from_i128(-129)).unwrap();
    let decimal = DecimalValueV1::new(Numeric::new(-129_i64, 2));
    let quantity = QuantityValueV1::new("1.29".parse().unwrap());
    let frames = [
        integer.encode_frame().unwrap(),
        decimal.encode_frame().unwrap(),
        quantity.encode_frame().unwrap(),
    ];
    assert_eq!(IntValueV1::decode_frame(&frames[0]).unwrap(), integer);
    assert_eq!(DecimalValueV1::decode_frame(&frames[1]).unwrap(), decimal);
    assert_eq!(QuantityValueV1::decode_frame(&frames[2]).unwrap(), quantity);
    frames
        .into_iter()
        .zip([
            INT_SCHEMA_NAME_V1,
            DECIMAL_SCHEMA_NAME_V1,
            QUANTITY_SCHEMA_NAME_V1,
        ])
        .map(|(frame, name)| {
            assert_eq!(frame[39], 0, "numeric ABI layout flags");
            norito::json!({"frame_name": name, "frame_hex": (hex(&frame))})
        })
        .collect()
}

#[test]
fn primitive_schema_identity_frames_match_pre_declaration_goldens() {
    let expected: Vec<json::Value> = json::from_str(include_str!(
        "../tests/fixtures/schema_identity_frames.json"
    ))
    .unwrap();
    assert_eq!(current_frames(), expected);
}

#[test]
fn numeric_abi_schema_identity_frames_keep_fixed_layout_and_reject_other_domains() {
    let expected: Vec<json::Value> = json::from_str(include_str!(
        "../tests/fixtures/schema_identity_numeric_abi_frames.json"
    ))
    .unwrap();
    assert_eq!(current_abi_frames(), expected);
    let frame = DecimalValueV1::new(Numeric::new(129_i64, 2))
        .encode_frame()
        .unwrap();
    assert!(QuantityValueV1::decode_frame(&frame).is_err());
    assert_eq!(DecimalValueV1::frame_name(), DECIMAL_SCHEMA_NAME_V1);
    assert_eq!(
        Option::<DecimalValueV1>::frame_name(),
        std::any::type_name::<Option<DecimalValueV1>>()
    );
    assert_ne!(
        Option::<DecimalValueV1>::frame_name(),
        format!("core::option::Option<{DECIMAL_SCHEMA_NAME_V1}>")
    );
}

#[derive(NoritoSchema)]
#[norito_schema(name = "example.marker.Payload")]
struct PayloadMarker;

#[test]
fn primitive_schema_identity_needs_no_marker_codec_and_preserves_capacity() {
    let marker = PayloadMarker::nominal_name();
    assert_eq!(
        ConstVec::<PayloadMarker>::nominal_name(),
        format!("iroha_primitives::const_vec::ConstVec<{marker}>")
    );
    assert_eq!(
        UniqueVec::<PayloadMarker>::nominal_name(),
        format!("iroha_primitives::unique_vec::UniqueVec<{marker}>")
    );
    assert_eq!(
        SmallVec::<[PayloadMarker; 2]>::nominal_name(),
        format!("iroha_primitives::small::small_vector::SmallVec<[{marker}; 2]>")
    );
    assert_ne!(
        ConstVec::<u32>::nominal_name(),
        ConstVec::<u64>::nominal_name()
    );
    assert_ne!(
        UniqueVec::<u32>::nominal_name(),
        UniqueVec::<u64>::nominal_name()
    );
    let two = norito::to_bytes(&SmallVec::<[u32; 2]>::from(vec![7, 11])).unwrap();
    let eight = norito::to_bytes(&SmallVec::<[u32; 8]>::from(vec![7, 11])).unwrap();
    assert_eq!(
        two[norito::core::Header::SIZE..],
        eight[norito::core::Header::SIZE..]
    );
    assert_ne!(
        norito::schema::identity::frame_hash::<SmallVec<[u32; 2]>>(),
        norito::schema::identity::frame_hash::<SmallVec<[u32; 8]>>()
    );
    assert!(norito::decode_from_bytes::<SmallVec<[u32; 8]>>(&two).is_err());
}
