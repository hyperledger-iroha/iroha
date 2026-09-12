//! Public frames for the derived query owners adjacent to the manual closure.

use std::num::NonZeroU64;

use iroha_data_model::{
    account::Account,
    domain::Domain,
    query::{
        ErasedIterQuery, QueryBox, QueryItemKind, QueryOutput, QueryOutputBatchBox, QueryRequest,
        QueryWithParams, SingularQueryBox, SingularQueryOutputBox,
        account::FindAccounts,
        domain::FindDomains,
        dsl::{CompoundPredicate, SelectorTuple},
        executor::{FindExecutorDataModel, FindParameters},
        json_wrappers::{QueryRequestJson, query_request_from_json, query_request_to_json},
        parameters::{FetchSize, ForwardCursor, Pagination, QueryParams, Sorting},
        runtime::{AbiVersion, FindAbiVersion},
    },
};
use norito::{NoritoDeserialize, NoritoSerialize, codec::Encode as _, core as ncore, json::Value};

use super::query_time::{
    checked_json, cursor, domain_query, empty_rows, family, one_column, request_continue,
    request_singular, request_start, response_exact, response_singular,
};

fn singular_request() -> QueryRequest {
    request_singular().into_parts().1
}
fn start_request() -> QueryRequest {
    request_start().into_parts().1
}
fn continue_request() -> QueryRequest {
    request_continue().into_parts().1
}

fn inspect_request(value: &QueryRequest) -> Value {
    let wrapper = query_request_to_json(value);
    let json = norito::json::to_json(&wrapper).unwrap();
    let restored: QueryRequestJson = norito::json::from_json(&json).unwrap();
    assert_eq!(restored, wrapper);
    let restored = query_request_from_json(restored).unwrap();
    assert_eq!(restored.encode(), value.encode());
    assert_eq!(
        norito::json::to_json(&query_request_to_json(&restored)).unwrap(),
        json
    );
    Value::String(json)
}

fn domain_with_params() -> QueryWithParams {
    QueryWithParams::new(&domain_query(), QueryParams::default()).unwrap()
}
fn account_with_params() -> QueryWithParams {
    let query: QueryBox<QueryOutputBatchBox> = Box::new(ErasedIterQuery::<Account>::new(
        CompoundPredicate::PASS,
        SelectorTuple::default(),
        FindAccounts.encode(),
    ));
    let parameters = QueryParams::new(
        Pagination::new(NonZeroU64::new(7), 2),
        Sorting::by_metadata_key("rank".parse().unwrap()),
        FetchSize::new(NonZeroU64::new(3)),
    );
    QueryWithParams::new(&query, parameters).unwrap()
}
fn inspect_with_params(value: &QueryWithParams) -> Value {
    // parts() returns item, predicate, selector, query payload in this exact
    // public API order. The binary field order remains independently captured.
    let (item, predicate, selector, payload) = value.parts();
    match item {
        QueryItemKind::Domain => {
            assert_eq!(payload, FindDomains.encode());
            assert_eq!(predicate, CompoundPredicate::<Domain>::PASS.encode());
            assert_eq!(selector, SelectorTuple::<Domain>::default().encode());
            assert_eq!(value.params(), &QueryParams::default());
        }
        QueryItemKind::Account => {
            assert_eq!(payload, FindAccounts.encode());
            assert_eq!(predicate, CompoundPredicate::<Account>::PASS.encode());
            assert_eq!(selector, SelectorTuple::<Account>::default().encode());
            assert_eq!(
                value.params().pagination().limit_value(),
                NonZeroU64::new(7)
            );
            assert_eq!(value.params().pagination().offset_value(), 2);
            assert_eq!(
                value
                    .params()
                    .sorting()
                    .sort_by_metadata_key()
                    .unwrap()
                    .as_ref(),
                "rank"
            );
            assert_eq!(value.params().fetch_size().value(), NonZeroU64::new(3));
        }
        _ => panic!("unexpected iterable query fixture"),
    }
    norito::json!({
        "item": item, "predicate_hex": (hex::encode(predicate)),
        "selector_hex": (hex::encode(selector)), "query_payload_hex": (hex::encode(payload)),
        "params_json": (checked_json(value.params())),
        "request_json": (inspect_request(&QueryRequest::Start(value.clone()))),
    })
}

fn item_kind_values(rows: &mut Vec<Value>) {
    // Current public declaration order, including the three distinct escrow
    // operation discriminators. No new mapping or operation is introduced.
    let factories: [fn() -> QueryItemKind; 31] = [
        || QueryItemKind::Domain,
        || QueryItemKind::Account,
        || QueryItemKind::AccountId,
        || QueryItemKind::Asset,
        || QueryItemKind::AssetDefinition,
        || QueryItemKind::RepoAgreement,
        || QueryItemKind::Nft,
        || QueryItemKind::Rwa,
        || QueryItemKind::Role,
        || QueryItemKind::RoleId,
        || QueryItemKind::PeerId,
        || QueryItemKind::TriggerId,
        || QueryItemKind::Trigger,
        || QueryItemKind::CommittedTransaction,
        || QueryItemKind::SignedBlock,
        || QueryItemKind::BlockHeader,
        || QueryItemKind::ProofRecord,
        || QueryItemKind::OracleFeedConfig,
        || QueryItemKind::OracleFeedEventRecord,
        || QueryItemKind::OracleProviderStatsRecord,
        || QueryItemKind::OracleDispute,
        || QueryItemKind::OracleChangeProposal,
        || QueryItemKind::TwitterBindingRecord,
        || QueryItemKind::DefiOracleAttestation,
        || QueryItemKind::Permission,
        || QueryItemKind::AssetEscrowRecord,
        || QueryItemKind::FeeSponsorProgram,
        || QueryItemKind::FeeSponsorProgramId,
        || QueryItemKind::AssetEscrowsBySeller,
        || QueryItemKind::AssetEscrowsByBuyer,
        || QueryItemKind::AssetEscrowsByStatus,
    ];
    for (tag, make) in factories.iter().enumerate() {
        assert_eq!(
            make().encode(),
            u32::try_from(tag).unwrap().encode(),
            "current public unit-variant tag"
        );
    }
    family(rows, "query_item_kind", &factories, checked_json);
    reject_payload::<QueryItemKind>(&u32::MAX.encode());
    assert!(norito::json::from_json::<QueryItemKind>(r#"{"kind":"UnknownQueryItem"}"#).is_err());
}

fn output_exact() -> QueryOutput {
    let iroha_data_model::query::QueryResponse::Iterable(value) = response_exact() else {
        unreachable!()
    };
    value
}
fn output_exact_zero() -> QueryOutput {
    QueryOutput::new(empty_rows(), 0, None)
}
fn output_bounded_cursor() -> QueryOutput {
    QueryOutput::new_bounded(one_column(), false, Some(cursor()))
}
fn inspect_output(value: &QueryOutput) -> Value {
    let (batch, remaining, has_more, cursor) = value.clone().into_parts_with_count_mode();
    assert_eq!(value.remaining_items_hint(), remaining.unwrap_or(0));
    let (hint_batch, hint_remaining, hint_cursor) = value.clone().into_parts();
    assert_eq!(hint_batch, batch);
    assert_eq!(hint_remaining, remaining.unwrap_or(0));
    assert_eq!(hint_cursor, cursor);
    assert!(batch.column_count() > 0);
    for column in batch.columns() {
        assert_eq!(column.len(), batch.len());
    }
    if remaining.is_none() {
        assert!(has_more);
        assert!(cursor.is_some());
    }
    if batch.is_empty() {
        assert_eq!(remaining, Some(0));
        assert!(!has_more);
        assert!(cursor.is_none());
    }
    norito::json!({
        "json": (checked_json(value)), "batch_json": (checked_json(&batch)),
        "remaining": remaining, "has_more": has_more, "cursor": cursor,
    })
}

fn strings_batch() -> QueryOutputBatchBox {
    one_column().into_columns().remove(0)
}
fn names_batch() -> QueryOutputBatchBox {
    QueryOutputBatchBox::Name(vec!["alpha".parse().unwrap(), "beta".parse().unwrap()])
}
fn empty_batch() -> QueryOutputBatchBox {
    QueryOutputBatchBox::String(Vec::new())
}
fn inspect_batch(value: &QueryOutputBatchBox) -> Value {
    let expected = match value {
        QueryOutputBatchBox::String(values) => values.len(),
        QueryOutputBatchBox::Name(values) => values.len(),
        _ => panic!("unexpected batch fixture"),
    };
    assert_eq!(value.len(), expected);
    assert_eq!(value.is_empty(), expected == 0);
    norito::json!({ "json": (checked_json(value)), "rows": expected })
}

fn singular_executor_query() -> SingularQueryBox {
    SingularQueryBox::FindExecutorDataModel(FindExecutorDataModel)
}
fn singular_parameters_query() -> SingularQueryBox {
    SingularQueryBox::FindParameters(FindParameters)
}
fn singular_abi_query() -> SingularQueryBox {
    SingularQueryBox::FindAbiVersion(FindAbiVersion)
}
fn singular_parameters_output() -> SingularQueryOutputBox {
    let iroha_data_model::query::QueryResponse::Singular(value) = response_singular() else {
        unreachable!()
    };
    value
}
fn singular_abi_output() -> SingularQueryOutputBox {
    SingularQueryOutputBox::AbiVersion(AbiVersion { abi_version: 1 })
}

fn cursor_with_budget() -> ForwardCursor {
    cursor()
}
fn cursor_without_budget() -> ForwardCursor {
    norito::json::from_json(r#"{"query":"fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210","cursor":2,"gas_budget":null}"#).unwrap()
}
fn inspect_cursor(value: &ForwardCursor) -> Value {
    assert_eq!(value.query().len(), 64);
    assert!(
        value
            .query()
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    );
    assert!(value.cursor().get() > 0);
    norito::json!({
        "json": (checked_json(value)), "query": (value.query()),
        "cursor": (value.cursor().get()), "gas_budget": (*value.gas_budget()),
    })
}

fn append_field<T: ncore::SerializePayload>(bytes: &mut Vec<u8>, value: &T) {
    let _flags = ncore::DecodeFlagsGuard::enter(ncore::default_encode_flags());
    let mut field = Vec::new();
    ncore::serialize_to_buffer(value, &mut field).unwrap();
    ncore::write_len_header_to_vec(bytes, u64::try_from(field.len()).unwrap());
    bytes.extend_from_slice(&field);
}
fn reject_payload<T>(bytes: &[u8])
where
    T: NoritoSerialize + for<'de> NoritoDeserialize<'de>,
{
    let flags = ncore::default_encode_flags();
    let frame = ncore::frame_bare_with_header_flags::<T>(bytes, flags).unwrap();
    let view = ncore::from_bytes_view(&frame).unwrap();
    assert_eq!(view.as_bytes(), bytes);
    assert!(norito::decode_canonical::<T>(&frame).is_err());
    assert!(
        view.decode_exact_with(ncore::decode_field_canonical::<T>)
            .is_err()
    );
    let _flags = ncore::DecodeFlagsGuard::enter(flags);
    let _payload = ncore::PayloadCtxGuard::enter_with_schema_and_flags(bytes, view.schema(), flags);
    let archived = ncore::from_bytes::<T>(&frame).unwrap();
    assert!(<T as ncore::DeserializePayload<'_>>::try_deserialize(archived).is_err());
}
fn cursor_payload(value: &ForwardCursor, position: u64) -> Vec<u8> {
    let mut bytes = Vec::new();
    append_field(&mut bytes, value.query());
    append_field(&mut bytes, &position);
    append_field(&mut bytes, value.gas_budget());
    bytes
}
fn cursor_rejections() {
    for value in [cursor_with_budget(), cursor_without_budget()] {
        assert_eq!(
            cursor_payload(&value, value.cursor().get()),
            value.encode(),
            "control reproduces the complete public cursor field layout"
        );
        reject_payload::<ForwardCursor>(&cursor_payload(&value, 0));
        let mut json = norito::json::to_value(&value).unwrap();
        json.as_object_mut()
            .unwrap()
            .insert("cursor".into(), Value::from(0_u64));
        assert!(norito::json::from_value::<ForwardCursor>(json).is_err());
    }
}
fn unknown_variant<T>(value: &T)
where
    T: NoritoSerialize + for<'de> NoritoDeserialize<'de>,
{
    let mut bytes = value.encode();
    assert!(bytes.len() >= 4);
    bytes[..4].copy_from_slice(&u32::MAX.to_le_bytes());
    reject_payload::<T>(&bytes);
}

fn capture_values() -> Vec<Value> {
    let mut rows = Vec::new();
    family(
        &mut rows,
        "query_request",
        &[singular_request, start_request, continue_request],
        inspect_request,
    );
    family(
        &mut rows,
        "query_with_params",
        &[domain_with_params, account_with_params],
        inspect_with_params,
    );
    item_kind_values(&mut rows);
    family(
        &mut rows,
        "query_output",
        &[output_exact, output_exact_zero, output_bounded_cursor],
        inspect_output,
    );
    family(
        &mut rows,
        "query_output_batch_box",
        &[strings_batch, names_batch, empty_batch],
        inspect_batch,
    );
    family(
        &mut rows,
        "singular_query_box",
        &[
            singular_executor_query,
            singular_parameters_query,
            singular_abi_query,
        ],
        checked_json,
    );
    family(
        &mut rows,
        "singular_query_output_box",
        &[singular_parameters_output, singular_abi_output],
        checked_json,
    );
    family(
        &mut rows,
        "forward_cursor",
        &[cursor_with_budget, cursor_without_budget],
        inspect_cursor,
    );
    cursor_rejections();
    unknown_variant(&singular_request());
    unknown_variant(&strings_batch());
    unknown_variant(&singular_executor_query());
    unknown_variant(&singular_parameters_output());
    assert_eq!(rows.len(), 122);
    rows
}

#[test]
fn public_derived_query_frames_match_capture() {
    let evidence = norito::json!({
        "format_version": 1,
        "purpose": "adjacent public derived query owners before identity declaration",
        "default_encode_flags": (ncore::default_encode_flags()),
        "rows": (capture_values()),
    });
    let captured: Value = norito::json::from_json(include_str!(
        "../fixtures/query_derived_identity_frames.json"
    ))
    .expect("decode immutable pre-declaration capture");
    assert_eq!(evidence, captured);
}
