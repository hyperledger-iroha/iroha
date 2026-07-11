#![no_main]

use ivm::numeric_tlv::{self, MAX_QUANTITY_ENVELOPE_BYTES_V1};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Feed the complete attacker-controlled slice. Truncating it at the
    // largest valid envelope size would turn an oversized/trailing-data input
    // into a different potentially valid message and leave the rejection path
    // unfuzzed.
    let envelope = data;

    if envelope.len() > MAX_QUANTITY_ENVELOPE_BYTES_V1 {
        assert!(numeric_tlv::decode_int_bytes(envelope).is_err());
        assert!(numeric_tlv::decode_decimal_bytes(envelope).is_err());
        assert!(numeric_tlv::decode_quantity_bytes(envelope).is_err());
        return;
    }

    if let Ok(value) = numeric_tlv::decode_int_bytes(envelope) {
        let canonical = numeric_tlv::encode_int(&value).expect("decoded int must re-encode");
        assert_eq!(canonical, envelope);
        assert_eq!(numeric_tlv::decode_int_bytes(&canonical), Ok(value));
    }

    if let Ok(value) = numeric_tlv::decode_decimal_bytes(envelope) {
        let canonical =
            numeric_tlv::encode_decimal(&value).expect("decoded decimal must re-encode");
        assert_eq!(canonical, envelope);
        assert_eq!(numeric_tlv::decode_decimal_bytes(&canonical), Ok(value));
    }

    if let Ok(value) = numeric_tlv::decode_quantity_bytes(envelope) {
        let canonical =
            numeric_tlv::encode_quantity(&value).expect("decoded quantity must re-encode");
        assert_eq!(canonical, envelope);
        assert_eq!(numeric_tlv::decode_quantity_bytes(&canonical), Ok(value));
    }
});
