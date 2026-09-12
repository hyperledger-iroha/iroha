//! Captured public block-signature frames and authenticated signing inputs.

use norito::codec::Encode as _;
use std::num::NonZeroU64;

use crate::frame_identity_test_support::record;
use iroha_crypto::{Algorithm, KeyPair, SignatureOf};
use norito::json::Value;

use iroha_data_model::block::{BlockHeader, BlockSignature};

#[test]
fn block_signature_frames_match_capture() {
    let first_header = BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, None, 0, 0);
    let second_header = BlockHeader::new(
        NonZeroU64::new(2).unwrap(),
        Some(first_header.hash()),
        None,
        None,
        1_700_000_000_007,
        3,
    );
    let mut signatures = Vec::new();
    let mut signing_inputs = Vec::new();
    for (index, seed, header) in [(0, 0x39, first_header), (42, 0x57, second_header)] {
        // These public deterministic seeds follow the existing model identity fixtures.
        let signer = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("construct deterministic Ed25519 fixture key");
        let signature = SignatureOf::try_from_hash(signer.private_key(), header.hash())
            .expect("sign the actual block-header consensus projection");
        signature
            .verify_hash(signer.public_key(), header.hash())
            .expect("verify the constructed signature");
        let value = BlockSignature::new(index, signature.clone());
        assert_eq!(value.index(), index);
        assert_eq!(value.signature(), &signature);
        // Public framing delegates only its payload to this unchanged tuple layout.
        assert_eq!(
            value.encode(),
            (index, signature.payload().to_vec()).encode()
        );
        let frame = norito::encode_canonical(&value).expect("frame block signature");
        let decoded: BlockSignature = norito::decode_canonical(&frame).unwrap();
        decoded
            .signature()
            .verify_hash(signer.public_key(), header.hash())
            .expect("decoded signature authenticates the same header");
        signing_inputs.push(norito::json!({
            "validator_index": index,
            "public_seed_hex": (hex::encode([seed; 32])),
            "public_key": (signer.public_key().to_string()),
            "header_json": (norito::json::to_json(&header).unwrap()),
        }));
        signatures.push(value);
    }
    assert_ne!(signatures[0], signatures[1]);
    let mut rows = Vec::new();
    record(&mut rows, "root_first", &signatures[0]);
    record(&mut rows, "root_second", &signatures[1]);
    record(&mut rows, "option_none", &None::<BlockSignature>);
    record(&mut rows, "option_first", &Some(signatures[0].clone()));
    record(&mut rows, "option_second", &Some(signatures[1].clone()));
    record(&mut rows, "vec_empty", &Vec::<BlockSignature>::new());
    record(&mut rows, "vec_two", &signatures);
    assert_eq!(rows.len(), 7);
    let evidence = norito::json!({
        "format_version": 1,
        "purpose": "public BlockSignature pre-declaration capture",
        "default_encode_flags": (norito::core::default_encode_flags()),
        "signing_inputs": signing_inputs,
        "rows": rows,
    });
    let expected: Value = norito::json::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/block_signature_identity_frames.json"
    )))
    .expect("immutable pre-declaration BlockSignature capture");
    assert_eq!(evidence, expected);
}
