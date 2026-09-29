use norito::{
    NoritoDeserialize, NoritoSerialize, SerializePayload,
    core::{self as norito_core, DecodeFlagsGuard, DecodeFromSlice, Header},
};
#[derive(Debug, Clone, PartialEq, NoritoSerialize, NoritoDeserialize)]
#[norito(decode_from_slice)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "norito.test.flatten.InnerSelector")]
struct InnerSelector {
    first: Option<u32>,
    second: Option<String>,
}
#[derive(Debug, Clone, PartialEq, NoritoSerialize, NoritoDeserialize)]
#[norito(decode_from_slice)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "norito.test.flatten.OuterRequest")]
struct OuterRequest {
    #[norito(flatten)]
    selector: InnerSelector,
    signer: String,
    gas_limit: Option<u64>,
}
fn sample_request() -> OuterRequest {
    OuterRequest {
        selector: InnerSelector {
            first: Some(7),
            second: Some("hbl.sbp".to_owned()),
        },
        signer: "signer-i105".to_owned(),
        gas_limit: Some(10_000),
    }
}
fn bare_payload_with_flags<T: NoritoSerialize>(value: &T, flags: u8) -> Vec<u8> {
    let _guard = DecodeFlagsGuard::enter(flags);
    let mut payload = Vec::new();
    norito_core::serialize_to_buffer(value, &mut payload).expect("serialize bare payload");
    payload
}
#[test]
fn flattened_struct_fields_are_binary_inline() {
    let request = sample_request();
    let bytes = norito::to_bytes(&request).expect("encode request");
    let view = norito_core::from_bytes_view(&bytes).expect("payload view");
    let flags = view.flags();
    let payload = view.as_bytes();
    let selector_payload = bare_payload_with_flags(&request.selector, flags);
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
    let _guard = DecodeFlagsGuard::enter(flags);
    let (selector, used) =
        <InnerSelector as DecodeFromSlice>::decode_from_slice(payload).expect("prefix selector");
    assert_eq!(selector, request.selector);
    assert_eq!(used, selector_payload.len());
    let decoded: OuterRequest = norito::decode_from_bytes(&bytes).expect("decode request");
    assert_eq!(decoded, request);
}
#[test]
fn flattened_fields_follow_the_fixed_width_layout_of_their_frame() {
    let request = sample_request();
    let bytes = {
        let _guard = DecodeFlagsGuard::enter(0);
        norito::to_bytes(&request).expect("encode fixed-width request")
    };
    assert_eq!(
        bytes[Header::SIZE - 1],
        0,
        "a flattened field must not promote a fixed-width frame to COMPACT_LEN"
    );
    let view = norito_core::from_bytes_view(&bytes).expect("payload view");
    assert_eq!(view.flags(), 0);
    let payload = view.as_bytes();
    let selector_payload = bare_payload_with_flags(&request.selector, 0);
    assert_eq!(
        payload.get(..selector_payload.len()),
        Some(selector_payload.as_slice()),
        "the flattened selector must use the frame's fixed-width prefixes"
    );
    {
        let _guard = DecodeFlagsGuard::enter(0);
        assert_eq!(
            request.encoded_len_exact(),
            Some(payload.len()),
            "exact length must match the fixed-width flattened payload"
        );
    }
    let decoded: OuterRequest = norito::decode_from_bytes(&bytes).expect("decode request");
    assert_eq!(decoded, request);
}
