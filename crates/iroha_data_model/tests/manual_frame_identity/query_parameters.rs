//! Captured public query parameter frames and checked reconstruction contracts.

use std::{fmt::Debug, num::NonZeroU64};

use iroha_data_model::query::parameters::{
    FetchSize, MAX_FETCH_SIZE, Pagination, QueryParams, SortOrder, Sorting,
};
use norito::{
    NoritoDeserialize, NoritoSerialize,
    codec::Encode as _,
    core as ncore,
    json::{JsonDeserialize, JsonSerialize, Value},
};

use crate::frame_identity_test_support::record;

fn family<T>(rows: &mut Vec<Value>, name: &str, values: &[T])
where
    T: norito::NoritoSchema
        + Clone
        + Debug
        + PartialEq
        + NoritoSerialize
        + for<'de> NoritoDeserialize<'de>
        + JsonSerialize
        + JsonDeserialize,
{
    assert!(!values.is_empty());
    for (index, value) in values.iter().enumerate() {
        assert!(!values[..index].contains(value));
        record(rows, &format!("{name}/root_{index}"), value);
        record(
            rows,
            &format!("{name}/option_{index}"),
            &Some(value.clone()),
        );
        let json = norito::json::to_json(value).unwrap();
        assert_eq!(
            norito::json::to_json_bounded(value, json.len()).unwrap(),
            json
        );
        assert_eq!(
            norito::json::to_json_bounded(value, json.len() - 1),
            Err(norito::json::BoundedJsonError::BodyTooLarge)
        );
    }
    record(rows, &format!("{name}/option_none"), &None::<T>);
    record(rows, &format!("{name}/vec_empty"), &Vec::<T>::new());
    record(rows, &format!("{name}/vec_all"), &values.to_vec());
}

fn reject_payload<T>(bytes: &[u8])
where
    T: NoritoSerialize + for<'de> NoritoDeserialize<'de>,
{
    let flags = ncore::default_encode_flags();
    let frame = ncore::frame_bare_with_header_flags::<T>(bytes, flags).unwrap();
    let view = ncore::from_bytes_view(&frame).expect("authenticate full malformed-field frame");
    assert_eq!(view.as_bytes(), bytes);
    assert!(
        view.decode_exact_with(ncore::decode_field_canonical::<T>)
            .is_err()
    );
    assert!(norito::decode_canonical::<T>(&frame).is_err());
    let _flags = ncore::DecodeFlagsGuard::enter(flags);
    let _payload = ncore::PayloadCtxGuard::enter_with_schema_and_flags(bytes, view.schema(), flags);
    let archived = ncore::from_bytes::<T>(&frame).unwrap();
    assert!(<T as ncore::DeserializePayload<'_>>::try_deserialize(archived).is_err());
}

fn append_field<T: ncore::SerializePayload>(bytes: &mut Vec<u8>, value: &T) {
    let _flags = ncore::DecodeFlagsGuard::enter(ncore::default_encode_flags());
    let mut field = Vec::new();
    ncore::serialize_to_buffer(value, &mut field).unwrap();
    ncore::write_len_header_to_vec(bytes, u64::try_from(field.len()).unwrap());
    bytes.extend_from_slice(&field);
}

fn pagination_payload(limit: u64, offset: u64) -> Vec<u8> {
    let mut bytes = Vec::new();
    append_field(&mut bytes, &Some(limit));
    append_field(&mut bytes, &offset);
    bytes
}

fn fetch_size_payload(size: u64) -> Vec<u8> {
    let mut bytes = Vec::new();
    append_field(&mut bytes, &Some(size));
    bytes
}

fn checked_controls() {
    let pagination = Pagination::new(NonZeroU64::new(7), 3);
    assert_eq!(pagination_payload(7, 3), pagination.encode());
    reject_payload::<Pagination>(&pagination_payload(0, 3));
    assert_eq!(
        fetch_size_payload(7),
        FetchSize::new(NonZeroU64::new(7)).encode()
    );
    reject_payload::<FetchSize>(&fetch_size_payload(0));
    assert!(norito::json::from_json::<Pagination>(r#"{"limit":0,"offset":3}"#).is_err());
    assert!(norito::json::from_json::<FetchSize>(r#"{"fetch_size":0}"#).is_err());

    for (value, tag, json) in [
        (SortOrder::Asc, 0_u32, "\"Asc\""),
        (SortOrder::Desc, 1, "\"Desc\""),
    ] {
        assert_eq!(value.encode(), tag.to_le_bytes());
        assert_eq!(norito::json::to_json(&value).unwrap(), json);
    }
    reject_payload::<SortOrder>(&u32::MAX.to_le_bytes());
    for json in ["\"asc\"", "\"DESC\"", "\"Unknown\"", "0", "null"] {
        assert!(norito::json::from_json::<SortOrder>(json).is_err());
    }

    let sorting = Sorting::by_metadata_key("rank".parse().unwrap());
    let mut json = norito::json::to_value(&sorting).unwrap();
    json.as_object_mut().unwrap().remove("order");
    assert_eq!(norito::json::from_value::<Sorting>(json).unwrap(), sorting);
    assert_eq!(sorting.sort_by_metadata_key().unwrap().as_ref(), "rank");
    assert_eq!(sorting.order(), None);
    assert_eq!(Pagination::default().offset_value(), 0);
    assert_eq!(Pagination::default().limit_value(), None);
    assert_eq!(FetchSize::default().value(), None);

    let params = QueryParams::new(pagination, sorting, FetchSize::new(NonZeroU64::new(7)));
    let mut json = norito::json::to_value(&params).unwrap();
    let fetch = json.as_object_mut().unwrap().get_mut("fetch_size").unwrap();
    fetch
        .as_object_mut()
        .unwrap()
        .insert("fetch_size".into(), norito::json!(0));
    assert!(norito::json::from_value::<QueryParams>(json).is_err());
    assert_eq!(params.pagination(), &pagination);
    assert_eq!(params.fetch_size().value(), NonZeroU64::new(7));
    assert_eq!(params.sorting().order(), None);
}

fn capture_values() -> Vec<Value> {
    let mut rows = Vec::new();
    family(
        &mut rows,
        "pagination",
        &[
            Pagination::default(),
            Pagination::new(NonZeroU64::new(1), 3),
            Pagination::new(NonZeroU64::new(u64::MAX), u64::MAX),
        ],
    );
    family(&mut rows, "sort_order", &[SortOrder::Asc, SortOrder::Desc]);
    family(
        &mut rows,
        "sorting",
        &[
            Sorting::default(),
            Sorting::by_metadata_key("rank".parse().unwrap()),
            Sorting::new(Some("rank".parse().unwrap()), Some(SortOrder::Asc)),
            Sorting::new(Some("rank".parse().unwrap()), Some(SortOrder::Desc)),
            Sorting::new(None, Some(SortOrder::Desc)),
        ],
    );
    // FetchSize is a structural request hint. The executing service enforces its
    // configured maximum; a wire declaration must not introduce a second policy.
    family(
        &mut rows,
        "fetch_size",
        &[
            FetchSize::default(),
            FetchSize::new(NonZeroU64::new(1)),
            FetchSize::new(Some(MAX_FETCH_SIZE)),
            FetchSize::new(NonZeroU64::new(MAX_FETCH_SIZE.get() + 1)),
            FetchSize::new(NonZeroU64::new(u64::MAX)),
        ],
    );
    family(
        &mut rows,
        "query_params",
        &[
            QueryParams::default(),
            QueryParams::new(
                Pagination::new(NonZeroU64::new(7), 3),
                Sorting::by_metadata_key("rank".parse().unwrap()),
                FetchSize::new(NonZeroU64::new(5)),
            ),
            QueryParams::new(
                Pagination::new(NonZeroU64::new(u64::MAX), u64::MAX),
                Sorting::new(Some("rank".parse().unwrap()), Some(SortOrder::Desc)),
                FetchSize::new(NonZeroU64::new(u64::MAX)),
            ),
        ],
    );
    checked_controls();
    assert_eq!(rows.len(), 51);
    rows
}

#[test]
fn public_query_parameter_frames_match_capture() {
    let evidence = norito::json!({
        "format_version": 1,
        "purpose": "public query parameter owners before identity declaration",
        "default_encode_flags": (ncore::default_encode_flags()),
        "rows": (capture_values()),
    });
    let captured: Value = norito::json::from_json(include_str!(
        "../fixtures/query_parameter_identity_frames.json"
    ))
    .expect("decode immutable pre-declaration capture");
    assert_eq!(evidence, captured);
}
