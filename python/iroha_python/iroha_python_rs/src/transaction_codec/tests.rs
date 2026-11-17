//! Transaction layout parity at the native Python API boundary.

use super::*;
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use iroha_crypto::{Algorithm, KeyPair};
use pyo3::types::{PyAnyMethods, PyBytesMethods};

fn fixtures(name: &str) -> Vec<json::Value> {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/norito_rpc");
    let value: json::Value =
        json::from_str(&std::fs::read_to_string(root.join(name)).unwrap()).unwrap();
    value["fixtures"].as_array().unwrap().clone()
}

fn bytes(entry: &json::Value, key: &str) -> Vec<u8> {
    BASE64.decode(entry[key].as_str().unwrap()).unwrap()
}

#[test]
fn every_transaction_fixture_uses_the_existing_typed_codec_and_dense_signer() {
    Python::initialize();
    let current = fixtures("transaction_fixtures.manifest.json");
    let dense = fixtures("transaction_fixtures.dense.json");
    let key = KeyPair::try_from_seed(
        b"android-fixture-signing-key-0102".to_vec(),
        Algorithm::Ed25519,
    )
    .unwrap();
    let (_, private_key) = key.private_key().to_bytes();
    assert_eq!(current.len(), dense.len());
    assert!(!dense.is_empty());
    Python::attach(|py| {
        let module = PyModule::new(py, "transaction_layout_tests").unwrap();
        register(&module).unwrap();
        assert!(
            module
                .hasattr("decode_signed_transaction_with_layout")
                .unwrap()
        );
        for entry in &dense {
            let name = entry["name"].as_str().unwrap();
            let old = current
                .iter()
                .find(|old| old["name"] == entry["name"])
                .unwrap();
            let payload = bytes(entry, "payload_base64");
            let signed = bytes(entry, "signed_base64");
            let payload_json =
                decode_transaction_payload_with_layout(&payload, "dense", 753, false)
                    .unwrap_or_else(|error| panic!("{name}: {error}"));
            assert_eq!(
                payload_json,
                decode_transaction_payload_with_layout(
                    &bytes(old, "payload_base64"),
                    "current",
                    753,
                    true
                )
                .unwrap(),
                "{name}"
            );
            assert_eq!(
                encode_transaction_payload_with_layout(py, &payload_json, "dense", 753, false)
                    .unwrap()
                    .bind(py)
                    .as_bytes(),
                payload
            );
            assert_eq!(
                encode_transaction_payload_with_layout(py, &payload_json, "current", 753, true)
                    .unwrap()
                    .bind(py)
                    .as_bytes(),
                bytes(old, "payload_base64")
            );
            let signed_json =
                decode_signed_transaction_with_layout(&signed, "dense", 753, false).unwrap();
            assert_eq!(
                encode_signed_transaction_with_layout(py, &signed_json, "dense", 753, false)
                    .unwrap()
                    .bind(py)
                    .as_bytes(),
                signed
            );
            let old_signed_json = decode_signed_transaction_with_layout(
                &bytes(old, "signed_base64"),
                "current",
                753,
                true,
            )
            .unwrap();
            assert_eq!(
                encode_signed_transaction_with_layout(py, &old_signed_json, "current", 753, true)
                    .unwrap()
                    .bind(py)
                    .as_bytes(),
                bytes(old, "signed_base64")
            );
            let hash = transaction_payload_hash_with_layout(py, &payload, "dense", false).unwrap();
            assert_eq!(
                hex::encode(hash.bind(py).as_bytes()),
                entry["signing_hash"].as_str().unwrap()
            );
            assert_eq!(
                sign_transaction_payload_with_layout(py, &payload, &private_key, "dense", false)
                    .unwrap()
                    .bind(py)
                    .as_bytes(),
                signed
            );
            let model: SignedTransaction = decode(&signed, WireLayout::Dense, false).unwrap();
            model
                .signature()
                .payload()
                .verify(key.public_key(), hash.bind(py).as_bytes())
                .unwrap();
        }
    });
}

#[test]
fn typed_layout_readers_refuse_wrong_layout_and_noncanonical_frames() {
    Python::initialize();
    let entry = &fixtures("transaction_fixtures.dense.json")[0];
    let bare = bytes(entry, "signed_base64");
    let value: SignedTransaction = decode(&bare, WireLayout::Dense, false).unwrap();
    for selected in [WireLayout::Compact, WireLayout::Dense] {
        let frame = encode(&value, selected, true).unwrap();
        let decoded: SignedTransaction = decode(&frame, selected, true).unwrap();
        assert_eq!(decoded, value);
        let opposite = if selected == WireLayout::Dense {
            WireLayout::Compact
        } else {
            WireLayout::Dense
        };
        assert!(decode::<SignedTransaction>(&frame, opposite, true).is_err());
        assert!(decode::<SignedTransaction>(&frame[..frame.len() - 1], selected, true).is_err());
        let mut trailing = frame.clone();
        trailing.push(0);
        assert!(decode::<SignedTransaction>(&trailing, selected, true).is_err());
        let mut unknown = frame;
        unknown[5] = 2;
        assert!(decode::<SignedTransaction>(&unknown, selected, true).is_err());
    }
    assert!(decode::<SignedTransaction>(&bare, WireLayout::Compact, false).is_err());
    assert!(
        decode::<SignedTransaction>(&bare[..bare.len() - 1], WireLayout::Dense, false).is_err()
    );
    assert!(layout("compact").is_err());
    assert!(layout(" Dense").is_err());
    assert!(layout("unknown").is_err());
}

#[test]
fn python_builder_layout_methods_keep_current_bytes_and_sign_dense_payloads() {
    Python::initialize();
    Python::attach(|py| {
        let key = super::super::parse_private_key(&[0x51; 32]).unwrap();
        let authority =
            iroha_data_model::account::AccountId::new(iroha_crypto::PublicKey::from(key));
        let network = super::super::PyNetworkId::from_exact_bytes(&[0xA5; 32]).unwrap();
        let mut builder = super::super::TransactionBuilder::new(
            &network,
            &authority.to_i105_for_discriminant(753).unwrap(),
            r#"{"payer":"authority","value":{"charge_limits":[],"gas_limit":null}}"#,
            753,
        )
        .unwrap();
        builder.set_creation_time_ms(42).unwrap();
        builder.set_ttl_ms(30_000).unwrap();
        builder.add_instruction_json(&format!(
            r#"{{"name":"SetAssetHoldingLimit","params":{{"account_id":"{}","asset_definition_id":"66owaQmAQMuHxPzxUN3bqZ6FJfDa","holding_limit":"100"}}}}"#,
            authority.to_i105_for_discriminant(753).unwrap()
        )).unwrap();
        assert_eq!(
            builder.encode_payload(py).unwrap().as_bytes(),
            builder
                .encode_payload_with_layout(py, "current")
                .unwrap()
                .as_bytes()
        );
        assert_eq!(
            builder.payload_hash(py).unwrap().as_bytes(),
            builder
                .payload_hash_with_layout(py, "current")
                .unwrap()
                .as_bytes()
        );
        let payload = builder.encode_payload_with_layout(py, "dense").unwrap();
        let expected = sign_transaction_payload_with_layout(
            py,
            payload.as_bytes(),
            &[0x51; 32],
            "dense",
            false,
        )
        .unwrap();
        let hash =
            transaction_payload_hash_with_layout(py, payload.as_bytes(), "dense", false).unwrap();
        assert_eq!(
            builder
                .payload_hash_with_layout(py, "dense")
                .unwrap()
                .as_bytes(),
            hash.bind(py).as_bytes()
        );
        assert!(
            builder
                .sign_with_layout(py, &[0x51; 32], "unknown")
                .is_err()
        );
        assert_eq!(
            builder
                .encode_payload_with_layout(py, "dense")
                .unwrap()
                .as_bytes(),
            payload.as_bytes()
        );
        assert_eq!(
            builder
                .sign_with_layout(py, &[0x51; 32], "dense")
                .unwrap()
                .as_bytes(),
            expected.bind(py).as_bytes()
        );
        assert!(builder.executable_items.is_empty());
    });
}
