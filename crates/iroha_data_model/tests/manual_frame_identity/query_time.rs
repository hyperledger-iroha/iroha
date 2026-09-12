//! Captured public query and time frames and checked reconstruction contracts.

use std::{num::NonZeroU64, time::Duration};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use iroha_crypto::{Algorithm, KeyPair, Signature, SignatureOf};
use iroha_data_model::{
    NetworkId,
    account::{Account, AccountId},
    domain::Domain,
    events::time::TimeInterval,
    parameter::Parameters,
    query::{
        ErasedIterQuery, QueryBox, QueryOutput, QueryOutputBatchBox, QueryOutputBatchBoxTuple,
        QueryOutputBatchBoxTupleError, QueryRequest, QueryRequestWithAuthority, QueryResponse,
        QuerySignature, QueryWithParams, SignedQuery, SingularQueryBox, SingularQueryOutputBox,
        account::FindAccounts,
        domain::FindDomains,
        dsl::{CompoundPredicate, SelectorTuple},
        executor::FindExecutorDataModel,
        iter_query_inner,
        json_wrappers::{
            QueryRequestWithAuthorityJson, SignedQueryJson, query_request_from_json,
            query_request_to_json,
        },
        parameters::{ForwardCursor, QueryParams},
    },
};
use iroha_version::codec::{DecodeVersioned as _, EncodeVersioned as _};
use norito::{
    NoritoDeserialize, NoritoSerialize,
    codec::{DecodeAll as _, Encode as _},
    core as ncore,
    json::{JsonDeserialize, JsonSerialize, Value},
};

// QueryBox, QueryRequestWithAuthority and SignedQuery intentionally need no
// Clone/Debug/Eq or direct JSON API. Inspect public semantics after each decode.
fn record<T>(rows: &mut Vec<Value>, case: &str, value: &T, inspect: impl Fn(&T) -> Value)
where
    T: norito::NoritoSchema + NoritoSerialize + for<'de> NoritoDeserialize<'de>,
{
    let expected = inspect(value);
    crate::frame_identity_test_support::record_checked(rows, case, value, |decoded| {
        assert_eq!(
            inspect(decoded),
            expected,
            "{case}: complete decoded semantics"
        );
    });
    rows.last_mut()
        .expect("record_checked appended one row")
        .as_object_mut()
        .expect("captured row is an object")
        .insert("semantics".into(), expected);
}

/// Capture actual frames for a factory family without adding public model traits.
pub fn family<T>(
    rows: &mut Vec<Value>,
    name: &str,
    factories: &[fn() -> T],
    inspect: fn(&T) -> Value,
) where
    T: norito::NoritoSchema + NoritoSerialize + for<'de> NoritoDeserialize<'de>,
{
    assert!(!factories.is_empty());
    let mut seen = Vec::new();
    for (index, make) in factories.iter().enumerate() {
        let value = make();
        let semantic = inspect(&value);
        assert!(!seen.contains(&semantic), "distinct populated cases");
        seen.push(semantic);
        record(rows, &format!("{name}/root_{index}"), &value, inspect);
        record(
            rows,
            &format!("{name}/option_{index}"),
            &Some(make()),
            |option| option.as_ref().map_or(Value::Null, inspect),
        );
    }
    record(rows, &format!("{name}/option_none"), &None::<T>, |option| {
        option.as_ref().map_or(Value::Null, inspect)
    });
    let inspect_vec = |values: &Vec<T>| Value::Array(values.iter().map(inspect).collect());
    record(
        rows,
        &format!("{name}/vec_empty"),
        &Vec::<T>::new(),
        inspect_vec,
    );
    let values: Vec<T> = factories.iter().map(|make| make()).collect();
    record(rows, &format!("{name}/vec_all"), &values, inspect_vec);
}

/// Check the existing JSON API and return its canonical JSON text.
pub fn checked_json<T: JsonSerialize + JsonDeserialize + ncore::SerializePayload>(
    value: &T,
) -> Value {
    let json = norito::json::to_json(value).unwrap();
    let decoded: T = norito::json::from_json(&json).unwrap();
    assert_eq!(
        decoded.encode(),
        value.encode(),
        "JSON preserves full payload"
    );
    assert_eq!(norito::json::to_json(&decoded).unwrap(), json);
    Value::String(json)
}

fn reject_payload<T>(bytes: &[u8])
where
    T: NoritoSerialize + for<'de> NoritoDeserialize<'de>,
{
    let flags = ncore::default_encode_flags();
    let frame = ncore::frame_bare_with_header_flags::<T>(bytes, flags)
        .expect("authenticate malformed fields with correct metadata and checksum");
    let view = ncore::from_bytes_view(&frame).expect("authenticate malformed-field frame");
    assert_eq!(view.as_bytes(), bytes);
    assert!(
        view.decode_exact_with(ncore::decode_field_canonical::<T>)
            .is_err()
    );
    assert!(norito::decode_canonical::<T>(&frame).is_err());
    let _flags = ncore::DecodeFlagsGuard::enter(flags);
    let _payload = ncore::PayloadCtxGuard::enter_with_schema_and_flags(bytes, view.schema(), flags);
    let archived = ncore::from_bytes::<T>(&frame).expect("authenticate typed archive");
    assert!(
        <T as ncore::DeserializePayload<'_>>::try_deserialize(archived).is_err(),
        "direct fallible reconstruction must reject without a panic catcher"
    );
}

fn append_field<T: ncore::SerializePayload>(bytes: &mut Vec<u8>, value: &T) {
    let _flags = ncore::DecodeFlagsGuard::enter(ncore::default_encode_flags());
    let mut field = Vec::new();
    ncore::serialize_to_buffer(value, &mut field).unwrap();
    ncore::write_len_header_to_vec(bytes, u64::try_from(field.len()).unwrap());
    bytes.extend_from_slice(&field);
}

/// Construct a public erased domain query using the canonical registry owner.
pub fn domain_query() -> QueryBox<QueryOutputBatchBox> {
    Box::new(ErasedIterQuery::<Domain>::new(
        CompoundPredicate::PASS,
        SelectorTuple::default(),
        FindDomains.encode(),
    ))
}

fn account_query() -> QueryBox<QueryOutputBatchBox> {
    Box::new(ErasedIterQuery::<Account>::new(
        CompoundPredicate::PASS,
        SelectorTuple::default(),
        FindAccounts.encode(),
    ))
}

fn inspect_query(value: &QueryBox<QueryOutputBatchBox>) -> Value {
    let (wire_id, inner_bytes) =
        <(String, Vec<u8>)>::decode_all(&mut value.encode().as_slice()).unwrap();
    assert_eq!(inner_bytes, value.encode_bytes());
    let (expected_id, concrete_nominal_name, concrete_payload, predicate, selector) =
        iter_query_inner::<Domain>(value).map_or_else(
            || {
                let inner =
                    iter_query_inner::<Account>(value).expect("fixture account query downcast");
                assert_eq!(
                    value.type_name_key(),
                    std::any::type_name::<ErasedIterQuery<Account>>()
                );
                assert_eq!(inner.payload(), FindAccounts.encode());
                assert_eq!(
                    inner.predicate().encode(),
                    CompoundPredicate::<Account>::PASS.encode()
                );
                assert_eq!(
                    inner.selector().encode(),
                    SelectorTuple::<Account>::default().encode()
                );
                (
                    "iroha.query.v1::iterable::account::Account",
                    <ErasedIterQuery<Account> as norito::NoritoSchema>::nominal_name(),
                    inner.payload(),
                    inner.predicate().encode(),
                    inner.selector().encode(),
                )
            },
            |inner| {
                assert_eq!(
                    value.type_name_key(),
                    std::any::type_name::<ErasedIterQuery<Domain>>()
                );
                assert_eq!(inner.payload(), FindDomains.encode());
                assert_eq!(
                    inner.predicate().encode(),
                    CompoundPredicate::<Domain>::PASS.encode()
                );
                assert_eq!(
                    inner.selector().encode(),
                    SelectorTuple::<Domain>::default().encode()
                );
                (
                    "iroha.query.v1::iterable::domain::Domain",
                    <ErasedIterQuery<Domain> as norito::NoritoSchema>::nominal_name(),
                    inner.payload(),
                    inner.predicate().encode(),
                    inner.selector().encode(),
                )
            },
        );
    // These stable IDs are taken from define_builtin_query_registry!, not a
    // guessed Rust nominal identity or a new registry installed by this test.
    assert_eq!(wire_id, expected_id);
    assert_ne!(wire_id, value.type_name_key());
    let _flags = ncore::DecodeFlagsGuard::enter(ncore::default_encode_flags());
    let mut streamed = Vec::new();
    let mut encoder = ncore::Encoder::new(&mut streamed);
    let written = value.encode_payload_to(&mut encoder).unwrap();
    assert_eq!(written, inner_bytes.len());
    assert_eq!(streamed, inner_bytes);
    if let Some(exact) = value.encoded_payload_len_exact() {
        assert_eq!(exact, inner_bytes.len());
    }
    norito::json!({
        "wire_id": wire_id,
        // The capture recorded this owner's Rust name before declaration. Compare
        // its declared nominal name here; the live dispatch key is checked above.
        "concrete_type_name_key": concrete_nominal_name,
        "concrete_payload_hex": (hex::encode(concrete_payload)),
        "predicate_hex": (hex::encode(predicate)),
        "selector_hex": (hex::encode(selector)),
        "erased_payload_hex": (hex::encode(inner_bytes)),
    })
}

fn query_rejections() {
    let value = domain_query();
    let (wire_id, bytes) = <(String, Vec<u8>)>::decode_all(&mut value.encode().as_slice()).unwrap();
    assert_eq!((wire_id, bytes.clone()).encode(), value.encode());
    reject_payload::<QueryBox<QueryOutputBatchBox>>(
        &(value.type_name_key().to_owned(), bytes.clone()).encode(),
    );
    reject_payload::<QueryBox<QueryOutputBatchBox>>(
        &("capture.unknown.query.v1".to_owned(), bytes).encode(),
    );
    // Invalid concrete bytes under a real registered ID are not an alias error.
    reject_payload::<QueryBox<QueryOutputBatchBox>>(
        &(
            "iroha.query.v1::iterable::domain::Domain".to_owned(),
            vec![0xff_u8],
        )
            .encode(),
    );
    assert_eq!(inspect_query(&domain_query()), inspect_query(&value));
}

fn key(seed: u8) -> KeyPair {
    KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
        .expect("checked deterministic query key")
}

fn network() -> NetworkId {
    "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"
        .parse()
        .expect("canonical checked fixture network")
}

/// Construct a deterministic opaque cursor wire fixture through its public JSON API.
pub fn cursor() -> ForwardCursor {
    // A fixed opaque server-token wire fixture; no test issues a live continuation
    // or treats this synthetic value as portable between Torii instances.
    norito::json::from_json(r#"{"query":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","cursor":1,"gas_budget":5}"#)
        .expect("public JSON cursor constructor")
}

fn request(kind: u8) -> QueryRequestWithAuthority {
    let request = match kind {
        0 => QueryRequest::Singular(SingularQueryBox::FindExecutorDataModel(
            FindExecutorDataModel,
        )),
        1 => QueryRequest::Start(
            QueryWithParams::new(&domain_query(), QueryParams::default()).unwrap(),
        ),
        2 => QueryRequest::Continue(cursor()),
        _ => unreachable!("three query request variants"),
    };
    request.with_authority(
        network(),
        AccountId::new(key(0x51).public_key().clone()),
        1_000_000,
        NonZeroU64::new(10_000).unwrap(),
        [0x5a + kind; 32],
    )
}

/// Construct an unsigned singular request with a complete explicit signing context.
pub fn request_singular() -> QueryRequestWithAuthority {
    request(0)
}
/// Construct an unsigned iterable request with a complete explicit signing context.
pub fn request_start() -> QueryRequestWithAuthority {
    request(1)
}
/// Construct an unsigned continuation request with a complete explicit signing context.
pub fn request_continue() -> QueryRequestWithAuthority {
    request(2)
}

fn request_json(value: &QueryRequestWithAuthority) -> QueryRequestWithAuthorityJson {
    QueryRequestWithAuthorityJson {
        network_id: value.network_id(),
        authority: value.authority().clone(),
        creation_time_ms: value.creation_time_ms(),
        time_to_live_ms: value.time_to_live_ms(),
        nonce: *value.nonce(),
        request: query_request_to_json(value.request()),
    }
}

fn inspect_request(value: &QueryRequestWithAuthority) -> Value {
    let dto = request_json(value);
    let json = norito::json::to_json(&dto).unwrap();
    let restored: QueryRequestWithAuthorityJson = norito::json::from_json(&json).unwrap();
    assert_eq!(dto, restored);
    let request = query_request_from_json(restored.request)
        .unwrap()
        .with_authority(
            restored.network_id,
            restored.authority,
            restored.creation_time_ms,
            restored.time_to_live_ms,
            restored.nonce,
        );
    assert_eq!(request.encode(), value.encode());
    assert_eq!(
        norito::json::to_json(&request_json(&request)).unwrap(),
        json
    );
    Value::String(json)
}

fn signed_singular() -> SignedQuery {
    request_singular().try_sign(&key(0x51)).unwrap()
}
fn signed_start() -> SignedQuery {
    request_start().try_sign(&key(0x51)).unwrap()
}
fn signed_continue() -> SignedQuery {
    request_continue().try_sign(&key(0x51)).unwrap()
}

fn inspect_signed(value: &SignedQuery) -> Value {
    value
        .verify_signature()
        .expect("fixture signature verifies complete context");
    let dto = SignedQueryJson::from(value);
    let json = norito::json::to_json(&dto).unwrap();
    let restored: SignedQueryJson = norito::json::from_json(&json).unwrap();
    assert_eq!(restored, dto);
    let restored = SignedQuery::try_from(restored).unwrap();
    restored.verify_signature().unwrap();
    assert_eq!(restored.encode(), value.encode());
    assert_eq!(value.authority(), restored.authority());
    assert_eq!(
        query_request_to_json(value.request()),
        query_request_to_json(restored.request())
    );
    let versioned = value.encode_versioned();
    assert_eq!(versioned[0], 1);
    assert_eq!(&versioned[1..], value.encode());
    let decoded = SignedQuery::decode_all_versioned(&versioned).unwrap();
    decoded.verify_signature().unwrap();
    assert_eq!(
        norito::json::to_json(&SignedQueryJson::from(&decoded)).unwrap(),
        json
    );
    assert_eq!(decoded.encode_versioned(), versioned);
    norito::json!({ "json": json, "versioned_hex": (hex::encode(versioned)) })
}

fn signature_singular() -> QuerySignature {
    let SignedQueryJson::Canonical(dto) = SignedQueryJson::from(&signed_singular());
    dto.signature
}
fn signature_continue() -> QuerySignature {
    let SignedQueryJson::Canonical(dto) = SignedQueryJson::from(&signed_continue());
    dto.signature
}

fn inspect_signature(value: &QuerySignature) -> Value {
    let signature =
        SignatureOf::<QueryRequestWithAuthority>::decode_all(&mut value.encode().as_slice())
            .unwrap();
    let matches: Vec<_> = [0, 2]
        .into_iter()
        .filter(|kind| {
            signature
                .verify(key(0x51).public_key(), &request(*kind))
                .is_ok()
        })
        .collect();
    assert_eq!(
        matches.len(),
        1,
        "signature identifies exactly its original complete request"
    );
    assert!(
        signature
            .verify(key(0x52).public_key(), &request(matches[0]))
            .is_err()
    );
    norito::json!({ "json": (checked_json(value)), "request_kind": (matches[0]) })
}

fn signature_projection_and_rejections() {
    for value in [signature_singular(), signature_continue()] {
        let typed =
            SignatureOf::<QueryRequestWithAuthority>::decode_all(&mut value.encode().as_slice())
                .unwrap();
        let untyped: Signature = typed.clone().into();
        let outer_frame = norito::encode_canonical(&value).unwrap();
        let typed_frame = norito::encode_canonical(&typed).unwrap();
        assert_eq!(
            outer_frame, typed_frame,
            "root projects to the typed signature"
        );
        assert_eq!(
            norito::decode_canonical::<QuerySignature>(&typed_frame).unwrap(),
            value
        );
        assert_eq!(
            norito::decode_canonical::<SignatureOf<QueryRequestWithAuthority>>(&outer_frame)
                .unwrap(),
            typed
        );
        assert_eq!(value.encode(), untyped.encode());
        assert_ne!(norito::encode_canonical(&untyped).unwrap(), outer_frame);
        assert!(norito::decode_canonical::<Signature>(&outer_frame).is_err());
        assert!(
            norito::decode_canonical::<QuerySignature>(
                &norito::encode_canonical(&untyped).unwrap()
            )
            .is_err()
        );
        let outer_option = norito::encode_canonical(&Some(value.clone())).unwrap();
        let typed_option = norito::encode_canonical(&Some(typed.clone())).unwrap();
        assert_eq!(Some(value.clone()).encode(), Some(typed.clone()).encode());
        assert_ne!(
            outer_option, typed_option,
            "Option retains nominal wrapper identity"
        );
        assert!(norito::decode_canonical::<Option<QuerySignature>>(&typed_option).is_err());
        assert!(
            norito::decode_canonical::<Option<SignatureOf<QueryRequestWithAuthority>>>(
                &outer_option
            )
            .is_err()
        );
        let outer_vec = norito::encode_canonical(&vec![value.clone()]).unwrap();
        let typed_vec = norito::encode_canonical(&vec![typed.clone()]).unwrap();
        assert_eq!(vec![value].encode(), vec![typed].encode());
        assert_ne!(outer_vec, typed_vec, "Vec retains nominal wrapper identity");
        assert!(norito::decode_canonical::<Vec<QuerySignature>>(&typed_vec).is_err());
        assert!(
            norito::decode_canonical::<Vec<SignatureOf<QueryRequestWithAuthority>>>(&outer_vec)
                .is_err()
        );
    }
    for bytes in [&[][..], &[0_u8; 64][..]] {
        reject_payload::<QuerySignature>(&Signature::from_bytes(bytes).encode());
    }
    for text in [
        String::new(),
        String::from("not base64"),
        STANDARD.encode([0_u8; 64]),
    ] {
        assert!(norito::json::from_value::<QuerySignature>(Value::String(text)).is_err());
    }
}

fn request_payload_with_ttl(value: &QueryRequestWithAuthority, ttl: u64) -> Vec<u8> {
    let _flags = ncore::DecodeFlagsGuard::enter(ncore::default_encode_flags());
    let mut bytes = Vec::new();
    append_field(&mut bytes, &value.network_id());
    append_field(&mut bytes, value.authority());
    append_field(&mut bytes, &value.creation_time_ms());
    append_field(&mut bytes, &ttl);
    // A derived [u8; N] field stores its raw width, unlike the generic array codec.
    ncore::write_len_header_to_vec(&mut bytes, u64::try_from(value.nonce().len()).unwrap());
    bytes.extend_from_slice(value.nonce());
    append_field(&mut bytes, value.request());
    bytes
}

fn signed_context_and_rejections() {
    let factories: [fn() -> SignedQuery; 3] = [signed_singular, signed_start, signed_continue];
    for make in factories {
        let valid = make();
        let versioned = valid.encode_versioned();
        for length in 0..versioned.len() {
            assert!(SignedQuery::decode_all_versioned(&versioned[..length]).is_err());
        }
        let mut wrong_version = versioned.clone();
        wrong_version[0] = 2;
        assert!(SignedQuery::decode_all_versioned(&wrong_version).is_err());
        let mut trailing = versioned;
        trailing.push(0);
        assert!(SignedQuery::decode_all_versioned(&trailing).is_err());
        // Decoding is structural. Authentication must bind every context field,
        // while ingress separately owns network, freshness and replay admission.
        for field in 0..6 {
            let SignedQueryJson::Canonical(mut dto) = SignedQueryJson::from(&valid);
            match field {
                0 => {
                    dto.payload.network_id = NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
                        iroha_data_model::block::BlockHeader,
                    >::from_untyped_unchecked(
                        iroha_crypto::Hash::new(b"different query capture network"),
                    ))
                }
                1 => dto.payload.authority = AccountId::new(key(0x52).public_key().clone()),
                2 => dto.payload.creation_time_ms += 1,
                3 => {
                    dto.payload.time_to_live_ms =
                        NonZeroU64::new(dto.payload.time_to_live_ms.get() + 1).unwrap()
                }
                4 => dto.payload.nonce[0] ^= 1,
                5 => {
                    dto.payload.request = if matches!(valid.request(), QueryRequest::Continue(_)) {
                        query_request_to_json(request_singular().request())
                    } else {
                        query_request_to_json(request_continue().request())
                    }
                }
                _ => unreachable!(),
            }
            let changed = SignedQuery::try_from(SignedQueryJson::Canonical(dto)).unwrap();
            assert_ne!(changed.encode(), valid.encode());
            assert!(changed.verify_signature().is_err());
            let decoded: SignedQuery =
                norito::decode_canonical(&norito::encode_canonical(&changed).unwrap()).unwrap();
            assert_eq!(decoded.encode(), changed.encode());
            assert!(decoded.verify_signature().is_err());
            let decoded = SignedQuery::decode_all_versioned(&changed.encode_versioned()).unwrap();
            assert_eq!(decoded.encode(), changed.encode());
            assert!(decoded.verify_signature().is_err());
        }
    }
    let factories: [fn() -> QueryRequestWithAuthority; 3] =
        [request_singular, request_start, request_continue];
    for make in factories {
        let value = make();
        assert_eq!(
            request_payload_with_ttl(&value, value.time_to_live_ms().get()),
            value.encode(),
            "manual malformed fixture has exactly the public field layout"
        );
        reject_payload::<QueryRequestWithAuthority>(&request_payload_with_ttl(&value, 0));
        let mut dto = request_json(&value);
        let valid_json = norito::json::to_value(&dto).unwrap();
        let mut zero_ttl = valid_json.clone();
        zero_ttl
            .as_object_mut()
            .unwrap()
            .insert("time_to_live_ms".into(), Value::from(0_u64));
        assert!(norito::json::from_value::<QueryRequestWithAuthorityJson>(zero_ttl).is_err());
        dto.nonce[0] ^= 1;
        assert_ne!(norito::json::to_value(&dto).unwrap(), valid_json);
    }
}

/// Construct a valid one-column, two-row query batch.
pub fn one_column() -> QueryOutputBatchBoxTuple {
    QueryOutputBatchBoxTuple::new(vec![QueryOutputBatchBox::String(vec![
        "first".into(),
        "second".into(),
    ])])
    .unwrap()
}
/// Construct a valid two-column, two-row query batch.
pub fn two_columns() -> QueryOutputBatchBoxTuple {
    QueryOutputBatchBoxTuple::new(vec![
        QueryOutputBatchBox::String(vec!["first".into(), "second".into()]),
        QueryOutputBatchBox::Name(vec!["alpha".parse().unwrap(), "beta".parse().unwrap()]),
    ])
    .unwrap()
}
/// Construct a valid two-column batch with no rows.
pub fn empty_rows() -> QueryOutputBatchBoxTuple {
    QueryOutputBatchBoxTuple::new(vec![
        QueryOutputBatchBox::String(Vec::new()),
        QueryOutputBatchBox::Name(Vec::new()),
    ])
    .unwrap()
}
fn inspect_batch(value: &QueryOutputBatchBoxTuple) -> Value {
    assert!(value.column_count() > 0);
    assert_eq!(value.column_count(), value.columns().len());
    for column in value.columns() {
        assert_eq!(column.len(), value.len());
    }
    assert_eq!(
        value.is_empty(),
        value.columns().iter().all(QueryOutputBatchBox::is_empty)
    );
    norito::json!({ "json": (checked_json(value)), "columns": (value.column_count()), "rows": (value.len()) })
}
fn batch_payload(columns: &Vec<QueryOutputBatchBox>) -> Vec<u8> {
    let raw = columns.encode();
    let mut input = raw.as_slice();
    assert_eq!(
        &Vec::<QueryOutputBatchBox>::decode_all(&mut input).unwrap(),
        columns
    );
    assert!(
        input.is_empty(),
        "malformed batch controls contain complete valid columns"
    );
    let mut payload = Vec::new();
    append_field(&mut payload, columns);
    payload
}
fn batch_rejections() {
    assert_eq!(
        QueryOutputBatchBoxTuple::from_batch(one_column().into_columns().remove(0)),
        one_column()
    );
    let valid = two_columns();
    assert_eq!(batch_payload(&valid.columns().to_vec()), valid.encode());
    for columns in [
        Vec::new(),
        vec![
            QueryOutputBatchBox::String(vec!["one".into()]),
            QueryOutputBatchBox::Name(Vec::new()),
        ],
        vec![
            QueryOutputBatchBox::String(vec!["one".into()]),
            QueryOutputBatchBox::Name(vec!["one".parse().unwrap()]),
            QueryOutputBatchBox::String(Vec::new()),
        ],
    ] {
        assert!(QueryOutputBatchBoxTuple::new(columns.clone()).is_err());
        reject_payload::<QueryOutputBatchBoxTuple>(&batch_payload(&columns));
        let json = norito::json!({ "tuple": columns });
        assert!(norito::json::from_value::<QueryOutputBatchBoxTuple>(json).is_err());
    }
    let mut value = two_columns();
    let before = value.encode();
    assert!(matches!(
        value.extend(one_column()),
        Err(QueryOutputBatchBoxTupleError::ColumnCountMismatch { .. })
    ));
    assert_eq!(value.encode(), before);
    let wrong_late_type = QueryOutputBatchBoxTuple::new(vec![
        QueryOutputBatchBox::String(vec!["third".into()]),
        QueryOutputBatchBox::String(vec!["third".into()]),
    ])
    .unwrap();
    assert!(matches!(
        value.extend(wrong_late_type),
        Err(QueryOutputBatchBoxTupleError::ColumnTypeMismatch { column: 1 })
    ));
    assert_eq!(
        value.encode(),
        before,
        "late type failure cannot partially append column zero"
    );
    value.extend(two_columns()).unwrap();
    assert_eq!(value.len(), 4);
    assert_eq!(value.column_count(), 2);
    let expected = QueryOutputBatchBoxTuple::new(vec![
        QueryOutputBatchBox::String(vec![
            "first".into(),
            "second".into(),
            "first".into(),
            "second".into(),
        ]),
        QueryOutputBatchBox::Name(vec![
            "alpha".parse().unwrap(),
            "beta".parse().unwrap(),
            "alpha".parse().unwrap(),
            "beta".parse().unwrap(),
        ]),
    ])
    .unwrap();
    assert_eq!(value, expected);
    value.extend(empty_rows()).unwrap();
    assert_eq!(value, expected);
}

/// Construct a singular parameters response through its public enum.
pub fn response_singular() -> QueryResponse {
    QueryResponse::Singular(SingularQueryOutputBox::Parameters(Parameters::default()))
}
/// Construct an iterable response carrying an exact remaining count.
pub fn response_exact() -> QueryResponse {
    QueryResponse::Iterable(QueryOutput::new(two_columns(), 3, Some(cursor())))
}
/// Construct an iterable response with an intentionally omitted count.
pub fn response_bounded() -> QueryResponse {
    QueryResponse::Iterable(QueryOutput::new_bounded(empty_rows(), false, None))
}
fn inspect_response(value: &QueryResponse) -> Value {
    let details = match value {
        QueryResponse::Singular(SingularQueryOutputBox::Parameters(parameters)) => {
            assert_eq!(parameters, &Parameters::default());
            Value::String("singular parameters".into())
        }
        QueryResponse::Singular(_) => panic!("unexpected singular fixture"),
        QueryResponse::Iterable(output) => {
            let (batch, remaining, has_more, cursor) = output.clone().into_parts_with_count_mode();
            if batch.is_empty() {
                assert_eq!(remaining, None);
                assert!(!has_more);
                assert!(cursor.is_none());
            } else {
                assert_eq!(remaining, Some(3));
                assert!(has_more);
                assert!(cursor.is_some());
            }
            norito::json!({ "batch": (inspect_batch(&batch)), "remaining": remaining, "has_more": has_more, "cursor": cursor })
        }
    };
    norito::json!({ "json": (checked_json(value)), "parts": details })
}

fn time_zero() -> TimeInterval {
    TimeInterval::new(Duration::ZERO, Duration::ZERO)
}
fn time_ordinary() -> TimeInterval {
    TimeInterval::new(Duration::from_millis(10), Duration::from_millis(20))
}
fn time_large() -> TimeInterval {
    TimeInterval::new(
        Duration::from_millis(u64::MAX),
        Duration::from_millis(u64::MAX),
    )
}
fn inspect_time(value: &TimeInterval) -> Value {
    assert_eq!(TimeInterval::new(value.since(), value.length()), *value);
    norito::json!({ "json": (checked_json(value)), "since_ms": (u64::try_from(value.since().as_millis()).unwrap()), "length_ms": (u64::try_from(value.length().as_millis()).unwrap()) })
}
fn time_rejections() {
    for json in [
        r"{}",
        r#"{"since_ms":0}"#,
        r#"{"length_ms":0}"#,
        r#"{"since_ms":-1,"length_ms":0}"#,
        r#"{"since_ms":0,"length_ms":-1}"#,
        r#"{"since_ms":0,"length_ms":0,"extra":0}"#,
    ] {
        assert!(norito::json::from_json::<TimeInterval>(json).is_err());
    }
    let ordinary = time_ordinary();
    assert_eq!(
        TimeInterval::new_since_to(Duration::from_millis(10), Duration::from_millis(30)),
        ordinary
    );
}

fn capture_values() -> Vec<Value> {
    let mut rows = Vec::new();
    family(
        &mut rows,
        "query_box",
        &[domain_query, account_query],
        inspect_query,
    );
    family(
        &mut rows,
        "query_signature",
        &[signature_singular, signature_continue],
        inspect_signature,
    );
    family(
        &mut rows,
        "query_request_with_authority",
        &[request_singular, request_start, request_continue],
        inspect_request,
    );
    family(
        &mut rows,
        "signed_query",
        &[signed_singular, signed_start, signed_continue],
        inspect_signed,
    );
    family(
        &mut rows,
        "query_output_batch_box_tuple",
        &[one_column, two_columns, empty_rows],
        inspect_batch,
    );
    family(
        &mut rows,
        "query_response",
        &[response_singular, response_exact, response_bounded],
        inspect_response,
    );
    family(
        &mut rows,
        "time_interval",
        &[time_zero, time_ordinary, time_large],
        inspect_time,
    );
    query_rejections();
    signature_projection_and_rejections();
    signed_context_and_rejections();
    batch_rejections();
    time_rejections();
    assert_eq!(rows.len(), 59);
    rows
}

#[test]
fn public_query_and_time_frames_match_capture() {
    let evidence = norito::json!({
        "format_version": 1,
        "purpose": "public query and time owners before identity declaration",
        "default_encode_flags": (ncore::default_encode_flags()),
        "rows": (capture_values()),
    });
    let captured: Value = norito::json::from_json(include_str!(
        "../fixtures/query_manual_identity_frames.json"
    ))
    .expect("decode immutable pre-declaration capture");
    assert_eq!(evidence, captured);
}
