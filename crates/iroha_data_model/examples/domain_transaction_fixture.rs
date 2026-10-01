//! Emit native domain-instruction frames for cross-library SDK wire tests.

use std::fmt::Write as _;

use iroha_crypto::{Algorithm, KeyPair, PrivateKey};
use iroha_data_model::{
    account::{AccountId, address::ChainDiscriminantGuard},
    isi::{
        InstructionBox, RemoveKeyValue, SetKeyValue, Transfer, decode_instruction_from_pair,
        frame_instruction_payload, framed_instruction_payload,
    },
};
use iroha_model_base::domain::DomainId;
use norito::{json, json::Value};

// Fixture v1 binds every bare payload to Norito v1 COMPACT_LEN (0x02).
// Full instruction frames also carry their own exact schema and layout headers.
const LAYOUT_FLAGS: u8 = 0x02;

fn hex(bytes: &[u8]) -> String {
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(result, "{byte:02x}").expect("write into String");
    }
    result
}

// Keep the framing/checksum valid when reproducing each managed encoding defect.
fn reject_retired_payload_shapes(wire_id: &str, domain: &str, frame: &[u8]) {
    let header = norito::core::Header::read(frame).expect("native header");
    let payload = &frame[frame.len() - header.length as usize..];
    assert_eq!(&payload[..4], &[0_u8; 4]);
    let (variant_len, variant_prefix) =
        norito::core::read_len_from_slice_with_flags(&payload[4..], LAYOUT_FLAGS).unwrap();
    let fields = &payload[4 + variant_prefix..];
    assert_eq!(variant_len, fields.len());
    let mut flattened = vec![0_u8; 4];
    flattened.extend_from_slice(fields);
    let bad = frame_instruction_payload(wire_id, &flattened).unwrap();
    assert!(decode_instruction_from_pair(wire_id, &bad).is_err());

    let domain_offset = if wire_id == "iroha.transfer" {
        let (len, prefix) =
            norito::core::read_len_from_slice_with_flags(fields, LAYOUT_FLAGS).unwrap();
        len + prefix
    } else {
        0
    };
    let (len, prefix) =
        norito::core::read_len_from_slice_with_flags(&fields[domain_offset..], LAYOUT_FLAGS)
            .unwrap();
    let bare_name = norito::codec::encode_adaptive(&domain.to_owned());
    let mut old_fields = fields[..domain_offset].to_vec();
    norito::core::write_len_to_vec_with_flags(
        &mut old_fields,
        bare_name.len() as u64,
        LAYOUT_FLAGS,
    );
    old_fields.extend_from_slice(&bare_name);
    old_fields.extend_from_slice(&fields[domain_offset + prefix + len..]);
    let mut old_domain = vec![0_u8; 4];
    norito::core::write_len_to_vec_with_flags(
        &mut old_domain,
        old_fields.len() as u64,
        LAYOUT_FLAGS,
    );
    old_domain.extend_from_slice(&old_fields);
    let bad = frame_instruction_payload(wire_id, &old_domain).unwrap();
    assert!(decode_instruction_from_pair(wire_id, &bad).is_err());
}

fn case(name: &str, domain: &str, instruction: InstructionBox) -> Value {
    let (wire_id, frame) =
        framed_instruction_payload(&instruction).expect("registered instruction");
    let decoded = decode_instruction_from_pair(wire_id, &frame).expect("native instruction decode");
    assert_eq!(decoded, instruction);
    let instruction_box = norito::to_bytes(&instruction).expect("native InstructionBox frame");
    let decoded_box: InstructionBox =
        norito::decode_from_bytes(&instruction_box).expect("native InstructionBox decode");
    assert_eq!(decoded_box, instruction);
    let header = norito::core::Header::read(frame.as_slice()).expect("native frame header");
    assert_eq!(header.flags, LAYOUT_FLAGS);
    reject_retired_payload_shapes(wire_id, domain, &frame);
    let bare = norito::codec::encode_adaptive(&instruction);
    json!({
        "name": name,
        "domain_id": domain,
        "wire_id": wire_id,
        "framed_instruction_hex": (hex(&frame)),
        "instruction_box_payload_hex": (hex(&bare)),
        "instruction_box_frame_hex": (hex(&instruction_box)),
    })
}

fn fixture() -> Value {
    assert_eq!(norito::core::default_encode_flags(), LAYOUT_FLAGS);
    let _guard = ChainDiscriminantGuard::enter(753);
    // Public disposable inputs used only to construct fixture account identities.
    let source = AccountId::new(
        KeyPair::from_private_key(PrivateKey::from_bytes(Algorithm::Ed25519, &[0x31; 32]).unwrap())
            .unwrap()
            .public_key()
            .clone(),
    );
    let destination = AccountId::new(
        KeyPair::from_private_key(PrivateKey::from_bytes(Algorithm::Ed25519, &[0x32; 32]).unwrap())
            .unwrap()
            .public_key()
            .clone(),
    );
    let maximum = format!("{}.{}", "a".repeat(63), "b".repeat(63));
    let domains = [
        "banka.universal",
        "xn--bcher-kva.universal",
        "bank_a.universal",
        &maximum,
    ];
    let mut cases = Vec::new();
    for text in domains {
        let domain = DomainId::parse_fully_qualified(text).expect("canonical fixture domain");
        assert_eq!(domain.to_string(), text);
        cases.push(case(
            "TransferDomain",
            text,
            Transfer::domain(source.clone(), domain.clone(), destination.clone()).into(),
        ));
        cases.push(case(
            "SetDomainKeyValue",
            text,
            SetKeyValue::domain(domain.clone(), "memo".parse().unwrap(), "fixture metadata").into(),
        ));
        cases.push(case(
            "RemoveDomainKeyValue",
            text,
            RemoveKeyValue::domain(domain, "memo".parse().unwrap()).into(),
        ));
    }
    json!({
        "fixture_version": 1,
        "norito_layout_version": 1,
        "norito_layout_flags": LAYOUT_FLAGS,
        "generator": "iroha_data_model/examples/domain_transaction_fixture.rs",
        "authority": (source.to_string()),
        "destination": (destination.to_string()),
        "metadata_key": "memo",
        "metadata_value": "fixture metadata",
        "cases": cases,
    })
}

fn main() {
    println!(
        "{}",
        json::to_json_pretty(&fixture()).expect("render native fixture")
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_twelve_native_instruction_pairs_and_boxes_roundtrip() {
        let generated = fixture();
        assert_eq!(
            generated.get("cases").unwrap().as_array().unwrap().len(),
            12
        );
    }

    #[test]
    fn declared_layout_matches_native_default() {
        assert_eq!(LAYOUT_FLAGS, norito::core::default_encode_flags());
        assert_eq!(LAYOUT_FLAGS, 0x02);
    }

    #[test]
    fn native_generation_is_deterministic() {
        assert_eq!(fixture(), fixture());
    }
}
