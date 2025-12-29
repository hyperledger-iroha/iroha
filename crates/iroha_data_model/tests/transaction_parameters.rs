//! Tests covering transaction parameter JSON encoding/decoding.
use iroha_data_model::parameter::TransactionParameters;
use nonzero_ext::nonzero;
use norito::json::{JsonDeserialize, JsonSerialize, Parser};

#[test]
fn transaction_parameters_json_serializes_max_signatures() {
    let defaults = TransactionParameters::default();
    let params = TransactionParameters::with_max_signatures(
        nonzero!(3_u64),
        nonzero!(4096_u64),
        nonzero!(4_194_304_u64),
        defaults.max_tx_bytes(),
        defaults.max_decompressed_bytes(),
        defaults.max_metadata_depth(),
    );
    let mut out = String::new();
    JsonSerialize::json_serialize(&params, &mut out);

    assert!(
        out.contains("\"max_signatures\":3"),
        "serialized parameters must encode max_signatures; got {out}"
    );
    assert!(
        out.contains("\"max_tx_bytes\""),
        "serialized parameters must include max_tx_bytes; got {out}"
    );
    assert!(
        out.contains("\"max_metadata_depth\""),
        "serialized parameters must include max_metadata_depth; got {out}"
    );
}

#[test]
fn transaction_parameters_json_defaults_missing_max_signatures() {
    let json = r#"{"max_instructions":4096,"ivm_bytecode_size":4194304}"#;
    let mut parser = Parser::new(json);
    let parsed =
        TransactionParameters::json_deserialize(&mut parser).expect("json parameters decode");

    assert_eq!(
        parsed.max_signatures(),
        TransactionParameters::default().max_signatures()
    );
    assert_eq!(
        parsed.max_tx_bytes(),
        TransactionParameters::default().max_tx_bytes()
    );
    assert_eq!(
        parsed.max_decompressed_bytes(),
        TransactionParameters::default().max_decompressed_bytes()
    );
    assert_eq!(
        parsed.max_metadata_depth(),
        TransactionParameters::default().max_metadata_depth()
    );
}
