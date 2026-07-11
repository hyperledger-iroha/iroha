#![no_main]

use ivm::numeric_tlv;
use libfuzzer_sys::fuzz_target;

const MAX_ENVELOPE_BYTES: usize = 148;

fuzz_target!(|data: &[u8]| {
    let envelope = &data[..data.len().min(MAX_ENVELOPE_BYTES)];

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
