//! Both closed Check wire variants and the exact typed generated-record seed producer.

use super::generated_identity_values::pin_outbox_checks;
use super::*;

#[test]
fn pin_outbox_check_both_variants_roundtrip_with_explicit_identity() {
    use norito::NoritoSchema;
    assert_eq!(
        CheckMusubiPinOutboxV1::nominal_name(),
        "iroha_data_model::isi::musubi::CheckMusubiPinOutboxV1"
    );
    for value in pin_outbox_checks() {
        value.validate().unwrap();
        let bytes = norito::encode_canonical(&value).unwrap();
        assert!(bytes.len() <= MUSUBI_PIN_OUTBOX_CHECK_MAX_BYTES_V1);
        assert_eq!(
            norito::decode_canonical::<CheckMusubiPinOutboxV1>(&bytes).unwrap(),
            value
        );
        let json = norito::json::to_json(&value).unwrap();
        assert_eq!(
            norito::json::from_str::<CheckMusubiPinOutboxV1>(&json).unwrap(),
            value
        );
        let boxed: InstructionBox = value.into();
        let frame = norito::encode_canonical(&boxed).unwrap();
        assert_eq!(
            norito::decode_canonical::<InstructionBox>(&frame).unwrap(),
            boxed
        );
        assert_eq!(
            crate::isi::instruction_wire_id(&boxed),
            Some(CheckMusubiPinOutboxV1::WIRE_ID)
        );
    }
}

#[test]
fn pin_outbox_check_requires_outer_session_inventory_challenge_and_bound_entire_row() {
    for value in pin_outbox_checks() {
        for field in 0..7 {
            let mut changed = value.clone();
            match field {
                0 => changed.session_id = [0; 32],
                1 => changed.inventory_digest = [0; 32],
                2 => changed.challenge = [0; 32],
                3 => changed.floor.height = 0,
                4 => changed.floor.block_hash = [0; 32],
                5 => {
                    changed.floor.context_id = crate::block::consensus::HeightContextId(
                        iroha_crypto::HashOf::from_untyped_unchecked(
                            iroha_crypto::Hash::prehashed([0; 32]),
                        ),
                    )
                }
                6 => {
                    changed.network_id = crate::NetworkId::from_genesis_hash(
                        iroha_crypto::HashOf::from_untyped_unchecked(
                            iroha_crypto::Hash::prehashed([0; 32]),
                        ),
                    )
                }
                _ => unreachable!(),
            }
            // NetworkId marks prehashed values, so a different canonical network is valid for
            // Absent. Only Present has an inner row whose independently supplied network differs.
            if field == 6 && matches!(changed.expected, MusubiPinOutboxCheckExpectationV1::Absent) {
                continue;
            }
            assert!(changed.validate().is_err(), "field {field}");
        }
    }
    for field in 0..4 {
        let mut changed = pin_outbox_checks()[1].clone();
        let MusubiPinOutboxCheckExpectationV1::Present(row) = &mut changed.expected else {
            unreachable!()
        };
        match field {
            0 => row.session_id = [0xd1; 32],
            1 => row.inventory_digest = [0xd2; 32],
            2 => row.version = 0,
            3 => row.transaction_hash = [0; 32],
            _ => unreachable!(),
        }
        assert!(changed.validate().is_err());
    }
}

#[test]
#[ignore = "explicit current typed seed for both Check record cases before complete capture"]
fn print_native_pin_outbox_check_record_capture_v1() {
    use crate::isi::generated_record_identity_tests::capture;
    use norito::json::{Value, object};
    use std::io::Write;
    let captures = pin_outbox_checks().map(capture);
    let fields = ["frame", "vector_frame", "option_frame", "map_frame"];
    let cases = captures
        .iter()
        .map(|capture| {
            object(fields.map(|field| (field, capture.get(field).expect("typed frame").clone())))
                .unwrap()
        })
        .collect();
    let row = object([
        ("nominal", captures[0].get("nominal").unwrap().clone()),
        (
            "serialize_hash",
            captures[0].get("serialize_hash").unwrap().clone(),
        ),
        (
            "deserialize_hash",
            captures[0].get("deserialize_hash").unwrap().clone(),
        ),
        ("cases", Value::Array(cases)),
    ])
    .unwrap();
    writeln!(
        std::io::stdout().lock(),
        "NATIVE_PIN_OUTBOX_CHECK_RECORD_V1\t{}",
        norito::json::to_json(&row).unwrap()
    )
    .unwrap();
}
