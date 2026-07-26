use norito::{
    NoritoDeserialize, NoritoSerialize,
    core::{self as norito_core, DecodeFlagsGuard, DecodeFromSlice, header_flags},
};

#[derive(Debug, Clone, PartialEq, NoritoSerialize, NoritoDeserialize)]
#[norito(decode_from_slice)]
struct InnerSelector {
    first: Option<u32>,
    second: Option<String>,
}

#[derive(Debug, Clone, PartialEq, NoritoSerialize, NoritoDeserialize)]
#[norito(decode_from_slice)]
struct OuterRequest {
    #[norito(flatten)]
    selector: InnerSelector,
    signer: String,
    gas_limit: Option<u64>,
}

fn bare_payload_with_flags<T: NoritoSerialize>(value: &T, flags: u8, flags_hint: u8) -> Vec<u8> {
    let _guard = DecodeFlagsGuard::enter_with_hint(flags, flags_hint);
    let mut payload = Vec::new();
    value
        .serialize(&mut payload)
        .expect("serialize bare payload");
    payload
}

fn sequential_bare_payload_with_flags<T: NoritoSerialize>(
    value: &T,
    flags: u8,
    flags_hint: u8,
) -> Vec<u8> {
    let _guard = DecodeFlagsGuard::enter_with_hint(flags, flags_hint);
    let _sequential = norito_core::SequentialOverrideGuard::enter();
    let mut payload = Vec::new();
    value
        .serialize(&mut payload)
        .expect("serialize bare payload");
    payload
}

#[test]
fn flattened_struct_fields_are_binary_inline() {
    let request = OuterRequest {
        selector: InnerSelector {
            first: Some(7),
            second: Some("hbl.sbp".to_owned()),
        },
        signer: "signer-i105".to_owned(),
        gas_limit: Some(10_000),
    };

    let bytes = norito::to_bytes(&request).expect("encode request");
    let view = norito_core::from_bytes_view(&bytes).expect("payload view");
    let flags = view.flags();
    let flags_hint = view.flags_hint();
    let payload = view.as_bytes();
    let selector_payload = bare_payload_with_flags(&request.selector, flags, flags_hint);

    assert_eq!(
        payload.get(..selector_payload.len()),
        Some(selector_payload.as_slice()),
        "flattened selector must not be wrapped in an outer field length"
    );
    assert_eq!(
        request.encoded_len_exact(),
        Some(payload.len()),
        "exact length must match the flattened wire payload"
    );

    let _guard = DecodeFlagsGuard::enter_with_hint(flags, flags_hint);
    let (selector, used) =
        <InnerSelector as DecodeFromSlice>::decode_from_slice(payload).expect("prefix selector");
    assert_eq!(selector, request.selector);
    assert_eq!(used, selector_payload.len());

    let decoded: OuterRequest = norito::decode_from_bytes(&bytes).expect("decode request");
    assert_eq!(decoded, request);
}

#[test]
fn flattened_struct_uses_sequential_layout_even_when_packed_struct_is_requested() {
    let request = OuterRequest {
        selector: InnerSelector {
            first: None,
            second: Some("ubl.sbp".to_owned()),
        },
        signer: "operator-i105".to_owned(),
        gas_limit: None,
    };
    let flags =
        header_flags::PACKED_STRUCT | header_flags::COMPACT_LEN | header_flags::FIELD_BITSET;

    let payload = bare_payload_with_flags(&request, flags, flags);
    let selector_payload = sequential_bare_payload_with_flags(&request.selector, flags, flags);
    assert_eq!(
        payload.get(..selector_payload.len()),
        Some(selector_payload.as_slice()),
        "packed-struct mode must not introduce a synthetic slot for a flattened field"
    );

    let framed =
        norito_core::frame_bare_with_header_flags::<OuterRequest>(&payload, flags).expect("frame");
    let decoded: OuterRequest = norito::decode_from_bytes(&framed).expect("decode packed request");
    assert_eq!(decoded, request);
}
