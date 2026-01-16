use std::io::Cursor;

use norito::{core::Error, deserialize_from, to_bytes};

#[test]
fn truncated_payload_is_rejected() {
    let bytes = to_bytes(&"safe".to_string()).expect("serialize");
    let mut truncated = bytes.clone();
    truncated.pop();

    let err =
        deserialize_from::<_, String>(Cursor::new(&truncated)).expect_err("truncate should fail");
    assert!(
        matches!(err, Error::LengthMismatch),
        "unexpected error: {err:?}"
    );
}

#[test]
fn packed_sequences_use_fixed_headers() {
    use std::collections::BTreeSet;

    let mut validators = BTreeSet::new();
    validators.insert(42u64);
    let encoded = norito::codec::Encode::encode(&validators);

    assert_eq!(
        &encoded[..8],
        &1u64.to_le_bytes(),
        "expected fixed-length header for len=1 set"
    );
}
