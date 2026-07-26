#![no_main]
use libfuzzer_sys::fuzz_target;

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::NoritoSerialize,
    norito::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
struct Inner {
    count: u32,
    title: String,
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    norito::NoritoSerialize,
    norito::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
struct Outer {
    id: u64,
    name: String,
    active: bool,
    ratio: f64,
    tags: Vec<String>,
    nested: Option<Inner>,
    nums: Vec<u64>,
}

fuzz_target!(|data: &[u8]| {
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };

    let fast = match norito::json::from_json_fast::<Outer>(input) {
        Ok(v) => v,
        Err(_) => return,
    };

    // Generic parser must agree with the fast path on successful parses.
    let generic = norito::json::from_json::<Outer>(input).expect("generic parser");
    assert_eq!(fast, generic);

    // Round-trip via canonical writer.
    let encoded = norito::json::to_json(&fast).expect("encode");
    let reparsed = norito::json::from_json_fast::<Outer>(&encoded).expect("reparse");
    assert_eq!(fast, reparsed);

    // Writer-based encoding should match `to_json_fast` for the same value.
    let fast_encoded = norito::json::to_json_fast(&fast).expect("encode fast");
    assert_eq!(encoded, fast_encoded);
});
