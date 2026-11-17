//! Native JavaScript boundary parity for every committed transaction layout fixture.

use super::*;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use iroha_crypto::KeyPair;

fn fixtures(name: &str) -> Vec<json::Value> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/norito_rpc")
        .join(name);
    let value: json::Value = json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    value["fixtures"].as_array().unwrap().clone()
}

fn fixture_bytes(entry: &json::Value, key: &str) -> Vec<u8> {
    STANDARD.decode(entry[key].as_str().unwrap()).unwrap()
}

fn text(value: &str) -> Option<String> {
    Some(value.to_owned())
}

#[test]
fn all_current_and_dense_fixtures_roundtrip_through_typed_native_json_and_signing() {
    let _network = ChainDiscriminantGuard::enter(753);
    let current = fixtures("transaction_fixtures.manifest.json");
    let dense = fixtures("transaction_fixtures.dense.json");
    assert_eq!(dense.len(), 27);
    assert_eq!(current.len(), dense.len());
    let key = KeyPair::try_from_seed(
        b"android-fixture-signing-key-0102".to_vec(),
        Algorithm::Ed25519,
    )
    .unwrap();
    let (_, private_key) = key.private_key().to_bytes();
    let (_, public_key) = key.public_key().try_to_bytes().unwrap();
    for entry in &dense {
        let name = entry["name"].as_str().unwrap();
        let old = current
            .iter()
            .find(|old| old["name"] == entry["name"])
            .unwrap();
        let dense_payload = fixture_bytes(entry, "payload_base64");
        let dense_signed = fixture_bytes(entry, "signed_base64");
        let current_payload = fixture_bytes(old, "payload_base64");
        let current_signed = fixture_bytes(old, "signed_base64");
        let dense_json = crate::decode_transaction_payload_json(
            Uint8Array::from(dense_payload.clone()),
            753.0,
            text("dense"),
            text("bare"),
        )
        .unwrap_or_else(|error| panic!("{name}: {error}"));
        let current_json = crate::decode_transaction_payload_json(
            Uint8Array::from(current_payload.clone()),
            753.0,
            text("current"),
            text("framed"),
        )
        .unwrap();
        assert_eq!(dense_json, current_json, "{name}: semantic payload drift");
        for (selected, framing, payload_bytes, signed_bytes) in [
            ("dense", "bare", &dense_payload, &dense_signed),
            ("current", "framed", &current_payload, &current_signed),
        ] {
            assert_eq!(
                encode_transaction_payload_json(
                    dense_json.clone(),
                    753.0,
                    text(selected),
                    text(framing)
                )
                .unwrap()
                .as_ref(),
                payload_bytes,
                "{name}"
            );
            let signed_json = crate::decode_signed_transaction_json(
                Uint8Array::from(signed_bytes.clone()),
                753.0,
                text(selected),
                text(framing),
            )
            .unwrap();
            assert_eq!(
                encode_signed_transaction_json(signed_json, 753.0, text(selected), text(framing))
                    .unwrap()
                    .as_ref(),
                signed_bytes,
                "{name}"
            );
            let options = Options::payload(Some(selected), Some(framing)).unwrap();
            let builder = decode_payload(payload_bytes, options).unwrap();
            let network_id = builder.payload().network_id().unwrap();
            // Exercise the existing ergonomic instruction adapters only for fixtures
            // whose variants have that separate strict JSON contract. Every fixture
            // still traverses the typed model JSON codec and signing paths below.
            if matches!(name, "transfer_asset" | "register_asset_definition") {
                assert_instruction_builders(
                    name,
                    builder.payload(),
                    key.private_key(),
                    selected,
                    framing,
                    signed_bytes,
                );
            }
            let resigned = crate::sign_transaction(
                Uint8Array::from(network_id.as_bytes().to_vec()),
                Uint8Array::from(signed_bytes.clone()),
                Uint8Array::from(private_key.clone()),
                text(selected),
                text(framing),
            )
            .unwrap();
            assert_eq!(resigned.as_ref(), signed_bytes, "{name}: re-signing parity");
            let signed_again = sign_transaction_payload(
                Uint8Array::from(network_id.as_bytes().to_vec()),
                Uint8Array::from(payload_bytes.clone()),
                Uint8Array::from(private_key.clone()),
                753.0,
                text(selected),
                text(framing),
            )
            .unwrap();
            assert_eq!(
                signed_again.as_ref(),
                signed_bytes,
                "{name}: signing parity"
            );
            let signing_hash = crate::transaction_payload_signing_hash(
                Uint8Array::from(payload_bytes.clone()),
                text(selected),
                text(framing),
            )
            .unwrap();
            let signed_hash = crate::hash_signed_transaction_payload(
                Uint8Array::from(signed_bytes.clone()),
                text(selected),
                text(framing),
            )
            .unwrap();
            assert_eq!(signing_hash.as_ref(), signed_hash.as_ref(), "{name}");
            if selected == "dense" {
                assert_eq!(
                    hex::encode(signing_hash.as_ref()),
                    entry["signing_hash"].as_str().unwrap(),
                    "{name}"
                );
                let tx = decode_signed(signed_bytes, options).unwrap();
                tx.signature()
                    .payload()
                    .verify(key.public_key(), signing_hash.as_ref())
                    .unwrap();
                let finalized = crate::finalize_signed_transaction(
                    crate::JsExternalTransactionSignature {
                        network_id: Buffer::from(network_id.as_bytes().to_vec()),
                        payload_bytes: Buffer::from(payload_bytes.clone()),
                        payload_hash_hex: Some(hex::encode(signing_hash.as_ref())),
                        signature: Buffer::from(tx.signature().payload().payload().to_vec()),
                        public_key: Buffer::from(public_key.to_vec()),
                        authority: old["authority"].as_str().unwrap().to_owned(),
                    },
                    text("dense"),
                    text("bare"),
                )
                .unwrap();
                assert_eq!(
                    finalized.signed_transaction.as_ref(),
                    signed_bytes,
                    "{name}: external signature parity"
                );
                assert_eq!(
                    finalized.hash.as_ref(),
                    crate::hash_signed_transaction(
                        Uint8Array::from(signed_bytes.clone()),
                        text("dense"),
                        text("bare")
                    )
                    .unwrap()
                    .as_ref()
                );
            } else {
                let hash = crate::hash_signed_transaction(
                    Uint8Array::from(signed_bytes.clone()),
                    text("current"),
                    text("framed"),
                )
                .unwrap();
                assert_eq!(
                    hex::encode(hash.as_ref()),
                    old["signed_hash"].as_str().unwrap(),
                    "{name}: manifest intent hash changed"
                );
                let typed = decode_signed(signed_bytes, options).unwrap();
                assert_eq!(
                    hash.as_ref(),
                    typed.hash().as_ref(),
                    "{name}: current intent hash changed"
                );
                let versioned = typed.encode_wire_v1().unwrap();
                let default_json = crate::decode_signed_transaction_json(
                    Uint8Array::from(versioned.clone()),
                    753.0,
                    None,
                    None,
                )
                .unwrap();
                assert_eq!(
                    encode_signed_transaction_json(default_json, 753.0, None, None)
                        .unwrap()
                        .as_ref(),
                    versioned
                );
                assert_eq!(
                    crate::hash_signed_transaction(Uint8Array::from(versioned), None, None)
                        .unwrap()
                        .as_ref(),
                    hash.as_ref()
                );
            }
        }
    }
}

fn assert_instruction_builders(
    fixture_name: &str,
    payload: &TransactionPayload,
    key: &PrivateKey,
    selected: &str,
    framing: &str,
    signed_bytes: &[u8],
) {
    let iroha_data_model::transaction::Executable::Instructions(instructions) =
        &payload.instructions
    else {
        return;
    };
    let instructions_json: Vec<String> = instructions
        .iter()
        .map(|instruction| {
            let value =
                iroha_js_codec::instruction_to_json_value(instruction).unwrap_or_else(|error| {
                    panic!("{fixture_name}/{selected}: ergonomic instruction JSON: {error}")
                });
            json::to_json(&value).unwrap()
        })
        .collect();
    let authority =
        iroha_data_model::account::address::AccountAddress::from_account_id(&payload.authority)
            .unwrap()
            .to_i105_for_discriminant(753)
            .unwrap();
    let network = payload.network_id().unwrap().as_bytes().to_vec();
    let fee = json::to_json(&payload.fee_payment).unwrap();
    let metadata = Some(json::to_json(&payload.metadata).unwrap());
    let created = Some(i64::try_from(payload.creation_time_ms).unwrap());
    let ttl = payload
        .time_to_live_ms
        .map(|value| i64::try_from(value.get()).unwrap());
    let nonce = payload.nonce.map(std::num::NonZeroU32::get);
    let draft = crate::build_transaction_payload(
        Uint8Array::from(network.clone()),
        authority.clone(),
        instructions_json.clone(),
        fee.clone(),
        metadata.clone(),
        created,
        ttl,
        nonce,
        text(selected),
    )
    .unwrap();
    let bare = Options::payload(Some(selected), Some("bare")).unwrap();
    assert_eq!(
        draft.payload_bytes.as_ref(),
        encode_payload(payload, bare).unwrap()
    );
    assert_eq!(
        draft.payload_hash.as_ref(),
        payload_hash(payload, bare.layout).unwrap()
    );
    let (_, private_key) = key.to_bytes();
    let signed = crate::build_transaction(
        Uint8Array::from(network.clone()),
        authority,
        instructions_json,
        fee.clone(),
        metadata,
        created,
        ttl,
        nonce,
        Uint8Array::from(private_key.clone()),
        text("ed25519"),
        text(selected),
        text(framing),
    )
    .unwrap();
    assert_eq!(
        signed.signed_transaction.as_ref(),
        signed_bytes,
        "{fixture_name}/{selected}: ergonomic signed builder"
    );
    let quoted = crate::sign_quoted_transaction_payload(
        Uint8Array::from(network),
        draft.payload_json,
        fee,
        Uint8Array::from(private_key),
        text("ed25519"),
        text(selected),
        text(framing),
    )
    .unwrap();
    assert_eq!(
        quoted.signed_transaction.as_ref(),
        signed_bytes,
        "{fixture_name}/{selected}: quoted signer"
    );
}

#[test]
fn explicit_formats_reject_wrong_layout_unknown_minor_truncation_and_suffix() {
    let entry = fixtures("transaction_fixtures.dense.json").remove(0);
    let bare = fixture_bytes(&entry, "signed_base64");
    let dense = Options::signed(Some("dense"), Some("bare")).unwrap();
    let transaction = decode_signed(&bare, dense).unwrap();
    for ambient in [WireLayout::Compact, WireLayout::Dense] {
        let _caller = norito::core::DecodeFlagsGuard::enter(ambient.code());
        decode_signed(&bare, dense).unwrap();
        assert_eq!(norito::core::get_decode_flags(), ambient.code());
        assert!(decode_signed(&bare[..bare.len() - 1], dense).is_err());
        assert_eq!(norito::core::get_decode_flags(), ambient.code());
    }
    assert!(Options::signed(Some("dense"), None).is_err());
    assert!(Options::payload(None, Some("versioned")).is_err());
    assert!(Options::signed(None, Some("guess")).is_err());
    for invalid in ["compact", " Dense", "future"] {
        assert!(layout(Some(invalid)).is_err());
    }
    assert!(decode_signed(&bare, Options::signed(None, Some("bare")).unwrap()).is_err());
    for selected in ["current", "dense"] {
        let options = Options::signed(Some(selected), Some("framed")).unwrap();
        let frame = encode_signed(&transaction, options).unwrap();
        assert_eq!(decode_signed(&frame, options).unwrap(), transaction);
        let opposite = Options::signed(
            Some(if selected == "dense" {
                "current"
            } else {
                "dense"
            }),
            Some("framed"),
        )
        .unwrap();
        assert!(decode_signed(&frame, opposite).is_err());
        assert!(decode_signed(&frame[..frame.len() - 1], options).is_err());
        let mut trailing = frame.clone();
        trailing.push(0);
        assert!(decode_signed(&trailing, options).is_err());
        let mut unknown = frame;
        unknown[5] = 2;
        assert!(decode_signed(&unknown, options).is_err());
    }
    assert!(decode_signed(&bare[..bare.len() - 1], dense).is_err());
    assert!(decode_signed(&[bare.as_slice(), &[0]].concat(), dense).is_err());
    assert!(decode_signed(&vec![0; SIGNED_MAX_BYTES + 1], dense).is_err());
    assert!(encode_transaction_payload_json("".to_owned(), 753.0, None, None).is_err());
    assert!(
        encode_signed_transaction_json("{}".to_owned(), 753.0, text("dense"), text("bare"))
            .is_err()
    );
}

#[test]
fn native_instruction_exports_preserve_current_and_roundtrip_dense() {
    let current = fixtures("transaction_fixtures.manifest.json");
    let payload = fixture_bytes(&current[1], "payload_base64");
    let builder =
        decode_payload(&payload, Options::payload(None, Some("framed")).unwrap()).unwrap();
    let iroha_data_model::transaction::Executable::Instructions(instructions) =
        &builder.payload().instructions
    else {
        panic!("instruction fixture")
    };
    let json_value = iroha_js_codec::instruction_to_json_value(&instructions[0]).unwrap();
    let source = json::to_json(&json_value).unwrap();
    for selected in ["current", "dense"] {
        let frame =
            crate::norito_encode_instruction(source.clone(), 753.0, text(selected)).unwrap();
        let decoded = crate::norito_decode_instruction(
            Uint8Array::from(frame.as_ref().to_vec()),
            753.0,
            text(selected),
        )
        .unwrap();
        assert_eq!(
            crate::norito_encode_instruction(decoded.clone(), 753.0, text(selected))
                .unwrap()
                .as_ref(),
            frame.as_ref()
        );
        let bare =
            crate::norito_encode_instruction_box_archive(source.clone(), 753.0, text(selected))
                .unwrap();
        let decoded_bare = crate::norito_decode_instruction_box_archive(
            Uint8Array::from(bare.as_ref().to_vec()),
            753.0,
            text(selected),
        )
        .unwrap();
        assert_eq!(decoded, decoded_bare);
        assert_eq!(
            crate::norito_encode_instruction_box_archive(decoded_bare, 753.0, text(selected))
                .unwrap()
                .as_ref(),
            bare.as_ref()
        );
        assert!(
            crate::norito_decode_instruction(
                Uint8Array::from(frame.as_ref().to_vec()),
                753.0,
                text(if selected == "dense" {
                    "current"
                } else {
                    "dense"
                })
            )
            .is_err()
        );
    }
}

#[test]
fn dense_signer_binds_network_authority_and_exact_format() {
    let entry = fixtures("transaction_fixtures.dense.json").remove(0);
    let bytes = fixture_bytes(&entry, "payload_base64");
    let options = Options::payload(Some("dense"), None).unwrap();
    let builder = decode_payload(&bytes, options).unwrap();
    let network = builder.payload().network_id().unwrap().as_bytes().to_vec();
    let key = KeyPair::try_from_seed(
        b"android-fixture-signing-key-0102".to_vec(),
        Algorithm::Ed25519,
    )
    .unwrap();
    let (_, secret) = key.private_key().to_bytes();
    for (expected_network, selected_key, selected_layout, selected_format) in [
        (vec![1; 32], secret.clone(), "dense", "bare"),
        (network.clone(), vec![7; 32], "dense", "bare"),
        (network.clone(), secret.clone(), "current", "bare"),
        (network.clone(), secret.clone(), "dense", "framed"),
        (network.clone(), secret.clone(), "dense", "versioned"),
    ] {
        assert!(
            sign_transaction_payload(
                Uint8Array::from(expected_network),
                Uint8Array::from(bytes.clone()),
                Uint8Array::from(selected_key),
                753.0,
                text(selected_layout),
                text(selected_format),
            )
            .is_err()
        );
    }
    assert!(
        sign_transaction_payload(
            Uint8Array::from(network),
            Uint8Array::from(bytes),
            Uint8Array::from(secret),
            f64::NAN,
            text("dense"),
            text("bare"),
        )
        .is_err()
    );
}
