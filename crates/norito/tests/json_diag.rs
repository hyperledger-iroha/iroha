//! Diagnostic test to sanity-check Norito JSON parsing on sample payloads.

#[test]
fn diag_samples() {
    let samples = vec![
        "null",
        "true",
        "false",
        "0",
        "-0",
        "1",
        "-42",
        "3.1415",
        "1e-6",
        "-1.234e+10",
        "\"simple\"",
        "\"escapes\\n\\t\\\\\\\"\\\"\"",
        "\"unicode: \\u0416\"",
        "\"surrogate: \\uD834\\uDD1E\"",
        "[]",
        "[1,2,3,4,5]",
        "{}",
        "{\"a\":1,\"b\":true,\"c\":\"x\"}",
    ];
    for s in samples {
        let parsed =
            norito::json::from_str::<norito::json::Value>(s).expect("Norito JSON parse failure");
        let roundtrip = norito::json::to_string(&parsed).expect("Norito JSON serialize failure");
        eprintln!("s={s:?}\n  norito: {parsed:?}\n  roundtrip: {roundtrip:?}");
    }
}
