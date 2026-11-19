//! Explicit-layout parity for every committed dense transaction fixture.

use std::ptr;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use iroha_crypto::{Algorithm, HashOf, KeyPair};
use iroha_data_model::account::address::ChainDiscriminantGuard;

use super::*;
use crate::{
    ERR_INSTRUCTION_PAYLOAD, ERR_INSTRUCTION_UNREGISTERED, ERR_NULL_PTR, ERR_TRANSACTION_ARCHIVE,
    ERR_WIRE_LAYOUT, connect_norito_free,
};

/// The signing seed every committed transaction fixture uses.
const FIXTURE_SEED: &[u8] = b"android-fixture-signing-key-0102";
/// The chain discriminant the fixtures render accounts with.
const FIXTURE_DISCRIMINANT: u16 = 753;

fn fixtures(name: &str) -> Vec<JsonValue> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/norito_rpc")
        .join(name);
    let value: JsonValue =
        norito::json::from_str(&std::fs::read_to_string(path).expect("fixture file"))
            .expect("fixture JSON");
    value["fixtures"].as_array().expect("fixture list").clone()
}

fn bytes(entry: &JsonValue, key: &str) -> Vec<u8> {
    STANDARD
        .decode(entry[key].as_str().expect("base64 field"))
        .expect("canonical base64")
}

fn fixture_key() -> KeyPair {
    KeyPair::try_from_seed(FIXTURE_SEED.to_vec(), Algorithm::Ed25519).expect("fixture key")
}

fn dense(format: c_uchar) -> Options {
    Options::signed(LAYOUT_DENSE, format).expect("dense selection")
}

fn current(format: c_uchar) -> Options {
    Options::signed(LAYOUT_CURRENT, format).expect("current selection")
}

/// The canonical literal of a payload's network, as the C signer takes it.
fn network_literal(network: &NetworkId) -> String {
    norito::json::to_value(network)
        .expect("network JSON")
        .as_str()
        .expect("network literal")
        .to_owned()
}

/// Call the C transcoder and return its status with any output.
fn transcode_ffi(wire_id: &str, payload: &[u8], from: c_uchar, to: c_uchar) -> (c_int, Vec<u8>) {
    let mut out_ptr: *mut c_uchar = ptr::dangling_mut();
    let mut out_len: c_ulong = 7;
    let status = unsafe {
        connect_norito_transcode_instruction_v1(
            wire_id.as_ptr().cast(),
            wire_id.len() as c_ulong,
            payload.as_ptr(),
            payload.len() as c_ulong,
            from,
            to,
            &mut out_ptr,
            &mut out_len,
        )
    };
    take_output(status, out_ptr, out_len)
}

fn take_output(status: c_int, out_ptr: *mut c_uchar, out_len: c_ulong) -> (c_int, Vec<u8>) {
    if status != 0 {
        assert!(out_ptr.is_null(), "a refusal clears the output pointer");
        assert_eq!(out_len, 0, "a refusal clears the output length");
        return (status, Vec::new());
    }
    if out_len == 0 {
        assert!(out_ptr.is_null(), "an empty output is a null pointer");
        return (status, Vec::new());
    }
    let output = unsafe { slice::from_raw_parts(out_ptr, out_len as usize) }.to_vec();
    connect_norito_free(out_ptr);
    (status, output)
}

fn decode_json_ffi(signed: &[u8], layout: c_uchar, format: c_uchar) -> (c_int, Vec<u8>) {
    let mut out_ptr: *mut c_uchar = ptr::dangling_mut();
    let mut out_len: c_ulong = 7;
    let status = unsafe {
        connect_norito_decode_signed_transaction_json_v1(
            signed.as_ptr(),
            signed.len() as c_ulong,
            layout,
            format,
            &mut out_ptr,
            &mut out_len,
        )
    };
    take_output(status, out_ptr, out_len)
}

fn sign_ffi(
    network: &str,
    payload: &[u8],
    layout: c_uchar,
    format: c_uchar,
    secret: &[u8],
) -> (c_int, Vec<u8>) {
    let mut out_ptr: *mut c_uchar = ptr::dangling_mut();
    let mut out_len: c_ulong = 7;
    let status = unsafe {
        connect_norito_sign_transaction_payload_v1(
            network.as_ptr().cast(),
            network.len() as c_ulong,
            payload.as_ptr(),
            payload.len() as c_ulong,
            layout,
            format,
            secret.as_ptr(),
            secret.len() as c_ulong,
            Algorithm::Ed25519 as u8,
            &mut out_ptr,
            &mut out_len,
        )
    };
    take_output(status, out_ptr, out_len)
}

/// Typed JSON of a signed transaction without its layout-bound signature.
fn json_without_signature(json: &[u8]) -> JsonValue {
    let mut value: JsonValue = norito::json::from_slice(json).expect("transaction JSON");
    value
        .as_object_mut()
        .expect("transaction object")
        .remove("signature")
        .expect("signature field");
    value
}

#[test]
fn every_dense_fixture_decodes_typed_reencodes_hashes_and_signs() {
    let _discriminant = ChainDiscriminantGuard::enter(FIXTURE_DISCRIMINANT);
    let current_fixtures = fixtures("transaction_fixtures.manifest.json");
    let dense_fixtures = fixtures("transaction_fixtures.dense.json");
    assert_eq!(dense_fixtures.len(), 27);
    let key = fixture_key();
    let (_, secret) = key.private_key().to_bytes();
    let mut instructions_checked = 0_usize;
    for entry in &dense_fixtures {
        let name = entry["name"].as_str().expect("fixture name");
        let twin = current_fixtures
            .iter()
            .find(|candidate| candidate["name"] == entry["name"])
            .unwrap_or_else(|| panic!("{name}: current twin"));
        let dense_payload = bytes(entry, "payload_base64");
        let dense_signed = bytes(entry, "signed_base64");
        let current_payload = bytes(twin, "payload_base64");
        let current_signed = bytes(twin, "signed_base64");

        // Typed decode in each layout yields the same payload, re-encoded byte for byte.
        let builder = decode_payload(&dense_payload, dense(FORMAT_BARE))
            .unwrap_or_else(|error| panic!("{name}: dense payload: {error:?}"));
        let twin_builder = decode_payload(&current_payload, current(FORMAT_FRAMED))
            .unwrap_or_else(|error| panic!("{name}: current payload: {error:?}"));
        assert_eq!(
            builder.payload(),
            twin_builder.payload(),
            "{name}: semantic drift"
        );
        assert_eq!(
            encode_archive(builder.payload(), dense(FORMAT_BARE)).unwrap(),
            dense_payload,
            "{name}"
        );
        let dense_frame = encode_archive(builder.payload(), dense(FORMAT_FRAMED)).unwrap();
        assert_eq!(
            decode_payload(&dense_frame, dense(FORMAT_FRAMED))
                .unwrap()
                .payload(),
            builder.payload(),
            "{name}"
        );

        // The dense signing hash is the fixture's, and deterministic signing reproduces it.
        let signing_hash = HashOf::try_new_with_layout(builder.payload(), WireLayout::Dense)
            .unwrap_or_else(|error| panic!("{name}: dense hash: {error}"));
        assert_eq!(
            hex::encode(signing_hash.as_ref()),
            entry["signing_hash"].as_str().unwrap(),
            "{name}: signing hash"
        );
        let network = network_literal(builder.payload().network_id().expect("network"));
        let (status, signed) =
            sign_ffi(&network, &dense_payload, LAYOUT_DENSE, FORMAT_BARE, &secret);
        assert_eq!(status, 0, "{name}: dense signing");
        assert_eq!(
            signed, dense_signed,
            "{name}: deterministic dense signature"
        );
        let (status, signed) = sign_ffi(
            &network,
            &current_payload,
            LAYOUT_CURRENT,
            FORMAT_FRAMED,
            &secret,
        );
        assert_eq!(status, 0, "{name}: current signing");
        assert_eq!(
            signed, current_signed,
            "{name}: current signature unchanged"
        );

        let transaction = decode_signed(&dense_signed, dense(FORMAT_BARE))
            .unwrap_or_else(|error| panic!("{name}: dense signed: {error:?}"));
        transaction
            .signature()
            .payload()
            .verify(key.public_key(), signing_hash.as_ref())
            .unwrap_or_else(|error| panic!("{name}: dense signature: {error}"));
        assert_eq!(
            encode_signed(&transaction, dense(FORMAT_BARE)).unwrap(),
            dense_signed
        );

        // Typed JSON matches the current twin's apart from the layout-bound signature.
        let (status, dense_json) = decode_json_ffi(&dense_signed, LAYOUT_DENSE, FORMAT_BARE);
        assert_eq!(status, 0, "{name}: dense JSON");
        let (status, current_json) =
            decode_json_ffi(&current_signed, LAYOUT_CURRENT, FORMAT_FRAMED);
        assert_eq!(status, 0, "{name}: current JSON");
        assert_eq!(
            json_without_signature(&dense_json),
            json_without_signature(&current_json),
            "{name}: typed JSON drift"
        );

        // Every instruction transcodes between its current frame and its bare dense payload.
        for instruction in builder.payload().instructions.explicit_instructions() {
            let (wire_id, frame) =
                instruction_payload_with_layout(instruction, WireLayout::Compact).unwrap();
            let (_, bare) =
                instruction_payload_with_layout(instruction, WireLayout::Dense).unwrap();
            assert!(bare.len() < frame.len(), "{name}/{wire_id}");
            assert_eq!(
                transcode_ffi(wire_id, &frame, LAYOUT_CURRENT, LAYOUT_DENSE),
                (0, bare.clone()),
                "{name}/{wire_id}: to dense"
            );
            assert_eq!(
                transcode_ffi(wire_id, &bare, LAYOUT_DENSE, LAYOUT_CURRENT),
                (0, frame.clone()),
                "{name}/{wire_id}: to current"
            );
            for (payload, from) in [(&frame, LAYOUT_CURRENT), (&bare, LAYOUT_DENSE)] {
                assert_eq!(
                    transcode_ffi(wire_id, payload, from, from),
                    (0, payload.clone())
                );
            }
            // A payload is never read in the layout it was not selected for.
            assert_eq!(
                transcode_ffi(wire_id, &bare, LAYOUT_CURRENT, LAYOUT_DENSE).0,
                ERR_INSTRUCTION_PAYLOAD,
                "{name}/{wire_id}"
            );
            assert_eq!(
                transcode_ffi(wire_id, &frame, LAYOUT_DENSE, LAYOUT_CURRENT).0,
                ERR_INSTRUCTION_PAYLOAD,
                "{name}/{wire_id}"
            );
            instructions_checked += 1;
        }

        // Wrong-layout and wrong-framing selections are refused.
        for (bytes, options) in [
            (&dense_signed, current(FORMAT_BARE)),
            (&dense_signed, current(FORMAT_FRAMED)),
            (&dense_signed, current(FORMAT_VERSIONED)),
            (&dense_signed, dense(FORMAT_FRAMED)),
            (&current_signed, dense(FORMAT_FRAMED)),
            (&current_signed, dense(FORMAT_BARE)),
        ] {
            assert!(
                decode_signed(bytes, options).is_err(),
                "{name}: {options:?}"
            );
        }
        assert!(decode_payload(&dense_payload, current(FORMAT_BARE)).is_err());
        assert!(decode_payload(&dense_frame, current(FORMAT_FRAMED)).is_err());
        assert!(decode_payload(&current_payload, dense(FORMAT_FRAMED)).is_err());
        assert_eq!(
            decode_json_ffi(&dense_signed, LAYOUT_CURRENT, FORMAT_BARE).0,
            ERR_TRANSACTION_ARCHIVE,
            "{name}"
        );
    }
    assert!(
        instructions_checked >= 27,
        "every fixture carries instructions"
    );
}

#[test]
fn archives_refuse_truncation_suffixes_unknown_selectors_and_keep_the_caller_layout() {
    let entry = fixtures("transaction_fixtures.dense.json").remove(0);
    let signed = bytes(&entry, "signed_base64");
    let options = dense(FORMAT_BARE);
    let transaction = decode_signed(&signed, options).unwrap();
    for ambient in [WireLayout::Compact, WireLayout::Dense] {
        let _caller = norito::core::DecodeFlagsGuard::enter(ambient.code());
        decode_signed(&signed, options).unwrap();
        assert_eq!(norito::core::get_decode_flags(), ambient.code());
        assert!(decode_signed(&signed[..signed.len() - 1], options).is_err());
        assert_eq!(norito::core::get_decode_flags(), ambient.code());
    }
    let mut suffixed = signed.clone();
    suffixed.push(0);
    assert!(decode_signed(&suffixed, options).is_err());
    assert!(decode_signed(&[], options).is_err());
    for selected in [LAYOUT_CURRENT, LAYOUT_DENSE] {
        let framed = Options::signed(selected, FORMAT_FRAMED).unwrap();
        let frame = encode_signed(&transaction, framed).unwrap();
        assert_eq!(decode_signed(&frame, framed).unwrap(), transaction);
        let mut unknown_minor = frame.clone();
        unknown_minor[5] = 2;
        assert!(decode_signed(&unknown_minor, framed).is_err());
        assert!(decode_signed(&frame[..frame.len() - 1], framed).is_err());
    }
    // Only the current layout names the versioned submission wire; no selector is guessed.
    assert!(Options::signed(LAYOUT_DENSE, FORMAT_VERSIONED).is_err());
    assert!(Options::payload(LAYOUT_CURRENT, FORMAT_VERSIONED).is_err());
    for (layout_code, format_code) in [(2, FORMAT_BARE), (0xff, FORMAT_BARE), (LAYOUT_DENSE, 3)] {
        assert!(Options::signed(layout_code, format_code).is_err());
        assert_eq!(
            decode_json_ffi(&signed, layout_code, format_code).0,
            ERR_WIRE_LAYOUT
        );
    }
    assert!(layout(2).is_err());
    let versioned = transaction.encode_versioned();
    assert_eq!(
        decode_json_ffi(&versioned, LAYOUT_DENSE, FORMAT_VERSIONED).0,
        ERR_WIRE_LAYOUT
    );
}

#[test]
fn transcoder_refuses_unregistered_ids_noncanonical_payloads_and_null_inputs() {
    let _discriminant = ChainDiscriminantGuard::enter(FIXTURE_DISCRIMINANT);
    let entry = fixtures("transaction_fixtures.dense.json")
        .into_iter()
        .find(|entry| entry["name"].as_str() == Some("transfer_asset"))
        .expect("transfer fixture");
    let builder = decode_payload(&bytes(&entry, "payload_base64"), dense(FORMAT_BARE)).unwrap();
    let instruction = builder
        .payload()
        .instructions
        .explicit_instructions()
        .next()
        .expect("transfer instruction");
    let (wire_id, bare) = instruction_payload_with_layout(instruction, WireLayout::Dense).unwrap();
    let (_, frame) = instruction_payload_with_layout(instruction, WireLayout::Compact).unwrap();
    assert_eq!(
        transcode_ffi(
            "test.unreleased.instruction.v1",
            &bare,
            LAYOUT_DENSE,
            LAYOUT_CURRENT
        )
        .0,
        ERR_INSTRUCTION_UNREGISTERED
    );
    let mut suffixed = bare.clone();
    suffixed.push(0);
    let mut noncanonical_frame = frame.clone();
    let last = noncanonical_frame.len() - 1;
    noncanonical_frame[last] ^= 1;
    for (payload, from) in [
        (&bare[..bare.len() - 1], LAYOUT_DENSE),
        (suffixed.as_slice(), LAYOUT_DENSE),
        (&[][..], LAYOUT_DENSE),
        (noncanonical_frame.as_slice(), LAYOUT_CURRENT),
    ] {
        assert_eq!(
            transcode_ffi(wire_id, payload, from, LAYOUT_DENSE).0,
            ERR_INSTRUCTION_PAYLOAD
        );
    }
    // Another registered instruction's wire ID never reinterprets the payload.
    assert_ne!(
        transcode_ffi("iroha.mint", &bare, LAYOUT_DENSE, LAYOUT_CURRENT).0,
        0
    );
    assert_eq!(
        transcode_ffi(wire_id, &bare, LAYOUT_DENSE, 2).0,
        ERR_WIRE_LAYOUT
    );
    let mut out_ptr: *mut c_uchar = ptr::null_mut();
    let mut out_len: c_ulong = 0;
    let status = unsafe {
        connect_norito_transcode_instruction_v1(
            ptr::null(),
            0,
            bare.as_ptr(),
            bare.len() as c_ulong,
            LAYOUT_DENSE,
            LAYOUT_CURRENT,
            &mut out_ptr,
            &mut out_len,
        )
    };
    assert_eq!(status, ERR_NULL_PTR);
    let status = unsafe {
        connect_norito_transcode_instruction_v1(
            wire_id.as_ptr().cast(),
            wire_id.len() as c_ulong,
            bare.as_ptr(),
            bare.len() as c_ulong,
            LAYOUT_DENSE,
            LAYOUT_CURRENT,
            ptr::null_mut(),
            &mut out_len,
        )
    };
    assert_eq!(status, ERR_NULL_PTR);
}

#[test]
fn payload_signer_binds_network_key_algorithm_and_exact_selection() {
    let _discriminant = ChainDiscriminantGuard::enter(FIXTURE_DISCRIMINANT);
    let entry = fixtures("transaction_fixtures.dense.json").remove(0);
    let payload = bytes(&entry, "payload_base64");
    let builder = decode_payload(&payload, dense(FORMAT_BARE)).unwrap();
    let network = network_literal(builder.payload().network_id().unwrap());
    let other_network = network_literal(&NetworkId::from_genesis_hash(
        HashOf::from_untyped_unchecked(iroha_crypto::Hash::new(b"another genesis")),
    ));
    assert_ne!(other_network, network);
    let (_, secret) = fixture_key().private_key().to_bytes();
    let (status, _) = sign_ffi(&network, &payload, LAYOUT_DENSE, FORMAT_BARE, &secret);
    assert_eq!(status, 0);
    for (selected_network, selected_layout, selected_format, selected_key) in [
        (
            other_network.as_str(),
            LAYOUT_DENSE,
            FORMAT_BARE,
            secret.clone(),
        ),
        (network.as_str(), LAYOUT_DENSE, FORMAT_BARE, vec![7; 32]),
        (
            network.as_str(),
            LAYOUT_CURRENT,
            FORMAT_BARE,
            secret.clone(),
        ),
        (
            network.as_str(),
            LAYOUT_DENSE,
            FORMAT_FRAMED,
            secret.clone(),
        ),
        (
            network.as_str(),
            LAYOUT_DENSE,
            FORMAT_VERSIONED,
            secret.clone(),
        ),
        (
            network.as_str(),
            LAYOUT_CURRENT,
            FORMAT_VERSIONED,
            secret.clone(),
        ),
    ] {
        let (status, _) = sign_ffi(
            selected_network,
            &payload,
            selected_layout,
            selected_format,
            &selected_key,
        );
        assert_ne!(
            status, 0,
            "{selected_network} {selected_layout} {selected_format}"
        );
    }
}
